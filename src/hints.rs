//! Check-specific hints -- session directives and contextual suggestions.
//!
//! Pure string logic. No I/O, no heavy imports.

use crate::lang;

// ---------------------------------------------------------------------------
// Session pattern directives (Phase 2)
// ---------------------------------------------------------------------------

/// Return a behavioral directive message for a check that has fired repeatedly.
///
/// Used by session pattern detection to tell the agent what to do differently.
pub fn pattern_directive(check: &str) -> &'static str {
    match check {
        "bare-except" => "use `except Exception as e:` in all new code",
        "unused-imports" => "only import modules you actually reference in the function body",
        "no-var" => "use `const` for values that don't change, `let` for reassignment",
        "placeholder-code" => "implement function bodies before moving to the next file",
        "singleton-comparison" => "use `is None` / `is not None` instead of `== None`",
        "mutable-default-args" => "use `None` as default, then assign inside the function body",
        "star-imports" => "import specific names instead of using `*`",
        "builtin-shadowing" => "avoid reusing Python builtin names as variable names",
        "debugger-statement" => "remove `debugger` statements before committing",
        "duplicate-keys" => "check for duplicate keys in objects and dicts before adding new ones",
        "unreachable-code" => "remove or move code after return/break/continue statements",
        "empty-error-check" => {
            "always handle errors explicitly, don't use empty `if err != nil {}`"
        }
        "todo-macro" => "resolve `todo!()` / `unimplemented!()` before committing",
        "unicode-artifacts" => {
            "use ASCII equivalents: `--` not em dash, straight quotes not smart quotes"
        }
        "fixed-wait" => "avoid `time.sleep()` with fixed values in tests -- use polling or events",
        "test-conditional" => {
            "avoid conditional logic in tests -- write separate test cases instead"
        }
        "empty-block-statements" => {
            "don't leave empty catch/if/else blocks -- add handling or a comment"
        }
        "useless-catch" => "don't catch an exception just to rethrow it unchanged",
        _ => "review this pattern before writing more code",
    }
}

// ---------------------------------------------------------------------------
// Contextual echo suggestions (Phase 3)
// ---------------------------------------------------------------------------

/// Return a file-type-aware suggestion for a specific echo.
///
/// Called by check implementations to populate the `suggestion` field.
/// Returns empty string when no contextual hint adds value.
pub fn contextual_suggestion(check: &str, file_path: &str, _message: &str) -> String {
    let is_test = lang::is_test_file(file_path);
    let is_init = file_path.ends_with("__init__.py");

    match check {
        "unused-imports" => {
            if is_init {
                "if re-exporting, add the name to `__all__`. Suppress: `# ecko:ignore[unused-imports]`"
                    .to_string()
            } else if is_test && file_path.ends_with(".py") {
                "if this is a pytest fixture, ensure the fixture name matches the import exactly"
                    .to_string()
            } else {
                "remove the unused import, or suppress: `# ecko:ignore[unused-imports]`".to_string()
            }
        }
        "bare-except" => {
            "catch specific exceptions: `except ValueError:` or at minimum `except Exception:`"
                .to_string()
        }
        "star-imports" => {
            "import specific names instead of using `from module import *`".to_string()
        }
        "singleton-comparison" => {
            "use `is None` / `is not None` instead of `== None` / `!= None`".to_string()
        }
        "mutable-default-args" => {
            "use `None` as default, assign inside the body: `if arg is None: arg = []`".to_string()
        }
        "builtin-shadowing" => {
            "rename the variable to avoid shadowing a Python builtin".to_string()
        }
        "placeholder-code" => {
            if is_test {
                String::new() // test stubs are normal
            } else {
                "implement the function body or mark as abstract".to_string()
            }
        }
        "unreachable-code" => "remove or move code after return/break/continue".to_string(),
        "duplicate-keys" => {
            "remove the duplicate key -- only the last value takes effect".to_string()
        }
        "test-conditional" => {
            "split into separate test cases instead of branching inside a test".to_string()
        }
        "fixed-wait" => {
            "use polling, events, or retry loops instead of `time.sleep()` with a fixed delay"
                .to_string()
        }
        "mock-spec-bypass" => {
            "add `spec=ClassName` or `autospec=True` to mock to catch typos in attribute access"
                .to_string()
        }
        "no-var" => "use `const` for values that don't change, `let` for reassignment".to_string(),
        "debugger-statement" => "remove `debugger` before committing".to_string(),
        "empty-block-statements" => {
            "add error handling or a comment explaining why the block is empty".to_string()
        }
        "useless-catch" => "remove the try/catch -- rethrowing unchanged adds no value".to_string(),
        "empty-error-check" => {
            "handle the error: log it, return it, or wrap it with context".to_string()
        }
        "todo-macro" => "implement the function or add a tracking issue".to_string(),
        "unicode-artifacts" => {
            "replace with ASCII: `--` for em dash, straight quotes for smart quotes".to_string()
        }
        "banned-patterns" => String::new(), // user-provided message is sufficient
        "import-layers" => String::new(),   // user-provided message is sufficient
        "obsolete-terms" => String::new(),  // user-provided message is sufficient
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_directive_known_checks() {
        assert!(pattern_directive("bare-except").contains("except Exception"));
        assert!(pattern_directive("unused-imports").contains("import"));
        assert!(pattern_directive("no-var").contains("const"));
    }

    #[test]
    fn test_pattern_directive_unknown_falls_back() {
        assert_eq!(
            pattern_directive("some-future-check"),
            "review this pattern before writing more code"
        );
    }

    #[test]
    fn test_contextual_suggestion_unused_imports_init() {
        let hint = contextual_suggestion("unused-imports", "/proj/__init__.py", "");
        assert!(hint.contains("re-export"));
        assert!(hint.contains("__all__"));
    }

    #[test]
    fn test_contextual_suggestion_unused_imports_test() {
        let hint = contextual_suggestion("unused-imports", "/proj/test_app.py", "");
        assert!(hint.contains("fixture"));
    }

    #[test]
    fn test_contextual_suggestion_unused_imports_regular() {
        let hint = contextual_suggestion("unused-imports", "/proj/app.py", "");
        assert!(hint.contains("remove"));
    }

    #[test]
    fn test_contextual_suggestion_placeholder_in_test() {
        let hint = contextual_suggestion("placeholder-code", "/proj/test_app.py", "");
        assert!(hint.is_empty(), "test stubs should not get a hint");
    }

    #[test]
    fn test_contextual_suggestion_placeholder_in_production() {
        let hint = contextual_suggestion("placeholder-code", "/proj/app.py", "");
        assert!(hint.contains("implement"));
    }

    #[test]
    fn test_contextual_suggestion_banned_patterns_empty() {
        let hint = contextual_suggestion("banned-patterns", "/proj/app.py", "");
        assert!(hint.is_empty(), "user-provided message is sufficient");
    }

    #[test]
    fn test_contextual_suggestion_unknown_check() {
        let hint = contextual_suggestion("some-future-check", "/proj/app.py", "");
        assert!(hint.is_empty());
    }
}
