pub const NICKNAME_MIN_LENGTH: usize = 3;
pub const NICKNAME_MAX_LENGTH: usize = 20;

// Erlaubt sind Buchstaben, Ziffern, '_' und '-' (nur ASCII)
pub fn validate_nickname(nickname: &str) -> bool {
    (NICKNAME_MIN_LENGTH..=NICKNAME_MAX_LENGTH).contains(&nickname.len())
        && nickname
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
