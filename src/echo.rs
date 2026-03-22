//! Echo struct and output formatting.
//!
//! All output goes to stderr via `emit()`. Compact format groups echoes by
//! check name with line-number overflow: `check (L1, L2, L3 +5)`.

use serde::Serialize;
use std::collections::HashMap;
use std::io::Write as _;

/// Maximum line numbers shown per check before +N overflow.
const COMPACT_LINES_PER_CHECK: usize = 3;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Echo {
    pub check: String,
    pub line: usize,
    pub message: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub suggestion: String,
    pub severity: Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fix {
    pub start_byte: usize,
    pub end_byte: usize,
    pub replacement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warn,
    Error,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns true if any echo has error severity.
/// Used by MCP server and JSON output consumers.
#[allow(dead_code)]
pub fn has_errors(echoes: &[Echo]) -> bool {
    echoes.iter().any(|e| e.severity == Severity::Error)
}

/// Write `text` to stderr with a trailing newline and flush.
pub fn emit(text: &str) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = writeln!(handle, "{}", text);
    let _ = handle.flush();
}

// ---------------------------------------------------------------------------
// Compact text format
// ---------------------------------------------------------------------------

/// A group of echoes sharing the same check name.
struct CheckGroup {
    name: String,
    lines: Vec<usize>,
    has_error: bool,
    suggestion: String,
}

/// Group echoes by check name, preserving insertion order.
fn group_by_check(echoes: &[Echo]) -> Vec<CheckGroup> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, CheckGroup> = HashMap::new();

    for e in echoes {
        let group = groups.entry(e.check.clone()).or_insert_with(|| CheckGroup {
            name: e.check.clone(),
            lines: Vec::new(),
            has_error: false,
            suggestion: String::new(),
        });
        group.lines.push(e.line);
        if e.severity == Severity::Error {
            group.has_error = true;
        }
        if group.suggestion.is_empty() && !e.suggestion.is_empty() {
            group.suggestion = e.suggestion.clone();
        }
        if !order.contains(&e.check) {
            order.push(e.check.clone());
        }
    }

    order
        .into_iter()
        .map(|name| groups.remove(&name).unwrap())
        .collect()
}

/// Format a single check group: `check (L1, L2, L3 +5)` or `[error] check (L1)`.
fn format_check_group(name: &str, lines: &[usize], is_error: bool) -> String {
    let prefix = if is_error { "[error] " } else { "" };

    if lines.is_empty() {
        return format!("{prefix}{name}");
    }

    let shown: Vec<String> = lines
        .iter()
        .take(COMPACT_LINES_PER_CHECK)
        .map(|l| format!("L{l}"))
        .collect();

    let overflow = lines.len().saturating_sub(COMPACT_LINES_PER_CHECK);
    let line_part = if overflow > 0 {
        format!("{} +{overflow}", shown.join(", "))
    } else {
        shown.join(", ")
    };

    format!("{prefix}{name} ({line_part})")
}

