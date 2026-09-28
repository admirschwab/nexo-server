use axum::http::StatusCode;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

// Prüft eine Signatur über Kontext + Public Key (32 Bytes) + weitere Daten.
// Gibt bei Erfolg den Public Key als Bytes zurück.
pub fn verify_signature(
    public_key: &str,
    signature: &str,
    context: &[u8],
    data: &[u8],
) -> Result<[u8; 32], StatusCode> {
    let public_key_bytes: [u8; 32] = hex::decode(public_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let signature_bytes = hex::decode(signature)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let signed_message = [context, public_key_bytes.as_slice(), data].concat();

    verifying_key
        .verify(&signed_message, &signature)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(public_key_bytes)
}
