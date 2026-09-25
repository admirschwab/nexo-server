use crate::models::user::User;
use axum::{extract::State, http::StatusCode, Json};
use ed25519_dalek::VerifyingKey;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub async fn register(
    State(database): State<Arc<Mutex<Connection>>>,
    Json(user): Json<User>,
) -> Result<StatusCode, StatusCode> {
    let public_key_bytes = hex::decode(&user.public_key)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let public_key_bytes: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let database = database
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    database
        .execute(
            "INSERT INTO users (public_key, nickname) VALUES (?1, ?2)",
            (&user.public_key, &user.nickname),
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(StatusCode::CREATED)
}