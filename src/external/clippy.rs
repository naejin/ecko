//! Clippy adapter -- run cargo clippy and parse streaming JSON output into Echoes.
//!
//! Layer 3 only. Runs `cargo clippy --message-format=json -- -W clippy::all` at
//! project level. Parses streaming JSON (one object per stdout line), filters to
//! `reason == "compiler-message"`, uses primary spans (`is_primary: true`), and
//! post-filters to modified files.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::debug::debug;
use crate::echo::{Echo, Severity};
use crate::external::run_with_timeout;

const TIMEOUT: Duration = Duration::from_secs(120);

/// Run cargo clippy on a Rust project. Returns echoes grouped by absolute file path.
///
/// Requires `Cargo.toml` in `cwd`. Check names are formatted as `rust-{code}`
/// (e.g., `rust-clippy::needless_return`). Severity is mapped from the `level` field.
pub fn run_clippy(cwd: &str, modified_files: &[String]) -> HashMap<String, Vec<Echo>> {
    // Gate on Cargo.toml existing in cwd.
    let cargo_toml = Path::new(cwd).join("Cargo.toml");
    if !cargo_toml.is_file() {
        debug("external: clippy skipped -- no Cargo.toml");
        return HashMap::new();
    }

    let mut cmd = Command::new("cargo");
    cmd.args(["clippy", "--message-format=json", "--", "-W", "clippy::all"])
        .current_dir(cwd);

    debug("external: running cargo clippy");

    let output = match run_with_timeout(cmd, TIMEOUT, "clippy") {
        Some(o) => o,
        None => return HashMap::new(),
    };

    let stdout = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };

    // Build modified file set for post-filtering.
    let modified_set: Option<HashSet<PathBuf>> = if modified_files.is_empty() {
        None
    } else {
        Some(
            modified_files
                .iter()
                .map(|f| crate::git::canonicalize_or_normalize(f))
                .collect(),
        )
    };

    let cwd_path = Path::new(cwd);
    let mut file_echoes: HashMap<String, Vec<Echo>> = HashMap::new();

    // Streaming JSON: one JSON object per stdout line.
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Only process compiler messages.
        if obj.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }

        let msg = match obj.get("message") {
            Some(m) => m,
            None => continue,
        };

        // Must have a code with a non-empty code string.
        let code = match msg
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
        {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        let text = msg
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        let spans = match msg.get("spans").and_then(|s| s.as_array()) {
            Some(arr) if !arr.is_empty() => arr,
            _ => continue,
        };

        // Use the primary span (clippy marks it with is_primary: true).
        let span = spans
            .iter()
            .find(|s| s.get("is_primary").and_then(|p| p.as_bool()) == Some(true))
            .unwrap_or(&spans[0]);

        let file_name = match span.get("file_name").and_then(|f| f.as_str()) {
            Some(f) if !f.is_empty() => f,
            _ => continue,
        };

        let abs_path = normalize_path(&cwd_path.join(file_name));

        // Post-filter to modified files.
        if let Some(ref ms) = modified_set {
            if !ms.contains(&abs_path) {
                continue;
            }
        }

        let line_num = span.get("line_start").and_then(|l| l.as_u64()).unwrap_or(0) as usize;

        let check = format!("rust-{}", code);

        let level = msg
            .get("level")
            .and_then(|l| l.as_str())
            .unwrap_or("warning");

        let severity = if level == "error" {
            Severity::Error
        } else {
            Severity::Warn
        };

        let abs_str = abs_path.to_string_lossy().to_string();
        file_echoes.entry(abs_str).or_default().push(Echo {
            check,
            line: line_num,
            message: text,
            suggestion: String::new(),
            severity,
            fix: None,
        });
    }

    debug(&format!(
        "external: clippy found echoes in {} files",
        file_echoes.len()
    ));

    file_echoes
}

/// Normalize a path without requiring it to exist on disk.
fn normalize_path(path: &Path) -> PathBuf {
    crate::git::canonicalize_or_normalize(&path.to_string_lossy())
}
