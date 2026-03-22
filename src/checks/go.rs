//! Go checks -- tree-sitter query-based.
//!
//! Checks: unused-imports, empty-error-check, unreachable-code, placeholder-code.

use streaming_iterator::StreamingIterator;

use crate::config::EckoConfig;
use crate::debug;
use crate::echo::{Echo, Severity};
use crate::lang::{self, Lang};
use crate::query_engine;

const UNUSED_IMPORTS_QUERY: &str = include_str!("../../queries/go/unused_imports.scm");
const EMPTY_ERROR_CHECK_QUERY: &str = include_str!("../../queries/go/empty_error_check.scm");
const PLACEHOLDER_QUERY: &str = include_str!("../../queries/go/placeholder.scm");

pub fn run_checks(_file_path: &str, source: &str, _config: &EckoConfig) -> Vec<Echo> {
    let (ts_lang, tree) = match lang::parse_for_checks(Lang::Go, source) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let source_bytes = source.as_bytes();
    let mut echoes = Vec::new();

    echoes.extend(check_unused_imports(&ts_lang, &tree, source, source_bytes));
    echoes.extend(check_empty_error_check(&ts_lang, &tree, source_bytes));
    echoes.extend(check_unreachable_code(&tree));
    echoes.extend(check_placeholder(&ts_lang, &tree, source_bytes));

    echoes
}

