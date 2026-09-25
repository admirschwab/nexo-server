use crate::functions::validate_nickname::validate_nickname;
use crate::models::user::User;
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use ed25519_dalek::VerifyingKey;
use rusqlite::ErrorCode;
use std::sync::Arc;

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(user): Json<User>,
) -> Result<StatusCode, StatusCode> {
    if !validate_nickname(&user.nickname) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let public_key_bytes = hex::decode(&user.public_key)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let public_key_bytes: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let database = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    database
        .execute(
            "INSERT INTO users (public_key, nickname) VALUES (?1, ?2)",
            (&user.public_key, &user.nickname),
        )
        .map_err(|error| match error.sqlite_error_code() {
            // Nickname oder Public Key existiert bereits
            Some(ErrorCode::ConstraintViolation) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok(StatusCode::CREATED)
}
