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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetSubscriptionsParams {
    pub store_id: Option<i32>,
    pub order_id: Option<i32>,
    pub product_id: Option<i32>,
    pub variant_id: Option<i32>,
    pub status: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetSubscriptionsResponse {
    pub data: Vec<SubscriptionObject>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionObject {}

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
