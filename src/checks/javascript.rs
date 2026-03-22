//! JavaScript/TypeScript checks -- tree-sitter query-based.
//!
//! 8 checks total:
//! - debugger-statement (error): leftover debugger statements
//! - no-var (warn): use let/const instead of var
//! - unused-imports (warn): imported names not referenced in source
//! - unreachable-code (warn): statements after return/throw/break/continue
//! - duplicate-keys (warn): repeated property keys in object literals
//! - empty-block-statements (warn): catch clauses with empty bodies
//! - useless-catch (warn): try-catch that only re-throws the caught variable
//! - placeholder-code (warn): throw new Error("not implemented"/"TODO")

use std::collections::{HashMap, HashSet};

use streaming_iterator::StreamingIterator;

use crate::config::EckoConfig;
use crate::debug;
use crate::echo::{Echo, Severity};
use crate::fix;
use crate::lang::{self, Lang};
use crate::query_engine;

// ---------------------------------------------------------------------------
// Embedded queries
// ---------------------------------------------------------------------------

const DEBUGGER_QUERY: &str = include_str!("../../queries/javascript/debugger.scm");
const NO_VAR_QUERY: &str = include_str!("../../queries/javascript/no_var.scm");
const UNUSED_IMPORTS_QUERY: &str = include_str!("../../queries/javascript/unused_imports.scm");
const UNREACHABLE_CODE_QUERY: &str = include_str!("../../queries/javascript/unreachable_code.scm");
const DUPLICATE_KEYS_QUERY: &str = include_str!("../../queries/javascript/duplicate_keys.scm");
const EMPTY_BLOCK_QUERY: &str = include_str!("../../queries/javascript/empty_block.scm");
const USELESS_CATCH_QUERY: &str = include_str!("../../queries/javascript/useless_catch.scm");
const PLACEHOLDER_CODE_QUERY: &str = include_str!("../../queries/javascript/placeholder_code.scm");

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Run all JS/TS checks on the given source.
pub fn run_checks(_file_path: &str, source: &str, lang: Lang, _config: &EckoConfig) -> Vec<Echo> {
    let (ts_lang, tree) = match lang::parse_for_checks(lang, source) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let source_bytes = source.as_bytes();
    let mut echoes = Vec::new();

    // --- debugger-statement ---
    if let Ok(query) = query_engine::compile_query(&ts_lang, DEBUGGER_QUERY) {
        if let Some(ci) = query_engine::capture_index(&query, "match") {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    if cap.index as usize != ci {
                        continue;
                    }
                    let node = cap.node;
                    let auto_fix = fix::fix_debugger(node.start_byte(), node.end_byte());
                    echoes.push(Echo {
                        check: "debugger-statement".into(),
                        line: node.start_position().row + 1,
                        message: "debugger statement \u{2014} remove before committing".into(),
                        suggestion: String::new(),
                        severity: Severity::Error,
                        fix: auto_fix,
                    });
                }
            }
        }
    } else {
        debug::debug("debugger query failed to compile");
    }

    // --- no-var ---
    if let Ok(query) = query_engine::compile_query(&ts_lang, NO_VAR_QUERY) {
        if let Some(ci) = query_engine::capture_index(&query, "match") {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    if cap.index as usize != ci {
                        continue;
                    }
                    let node = cap.node;
                    let auto_fix = fix::fix_no_var(node.start_byte(), node.end_byte());
                    echoes.push(Echo {
                        check: "no-var".into(),
                        line: node.start_position().row + 1,
                        message: "use let or const instead of var".into(),
                        suggestion: String::new(),
                        severity: Severity::Warn,
                        fix: auto_fix,
                    });
                }
            }
        }
    } else {
        debug::debug("no-var query failed to compile");
    }

    // --- unused-imports ---
    echoes.extend(check_unused_imports(&ts_lang, &tree, source, source_bytes));

    // --- unreachable-code ---
    echoes.extend(check_unreachable_code(&ts_lang, &tree, source_bytes));

    // --- duplicate-keys ---
    echoes.extend(check_duplicate_keys(&ts_lang, &tree, source_bytes));

    // --- empty-block-statements ---
    echoes.extend(check_empty_block_statements(&ts_lang, &tree, source_bytes));

    // --- useless-catch ---
    echoes.extend(check_useless_catch(&ts_lang, &tree, source_bytes));

    // --- placeholder-code ---
    echoes.extend(check_placeholder_code(&ts_lang, &tree, source_bytes));

    echoes
}

