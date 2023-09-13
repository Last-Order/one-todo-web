use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use entity::users;
use serde_json::json;

use crate::services::subscription::get_user_quota_and_subscription;

use super::{AppError, AppState};

pub async fn get_user_profile(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let user_quota_and_subscription = get_user_quota_and_subscription(&state, &user)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, Json(err)))?;

    let mut subscription_info = json!({});

    if user_quota_and_subscription.subscription.is_some() {
        let subscription = user_quota_and_subscription.subscription.unwrap();
        subscription_info = json!({
            "type": subscription.r#type,
            "start_time": subscription.start_time,
            "end_time": subscription.end_time
        })
    }

    Ok(Json(json!({
        "first_name": user.first_name,
        "last_name": user.last_name,
        "avatar": user.avatar,
        "quota_info": json!({
            "quota": user_quota_and_subscription.quota_info.quota,
            "used_count": user_quota_and_subscription.quota_info.used_count,
        }),
        "subscription": subscription_info,
    })))
}
