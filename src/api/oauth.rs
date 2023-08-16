use std::{collections::HashMap, env};

use anyhow::anyhow;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use entity::oauth2_state_storage;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RevocationUrl, Scope, TokenUrl,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};

use super::AppState;

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

pub async fn login(
    Query(mut params): Query<HashMap<String, String>>,
    state: State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let oauth_client = get_oauth_client();
    if let Err(err) = oauth_client {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let return_url = params
        .remove("return_url")
        .unwrap_or_else(|| "/".to_string());
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = oauth_client
        .unwrap()
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
