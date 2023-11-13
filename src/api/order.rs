use std::env;

use axum::{
    extract::{self, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Extension, Json,
};
use entity::{orders, users};
use lemon_squeezy::CreateCheckoutParams;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{constants::OrderStatus, AppError, AppState};

#[derive(Serialize, Deserialize)]
pub struct CreateOrderParams {
    /**
     * Redirect URL to chrome extension. Different from the redirect url passing to LemonSqueezy.
     * The backend will record this redirect url and assign it with the `internal_order_id`.
     * The redirect chain after success payment:
     * LemonSqueezy -- Redirect --> Backend -- Redirect --> Chrome Extension
     */
    redirect_url: Option<String>,
}

pub async fn crate_order(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<CreateOrderParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let email = user.email;
    let redirect_url = params.redirect_url.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_redirect_url",
            message: "",
        }),
    ))?;
    let client = lemon_squeezy::LemonSqueezy::new(
        env::var("LEMON_SQUEEZY_API_KEY").expect("LEMON_SQUEEZY_API_KEY is not set in .env file"),
    );

    let internal_order_id = uuid::Uuid::new_v4().to_string();

    let app_endpoint = env::var("APP_ENDPOINT").expect("APP_ENDPOINT is not set in .env file");
    let create_order_result = client
        .create_checkout(CreateCheckoutParams {
            email: Some(email),
            store_id: 43821,
            variant_id: 138344,
            redirect_url: format!(
                "{}/order/checkout_callback?internal_order_id={}",
                app_endpoint, internal_order_id
            ),
            custom_data: json!({
                "internal_order_id": internal_order_id,
            }),
        })
        .await
        .map_err(|err| {
            sentry::integrations::anyhow::capture_anyhow(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "failed_to_create_order",
                    message: "Failed to create order. Please try again later.",
                }),
            )
        })?;

    let new_order = orders::ActiveModel {
        user_id: Set(user.id),
        product_id: Set(120215),
        variant_id: Set(138344),
        status: Set(OrderStatus::Created as i32),
        internal_order_id: Set(internal_order_id.clone()),
        redirect_url: Set(format!("{}/{}", redirect_url, internal_order_id.clone())),
        ..Default::default()
    };

    let result = new_order.insert(&state.conn).await.map_err(|err| {
        sentry::capture_error(&err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppError {
                code: "database_error",
                message: "Failed to create order. Please try again later.",
            }),
        )
    })?;

    Ok(Json(json!({
        "checkout_url": create_order_result.attributes.url,
        "order_status": result.status
    })))
}

#[derive(Serialize, Deserialize)]
pub struct CheckoutCallbackQuery {
    internal_order_id: Option<String>,
}

pub async fn checkout_callback(
    state: State<AppState>,
    extract::Query(query): extract::Query<CheckoutCallbackQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let internal_order_id = query.internal_order_id.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_internal_order_id",
            message: "Missing order id.",
        }),
    ))?;

    let order = orders::Entity::find()
        .filter(Condition::all().add(orders::Column::InternalOrderId.eq(internal_order_id)))
        .one(&state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Failed to query order status. Please try again later.",
                }),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "order_not_found",
                message: "Order not found.",
            }),
        ))?;

    Ok(Redirect::temporary(order.redirect_url.as_str()))
}

#[derive(Serialize, Deserialize)]
pub struct CheckOrderStatusParams {
    internal_order_id: Option<i32>,
}

pub async fn check_order_status(
    state: State<AppState>,
    Extension(user): Extension<users::Model>,
    extract::Json(params): extract::Json<CheckOrderStatusParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<AppError>)> {
    let internal_order_id = params.internal_order_id.ok_or((
        StatusCode::BAD_REQUEST,
        Json(AppError {
            code: "missing_internal_order_id",
            message: "Missing order id.",
        }),
    ))?;

    let order = orders::Entity::find()
        .filter(
            Condition::all()
                .add(orders::Column::InternalOrderId.eq(internal_order_id))
                .add(orders::Column::UserId.eq(user.id)),
        )
        .into_json()
        .one(&state.conn)
        .await
        .map_err(|err| {
            sentry::capture_error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppError {
                    code: "database_error",
                    message: "Failed to query order status. Please try again later.",
                }),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(AppError {
                code: "order_not_found",
                message: "Order not found.",
            }),
        ))?;

    // let order_status: OrderStatus = order.status.try_into().map_err(|_| {
    //     (
    //         StatusCode::INTERNAL_SERVER_ERROR,
    //         Json(AppError {
    //             code: "invalid_order_status",
    //             message: "",
    //         }),
    //     )
    // })?;

    // if matches!(order_status, OrderStatus::Created) {

    // }

    // // TODO: check subscriptions by querying

    Ok(Json(order))
}
