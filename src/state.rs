use crate::functions::rate_limiter::IpRateLimiter;
use crate::models::protocol::ServerMessage;
use rusqlite::Connection;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{atomic::AtomicUsize, Mutex},
};
use tokio::sync::{
    mpsc::{error::TrySendError, Sender},
    watch,
};

pub struct OnlineUser {
    pub nickname: String,
    // Unterscheidet mehrere Verbindungen desselben Nutzers
    pub connection_id: u64,
    // Alles, was hier hineingeschickt wird, geht an diesen Client.
    // Der Puffer ist begrenzt, damit ein Client, der nichts liest,
    // den Speicher des Servers nicht füllen kann.
    pub sender: Sender<ServerMessage>,
}

impl OnlineUser {
    // Gibt false zurück, wenn der Puffer des Clients voll ist.
    // Der Client liest dann offenbar nicht mehr und sollte getrennt werden.
    // Ist die Verbindung schon geschlossen, zählt das nicht als Fehler.
    pub fn deliver(&self, message: ServerMessage) -> bool {
        !matches!(self.sender.try_send(message), Err(TrySendError::Full(_)))
    }
}

pub struct AppState {
    pub db: Mutex<Connection>,
    // Aktuell verbundene Nutzer, Schlüssel ist der Public Key (hex)
    pub online: Mutex<HashMap<String, OnlineUser>>,
    pub register_limiter: IpRateLimiter,
    pub connect_limiter: IpRateLimiter,
    // Proxys, deren X-Forwarded-For-Header geglaubt wird (leer: keinem)
    pub trusted_proxies: Vec<IpAddr>,
    // Offene WebSocket-Verbindungen, damit beim Beenden gewartet werden kann,
    // bis alle sauber geschlossen sind
    pub active_connections: AtomicUsize,
    // Wird beim Beenden auf true gesetzt. Verbindungen, die gerade noch bei der
    // Anmeldung sind, brechen dann ab, neue werden nicht mehr angenommen.
    pub shutdown: watch::Sender<bool>,
}
