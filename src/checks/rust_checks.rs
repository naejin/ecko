//! Rust checks -- tree-sitter query-based.
//!
//! Checks: unused-imports, todo-macro, unreachable-code, placeholder-code.

use streaming_iterator::StreamingIterator;

use crate::config::EckoConfig;
use crate::debug;
use crate::echo::{Echo, Severity};
use crate::lang::{self, Lang};
use crate::query_engine;

const TODO_MACRO_QUERY: &str = include_str!("../../queries/rust/todo_macro.scm");
const UNUSED_IMPORTS_QUERY: &str = include_str!("../../queries/rust/unused_imports.scm");

/// Traits commonly imported for their methods but never referenced by name.
/// Tree-sitter can't resolve trait method dispatch, so these are always FPs.
const TRAIT_IMPORTS: &[&str] = &[
    // std traits used via method calls
    "Read",
    "Write",
    "Display",
    "Debug",
    "Iterator",
    "IntoIterator",
    "FromStr",
    "Into",
    "From",
    "TryFrom",
    "TryInto",
    "AsRef",
    "AsMut",
    "Deref",
    "DerefMut",
    "Clone",
    "Default",
    "Drop",
    "Hash",
    "Eq",
    "Ord",
    "PartialEq",
    "PartialOrd",
    "Send",
    "Sync",
    "Unpin",
    "Sized",
    // async traits
    "Future",
    "Stream",
    "Sink",
    // common external crate traits
    "StreamingIterator",
    "Serialize",
    "Deserialize",
];

pub fn run_checks(_file_path: &str, source: &str, _config: &EckoConfig) -> Vec<Echo> {
    let (ts_lang, tree) = match lang::parse_for_checks(Lang::Rust, source) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let source_bytes = source.as_bytes();
    let mut echoes = Vec::new();

    echoes.extend(check_unused_imports(&ts_lang, &tree, source, source_bytes));
    echoes.extend(check_todo_macro(&ts_lang, &tree, source_bytes));
    echoes.extend(check_unreachable_code(&tree));
    echoes.extend(check_placeholder(&tree, source_bytes));

    echoes
}

