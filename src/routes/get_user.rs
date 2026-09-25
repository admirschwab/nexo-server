use crate::models::user::User;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub async fn get_user(
    State(database): State<Arc<Mutex<Connection>>>,
    Path(public_key): Path<String>,
) -> Result<Json<User>, StatusCode> {
    let database = database
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