//! Tree-sitter query compilation and execution.
//!
//! Provides a `QueryCheck` struct that pairs a compiled tree-sitter query
//! with metadata (check name, message, severity, target capture). The
//! `run_query` function executes a check against a parsed syntax tree and
//! produces `Echo` results.

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Query, QueryCursor, Tree};

use crate::echo::{Echo, Severity};

/// A single tree-sitter-based check.
pub struct QueryCheck {
    /// Kebab-case check name (e.g. `"unused-imports"`).
    pub name: String,
    /// Compiled tree-sitter query.
    pub query: Query,
    /// Human-readable message template for echoes. Use `{text}` as a
    /// placeholder for the captured node text.
    pub message: String,
    /// Echo severity.
    pub severity: Severity,
    /// Which capture to report (e.g. `"match"`, without the `@` prefix).
    pub capture_name: String,
}

/// Compile a tree-sitter query from an S-expression source string.
///
/// Returns `Err(String)` with the error description if the query is invalid.
pub fn compile_query(language: &Language, source: &str) -> Result<Query, String> {
    Query::new(language, source).map_err(|e| format!("query compilation failed: {e}"))
}

/// Parse source code into a tree-sitter syntax tree.
///
/// Returns `None` if parsing fails (e.g. no language set on the parser).
pub fn parse_source(parser: &mut tree_sitter::Parser, source: &str) -> Option<Tree> {
    parser.parse(source, None)
}

/// Execute a single `QueryCheck` against a parsed tree and return echoes.
///
/// For each query match, finds the capture whose name matches
/// `check.capture_name`, extracts its location and text, and produces
/// an `Echo`. The returned echoes have empty `suggestion` fields -- the
/// caller should fill those in or leave them empty.
pub fn run_query(check: &QueryCheck, tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let mut echoes = Vec::new();
    let mut cursor = QueryCursor::new();

    // Find the capture index that matches check.capture_name.
    let capture_index = check
        .query
        .capture_names()
        .iter()
        .position(|name| *name == check.capture_name);

    let capture_idx = match capture_index {
        Some(idx) => idx,
        None => return echoes, // capture name not found in query -- no results
    };

    let mut matches = cursor.matches(&check.query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        for capture in m.captures {
            if capture.index as usize != capture_idx {
                continue;
            }

            let node = capture.node;
            let line = node.start_position().row + 1; // 1-based line numbers
            let text = node_text(node, source).to_string();

            echoes.push(Echo {
                check: check.name.clone(),
                line,
                message: check.message.replace("{text}", &text),
                suggestion: String::new(),
                severity: check.severity,
                fix: None,
            });
        }
    }

    echoes
}

/// Find the index of a named capture in a query. Returns None if not found.
pub fn capture_index(query: &Query, name: &str) -> Option<usize> {
    query.capture_names().iter().position(|n| *n == name)
}

/// Find the index of a named capture, returning `usize::MAX` if not found.
/// Use this in check functions where a missing capture means "no matches" --
/// the sentinel value ensures `cap.index as usize != idx` always holds,
/// so the check safely produces zero echoes.
pub fn capture_index_or_skip(query: &Query, name: &str) -> usize {
    query
        .capture_names()
        .iter()
        .position(|n| *n == name)
        .unwrap_or(usize::MAX)
}

/// Extract UTF-8 text for a syntax node from the source bytes.
///
/// Returns `"<invalid utf-8>"` if the byte range is not valid UTF-8.
pub fn node_text<'a>(node: Node<'a>, source: &'a [u8]) -> &'a str {
    let start = node.start_byte();
    let end = node.end_byte();
    if end <= source.len() {
        std::str::from_utf8(&source[start..end]).unwrap_or("<invalid utf-8>")
    } else {
        "<invalid utf-8>"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{self, Lang};

    #[test]
    fn test_compile_query_valid() {
        let language = lang::get_tree_sitter_language(Lang::Python).unwrap();
        let result = compile_query(&language, "(function_definition name: (identifier) @name)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_query_invalid() {
        let language = lang::get_tree_sitter_language(Lang::Python).unwrap();
        let result = compile_query(&language, "(not_a_real_node @x)");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_source() {
        let mut parser = lang::create_parser(Lang::Python).unwrap();
        let tree = parse_source(&mut parser, "def foo():\n    pass\n");
        assert!(tree.is_some());
        let tree = tree.unwrap();
        assert_eq!(tree.root_node().kind(), "module");
    }

    #[test]
    fn test_run_query_python_functions() {
        let ts_lang = lang::get_tree_sitter_language(Lang::Python).unwrap();
        let mut parser = lang::create_parser(Lang::Python).unwrap();

        let source = "def foo():\n    pass\n\ndef bar():\n    pass\n";
        let tree = parse_source(&mut parser, source).unwrap();

        let query =
            compile_query(&ts_lang, "(function_definition name: (identifier) @match)").unwrap();

        let check = QueryCheck {
            name: "test-check".to_string(),
            query,
            message: "found function: {text}".to_string(),
            severity: Severity::Warn,
            capture_name: "match".to_string(),
        };

        let echoes = run_query(&check, &tree, source.as_bytes());
        assert_eq!(echoes.len(), 2);
        assert_eq!(echoes[0].line, 1);
        assert!(echoes[0].message.contains("foo"));
        assert_eq!(echoes[1].line, 4);
        assert!(echoes[1].message.contains("bar"));
    }

    #[test]
    fn test_node_text_valid_utf8() {
        let mut parser = lang::create_parser(Lang::Python).unwrap();
        let source = "x = 42\n";
        let tree = parse_source(&mut parser, source).unwrap();
        let root = tree.root_node();
        let text = node_text(root, source.as_bytes());
        assert_eq!(text, "x = 42\n");
    }

    #[test]
    fn test_capture_name_not_found() {
        let ts_lang = lang::get_tree_sitter_language(Lang::Python).unwrap();
        let mut parser = lang::create_parser(Lang::Python).unwrap();

        let source = "def foo():\n    pass\n";
        let tree = parse_source(&mut parser, source).unwrap();

        let query =
            compile_query(&ts_lang, "(function_definition name: (identifier) @name)").unwrap();

        let check = QueryCheck {
            name: "test".to_string(),
            query,
            message: "found".to_string(),
            severity: Severity::Warn,
            capture_name: "nonexistent".to_string(), // wrong capture name
        };

        let echoes = run_query(&check, &tree, source.as_bytes());
        assert!(echoes.is_empty());
    }
}