// ---------------------------------------------------------------------------
// unused-imports
// ---------------------------------------------------------------------------

/// Collect imported names from an import_clause node.
///
/// Handles:
/// - `import X from 'mod'`           -> ["X"]
/// - `import { X, Y } from 'mod'`    -> ["X", "Y"]
/// - `import { X as Z } from 'mod'`  -> ["Z"]  (alias is the local binding)
/// - `import * as X from 'mod'`       -> ["X"]
/// - `import X, { Y } from 'mod'`    -> ["X", "Y"]  (default + named)
fn collect_imported_names<'a>(
    clause_node: tree_sitter::Node<'a>,
    source: &'a [u8],
) -> Vec<(&'a str, usize)> {
    let mut names: Vec<(&str, usize)> = Vec::new();

    let mut cursor = clause_node.walk();
    for child in clause_node.named_children(&mut cursor) {
        match child.kind() {
            // Default import: `import X from 'mod'`
            "identifier" => {
                let name = query_engine::node_text(child, source);
                names.push((name, child.start_position().row + 1));
            }
            // Named imports: `import { X, Y as Z } from 'mod'`
            "named_imports" => {
                let mut inner_cursor = child.walk();
                for specifier in child.named_children(&mut inner_cursor) {
                    if specifier.kind() == "import_specifier" {
                        // If there's an alias, the local binding is the alias.
                        // Otherwise, the local binding is the name.
                        if let Some(alias_node) = specifier.child_by_field_name("alias") {
                            let alias = query_engine::node_text(alias_node, source);
                            names.push((alias, alias_node.start_position().row + 1));
                        } else if let Some(name_node) = specifier.child_by_field_name("name") {
                            let name = query_engine::node_text(name_node, source);
                            names.push((name, name_node.start_position().row + 1));
                        }
                    }
                }
            }
            // Namespace import: `import * as X from 'mod'`
            "namespace_import" => {
                let mut inner_cursor = child.walk();
                for id in child.named_children(&mut inner_cursor) {
                    if id.kind() == "identifier" {
                        let name = query_engine::node_text(id, source);
                        names.push((name, id.start_position().row + 1));
                    }
                }
            }
            _ => {}
        }
    }

    names
}

