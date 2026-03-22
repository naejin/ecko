//! Session ledger -- append-only JSONL log with rolling time window.
//!
//! Records echo counts per hook invocation for self-correction tracking.
//! Each post-tool-use appends one entry; the stop hook reads and summarizes.
//! Stale entries (outside the session window) are filtered at read time.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LEDGER_DIR: &str = ".ecko-session";
const LEDGER_FILE: &str = "ledger.jsonl";
const PRUNE_SIZE_THRESHOLD: u64 = 50_000; // 50KB

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub ts: f64,
    pub file: String,
    pub mode: String,
    pub echoes: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn ledger_path(cwd: &str) -> PathBuf {
    Path::new(cwd).join(LEDGER_DIR).join(LEDGER_FILE)
}

fn ensure_dir(cwd: &str) -> Result<(), std::io::Error> {
    let dir = Path::new(cwd).join(LEDGER_DIR);
    std::fs::create_dir_all(&dir)?;
    let gitignore = dir.join(".gitignore");
    if !gitignore.is_file() {
        std::fs::write(&gitignore, "*\n")?;
    }
    Ok(())
}

fn now_epoch() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Compute relative path from `cwd`, using forward slashes for portability.
fn relative_path(file_path: &str, cwd: &str) -> String {
    crate::git::relative_path(file_path, cwd)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Append a ledger entry.
///
/// Creates `.ecko-session/` with `.gitignore` if needed. Converts `file_path`
/// to a relative path for portable storage. All I/O is guarded -- failures are
/// silently ignored (ledger is best-effort).
pub fn append(cwd: &str, file_path: &str, mode: &str, echoes: &HashMap<String, usize>) {
    if ensure_dir(cwd).is_err() {
        return; // Can't create directory -- skip ledger write
    }

    let rel = relative_path(file_path, cwd);
    let ts = (now_epoch() * 10.0).round() / 10.0; // round to 1 decimal

    let entry = LedgerEntry {
        ts,
        file: rel,
        mode: mode.to_string(),
        echoes: echoes.clone(),
    };

    let path = ledger_path(cwd);

    // True append -- no read-modify-write, safe under concurrent access.
    let line = match serde_json::to_string(&entry) {
        Ok(s) => s,
        Err(_) => return,
    };

    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);
    if let Ok(mut f) = file {
        let _ = writeln!(f, "{}", line);
    }
}

/// Read all ledger entries within the current session window.
///
/// Entries older than `session_hours` are filtered out at read time.
/// Triggers pruning when the file is large and mostly stale.
pub fn read_session(cwd: &str, session_hours: f64) -> Vec<LedgerEntry> {
    let path = ledger_path(cwd);
    let cutoff = now_epoch() - (session_hours * 3600.0);
    let entries = read_raw(&path, cutoff);
    maybe_prune(&path, entries.len(), cutoff);
    entries
}

/// Compute self-corrections from ledger entries.
///
/// For each (file, check) pair, compares the count from the first
/// post-tool-use entry to the last. Positive delta = echoes resolved.
///
/// Returns `{check_name: total_corrections_across_all_files}`.
pub fn compute_self_corrections(entries: &[LedgerEntry]) -> HashMap<String, i32> {
    // Group post-tool-use entries by file, preserving timestamp order
    let mut by_file: HashMap<String, Vec<&LedgerEntry>> = HashMap::new();
    for entry in entries {
        if entry.mode != "post-tool-use" {
            continue;
        }
        if !entry.file.is_empty() {
            by_file.entry(entry.file.clone()).or_default().push(entry);
        }
    }

    let mut corrections: HashMap<String, i32> = HashMap::new();
    for file_entries in by_file.values() {
        if file_entries.len() < 2 {
            continue;
        }
        let mut ordered: Vec<&&LedgerEntry> = file_entries.iter().collect();
        ordered.sort_by(|a, b| a.ts.partial_cmp(&b.ts).unwrap_or(std::cmp::Ordering::Equal));

        let first_echoes = &ordered[0].echoes;
        let last_echoes = &ordered[ordered.len() - 1].echoes;

        for (check, &count) in first_echoes {
            let current = last_echoes.get(check).copied().unwrap_or(0);
            let delta = count as i32 - current as i32;
            if delta > 0 {
                *corrections.entry(check.clone()).or_insert(0) += delta;
            }
        }
    }

    corrections
}

