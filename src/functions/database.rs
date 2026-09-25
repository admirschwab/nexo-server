use rusqlite::Connection;

pub fn init_database() -> Result<Connection, rusqlite::Error> {
    let connection = Connection::open("nexo.db")?;

    connection.execute(
        "
        CREATE TABLE IF NOT EXISTS users (
            public_key TEXT PRIMARY KEY,
            nickname TEXT NOT NULL
        )
        ",
        [],
    )?;

    Ok(connection)
}