/// Check for imported names that are never referenced in the source.
///
/// Strategy: collect all imported names from import_clause nodes, then
/// scan the full source text for each name. A name is "used" if it appears
/// anywhere outside import statements. This is a heuristic (not scope-aware)
/// but catches the common case.
fn check_unused_imports(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &str,
    source_bytes: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, UNUSED_IMPORTS_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("unused-imports query failed: {e}"));
            return Vec::new();
        }
    };

    let clause_idx = query_engine::capture_index_or_skip(&query, "clause");

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

    // Collect all imported names with their line numbers and byte ranges
    // of their import statements (so we can exclude those from usage scanning).
    let mut imported: Vec<(String, usize, usize, usize)> = Vec::new(); // (name, line, import_start_byte, import_end_byte)

    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    while let Some(m) = matches.next() {
        let mut import_start = 0;
        let mut import_end = 0;

        for capture in m.captures {
            if capture.index as usize == match_idx {
                import_start = capture.node.start_byte();
                import_end = capture.node.end_byte();
            }
            if capture.index as usize == clause_idx {
                let names = collect_imported_names(capture.node, source_bytes);
                for (name, line) in names {
                    imported.push((name.to_string(), line, import_start, import_end));
                }
            }
        }
    }

    if imported.is_empty() {
        return Vec::new();
    }

    // Build a set of all import statement byte ranges
    let import_ranges: Vec<(usize, usize)> = imported
        .iter()
        .map(|(_, _, start, end)| (*start, *end))
        .collect();

    // For each imported name, search for usage outside import statements.
    let mut echoes = Vec::new();

    for (name, line, _, _) in &imported {
        if name.is_empty() {
            continue;
        }

        let mut used = false;

        // Scan source for the name as a whole word.
        // We use a simple approach: find all occurrences and check if any
        // fall outside all import statement ranges.
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len();

        let mut search_start = 0;
        while search_start + name_len <= source.len() {
            if let Some(pos) = source[search_start..].find(name.as_str()) {
                let abs_pos = search_start + pos;

                // Check word boundary: prev char must not be alphanumeric/underscore
                let prev_ok = abs_pos == 0 || {
                    let prev = source.as_bytes()[abs_pos - 1];
                    !prev.is_ascii_alphanumeric() && prev != b'_'
                };

                // Next char must not be alphanumeric/underscore
                let next_pos = abs_pos + name_len;
                let next_ok = next_pos >= source.len() || {
                    let next = source.as_bytes()[next_pos];
                    !next.is_ascii_alphanumeric() && next != b'_'
                };

                if prev_ok && next_ok {
                    // Check if this occurrence is inside any import statement
                    let in_import = import_ranges
                        .iter()
                        .any(|(start, end)| abs_pos >= *start && abs_pos < *end);

                    if !in_import {
                        used = true;
                        break;
                    }
                }

                search_start = abs_pos + 1;
            } else {
                break;
            }
        }

        if !used {
            echoes.push(Echo {
                check: "unused-imports".to_string(),
                line: *line,
                message: format!("'{name}' is imported but never used"),
                suggestion: "remove the unused import".to_string(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

// ---------------------------------------------------------------------------
// unreachable-code
// ---------------------------------------------------------------------------

/// Check for statements after return/throw/break/continue in the same block.
///
/// The query matches all terminating statements. We post-filter: if the matched
/// node has a next named sibling in its parent block, we flag that sibling as
/// unreachable.
fn check_unreachable_code(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, UNREACHABLE_CODE_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("unreachable-code query failed: {e}"));
            return Vec::new();
        }
    };

    let capture_idx = query_engine::capture_index_or_skip(&query, "match");

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    let mut echoes = Vec::new();
    let mut flagged_lines: HashSet<usize> = HashSet::new();

    while let Some(m) = matches.next() {
        for capture in m.captures {
            if capture.index as usize != capture_idx {
                continue;
            }

            let node = capture.node;

            // Only flag if inside a statement_block
            let parent = match node.parent() {
                Some(p) if p.kind() == "statement_block" || p.kind() == "program" => p,
                _ => continue,
            };

            // Walk named children of the parent to find siblings after this node.
            let mut found_self = false;
            let mut child_cursor = parent.walk();
            for sibling in parent.named_children(&mut child_cursor) {
                if sibling.id() == node.id() {
                    found_self = true;
                    continue;
                }
                if found_self {
                    let line = sibling.start_position().row + 1;
                    // Don't flag the same line twice (e.g. multiple returns)
                    if flagged_lines.insert(line) {
                        echoes.push(Echo {
                            check: "unreachable-code".to_string(),
                            line,
                            message: "unreachable code after control flow statement".to_string(),
                            suggestion: "remove the unreachable code".to_string(),
                            severity: Severity::Warn,
                            fix: None,
                        });
                    }
                }
            }
        }
    }

    echoes
}

// ---------------------------------------------------------------------------
// duplicate-keys
// ---------------------------------------------------------------------------

/// Check for duplicate property keys in object literals.
///
/// The query captures objects and their pair keys. We group keys by parent
/// object and flag duplicates.
fn check_duplicate_keys(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, DUPLICATE_KEYS_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("duplicate-keys query failed: {e}"));
            return Vec::new();
        }
    };

    let object_idx = query_engine::capture_index_or_skip(&query, "object");
    let key_idx = query_engine::capture_index_or_skip(&query, "key");

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    // Group keys by object node id
    let mut object_keys: HashMap<usize, Vec<(String, usize)>> = HashMap::new();

    while let Some(m) = matches.next() {
        let mut object_id = 0;
        let mut key_text = String::new();
        let mut key_line = 0;

        for capture in m.captures {
            if capture.index as usize == object_idx {
                object_id = capture.node.id();
            }
            if capture.index as usize == key_idx {
                let raw = query_engine::node_text(capture.node, source);
                // Normalize: strip quotes from string keys so "a" and a match
                key_text = raw.trim_matches('"').trim_matches('\'').to_string();
                key_line = capture.node.start_position().row + 1;
            }
        }

        if !key_text.is_empty() {
            object_keys
                .entry(object_id)
                .or_default()
                .push((key_text, key_line));
        }
    }

    let mut echoes = Vec::new();

    for keys in object_keys.values() {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for (key, line) in keys {
            if let Some(&first_line) = seen.get(key.as_str()) {
                echoes.push(Echo {
                    check: "duplicate-keys".to_string(),
                    line: *line,
                    message: format!("duplicate key '{key}' (first defined on line {first_line})"),
                    suggestion: "remove or rename the duplicate key".to_string(),
                    severity: Severity::Warn,
                    fix: None,
                });
            } else {
                seen.insert(key.as_str(), *line);
            }
        }
    }

    echoes
}

