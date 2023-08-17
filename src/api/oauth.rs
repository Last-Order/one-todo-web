use std::{collections::HashMap, env};

use anyhow::anyhow;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_macros::debug_handler;
use entity::oauth2_state_storage;
use oauth2::{
    basic::BasicClient, reqwest::http_client, AuthUrl, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RevocationUrl, Scope,
    TokenResponse, TokenUrl,
};
use reqwest;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::{AppError, AppState};

fn get_oauth_client() -> Result<BasicClient, anyhow::Error> {
    let google_client_id = ClientId::new(
        env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID is not set in .env file"),
    );
    let google_client_secret = ClientSecret::new(
        env::var("GOOGLE_CLIENT_SECRET").expect("GOOGLE_CLIENT_SECRET is not set in .env file"),
    );
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .map_err(|_| anyhow!("OAuth: invalid authorization endpoint URL"))?;
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
        .map_err(|_| anyhow!("OAuth: invalid token endpoint URL"))?;

    let redirect_url = RedirectUrl::new(
        env::var("GOOGLE_RETURN_URL").expect("GOOGLE_RETURN_URL is not set in .env file"),
    )
    .map_err(|_| anyhow!("OAuth: invalid redirect URL"))?;

    let revocation_url = RevocationUrl::new("https://oauth2.googleapis.com/revoke".to_string())
        .map_err(|_| anyhow!("OAuth: invalid revocation endpoint URL"))?;

    let client = BasicClient::new(
        google_client_id,
        Some(google_client_secret),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(redirect_url)
    .set_revocation_uri(revocation_url);
    Ok(client)
}

#[debug_handler]
pub async fn login(
    Query(mut params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let oauth_client = get_oauth_client().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "oauth_client_error".to_owned(),
                message: "".to_owned(),
            }),
        )
    })?;
    let return_url = params
        .remove("return_url")
        .unwrap_or_else(|| "/".to_string());
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.profile".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/calendar".to_string(),
        ))
        .set_pkce_challenge(pkce_code_challenge)
        .url();
    let result = oauth2_state_storage::ActiveModel {
        csrf_state: Set(csrf_state.secret().to_owned()),
        pkce_code_verifier: Set(pkce_code_verifier.secret().to_owned()),
        return_url: Set(return_url),
        ..Default::default()
    }
    .save(&state.conn)
    .await;
    Ok(Redirect::to(authorize_url.as_str()))
}

pub async fn oauth_callback(
    Query(mut params): Query<HashMap<String, String>>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let state = CsrfToken::new(params.remove("state").ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_state".to_owned(),
            message: "".to_owned(),
        }),
    ))?);

    let code = AuthorizationCode::new(params.remove("code").ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_code".to_owned(),
            message: "".to_owned(),
        }),
    ))?);

    let result = oauth2_state_storage::Entity::find()
        .filter(oauth2_state_storage::Column::CsrfState.eq(state.secret()))
        .one(&app_state.conn)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error".to_owned(),
                    message: "".to_owned(),
                }),
            )
        })?
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_state".to_owned(),
                message: "".to_owned(),
            }),
        ))?;

    let pkce_code_verifier = PkceCodeVerifier::new(result.pkce_code_verifier);
    let return_url = result.return_url;
    let oauth_client = get_oauth_client().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "oauth_client_error".to_owned(),
                message: "".to_owned(),
            }),
        )
    })?;

    // Exchange access token
    let token_response = tokio::task::spawn_blocking(move || {
        oauth_client
            .exchange_code(code)
            .set_pkce_verifier(pkce_code_verifier)
            .request(http_client)
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "exchange_code_failed".to_owned(),
                message: "".to_owned(),
            }),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "spawning_failed".to_owned(),
                message: "".to_owned(),
            }),
        )
    })?;

    let access_token = token_response.access_token().secret();

    // Get user info from Google

    let url =
        "https://www.googleapis.com/oauth2/v2/userinfo?oauth_token=".to_owned() + access_token;
    let body = reqwest::get(url)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "get_userinfo_failed".to_owned(),
                    message: "".to_owned(),
                }),
            )
        })?
        .text()
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "invalid_user_info".to_owned(),
                    message: "".to_owned(),
                }),
            )
        })?;

    let mut body: serde_json::Value = serde_json::from_str(body.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "invalid_user_info".to_owned(),
                message: "".to_owned(),
            }),
        )
    })?;

    let email = body["email"]
        .take()
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "missing_email".to_owned(),
                message: "".to_owned(),
            }),
        ))?
        .to_owned();
    let verified_email = body["verified_email"].take().as_bool().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AppError {
            code: "missing_verified_email".to_owned(),
            message: "".to_owned(),
        }),
    ))?;

    if !verified_email {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AppError {
                code: "unverified_email".to_owned(),
                message: "".to_owned(),
            }),
        ));
    }

    return Ok(email);
}
