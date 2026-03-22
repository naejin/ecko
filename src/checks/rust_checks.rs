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
                    if !name.is_empty() {
                        imports.push((name, line, start, end));
                    }
                }
            }
        }
    }

    // Find end of all use declarations
    let import_end = imports.iter().map(|(_, _, _, end)| *end).max().unwrap_or(0);
    let code_after = if import_end < source.len() {
        &source[import_end..]
    } else {
        ""
    };

    let mut echoes = Vec::new();
    for (name, line, _start, _end) in &imports {
        // Check if the name appears in code after imports
        // Look for name as a standalone identifier (preceded/followed by non-alphanumeric)
        let pattern = name.to_string();
        let used = code_after.contains(&pattern);
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
