use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use entity::users;
use serde_json::json;

use crate::services::subscription::get_user_quota_and_subscription;

use super::{constants::SubscriptionType, AppError, AppState};

pub async fn get_user_profile(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let user_quota_and_subscription = get_user_quota_and_subscription(&state, &user)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err))
        })?;

    let subscription_info;

    if user_quota_and_subscription.subscription.is_some() {
        let subscription = user_quota_and_subscription.subscription.unwrap();
        let subscription_type: SubscriptionType = subscription.r#type.try_into().map_err(|_| {
            sentry::capture_message(
                "Failed to convert subscription type to text.",
                sentry::Level::Error,
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "invalid_subscription_type",
                    message: "Invalid subscription type.",
                }),
            )
        })?;
        let subscription_name: String = subscription_type.into();

        subscription_info = json!({
            "subscription_name": subscription_name,
            "subscription_type": subscription.r#type,
            "start_time": subscription.start_time,
            "end_time": subscription.end_time
        });
    } else {
        let subscription_name: String = SubscriptionType::Free.into();
        subscription_info = json!({
            "subscription_name": subscription_name,
        });
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
