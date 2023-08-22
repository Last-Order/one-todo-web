mod api;

use std::env;

use api::{
    jwt_auth,
    oauth::{login, oauth_callback},
    todo::get_upcoming_events,
    AppState,
};
use axum::{
    http::{header, Method},
    middleware,
    routing::get,
    Router,
};
use sea_orm::Database;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");

    let conn = Database::connect(db_url)
        .await
        .expect("Database connection failed");

    let state = AppState { conn };

    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route(
            "/upcoming",
            get(get_upcoming_events).route_layer(middleware::from_fn_with_state(
                state.clone(),
                jwt_auth::auth,
            )),
        )
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_origin(Any)
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
        )
        .route("/oauth/google/login", get(login))
        .route("/oauth/google/callback", get(oauth_callback))
        .with_state(state);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
