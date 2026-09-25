use crate::models::user::User;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(public_key): Path<String>,
) -> Result<Json<User>, StatusCode> {
    let database = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut statement = database
        .prepare(
            "SELECT public_key, nickname
             FROM users
             WHERE public_key = ?1",
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = statement
        .query_row([public_key], |row| {
            Ok(User {
                public_key: row.get(0)?,
                nickname: row.get(1)?,
            })
        })
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(user))
}
