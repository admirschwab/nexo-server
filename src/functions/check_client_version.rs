use std::{fs, io::ErrorKind};

// Datei im Arbeitsverzeichnis (neben nexo.db) mit der ältesten Client-Version,
// die noch angenommen wird, z. B. "0.2.0". Fehlt sie, ist jede Version erlaubt.
// Sie wird bei jeder Anmeldung neu gelesen, ändern geht also ohne Neustart.
const MIN_VERSION_FILE: &str = "min_client_version";

const INSTALL_COMMAND: &str =
    "irm https://raw.githubusercontent.com/admirschwab/nexo/main/install.ps1 | iex";

// Prüft die Version, die der Client mitschickt. Clients vor 0.2.0 schicken keine.
// Ist sie zu alt, kommt die Meldung für den Nutzer zurück. Der Client zeigt sie
// an und beendet sich (auch 0.1.0 zeigt den Grund einer AuthError schon an).
pub fn check_client_version(version: Option<&str>) -> Result<(), String> {
    let Some(min_version) = read_min_version() else {
        return Ok(());
    };

    let client_version = version.and_then(parse_version);

    if client_version.is_some_and(|client_version| client_version >= min_version.1) {
        return Ok(());
    }

    let installed = version.unwrap_or("an older version");

    Err(format!(
        "This server requires Nexo {} or newer (you have {installed}).\n\
         Please update by running this in a PowerShell window:\n\n    {INSTALL_COMMAND}",
        min_version.0
    ))
}

// Gibt die Version als Text und als Zahlen zurück
fn read_min_version() -> Option<(String, (u64, u64, u64))> {
    let text = match fs::read_to_string(MIN_VERSION_FILE) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => {
            eprintln!("Warning: could not read {MIN_VERSION_FILE}, allowing all versions: {error}");
            return None;
        }
    };

    let text = text.trim();

    match parse_version(text) {
        Some(version) => Some((text.to_string(), version)),
        None => {
            eprintln!(
                "Warning: {MIN_VERSION_FILE} must contain a version like 0.2.0, got '{text}'. \
                 Allowing all versions."
            );
            None
        }
    }
}

// "1.2.3" -> (1, 2, 3)
fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().split('.').map(|part| part.parse::<u64>().ok());

    let version = (parts.next()??, parts.next()??, parts.next()??);

    parts.next().is_none().then_some(version)
}
