//! Debug output controlled by `ECKO_DEBUG=1` environment variable.
//!
//! Pure utility -- no dependencies on other ecko modules.

use std::sync::OnceLock;

static DEBUG: OnceLock<bool> = OnceLock::new();

fn is_debug() -> bool {
    *DEBUG.get_or_init(|| std::env::var("ECKO_DEBUG").is_ok_and(|v| v == "1"))
}

/// Emit a debug message to stderr if `ECKO_DEBUG=1` is set.
pub fn debug(msg: &str) {
    if is_debug() {
        eprintln!("[ecko debug] {}", msg);
    }
}
