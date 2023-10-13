use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum TodoStatus {
    Created = 0,
    Done = 1,
    Deleted = 2,
}

impl TryFrom<i32> for TodoStatus {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TodoStatus::Created),
            1 => Ok(TodoStatus::Done),
            2 => Ok(TodoStatus::Deleted),
            _ => Err("Invalid status"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SubscriptionType {
    Free = 1,
    Pro = 2,
}

impl TryFrom<i32> for SubscriptionType {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SubscriptionType::Free),
            2 => Ok(SubscriptionType::Pro),
            _ => Err("Invalid subscription type"),
        }
    }
}

impl From<SubscriptionType> for String {
    fn from(value: SubscriptionType) -> Self {
        match value {
            SubscriptionType::Free => String::from("Free Plan"),
            SubscriptionType::Pro => String::from("Pro Plan"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OrderStatus {
    Created = 0,
    Finished = 1,
    Cancelled = 2,
    Timeout = 3,
}

impl TryFrom<i32> for OrderStatus {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(OrderStatus::Created),
            1 => Ok(OrderStatus::Finished),
            2 => Ok(OrderStatus::Cancelled),
            4 => Ok(OrderStatus::Timeout),
            _ => Err("Invalid order status"),
        }
    }
}
