//! Bash command guard -- blocks dangerous commands.
//!
//! Checks commands against hardcoded destructive patterns and user-configured
//! `blocked_commands` from `ecko.yaml`. Returns exit code 2 to block,
//! exit code 0 to allow.

use std::sync::LazyLock;

use regex::Regex;

use crate::config::{load_config, PatternRule};
use crate::debug;
use crate::echo::emit;

// ---------------------------------------------------------------------------
// Hardcoded dangerous patterns
// ---------------------------------------------------------------------------

/// Each entry is `(pattern, block_message)`. Patterns use regex syntax.
///
/// Design notes (from Python version):
/// - Avoid `$` anchors (bypassed by trailing args), use `(\s|$|;|&|\|)` terminators
/// - `--force` pattern uses command-wide `(?!.*--force-with-lease)` lookahead
/// - Full-path (`/bin/rm`), backslash-escaped (`\rm`), `command rm` variants covered
const HARDCODED_PATTERNS: &[(&str, &str)] = &[
    (
        r"git\b.*--no-verify",
        "git --no-verify bypasses safety hooks",
    ),
    (
        r"(?:/(?:usr/)?(?:s?bin)/|\\|command\s+)?rm\s+.*-[^\s]*r[^\s]*f.*\s+/(\s|$|;|&|\|)",
        "rm -rf / is catastrophically destructive",
    ),
    (
        r"(?:/(?:usr/)?(?:s?bin)/|\\|command\s+)?rm\s+.*-[^\s]*r[^\s]*f.*\s+~(/|\s|$|;|&|\|)",
        "rm -rf ~ deletes your home directory",
    ),
    // Note: Rust regex doesn't support lookaheads, so we handle
    // --force-with-lease exclusion in check_bash_command() logic.
    // Use `git\b.*\bsubcommand\b` to match regardless of intervening args like `-C /dir`.
    (
        r"git\b.*\bpush\b.*(\s--force(\s|$)|\s-f(\s|$))",
        "git push --force can overwrite remote history",
    ),
    (
        r"git\b.*\breset\s+--hard",
        "git reset --hard permanently discards commits",
    ),
    (
        r"git\b.*\bclean\s+.*-[^\s]*f",
        "git clean -f permanently deletes untracked files",
    ),
];

// ---------------------------------------------------------------------------
// Lazy-compiled hardcoded patterns
// ---------------------------------------------------------------------------

static COMPILED_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    HARDCODED_PATTERNS
        .iter()
        .filter_map(|&(pat, msg)| Regex::new(pat).ok().map(|re| (re, msg)))
        .collect()
});

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check a bash command against hardcoded + user-configured dangerous patterns.
///
/// Returns `Some(block_message)` if the command matches a blocked pattern,
/// `None` if the command is allowed.
pub fn check_bash_command(command: &str, user_patterns: &[PatternRule]) -> Option<String> {
    // Check hardcoded patterns first (pre-compiled via LazyLock).
    for (re, message) in &*COMPILED_PATTERNS {
        if re.is_match(command) {
            // Special case: allow --force-with-lease (safe alternative to --force)
            // but only if --force doesn't also appear as a standalone flag.
            if message.contains("--force") && command.contains("--force-with-lease") {
                let without_lease = command.replace("--force-with-lease", "");
                if !without_lease.contains("--force") {
                    continue;
                }
            }
            return Some(message.to_string());
        }
    }

    // Check user-configured blocked_commands.
    for rule in user_patterns {
        match Regex::new(&rule.pattern) {
            Ok(re) => {
                if re.is_match(command) {
                    let msg = if rule.message.is_empty() {
                        format!("blocked by pattern: {}", rule.pattern)
                    } else {
                        rule.message.clone()
                    };
                    return Some(msg);
                }
            }
            Err(e) => {
                debug::debug(&format!(
                    "guard: invalid user pattern '{}': {e}",
                    rule.pattern
                ));
            }
        }
    }

    None
}

