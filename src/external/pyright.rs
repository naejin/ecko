//! Pyright adapter -- run pyright and parse JSON output into Echoes.
//!
//! Layer 3 only. Runs `pyright --outputjson <files>` and extracts
//! `generalDiagnostics` with severity "error", skipping unresolved imports.

use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use crate::debug::debug;
use crate::echo::{Echo, Severity};
use crate::external::run_with_timeout;

const TIMEOUT: Duration = Duration::from_secs(60);

/// Run pyright on the given files. Returns echoes grouped by file path.
///
/// Filters: only severity=="error", skips "could not be resolved" messages
/// (missing deps, not code defects). Pyright uses 0-indexed lines -> +1.
pub fn run_pyright(files: &[String], cwd: &str) -> HashMap<String, Vec<Echo>> {
    if files.is_empty() {
        return HashMap::new();
    }

    let mut cmd = Command::new("pyright");
    cmd.arg("--outputjson").args(files).current_dir(cwd);

    debug(&format!(
        "external: running pyright on {} files",
        files.len()
    ));

    let output = match run_with_timeout(cmd, TIMEOUT, "pyright") {
        Some(o) => o,
        None => return HashMap::new(),
    };

    let stdout = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };

    let stdout = stdout.trim();
    if stdout.is_empty() {
        return HashMap::new();
    }

    let data: serde_json::Value = match serde_json::from_str(stdout) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let mut file_echoes: HashMap<String, Vec<Echo>> = HashMap::new();

    let diagnostics = match data.get("generalDiagnostics").and_then(|d| d.as_array()) {
        Some(arr) => arr,
        None => return HashMap::new(),
    };

    for diag in diagnostics {
        let severity = diag.get("severity").and_then(|s| s.as_str()).unwrap_or("");
        if severity != "error" {
            continue;
        }

        let path = match diag.get("file").and_then(|f| f.as_str()) {
            Some(p) => p.to_string(),
            None => continue,
        };

        let message = diag
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        // Skip unresolved imports -- indicates missing deps, not code defects.
        if message.contains("could not be resolved") {
            continue;
        }

        // Pyright uses 0-indexed lines.
        let line = diag
            .get("range")
            .and_then(|r| r.get("start"))
            .and_then(|s| s.get("line"))
            .and_then(|l| l.as_u64())
            .map(|l| l as usize + 1)
            .unwrap_or(0);

        file_echoes.entry(path).or_default().push(Echo {
            check: "type-error".to_string(),
            line,
            message,
            suggestion: String::new(),
            severity: Severity::Error,
            fix: None,
        });
    }

    debug(&format!(
        "external: pyright found echoes in {} files",
        file_echoes.len()
    ));

    file_echoes
}
