//! Layer 1 autofix -- subprocess dispatch to formatters and in-process whitespace stripping.

use std::process::Command;

use crate::config::{self, EckoConfig};
use crate::debug;
use crate::lang::Lang;

/// Run Layer 1 autofix on a file based on its language and config.
///
/// Dispatches to external formatters (black, isort, prettier) if configured
/// and available on PATH. Always strips trailing whitespace in-process.
///
/// Failures are silent (debug-logged only) -- autofix is best-effort.
pub fn autofix(file_path: &str, lang: Lang, config: &EckoConfig) {
    match lang {
        Lang::Python => {
            if config::is_autofix_enabled(config, "black") {
                run_formatter("black", &["--quiet", file_path]);
            }
            if config::is_autofix_enabled(config, "isort") {
                run_formatter("isort", &["--profile", "black", file_path]);
            }
        }
        Lang::JavaScript | Lang::TypeScript | Lang::Tsx => {
            if config::is_autofix_enabled(config, "prettier") {
                run_formatter("prettier", &["--write", file_path]);
            }
        }
        _ => {}
    }

    // Always strip trailing whitespace.
    strip_trailing_whitespace(file_path);
}

// ---------------------------------------------------------------------------
// External formatter dispatch
// ---------------------------------------------------------------------------

/// Try to run an external formatter. Silently skip if not found.
fn run_formatter(tool: &str, args: &[&str]) {
    debug::debug(&format!("autofix: trying {tool}"));

    match Command::new(tool).args(args).output() {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                debug::debug(&format!("autofix: {tool} failed: {stderr}"));
            } else {
                debug::debug(&format!("autofix: {tool} succeeded"));
            }
        }
        Err(e) => {
            debug::debug(&format!("autofix: {tool} not found or failed to run: {e}"));
        }
    }
}

// ---------------------------------------------------------------------------
// In-process trailing whitespace stripping
// ---------------------------------------------------------------------------

/// Strip trailing whitespace from each line while preserving line endings.
///
/// Handles both `\r\n` and `\n` line endings. Only writes back if content changed.
fn strip_trailing_whitespace(file_path: &str) {
    let original = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            debug::debug(&format!("strip_trailing_whitespace: failed to read: {e}"));
            return;
        }
    };

    let mut result = String::with_capacity(original.len());
    let mut changed = false;
    let mut remaining = original.as_str();

    while !remaining.is_empty() {
        // Find line ending: check \r\n before \n (CRLF-safe).
        let (line, ending, rest) = if let Some(pos) = remaining.find("\r\n") {
            let nl_pos = remaining.find('\n').unwrap_or(pos + 1);
            if pos < nl_pos || pos == nl_pos.saturating_sub(1) {
                (&remaining[..pos], "\r\n", &remaining[pos + 2..])
            } else {
                // \n comes before \r\n
                (&remaining[..nl_pos], "\n", &remaining[nl_pos + 1..])
            }
        } else if let Some(pos) = remaining.find('\n') {
            (&remaining[..pos], "\n", &remaining[pos + 1..])
        } else {
            // Last line without trailing newline.
            (remaining, "", "")
        };

        let stripped = line.trim_end();
        if stripped.len() != line.len() {
            changed = true;
        }
        result.push_str(stripped);
        result.push_str(ending);
        remaining = rest;
    }

    if changed {
        if let Err(e) = std::fs::write(file_path, &result) {
            debug::debug(&format!("strip_trailing_whitespace: failed to write: {e}"));
        } else {
            debug::debug("strip_trailing_whitespace: stripped trailing whitespace");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_strip_trailing_whitespace_lf() {
        let dir = std::env::temp_dir().join("ecko_test_strip_lf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.py");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "hello   \nworld  \n").unwrap();
        drop(f);

        strip_trailing_whitespace(path.to_str().unwrap());

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "hello\nworld\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_strip_trailing_whitespace_crlf() {
        let dir = std::env::temp_dir().join("ecko_test_strip_crlf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.py");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "hello   \r\nworld  \r\n").unwrap();
        drop(f);

        strip_trailing_whitespace(path.to_str().unwrap());

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "hello\r\nworld\r\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_strip_no_change() {
        let dir = std::env::temp_dir().join("ecko_test_strip_nochange");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.py");
        let content = "hello\nworld\n";
        std::fs::write(&path, content).unwrap();

        strip_trailing_whitespace(path.to_str().unwrap());

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, content);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
