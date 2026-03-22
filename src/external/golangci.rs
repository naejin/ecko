//! golangci-lint adapter -- run golangci-lint and parse JSON output into Echoes.
//!
//! Layer 3 only. Runs `golangci-lint run --out-format json ./...` at project
//! level, parses the `Issues` array (capital I), and post-filters to modified files.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::debug::debug;
use crate::echo::{emit, Echo, Severity};
use crate::external::run_with_timeout;

const TIMEOUT: Duration = Duration::from_secs(120);

/// Run golangci-lint on a Go project. Returns echoes grouped by absolute file path.
///
/// Post-filters results to `modified_files` when provided. Check names are
/// formatted as `go-{linter}` (e.g., `go-errcheck`). Severity is mapped from
/// the tool's `Severity` field.
pub fn run_golangci(cwd: &str, modified_files: &[String]) -> HashMap<String, Vec<Echo>> {
    let mut cmd = Command::new("golangci-lint");
    cmd.args(["run", "--out-format", "json", "./..."])
        .current_dir(cwd);

    debug("external: running golangci-lint");

    let output = match run_with_timeout(cmd, TIMEOUT, "golangci-lint") {
        Some(o) => o,
        None => return HashMap::new(),
    };

    let stdout = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };

    let stdout_trimmed = stdout.trim();
    if stdout_trimmed.is_empty() {
        // Check stderr for error messages.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_trimmed = stderr.trim();
        if !output.status.success() && !stderr_trimmed.is_empty() {
            let truncated: String = stderr_trimmed.chars().take(200).collect();
            emit(&format!(
                "~~ ecko ~~ warning: golangci-lint: {}\n",
                truncated
            ));
        }
        return HashMap::new();
    }

    let data: serde_json::Value = match serde_json::from_str(stdout_trimmed) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let issues = match data.get("Issues").and_then(|i| i.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return HashMap::new(),
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

    for issue in issues {
        let pos = match issue.get("Pos") {
            Some(p) => p,
            None => continue,
        };

        let rel_path = match pos.get("Filename").and_then(|f| f.as_str()) {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };

        let abs_path = normalize_path(&cwd_path.join(rel_path));

        // Post-filter to modified files.
        if let Some(ref ms) = modified_set {
            if !ms.contains(&abs_path) {
                continue;
            }
        }

        let line = pos.get("Line").and_then(|l| l.as_u64()).unwrap_or(0) as usize;

        let message = issue
            .get("Text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let linter = issue
            .get("FromLinter")
            .and_then(|l| l.as_str())
            .unwrap_or("unknown");

        let check = format!("go-{}", linter);

        let issue_severity = issue
            .get("Severity")
            .and_then(|s| s.as_str())
            .unwrap_or("warning");

        let severity = if issue_severity == "error" {
            Severity::Error
        } else {
            Severity::Warn
        };

        let abs_str = abs_path.to_string_lossy().to_string();
        file_echoes.entry(abs_str).or_default().push(Echo {
            check,
            line,
            message,
            suggestion: String::new(),
            severity,
            fix: None,
        });
    }

    debug(&format!(
        "external: golangci-lint found echoes in {} files",
        file_echoes.len()
    ));

    file_echoes
}

/// Normalize a path without requiring it to exist on disk.
fn normalize_path(path: &Path) -> PathBuf {
    crate::git::canonicalize_or_normalize(&path.to_string_lossy())
}
