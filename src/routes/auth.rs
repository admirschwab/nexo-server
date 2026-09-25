use crate::state::{AppState, PendingChallenge};
use axum::{extract::State, http::StatusCode, Json};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use getrandom::{
    rand_core::{Rng, UnwrapErr},
    SysRng,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// Wie lange eine Challenge gültig ist
const CHALLENGE_TTL: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
pub struct ChallengeRequest {
    public_key: String,
}

#[derive(Serialize)]
pub struct ChallengeResponse {
    challenge: String,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    public_key: String,
    signature: String,
}

#[derive(Serialize)]
pub struct VerifyResponse {
    success: bool,
    nickname: String,
}

pub async fn create_challenge(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChallengeRequest>,
) -> Result<Json<ChallengeResponse>, StatusCode> {
    let exists: bool = {
        let database = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        database
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM users WHERE public_key = ?1)",
                params![request.public_key],
                |row| row.get(0),
            )
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    if !exists {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut challenge = [0u8; 32];

    let mut rng = UnwrapErr(SysRng);
    rng.fill_bytes(&mut challenge);

    let mut challenges = state
        .challenges
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    challenges.insert(
        request.public_key,
        PendingChallenge {
            challenge,
            created_at: Instant::now(),
        },
    );

    Ok(Json(ChallengeResponse {
        challenge: hex::encode(challenge),
    }))
}

pub async fn verify(
    State(state): State<Arc<AppState>>,
    Json(request): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, StatusCode> {
    // Challenge wird entfernt, damit sie nur einmal benutzt werden kann
    let pending = {
        let mut challenges = state
            .challenges
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        challenges
            .remove(&request.public_key)
            .ok_or(StatusCode::UNAUTHORIZED)?
    };

    if pending.created_at.elapsed() > CHALLENGE_TTL {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let public_key_bytes = hex::decode(&request.public_key)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let public_key_array: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_array)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let signature_bytes = hex::decode(&request.signature)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    verifying_key
        .verify(&pending.challenge, &signature)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Nickname so zurückgeben, wie er auf dem Server registriert ist
    let nickname: String = {
        let database = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        database
            .query_row(
                "SELECT nickname FROM users WHERE public_key = ?1",
                params![request.public_key],
                |row| row.get(0),
            )
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    Ok(Json(VerifyResponse {
        success: true,
        nickname,
    }))
}
