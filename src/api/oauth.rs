use std::{collections::HashMap, env};

use anyhow::anyhow;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_macros::debug_handler;
use chrono;
use entity::{oauth2_state_storage, users};
use jsonwebtoken::{encode, EncodingKey, Header};
use oauth2::{
    basic::BasicClient, reqwest::http_client, AuthUrl, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RevocationUrl, Scope,
    TokenResponse, TokenUrl,
};
use reqwest;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use url;

use super::{model::TokenClaims, AppError, AppState};

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
                code: "oauth_client_error",
                message: "",
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
        // .add_scope(Scope::new(
        //     "https://www.googleapis.com/auth/calendar".to_string(),
        // ))
        .set_pkce_challenge(pkce_code_challenge)
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
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
            code: "missing_state",
            message: "",
        }),
    ))?);

    let code = AuthorizationCode::new(params.remove("code").ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_code",
            message: "",
        }),
    ))?);

    let scope = params.remove("scope").ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_scope",
            message: "",
        }),
    ))?;

    let has_calendar_access: i8 = if scope.contains("calendar") { 1 } else { 0 };

    let result = oauth2_state_storage::Entity::find()
        .filter(oauth2_state_storage::Column::CsrfState.eq(state.secret()))
        .one(&app_state.conn)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "",
                }),
            )
        })?
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_state",
                message: "",
            }),
        ))?;

    let pkce_code_verifier = PkceCodeVerifier::new(result.pkce_code_verifier);
    let return_url = result.return_url;
    let oauth_client = get_oauth_client().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "oauth_client_error",
                message: "",
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
                code: "exchange_code_failed",
                message: "",
            }),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "spawning_failed",
                message: "",
            }),
        )
    })?;

    let access_token = token_response.access_token().secret();
    let refresh_token = token_response
        .refresh_token()
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "missing_refresh_token",
                message: "",
            }),
        ))?
        .secret();

    // Get user info from Google

    let google_token_exchange_url =
        "https://www.googleapis.com/oauth2/v2/userinfo?oauth_token=".to_owned() + access_token;
    let body = reqwest::get(google_token_exchange_url)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "get_userinfo_failed",
                    message: "",
                }),
            )
        })?
        .text()
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "invalid_user_info",
                    message: "",
                }),
            )
        })?;

    let mut body: serde_json::Value = serde_json::from_str(body.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "invalid_user_info",
                message: "",
            }),
        )
    })?;

    let email = body["email"]
        .take()
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "missing_email",
                message: "",
            }),
        ))?
        .to_owned();

    let verified_email = body["verified_email"].take().as_bool().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AppError {
            code: "missing_verified_email",
            message: "",
        }),
    ))?;

    let name = body["name"]
        .take()
        .as_str()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "invalid_name",
                message: "",
            }),
        ))?
        .to_owned();

    if !verified_email {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AppError {
                code: "unverified_email",
                message: "",
            }),
        ));
    }

    // Create user if not exists

    let existed_user = users::Entity::find()
        .filter(users::Column::Email.eq(&email))
        .one(&app_state.conn)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "create_user_error_1",
                    message: "",
                }),
            )
        })?;

    if existed_user.is_none() {
        // Create user
        users::ActiveModel {
            name: Set(name.clone()),
            email: Set(email.clone()),
            google_access_token: Set(access_token.to_owned()),
            google_refresh_token: Set(refresh_token.to_owned()),
            has_google_calendar_access: Set(has_calendar_access),
            ..Default::default()
        }
        .save(&app_state.conn)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "create_user_error_2",
                    message: "",
                }),
            )
        })?;
    } else {
        // Refresh tokens
        let mut modified_user: users::ActiveModel = existed_user.unwrap().into();
        modified_user.google_access_token = Set(access_token.to_owned());
        modified_user.google_refresh_token = Set(refresh_token.to_owned());
        modified_user.save(&app_state.conn).await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "refresh_token_error",
                    message: "",
                }),
            )
        })?;
    }

    // issue JWT token
    let now = chrono::Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + chrono::Duration::minutes(60 * 24 * 30 * 12)).timestamp() as usize;
    let claims: TokenClaims = TokenClaims {
        sub: email,
        name: name,
        exp,
        iat,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(
            env::var("JWT_SECRET")
                .expect("JWT_SECRET is not set in .env file")
                .as_ref(),
        ),
    )
    .unwrap();

    let redirect_url = url::Url::parse(&return_url).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(AppError {
                code: "invalid_return_url",
                message: "",
            }),
        )
    })?;

    let final_url = format!("{}/{}", redirect_url, token);

    return Ok(Redirect::to(final_url.as_str()));
}
