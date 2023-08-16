mod api;

use std::env;

use api::{oauth::login, todo::list, AppState};
use axum::{routing::get, Router};
use sea_orm::Database;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");

    let conn = Database::connect(db_url)
        .await
        .expect("Database connection failed");

    let state = AppState { conn };

    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/list", get(list))
        .route("/oauth/google/login", get(login))
        .with_state(state);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
