use crate::models::protocol::ServerMessage;
use rusqlite::Connection;
use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Mutex},
};
use tokio::sync::mpsc::UnboundedSender;

pub struct OnlineUser {
    pub nickname: String,
    // Unterscheidet mehrere Verbindungen desselben Nutzers
    pub connection_id: u64,
    // Alles, was hier hineingeschickt wird, geht an diesen Client
    pub sender: UnboundedSender<ServerMessage>,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    // Aktuell verbundene Nutzer, Schlüssel ist der Public Key (hex)
    pub online: Mutex<HashMap<String, OnlineUser>>,
    pub next_connection_id: AtomicU64,
}
