//! tsc adapter -- run TypeScript compiler and parse error output into Echoes.
//!
//! Layer 3 only. Runs `tsc --noEmit` in the project root (requires tsconfig.json)
//! and parses the text error format: `path(line,col): error TSxxxx: message`.

use std::collections::HashMap;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;

use crate::debug::debug;
use crate::echo::{Echo, Severity};
use crate::external::run_with_timeout;

const TIMEOUT: Duration = Duration::from_secs(60);

/// Compiled regex for tsc error output format.
fn tsc_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+?)\((\d+),(\d+)\):\s+error\s+TS\d+:\s+(.+)$").unwrap())
}

/// Run `tsc --noEmit` in the project root. Returns echoes grouped by file.
///
/// tsc writes errors to stdout in the format `path(line,col): error TSxxxx: message`.
pub fn run_tsc(cwd: &str) -> HashMap<String, Vec<Echo>> {
    let mut cmd = Command::new("tsc");
    cmd.arg("--noEmit").current_dir(cwd);

    debug("external: running tsc --noEmit");

    let output = match run_with_timeout(cmd, TIMEOUT, "tsc") {
        Some(o) => o,
        None => return HashMap::new(),
    };

    // tsc writes errors to stdout, but also check stderr.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    if combined.trim().is_empty() {
        return HashMap::new();
    }

    let re = tsc_pattern();
    let mut file_echoes: HashMap<String, Vec<Echo>> = HashMap::new();

    for line in combined.lines() {
        let line = line.trim();
        if let Some(caps) = re.captures(line) {
            let path = caps[1].to_string();
            let lineno: usize = caps[2].parse().unwrap_or(0);
            let message = caps[4].to_string();

            file_echoes.entry(path).or_default().push(Echo {
                check: "type-error".to_string(),
                line: lineno,
                message,
                suggestion: String::new(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    debug(&format!(
        "external: tsc found echoes in {} files",
        file_echoes.len()
    ));

    file_echoes
}
