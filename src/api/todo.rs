use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::prelude::*;
use entity::{prelude::*, todos, users};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;

use super::{AppError, AppState};

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
