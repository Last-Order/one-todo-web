pub mod oauth;
pub mod todo;

use sea_orm::DatabaseConnection;
use serde::Serialize;

#[derive(Clone)]
pub struct AppState {
    pub conn: DatabaseConnection,
}

#[derive(Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}
