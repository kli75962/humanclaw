use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// How long a pairing token remains valid after generation.
const TOKEN_TTL: Duration = Duration::from_secs(300); // 5 minutes

static TOKEN: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// Generate a new one-time pairing token (64 hex chars), replacing any previous one.
/// The real hash_key is NEVER embedded in the QR — only this token is.
pub fn generate() -> String {
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    *TOKEN.lock().unwrap() = Some((token.clone(), Instant::now()));
    token
}

/// Check if `value` matches the active token and it hasn't expired.
/// On a successful match the token is immediately consumed (one-time use).
pub fn validate_and_consume(value: &str) -> bool {
    let mut guard = TOKEN.lock().unwrap();
    let matched = guard
        .as_ref()
        .is_some_and(|(t, created)| t == value && created.elapsed() < TOKEN_TTL);
    if guard.as_ref().is_some_and(|(_, created)| created.elapsed() >= TOKEN_TTL) {
        *guard = None; // evict expired token
    }
    if matched {
        *guard = None; // consume — can never be used again
    }
    matched
}
