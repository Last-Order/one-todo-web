use axum::{
    extract::{self, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::{prelude::*, Duration};
use entity::{todos, users};
use regex::Regex;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, TryIntoModel,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    constants::{SubscriptionType, TodoStatus},
    AppError, AppState,
};
use crate::services::{
    extract_history::{self},
    openai,
    subscription::get_user_quota_and_subscription,
};
#[derive(Deserialize)]
pub struct GetUpcomingEventPayload {
    current_time: Option<String>,
    // page: Option<i32>,
}

pub async fn get_upcoming_events(
    state: State<AppState>,
    Query(params): Query<GetUpcomingEventPayload>,
    Extension(user): Extension<users::Model>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let current_time = params
        .current_time
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_current_time",
                message: "",
            }),
        ))?
        .parse::<DateTime<Local>>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_time",
                    message: "",
                }),
            )
        })?;

    let start_of_day = Local
        .with_ymd_and_hms(
            current_time.year(),
            current_time.month(),
            current_time.day(),
            0,
            0,
            0,
        )
        .unwrap();

    let user_id = user.id;
    let result = todos::Entity::find()
        .filter(
            Condition::all()
                .add(todos::Column::UserId.eq(user_id))
                .add(todos::Column::ScheduledTime.gte(start_of_day))
                .add(todos::Column::Status.ne(TodoStatus::Deleted as i32)),
        )
        .order_by_asc(todos::Column::ScheduledTime)
        .limit(50)
        .into_json()
        .all(&state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Failed to get upcoming events",
                }),
            )
        })?;
    return Ok(Json(result));
}

#[derive(Deserialize)]
pub struct UpdateEventStatusPayload {
    id: Option<i32>,
    status: Option<i32>,
}