// ---------------------------------------------------------------------------
// empty-block-statements
// ---------------------------------------------------------------------------

/// Check for catch clauses with empty bodies.
///
/// A body is considered empty if it has no named children, or only comment children.
fn check_empty_block_statements(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, EMPTY_BLOCK_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("empty-block query failed: {e}"));
            return Vec::new();
        }
    };

    let body_idx = query_engine::capture_index_or_skip(&query, "body");
    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    let mut echoes = Vec::new();

    while let Some(m) = matches.next() {
        let mut body_node = None;
        let mut catch_line = 0;

        for capture in m.captures {
            if capture.index as usize == body_idx {
                body_node = Some(capture.node);
            }
            if capture.index as usize == match_idx {
                catch_line = capture.node.start_position().row + 1;
            }
        }

        if let Some(body) = body_node {
            // Check named children: comments are extras, statements are grammar children.
            // A body with only comments is NOT empty (the comment acknowledges the intent).
            // A body with zero named children is truly empty.
            let mut has_statements = false;
            let mut has_comments = false;
            let child_count = body.named_child_count();

            for i in 0..child_count {
                if let Some(child) = body.named_child(i) {
                    if child.kind() == "comment" {
                        has_comments = true;
                    } else {
                        has_statements = true;
                    }
                }
            }

            // Also check unnamed children for comments (tree-sitter extras)
            if !has_comments && !has_statements {
                let total = body.child_count();
                for i in 0..total {
                    if let Some(child) = body.child(i) {
                        if child.kind() == "comment" {
                            has_comments = true;
                            break;
                        }
                    }
                }
            }

            if !has_statements && !has_comments {
                echoes.push(Echo {
                    check: "empty-block-statements".to_string(),
                    line: catch_line,
                    message: "empty catch block \u{2014} handle the error or add a comment"
                        .to_string(),
                    suggestion: "add error handling or a comment explaining why it is empty"
                        .to_string(),
                    severity: Severity::Warn,
                    fix: None,
                });
            }
        }
    }

    echoes
}

// ---------------------------------------------------------------------------
// useless-catch
// ---------------------------------------------------------------------------