/// Rust unused imports: collect `use` declaration names, check usage in code.
fn check_unused_imports(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &str,
    source_bytes: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, UNUSED_IMPORTS_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("rust unused-imports query failed: {e}"));
            return Vec::new();
        }
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    // Collect imports: (imported_name, line, start_byte, end_byte)
    let mut imports: Vec<(String, usize, usize, usize)> = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if cap.index as usize == match_idx {
                let node = cap.node;
                let line = node.start_position().row + 1;
                let start = node.start_byte();
                let end = node.end_byte();

                // Extract the imported name from the use_declaration
                // `use std::io;` -> "io", `use std::fs;` -> "fs"
                // Navigate to the argument (scoped_identifier), get the last name
                if let Some(arg) = node.child_by_field_name("argument") {
                    let name = extract_use_name(arg, source_bytes);
                    if !name.is_empty() && !TRAIT_IMPORTS.contains(&name.as_str()) {
                        imports.push((name, line, start, end));
                    }
                }
            }
        }
    }

    let mut echoes = Vec::new();
    for (name, line, start, end) in &imports {
        // Check if the name appears anywhere in the file outside its own import statement.
        // We check both before and after the import to handle test module imports
        // that appear after the main code, and derive macros between imports.
        let before = &source[..*start];
        let after = if *end < source.len() {
            &source[*end..]
        } else {
            ""
        };
        let used = name_used_in(before, name) || name_used_in(after, name);
        if !used {
            echoes.push(Echo {
                check: "unused-imports".to_string(),
                line: *line,
                message: format!("unused import `{}`", name),
                suggestion: "Remove unused import.".to_string(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

/// Check if `name` appears as a used identifier in `text`.
///
/// Searches line-by-line, skipping `use` declaration lines (to avoid matching
/// the same name inside other import paths like `std::io` in `use std::io::Write`).
/// Uses word-boundary checks to avoid substring matches inside longer identifiers.
fn name_used_in(text: &str, name: &str) -> bool {
    for line in text.lines() {
        let trimmed = line.trim_start();
        // Skip use declarations -- these are imports, not usage.
        if trimmed.starts_with("use ") {
            continue;
        }
        if line_has_word(line, name) {
            return true;
        }
    }
    false
}

/// Check if `name` appears as a standalone word in `line`.
///
/// A "word" is bounded by non-alphanumeric, non-underscore characters.
fn line_has_word(line: &str, name: &str) -> bool {
    let name_bytes = name.as_bytes();
    let line_bytes = line.as_bytes();
    if name_bytes.len() > line_bytes.len() {
        return false;
    }
    for (i, window) in line_bytes.windows(name_bytes.len()).enumerate() {
        if window != name_bytes {
            continue;
        }
        let before_ok = i == 0 || !is_ident_char(line_bytes[i - 1]);
        let after_ok = i + name_bytes.len() >= line_bytes.len()
            || !is_ident_char(line_bytes[i + name_bytes.len()]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Extract the last name from a use declaration argument.
/// `std::io` -> "io", `std::collections::HashMap` -> "HashMap"
fn extract_use_name(node: tree_sitter::Node, source: &[u8]) -> String {
    match node.kind() {
        "scoped_identifier" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return query_engine::node_text(name_node, source).to_string();
            }
        }
        "identifier" => {
            return query_engine::node_text(node, source).to_string();
        }
        "use_as_clause" => {
            // `use X as Y` -- the alias Y is what's imported
            if let Some(alias) = node.child_by_field_name("alias") {
                return query_engine::node_text(alias, source).to_string();
            }
        }
        "scoped_use_list" | "use_list" => {
            // `use std::{io, fs}` -- skip these, too complex for simple check
            return String::new();
        }
        "use_wildcard" => {
            // `use std::*` -- always "used"
            return String::new();
        }
        _ => {}
    }
    // Fallback: use full text
    let text = query_engine::node_text(node, source);
    // Get last segment after ::
    text.rsplit("::").next().unwrap_or(text).to_string()
}

/// Rust todo!() and unimplemented!() macros left in code.
fn check_todo_macro(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source_bytes: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, TODO_MACRO_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("rust todo-macro query failed: {e}"));
            return Vec::new();
        }
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let name_idx = query_engine::capture_index_or_skip(&query, "macro_name");
    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    let mut echoes = Vec::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        let mut macro_name = "";
        let mut line = 0;

        for cap in m.captures {
            if cap.index as usize == name_idx {
                macro_name = query_engine::node_text(cap.node, source_bytes);
            }
            if cap.index as usize == match_idx {
                line = cap.node.start_position().row + 1;
            }
        }

        if macro_name == "todo" || macro_name == "unimplemented" {
            echoes.push(Echo {
                check: "todo-macro".to_string(),
                line,
                message: format!("{}!() -- implement before merging", macro_name),
                suggestion: String::new(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

/// Rust unreachable code: statements after return in the same block.
fn check_unreachable_code(tree: &tree_sitter::Tree) -> Vec<Echo> {
    let mut echoes = Vec::new();
    find_unreachable(tree.root_node(), &mut echoes);
    echoes
}

fn find_unreachable(node: tree_sitter::Node, echoes: &mut Vec<Echo>) {
    if node.kind() == "block" {
        let mut found_terminal = false;
        let mut cursor = node.walk();
        for stmt in node.named_children(&mut cursor) {
            // Skip comments -- they are not executable code.
            if stmt.kind() == "line_comment" || stmt.kind() == "block_comment" {
                continue;
            }
            if found_terminal {
                echoes.push(Echo {
                    check: "unreachable-code".to_string(),
                    line: stmt.start_position().row + 1,
                    message: "unreachable code after return".to_string(),
                    suggestion: "Remove unreachable code.".to_string(),
                    severity: Severity::Error,
                    fix: None,
                });
                break;
            }
            // return expression or return;
            if stmt.kind() == "return_expression" {
                found_terminal = true;
            }
            // expression_statement containing return_expression
            if stmt.kind() == "expression_statement" {
                let mut inner_cursor = stmt.walk();
                for child in stmt.children(&mut inner_cursor) {
                    if child.kind() == "return_expression" {
                        found_terminal = true;
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_unreachable(child, echoes);
    }
}

/// Rust placeholder: function bodies that are just unimplemented!() or todo!().
fn check_placeholder(tree: &tree_sitter::Tree, source_bytes: &[u8]) -> Vec<Echo> {
    let mut echoes = Vec::new();
    let mut cursor = tree.root_node().walk();

    for child in tree.root_node().children(&mut cursor) {
        if child.kind() == "function_item" {
            if let Some(body) = child.child_by_field_name("body") {
                // Check if body has exactly 1 named child that's a macro invocation
                if body.named_child_count() == 1 {
                    if let Some(stmt) = body.named_child(0) {
                        let macro_node = if stmt.kind() == "expression_statement" {
                            stmt.named_child(0)
                        } else {
                            Some(stmt)
                        };

                        if let Some(mn) = macro_node {
                            if mn.kind() == "macro_invocation" {
                                if let Some(name_node) = mn.child_by_field_name("macro") {
                                    let name = query_engine::node_text(name_node, source_bytes);
                                    if name == "todo" || name == "unimplemented" {
                                        let fn_name = child
                                            .child_by_field_name("name")
                                            .map(|n| query_engine::node_text(n, source_bytes))
                                            .unwrap_or("<unknown>");
                                        echoes.push(Echo {
                                            check: "placeholder-code".to_string(),
                                            line: child.start_position().row + 1,
                                            message: format!(
                                                "function `{}` is a placeholder -- implement before merging",
                                                fn_name
                                            ),
                                            suggestion: String::new(),
                                            severity: Severity::Warn,
                                            fix: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    echoes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EckoConfig;

    #[test]
    fn test_unused_import() {
        let source = "use std::io;\nuse std::fs;\n\nfn main() {\n    let _ = fs::read(\"x\");\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let unused: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unused-imports")
            .collect();
        assert_eq!(unused.len(), 1, "io should be unused: {unused:?}");
        assert!(unused[0].message.contains("io"));
    }

    #[test]
    fn test_all_imports_used() {
        let source = "use std::io;\n\nfn main() {\n    let _ = io::stdin();\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let unused: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unused-imports")
            .collect();
        assert!(unused.is_empty(), "all imports used: {unused:?}");
    }

    #[test]
    fn test_trait_import_not_flagged() {
        // Trait imports used implicitly via method calls should be skipped.
        let source =
            "use std::io::Write;\n\nfn main() {\n    let mut f = std::fs::File::create(\"x\").unwrap();\n    f.write_all(b\"hello\").unwrap();\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let unused: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unused-imports")
            .collect();
        assert!(
            unused.is_empty(),
            "Write is a trait import (allowlisted): {unused:?}"
        );
    }

    #[test]
    fn test_derive_macro_not_flagged() {
        let source = "use serde::Deserialize;\n\n#[derive(Debug, Deserialize, Clone)]\npub struct Config {\n    pub name: String,\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let unused: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unused-imports")
            .collect();
        assert!(
            unused.is_empty(),
            "Deserialize used via derive should not be flagged: {unused:?}"
        );
    }

    #[test]
    fn test_import_used_before_test_module_import() {
        // Simulates: main code uses `config::load_config()`, test module has its own imports
        let source = "use crate::config;\nuse std::io;\n\nfn main() {\n    config::load_config();\n}\n\n#[cfg(test)]\nmod tests {\n    use std::io::Write;\n    fn t() { let _ = Write; }\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let unused: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unused-imports")
            .collect();
        // `io` is unused, `config` is used -- only `io` should be flagged
        assert_eq!(unused.len(), 1, "only `io` unused: {unused:?}");
        assert!(unused[0].message.contains("io"));
    }

    #[test]
    fn test_todo_macro() {
        let source = "fn foo() {\n    todo!();\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let todos: Vec<_> = echoes.iter().filter(|e| e.check == "todo-macro").collect();
        assert_eq!(todos.len(), 1, "should detect todo!(): {todos:?}");
    }

    #[test]
    fn test_unimplemented_macro() {
        let source = "fn foo() {\n    unimplemented!();\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let todos: Vec<_> = echoes.iter().filter(|e| e.check == "todo-macro").collect();
        assert_eq!(todos.len(), 1, "should detect unimplemented!(): {todos:?}");
    }

    #[test]
    fn test_println_not_flagged() {
        let source = "fn main() {\n    println!(\"hello\");\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let todos: Vec<_> = echoes.iter().filter(|e| e.check == "todo-macro").collect();
        assert!(todos.is_empty(), "println should not be flagged");
    }

    #[test]
    fn test_unreachable_code() {
        let source = "fn foo() {\n    return;\n    let x = 1;\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let unr: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unreachable-code")
            .collect();
        assert_eq!(unr.len(), 1, "should detect unreachable: {unr:?}");
    }

    #[test]
    fn test_comment_after_return_not_unreachable() {
        // `return; // comment` in an if body -- the comment is not executable code
        let source = "fn f(cwd: &str) {\n    if cwd.is_empty() {\n        return; // skip\n    }\n    println!(\"ok\");\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let unr: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unreachable-code")
            .collect();
        assert!(
            unr.is_empty(),
            "comment after return in if body is not unreachable: {unr:?}"
        );
    }

    #[test]
    fn test_placeholder_function() {
        let source =
            "fn placeholder() {\n    todo!();\n}\n\nfn real() {\n    println!(\"real\");\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.rs", source, &config);
        let ph: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "placeholder-code")
            .collect();
        assert_eq!(ph.len(), 1, "should detect placeholder: {ph:?}");
        assert!(ph[0].message.contains("placeholder"));
    }
}
