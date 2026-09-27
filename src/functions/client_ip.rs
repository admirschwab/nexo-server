use axum::http::HeaderMap;
use std::net::{IpAddr, SocketAddr};

// Ermittelt die IP-Adresse des Clients für die Rate-Limits.
//
// Läuft der Server hinter einem Reverse Proxy, kommen alle Verbindungen vom Proxy.
// Die echte Adresse steht dann im Header X-Forwarded-For. Dem Header wird nur
// geglaubt, wenn die Verbindung von einem vertrauenswürdigen Proxy kommt,
// sonst könnte jeder Client eine beliebige Adresse angeben.
//
// Verwendet wird der letzte Eintrag: Den hängt unser eigener Proxy an.
// Alles davor kann der Client selbst mitgeschickt haben.
pub fn client_ip(peer: SocketAddr, headers: &HeaderMap, trusted_proxies: &[IpAddr]) -> IpAddr {
    let peer_ip = peer.ip().to_canonical();

    if !trusted_proxies.contains(&peer_ip) {
        return peer_ip;
    }

    headers
        .get_all("x-forwarded-for")
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .and_then(|ip| ip.trim().parse::<IpAddr>().ok())
        .map(|ip| ip.to_canonical())
        .unwrap_or(peer_ip)
}