pub async fn update_event_status(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<UpdateEventStatusPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let event_id = params.id.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_id",
            message: "",
        }),
    ))?;

    let status: TodoStatus = params
        .status
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_id",
                message: "",
            }),
        ))?
        .try_into()
        .map_err(|err| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: err,
                    message: "",
                }),
            )
        })?;

    let event = todos::Entity::find()
        .filter(
            Condition::all()
                .add(todos::Column::Id.eq(event_id))
                .add(todos::Column::UserId.eq(user.id)),
        )
        // .into_json()
        .one(&state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Failed to update event status.",
                }),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "event_not_found",
                message: "",
            }),
        ))?;

    let mut event: todos::ActiveModel = event.into();

    event.status = Set(status as i32);

    event.save(&state.conn).await.map_err(|err| {
        sentry::capture_error(&err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "database_error",
                message: "Failed to update event status.",
            }),
        )
    })?;

    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct PrepareCreateEventPayload {
    current_time: Option<String>,
    description: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct PrepareCreateEventResult {
    event_name: String,
    scheduled_time: DateTime<Utc>,
}

pub async fn prepare_create_event(
    app_state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<PrepareCreateEventPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    // parameter validation
    let current_time = params
        .current_time
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_current_time",
                message: "",
            }),
        ))?
        .parse::<DateTime<Local>>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_time",
                    message: "",
                }),
            )
        })?;
    let event_description = params.description.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_event_description",
            message: "A description of the event is required.",
        }),
    ))?;

    if event_description.len() > 300 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "description_too_long",
                message: "The description must 300 characters or fewer.",
            }),
        ));
    }

    // check user subscriptions
    let quota_and_subscription_info = get_user_quota_and_subscription(&app_state, &user)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, Json(err)))?;

    if quota_and_subscription_info.quota_info.quota
        <= quota_and_subscription_info.quota_info.used_count
    {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AppError {
                code: "exceed_quota",
                message: "Quota exceed. Please try again later or upgrade your plan.",
            }),
        ));
    }

    // get completion from openai
    let mut selected_model = "gpt-3.5-turbo";

    // apply GPT-4 for paid user
    if let Some(subscription) = quota_and_subscription_info.subscription {
        if subscription.r#type == SubscriptionType::Pro as i32 {
            selected_model = "gpt-4";
        }
    }

    let prompt = format!("Please extract the event details from the given text. The extracted information should include 'event_time' (the event's start time) and 'name' (the event's name).  The 'event_time' should be presented in ISO format, such as \"2023-01-01T20:00:00Z\". If the text is in a foreign language, deduce the timezone from the language used. For instance, if the text is in Japanese, the timezone should correspond to Japan's. Your timezone should be correct. If the text indicates a time beyond 24:00, interpret it according to the 30-hour system used in Japan, where, for example, 26:00 refers to 2:00 on the following day. If both start and end times are given, use the start time. Your response should be in JSON format, like this: {{\"name\": \"Event Name\", \"event_time\": \"2023-01-01T20:00:00Z\"}}. Ensure your response is accurate and straightforward, without including any explanations or error messages. The current time is: {}. Think it carefully and I will tip you $200 if your answer is correct. Please process the following text: {}", format!("{:?}", current_time), event_description);

    let openai_result = openai::get_completion(selected_model, &prompt)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err))
        })?;

    let result = openai_result.replace("\n", "");

    let filtered_regex = Regex::new(r"(?<main>\{.+})").unwrap();
    let filtered_result = filtered_regex.captures(&result).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AppError {
            code: "invalid_completion_response",
            message: "",
        }),
    ))?["main"]
        .to_owned();

    let parsed_result: serde_json::Value =
        serde_json::from_str(filtered_result.as_str()).map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "invalid_completion_response",
                    message: "",
                }),
            )
        })?;

    if parsed_result.get("name").is_none() || parsed_result.get("event_time").is_none() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "failed_to_parse_event",
                message: "Failed to extract event information. Please try another text.",
            }),
        ));
    }

    let event_name = parsed_result
        .get("name")
        .unwrap()
        .as_str()
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_event_time",
                message: "Failed to extract event information. Please try another text.",
            }),
        ))?
        .to_owned();
    let event_time = parsed_result
        .get("event_time")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
        .parse::<DateTime<Utc>>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_event_time",
                    message: "Failed to extract event information. Please try another text.",
                }),
            )
        })?;

    extract_history::record_extract_history(&app_state, &user, &prompt)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err))
        })?;

    Ok(Json(PrepareCreateEventResult {
        event_name,
        scheduled_time: event_time,
    }))
}

#[derive(Serialize, Deserialize)]
pub struct CreateEventPayload {
    event_name: Option<String>,
    scheduled_time: Option<String>,
    remind_time: Option<String>,
    description: Option<String>,
}

pub async fn create_event(
    app_state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<CreateEventPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let scheduled_time = params
        .scheduled_time
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_scheduled_time",
                message: "",
            }),
        ))?
        .parse::<DateTime<Utc>>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_time",
                    message: "",
                }),
            )
        })?;

    let remind_time = params
        .remind_time
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_remind_time",
                message: "",
            }),
        ))?
        .parse::<DateTime<Utc>>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_time",
                    message: "",
                }),
            )
        })?;

    if scheduled_time - remind_time < Duration::seconds(0) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_remind_time",
                message: "Reminder time cannot be later than the event time.",
            }),
        ));
    }

    let event_name = params.event_name.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_event_name",
            message: "A name of the event is required.",
        }),
    ))?;

    let event_description = params.description.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_event_description",
            message: "A description of the event is required.",
        }),
    ))?;

    if event_description.len() > 300 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "description_too_long",
                message: "The description must 300 characters or fewer.",
            }),
        ));
    }

    let result = todos::ActiveModel {
        user_id: Set(user.id),
        event_name: Set(event_name),
        description: Set(Some(event_description)),
        scheduled_time: Set(Some(scheduled_time)),
        remind_time: Set(Some(remind_time)),
        status: Set(TodoStatus::Created as i32),
        ..Default::default()
    }
    .insert(&app_state.conn)
    .await
    .map_err(|err| {
        sentry::capture_error(&err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "database_error",
                message: "Failed to create event. Please try again later.",
            }),
        )
    })?;

    Ok(Json(json!({
        "id": result.id,
        "event_name": result.event_name,
        "description": result.description,
        "scheduled_time": format!("{:?}", result.scheduled_time.unwrap()),
        "remind_time": format!("{:?}", result.remind_time.unwrap()),
        "status": result.status
    })))
}

