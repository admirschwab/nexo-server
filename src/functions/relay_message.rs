use crate::models::protocol::{ServerMessage, MAX_PAYLOAD_LENGTH};
use crate::state::AppState;
use std::sync::PoisonError;

// Leitet eine Nachricht sofort an den Empfänger weiter. Nichts wird gespeichert.
// Ist der Absender zu schnell (rate_limited), wird die Nachricht verworfen
// und der Absender benachrichtigt.
// Gibt false zurück, wenn diese Verbindung beendet werden soll: Sie ist nicht mehr
// die aktive des Absenders, oder der Absender liest seine eigenen Nachrichten nicht.
pub fn relay_message(
    state: &AppState,
    from: &str,
    connection_id: u64,
    to: String,
    payload: String,
    rate_limited: bool,
) -> bool {
    let mut online = state
        .online
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    if !online
        .get(from)
        .is_some_and(|user| user.connection_id == connection_id)
    {
        return false;
    }

    // Antwort an den Absender. Ist sein Puffer voll, wird er getrennt.
    let reply = if rate_limited {
        Some(ServerMessage::RateLimited { to })
    } else if payload.len() > MAX_PAYLOAD_LENGTH {
        return true;
    } else {
        match online.get(&to) {
            Some(recipient) => {
                if recipient.deliver(ServerMessage::Received {
                    from: from.to_string(),
                    payload,
                }) {
                    None
                } else {
                    // Der Empfänger liest nicht mehr: trennen
                    online.remove(&to);
                    Some(ServerMessage::NotDelivered { to })
                }
            }
            None => Some(ServerMessage::NotDelivered { to }),
        }
    };

    let Some(reply) = reply else {
        return true;
    };

    // Erneut nachschlagen, weil `online` oben verändert worden sein kann
    let delivered = online
        .get(from)
        .is_some_and(|sender| sender.deliver(reply));

    if !delivered {
        online.remove(from);
    }

    delivered
}
