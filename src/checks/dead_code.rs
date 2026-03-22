//! Cross-file dead code detection -- replaces vulture (Python) and knip (JS/TS).
//!
//! Algorithm (one hard thing per function):
//! 1. collect_definitions() -- parse files, gather top-level function/class names
//! 2. collect_usages() -- parse files, gather all identifier references
//! 3. find_unused() -- diff definitions vs usages, apply skip filters
//! 4. run_dead_code_analysis() -- orchestrates 1-3, returns echoes by file
//!
//! Scope: Python + JS/TS only (Go/Rust have compiler-enforced unused detection).
//! Function/class level only (not variable-level -- too noisy).

use std::collections::{HashMap, HashSet};

use crate::config::EckoConfig;
use crate::debug;
use crate::echo::{Echo, Severity};
use crate::fingerprint;
use crate::lang::{self, Lang};
use crate::query_engine;

// ---------------------------------------------------------------------------
// Skip filters (ported from Python vulture_adapter.py)
// ---------------------------------------------------------------------------

/// Protocol params -- always framework-injected, never genuinely unused.
const ALWAYS_SKIP: &[&str] = &[
    "exc_type", "exc_val", "exc_tb", "exc_info", // __exit__ (PEP 343)
    "signum", "frame", // signal handlers
    "objtype", "owner",  // descriptor __get__
    "sender", // signal/event handlers
];

/// pytest built-in fixtures -- only skip in test/conftest files.
const PYTEST_SKIP: &[&str] = &[
    "tmp_path",
    "tmp_path_factory",
    "capsys",
    "capfd",
    "caplog",
    "monkeypatch",
    "pytestconfig",
    "recwarn",
    "tmpdir",
    "tmpdir_factory",
];

