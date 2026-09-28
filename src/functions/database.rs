use rusqlite::Connection;

pub fn init_database() -> Result<Connection, rusqlite::Error> {
    let connection = Connection::open("nexo.db")?;

    // Gelöschte Daten (z. B. nach `nexo unregister`) sofort mit Nullen überschreiben.
    // Ohne diese Einstellung blieben Public Key und Nickname als freier Speicher
    // in nexo.db lesbar, bis der Platz irgendwann neu belegt wird.
    connection.pragma_update(None, "secure_delete", true)?;

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
