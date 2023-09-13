use axum::extract::State;
use chrono::Duration;
use entity::{user_subscriptions, users};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::api::{AppError, AppState};

use super::extract_history::count_extract_history;

pub async fn get_valid_subscription(
    app_state: &State<AppState>,
    user: &users::Model,
) -> Result<Option<user_subscriptions::Model>, AppError> {
    let current_time = chrono::Utc::now();

    let result = user_subscriptions::Entity::find()
        .filter(
            Condition::all()
                .add(user_subscriptions::Column::StartTime.lt(current_time))
                .add(user_subscriptions::Column::EndTime.gt(current_time))
                .add(user_subscriptions::Column::UserId.eq(user.id)),
        )
        .one(&app_state.conn)
        .await
        .map_err(|_| AppError {
            code: "database_error",
            message: "",
        })?;

    Ok(result)
}

pub struct UserQuotaInfo {
    pub quota: i32,
    pub used_count: i32,
}

pub struct UserQuotaAndSubscriptionInfo {
    pub quota_info: UserQuotaInfo,
    pub subscription: Option<user_subscriptions::Model>,
}

pub async fn get_user_quota_and_subscription(
    app_state: &State<AppState>,
    user: &users::Model,
) -> Result<UserQuotaAndSubscriptionInfo, AppError> {
    let user = users::Entity::find()
        .filter(users::Column::Id.eq(user.id))
        // .into_json()
        .one(&app_state.conn)
        .await
        .map_err(|_| AppError {
            code: "database_error",
            message: "",
        })?
        .ok_or(AppError {
            code: "user_not_found",
            message: "",
        })?;

    let subscription = get_valid_subscription(&app_state, &user)
        .await
        .map_err(|_| AppError {
            code: "database_error",
            message: "",
        })?;

    let mut quota = 10; // Free plan
    let mut period_start_time = chrono::Utc::now() - Duration::seconds(60 * 60 * 24 * 31);

    if subscription.is_some() {
        quota = subscription.as_ref().unwrap().quota;
        period_start_time = subscription.as_ref().unwrap().start_time;
    }

    let extract_count =
        count_extract_history(&app_state, &user, period_start_time, chrono::Utc::now()).await?;

    let result = UserQuotaAndSubscriptionInfo {
        quota_info: UserQuotaInfo {
            used_count: extract_count,
            quota,
        },
        subscription,
    };

    return Ok(result);
}
