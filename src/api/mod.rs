pub mod jwt_auth;
pub mod oauth;
pub mod todo;

pub mod model;

use sea_orm::DatabaseConnection;
use serde::Serialize;

#[derive(Clone)]
pub struct AppState {
    pub conn: DatabaseConnection,
}

#[derive(Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Serialize)]
pub struct UserData {
    pub id: i64,
    pub email: String,
    pub name: String,
}
