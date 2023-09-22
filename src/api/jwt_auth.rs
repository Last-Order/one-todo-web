use std::env;

use axum::{
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use entity::users;
use jsonwebtoken::{decode, DecodingKey, Validation};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::{model::TokenClaims, AppError, AppState};

pub async fn auth<T>(
    State(app_state): State<AppState>,
    mut req: Request<T>,
    next: Next<T>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_value| {
            if auth_value.starts_with("Bearer ") {
                Some(auth_value[7..].to_owned())
            } else {
                None
            }
        })
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_token",
                message: "",
            }),
        ))?;

    let claims = decode::<TokenClaims>(
        &token,
        &DecodingKey::from_secret(
            env::var("JWT_SECRET")
                .expect("JWT_SECRET is not set in .env file")
                .as_ref(),
        ),
        &Validation::default(),
    )
    .map_err(|_| {
        // invalid token or token expired
        (
            StatusCode::UNAUTHORIZED,
            Json(AppError {
                code: "need_login",
                message: "",
            }),
        )
    })?
    .claims;

    let user = users::Entity::find()
        .filter(users::Column::Email.eq(&claims.sub))
        .one(&app_state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "invalid_jwt_user",
                    message: "",
                }),
            )
        })?
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_user",
                message: "",
            }),
        ))?;

    req.extensions_mut().insert(user);

    Ok(next.run(req).await)
}
