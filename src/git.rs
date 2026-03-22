//! Git utilities -- modified file detection, path normalization.
//!
//! `get_modified_files()` uses git diff/log to find files touched in the
//! current session. `normalize_path()` resolves relative paths against cwd.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use crate::debug;

/// Compute a relative path from `cwd`, using forward slashes for portability.
///
/// If `file_path` is not under `cwd`, returns the original path with forward slashes.
pub fn relative_path(file_path: &str, cwd: &str) -> String {
    let fp = Path::new(file_path);
    let cw = Path::new(cwd);
    match fp.strip_prefix(cw) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => file_path.replace('\\', "/"),
    }
}

/// Normalize a file path to absolute, resolving relative paths against `cwd`.
pub fn normalize_path(path: &str, cwd: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        // Normalize (remove . / ..) but keep absolute
        match p.canonicalize() {
            Ok(c) => c.to_string_lossy().to_string(),
            Err(_) => p.to_string_lossy().to_string(),
        }
    } else {
        let joined = Path::new(cwd).join(path);
        match joined.canonicalize() {
            Ok(c) => c.to_string_lossy().to_string(),
            Err(_) => joined.to_string_lossy().to_string(),
        }
    }
}

/// Best-effort path normalization: canonicalize if the file exists, otherwise
/// use `Path::components()` to produce a cleaned path (resolves `.` and `..`).
///
/// Shared by external adapters that need to match tool-reported paths against
/// the modified file set.
pub fn canonicalize_or_normalize(path: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if let Ok(canonical) = std::fs::canonicalize(p) {
        canonical
    } else {
        let mut components = Vec::new();
        for comp in p.components() {
            match comp {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {}
                other => components.push(other),
            }
        }
        components.iter().collect()
    }
}

/// Get files modified in the current session via git.
///
/// Collects:
/// - Staged changes (`git diff --cached`)
/// - Unstaged changes (`git diff`)
/// - Untracked files (`git ls-files --others`)
/// - Recently committed files (`git log --since`)
///
/// Returns absolute paths, sorted for deterministic output.
pub fn get_modified_files(cwd: &str, session_hours: f64) -> Vec<String> {
    let mut files: BTreeSet<String> = BTreeSet::new();
    let since_minutes = (session_hours * 60.0) as u64;
    let since_arg = format!("--since={}m", since_minutes);

    // Helper: run a git command and collect non-empty output lines as absolute paths.
    let run_git = |args: &[&str], files: &mut BTreeSet<String>| {
        let result = Command::new("git").args(args).current_dir(cwd).output();
        if let Ok(output) = result {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        let abs = Path::new(cwd).join(line);
                        files.insert(abs.to_string_lossy().to_string());
                    }
                }
            }
        }
    };

    // Staged changes
    run_git(&["diff", "--name-only", "--cached"], &mut files);

    // Unstaged changes
    run_git(&["diff", "--name-only"], &mut files);

    // Untracked files
    run_git(&["ls-files", "--others", "--exclude-standard"], &mut files);

    // Recently committed files
    run_git(
        &[
            "log",
            &since_arg,
            "--diff-filter=ACMR",
            "--name-only",
            "--pretty=format:",
        ],
        &mut files,
    );

    debug::debug(&format!("git: found {} modified files", files.len()));

    // BTreeSet is already sorted
    files.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_relative() {
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let result = normalize_path("src/main.rs", &cwd);
        assert!(result.contains("src"));
        assert!(Path::new(&result).is_absolute());
    }

    #[test]
    fn test_normalize_path_absolute() {
        let result = normalize_path("/tmp/test.py", "/home/user");
        // Should remain absolute
        assert!(Path::new(&result).is_absolute());
    }

    #[test]
    fn test_get_modified_files_returns_sorted() {
        // In a git repo, this should at least not crash
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let files = get_modified_files(&cwd, 4.0);
        // Verify sorted
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
    }
}