/// Framework-specific skip sets.
fn framework_skips(frameworks: &HashSet<String>) -> HashSet<String> {
    let mut skips = HashSet::new();
    if frameworks.contains("fastapi") {
        for s in &["db", "session", "request", "response", "Depends"] {
            skips.insert(s.to_string());
        }
    }
    if frameworks.contains("flask") {
        for s in &["app", "g", "request", "session"] {
            skips.insert(s.to_string());
        }
    }
    if frameworks.contains("django") {
        for s in &["request", "queryset", "Meta", "verbose_name"] {
            skips.insert(s.to_string());
        }
    }
    skips
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Definition {
    name: String,
    file: String,
    line: usize,
}

// ---------------------------------------------------------------------------
// Core algorithm
// ---------------------------------------------------------------------------

/// Collect top-level function and class definitions from Python and JS/TS files.
fn collect_definitions(files: &[String]) -> Vec<Definition> {
    let mut defs = Vec::new();

    for file_path in files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let detected_lang = lang::detect_language(file_path);
        let (_ts_lang, tree) = match lang::parse_for_checks(detected_lang, &source) {
            Some(v) => v,
            None => continue,
        };

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        match detected_lang {
            Lang::Python => {
                // Top-level function_definition and class_definition
                let mut cursor = root.walk();
                for child in root.children(&mut cursor) {
                    match child.kind() {
                        "function_definition" | "decorated_definition" => {
                            let name = extract_python_def_name(child, source_bytes);
                            if let Some(name) = name {
                                defs.push(Definition {
                                    name,
                                    file: file_path.clone(),
                                    line: child.start_position().row + 1,
                                });
                            }
                        }
                        "class_definition" => {
                            if let Some(name_node) = child.child_by_field_name("name") {
                                let name = node_text(name_node, source_bytes).to_string();
                                defs.push(Definition {
                                    name,
                                    file: file_path.clone(),
                                    line: child.start_position().row + 1,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Lang::JavaScript | Lang::TypeScript | Lang::Tsx => {
                // Exported functions, classes, variables
                collect_js_exports(root, source_bytes, file_path, &mut defs);
            }
            _ => {} // Go/Rust: skip (compiler handles unused)
        }
    }

    defs
}

/// Extract the function name from a Python function_definition or decorated_definition.
fn extract_python_def_name(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    if node.kind() == "decorated_definition" {
        // Find the function_definition child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_definition" || child.kind() == "class_definition" {
                return child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source).to_string());
            }
        }
        None
    } else {
        node.child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
    }
}

/// Collect exported symbols from JS/TS files.
fn collect_js_exports(
    root: tree_sitter::Node,
    source: &[u8],
    file_path: &str,
    defs: &mut Vec<Definition>,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "export_statement" {
            // Look for the declaration inside the export
            let mut inner = child.walk();
            for grandchild in child.children(&mut inner) {
                match grandchild.kind() {
                    "function_declaration" | "class_declaration" => {
                        if let Some(name_node) = grandchild.child_by_field_name("name") {
                            defs.push(Definition {
                                name: node_text(name_node, source).to_string(),
                                file: file_path.to_string(),
                                line: grandchild.start_position().row + 1,
                            });
                        }
                    }
                    "lexical_declaration" | "variable_declaration" => {
                        // export const x = ..., export let y = ...
                        let mut decl_cursor = grandchild.walk();
                        for decl_child in grandchild.children(&mut decl_cursor) {
                            if decl_child.kind() == "variable_declarator" {
                                if let Some(name_node) = decl_child.child_by_field_name("name") {
                                    defs.push(Definition {
                                        name: node_text(name_node, source).to_string(),
                                        file: file_path.to_string(),
                                        line: decl_child.start_position().row + 1,
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Collect identifier references per file (the "usage" side).
/// Returns a map of name -> set of files that reference it.
fn collect_usages(files: &[String]) -> HashMap<String, HashSet<String>> {
    let mut usages: HashMap<String, HashSet<String>> = HashMap::new();

    for file_path in files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let detected_lang = lang::detect_language(file_path);
        let (_ts_lang, tree) = match lang::parse_for_checks(detected_lang, &source) {
            Some(v) => v,
            None => {
                let mut names = HashSet::new();
                collect_usages_from_text(&source, &mut names);
                for name in names {
                    usages.entry(name).or_default().insert(file_path.clone());
                }
                continue;
            }
        };

        let mut names = HashSet::new();
        collect_identifiers(tree.root_node(), source.as_bytes(), &mut names);

        for name in names {
            usages.entry(name).or_default().insert(file_path.clone());
        }

        // __all__ names count as "exported" -- mark as used from a synthetic external file
        if detected_lang == Lang::Python {
            let mut all_names = HashSet::new();
            collect_all_list(&source, &mut all_names);
            for name in all_names {
                usages
                    .entry(name)
                    .or_default()
                    .insert("__all__".to_string());
            }
        }
    }

    usages
}

/// Recursively collect all identifier text from a tree-sitter tree.
fn collect_identifiers(node: tree_sitter::Node, source: &[u8], usages: &mut HashSet<String>) {
    if node.kind() == "identifier"
        || node.kind() == "property_identifier"
        || node.kind() == "type_identifier"
    {
        let text = node_text(node, source);
        if !text.is_empty() {
            usages.insert(text.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifiers(child, source, usages);
    }
}

/// Fallback: extract word-like tokens from source text.
fn collect_usages_from_text(source: &str, usages: &mut HashSet<String>) {
    for word in source.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if !word.is_empty()
            && word
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            usages.insert(word.to_string());
        }
    }
}

/// Extract names from Python `__all__ = [...]` lists.
fn collect_all_list(source: &str, usages: &mut HashSet<String>) {
    // Simple regex: __all__ = ["name1", "name2", ...]
    use std::sync::OnceLock;
    static ALL_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = ALL_RE.get_or_init(|| regex::Regex::new(r#"['"](\w+)['"]"#).unwrap());
    // Find the byte offset where __all__ starts, scan only from there
    if let Some(all_pos) = source.find("__all__") {
        let from_all = &source[all_pos..];
        for cap in re.captures_iter(from_all) {
            if let Some(name) = cap.get(1) {
                usages.insert(name.as_str().to_string());
            }
        }
    }
}

/// Diff definitions against usages, applying skip filters.
fn find_unused(
    defs: Vec<Definition>,
    usages: &HashMap<String, HashSet<String>>,
    frameworks: &HashSet<String>,
    test_files: &HashSet<String>,
) -> HashMap<String, Vec<Echo>> {
    let always_skip: HashSet<&str> = ALWAYS_SKIP.iter().copied().collect();
    let pytest_skip: HashSet<&str> = PYTEST_SKIP.iter().copied().collect();
    let fw_skips = framework_skips(frameworks);

    let mut echoes: HashMap<String, Vec<Echo>> = HashMap::new();

    for def in defs {
        // Skip filter: name in ALWAYS_SKIP
        if always_skip.contains(def.name.as_str()) {
            continue;
        }

        // Skip filter: dunder names
        if def.name.starts_with("__") && def.name.ends_with("__") {
            continue;
        }

        // Skip filter: underscore-prefixed (private convention)
        if def.name.starts_with('_') && !def.name.starts_with("__") {
            continue;
        }

        // Skip filter: pytest fixtures in test files
        if test_files.contains(&def.file) && pytest_skip.contains(def.name.as_str()) {
            continue;
        }

        // Skip filter: framework-injected names
        if fw_skips.contains(&def.name) {
            continue;
        }

        // Skip filter: name is referenced from a file OTHER than its definition file.
        // A name appearing only in its own file (as the definition itself) is unused.
        if let Some(ref_files) = usages.get(&def.name) {
            let used_elsewhere = ref_files.iter().any(|f| f != &def.file);
            if used_elsewhere {
                continue;
            }
        }

        // Skip filter: common entry points
        if def.name == "main" || def.name == "setup" || def.name == "teardown" {
            continue;
        }

        // Skip test functions (test_* names are entry points for pytest)
        if def.name.starts_with("test_") {
            continue;
        }

        echoes.entry(def.file.clone()).or_default().push(Echo {
            check: "dead-code".to_string(),
            line: def.line,
            message: format!("`{}` appears to be unused", def.name),
            suggestion: "Remove it if truly unused.".to_string(),
            severity: Severity::Warn,
            fix: None,
        });
    }

    echoes
}

/// Run dead code analysis across multiple files.
/// Returns echoes grouped by file path.
pub fn run_dead_code_analysis(
    files: &[String],
    cwd: &str,
    _config: &EckoConfig,
) -> HashMap<String, Vec<Echo>> {
    // Safety: require at least 2 files to avoid "everything is unused" on single-file projects
    if files.len() < 2 {
        debug::debug("dead-code: skipping (fewer than 2 files)");
        return HashMap::new();
    }

    // Only analyze Python and JS/TS files
    let analyzable: Vec<String> = files
        .iter()
        .filter(|f| {
            matches!(
                lang::detect_language(f),
                Lang::Python | Lang::JavaScript | Lang::TypeScript | Lang::Tsx
            )
        })
        .cloned()
        .collect();

    if analyzable.is_empty() {
        return HashMap::new();
    }

    debug::debug(&format!("dead-code: analyzing {} files", analyzable.len()));

    // Detect frameworks for skip filtering
    let frameworks = fingerprint::detect_frameworks(cwd);

    // Identify test files for pytest fixture skipping
    let test_files: HashSet<String> = analyzable
        .iter()
        .filter(|f| lang::is_test_file(f))
        .cloned()
        .collect();

    // Core algorithm
    let defs = collect_definitions(&analyzable);
    let usages = collect_usages(&analyzable);

    debug::debug(&format!(
        "dead-code: {} definitions, {} unique usages",
        defs.len(),
        usages.len()
    ));

    find_unused(defs, &usages, &frameworks, &test_files)
}

/// Delegate to query_engine::node_text (no circular dependency).
fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a [u8]) -> &'a str {
    query_engine::node_text(node, source)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(content: &str, suffix: &str) -> (tempfile::NamedTempFile, String) {
        let mut f = tempfile::Builder::new().suffix(suffix).tempfile().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        let path = f.path().to_string_lossy().to_string();
        (f, path)
    }

    #[test]
    fn test_unused_function_detected() {
        let source_a = "def used_func():\n    pass\n\ndef unused_func():\n    pass\n";
        let source_b = "from a import used_func\nused_func()\n";
        let (_fa, path_a) = write_temp(source_a, ".py");
        let (_fb, path_b) = write_temp(source_b, ".py");

        let files = vec![path_a.clone(), path_b];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        let echoes = result.get(&path_a).cloned().unwrap_or_default();
        let dead: Vec<_> = echoes
            .iter()
            .filter(|e| e.message.contains("unused_func"))
            .collect();
        assert_eq!(dead.len(), 1, "unused_func should be flagged: {echoes:?}");
    }

    #[test]
    fn test_used_function_not_flagged() {
        let source_a = "def my_func():\n    pass\n";
        let source_b = "from a import my_func\nmy_func()\n";
        let (_fa, path_a) = write_temp(source_a, ".py");
        let (_fb, path_b) = write_temp(source_b, ".py");

        let files = vec![path_a.clone(), path_b];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        let echoes = result.get(&path_a).cloned().unwrap_or_default();
        assert!(
            echoes.is_empty(),
            "used function should not be flagged: {echoes:?}"
        );
    }

    #[test]
    fn test_dunder_skipped() {
        let source = "def __init__(self):\n    pass\n\ndef __repr__(self):\n    pass\n";
        let source_b = "x = 1\n";
        let (_fa, path_a) = write_temp(source, ".py");
        let (_fb, path_b) = write_temp(source_b, ".py");

        let files = vec![path_a.clone(), path_b];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        let echoes = result.get(&path_a).cloned().unwrap_or_default();
        assert!(
            echoes.is_empty(),
            "dunder methods should be skipped: {echoes:?}"
        );
    }

    #[test]
    fn test_private_skipped() {
        let source = "def _private_helper():\n    pass\n";
        let source_b = "x = 1\n";
        let (_fa, path_a) = write_temp(source, ".py");
        let (_fb, path_b) = write_temp(source_b, ".py");

        let files = vec![path_a.clone(), path_b];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        let echoes = result.get(&path_a).cloned().unwrap_or_default();
        assert!(
            echoes.is_empty(),
            "private functions should be skipped: {echoes:?}"
        );
    }

    #[test]
    fn test_single_file_skipped() {
        let source = "def unused():\n    pass\n";
        let (_f, path) = write_temp(source, ".py");

        let files = vec![path];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        assert!(result.is_empty(), "single file should be skipped");
    }

    #[test]
    fn test_test_functions_not_flagged() {
        let source = "def test_something():\n    assert True\n\ndef helper():\n    pass\n";
        let source_b = "x = 1\n";
        let (_fa, path_a) = write_temp(source, ".py");
        let (_fb, path_b) = write_temp(source_b, ".py");

        let files = vec![path_a.clone(), path_b];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        let echoes = result.get(&path_a).cloned().unwrap_or_default();
        let test_flagged: Vec<_> = echoes
            .iter()
            .filter(|e| e.message.contains("test_"))
            .collect();
        assert!(
            test_flagged.is_empty(),
            "test_ functions should not be flagged"
        );
    }

    #[test]
    fn test_all_list_marks_as_used() {
        let source = "def exported():\n    pass\n\n__all__ = ['exported']\n";
        let source_b = "x = 1\n";
        let (_fa, path_a) = write_temp(source, ".py");
        let (_fb, path_b) = write_temp(source_b, ".py");

        let files = vec![path_a.clone(), path_b];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        let echoes = result.get(&path_a).cloned().unwrap_or_default();
        assert!(echoes.is_empty(), "__all__ should mark as used: {echoes:?}");
    }

    #[test]
    fn test_protocol_params_skipped() {
        let source = "def handler(exc_type, exc_val, exc_tb):\n    pass\n";
        let source_b = "x = 1\n";
        let (_fa, path_a) = write_temp(source, ".py");
        let (_fb, path_b) = write_temp(source_b, ".py");

        let files = vec![path_a.clone(), path_b];
        let config = EckoConfig::default();
        let result = run_dead_code_analysis(&files, "/tmp", &config);

        let echoes = result.get(&path_a).cloned().unwrap_or_default();
        let proto: Vec<_> = echoes
            .iter()
            .filter(|e| e.message.contains("exc_type"))
            .collect();
        assert!(proto.is_empty(), "protocol params should be skipped");
    }
}
