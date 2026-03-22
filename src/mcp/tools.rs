//! MCP tool implementations -- the actual logic behind each MCP tool.
//!
//! `check_workspace` delegates to `runner::run_stop_inner()` so hooks and MCP
//! tools share a single codepath (Deep Module pattern).

use crate::checks;
use crate::config;
use crate::echo;
use crate::lang;
use crate::runner;
use crate::suppress;

/// Run checks on a single file, return JSON result.
pub fn check_file(file_path: &str, cwd: &str) -> String {
    let cfg = config::load_config(cwd);
    let disabled = config::get_disabled_checks(&cfg);

    let source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => return format!("{{\"error\": \"cannot read file: {}\"}}", e),
    };

    let detected_lang = lang::detect_language(file_path);
    if detected_lang == lang::Lang::Unknown {
        return r#"{"echoes": [], "language": "unknown"}"#.to_string();
    }

    let mut echoes = checks::run_layer2_checks(file_path, detected_lang, &source, cwd, &cfg);
    echoes = suppress::filter_suppressed(echoes, file_path);
    echoes.retain(|e| !disabled.contains(&e.check));

    echo::format_file_echoes_json(file_path, &echoes, &[])
}

/// Run checks on all modified files in workspace, return JSON result.
///
/// Delegates to `runner::run_stop_inner()` -- same codepath as the CLI stop hook.
/// Gets exclusion filtering, ledger scoping, deduplication, dead code analysis,
/// external adapters, and echo caps for free.
///
/// Always returns JSON regardless of `output_format` config -- MCP consumers
/// are machine callers that need structured data.
pub fn check_workspace(cwd: &str) -> String {
    let result = runner::run_stop_inner(cwd, None);
    echo::format_stop_echoes_json(&result.all_echoes, result.elapsed, &[], &result.corrections)
}

/// Show ecko status -- config, available checks, language support.
pub fn status(cwd: &str) -> String {
    let cfg = config::load_config(cwd);
    let disabled = config::get_disabled_checks(&cfg);

    let languages = ["Python", "JavaScript", "TypeScript", "Go", "Rust"];
    let all_checks = [
        // Python
        "unused-imports",
        "singleton-comparison",
        "bare-except",
        "star-imports",
        "mutable-default-args",
        "builtin-shadowing",
        "placeholder-code",
        "unreachable-code",
        "duplicate-keys",
        "test-conditional",
        "fixed-wait",
        "mock-spec-bypass",
        // JS/TS
        "debugger-statement",
        "no-var",
        "empty-block-statements",
        "useless-catch",
        // Go
        "empty-error-check",
        // Rust
        "todo-macro",
        // Universal
        "trailing-whitespace",
        "unicode-artifacts",
        "banned-patterns",
        "import-layers",
        // Workspace
        "dead-code",
        "unused-exports",
    ];

    let mut lines = Vec::new();
    lines.push(format!(
        "ecko v{} -- deterministic code quality checks for AI agents",
        env!("CARGO_PKG_VERSION")
    ));
    lines.push(String::new());
    lines.push(format!("Languages: {}", languages.join(", ")));
    lines.push(format!(
        "Config: {}",
        if std::path::Path::new(cwd).join("ecko.yaml").exists() {
            "ecko.yaml loaded"
        } else {
            "defaults (no ecko.yaml)"
        }
    ));
    lines.push(format!("Output format: {}", cfg.output_format));
    lines.push(format!("Session hours: {}", cfg.session_hours));
    lines.push(format!("Fix suggestions: {}", cfg.fix_suggestions));
    lines.push(String::new());
    lines.push("Checks:".to_string());
    for check in &all_checks {
        let status = if disabled.contains(*check) {
            "disabled"
        } else {
            "enabled"
        };
        lines.push(format!("  {} [{}]", check, status));
    }

    if !cfg.custom_checks.is_empty() {
        lines.push(String::new());
        lines.push("Custom checks:".to_string());
        for cc in &cfg.custom_checks {
            lines.push(format!(
                "  {} [{}] ({})",
                cc.name,
                cc.severity,
                cc.languages.join(", ")
            ));
        }
    }

    lines.join("\n")
}

/// List applicable checks for a file without running them.
pub fn dry_run(file_path: &str, cwd: &str) -> String {
    let cfg = config::load_config(cwd);
    let disabled = config::get_disabled_checks(&cfg);
    let detected_lang = lang::detect_language(file_path);

    let checks = checks::list_applicable_checks(detected_lang);
    let mut lines = Vec::new();

    let lang_name = format!("{:?}", detected_lang);
    let rel = crate::git::relative_path(file_path, cwd);

    lines.push(format!("ecko dry-run: {} ({})", rel, lang_name));
    for check in &checks {
        let status = if disabled.contains(check) {
            "disabled"
        } else {
            "enabled"
        };
        lines.push(format!("  [{}] {}", status, check));
    }

    lines.join("\n")
}

