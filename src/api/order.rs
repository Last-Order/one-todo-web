use std::env;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use entity::users;
use lemon_squeezy::GenerateCheckoutUrlParams;
use serde_json::json;

use super::{AppError, AppState};

pub async fn crate_order(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
    // extract::Json(params): extract::Json<UpdateEventStatusPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let email = user.email;
    let client = lemon_squeezy::LemonSqueezy::new(
        env::var("LEMON_SQUEEZY_API_KEY").expect("LEMON_SQUEEZY_API_KEY is not set in .env file"),
    );
    let checkout_url = client
        .generate_checkout_url(GenerateCheckoutUrlParams { email: Some(email) })
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
    Ok(Json(json!({ "checkout_url": checkout_url })))
}
