use std::env;

use reqwest;
use serde::{Deserialize, Serialize};

use crate::api::AppError;

#[derive(Serialize, Deserialize)]
struct GetCompletionPayload {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

pub async fn get_completion(prompt: &str) -> Result<String, AppError> {
    let api_endpoint =
        env::var("OPENAI_API_ENDPOINT").expect("OPENAI_API_ENDPOINT is not set in .env file");
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set in .env file");
    let api = format!("{}/v1/chat/completions", api_endpoint);

    let client = reqwest::Client::new();

    let request_payload = GetCompletionPayload {
        model: "gpt-3.5-turbo".to_owned(),
        messages: vec![Message {
            role: "user".to_owned(),
            content: prompt.to_owned(),
        }],
        temperature: 0.2,
    };

    let response = client
        .post(api)
        .json(&request_payload)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", api_key),
        )
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .send()
        .await
        .map_err(|_| AppError {
            code: "failed_to_get_completion",
            message: "Please try again later",
        })?
        .text()
        .await
        .map_err(|_| AppError {
            code: "failed_to_parse_completion_response",
            message: "Please try again later",
        })?;

    let mut body: serde_json::Value =
        serde_json::from_str(response.as_str()).map_err(|_| AppError {
            code: "invalid_completion_response",
            message: "",
        })?;

    let openai_result = body["choices"]
        .as_array_mut()
        .ok_or(AppError {
            code: "invalid_completion_response",
            message: "",
        })?
        .first()
        .ok_or(AppError {
            code: "invalid_completion_response",
            message: "",
        })?
        .get("message")
        .ok_or(AppError {
            code: "invalid_completion_response",
            message: "",
        })?
        .get("content")
        .ok_or(AppError {
            code: "invalid_completion_response",
            message: "",
        })?
        .as_str()
        .ok_or(AppError {
            code: "invalid_completion_response",
            message: "",
        })?
        .to_owned();

    dbg!(&openai_result);

    Ok(openai_result)
}
