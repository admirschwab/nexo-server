use rusqlite::Connection;
use std::{collections::HashMap, sync::Mutex, time::Instant};

pub struct PendingChallenge {
    pub challenge: [u8; 32],
    pub created_at: Instant,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    // Offene Login-Challenges, Schlüssel ist der Public Key (hex)
    pub challenges: Mutex<HashMap<String, PendingChallenge>>,
}
