use crate::models::protocol::{OnlineUserInfo, ServerMessage};
use crate::state::AppState;
use std::sync::PoisonError;

// Schickt allen verbundenen Clients die aktuelle Online-Liste
pub fn broadcast_online_users(state: &AppState) {
    let mut online = state
        .online
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    let mut users: Vec<OnlineUserInfo> = online
        .iter()
        .map(|(public_key, user)| OnlineUserInfo {
            public_key: public_key.clone(),
            nickname: user.nickname.clone(),
            connection_id: user.connection_id,
        })
        .collect();

    users.sort_by_key(|user| user.nickname.to_lowercase());

    // Clients, deren Puffer voll ist, werden entfernt. Dadurch fällt ihr
    // Sender weg und die Verbindung beendet sich. Beim Aufräumen der
    // Verbindung wird die Liste erneut verschickt.
    online.retain(|_, user| {
        user.deliver(ServerMessage::OnlineUsers {
            users: users.clone(),
        })
    });
}
