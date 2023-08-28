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

impl TryFrom<i32> for TodoStatus {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TodoStatus::CREATED),
            1 => Ok(TodoStatus::DONE),
            2 => Ok(TodoStatus::DELETED),
            _ => Err("Invalid status"),
        }
    }
}