/// Convert ledger entries to `serde_json::Value` for `format_session_stats`.
pub fn entries_to_json_values(entries: &[LedgerEntry]) -> Vec<serde_json::Value> {
    entries
        .iter()
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect()
}

// ---------------------------------------------------------------------------
// Internal I/O
// ---------------------------------------------------------------------------

fn read_raw(path: &Path, cutoff: f64) -> Vec<LedgerEntry> {
    if !path.is_file() {
        return Vec::new();
    }
    let contents = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<LedgerEntry>(line) {
            Ok(entry) if entry.ts >= cutoff => entries.push(entry),
            _ => continue, // Skip malformed or stale lines
        }
    }
    entries
}

/// Compact ledger when >50% stale AND file >50KB. Atomic via rename.
fn maybe_prune(path: &Path, active_count: usize, cutoff: f64) {
    let file_size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return,
    };

    if file_size < PRUNE_SIZE_THRESHOLD {
        return;
    }

    // Count total lines to compute stale ratio
    let contents = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let total = contents.lines().filter(|l| !l.trim().is_empty()).count();

    if total == 0 || (active_count as f64 / total as f64) >= 0.5 {
        return; // Not enough stale entries to justify rewrite
    }

    // Rewrite with only active entries
    let tmp_path = path.with_extension("jsonl.tmp");
    let active_lines: Vec<String> = contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            match serde_json::from_str::<LedgerEntry>(line) {
                Ok(entry) if entry.ts >= cutoff => Some(format!("{}\n", line)),
                _ => None,
            }
        })
        .collect();

    if let Ok(()) = std::fs::write(&tmp_path, active_lines.join("")) {
        if std::fs::rename(&tmp_path, path).is_err() {
            let _ = std::fs::remove_file(&tmp_path);
        }
    } else {
        let _ = std::fs::remove_file(&tmp_path);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("create temp dir")
    }

    #[test]
    fn test_append_and_read() {
        let dir = make_temp_dir();
        let cwd = dir.path().to_str().unwrap();

        let mut echoes = HashMap::new();
        echoes.insert("ruff".to_string(), 2_usize);
        echoes.insert("vulture".to_string(), 1_usize);

        append(
            cwd,
            &format!("{}/src/app.py", cwd),
            "post-tool-use",
            &echoes,
        );

        let entries = read_session(cwd, 1.0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "src/app.py");
        assert_eq!(entries[0].mode, "post-tool-use");
        assert_eq!(entries[0].echoes.get("ruff"), Some(&2));
        assert_eq!(entries[0].echoes.get("vulture"), Some(&1));
    }

    #[test]
    fn test_append_creates_gitignore() {
        let dir = make_temp_dir();
        let cwd = dir.path().to_str().unwrap();

        append(
            cwd,
            &format!("{}/foo.py", cwd),
            "post-tool-use",
            &HashMap::new(),
        );

        let gitignore = dir.path().join(LEDGER_DIR).join(".gitignore");
        assert!(gitignore.is_file());
        let contents = fs::read_to_string(&gitignore).unwrap();
        assert_eq!(contents, "*\n");
    }

    #[test]
    fn test_empty_echoes_recorded() {
        let dir = make_temp_dir();
        let cwd = dir.path().to_str().unwrap();

        append(
            cwd,
            &format!("{}/clean.py", cwd),
            "post-tool-use",
            &HashMap::new(),
        );

        let entries = read_session(cwd, 1.0);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].echoes.is_empty());
    }

    #[test]
    fn test_session_window_filtering() {
        let dir = make_temp_dir();
        let cwd = dir.path().to_str().unwrap();

        // Write an entry with a very old timestamp directly
        let old_entry = LedgerEntry {
            ts: 1000.0, // very old
            file: "old.py".to_string(),
            mode: "post-tool-use".to_string(),
            echoes: HashMap::new(),
        };
        let path = ledger_path(cwd);
        ensure_dir(cwd).unwrap();
        let line = serde_json::to_string(&old_entry).unwrap();
        fs::write(&path, format!("{}\n", line)).unwrap();

        // Append a fresh entry
        append(
            cwd,
            &format!("{}/new.py", cwd),
            "post-tool-use",
            &HashMap::new(),
        );

        let entries = read_session(cwd, 1.0);
        // Only the fresh entry should appear
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "new.py");
    }

    #[test]
    fn test_compute_self_corrections_basic() {
        let entries = vec![
            LedgerEntry {
                ts: 1000.0,
                file: "app.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: {
                    let mut m = HashMap::new();
                    m.insert("ruff".to_string(), 3);
                    m
                },
            },
            LedgerEntry {
                ts: 2000.0,
                file: "app.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: {
                    let mut m = HashMap::new();
                    m.insert("ruff".to_string(), 1);
                    m
                },
            },
        ];

        let corrections = compute_self_corrections(&entries);
        assert_eq!(corrections.get("ruff"), Some(&2)); // 3 - 1 = 2 resolved
    }

    #[test]
    fn test_compute_self_corrections_ignores_stop_entries() {
        let entries = vec![LedgerEntry {
            ts: 1000.0,
            file: "app.py".to_string(),
            mode: "stop".to_string(),
            echoes: {
                let mut m = HashMap::new();
                m.insert("ruff".to_string(), 5);
                m
            },
        }];

        let corrections = compute_self_corrections(&entries);
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_compute_self_corrections_needs_at_least_two_entries() {
        let entries = vec![LedgerEntry {
            ts: 1000.0,
            file: "app.py".to_string(),
            mode: "post-tool-use".to_string(),
            echoes: {
                let mut m = HashMap::new();
                m.insert("ruff".to_string(), 3);
                m
            },
        }];

        let corrections = compute_self_corrections(&entries);
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_compute_self_corrections_no_regression() {
        // If last has more echoes than first, delta is negative -- not reported
        let entries = vec![
            LedgerEntry {
                ts: 1000.0,
                file: "app.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: {
                    let mut m = HashMap::new();
                    m.insert("ruff".to_string(), 1);
                    m
                },
            },
            LedgerEntry {
                ts: 2000.0,
                file: "app.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: {
                    let mut m = HashMap::new();
                    m.insert("ruff".to_string(), 5);
                    m
                },
            },
        ];

        let corrections = compute_self_corrections(&entries);
        assert!(corrections.is_empty()); // negative delta not included
    }

    #[test]
    fn test_compute_self_corrections_multiple_files() {
        let entries = vec![
            // File A: ruff 3 -> 1 (delta +2)
            LedgerEntry {
                ts: 1000.0,
                file: "a.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: {
                    let mut m = HashMap::new();
                    m.insert("ruff".to_string(), 3);
                    m
                },
            },
            LedgerEntry {
                ts: 2000.0,
                file: "a.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: {
                    let mut m = HashMap::new();
                    m.insert("ruff".to_string(), 1);
                    m
                },
            },
            // File B: ruff 2 -> 0 (delta +2)
            LedgerEntry {
                ts: 1000.0,
                file: "b.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: {
                    let mut m = HashMap::new();
                    m.insert("ruff".to_string(), 2);
                    m
                },
            },
            LedgerEntry {
                ts: 2000.0,
                file: "b.py".to_string(),
                mode: "post-tool-use".to_string(),
                echoes: HashMap::new(),
            },
        ];

        let corrections = compute_self_corrections(&entries);
        assert_eq!(corrections.get("ruff"), Some(&4)); // 2 + 2 = 4 total
    }

    #[test]
    fn test_entries_to_json_values() {
        let entries = vec![LedgerEntry {
            ts: 1000.0,
            file: "app.py".to_string(),
            mode: "post-tool-use".to_string(),
            echoes: HashMap::new(),
        }];
        let values = entries_to_json_values(&entries);
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].get("file").unwrap().as_str(), Some("app.py"));
    }

    #[test]
    fn test_relative_path_conversion() {
        assert_eq!(
            relative_path("/home/user/proj/src/app.py", "/home/user/proj"),
            "src/app.py"
        );
        // Non-prefix case returns as-is
        assert_eq!(
            relative_path("/other/path.py", "/home/user/proj"),
            "/other/path.py"
        );
    }

    #[test]
    fn test_maybe_prune_small_file() {
        // Files under 50KB should never be pruned
        let dir = make_temp_dir();
        let cwd = dir.path().to_str().unwrap();

        append(
            cwd,
            &format!("{}/foo.py", cwd),
            "post-tool-use",
            &HashMap::new(),
        );

        let path = ledger_path(cwd);
        let size_before = fs::metadata(&path).unwrap().len();
        assert!(size_before < PRUNE_SIZE_THRESHOLD);

        // Prune should be a no-op
        maybe_prune(&path, 1, 0.0);
        let size_after = fs::metadata(&path).unwrap().len();
        assert_eq!(size_before, size_after);
    }
}
