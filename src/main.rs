mod functions;
mod models;
mod routes;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use functions::database::init_database;
use routes::{
    auth::{create_challenge, verify},
    get_user::get_user,
    register::register,
};
use state::AppState;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let connection = init_database()
        .expect("Failed to initialize database");

    let state = Arc::new(AppState {
        db: Mutex::new(connection),
        challenges: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/register", post(register))
        .route("/auth/challenge", post(create_challenge))
        .route("/auth/verify", post(verify))
        .route("/users/{public_key}", get(get_user))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind server");

    println!("Nexo server listening on 127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
