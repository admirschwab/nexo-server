use crate::functions::{
    broadcast_online_users::broadcast_online_users,
    rate_limiter::RateLimiter,
    relay_message::relay_message,
};
use crate::models::protocol::{ClientMessage, ServerMessage, AUTH_CONTEXT};
use crate::state::{AppState, OnlineUser};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::{SinkExt, StreamExt};
use getrandom::{
    rand_core::{Rng, UnwrapErr},
    SysRng,
};
use rusqlite::params;
use std::{
    sync::{atomic::Ordering, Arc, PoisonError},
    time::Duration,
};
use tokio::{sync::mpsc, time::timeout};

// So lange hat der Client Zeit, die Challenge zu beantworten
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

// Nachrichten pro Verbindung: dauerhaft 5 pro Sekunde, kurzzeitig bis zu 20 am Stück
const MESSAGES_PER_SECOND: u32 = 5;
const MESSAGE_BURST: u32 = 20;

// Obergrenze für eine einzelne WebSocket-Nachricht
const MAX_MESSAGE_SIZE: usize = 64 * 1024;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.max_message_size(MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let Some((public_key, nickname)) = authenticate(&mut socket, &state).await else {
        let _ = socket.close().await;
        return;
    };

    let connection_id = state.next_connection_id.fetch_add(1, Ordering::Relaxed);
    let (sender, mut receiver) = mpsc::unbounded_channel::<ServerMessage>();

    // Ist derselbe Nutzer schon verbunden, wird die alte Verbindung ersetzt.
    // Deren Sender wird dabei verworfen, wodurch sie sich selbst beendet.
    state
        .online
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            public_key.clone(),
            OnlineUser {
                nickname,
                connection_id,
                sender,
            },
        );

    broadcast_online_users(&state);

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Schreibt alle Nachrichten aus dem Channel auf den WebSocket
    let mut writer = tokio::spawn(async move {
        while let Some(message) = receiver.recv().await {
            let Ok(text) = serde_json::to_string(&message) else {
                continue;
            };

            if ws_sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }

        let _ = ws_sender.close().await;
    });

    // Liest Nachrichten vom Client und leitet sie weiter
    let reader_state = state.clone();
    let reader_public_key = public_key.clone();

    let mut reader = tokio::spawn(async move {
        let mut rate_limiter = RateLimiter::new(MESSAGE_BURST, MESSAGES_PER_SECOND);

        while let Some(Ok(message)) = ws_receiver.next().await {
            match message {
                Message::Text(text) => {
                    let Ok(ClientMessage::Send { to, payload }) = serde_json::from_str(&text)
                    else {
                        continue;
                    };

                    if !relay_message(
                        &reader_state,
                        &reader_public_key,
                        connection_id,
                        to,
                        payload,
                        !rate_limiter.try_acquire(),
                    ) {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Endet eine Seite, wird die andere auch beendet
    tokio::select! {
        _ = &mut writer => reader.abort(),
        _ = &mut reader => writer.abort(),
    }

    {
        let mut online = state
            .online
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        // Nur entfernen, wenn der Eintrag noch zu dieser Verbindung gehört
        if online
            .get(&public_key)
            .is_some_and(|user| user.connection_id == connection_id)
        {
            online.remove(&public_key);
        }
    }

    broadcast_online_users(&state);
}

// Challenge-Response über den WebSocket.
// Gibt bei Erfolg (Public Key, Nickname) zurück.
async fn authenticate(
    socket: &mut WebSocket,
    state: &AppState,
) -> Option<(String, String)> {
    let mut challenge = [0u8; 32];

    let mut rng = UnwrapErr(SysRng);
    rng.fill_bytes(&mut challenge);

    send(socket, &ServerMessage::Challenge {
        challenge: hex::encode(challenge),
    })
        .await?;

    let message = timeout(AUTH_TIMEOUT, socket.recv()).await.ok()??.ok()?;

    let Message::Text(text) = message else {
        return None;
    };

    let Ok(ClientMessage::Auth {
        public_key,
        signature,
    }) = serde_json::from_str(&text)
    else {
        return None;
    };

    match verify_auth(state, &public_key, &signature, &challenge) {
        Ok(nickname) => {
            send(socket, &ServerMessage::AuthOk {
                nickname: nickname.clone(),
            })
                .await?;

            Some((public_key, nickname))
        }
        Err(reason) => {
            send(socket, &ServerMessage::AuthError {
                reason: reason.to_string(),
            })
                .await;

            None
        }
    }
}

// Prüft die Signatur und gibt den registrierten Nickname zurück
fn verify_auth(
    state: &AppState,
    public_key: &str,
    signature: &str,
    challenge: &[u8; 32],
) -> Result<String, &'static str> {
    let public_key_bytes: [u8; 32] = hex::decode(public_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("Invalid public key")?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| "Invalid public key")?;

    let signature_bytes = hex::decode(signature)
        .map_err(|_| "Invalid signature")?;

    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| "Invalid signature")?;

    let signed_message = [AUTH_CONTEXT, challenge.as_slice()].concat();

    verifying_key
        .verify(&signed_message, &signature)
        .map_err(|_| "Authentication failed")?;

    let database = state
        .db
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    database
        .query_row(
            "SELECT nickname FROM users WHERE public_key = ?1",
            params![public_key],
            |row| row.get(0),
        )
        .map_err(|_| "This identity is not registered on the server")
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Option<()> {
    let text = serde_json::to_string(message).ok()?;

    socket.send(Message::Text(text.into())).await.ok()
}