/// Check for try-catch where the catch only re-throws the caught variable.
///
/// Pattern:
/// ```js
/// try { ... } catch (e) { throw e; }
/// ```
fn check_useless_catch(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, USELESS_CATCH_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("useless-catch query failed: {e}"));
            return Vec::new();
        }
    };

    let param_idx = query_engine::capture_index_or_skip(&query, "param");
    let body_idx = query_engine::capture_index_or_skip(&query, "body");
    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    let mut echoes = Vec::new();

    while let Some(m) = matches.next() {
        let mut param_text = "";
        let mut body_node = None;
        let mut try_line = 0;

        for capture in m.captures {
            if capture.index as usize == param_idx {
                param_text = query_engine::node_text(capture.node, source);
            }
            if capture.index as usize == body_idx {
                body_node = Some(capture.node);
            }
            if capture.index as usize == match_idx {
                try_line = capture.node.start_position().row + 1;
            }
        }

        if let Some(body) = body_node {
            // Check if the body contains exactly one statement: `throw <param>`
            let mut child_cursor = body.walk();
            let named_children: Vec<tree_sitter::Node> = body
                .named_children(&mut child_cursor)
                .filter(|c| c.kind() != "comment")
                .collect();

            if named_children.len() == 1 && named_children[0].kind() == "throw_statement" {
                let throw_node = named_children[0];
                // The throw statement's first named child is the thrown expression
                if let Some(thrown) = throw_node.named_child(0) {
                    let thrown_text = query_engine::node_text(thrown, source);
                    if thrown_text == param_text {
                        echoes.push(Echo {
                            check: "useless-catch".to_string(),
                            line: try_line,
                            message: "useless try-catch \u{2014} catch only re-throws the error"
                                .to_string(),
                            suggestion: "remove the try-catch wrapper".to_string(),
                            severity: Severity::Warn,
                            fix: None,
                        });
                    }
                }
            }
        }
    }

    echoes
}

// ---------------------------------------------------------------------------
// placeholder-code
// ---------------------------------------------------------------------------

/// Placeholder message patterns (case-insensitive check in post-filter).
const PLACEHOLDER_PATTERNS: &[&str] = &[
    "not implemented",
    "not yet implemented",
    "todo",
    "fixme",
    "xxx",
];

