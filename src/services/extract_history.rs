use axum::extract::State;
use entity::{extract_history, users};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};

use crate::api::{AppError, AppState};

pub async fn record_extract_history(
    app_state: &State<AppState>,
    user: &users::Model,
    prompt: &str,
) -> Result<(), AppError> {
    extract_history::ActiveModel {
        user_id: Set(user.id),
        prompt: Set(Some(prompt.clone().to_owned())),
        extract_time: Set(chrono::Utc::now()),
        ..Default::default()
    }
    .save(&app_state.conn)
    .await
    .map_err(|_| AppError {
        code: "refresh_token_error",
        message: "",
    })?;
    Ok(())
}
