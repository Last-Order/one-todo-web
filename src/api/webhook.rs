use std::env;

use axum::{
    extract::{self, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use entity::{orders, users};
use lemon_squeezy::GetSubscriptionsParams;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::services::subscription::sync_subscription_status_with_lemon_squeezy;

use super::{constants::OrderStatus, AppError, AppState};

#[derive(Serialize, Deserialize)]
pub struct LemonSqueezyWebhookPayload<T> {
    meta: LemonSqueezyWebhookPayloadMeta<T>,
    data: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct LemonSqueezyWebhookPayloadMeta<T> {
    event_name: String,
    custom_data: T,
}

#[derive(Serialize, Deserialize)]
pub struct LemonSqueezyWebhookCustomData {
    pub internal_order_id: Option<String>,
}

pub async fn handle_lemon_squeezy_webhook(
    state: State<AppState>,
    extract::Json(params): extract::Json<LemonSqueezyWebhookPayload<LemonSqueezyWebhookCustomData>>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let event_name = params.meta.event_name.clone();
    let internal_order_id = params.meta.custom_data.internal_order_id.clone();

    if internal_order_id.is_none() {
        sentry::capture_message(
            "Missing internal_order_id for webhook event.",
            sentry::Level::Error,
        );
        return Ok(Json({}));
    }

    let internal_order_id = internal_order_id.unwrap();

    let order = orders::Entity::find()
        .filter(orders::Column::InternalOrderId.eq(internal_order_id))
        .one(&state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "",
                }),
            )
        })?;

    if order.is_none() {
        sentry::capture_message("Corresponding order not found.", sentry::Level::Error);
        return Ok(Json({}));
    }

    let order = order.unwrap();

    match event_name.as_str() {
        "subscription_payment_success" => {
            let result = handle_subscription_payment_success(state, params, order).await;
            return result;
        }
        _ => Ok(Json({})),
    }
}

/**
 * 订阅成功
 * 1. 更新订单状态
 * 2. 同步订阅状态
 */
pub async fn handle_subscription_payment_success(
    state: State<AppState>,
    payload: LemonSqueezyWebhookPayload<LemonSqueezyWebhookCustomData>,
    order: orders::Model,
) -> Result<Json<()>, (StatusCode, Json<AppError>)> {
    if order.status == OrderStatus::Created as i32 {
        let mut order: orders::ActiveModel = order.clone().into();
        order.status = Set(OrderStatus::Finished as i32);
        let _ = order.save(&state.conn).await.map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "",
                }),
            )
        })?;
    }

    let user = users::Entity::find()
        .filter(users::Column::Id.eq(order.user_id))
        .one(&state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
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

    let _ = sync_subscription_status_with_lemon_squeezy(&state, user)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
        });

    Ok(Json(()))
}
