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

    // Nicknames sind eindeutig, Groß-/Kleinschreibung wird ignoriert
    connection.execute(
        "
        CREATE UNIQUE INDEX IF NOT EXISTS users_nickname_unique
        ON users (nickname COLLATE NOCASE)
        ",
        [],
    )?;

    Ok(connection)
}
