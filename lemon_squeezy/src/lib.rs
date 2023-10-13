use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub mod constants;

#[derive(Clone, Debug)]
pub struct LemonSqueezy {
    client: reqwest::Client,
    headers: HeaderMap,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOrderParams {
    pub email: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    data: CreateOrderResponseData,
    links: CreateOrderResponseDataLinks,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOrderResponseData {
    id: String,
    attributes: CreateOrderResponseDataAttributes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOrderResponseDataLinks {
    #[serde(rename = "self")]
    se_lf: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOrderResponseDataAttributes {
    url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateOrderResult {
    pub checkout_url: String,
    pub order_id: String,
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

    pub async fn create_order(
        &self,
        params: CreateOrderParams,
    ) -> Result<CreateOrderResult, anyhow::Error> {
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
                        }
                    },
                    "relationships": {
                        "store": {
                        "data": {
                            "type": "stores",
                            "id": "43821"
                        }
                        },
                        "variant": {
                        "data": {
                            "type": "variants",
                            "id": "138344"
                        }
                        }
                    }
                }
            }))
            .headers(self.headers.clone())
            .send()
            .await?
            .json::<CreateOrderResponse>()
            .await?;

        return Ok(CreateOrderResult {
            checkout_url: response.data.attributes.url,
            order_id: response.data.id,
        });
    }
}
