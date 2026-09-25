mod functions;
mod models;
mod routes;
mod state;

use axum::{
    routing::{any, get, post},
    Router,
};
use functions::database::init_database;
use routes::{get_user::get_user, register::register, ws::ws_handler};
use state::AppState;
use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, Mutex},
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let connection = init_database()
        .expect("Failed to initialize database");

    let state = Arc::new(AppState {
        db: Mutex::new(connection),
        online: Mutex::new(HashMap::new()),
        next_connection_id: AtomicU64::new(0),
    });

    let app = Router::new()
        .route("/register", post(register))
        .route("/users/{public_key}", get(get_user))
        .route("/ws", any(ws_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind server");

    println!("Nexo server listening on 127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
