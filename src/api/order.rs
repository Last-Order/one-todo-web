use std::env;

use axum::{
    extract::{self, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use entity::{orders, users};
use lemon_squeezy::CreateOrderParams as LemonSqueezyCreateOrderParams;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{constants::OrderStatus, AppError, AppState};

#[derive(Serialize, Deserialize)]
pub struct CreateOrderParams {
    /**
     * Redirect URL to chrome extension. Different from the redirect url passing to LemonSqueezy.
     * The backend will record this redirect url and assign it with the `internal_order_id`.
     * The redirect chain after success payment:
     * LemonSqueezy -- Redirect --> Backend -- Redirect --> Chrome Extension
     */
    redirect_url: Option<String>,
}

pub async fn crate_order(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<CreateOrderParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let email = user.email;
    let redirect_url = params.redirect_url.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_redirect_url",
            message: "",
        }),
    ))?;
    let client = lemon_squeezy::LemonSqueezy::new(
        env::var("LEMON_SQUEEZY_API_KEY").expect("LEMON_SQUEEZY_API_KEY is not set in .env file"),
    );

    let internal_order_id = uuid::Uuid::new_v4().to_string();

    let app_endpoint = env::var("APP_ENDPOINT").expect("APP_ENDPOINT is not set in .env file");
    let create_order_result = client
        .create_order(LemonSqueezyCreateOrderParams {
            email: Some(email),
            store_id: 43821,
            variant_id: 138344,
            redirect_url: format!(
                "{}/order/checkout_callback?internal_order_id={}",
                app_endpoint, internal_order_id
            ),
        })
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
        internal_order_id: Set(internal_order_id.clone()),
        redirect_url: Set(format!("{}/{}", redirect_url, internal_order_id.clone())),
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
