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
pub struct GenerateCheckoutUrlParams {
    pub email: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCheckoutResponse {
    data: CreateCheckoutResponseData,
    links: CreateCheckoutResponseDataLinks,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCheckoutResponseData {
    id: String,
    attributes: CreateCheckoutResponseDataAttributes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCheckoutResponseDataLinks {
    #[serde(rename = "self")]
    se_lf: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCheckoutResponseDataAttributes {
    url: String,
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

    pub async fn generate_checkout_url(
        &self,
        params: GenerateCheckoutUrlParams,
    ) -> Result<String, anyhow::Error> {
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
            .json::<CreateCheckoutResponse>()
            .await?;

        return Ok(String::from(response.data.attributes.url));
    }
}