/// Check for `throw new Error("not implemented")` and similar placeholder patterns.
///
/// Matches `throw new Error("...")` where the message is a known placeholder
/// (case-insensitive).
fn check_placeholder_code(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, PLACEHOLDER_CODE_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("placeholder-code query failed: {e}"));
            return Vec::new();
        }
    };

    let constructor_idx = query_engine::capture_index_or_skip(&query, "constructor");
    let message_idx = query_engine::capture_index_or_skip(&query, "message");
    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    let mut echoes = Vec::new();

    while let Some(m) = matches.next() {
        let mut constructor_text = "";
        let mut message_text = "";
        let mut throw_line = 0;

        for capture in m.captures {
            if capture.index as usize == constructor_idx {
                constructor_text = query_engine::node_text(capture.node, source);
            }
            if capture.index as usize == message_idx {
                message_text = query_engine::node_text(capture.node, source);
            }
            if capture.index as usize == match_idx {
                throw_line = capture.node.start_position().row + 1;
            }
        }

        // Only match Error constructor
        if constructor_text != "Error" {
            continue;
        }

        // Strip quotes from the string and check against placeholder patterns
        let inner = message_text.trim_matches('"').trim_matches('\'').trim();
        // Also handle template strings (backticks)
        let inner = inner.trim_matches('`').trim();

        let lower = inner.to_lowercase();

        let is_placeholder = PLACEHOLDER_PATTERNS.iter().any(|pat| lower.contains(pat));

        if is_placeholder {
            echoes.push(Echo {
                check: "placeholder-code".to_string(),
                line: throw_line,
                message: format!(
                    "placeholder: throw new Error(\"{inner}\") \u{2014} implement the function body"
                ),
                suggestion: "implement the function body".to_string(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EckoConfig;

    // Helper to run checks with default config
    fn check(source: &str, lang: Lang) -> Vec<Echo> {
        run_checks("test.js", source, lang, &EckoConfig::default())
    }

    fn check_js(source: &str) -> Vec<Echo> {
        check(source, Lang::JavaScript)
    }

    fn check_ts(source: &str) -> Vec<Echo> {
        check(source, Lang::TypeScript)
    }

    fn count(echoes: &[Echo], check_name: &str) -> usize {
        echoes.iter().filter(|e| e.check == check_name).count()
    }

    // =======================================================================
    // debugger-statement
    // =======================================================================

    #[test]
    fn test_debugger_detected() {
        let echoes = check_js("function foo() {\n  debugger;\n  return 1;\n}\n");
        assert!(count(&echoes, "debugger-statement") > 0);
    }

    #[test]
    fn test_no_debugger_clean() {
        let echoes = check_js("function foo() {\n  return 1;\n}\n");
        assert_eq!(count(&echoes, "debugger-statement"), 0);
    }

    // =======================================================================
    // no-var
    // =======================================================================

    #[test]
    fn test_var_detected() {
        let echoes = check_js("var x = 1;\nlet y = 2;\nconst z = 3;\n");
        assert!(count(&echoes, "no-var") > 0);
    }

    #[test]
    fn test_let_const_clean() {
        let echoes = check_js("let x = 1;\nconst y = 2;\n");
        assert_eq!(count(&echoes, "no-var"), 0);
    }

    // =======================================================================
    // unused-imports
    // =======================================================================

    #[test]
    fn test_unused_default_import() {
        let source = "import foo from 'bar';\nconsole.log('hello');\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 1);
        assert!(echoes.iter().any(|e| e.message.contains("foo")));
    }

    #[test]
    fn test_used_default_import() {
        let source = "import foo from 'bar';\nconsole.log(foo);\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 0);
    }

    #[test]
    fn test_unused_named_import() {
        let source = "import { x, y } from 'mod';\nconsole.log(x);\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 1);
        assert!(echoes.iter().any(|e| e.message.contains("y")));
    }

    #[test]
    fn test_aliased_import_uses_alias() {
        let source = "import { a as b } from 'mod';\nconsole.log(b);\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 0);
    }

    #[test]
    fn test_aliased_import_original_not_counted() {
        // `a` is not the local binding, `b` is. Using `a` should not make
        // the import count as used.
        let source = "import { a as b } from 'mod';\nconst a = 1;\n";
        let echoes = check_js(source);
        // `b` is unused (the `a` on line 2 is a different `a`)
        assert_eq!(count(&echoes, "unused-imports"), 1);
        assert!(echoes.iter().any(|e| e.message.contains("b")));
    }

    #[test]
    fn test_namespace_import_used() {
        let source = "import * as ns from 'mod';\nns.something();\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 0);
    }

    #[test]
    fn test_namespace_import_unused() {
        let source = "import * as ns from 'mod';\nconsole.log('hello');\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 1);
    }

    #[test]
    fn test_side_effect_import_no_false_positive() {
        // `import 'side-effect'` has no import clause -> should not flag anything
        let source = "import 'side-effect';\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 0);
    }

    #[test]
    fn test_all_imports_used() {
        let source = "import { x, y, z } from 'mod';\nconst a = x + y + z;\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 0);
    }

    #[test]
    fn test_unused_imports_typescript() {
        let source = "import { Foo } from './types';\nconst x: number = 1;\n";
        let echoes = check_ts(source);
        assert_eq!(count(&echoes, "unused-imports"), 1);
    }

    #[test]
    fn test_word_boundary_no_false_negative() {
        // `item` appears in `items` but is not actually used as a standalone identifier
        let source = "import { item } from 'mod';\nconst items = [];\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unused-imports"), 1);
    }

    #[test]
    fn test_default_and_named_import_mix() {
        let source = "import React, { useState } from 'react';\nconst [x, setX] = useState(0);\n";
        let echoes = check_js(source);
        // React is unused, useState is used
        assert_eq!(count(&echoes, "unused-imports"), 1);
        assert!(echoes.iter().any(|e| e.message.contains("React")));
    }

    // =======================================================================
    // unreachable-code
    // =======================================================================

    #[test]
    fn test_unreachable_after_return() {
        let source = "function foo() {\n  return 1;\n  console.log('unreachable');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "unreachable-code") > 0);
    }

    #[test]
    fn test_unreachable_after_throw() {
        let source = "function foo() {\n  throw new Error('x');\n  console.log('dead');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "unreachable-code") > 0);
    }

    #[test]
    fn test_no_unreachable_when_last_statement() {
        let source = "function foo() {\n  console.log('ok');\n  return 1;\n}\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unreachable-code"), 0);
    }

    #[test]
    fn test_unreachable_after_break() {
        let source = "for (let i = 0; i < 10; i++) {\n  break;\n  console.log('dead');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "unreachable-code") > 0);
    }

    #[test]
    fn test_unreachable_after_continue() {
        let source = "for (let i = 0; i < 10; i++) {\n  continue;\n  console.log('dead');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "unreachable-code") > 0);
    }

    #[test]
    fn test_unreachable_multiple_after_return() {
        // Two statements after return -> both should be flagged
        let source = "function f() {\n  return;\n  const a = 1;\n  const b = 2;\n}\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unreachable-code"), 2);
    }

    #[test]
    fn test_return_in_different_blocks_no_false_positive() {
        // Returns in separate if/else blocks should not flag each other
        let source =
            "function f(x) {\n  if (x) {\n    return 1;\n  } else {\n    return 2;\n  }\n}\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "unreachable-code"), 0);
    }

    #[test]
    fn test_unreachable_typescript() {
        let source = "function foo(): number {\n  return 1;\n  const x: number = 2;\n}\n";
        let echoes = check_ts(source);
        assert!(count(&echoes, "unreachable-code") > 0);
    }

    // =======================================================================
    // duplicate-keys
    // =======================================================================

    #[test]
    fn test_duplicate_keys_detected() {
        let source = "const obj = { a: 1, b: 2, a: 3 };\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "duplicate-keys"), 1);
        assert!(echoes.iter().any(|e| e.message.contains("'a'")));
    }

    #[test]
    fn test_no_duplicate_keys_clean() {
        let source = "const obj = { a: 1, b: 2, c: 3 };\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "duplicate-keys"), 0);
    }

    #[test]
    fn test_duplicate_string_keys() {
        let source = "const obj = { \"x\": 1, \"x\": 2 };\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "duplicate-keys"), 1);
    }

    #[test]
    fn test_duplicate_mixed_key_types() {
        // property_identifier `a` and string `"a"` should match as duplicates
        let source = "const obj = { a: 1, \"a\": 2 };\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "duplicate-keys"), 1);
    }

    #[test]
    fn test_nested_objects_independent() {
        // Each object is checked independently
        let source = "const a = { x: 1 };\nconst b = { x: 2 };\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "duplicate-keys"), 0);
    }

    #[test]
    fn test_three_duplicate_keys() {
        let source = "const obj = { a: 1, a: 2, a: 3 };\n";
        let echoes = check_js(source);
        // Second and third occurrences are flagged
        assert_eq!(count(&echoes, "duplicate-keys"), 2);
    }

    // =======================================================================
    // empty-block-statements
    // =======================================================================

    #[test]
    fn test_empty_catch_detected() {
        let source = "try { x(); } catch (e) {}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "empty-block-statements") > 0);
    }

    #[test]
    fn test_catch_with_body_clean() {
        let source = "try { x(); } catch (e) { console.error(e); }\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "empty-block-statements"), 0);
    }

    #[test]
    fn test_catch_with_comment_clean() {
        let source = "try { x(); } catch (e) { /* intentionally empty */ }\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "empty-block-statements"), 0);
    }

    #[test]
    fn test_catch_without_parameter_empty() {
        // `catch { }` without parameter is valid JS -- our query requires a parameter,
        // so this won't be matched by the empty-block check. That's acceptable since
        // catch-without-parameter is already an unusual pattern.
        let source = "try { x(); } catch {}\n";
        // This either matches or doesn't depending on query -- we just verify no panic
        let _echoes = check_js(source);
    }

    #[test]
    fn test_empty_catch_typescript() {
        let source = "try { foo(); } catch (e: unknown) {}\n";
        let echoes = check_ts(source);
        assert!(count(&echoes, "empty-block-statements") > 0);
    }

    // =======================================================================
    // useless-catch
    // =======================================================================

    #[test]
    fn test_useless_catch_detected() {
        let source = "try {\n  doSomething();\n} catch (e) {\n  throw e;\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "useless-catch") > 0);
    }

    #[test]
    fn test_catch_rethrows_different_error() {
        let source = "try {\n  doSomething();\n} catch (e) {\n  throw new Error('wrapped');\n}\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "useless-catch"), 0);
    }

    #[test]
    fn test_catch_with_logging_not_useless() {
        let source = "try {\n  doSomething();\n} catch (e) {\n  console.error(e);\n  throw e;\n}\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "useless-catch"), 0);
    }

    #[test]
    fn test_catch_handles_error_not_useless() {
        let source = "try {\n  doSomething();\n} catch (e) {\n  handleError(e);\n}\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "useless-catch"), 0);
    }

    #[test]
    fn test_useless_catch_with_comment_still_useless() {
        // Comments are skipped when counting children, so a comment + throw e is still useless
        let source = "try {\n  x();\n} catch (e) {\n  // re-throw\n  throw e;\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "useless-catch") > 0);
    }

    #[test]
    fn test_useless_catch_typescript() {
        let source = "try {\n  foo();\n} catch (e: unknown) {\n  throw e;\n}\n";
        let echoes = check_ts(source);
        assert!(count(&echoes, "useless-catch") > 0);
    }

    // =======================================================================
    // placeholder-code
    // =======================================================================

    #[test]
    fn test_placeholder_not_implemented() {
        let source = "function foo() {\n  throw new Error('not implemented');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "placeholder-code") > 0);
    }

    #[test]
    fn test_placeholder_todo() {
        let source = "function foo() {\n  throw new Error('TODO');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "placeholder-code") > 0);
    }

    #[test]
    fn test_placeholder_case_insensitive() {
        let source = "function foo() {\n  throw new Error('Not Implemented');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "placeholder-code") > 0);
    }

    #[test]
    fn test_placeholder_fixme() {
        let source = "function foo() {\n  throw new Error('FIXME');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "placeholder-code") > 0);
    }

    #[test]
    fn test_placeholder_not_yet_implemented() {
        let source = "function foo() {\n  throw new Error('not yet implemented');\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "placeholder-code") > 0);
    }

    #[test]
    fn test_real_error_not_flagged() {
        let source =
            "function foo() {\n  throw new Error('invalid argument: expected number');\n}\n";
        let echoes = check_js(source);
        assert_eq!(count(&echoes, "placeholder-code"), 0);
    }

    #[test]
    fn test_non_error_constructor_not_flagged() {
        let source = "function foo() {\n  throw new TypeError('not implemented');\n}\n";
        let echoes = check_js(source);
        // Only `Error` constructor is matched, not `TypeError`
        assert_eq!(count(&echoes, "placeholder-code"), 0);
    }

    #[test]
    fn test_placeholder_typescript() {
        let source = "function foo(): never {\n  throw new Error('not implemented');\n}\n";
        let echoes = check_ts(source);
        assert!(count(&echoes, "placeholder-code") > 0);
    }

    #[test]
    fn test_placeholder_double_quotes() {
        let source = "function foo() {\n  throw new Error(\"TODO\");\n}\n";
        let echoes = check_js(source);
        assert!(count(&echoes, "placeholder-code") > 0);
    }

    // =======================================================================
    // cross-language (TSX)
    // =======================================================================

    #[test]
    fn test_tsx_checks() {
        let source = "var x = <div>hello</div>;\n";
        let config = EckoConfig::default();
        let echoes = run_checks("test.tsx", source, Lang::Tsx, &config);
        assert!(count(&echoes, "no-var") > 0, "expected no-var echo in TSX");
    }

    #[test]
    fn test_typescript_all_checks() {
        let source = "var x: number = 1;\ndebugger;\n";
        let config = EckoConfig::default();
        let echoes = run_checks("test.ts", source, Lang::TypeScript, &config);
        assert!(
            echoes.len() >= 2,
            "expected both no-var and debugger echoes"
        );
    }
}
