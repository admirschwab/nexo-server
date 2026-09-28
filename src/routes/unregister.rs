use crate::functions::{
    broadcast_online_users::broadcast_online_users, client_ip::client_ip,
    verify_signature::verify_signature,
};
use crate::models::protocol::UNREGISTER_CONTEXT;
use crate::state::AppState;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use std::{
    net::SocketAddr,
    sync::{Arc, PoisonError},
};

#[derive(Deserialize)]
pub struct UnregisterRequest {
    public_key: String,
    // Signatur über UNREGISTER_CONTEXT + Public Key (32 Bytes)
    signature: String,
}

// Löscht ein Konto vollständig. Danach weiß der Server nichts mehr über diesen
// Nutzer, und der Nickname ist wieder frei.
pub async fn unregister(
    State(state): State<Arc<AppState>>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<UnregisterRequest>,
) -> Result<StatusCode, StatusCode> {
    let ip = client_ip(address, &headers, &state.trusted_proxies);

    // Teilt sich das Limit mit der Registrierung
    if !state.register_limiter.try_acquire(ip) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let public_key_bytes = verify_signature(
        &request.public_key,
        &request.signature,
        UNREGISTER_CONTEXT,
        &[],
    )?;

    let public_key = hex::encode(public_key_bytes);

    let deleted = {
        let database = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        database
            .execute("DELETE FROM users WHERE public_key = ?1", [&public_key])
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    if deleted == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Ist der Nutzer gerade verbunden, wird er getrennt: Mit dem Eintrag
    // fällt sein Sender weg und die Verbindung beendet sich
    let was_online = state
        .online
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&public_key)
        .is_some();

    if was_online {
        broadcast_online_users(&state);
    }

    Ok(StatusCode::NO_CONTENT)
}
