use crate::functions::{
    check_client_version::check_client_version, client_ip::client_ip,
    validate_nickname::validate_nickname, verify_signature::verify_signature,
};
use crate::models::protocol::REGISTER_CONTEXT;
use crate::state::AppState;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rusqlite::ErrorCode;
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
pub struct RegisterRequest {
    public_key: String,
    nickname: String,
    // Signatur über REGISTER_CONTEXT + Public Key (32 Bytes) + Nickname
    signature: String,
    // Version des Clients (ab 0.2.0, ältere schicken keine)
    version: Option<String>,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<RegisterRequest>,
) -> Response {
    let ip = client_ip(address, &headers, &state.trusted_proxies);

    if !state.register_limiter.try_acquire(ip) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }

    // Zu alte Clients bekommen die Meldung für den Nutzer als Text zurück
    if let Err(reason) = check_client_version(request.version.as_deref()) {
        return (StatusCode::UPGRADE_REQUIRED, reason).into_response();
    }

    store_user(&state, &request).into_response()
}

fn store_user(state: &AppState, request: &RegisterRequest) -> Result<StatusCode, StatusCode> {
    if !validate_nickname(&request.nickname) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let public_key_bytes = verify_signature(
        &request.public_key,
        &request.signature,
        REGISTER_CONTEXT,
        request.nickname.as_bytes(),
    )?;

    let database = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    database
        .execute(
            "INSERT INTO users (public_key, nickname) VALUES (?1, ?2)",
            // Immer klein geschrieben speichern, damit derselbe Schlüssel
            // nicht in zwei Schreibweisen registriert werden kann
            (hex::encode(public_key_bytes), &request.nickname),
        )
        .map_err(|error| match error.sqlite_error_code() {
            // Nickname oder Public Key existiert bereits
            Some(ErrorCode::ConstraintViolation) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok(StatusCode::CREATED)
}
