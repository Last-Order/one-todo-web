use std::env;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use entity::{orders, users};
use lemon_squeezy::CreateOrderParams;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::json;

use super::{constants::OrderStatus, AppError, AppState};

pub async fn crate_order(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
    // extract::Json(params): extract::Json<UpdateEventStatusPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let email = user.email;
    let client = lemon_squeezy::LemonSqueezy::new(
        env::var("LEMON_SQUEEZY_API_KEY").expect("LEMON_SQUEEZY_API_KEY is not set in .env file"),
    );

    let create_order_result = client
        .create_order(CreateOrderParams { email: Some(email) })
        .await
        .map_err(|err| {
            sentry::integrations::anyhow::capture_anyhow(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "failed_to_create_order",
                    message: "Failed to create order. Please try again later.",
                }),
            )
        })?;

    let new_order = orders::ActiveModel {
        user_id: Set(user.id),
        order_id: Set(create_order_result.order_id),
        status: Set(OrderStatus::Created as i32),
        ..Default::default()
    };

    let result = new_order.insert(&state.conn).await.map_err(|err| {
        sentry::capture_error(&err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "database_error",
                message: "Failed to create order. Please try again later.",
            }),
        )
    })?;

    Ok(Json(json!({
        "checkout_url": create_order_result.checkout_url,
        "order_status": result.status
    })))
}
