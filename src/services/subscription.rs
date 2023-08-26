use axum::extract::State;
use entity::{user_subscriptions, users};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};

use crate::api::{AppError, AppState};

pub async fn get_valid_subscription(
    app_state: &State<AppState>,
    user: &users::Model,
) -> Result<Option<user_subscriptions::Model>, AppError> {
    let current_time = chrono::Utc::now();

    let result = user_subscriptions::Entity::find()
        .filter(
            Condition::all()
                .add(user_subscriptions::Column::StartTime.lt(current_time))
                .add(user_subscriptions::Column::EndTime.gt(current_time))
                .add(user_subscriptions::Column::UserId.eq(user.id)),
        )
        .one(&app_state.conn)
        .await
        .map_err(|_| AppError {
            code: "database_error",
            message: "",
        })?;

    Ok(result)
}
