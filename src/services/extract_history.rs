use axum::extract::State;
use entity::{extract_history, users};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, PaginatorTrait,
    QueryFilter,
};

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
    .map_err(|err| AppError {
        code: "record_extract_history_error",
        message: "Please try again later.",
    })?;
    Ok(())
}

pub async fn count_extract_history(
    app_state: &State<AppState>,
    user: &users::Model,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
) -> Result<i32, AppError> {
    let result: i32 = extract_history::Entity::find()
        .filter(
            Condition::all()
                .add(extract_history::Column::ExtractTime.gt(start_time))
                .add(extract_history::Column::ExtractTime.lt(end_time))
                .add(extract_history::Column::UserId.eq(user.id)),
        )
        .count(&app_state.conn)
        .await
        .map_err(|_| AppError {
            code: "database_error",
            message: "",
        })?
        .try_into()
        .map_err(|_| AppError {
            code: "database_error",
            message: "",
        })?;

    Ok(result)
}
