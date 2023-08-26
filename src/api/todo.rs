use axum::{
    extract::{self, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::prelude::*;
use entity::{prelude::*, todos, users};
use regex::Regex;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

use super::{AppError, AppState};
use crate::services::{extract_history, openai};
#[derive(Deserialize)]
pub struct GetUpcomingEventPayload {
    current_time: Option<String>,
    page: Option<i32>,
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
    let start_of_day = Utc
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
    let result = Todos::find()
        .filter(
            Condition::all()
                .add(todos::Column::UserId.eq(user_id))
                .add(todos::Column::ScheduledTime.gte(start_of_day)),
        )
        .order_by_asc(todos::Column::ScheduledTime)
        .limit(50)
        .into_json()
        .all(&state.conn)
        .await
        .map_err(|_| {
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
pub struct CreateEventPayload {
    current_time: Option<String>,
    description: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct GetCompletionPayload {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct PrepareCreateEventResult {
    name: String,
    event_time: DateTime<Utc>,
}

pub async fn prepare_create_event(
    app_state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<CreateEventPayload>,
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

    let prompt = format!("Your task is to extract event information from the following text. Your answer must include event_time, which is the start time of the event, must in ISO format such as \"2023-01-01T20:00:00Z\" and name which is the name of the event. If the timezone is not provided, you should infer it from the language. If a relative time is provided, you should infer it from the current time, the time now is: {}. If the time exceed 24:00, treat it as the 30時間制 in japan. For example, 26:00 means 2:00 next day. If both start time and end time are provided, use the start time. Your answer must be provided in JSON format and do not include anything else. For example, {{\"name\": \"Event Name\", \"event_time\": \"2023-01-01T20:00:00+0900\"}}. Your answer must be correct and clear. Do not provide any explanation or errors. The text you need to process is: {}", format!("{:?}", current_time), event_description);

    let openai_result = openai::get_completion(&prompt)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, Json(err)))?;

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
        serde_json::from_str(filtered_result.as_str()).map_err(|_| {
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
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, Json(err)))?;

    Ok(Json(PrepareCreateEventResult {
        name: event_name,
        event_time,
    }))
}
