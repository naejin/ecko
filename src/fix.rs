//! Auto-fix suggestion generation.
//!
//! Each function takes AST node info and returns an `Option<Fix>` with a
//! byte-range replacement. The caller constructs the Fix; these functions
//! just provide the replacement text.

use crate::echo::Fix;

// ---------------------------------------------------------------------------
// Python fixes
// ---------------------------------------------------------------------------

/// Replace `except:` with `except Exception:`.
///
/// Scans the node text for `except` followed by `:` (possibly with whitespace)
/// and inserts ` Exception` after `except`.
pub fn fix_bare_except(node_start: usize, node_end: usize, source: &str) -> Option<Fix> {
    let snippet = source.get(node_start..node_end)?;

    // Find "except" in the snippet
    let except_pos = snippet.find("except")?;
    let after_except = except_pos + "except".len();

    // Check what follows "except" -- should be whitespace/colon, not a letter
    let rest = &snippet[after_except..];
    let trimmed = rest.trim_start();
    if !trimmed.starts_with(':') {
        // Already has an exception type
        return None;
    }

    // Replace: insert " Exception" after "except"
    let abs_insert = node_start + after_except;
    Some(Fix {
        start_byte: abs_insert,
        end_byte: abs_insert,
        replacement: " Exception".to_string(),
    })
}

/// Replace singleton comparison operators:
/// - `== None` -> `is None`
/// - `!= None` -> `is not None`
/// - `== True` -> `is True`
/// - `== False` -> `is False`
/// - `!= True` -> `is not True`
/// - `!= False` -> `is not False`
pub fn fix_singleton_comparison(node_start: usize, node_end: usize, source: &str) -> Option<Fix> {
    let snippet = source.get(node_start..node_end)?;

    // Try each pattern
    let replacements: &[(&str, &str)] = &[
        ("== None", "is None"),
        ("!= None", "is not None"),
        ("== True", "is True"),
        ("== False", "is False"),
        ("!= True", "is not True"),
        ("!= False", "is not False"),
    ];

    for &(pattern, replacement) in replacements {
        if let Some(pos) = snippet.find(pattern) {
            return Some(Fix {
                start_byte: node_start + pos,
                end_byte: node_start + pos + pattern.len(),
                replacement: replacement.to_string(),
            });
        }
    }

    None
}

// ---------------------------------------------------------------------------
// JavaScript/TypeScript fixes
// ---------------------------------------------------------------------------

/// Replace `var` keyword with `const`.
pub fn fix_no_var(node_start: usize, _node_end: usize) -> Option<Fix> {
    // The node covers the entire var declaration; we only want to replace
    // the `var` keyword which is the first 3 bytes.
    Some(Fix {
        start_byte: node_start,
        end_byte: node_start + 3, // len("var") == 3
        replacement: "const".to_string(),
    })
}

/// Replace debugger statement with empty string (delete it).
pub fn fix_debugger(node_start: usize, node_end: usize) -> Option<Fix> {
    Some(Fix {
        start_byte: node_start,
        end_byte: node_end,
        replacement: String::new(),
    })
}

// ---------------------------------------------------------------------------
// Common fixes
// ---------------------------------------------------------------------------