/// Explain what a check does and why it matters.
pub fn explain(check_name: &str) -> String {
    let explanation = match check_name {
        "unused-imports" => "Detects import statements that bring in names never used in the code. Unused imports clutter the namespace, slow down startup, and can cause confusion about dependencies.",
        "bare-except" => "Flags `except:` clauses without a specific exception type. Bare excepts catch everything including KeyboardInterrupt and SystemExit, hiding real bugs. Use `except Exception:` instead.",
        "star-imports" => "Flags `from X import *` which pollutes the namespace with unknown names, makes it impossible to tell where names come from, and can cause shadowing bugs.",
        "singleton-comparison" => "Flags `== None`, `== True`, `== False` comparisons. Use `is None`, `is True`, `is False` instead -- singletons should be compared by identity, not equality.",
        "mutable-default-args" => "Flags function parameters with mutable default values like `def f(x=[])`. Mutable defaults are shared across calls, leading to subtle bugs. Use `None` and create the mutable in the function body.",
        "builtin-shadowing" => "Flags variables/parameters that shadow Python builtins like `type`, `id`, `list`. Shadowing builtins can cause confusing errors when the builtin is needed later.",
        "placeholder-code" => "Flags functions whose body is just `pass`, `...`, `raise NotImplementedError`, `todo!()`, or `unimplemented!()`. These are placeholders that should be implemented before merging.",
        "unreachable-code" => "Flags statements after `return`, `raise`, `break`, `continue`, or `panic!()`. Unreachable code is dead weight that confuses readers.",
        "duplicate-keys" => "Flags dict/object literals with repeated keys. The later value silently overwrites the earlier one -- almost always a bug.",
        "test-conditional" => "Flags `if` statements in test functions that aren't guard clauses. Conditional logic in tests makes it unclear what's actually being tested and can hide failures.",
        "fixed-wait" => "Flags `time.sleep()` and `asyncio.sleep()` in tests. Fixed waits make tests slow and flaky. Use polling, retries, or event-based synchronization instead.",
        "mock-spec-bypass" => "Flags attribute assignments on spec'd mocks that bypass the spec's type safety. If you need attributes not on the spec, the mock's spec is wrong.",
        "debugger-statement" => "Flags `debugger;` statements left in JavaScript/TypeScript code. Debugger statements pause execution and should never be committed.",
        "no-var" => "Flags `var` declarations in JavaScript. `var` has function scope and hoisting, which causes bugs. Use `const` or `let` instead.",
        "empty-block-statements" => "Flags empty catch/block bodies. Empty blocks silently swallow errors or do nothing, which is usually a bug.",
        "useless-catch" => "Flags `catch(e) { throw e }` which catches an error only to immediately re-throw it. The try-catch is redundant.",
        "empty-error-check" => "Flags `if err != nil {}` in Go with an empty body. The error is detected but not handled -- either handle it or explicitly ignore it with `_ = err`.",
        "todo-macro" => "Flags `todo!()` and `unimplemented!()` macros in Rust. These panic at runtime and should be implemented before merging.",
        "trailing-whitespace" => "Flags lines ending with spaces or tabs. Trailing whitespace clutters diffs and can cause issues in whitespace-sensitive contexts.",
        "unicode-artifacts" => "Flags Unicode characters like smart quotes, em dashes, and zero-width spaces in code. These are usually copy-paste artifacts from documentation or chat that break compilation or cause subtle bugs.",
        "dead-code" => "Detects functions, classes, and variables that are defined but never referenced anywhere in the project. Dead code is maintenance burden and confusion.",
        "unused-exports" => "Detects exported symbols in JS/TS modules that are never imported by any other file. Unused exports bloat the API surface and confuse consumers.",
        "banned-patterns" => "Flags code matching user-configured regex patterns in ecko.yaml. Use this to enforce project-specific rules like banning deprecated APIs or enforcing naming conventions.",
        "import-layers" => "Enforces import boundaries between layers of your codebase. Configure rules in ecko.yaml to prevent e.g. route handlers from importing database internals directly.",
        _ => return format!("Unknown check '{}'. Use ecko_status to see all available checks.", check_name),
    };
    format!("{}: {}", check_name, explanation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(content: &str, extension: &str) -> (tempfile::NamedTempFile, String) {
        let mut f = tempfile::Builder::new()
            .suffix(extension)
            .tempfile()
            .unwrap();
        f.write_all(content.as_bytes()).unwrap();
        let path = f.path().to_string_lossy().to_string();
        (f, path)
    }

    #[test]
    fn test_check_file_with_issues() {
        let (_f, path) = write_temp_file("try:\n    x = 1\nexcept:\n    pass\n", ".py");
        let result = check_file(&path, _f.path().parent().unwrap().to_str().unwrap());
        // Should contain JSON with echoes
        assert!(result.contains("echoes"));
    }

    #[test]
    fn test_check_file_clean() {
        let (_f, path) = write_temp_file("x = 1\n", ".py");
        let result = check_file(&path, _f.path().parent().unwrap().to_str().unwrap());
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let echoes = v.get("echoes").and_then(|e| e.as_array()).unwrap();
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_status_contains_checks() {
        let result = status("/tmp");
        assert!(result.contains("bare-except"));
        assert!(result.contains("unused-imports"));
        assert!(result.contains("debugger-statement"));
    }

    #[test]
    fn test_explain_known_check() {
        let result = explain("bare-except");
        assert!(result.contains("bare-except"));
        assert!(result.contains("except"));
    }

    #[test]
    fn test_explain_unknown_check() {
        let result = explain("nonexistent-check");
        assert!(result.contains("Unknown check"));
    }

    #[test]
    fn test_dry_run_python() {
        let (_f, path) = write_temp_file("x = 1\n", ".py");
        let result = dry_run(&path, _f.path().parent().unwrap().to_str().unwrap());
        assert!(result.contains("Python"));
        assert!(result.contains("bare-except"));
    }
}
