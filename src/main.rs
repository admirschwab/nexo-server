mod functions;
mod models;
mod routes;
mod state;

use axum::{
    routing::{any, post},
    Router,
};
use functions::{
    database::init_database,
    rate_limiter::{IpRateLimiter, PRUNE_INTERVAL},
    shutdown::{close_all_connections, shutdown_signal},
};
use routes::{register::register, unregister::unregister, ws::ws_handler};
use state::AppState;
use std::{
    collections::HashMap,
    env,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{atomic::AtomicUsize, Arc, Mutex},
};
use tokio::{net::TcpListener, sync::watch, time::interval};

const DEFAULT_BIND: &str = "127.0.0.1:3000";

#[tokio::main]
async fn main() {
    // Adresse, auf der der Server lauscht
    let bind = env::var("NEXO_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let trusted_proxies = trusted_proxies_from_env();

    let connection = init_database()
        .expect("Failed to initialize database");

    let state = Arc::new(AppState {
        db: Mutex::new(connection),
        online: Mutex::new(HashMap::new()),
        // Pro IP: bis zu 5 Registrierungen am Stück, danach eine pro Minute
        register_limiter: IpRateLimiter::new(5, 1.0 / 60.0),
        // Pro IP: bis zu 10 Verbindungsversuche am Stück, danach einer alle 5 Sekunden
        connect_limiter: IpRateLimiter::new(10, 1.0 / 5.0),
        trusted_proxies: trusted_proxies.clone(),
        active_connections: AtomicUsize::new(0),
        shutdown: watch::Sender::new(false),
    });

    // IP-Adressen aus den Rate-Limits regelmäßig vergessen, auch wenn keine
    // neuen Anfragen kommen
    let prune_state = state.clone();

    tokio::spawn(async move {
        let mut ticker = interval(PRUNE_INTERVAL);

        loop {
            ticker.tick().await;
            prune_state.register_limiter.prune();
            prune_state.connect_limiter.prune();
        }
    });

    let app = Router::new()
        .route("/register", post(register))
        .route("/unregister", post(unregister))
        .route("/ws", any(ws_handler))
        .with_state(state.clone());

    let listener = TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|error| panic!("Failed to bind server to {bind}: {error}"));

    println!("Nexo server listening on {bind}");

    if trusted_proxies.is_empty() {
        println!("Reverse proxy support: off");
    } else {
        println!("Reverse proxy support: trusting X-Forwarded-For from {trusted_proxies:?}");
    }

    // Die Adresse des Clients wird für die Rate-Limits pro IP gebraucht.
    // Nach Ctrl+C werden keine neuen Anfragen mehr angenommen, laufende
    // (z. B. eine Registrierung) dürfen noch fertig werden.
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");

    // WebSocket-Verbindungen zählen nicht als laufende Anfragen: selbst schließen
    close_all_connections(&state).await;

    // Datenbank schließen (Nexo speichert ohnehin nur Public Key und Nickname)
    drop(state);

    println!("Nexo server stopped");
}

// NEXO_TRUST_PROXY=true schaltet die Unterstützung für einen Reverse Proxy ein.
// Standardmäßig wird dann nur einem Proxy auf demselben Rechner vertraut.
// Mit NEXO_TRUSTED_PROXIES (kommagetrennte IPs) lassen sich andere Proxys angeben,
// z. B. wenn der Proxy in einem eigenen Docker-Container läuft.
fn trusted_proxies_from_env() -> Vec<IpAddr> {
    let trust_proxy = match env::var("NEXO_TRUST_PROXY") {
        Ok(value) => match value.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" | "" => false,
            _ => panic!("NEXO_TRUST_PROXY must be true or false, got '{value}'"),
        },
        Err(_) => false,
    };

    if !trust_proxy {
        return Vec::new();
    }

    match env::var("NEXO_TRUSTED_PROXIES") {
        Ok(list) => list
            .split(',')
            .map(str::trim)
            .filter(|ip| !ip.is_empty())
            .map(|ip| {
                ip.parse::<IpAddr>()
                    .unwrap_or_else(|_| panic!("Invalid IP address in NEXO_TRUSTED_PROXIES: '{ip}'"))
                    .to_canonical()
            })
            .collect(),
        Err(_) => vec![IpAddr::V4(Ipv4Addr::LOCALHOST), IpAddr::V6(Ipv6Addr::LOCALHOST)],
    }
}