/// Go unused imports: collect import paths, check if their package name appears in code.
fn check_unused_imports(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source: &str,
    source_bytes: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, UNUSED_IMPORTS_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("go unused-imports query failed: {e}"));
            return Vec::new();
        }
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let path_idx = query_engine::capture_index_or_skip(&query, "path");
    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    let mut imports: Vec<(String, usize, usize)> = Vec::new(); // (pkg_name, line, end_byte)

    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        let mut path_text = "";
        let mut line = 0;
        let mut end = 0;

        for cap in m.captures {
            if cap.index as usize == path_idx {
                path_text = query_engine::node_text(cap.node, source_bytes);
            }
            if cap.index as usize == match_idx {
                line = cap.node.start_position().row + 1;
                end = cap.node.end_byte();
            }
        }

        // Extract package name from path: "fmt" -> fmt, "encoding/json" -> json
        let clean_path = path_text.trim_matches('"');
        let pkg_name = clean_path.rsplit('/').next().unwrap_or(clean_path);
        if !pkg_name.is_empty() {
            imports.push((pkg_name.to_string(), line, end));
        }
    }

    let import_end = imports.iter().map(|(_, _, end)| *end).max().unwrap_or(0);
    let code_after_imports = if import_end < source.len() {
        &source[import_end..]
    } else {
        ""
    };

    let mut echoes = Vec::new();
    for (pkg_name, line, _end) in &imports {
        let used = code_after_imports.contains(&format!("{}.", pkg_name))
            || code_after_imports.contains(&format!("{} ", pkg_name));

        if !used {
            echoes.push(Echo {
                check: "unused-imports".to_string(),
                line: *line,
                message: format!("unused import \"{}\"", pkg_name),
                suggestion: "Remove unused import.".to_string(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

/// Go empty error check: `if err != nil {}` with empty block.
fn check_empty_error_check(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source_bytes: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, EMPTY_ERROR_CHECK_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("go empty-error-check query failed: {e}"));
            return Vec::new();
        }
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let match_idx = query_engine::capture_index_or_skip(&query, "match");
    let body_idx = query_engine::capture_index_or_skip(&query, "body");

    let mut echoes = Vec::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        let mut is_empty_body = false;
        let mut line = 0;

        for cap in m.captures {
            if cap.index as usize == match_idx {
                line = cap.node.start_position().row + 1;
            }
            if cap.index as usize == body_idx {
                is_empty_body = cap.node.named_child_count() == 0;
            }
        }

        if is_empty_body && line > 0 {
            echoes.push(Echo {
                check: "empty-error-check".to_string(),
                line,
                message: "error check with empty body -- handle or explicitly ignore the error"
                    .to_string(),
                suggestion: String::new(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

/// Go unreachable code: statements after return in the same block.
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
                    message: "unreachable code after return/panic".to_string(),
                    suggestion: "Remove unreachable code.".to_string(),
                    severity: Severity::Error,
                    fix: None,
                });
                break;
            }
            if stmt.kind() == "return_statement" {
                found_terminal = true;
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_unreachable(child, echoes);
    }
}

/// Go placeholder code: panic("not implemented") or panic("TODO").
fn check_placeholder(
    ts_lang: &tree_sitter::Language,
    tree: &tree_sitter::Tree,
    source_bytes: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, PLACEHOLDER_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("go placeholder query failed: {e}"));
            return Vec::new();
        }
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let fn_idx = query_engine::capture_index_or_skip(&query, "fn_name");
    let arg_idx = query_engine::capture_index_or_skip(&query, "arg");
    let match_idx = query_engine::capture_index_or_skip(&query, "match");

    let mut echoes = Vec::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        let mut fn_name = "";
        let mut arg_text = "";
        let mut line = 0;

        for cap in m.captures {
            if cap.index as usize == fn_idx {
                fn_name = query_engine::node_text(cap.node, source_bytes);
            }
            if cap.index as usize == arg_idx {
                arg_text = query_engine::node_text(cap.node, source_bytes);
            }
            if cap.index as usize == match_idx {
                line = cap.node.start_position().row + 1;
            }
        }

        if fn_name == "panic" {
            let arg_lower = arg_text.to_lowercase();
            if arg_lower.contains("not implemented")
                || arg_lower.contains("todo")
                || arg_lower.contains("unimplemented")
            {
                echoes.push(Echo {
                    check: "placeholder-code".to_string(),
                    line,
                    message: "placeholder panic -- implement before merging".to_string(),
                    suggestion: String::new(),
                    severity: Severity::Warn,
                    fix: None,
                });
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
    fn test_unused_import_detected() {
        let source = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n)\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.go", source, &config);
        let unused: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unused-imports")
            .collect();
        assert_eq!(unused.len(), 1, "os should be unused: {unused:?}");
        assert!(unused[0].message.contains("os"));
    }

    #[test]
    fn test_all_imports_used() {
        let source =
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.go", source, &config);
        let unused: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unused-imports")
            .collect();
        assert!(unused.is_empty(), "no unused imports expected: {unused:?}");
    }

    #[test]
    fn test_empty_error_check() {
        let source = "package main\n\nfunc foo() error {\n\terr := doSomething()\n\tif err != nil {\n\t}\n\treturn nil\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.go", source, &config);
        let empty: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "empty-error-check")
            .collect();
        assert_eq!(empty.len(), 1, "should detect empty error check: {empty:?}");
    }

    #[test]
    fn test_placeholder_panic() {
        let source = "package main\n\nfunc foo() {\n\tpanic(\"not implemented\")\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.go", source, &config);
        let ph: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "placeholder-code")
            .collect();
        assert_eq!(ph.len(), 1, "should detect placeholder panic: {ph:?}");
    }

    #[test]
    fn test_normal_panic_not_flagged() {
        let source = "package main\n\nfunc foo() {\n\tpanic(\"something went wrong\")\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.go", source, &config);
        let ph: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "placeholder-code")
            .collect();
        assert!(ph.is_empty(), "normal panic should not be flagged");
    }

    #[test]
    fn test_unreachable_code() {
        let source = "package main\n\nfunc foo() {\n\treturn\n\tx := 1\n\t_ = x\n}\n";
        let config = EckoConfig::default();
        let echoes = run_checks("main.go", source, &config);
        let unr: Vec<_> = echoes
            .iter()
            .filter(|e| e.check == "unreachable-code")
            .collect();
        assert_eq!(unr.len(), 1, "should detect unreachable code: {unr:?}");
    }
}