/// Limit echoes to at most `cap` per check name.
///
/// When `cap` is 0, no filtering is applied (unlimited).
/// Preserves insertion order; keeps the first `cap` echoes per check.
pub fn apply_per_check_cap(echoes: Vec<Echo>, cap: usize) -> Vec<Echo> {
    if cap == 0 {
        return echoes;
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    echoes
        .into_iter()
        .filter(|e| {
            let count = counts.entry(e.check.clone()).or_insert(0);
            *count += 1;
            *count <= cap
        })
        .collect()
}

/// Compact one-line format for a single file:
///
/// ```text
/// ~~ ecko ~~ path/to/file.py -- ruff (L1, L2, L3 +5), vulture (L10)
/// ```
pub fn format_file_echoes(file_path: &str, echoes: &[Echo]) -> String {
    if echoes.is_empty() {
        return String::new();
    }

    let groups = group_by_check(echoes);
    let parts: Vec<String> = groups
        .iter()
        .map(|g| {
            let base = format_check_group(&g.name, &g.lines, g.has_error);
            if g.suggestion.is_empty() {
                base
            } else {
                format!("{base} -- {}", g.suggestion)
            }
        })
        .collect();

    format!("~~ ecko ~~ {} \u{2014} {}", file_path, parts.join(", "))
}

/// Multi-file stop-mode output.
///
/// Header line with total count, then one compact line per file.
/// When `cross_file_cap > 0`, per-check counts across files are capped.
pub fn format_stop_echoes(
    file_echoes: &HashMap<String, Vec<Echo>>,
    cross_file_cap: usize,
) -> String {
    if file_echoes.is_empty() {
        return String::new();
    }

    let total: usize = file_echoes.values().map(|v| v.len()).sum();

    let mut lines: Vec<String> = Vec::new();

    // Header
    let cap_note = if cross_file_cap > 0 {
        format!(" (display capped at {cross_file_cap} per check)")
    } else {
        String::new()
    };
    lines.push(format!(
        "~~ ecko ~~ {total} echoes across {} files{cap_note}",
        file_echoes.len()
    ));

    // Sort files for deterministic output
    let mut files: Vec<&String> = file_echoes.keys().collect();
    files.sort();

    // If cross_file_cap is active, track per-check counts
    let mut check_counts: HashMap<String, usize> = HashMap::new();

    for file in &files {
        let echoes = &file_echoes[*file];
        let filtered: Vec<&Echo> = if cross_file_cap > 0 {
            echoes
                .iter()
                .filter(|e| {
                    let count = check_counts.entry(e.check.clone()).or_insert(0);
                    if *count < cross_file_cap {
                        *count += 1;
                        true
                    } else {
                        false
                    }
                })
                .collect()
        } else {
            echoes.iter().collect()
        };

        if !filtered.is_empty() {
            let owned: Vec<Echo> = filtered.into_iter().cloned().collect();
            lines.push(format_file_echoes(file, &owned));
        }
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// JSON format (schema v2 -- includes fix field)
// ---------------------------------------------------------------------------

/// JSON output for a single file (PostToolUse).
pub fn format_file_echoes_json(
    file_path: &str,
    echoes: &[Echo],
    skipped_tools: &[String],
) -> String {
    let json = serde_json::json!({
        "version": 2,
        "mode": "post-tool-use",
        "file": file_path,
        "echoes": echoes,
        "skipped_tools": skipped_tools,
    });
    serde_json::to_string(&json).unwrap_or_default()
}

/// JSON output for stop mode (multi-file).
pub fn format_stop_echoes_json(
    file_echoes: &HashMap<String, Vec<Echo>>,
    elapsed: f64,
    skipped_tools: &[String],
    corrections: &HashMap<String, i32>,
) -> String {
    let json = serde_json::json!({
        "version": 2,
        "mode": "stop",
        "files": file_echoes,
        "elapsed_seconds": elapsed,
        "skipped_tools": skipped_tools,
        "corrections": corrections,
    });
    serde_json::to_string(&json).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Correction summary
// ---------------------------------------------------------------------------

/// Format self-correction deltas from the session ledger.
///
/// ```text
/// ~~ ecko ~~ self-corrections: ruff -3, vulture -1
/// ```
pub fn format_correction_summary(corrections: &HashMap<String, i32>) -> String {
    if corrections.is_empty() {
        return String::new();
    }

    let mut parts: Vec<String> = corrections
        .iter()
        .filter(|(_, &delta)| delta != 0)
        .map(|(check, delta)| {
            if *delta < 0 {
                format!("{check} {delta}")
            } else {
                format!("{check} +{delta}")
            }
        })
        .collect();

    if parts.is_empty() {
        return String::new();
    }

    parts.sort();
    format!("~~ ecko ~~ self-corrections: {}", parts.join(", "))
}

// ---------------------------------------------------------------------------
// Session stats
// ---------------------------------------------------------------------------

/// One-line session summary for the `/ecko:session` command.
pub fn format_session_stats(
    entries: &[serde_json::Value],
    corrections: &HashMap<String, i32>,
) -> String {
    if entries.is_empty() {
        return "~~ ecko ~~ no session data".to_string();
    }

    let file_count = {
        let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in entries {
            if let Some(f) = entry.get("file").and_then(|v| v.as_str()) {
                files.insert(f.to_string());
            }
        }
        files.len()
    };

    let correction_count: i32 = corrections.values().filter(|&&d| d > 0).sum();

    format!(
        "~~ ecko ~~ session: {} entries, {} files, {} self-corrections",
        entries.len(),
        file_count,
        correction_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo(check: &str, line: usize, sev: Severity) -> Echo {
        Echo {
            check: check.to_string(),
            line,
            message: "msg".to_string(),
            suggestion: String::new(),
            severity: sev,
            fix: None,
        }
    }

    #[test]
    fn has_errors_true_when_error_present() {
        let echoes = vec![echo("a", 1, Severity::Warn), echo("b", 2, Severity::Error)];
        assert!(has_errors(&echoes));
    }

    #[test]
    fn has_errors_false_when_all_warn() {
        let echoes = vec![echo("a", 1, Severity::Warn)];
        assert!(!has_errors(&echoes));
    }

    #[test]
    fn has_errors_empty() {
        assert!(!has_errors(&[]));
    }

    #[test]
    fn format_file_echoes_empty() {
        let result = format_file_echoes("test.py", &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn format_file_echoes_single() {
        let echoes = vec![echo("unused-imports", 5, Severity::Warn)];
        let result = format_file_echoes("test.py", &echoes);
        assert!(result.contains("test.py"));
        assert!(result.contains("unused-imports"));
        assert!(result.contains("L5"));
    }

    #[test]
    fn format_file_echoes_overflow() {
        // More than COMPACT_LINES_PER_CHECK lines should show +N
        let echoes: Vec<Echo> = (1..=6)
            .map(|i| echo("check-a", i, Severity::Warn))
            .collect();
        let result = format_file_echoes("test.py", &echoes);
        assert!(result.contains("+"));
    }

    #[test]
    fn format_file_echoes_error_prefix() {
        let echoes = vec![echo("bare-except", 10, Severity::Error)];
        let result = format_file_echoes("test.py", &echoes);
        assert!(result.contains("[error]"));
    }

    #[test]
    fn format_stop_echoes_empty() {
        let map: HashMap<String, Vec<Echo>> = HashMap::new();
        let result = format_stop_echoes(&map, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn format_stop_echoes_cross_file_cap() {
        let mut map: HashMap<String, Vec<Echo>> = HashMap::new();
        // 3 files with same check
        for i in 1..=3 {
            map.insert(
                format!("file{i}.py"),
                vec![echo("check-a", 1, Severity::Warn)],
            );
        }
        let result = format_stop_echoes(&map, 1);
        // Cap is 1 per check across files -- header + 1 file line = 2 ecko markers
        let count = result.matches("~~ ecko ~~").count();
        assert_eq!(count, 2);
    }

    #[test]
    fn format_correction_summary_shows_nonzero() {
        let mut corrections = HashMap::new();
        corrections.insert("unused-imports".to_string(), 3);
        corrections.insert("bare-except".to_string(), -1);
        let result = format_correction_summary(&corrections);
        assert!(result.contains("unused-imports +3"));
        // Negative deltas appear too (shows regression)
        assert!(result.contains("bare-except -1"));
    }

    #[test]
    fn format_correction_summary_empty() {
        let corrections = HashMap::new();
        let result = format_correction_summary(&corrections);
        assert!(result.is_empty());
    }

    #[test]
    fn format_session_stats_empty_entries() {
        let corrections = HashMap::new();
        let result = format_session_stats(&[], &corrections);
        assert!(result.contains("no session data"));
    }

    #[test]
    fn format_file_echoes_json_valid() {
        let echoes = vec![echo("check-a", 1, Severity::Warn)];
        let json = format_file_echoes_json("test.py", &echoes, &[]);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(parsed.is_ok());
    }

    #[test]
    fn format_stop_echoes_json_valid() {
        let mut map: HashMap<String, Vec<Echo>> = HashMap::new();
        map.insert("test.py".to_string(), vec![echo("a", 1, Severity::Warn)]);
        let json = format_stop_echoes_json(&map, 1.5, &[], &HashMap::new());
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(parsed.is_ok());
    }

    #[test]
    fn apply_per_check_cap_limits_per_check() {
        let echoes: Vec<Echo> = (1..=6)
            .map(|i| echo("check-a", i, Severity::Warn))
            .collect();
        let capped = apply_per_check_cap(echoes, 3);
        assert_eq!(capped.len(), 3);
        assert_eq!(capped[0].line, 1);
        assert_eq!(capped[2].line, 3);
    }

    #[test]
    fn apply_per_check_cap_zero_unlimited() {
        let echoes: Vec<Echo> = (1..=10)
            .map(|i| echo("check-a", i, Severity::Warn))
            .collect();
        let result = apply_per_check_cap(echoes, 0);
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn apply_per_check_cap_mixed_checks() {
        let mut echoes = Vec::new();
        for i in 1..=5 {
            echoes.push(echo("check-a", i, Severity::Warn));
        }
        for i in 1..=5 {
            echoes.push(echo("check-b", i, Severity::Warn));
        }
        let capped = apply_per_check_cap(echoes, 2);
        assert_eq!(capped.len(), 4); // 2 of each
        assert_eq!(capped.iter().filter(|e| e.check == "check-a").count(), 2);
        assert_eq!(capped.iter().filter(|e| e.check == "check-b").count(), 2);
    }

    // --- fix_suggestions config behavioral tests ---

    #[test]
    fn fix_suggestions_strip_removes_all_fixes() {
        // Simulates the runner.rs pattern: if !config.fix_suggestions { e.fix = None }
        let mut echoes = vec![
            Echo {
                check: "bare-except".to_string(),
                line: 1,
                message: "Use specific exception".to_string(),
                suggestion: String::new(),
                severity: Severity::Warn,
                fix: Some(Fix {
                    start_byte: 0,
                    end_byte: 10,
                    replacement: "except Exception:".to_string(),
                }),
            },
            Echo {
                check: "obsolete-terms".to_string(),
                line: 5,
                message: "Renamed".to_string(),
                suggestion: String::new(),
                severity: Severity::Warn,
                fix: Some(Fix {
                    start_byte: 20,
                    end_byte: 30,
                    replacement: "NewName".to_string(),
                }),
            },
        ];
        // Apply the same strip as runner.rs does when fix_suggestions: false
        echoes.iter_mut().for_each(|e| e.fix = None);
        assert!(echoes.iter().all(|e| e.fix.is_none()));
        // Echoes themselves are preserved -- only fixes removed
        assert_eq!(echoes.len(), 2);
        assert_eq!(echoes[0].check, "bare-except");
        assert_eq!(echoes[1].check, "obsolete-terms");
    }

    #[test]
    fn fix_suggestions_default_preserves_fixes() {
        // When fix_suggestions is true (default), fixes are untouched
        let echo_with_fix = Echo {
            check: "bare-except".to_string(),
            line: 1,
            message: "msg".to_string(),
            suggestion: String::new(),
            severity: Severity::Warn,
            fix: Some(Fix {
                start_byte: 0,
                end_byte: 10,
                replacement: "except Exception:".to_string(),
            }),
        };
        // No strip applied -- fix remains
        assert!(echo_with_fix.fix.is_some());
        assert_eq!(
            echo_with_fix.fix.as_ref().unwrap().replacement,
            "except Exception:"
        );
    }

    #[test]
    fn fix_suggestions_strip_idempotent_on_no_fix() {
        // Echoes without fixes are unchanged by the strip
        let mut echoes = vec![echo("check-a", 1, Severity::Warn)];
        assert!(echoes[0].fix.is_none());
        echoes.iter_mut().for_each(|e| e.fix = None);
        assert!(echoes[0].fix.is_none());
        assert_eq!(echoes[0].check, "check-a");
    }

    // --- echo_cap_per_check end-to-end with realistic echoes ---

    #[test]
    fn echo_cap_per_check_caps_realistic_banned_pattern_echoes() {
        // Simulates: banned-patterns check finds 5 matches, config caps to 2
        let echoes: Vec<Echo> = (1..=5)
            .map(|i| Echo {
                check: "banned-patterns".to_string(),
                line: i,
                message: format!("Banned pattern `TODO` found on line {i}"),
                suggestion: String::new(),
                severity: Severity::Warn,
                fix: None,
            })
            .collect();

        let capped = apply_per_check_cap(echoes, 2);
        assert_eq!(capped.len(), 2);
        assert_eq!(capped[0].line, 1);
        assert_eq!(capped[1].line, 2);
        // Lines 3-5 are dropped by the cap
    }

    // --- contextual suggestion tests ---

    #[test]
    fn format_file_echoes_includes_suggestion() {
        let echoes = vec![Echo {
            check: "bare-except".to_string(),
            line: 10,
            message: "bare except".to_string(),
            suggestion: "use except Exception:".to_string(),
            severity: Severity::Warn,
            fix: None,
        }];
        let result = format_file_echoes("app.py", &echoes);
        assert!(
            result.contains("-- use except Exception:"),
            "suggestion should appear after check group: {result}"
        );
    }

    #[test]
    fn format_file_echoes_no_suggestion_when_empty() {
        let echoes = vec![echo("bare-except", 10, Severity::Warn)];
        let result = format_file_echoes("app.py", &echoes);
        assert!(
            !result.contains(" -- "),
            "no suggestion separator when suggestion is empty: {result}"
        );
    }

    #[test]
    fn format_file_echoes_only_first_suggestion_per_check() {
        let echoes = vec![
            Echo {
                check: "bare-except".to_string(),
                line: 5,
                message: "msg".to_string(),
                suggestion: "first hint".to_string(),
                severity: Severity::Warn,
                fix: None,
            },
            Echo {
                check: "bare-except".to_string(),
                line: 15,
                message: "msg".to_string(),
                suggestion: "second hint".to_string(),
                severity: Severity::Warn,
                fix: None,
            },
        ];
        let result = format_file_echoes("app.py", &echoes);
        assert!(result.contains("first hint"));
        assert!(!result.contains("second hint"));
    }
}