/// Check a bash command and block if it matches dangerous patterns.
///
/// Loads config from `cwd` to pick up user-configured `blocked_commands`.
/// Returns exit code 2 (block) or 0 (allow).
pub fn run_pre_tool_use_bash(command: &str, cwd: &str) -> i32 {
    debug::debug(&format!("guard: checking command (len={})", command.len()));

    let config = load_config(cwd);

    match check_bash_command(command, &config.blocked_commands) {
        Some(message) => {
            emit(&format!("~~ ecko ~~ blocked: {message}"));
            2 // exit code 2 = block
        }
        None => 0, // allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper -- no user patterns.
    fn check(cmd: &str) -> Option<String> {
        check_bash_command(cmd, &[])
    }

    // -----------------------------------------------------------------------
    // git --no-verify
    // -----------------------------------------------------------------------

    #[test]
    fn test_blocks_git_no_verify() {
        assert!(check("git commit --no-verify -m 'yolo'").is_some());
        assert!(check("git push --no-verify").is_some());
    }

    #[test]
    fn test_allows_normal_git_commit() {
        assert!(check("git commit -m 'fix: typo'").is_none());
    }

    // -----------------------------------------------------------------------
    // rm -rf / and rm -rf ~
    // -----------------------------------------------------------------------

    #[test]
    fn test_blocks_rm_rf_root() {
        assert!(check("rm -rf /").is_some());
        assert!(check("rm -rf / --no-preserve-root").is_some());
        assert!(check("/bin/rm -rf /").is_some());
    }

    #[test]
    fn test_blocks_rm_rf_home() {
        assert!(check("rm -rf ~").is_some());
        assert!(check("rm -rf ~/").is_some());
    }

    #[test]
    fn test_allows_safe_rm() {
        assert!(check("rm -rf ./build").is_none());
        assert!(check("rm file.txt").is_none());
    }

    // -----------------------------------------------------------------------
    // git push --force
    // -----------------------------------------------------------------------

    #[test]
    fn test_blocks_git_push_force() {
        assert!(check("git push --force").is_some());
        assert!(check("git push -f").is_some());
        assert!(check("git push origin main --force").is_some());
    }

    #[test]
    fn test_allows_force_with_lease() {
        assert!(check("git push --force-with-lease").is_none());
    }

    // -----------------------------------------------------------------------
    // git reset --hard
    // -----------------------------------------------------------------------

    #[test]
    fn test_blocks_git_reset_hard() {
        assert!(check("git reset --hard").is_some());
        assert!(check("git reset --hard HEAD~1").is_some());
    }

    #[test]
    fn test_allows_soft_reset() {
        assert!(check("git reset --soft HEAD~1").is_none());
    }

    // -----------------------------------------------------------------------
    // git clean -f
    // -----------------------------------------------------------------------

    #[test]
    fn test_blocks_git_clean_f() {
        assert!(check("git clean -fd").is_some());
        assert!(check("git clean -f").is_some());
    }

    #[test]
    fn test_allows_git_clean_dry_run() {
        assert!(check("git clean -n").is_none());
    }

    // -----------------------------------------------------------------------
    // git -C prefix bypass
    // -----------------------------------------------------------------------

    #[test]
    fn test_blocks_git_c_push_force() {
        assert!(check("git -C /tmp push --force").is_some());
        assert!(check("git -C /tmp push -f").is_some());
    }

    #[test]
    fn test_blocks_git_c_reset_hard() {
        assert!(check("git -C /tmp reset --hard").is_some());
        assert!(check("git -C /tmp reset --hard HEAD~1").is_some());
    }

    #[test]
    fn test_blocks_git_c_clean_f() {
        assert!(check("git -C /tmp clean -fd").is_some());
        assert!(check("git -C /tmp clean -f").is_some());
    }

    #[test]
    fn test_allows_git_c_safe_commands() {
        assert!(check("git -C /tmp status").is_none());
        assert!(check("git -C /tmp push").is_none());
        assert!(check("git -C /tmp push --force-with-lease").is_none());
    }

    // -----------------------------------------------------------------------
    // User-configured patterns
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_blocked_commands() {
        let rules = vec![PatternRule {
            pattern: r"docker\s+system\s+prune".to_string(),
            message: "docker system prune removes all unused data".to_string(),
            glob: String::new(),
        }];

        assert!(check_bash_command("docker system prune -a", &rules).is_some());
        assert!(check_bash_command("docker ps", &rules).is_none());
    }

    #[test]
    fn test_user_pattern_default_message() {
        let rules = vec![PatternRule {
            pattern: r"dangerous_cmd".to_string(),
            message: String::new(), // no custom message
            glob: String::new(),
        }];

        let result = check_bash_command("run dangerous_cmd now", &rules);
        assert!(result.is_some());
        assert!(result.unwrap().contains("blocked by pattern"));
    }

    #[test]
    fn test_invalid_user_pattern_skipped() {
        let rules = vec![PatternRule {
            pattern: r"[invalid".to_string(), // broken regex
            message: String::new(),
            glob: String::new(),
        }];

        // Should not panic; invalid pattern is silently skipped.
        assert!(check_bash_command("anything", &rules).is_none());
    }
}
