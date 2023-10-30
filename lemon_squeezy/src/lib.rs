use core::fmt;

use anyhow::anyhow;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_qs;

pub mod constants;

#[derive(Clone, Debug)]
pub struct LemonSqueezy {
    client: reqwest::Client,
    headers: HeaderMap,
}

impl LemonSqueezy {
    pub fn new(key: String) -> Self {
        let client = reqwest::Client::new();
        let mut headers = HeaderMap::new();

        headers.append(
            "Accept",
            HeaderValue::from_str("application/vnd.api+json").unwrap(),
        );
        headers.append(
            "Content-Type",
            HeaderValue::from_str("application/vnd.api+json").unwrap(),
        );
        headers.append(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", key)).unwrap(),
        );

        Self { client, headers }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCheckoutParams {
    pub email: Option<String>,
    pub store_id: i32,
    pub variant_id: i32,
    pub redirect_url: String,
    pub custom_data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCheckoutResponse {
    data: CheckoutObject,
    links: CheckoutLinks,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutObject {
    id: String,
    pub attributes: CheckoutAttributes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutLinks {
    #[serde(rename = "self")]
    se_lf: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutAttributes {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCheckoutResult {
    pub checkout_url: String,
    pub order_id: String,
}

impl LemonSqueezy {
    /**
     * 创建订单
     */
    pub async fn create_checkout(
        &self,
        params: CreateCheckoutParams,
    ) -> Result<CheckoutObject, anyhow::Error> {
        let url = format!("{}/checkouts", constants::API_HOST);

        let response = self
            .client
            .post(url)
            .json(&json!({
                "data": {
                    "type": "checkouts",
                    "attributes": {
                        "checkout_data": {
                            "email": params.email.unwrap_or(String::from("")),
                            "custom": json!(params.custom_data),
                        },
                        "product_options": {
                            "redirect_url": params.redirect_url,
                        }
                    },
                    "relationships": {
                        "store": {
                            "data": {
                                "type": "stores",
                                "id": format!("{}", params.store_id),
                            }
                        },
                        "variant": {
                            "data": {
                                "type": "variants",
                                "id": format!("{}", params.variant_id)
                            }
                        }
                    }
                }
            }))
            .headers(self.headers.clone())
            .send()
            .await?
            .json::<CreateCheckoutResponse>()
            .await?;

        return Ok(response.data);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    OnTrial,
    Active,
    Paused,
    PastDue,
    Unpaid,
    Cancelled,
    Expired,
    #[default]
    Unknown,
}

impl fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SubscriptionStatus::OnTrial => write!(f, "on_trial"),
            SubscriptionStatus::Active => write!(f, "active"),
            SubscriptionStatus::Paused => write!(f, "paused"),
            SubscriptionStatus::PastDue => write!(f, "past_due"),
            SubscriptionStatus::Unpaid => write!(f, "unpaid"),
            SubscriptionStatus::Cancelled => write!(f, "cancelled"),
            SubscriptionStatus::Expired => write!(f, "expired"),
            SubscriptionStatus::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GetSubscriptionsParams {
    pub store_id: i32,
    pub order_id: i32,
    pub product_id: i32,
    pub variant_id: i32,
    pub status: SubscriptionStatus,
    pub user_email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetSubscriptionsResponse {
    pub data: Vec<SubscriptionObject>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionObject {
    pub r#type: String,
    pub id: String,
    pub attributes: SubscriptionObjectAttributes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionObjectAttributes {
    pub store_id: i32,
    pub customer_id: i32,
    pub order_id: i32,
    pub order_item_id: i32,
    pub product_id: i32,
    pub variant_id: i32,
    pub user_email: i32,
    pub status: SubscriptionStatus,
    pub status_formatted: String,
    pub created_at: String,
    pub renews_at: String,
    pub ends_at: Option<String>,
}

impl LemonSqueezy {
    pub async fn get_subscriptions(
        &self,
        params: GetSubscriptionsParams,
    ) -> Result<Vec<SubscriptionObject>, anyhow::Error> {
        let query =
            serde_qs::to_string(&params).map_err(|_| anyhow!("Failed to serialize params."))?;
        let url = format!("{}/v1/subscriptions?{}", constants::API_HOST, query);
        let response = self
            .client
            .get(url)
            .headers(self.headers.clone())
            .send()
            .await?
            .json::<GetSubscriptionsResponse>()
            .await?;

        Ok(response.data)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionInvoiceObject {
    pub r#type: String,
    pub id: String,
    pub attributes: SubscriptionInvoiceObjectAttributes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionInvoiceObjectAttributes {
    pub store_id: i32,
    pub subscription_id: i32,
    pub customer_id: i32,
    pub user_name: String,
    pub user_email: String,
    pub status: String,
    pub status_formatted: String,
}
