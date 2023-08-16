use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use entity::prelude::*;
use sea_orm::EntityTrait;

use super::AppState;

pub async fn list(state: State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    let result = Todos::find().into_json().all(&state.conn).await;
    if let Ok(todos) = result {
        Ok(Json(todos))
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