#[derive(Serialize, Deserialize)]
pub struct UpdateEventPayload {
    id: Option<i32>,
    event_name: Option<String>,
    scheduled_time: Option<String>,
    remind_time: Option<String>,
    description: Option<String>,
}

pub async fn update_event(
    app_state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<UpdateEventPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let id = params.id.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_event_id",
            message: "Missing event id.",
        }),
    ))?;

    let scheduled_time = params
        .scheduled_time
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_scheduled_time",
                message: "",
            }),
        ))?
        .parse::<DateTime<Utc>>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_time",
                    message: "",
                }),
            )
        })?;

    let remind_time = params
        .remind_time
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_remind_time",
                message: "",
            }),
        ))?
        .parse::<DateTime<Utc>>()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_time",
                    message: "",
                }),
            )
        })?;

    let event_name = params.event_name.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_event_name",
            message: "A name of the event is required.",
        }),
    ))?;

    let event_description = params.description.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_event_description",
            message: "A description of the event is required.",
        }),
    ))?;

    if scheduled_time - remind_time < Duration::seconds(0) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_remind_time",
                message: "Reminder time cannot be later than the event time.",
            }),
        ));
    }

    let todo = todos::Entity::find()
        .filter(todos::Column::Id.eq(id))
        .one(&app_state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Failed to update event. Please try again later.",
                }),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "event_not_found",
                message: "Event not found.",
            }),
        ))?;

    if todo.user_id != user.id {
        // permission check
        return Err((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "event_not_found",
                message: "Event not found.",
            }),
        ));
    }

    let mut modified_todo: todos::ActiveModel = todo.into();
    modified_todo.event_name = Set(event_name);
    modified_todo.description = Set(Some(event_description));
    modified_todo.scheduled_time = Set(Some(scheduled_time));
    modified_todo.remind_time = Set(Some(remind_time));

    let result = modified_todo
        .save(&app_state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Failed to update event. Please try again later.",
                }),
            )
        })?
        .try_into_model()
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error_serialization",
                    message: "Failed to update event. Please try again later.",
                }),
            )
        })?;

    Ok(Json(json!({
        "id": result.id,
        "event_name": result.event_name,
        "description": result.description,
        "scheduled_time": format!("{:?}", result.scheduled_time.unwrap()),
        "remind_time": format!("{:?}", result.remind_time.unwrap()),
    })))
}

#[derive(Serialize, Deserialize)]
pub struct DeleteEventPayload {
    id: Option<i32>,
}

pub async fn delete_event(
    app_state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<DeleteEventPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let id = params.id.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_event_id",
            message: "Missing event id.",
        }),
    ))?;

    let todo = todos::Entity::find()
        .filter(todos::Column::Id.eq(id))
        .one(&app_state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Please try again later.",
                }),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "event_not_found",
                message: "Event not found.",
            }),
        ))?;

    if todo.user_id != user.id {
        // permission check
        return Err((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "event_not_found",
                message: "Event not found.",
            }),
        ));
    }

    let mut modified_todo: todos::ActiveModel = todo.into();
    modified_todo.status = Set(TodoStatus::Deleted as i32);

    let _ = modified_todo
        .save(&app_state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Please try again later.",
                }),
            )
        })?
        .try_into_model()
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error_serialization",
                    message: "Please try again later.",
                }),
            )
        })?;

    Ok(Json(json!({})))
}