/// Delete an entire import line (replace with empty string).
pub fn fix_unused_import_line(line_start: usize, line_end: usize) -> Option<Fix> {
    Some(Fix {
        start_byte: line_start,
        end_byte: line_end,
        replacement: String::new(),
    })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- fix_bare_except ---

    #[test]
    fn test_fix_bare_except_basic() {
        let source = "try:\n    pass\nexcept:\n    pass\n";
        // "except:" starts at byte 18, ends at byte 25
        let except_start = source.find("except:").unwrap();
        let except_end = except_start + "except:".len();
        let fix = fix_bare_except(except_start, except_end, source);
        assert!(fix.is_some());
        let fix = fix.unwrap();
        // Should insert " Exception" after "except"
        assert_eq!(fix.replacement, " Exception");
        // The insertion point is right after "except"
        assert_eq!(fix.start_byte, except_start + "except".len());
        assert_eq!(fix.end_byte, except_start + "except".len());
    }

    #[test]
    fn test_fix_bare_except_already_typed() {
        let source = "except ValueError:\n    pass\n";
        let fix = fix_bare_except(0, "except ValueError:".len(), source);
        // Should return None since it already has an exception type
        assert!(fix.is_none());
    }

    #[test]
    fn test_fix_bare_except_with_space() {
        let source = "except  :\n    pass\n";
        let fix = fix_bare_except(0, "except  :".len(), source);
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert_eq!(fix.replacement, " Exception");
    }

    // --- fix_singleton_comparison ---

    #[test]
    fn test_fix_eq_none() {
        let source = "x == None";
        let fix = fix_singleton_comparison(0, source.len(), source);
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert_eq!(fix.replacement, "is None");
        let result = format!(
            "{}{}{}",
            &source[..fix.start_byte],
            fix.replacement,
            &source[fix.end_byte..]
        );
        assert_eq!(result, "x is None");
    }

    #[test]
    fn test_fix_ne_none() {
        let source = "x != None";
        let fix = fix_singleton_comparison(0, source.len(), source);
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert_eq!(fix.replacement, "is not None");
    }

    #[test]
    fn test_fix_eq_true() {
        let source = "result == True";
        let fix = fix_singleton_comparison(0, source.len(), source);
        assert!(fix.is_some());
        assert_eq!(fix.unwrap().replacement, "is True");
    }

    #[test]
    fn test_fix_eq_false() {
        let source = "result == False";
        let fix = fix_singleton_comparison(0, source.len(), source);
        assert!(fix.is_some());
        assert_eq!(fix.unwrap().replacement, "is False");
    }

    #[test]
    fn test_fix_singleton_no_match() {
        let source = "x == 42";
        let fix = fix_singleton_comparison(0, source.len(), source);
        assert!(fix.is_none());
    }

    // --- fix_no_var ---

    #[test]
    fn test_fix_no_var() {
        let fix = fix_no_var(0, 15);
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert_eq!(fix.start_byte, 0);
        assert_eq!(fix.end_byte, 3);
        assert_eq!(fix.replacement, "const");
    }

    #[test]
    fn test_fix_no_var_apply() {
        let source = "var x = 1;";
        let fix = fix_no_var(0, source.len()).unwrap();
        let result = format!(
            "{}{}{}",
            &source[..fix.start_byte],
            fix.replacement,
            &source[fix.end_byte..]
        );
        assert_eq!(result, "const x = 1;");
    }

    // --- fix_debugger ---

    #[test]
    fn test_fix_debugger_basic() {
        let fix = fix_debugger(10, 18);
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert_eq!(fix.start_byte, 10);
        assert_eq!(fix.end_byte, 18);
        assert_eq!(fix.replacement, "");
    }

    #[test]
    fn test_fix_debugger_apply() {
        let source = "x = 1;\ndebugger;\ny = 2;\n";
        let start = source.find("debugger;").unwrap();
        let end = start + "debugger;".len();
        let fix = fix_debugger(start, end).unwrap();
        let result = format!(
            "{}{}{}",
            &source[..fix.start_byte],
            fix.replacement,
            &source[fix.end_byte..]
        );
        assert_eq!(result, "x = 1;\n\ny = 2;\n");
    }

    // --- fix_unused_import_line ---

    #[test]
    fn test_fix_unused_import_line() {
        let fix = fix_unused_import_line(0, 20);
        assert!(fix.is_some());
        let fix = fix.unwrap();
        assert_eq!(fix.start_byte, 0);
        assert_eq!(fix.end_byte, 20);
        assert_eq!(fix.replacement, "");
    }

    #[test]
    fn test_fix_unused_import_line_apply() {
        let source = "import os\nimport sys\nx = 1\n";
        // Delete "import os\n"
        let line_end = source.find('\n').unwrap() + 1;
        let fix = fix_unused_import_line(0, line_end).unwrap();
        let result = format!(
            "{}{}{}",
            &source[..fix.start_byte],
            fix.replacement,
            &source[fix.end_byte..]
        );
        assert_eq!(result, "import sys\nx = 1\n");
    }
}
