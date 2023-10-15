use std::env;

use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use ring::hmac;

use crate::api::AppError;

pub async fn auth(
    req: Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let content_type = req
        .headers()
        .get("content-type")
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_content_type",
                message: "",
            }),
        ))?
        .to_str()
        .ok();

    if Some("application/json") != content_type {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_content_type",
                message: "",
            }),
        ));
    }

    let signature = req
        .headers()
        .get("x-signature")
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_signature",
                message: "",
            }),
        ))?
        .to_str()
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::BAD_REQUEST,
                Json(AppError {
                    code: "invalid_signature",
                    message: "",
                }),
            )
        })?
        .to_owned();

    let (parts, body) = req.into_parts();

    let hmac_key_string = env::var("LEMON_SQUEEZY_WEBHOOK_SECRET")
        .expect("LEMON_SQUEEZY_WEBHOOK_SECRET is not set in .env file");
    let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, hmac_key_string.as_bytes());
    let body_bytes = hyper::body::to_bytes(body).await.unwrap();
    let expected_signature = hmac::sign(&hmac_key, &body_bytes);
    let expected_signature = expected_signature
        .as_ref()
        .iter()
        .map(|n| format!("{:02x}", n))
        .collect::<String>();

    if expected_signature != signature {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AppError {
                code: "invalid_signature",
                message: "",
            }),
        ));
    }

    let req = Request::from_parts(parts, hyper::Body::from(body_bytes));

    Ok(next.run(req).await)
}
