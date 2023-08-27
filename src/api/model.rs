use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub name: String,
    pub iat: usize,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TodoStatus {
    CREATED = 0,
    DONE = 1,
    DELETED = 2,
}
