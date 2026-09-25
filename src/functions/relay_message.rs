use crate::models::protocol::{ServerMessage, MAX_PAYLOAD_LENGTH};
use crate::state::AppState;
use std::sync::PoisonError;

// Leitet eine Nachricht sofort an den Empfänger weiter. Nichts wird gespeichert.
// Ist der Absender zu schnell (rate_limited), wird die Nachricht verworfen
// und der Absender benachrichtigt.
// Gibt false zurück, wenn diese Verbindung nicht mehr die aktive des Absenders ist.
pub fn relay_message(
    state: &AppState,
    from: &str,
    connection_id: u64,
    to: String,
    payload: String,
    rate_limited: bool,
) -> bool {
    let online = state
        .online
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    let Some(sender) = online
        .get(from)
        .filter(|user| user.connection_id == connection_id)
    else {
        return false;
    };

    if rate_limited {
        let _ = sender.sender.send(ServerMessage::RateLimited { to });
        return true;
    }

    if payload.len() > MAX_PAYLOAD_LENGTH {
        return true;
    }

    match online.get(&to) {
        Some(recipient) => {
            let _ = recipient.sender.send(ServerMessage::Received {
                from: from.to_string(),
                payload,
            });
        }
        None => {
            let _ = sender.sender.send(ServerMessage::NotDelivered { to });
        }
    }

    true
}
