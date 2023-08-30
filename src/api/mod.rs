pub mod jwt_auth;
pub mod oauth;
pub mod todo;
pub mod user;

pub mod model;

use std::fmt;

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

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl fmt::Debug for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AppError {{ code: {}, message: {} }}",
            self.code, self.message
        )
    }
}

#[derive(Clone, Serialize)]
pub struct UserData {
    pub id: i64,
    pub email: String,
    pub name: String,
}
