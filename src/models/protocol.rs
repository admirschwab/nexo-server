use serde::{Deserialize, Serialize};

// Nachrichten, die über den WebSocket laufen (als JSON mit Feld "type").
// Muss mit nexo-cli/src/models/protocol.rs übereinstimmen.

#[derive(Clone, Serialize)]
pub struct OnlineUserInfo {
    pub public_key: String,
    pub nickname: String,
    // Zufällig, neu bei jeder Verbindung. Daran erkennen Clients,
    // dass ein Partner neu verbunden ist und alte Chat-Schlüssel ungültig sind.
    // Zufällig statt fortlaufend, damit sie nichts über andere Verbindungen verrät.
    pub connection_id: u64,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Challenge { challenge: String },
    AuthOk { nickname: String },
    AuthError { reason: String },
    OnlineUsers { users: Vec<OnlineUserInfo> },
    // Weitergeleitete Nachricht eines anderen Clients. "from" setzt der Server
    // selbst anhand der Anmeldung, ein Client kann also keinen Absender fälschen.
    Received { from: String, payload: String },
    // Der Empfänger war nicht online, die Nachricht wurde verworfen
    NotDelivered { to: String },
    // Der Absender schickt zu schnell, die Nachricht wurde verworfen
    RateLimited { to: String },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Auth { public_key: String, signature: String },
    // "payload" ist für den Server undurchsichtig (Ende-zu-Ende verschlüsselt)
    Send { to: String, payload: String },
}

// Wird vor die Challenge gesetzt und mitsigniert, damit eine Login-Signatur
// nie mit einer anderen Signatur des Clients verwechselt werden kann
pub const AUTH_CONTEXT: &[u8] = b"nexo-auth-v1";

// Kontext für die Signatur bei der Registrierung (Kontext + Public Key + Nickname).
// Damit beweist der Client, dass er den privaten Schlüssel besitzt.
pub const REGISTER_CONTEXT: &[u8] = b"nexo-register-v1";

// Kontext für die Signatur beim Löschen des Kontos (Kontext + Public Key)
pub const UNREGISTER_CONTEXT: &[u8] = b"nexo-unregister-v1";

// Größere Payloads werden verworfen
pub const MAX_PAYLOAD_LENGTH: usize = 16 * 1024;
