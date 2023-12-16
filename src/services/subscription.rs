use std::env;

use axum::extract::State;
use chrono::Duration;
use entity::{user_subscriptions, users};
use lemon_squeezy::SubscriptionStatus;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter,
};

use crate::api::{constants::SubscriptionType, AppError, AppState};

use super::extract_history::count_extract_history;

pub async fn get_valid_subscription(
    app_state: &State<AppState>,
    user: &users::Model,
) -> Result<Option<user_subscriptions::Model>, AppError> {
    let result = user_subscriptions::Entity::find()
        .filter(
            Condition::all()
                .add(user_subscriptions::Column::Status.eq(SubscriptionStatus::Active.to_string()))
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
            message: "Please try again later.",
        })?;

    let mut quota = 10; // Free plan
    let mut period_start_time = chrono::Utc::now() - Duration::seconds(60 * 60 * 24 * 31);

    if subscription.is_some() {
        let subscription_start_time = subscription.as_ref().unwrap().start_time;
        quota = subscription.as_ref().unwrap().quota;
        if subscription_start_time > period_start_time {
            // 如果订阅开始时间在31天内 则以订阅开始时间为准 之前的调用不算
            period_start_time = subscription_start_time;
        }
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

/** 和LemonSqueezy同步订阅信息 需要给定LemonSqueezy的订阅ID */
pub async fn sync_subscription_status_with_lemon_squeezy(
    app_state: &State<AppState>,
    user: users::Model,
    external_subscription_id: i32,
) -> Result<(), AppError> {
    // let user_active_subscription = get_valid_subscription(app_state, &user).await?;
    let client = lemon_squeezy::LemonSqueezy::new(
        env::var("LEMON_SQUEEZY_API_KEY").expect("LEMON_SQUEEZY_API_KEY is not set in .env file"),
    );

    // remote subscription information
    let subscription = client
        .get_subscription(external_subscription_id)
        .await
        .map_err(|err| {
            sentry::integrations::anyhow::capture_anyhow(&err);
            AppError {
                code: "failed_to_sync_subscriptions",
                message: "failed_to_sync_subscriptions",
            }
        })?;

    let subscription_start_time = subscription
        .attributes
        .created_at
        .parse::<chrono::DateTime<chrono::Utc>>()
        .map_err(|err| {
            sentry::capture_error(&err);
            AppError {
                code: "invalid_subscription_start_time",
                message: "invalid_subscription_start_time",
            }
        })?;
    let subscription_renews_at = subscription
        .attributes
        .renews_at
        .parse::<chrono::DateTime<chrono::Utc>>()
        .map_err(|err| {
            sentry::capture_error(&err);
            AppError {
                code: "invalid_subscription_renews_at",
                message: "invalid_subscription_renews_at",
            }
        })?;
    let subscription_ends_at = subscription
        .attributes
        .ends_at
        .clone()
        .map(|ends_at| ends_at.parse::<chrono::DateTime<chrono::Utc>>().unwrap());

    // user subscription information in database

    let user_subscription = user_subscriptions::Entity::find()
        .filter(user_subscriptions::Column::ExternalSubscriptionId.eq(external_subscription_id))
        .one(&app_state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            AppError {
                code: "database_error",
                message: "",
            }
        })?;

    if user_subscription.is_none() {
        let new_subscription = user_subscriptions::ActiveModel {
            user_id: Set(user.id),
            start_time: Set(subscription_start_time),
            renews_at: Set(subscription_renews_at),
            ends_at: Set(subscription_ends_at),
            // maybe multiple plans in the future
            product_id: Set(subscription.attributes.product_id),
            variant_id: Set(subscription.attributes.variant_id),
            status: Set(subscription.attributes.status.to_string()),
            external_subscription_id: Set(subscription.id.clone()),
            r#type: Set(SubscriptionType::Pro as i32),
            quota: Set(125),
            ..Default::default()
        };

        let _ = new_subscription
            .save(&app_state.conn)
            .await
            .map_err(|err| {
                sentry::capture_error(&err);
                AppError {
                    code: "database_error",
                    message: "",
                }
            })?;
    } else {
        // update user subscription with remote information
        let mut modified_subscription: user_subscriptions::ActiveModel =
            user_subscription.unwrap().into();

        modified_subscription.start_time = Set(subscription_start_time);
        modified_subscription.renews_at = Set(subscription_renews_at);
        modified_subscription.ends_at = Set(subscription_ends_at);
        modified_subscription.status = Set(subscription.attributes.status.to_string());
        modified_subscription.external_subscription_id = Set(subscription.id.clone());

        let _ = modified_subscription
            .save(&app_state.conn)
            .await
            .map_err(|err| {
                sentry::capture_error(&err);
                AppError {
                    code: "database_error",
                    message: "",
                }
            })?;
    }

    Ok(())
}
