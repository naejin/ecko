//! Inline ecko:ignore suppression.
//!
//! Scans file lines for `ecko:ignore` comments. For each echo, checks if the
//! same line or the line above has an ecko:ignore directive. If the directive
//! lists specific check names, only those are suppressed; otherwise all checks
//! on that line are suppressed.

use crate::echo::Echo;

/// Filter echoes by removing those suppressed by `ecko:ignore` comments.
///
/// Reads the file to find suppression directives. If the file cannot be read,
/// returns echoes unfiltered (fail-open).
pub fn filter_suppressed(echoes: Vec<Echo>, file_path: &str) -> Vec<Echo> {
    if echoes.is_empty() {
        return echoes;
    }

    let source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(_) => return echoes, // fail-open
    };

    let lines: Vec<&str> = source.lines().collect();
    let suppressions = parse_suppressions(&lines);

    echoes
        .into_iter()
        .filter(|echo| !is_suppressed(echo, &suppressions))
        .collect()
}

// ---------------------------------------------------------------------------
// Internal types and parsing
// ---------------------------------------------------------------------------

/// A parsed ecko:ignore directive.
struct Suppression {
    /// 1-based line number this suppression applies to.
    line: usize,
    /// Specific check names to suppress. Empty = suppress all.
    checks: Vec<String>,
}

/// Parse all ecko:ignore directives from file lines.
fn parse_suppressions(lines: &[&str]) -> Vec<Suppression> {
    let mut suppressions = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1; // 1-based

        if let Some(directive) = extract_ecko_ignore(line) {
            let checks = parse_check_names(&directive);

            if is_standalone_comment(line) {
                // Standalone comment: suppresses the next line.
                suppressions.push(Suppression {
                    line: line_num + 1,
                    checks,
                });
            } else {
                // Inline comment: suppresses this line.
                suppressions.push(Suppression {
                    line: line_num,
                    checks,
                });
            }
        }
    }

    suppressions
}

/// Extract the part after `ecko:ignore` from a line, if present.
///
/// Returns `None` if no `ecko:ignore` is found.
fn extract_ecko_ignore(line: &str) -> Option<String> {
    let needle = "ecko:ignore";
    let pos = line.find(needle)?;
    let rest = &line[pos + needle.len()..];
    Some(rest.to_string())
}

/// Parse check names from the text after `ecko:ignore`.
///
/// E.g., `" unused-imports, bare-except"` -> `["unused-imports", "bare-except"]`
/// Empty/whitespace-only -> empty vec (suppress all).
fn parse_check_names(directive: &str) -> Vec<String> {
    let trimmed = directive.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    trimmed
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Check if a line is a standalone comment (not inline with code).
///
/// A line is standalone if, after stripping whitespace, it starts with
/// `#`, `//`, or `/*`.
fn is_standalone_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.starts_with("/*")
}

/// Check if an echo is suppressed by any of the given suppressions.
fn is_suppressed(echo: &Echo, suppressions: &[Suppression]) -> bool {
    for sup in suppressions {
        if sup.line != echo.line {
            continue;
        }
        // Empty checks list = suppress all checks on this line.
        if sup.checks.is_empty() {
            return true;
        }
        // Otherwise, only suppress if the echo's check is in the list.
        if sup.checks.iter().any(|c| c == &echo.check) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::echo::Severity;

    fn make_echo(line: usize, check: &str) -> Echo {
        Echo {
            check: check.to_string(),
            line,
            message: "test".to_string(),
            suggestion: String::new(),
            severity: Severity::Warn,
            fix: None,
        }
    }

    #[test]
    fn test_standalone_comment_suppresses_next_line() {
        let lines = vec!["# ecko:ignore", "x = 1"];
        let suppressions = parse_suppressions(&lines);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 2);
        assert!(suppressions[0].checks.is_empty()); // suppress all
    }

    #[test]
    fn test_inline_comment_suppresses_same_line() {
        let lines = vec!["x = 1  # ecko:ignore"];
        let suppressions = parse_suppressions(&lines);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 1);
        assert!(suppressions[0].checks.is_empty());
    }

    #[test]
    fn test_specific_checks_suppressed() {
        let lines = vec!["x = 1  # ecko:ignore unused-imports, bare-except"];
        let suppressions = parse_suppressions(&lines);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(
            suppressions[0].checks,
            vec!["unused-imports", "bare-except"]
        );
    }

    #[test]
    fn test_js_comment_standalone() {
        let lines = vec!["// ecko:ignore star-imports", "import * from 'foo';"];
        let suppressions = parse_suppressions(&lines);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 2);
        assert_eq!(suppressions[0].checks, vec!["star-imports"]);
    }

    #[test]
    fn test_is_suppressed_all() {
        let sup = Suppression {
            line: 5,
            checks: vec![],
        };
        let echo = make_echo(5, "any-check");
        assert!(is_suppressed(&echo, &[sup]));
    }

    #[test]
    fn test_is_suppressed_specific_match() {
        let sup = Suppression {
            line: 5,
            checks: vec!["bare-except".to_string()],
        };
        let echo = make_echo(5, "bare-except");
        assert!(is_suppressed(&echo, &[sup]));
    }

    #[test]
    fn test_is_suppressed_specific_no_match() {
        let sup = Suppression {
            line: 5,
            checks: vec!["bare-except".to_string()],
        };
        let echo = make_echo(5, "star-imports");
        assert!(!is_suppressed(&echo, &[sup]));
    }

    #[test]
    fn test_is_suppressed_wrong_line() {
        let sup = Suppression {
            line: 5,
            checks: vec![],
        };
        let echo = make_echo(3, "any-check");
        assert!(!is_suppressed(&echo, &[sup]));
    }
}
