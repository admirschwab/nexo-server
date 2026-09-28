use crate::state::AppState;
use std::{
    sync::{atomic::Ordering, PoisonError},
    time::Duration,
};
use tokio::time::{sleep, Instant};

// So lange bekommen offene Verbindungen beim Beenden Zeit, sich sauber zu schließen
const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

// Wartet auf Ctrl+C, unter Linux/macOS auch auf SIGTERM (z. B. von systemd oder Docker)
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};

        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    // Unter Windows: Ctrl+Break oder Schließen des Konsolenfensters
    #[cfg(windows)]
    let terminate = async {
        use tokio::signal::windows::{ctrl_break, ctrl_close};

        match (ctrl_break(), ctrl_close()) {
            (Ok(mut ctrl_break), Ok(mut ctrl_close)) => {
                tokio::select! {
                    _ = ctrl_break.recv() => {},
                    _ = ctrl_close.recv() => {},
                }
            }
            _ => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(any(unix, windows)))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("Shutting down ...");
}

// Trennt alle Clients sauber: Ihre Sender werden verworfen, dadurch schickt jede
// Verbindung noch einen Close-Frame und beendet sich. Die Clients sehen dann
// "disconnected" statt einer abgebrochenen Verbindung.
pub async fn close_all_connections(state: &AppState) {
    state.shutdown.send_replace(true);

    state
        .online
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();

    let deadline = Instant::now() + CLOSE_TIMEOUT;

    while state.active_connections.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
        sleep(Duration::from_millis(50)).await;
    }
}
