use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use chrono::Duration;
use entity::users;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use crate::services::{
    extract_history::count_extract_history, subscription::get_valid_subscription,
};

use super::{AppError, AppState};

pub async fn get_user_profile(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let user = users::Entity::find()
        .filter(users::Column::Id.eq(user.id))
        // .into_json()
        .one(&state.conn)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "",
                }),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "user_not_found",
                message: "",
            }),
        ))?;

    let subscription = get_valid_subscription(&state, &user).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "database_error",
                message: "",
            }),
        )
    })?;

    let mut quota = 10; // Free plan
    let mut period_start_time = chrono::Utc::now() - Duration::seconds(60 * 60 * 24 * 31);

    if subscription.is_some() {
        quota = subscription.as_ref().unwrap().quota;
        period_start_time = subscription.as_ref().unwrap().start_time;
    }

    let extract_count = count_extract_history(&state, &user, period_start_time, chrono::Utc::now())
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, Json(err)))?;

    Ok(Json(json!({
        "first_name": user.first_name,
        "last_name": user.last_name,
        "avatar": user.avatar,
        "subscription": json!({
            "quota": quota,
            "used_count": extract_count
        })
    })))
}
