mod functions;
mod models;
mod routes;

use axum::{routing::post, Router};
use functions::database::init_database;
use routes::{
    get_user::get_user,
    register::register,
};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let database = init_database()
        .expect("Failed to initialize database");

    let database = Arc::new(Mutex::new(database));

    let app = Router::new()
        .route("/register", post(register))
        .route("/users/{public_key}", axum::routing::get(get_user))
        .with_state(database);

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("Failed to bind server");

    println!("Nexo server listening on 127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}