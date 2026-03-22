//! Python checks -- tree-sitter query-based with post-filtering.
//!
//! All 12 checks:
//!   1. bare-except (E722), 2. star-imports (F403), 3. unused-imports (F401),
//!   4. singleton-comparison (E711/E712), 5. mutable-default-args (B006),
//!   6. builtin-shadowing (A001/A002), 7. placeholder-code,
//!   8. unreachable-code, 9. duplicate-keys, 10. test-conditional,
//!   11. fixed-wait, 12. mock-spec-bypass

use crate::config::{self, EckoConfig};
use crate::debug;
use crate::echo::{Echo, Severity};
use crate::fix;
use crate::lang::{self, Lang};
use crate::query_engine::{self, QueryCheck};
use std::collections::{HashMap, HashSet};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Tree};

// ---------------------------------------------------------------------------
// Embedded queries
// ---------------------------------------------------------------------------
const BARE_EXCEPT_QUERY: &str = include_str!("../../queries/python/bare_except.scm");
const STAR_IMPORTS_QUERY: &str = include_str!("../../queries/python/star_imports.scm");
const SINGLETON_CMP_QUERY: &str = include_str!("../../queries/python/singleton_comparison.scm");
const MUTABLE_DEFAULT_QUERY: &str = include_str!("../../queries/python/mutable_default.scm");
const IMPORT_ALL_QUERY: &str = include_str!("../../queries/python/import_all.scm");
const DICT_QUERY: &str = include_str!("../../queries/python/dict_pairs.scm");
const CALL_EXPR_QUERY: &str = include_str!("../../queries/python/call_expr.scm");
const ASSIGNMENT_QUERY: &str = include_str!("../../queries/python/assignment.scm");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const PYTHON_BUILTINS: &[&str] = &[
    "abs",
    "aiter",
    "all",
    "anext",
    "any",
    "ascii",
    "bin",
    "bool",
    "breakpoint",
    "bytearray",
    "bytes",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "copyright",
    "credits",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "exec",
    "exit",
    "filter",
    "float",
    "format",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "help",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "license",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "print",
    "property",
    "quit",
    "range",
    "repr",
    "reversed",
    "round",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
];
const ALLOWED_MOCK_ATTRS: &[&str] = &[
    "return_value",
    "side_effect",
    "assert_called",
    "assert_called_once",
    "assert_called_with",
    "assert_called_once_with",
    "assert_any_call",
    "assert_has_calls",
    "assert_not_called",
    "call_args",
    "call_args_list",
    "call_count",
    "called",
    "mock_calls",
    "reset_mock",
    "configure_mock",
];
const SKIP_DECORATORS: &[&str] = &["abstractmethod", "overload"];
const MOCK_CLASSES: &[&str] = &["Mock", "MagicMock"];
const TERMINAL_KINDS: &[&str] = &[
    "return_statement",
    "raise_statement",
    "break_statement",
    "continue_statement",
];

// ===========================================================================
// Node helpers
// ===========================================================================
fn ntext<'a>(node: Node<'a>, source: &'a [u8]) -> &'a str {
    query_engine::node_text(node, source)
}
fn ntext_owned(node: Node, source: &[u8]) -> String {
    query_engine::node_text(node, source).to_string()
}
fn ntext_eq(node: Node, source: &[u8], expected: &str) -> bool {
    ntext(node, source) == expected
}
fn find_child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            if c.node().kind() == kind {
                return Some(c.node());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    None
}
fn has_child_of_kind(node: Node, kind: &str) -> bool {
    find_child_of_kind(node, kind).is_some()
}
fn first_named_child(node: Node) -> Option<Node> {
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            if c.node().is_named() {
                return Some(c.node());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    None
}
fn named_children(node: Node) -> Vec<Node> {
    let mut r = Vec::new();
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            if c.node().is_named() {
                r.push(c.node());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    r
}
fn first_identifier(node: Node) -> Option<Node> {
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            if c.node().kind() == "identifier" {
                return Some(c.node());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    None
}
fn last_identifier(node: Node) -> Option<Node> {
    let mut c = node.walk();
    let mut last = None;
    if c.goto_first_child() {
        loop {
            if c.node().kind() == "identifier" {
                last = Some(c.node());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    last
}
fn contains_node_kind(node: Node, kind: &str) -> bool {
    let mut c = node.walk();
    let mut d = true;
    loop {
        if d && c.node().kind() == kind {
            return true;
        }
        if d && c.goto_first_child() {
            d = true;
            continue;
        }
        d = false;
        if c.goto_next_sibling() {
            d = true;
            continue;
        }
        if !c.goto_parent() {
            break;
        }
    }
    false
}
fn is_in_ranges(pos: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|(s, e)| pos >= *s && pos < *e)
}

// ===========================================================================
// Public entry point
// ===========================================================================
pub fn run_checks(file_path: &str, source: &str, config: &EckoConfig) -> Vec<Echo> {
    let (ts_lang, tree) = match lang::parse_for_checks(Lang::Python, source) {
        Some(v) => v,
        None => return Vec::new(),
    };
    let src = source.as_bytes();
    let is_test = lang::is_test_file(file_path);
    let shadow_al = config::get_builtin_shadow_allowlist(config);
    let mut echoes = Vec::new();
    echoes.extend(check_bare_except(&ts_lang, &tree, src));
    echoes.extend(check_star_imports(&ts_lang, &tree, src));
    echoes.extend(check_unused_imports(&ts_lang, &tree, source, src));
    echoes.extend(check_singleton_comparison(&ts_lang, &tree, src));
    echoes.extend(check_mutable_defaults(&ts_lang, &tree, src));
    echoes.extend(check_builtin_shadowing(&ts_lang, &tree, src, &shadow_al));
    if !is_test {
        echoes.extend(check_placeholder_code(&tree, src));
    }
    echoes.extend(check_unreachable_code(&tree, src));
    echoes.extend(check_duplicate_keys(&ts_lang, &tree, src));
    if is_test {
        echoes.extend(check_test_conditional(&tree, src));
        echoes.extend(check_fixed_wait(&ts_lang, &tree, src));
        echoes.extend(check_mock_spec_bypass(&tree, src));
    }
    echoes
}

// ===========================================================================
// 1. bare-except
// ===========================================================================
fn check_bare_except(ts_lang: &tree_sitter::Language, tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, BARE_EXCEPT_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("bare-except query: {e}"));
            return Vec::new();
        }
    };
    let ci = query_engine::capture_index_or_skip(&query, "match");
    let source_str = std::str::from_utf8(source).unwrap_or("");
    let mut echoes = Vec::new();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if cap.index as usize != ci {
                continue;
            }
            let text = ntext(cap.node, source);
            let after = text.strip_prefix("except").unwrap_or("").trim_start();
            if after.starts_with(':') {
                let auto_fix =
                    fix::fix_bare_except(cap.node.start_byte(), cap.node.end_byte(), source_str);
                echoes.push(Echo {
                    check: "bare-except".into(),
                    line: cap.node.start_position().row + 1,
                    message: "bare except \u{2014} catch a specific exception type".into(),
                    suggestion: String::new(),
                    severity: Severity::Error,
                    fix: auto_fix,
                });
            }
        }
    }
    echoes
}

// ===========================================================================
// 2. star-imports
// ===========================================================================
fn check_star_imports(ts_lang: &tree_sitter::Language, tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, STAR_IMPORTS_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("star-imports query: {e}"));
            return Vec::new();
        }
    };
    query_engine::run_query(
        &QueryCheck {
            name: "star-imports".into(),
            query,
            message: "wildcard import \u{2014} import specific names instead".into(),
            severity: Severity::Error,
            capture_name: "match".into(),
        },
        tree,
        source,
    )
}

// ===========================================================================
// 3. unused-imports
// ===========================================================================
struct ImportedName {
    local_name: String,
    line: usize,
    stmt_start: usize,
    stmt_end: usize,
}

fn check_unused_imports(
    ts_lang: &tree_sitter::Language,
    tree: &Tree,
    src_str: &str,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, IMPORT_ALL_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("import query: {e}"));
            return Vec::new();
        }
    };
    if src_str.contains("__all__") {
        return Vec::new();
    }
    let tc = collect_tc_ranges(tree.root_node(), source);
    let ci = query_engine::capture_index_or_skip(&query, "match");
    let mut imports: Vec<ImportedName> = Vec::new();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if cap.index as usize != ci {
                continue;
            }
            if is_in_ranges(cap.node.start_byte(), &tc) {
                continue;
            }
            if has_child_of_kind(cap.node, "wildcard_import") {
                continue;
            }
            extract_imports(cap.node, source, &mut imports);
        }
    }
    if imports.is_empty() {
        return Vec::new();
    }

    // Determine which imports are unused
    let unused: Vec<bool> = imports
        .iter()
        .map(|imp| !name_used(src_str, &imp.local_name, imp.line))
        .collect();

    // Count total names and unused names per statement (by start byte)
    let mut stmt_total: HashMap<usize, usize> = HashMap::new();
    let mut stmt_unused: HashMap<usize, usize> = HashMap::new();
    for (i, imp) in imports.iter().enumerate() {
        *stmt_total.entry(imp.stmt_start).or_insert(0) += 1;
        if unused[i] {
            *stmt_unused.entry(imp.stmt_start).or_insert(0) += 1;
        }
    }

    let mut echoes = Vec::new();
    for (i, imp) in imports.iter().enumerate() {
        if unused[i] {
            // Only attach fix when ALL names from the statement are unused
            // (safe to delete the entire line)
            let all_unused = stmt_total.get(&imp.stmt_start) == stmt_unused.get(&imp.stmt_start);
            let auto_fix = if all_unused {
                // Include trailing newline in the deletion range
                let end = if src_str.as_bytes().get(imp.stmt_end) == Some(&b'\n') {
                    imp.stmt_end + 1
                } else {
                    imp.stmt_end
                };
                fix::fix_unused_import_line(imp.stmt_start, end)
            } else {
                None
            };
            echoes.push(Echo {
                check: "unused-imports".into(),
                line: imp.line,
                message: format!("`{}` imported but unused", imp.local_name),
                suggestion: "Remove the unused import.".into(),
                severity: Severity::Warn,
                fix: auto_fix,
            });
        }
    }
    echoes
}

fn collect_tc_ranges(root: Node, source: &[u8]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut c = root.walk();
    let mut d = true;
    loop {
        if d && c.node().kind() == "if_statement" {
            if let Some(cond) = c.node().child_by_field_name("condition") {
                if cond.kind() == "identifier" && ntext_eq(cond, source, "TYPE_CHECKING") {
                    ranges.push((c.node().start_byte(), c.node().end_byte()));
                }
            }
        }
        if d && c.goto_first_child() {
            d = true;
            continue;
        }
        d = false;
        if c.goto_next_sibling() {
            d = true;
            continue;
        }
        if !c.goto_parent() {
            break;
        }
    }
    ranges
}

fn extract_imports(node: Node, source: &[u8], out: &mut Vec<ImportedName>) {
    let line = node.start_position().row + 1;
    let stmt_start = node.start_byte();
    let stmt_end = node.end_byte();
    if node.kind() == "import_statement" {
        let mut c = node.walk();
        if c.goto_first_child() {
            loop {
                match c.node().kind() {
                    "dotted_name" => {
                        if let Some(f) = first_identifier(c.node()) {
                            out.push(ImportedName {
                                local_name: ntext_owned(f, source),
                                line,
                                stmt_start,
                                stmt_end,
                            });
                        }
                    }
                    "aliased_import" => {
                        if let Some(a) = alias_name(c.node(), source) {
                            out.push(ImportedName {
                                local_name: a,
                                line,
                                stmt_start,
                                stmt_end,
                            });
                        }
                    }
                    _ => {}
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
    } else if node.kind() == "import_from_statement" {
        let mut c = node.walk();
        let mut past = false;
        if c.goto_first_child() {
            loop {
                let ch = c.node();
                if ch.kind() == "import" {
                    past = true;
                } else if past {
                    match ch.kind() {
                        "dotted_name" => {
                            if let Some(l) = last_identifier(ch) {
                                out.push(ImportedName {
                                    local_name: ntext_owned(l, source),
                                    line,
                                    stmt_start,
                                    stmt_end,
                                });
                            }
                        }
                        "aliased_import" => {
                            if let Some(a) = alias_name(ch, source) {
                                out.push(ImportedName {
                                    local_name: a,
                                    line,
                                    stmt_start,
                                    stmt_end,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

fn alias_name(node: Node, source: &[u8]) -> Option<String> {
    let mut c = node.walk();
    let mut found = false;
    if c.goto_first_child() {
        loop {
            if c.node().kind() == "as" {
                found = true;
            } else if found && c.node().kind() == "identifier" {
                return Some(ntext_owned(c.node(), source));
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

fn name_used(source: &str, name: &str, import_line: usize) -> bool {
    for (i, line) in source.lines().enumerate() {
        if i + 1 == import_line {
            continue;
        }
        if line_has_word(line, name) {
            return true;
        }
    }
    false
}

fn line_has_word(line: &str, name: &str) -> bool {
    let b = line.as_bytes();
    let nb = name.as_bytes();
    let nl = nb.len();
    if nl == 0 || b.len() < nl {
        return false;
    }
    let mut pos = 0;
    while pos + nl <= b.len() {
        if let Some(idx) = b[pos..].windows(nl).position(|w| w == nb).map(|p| p + pos) {
            let before = idx == 0 || !is_id(b[idx - 1]);
            let after = idx + nl >= b.len() || !is_id(b[idx + nl]);
            if before && after {
                return true;
            }
            pos = idx + 1;
        } else {
            break;
        }
    }
    false
}
fn is_id(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ===========================================================================
// 4. singleton-comparison
// ===========================================================================
fn check_singleton_comparison(
    ts_lang: &tree_sitter::Language,
    tree: &Tree,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, SINGLETON_CMP_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("singleton query: {e}"));
            return Vec::new();
        }
    };
    let ci = query_engine::capture_index_or_skip(&query, "match");
    let source_str = std::str::from_utf8(source).unwrap_or("");
    let mut echoes = Vec::new();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if cap.index as usize != ci {
                continue;
            }
            if let Some(e) = check_cmp(cap.node, source, source_str) {
                echoes.push(e);
            }
        }
    }
    echoes
}

fn check_cmp(node: Node, source: &[u8], source_str: &str) -> Option<Echo> {
    let cnt = node.child_count();
    if cnt < 3 {
        return None;
    }
    let mut i = 0;
    while i + 2 < cnt {
        let left = node.child(i)?;
        let op = node.child(i + 1)?;
        let right = node.child(i + 2)?;
        let ot = ntext(op, source);
        if ot == "==" || ot == "!=" {
            let s = singleton_kind(right).or_else(|| singleton_kind(left));
            if let Some(sk) = s {
                let sug = match (ot, sk) {
                    ("==", "None") => "Use `is None` instead of `== None`.",
                    ("!=", "None") => "Use `is not None` instead of `!= None`.",
                    ("==", "True") => "Use `is True` instead of `== True`.",
                    ("!=", "True") => "Use `is not True` instead of `!= True`.",
                    ("==", "False") => "Use `is False` instead of `== False`.",
                    ("!=", "False") => "Use `is not False` instead of `!= False`.",
                    _ => "Use identity comparison (is/is not) for singletons.",
                };
                let auto_fix =
                    fix::fix_singleton_comparison(node.start_byte(), node.end_byte(), source_str);
                return Some(Echo {
                    check: "singleton-comparison".into(),
                    line: node.start_position().row + 1,
                    message: format!("Comparison `{ot} {sk}` \u{2014} use identity check instead"),
                    suggestion: sug.into(),
                    severity: Severity::Warn,
                    fix: auto_fix,
                });
            }
        }
        i += 2;
    }
    None
}
fn singleton_kind(node: Node) -> Option<&'static str> {
    match node.kind() {
        "none" => Some("None"),
        "true" => Some("True"),
        "false" => Some("False"),
        _ => None,
    }
}

// ===========================================================================
// 5. mutable-default-args
// ===========================================================================
fn check_mutable_defaults(
    ts_lang: &tree_sitter::Language,
    tree: &Tree,
    source: &[u8],
) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, MUTABLE_DEFAULT_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("mutable query: {e}"));
            return Vec::new();
        }
    };
    let ns = query.capture_names();
    let mi = ns.iter().position(|n| *n == "match");
    let cli = ns.iter().position(|n| *n == "call_match");
    let mut echoes = Vec::new();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            let idx = cap.index as usize;
            let node = cap.node;
            if Some(idx) == mi {
                let k = match node.kind() {
                    "list" => "list",
                    "dictionary" => "dict",
                    "set" => "set",
                    _ => "mutable",
                };
                echoes.push(Echo {
                    check: "mutable-default-args".into(), line: node.start_position().row+1,
                    message: format!("Mutable default argument ({k} literal) \u{2014} use None and assign in body"),
                    suggestion: "Use `None` as default, then `arg = arg or {{}}` in the function body.".into(),
                    severity: Severity::Warn, fix: None,
                });
            } else if Some(idx) == cli {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "identifier" {
                        let n = ntext(func, source);
                        if n == "set" || n == "dict" || n == "list" {
                            echoes.push(Echo {
                                check: "mutable-default-args".into(), line: node.start_position().row+1,
                                message: format!("Mutable default argument ({n}()) \u{2014} use None and assign in body"),
                                suggestion: "Use `None` as default, then `arg = arg or {{}}` in the function body.".into(),
                                severity: Severity::Warn, fix: None,
                            });
                        }
                    }
                }
            }
        }
    }
    echoes
}

// ===========================================================================
// 6. builtin-shadowing
// ===========================================================================
fn check_builtin_shadowing(
    ts_lang: &tree_sitter::Language,
    tree: &Tree,
    source: &[u8],
    allowlist: &HashSet<String>,
) -> Vec<Echo> {
    let builtins: HashSet<&str> = PYTHON_BUILTINS.iter().copied().collect();
    let mut echoes = Vec::new();
    walk_funcs(tree.root_node(), &mut |func: Node| {
        if let Some(params) = func.child_by_field_name("parameters") {
            check_params_shadow(params, source, &builtins, allowlist, &mut echoes);
        }
    });
    check_assign_shadow(ts_lang, tree, source, &builtins, allowlist, &mut echoes);
    echoes
}

fn walk_funcs<F: FnMut(Node)>(root: Node, cb: &mut F) {
    let mut c = root.walk();
    let mut d = true;
    loop {
        let cur = c.node();
        if d {
            match cur.kind() {
                "function_definition" => cb(cur),
                "decorated_definition" => {
                    if let Some(f) = find_child_of_kind(cur, "function_definition") {
                        cb(f);
                    }
                }
                _ => {}
            }
        }
        if d && c.goto_first_child() {
            d = true;
            continue;
        }
        d = false;
        if c.goto_next_sibling() {
            d = true;
            continue;
        }
        if !c.goto_parent() {
            break;
        }
    }
}

fn check_params_shadow(
    params: Node,
    source: &[u8],
    builtins: &HashSet<&str>,
    al: &HashSet<String>,
    echoes: &mut Vec<Echo>,
) {
    let mut c = params.walk();
    if c.goto_first_child() {
        loop {
            let ch = c.node();
            let name = match ch.kind() {
                "identifier" => Some(ntext_owned(ch, source)),
                "default_parameter" | "typed_parameter" | "typed_default_parameter" => ch
                    .child_by_field_name("name")
                    .map(|n| ntext_owned(n, source)),
                "list_splat_pattern" | "dictionary_splat_pattern" => {
                    first_identifier(ch).map(|n| ntext_owned(n, source))
                }
                _ => None,
            };
            if let Some(name) = name {
                if !(name.starts_with("__") && name.ends_with("__"))
                    && builtins.contains(name.as_str())
                    && !al.contains(&name)
                {
                    echoes.push(Echo {
                        check: "builtin-shadowing".into(),
                        line: ch.start_position().row + 1,
                        message: format!("Parameter `{name}` shadows a Python builtin"),
                        suggestion: "Rename the parameter to avoid shadowing.".into(),
                        severity: Severity::Warn,
                        fix: None,
                    });
                }
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
}

fn check_assign_shadow(
    ts_lang: &tree_sitter::Language,
    tree: &Tree,
    source: &[u8],
    builtins: &HashSet<&str>,
    al: &HashSet<String>,
    echoes: &mut Vec<Echo>,
) {
    let query = match query_engine::compile_query(ts_lang, ASSIGNMENT_QUERY) {
        Ok(q) => q,
        Err(_) => return,
    };
    let ci = query_engine::capture_index_or_skip(&query, "match");
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if cap.index as usize != ci {
                continue;
            }
            if let Some(t) = cap.node.child_by_field_name("left") {
                if t.kind() == "identifier" {
                    let name = ntext(t, source);
                    if !(name.starts_with("__") && name.ends_with("__"))
                        && builtins.contains(name)
                        && !al.contains(name)
                    {
                        echoes.push(Echo {
                            check: "builtin-shadowing".into(),
                            line: t.start_position().row + 1,
                            message: format!("Variable `{name}` shadows a Python builtin"),
                            suggestion: "Rename the variable to avoid shadowing.".into(),
                            severity: Severity::Warn,
                            fix: None,
                        });
                    }
                }
            }
        }
    }
}

// ===========================================================================
// 7. placeholder-code
// ===========================================================================
fn check_placeholder_code(tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let mut echoes = Vec::new();
    let root = tree.root_node();
    for i in 0..root.child_count() {
        let ch = match root.child(i) {
            Some(c) => c,
            None => continue,
        };
        match ch.kind() {
            "function_definition" => {
                if let Some(e) = check_func_ph(ch, source, false) {
                    echoes.push(e);
                }
            }
            "decorated_definition" => {
                if !has_skip_dec(ch, source) {
                    if let Some(f) = find_child_of_kind(ch, "function_definition") {
                        if let Some(e) = check_func_ph(f, source, false) {
                            echoes.push(e);
                        }
                    }
                }
            }
            "class_definition" => {
                let proto = is_protocol_cls(ch, source);
                if let Some(body) = ch.child_by_field_name("body") {
                    for j in 0..body.child_count() {
                        if let Some(mem) = body.child(j) {
                            match mem.kind() {
                                "function_definition" => {
                                    if let Some(e) = check_func_ph(mem, source, proto) {
                                        echoes.push(e);
                                    }
                                }
                                "decorated_definition" => {
                                    if !has_skip_dec(mem, source) {
                                        if let Some(f) =
                                            find_child_of_kind(mem, "function_definition")
                                        {
                                            if let Some(e) = check_func_ph(f, source, proto) {
                                                echoes.push(e);
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
            _ => {}
        }
    }
    echoes
}

fn check_func_ph(func: Node, source: &[u8], is_proto: bool) -> Option<Echo> {
    if is_proto {
        return None;
    }
    let name = ntext(func.child_by_field_name("name")?, source);
    if name.starts_with("__") && name.ends_with("__") {
        return None;
    }
    let body = func.child_by_field_name("body")?;
    let mut stmts: Vec<Node> = Vec::new();
    for i in 0..body.child_count() {
        if let Some(c) = body.child(i) {
            if c.is_named() && !is_docstr(c) {
                stmts.push(c);
            }
        }
    }
    if stmts.len() != 1 {
        return None;
    }
    let s = stmts[0];
    let kind = match s.kind() {
        "pass_statement" => Some("pass"),
        "expression_statement" => first_named_child(s).and_then(|e| {
            if e.kind() == "ellipsis" {
                Some("...")
            } else {
                None
            }
        }),
        "raise_statement" => {
            if is_raise_nie(s, source) {
                Some("raise NotImplementedError")
            } else {
                None
            }
        }
        _ => None,
    };
    kind.map(|k| Echo {
        check: "placeholder-code".into(),
        line: func.start_position().row + 1,
        message: format!("Placeholder function body ({k})."),
        suggestion: "Implement the function or mark it @abstractmethod.".into(),
        severity: Severity::Warn,
        fix: None,
    })
}

fn is_docstr(node: Node) -> bool {
    node.kind() == "expression_statement"
        && first_named_child(node)
            .is_some_and(|c| c.kind() == "string" || c.kind() == "concatenated_string")
}

fn is_raise_nie(node: Node, source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        if let Some(ch) = node.child(i) {
            match ch.kind() {
                "identifier" if ntext_eq(ch, source, "NotImplementedError") => return true,
                "call" => {
                    if let Some(f) = ch.child_by_field_name("function") {
                        if f.kind() == "identifier" && ntext_eq(f, source, "NotImplementedError") {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    false
}

fn has_skip_dec(decorated: Node, source: &[u8]) -> bool {
    let mut c = decorated.walk();
    if c.goto_first_child() {
        loop {
            if c.node().kind() == "decorator" {
                for j in 0..c.node().child_count() {
                    if let Some(expr) = c.node().child(j) {
                        let dn = match expr.kind() {
                            "identifier" => Some(ntext(expr, source)),
                            "attribute" => expr
                                .child_by_field_name("attribute")
                                .map(|a| ntext(a, source)),
                            _ => None,
                        };
                        if let Some(n) = dn {
                            if SKIP_DECORATORS.contains(&n) {
                                return true;
                            }
                        }
                    }
                }
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

fn is_protocol_cls(cls: Node, source: &[u8]) -> bool {
    if let Some(supers) = cls.child_by_field_name("superclasses") {
        let mut c = supers.walk();
        if c.goto_first_child() {
            loop {
                match c.node().kind() {
                    "identifier" if ntext_eq(c.node(), source, "Protocol") => return true,
                    "attribute" => {
                        if let Some(a) = c.node().child_by_field_name("attribute") {
                            if ntext_eq(a, source, "Protocol") {
                                return true;
                            }
                        }
                    }
                    _ => {}
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    false
}

// ===========================================================================
// 8. unreachable-code
// ===========================================================================
fn check_unreachable_code(tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let mut echoes = Vec::new();
    walk_unreach(tree.root_node(), source, &mut echoes, None);
    echoes
}

fn walk_unreach<'a>(node: Node<'a>, source: &[u8], echoes: &mut Vec<Echo>, ef: Option<Node<'a>>) {
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            let ch = c.node();
            let func = if ch.kind() == "function_definition" {
                Some(ch)
            } else {
                ef
            };
            check_bodies(ch, source, echoes, func);
            walk_unreach(ch, source, echoes, func);
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
}

fn check_bodies<'a>(node: Node<'a>, source: &[u8], echoes: &mut Vec<Echo>, ef: Option<Node<'a>>) {
    if let Some(body) = node.child_by_field_name("body") {
        if body.kind() == "block" {
            check_block_unreach(body, source, echoes, ef);
        }
    }
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            match c.node().kind() {
                "else_clause" | "elif_clause" | "except_clause" | "finally_clause" => {
                    for j in 0..c.node().child_count() {
                        if let Some(b) = c.node().child(j) {
                            if b.kind() == "block" {
                                check_block_unreach(b, source, echoes, ef);
                            }
                        }
                    }
                }
                _ => {}
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
}

fn check_block_unreach<'a>(
    block: Node<'a>,
    source: &[u8],
    echoes: &mut Vec<Echo>,
    ef: Option<Node<'a>>,
) {
    let mut found = false;
    for i in 0..block.child_count() {
        let stmt = match block.child(i) {
            Some(s) => s,
            None => continue,
        };
        if !stmt.is_named() {
            continue;
        }
        if found {
            if is_yield_stmt(stmt) {
                if let Some(func) = ef {
                    if is_gen(func) || has_cm_dec(func, source) {
                        found = false;
                        continue;
                    }
                }
            }
            echoes.push(Echo {
                check: "unreachable-code".into(),
                line: stmt.start_position().row + 1,
                message: "Unreachable code after return/raise/break/continue.".into(),
                suggestion: "Remove the unreachable statement.".into(),
                severity: Severity::Error,
                fix: None,
            });
            break;
        }
        if TERMINAL_KINDS.contains(&stmt.kind()) {
            found = true;
        }
    }
}

fn is_yield_stmt(node: Node) -> bool {
    node.kind() == "expression_statement"
        && (0..node.child_count()).any(|i| node.child(i).is_some_and(|c| c.kind() == "yield"))
}
fn is_gen(func: Node) -> bool {
    contains_node_kind(func, "yield")
}

fn has_cm_dec(func: Node, source: &[u8]) -> bool {
    let target = if func.kind() == "function_definition" {
        func.parent()
            .filter(|p| p.kind() == "decorated_definition")
            .unwrap_or(func)
    } else {
        func
    };
    let mut c = target.walk();
    if c.goto_first_child() {
        loop {
            if c.node().kind() == "decorator" {
                for j in 0..c.node().child_count() {
                    if let Some(expr) = c.node().child(j) {
                        let n = match expr.kind() {
                            "identifier" => Some(ntext(expr, source)),
                            "attribute" => expr
                                .child_by_field_name("attribute")
                                .map(|a| ntext(a, source)),
                            _ => None,
                        };
                        if let Some(n) = n {
                            if n == "contextmanager" || n == "asynccontextmanager" {
                                return true;
                            }
                        }
                    }
                }
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

// ===========================================================================
// 9. duplicate-keys
// ===========================================================================
fn check_duplicate_keys(ts_lang: &tree_sitter::Language, tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, DICT_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("dict query: {e}"));
            return Vec::new();
        }
    };
    let ci = query_engine::capture_index_or_skip(&query, "match");
    let mut echoes = Vec::new();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if cap.index as usize != ci {
                continue;
            }
            check_dict_dups(cap.node, source, &mut echoes);
        }
    }
    echoes
}

fn check_dict_dups(dict: Node, source: &[u8], echoes: &mut Vec<Echo>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut c = dict.walk();
    if c.goto_first_child() {
        loop {
            if c.node().kind() == "pair" {
                if let Some(key) = c.node().child_by_field_name("key") {
                    if let Some(repr) = const_key(key, source) {
                        let line = key.start_position().row + 1;
                        match seen.entry(repr) {
                            std::collections::hash_map::Entry::Occupied(entry) => {
                                let repr = entry.key();
                                echoes.push(Echo {
                                    check: "duplicate-keys".into(),
                                    line,
                                    message: format!("Duplicate dictionary key `{repr}`."),
                                    suggestion: "Remove the duplicate or rename it.".into(),
                                    severity: Severity::Warn,
                                    fix: None,
                                });
                            }
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                entry.insert(line);
                            }
                        }
                    }
                }
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
}

fn const_key(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "string" | "integer" | "float" => Some(ntext_owned(node, source)),
        "none" => Some("None".into()),
        "true" => Some("True".into()),
        "false" => Some("False".into()),
        _ => None,
    }
}

// ===========================================================================
// 10. test-conditional
// ===========================================================================
fn check_test_conditional(tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let mut echoes = Vec::new();
    iter_test_fns(tree.root_node(), source, &mut |func: Node| {
        check_conds(func, source, &mut echoes);
    });
    echoes
}

fn check_conds(func: Node, source: &[u8], echoes: &mut Vec<Echo>) {
    let body = match func.child_by_field_name("body") {
        Some(b) => b,
        None => return,
    };
    let mut q: Vec<(Node, bool)> = Vec::new();
    for i in 0..body.child_count() {
        if let Some(c) = body.child(i) {
            if c.is_named() {
                q.push((c, false));
            }
        }
    }
    while let Some((node, in_loop)) = q.pop() {
        match node.kind() {
            "function_definition" | "class_definition" | "decorated_definition" => continue,
            _ => {}
        }
        if node.kind() == "if_statement" && !is_guard(node, source) {
            if in_loop && !if_has_assert(node) {
                for i in 0..node.child_count() {
                    if let Some(c) = node.child(i) {
                        if c.is_named() {
                            q.push((c, in_loop));
                        }
                    }
                }
                continue;
            }
            echoes.push(Echo {
                check: "test-conditional".into(), line: node.start_position().row+1,
                message: "Conditional (if/else) in test function \u{2014} tests should control state, not branch on it.".into(),
                suggestion: "Parametrize or split into separate test cases.".into(),
                severity: Severity::Warn, fix: None,
            });
        }
        let il = in_loop || matches!(node.kind(), "for_statement" | "while_statement");
        for i in 0..node.child_count() {
            if let Some(c) = node.child(i) {
                if c.is_named() {
                    q.push((c, il));
                }
            }
        }
    }
}

fn is_guard(node: Node, source: &[u8]) -> bool {
    if let Some(cond) = node.child_by_field_name("condition") {
        if cond.kind() == "comparison_operator" {
            if let Some(left) = cond.child(0) {
                if left.kind() == "identifier" && ntext_eq(left, source, "__name__") {
                    return true;
                }
                if is_plat(left, source) {
                    return true;
                }
            }
        }
        if cond.kind() == "identifier" && ntext_eq(cond, source, "TYPE_CHECKING") {
            return true;
        }
        if cond.kind() == "attribute" && is_plat(cond, source) {
            return true;
        }
    }
    if !has_else(node) {
        if let Some(body) = first_block(node) {
            let stmts = named_children(body);
            if stmts.len() == 1 {
                let s = stmts[0];
                if s.kind() == "return_statement" {
                    return true;
                }
                if s.kind() == "raise_statement" && has_pytest_ch(s, source, "skip") {
                    return true;
                }
                if s.kind() == "expression_statement" {
                    if let Some(call) = first_named_child(s) {
                        if call.kind() == "call"
                            && (is_pytest_m(call, source, "skip")
                                || is_pytest_m(call, source, "fail")
                                || is_skip_test(call, source))
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn is_plat(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "attribute" => {
            if let Some(a) = node.child_by_field_name("attribute") {
                let t = ntext(a, source);
                if t == "version_info" || t == "platform" {
                    return true;
                }
                if t == "name" {
                    if let Some(o) = node.child_by_field_name("object") {
                        if o.kind() == "identifier" && ntext_eq(o, source, "os") {
                            return true;
                        }
                    }
                }
            }
            false
        }
        "subscript" => node
            .child_by_field_name("value")
            .is_some_and(|v| is_plat(v, source)),
        _ => false,
    }
}

fn has_else(node: Node) -> bool {
    (0..node.child_count()).any(|i| {
        node.child(i)
            .is_some_and(|c| c.kind() == "else_clause" || c.kind() == "elif_clause")
    })
}
fn first_block(node: Node) -> Option<Node> {
    (0..node.child_count()).find_map(|i| node.child(i).filter(|c| c.kind() == "block"))
}
fn has_pytest_ch(node: Node, source: &[u8], method: &str) -> bool {
    (0..node.child_count()).any(|i| {
        node.child(i)
            .is_some_and(|c| c.kind() == "call" && is_pytest_m(c, source, method))
    })
}
fn is_pytest_m(call: Node, source: &[u8], method: &str) -> bool {
    if let Some(f) = call.child_by_field_name("function") {
        if f.kind() == "attribute" {
            if let (Some(o), Some(a)) = (
                f.child_by_field_name("object"),
                f.child_by_field_name("attribute"),
            ) {
                return o.kind() == "identifier"
                    && ntext_eq(o, source, "pytest")
                    && ntext_eq(a, source, method);
            }
        }
    }
    false
}
fn is_skip_test(call: Node, source: &[u8]) -> bool {
    call.child_by_field_name("function")
        .and_then(|f| {
            if f.kind() == "attribute" {
                f.child_by_field_name("attribute")
            } else {
                None
            }
        })
        .is_some_and(|a| ntext_eq(a, source, "skipTest"))
}
fn if_has_assert(node: Node) -> bool {
    let mut q: Vec<Node> = Vec::new();
    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) {
            q.push(c);
        }
    }
    while let Some(cur) = q.pop() {
        if cur.kind() == "assert_statement" {
            return true;
        }
        if matches!(
            cur.kind(),
            "function_definition" | "class_definition" | "decorated_definition"
        ) {
            continue;
        }
        for i in 0..cur.child_count() {
            if let Some(c) = cur.child(i) {
                q.push(c);
            }
        }
    }
    false
}

fn iter_test_fns<F: FnMut(Node)>(root: Node, source: &[u8], cb: &mut F) {
    for i in 0..root.child_count() {
        let ch = match root.child(i) {
            Some(c) => c,
            None => continue,
        };
        match ch.kind() {
            "function_definition" => {
                if is_test_fn(ch, source) {
                    cb(ch);
                }
            }
            "decorated_definition" => {
                if let Some(f) = find_child_of_kind(ch, "function_definition") {
                    if is_test_fn(f, source) {
                        cb(f);
                    }
                }
            }
            "class_definition" => {
                if let Some(body) = ch.child_by_field_name("body") {
                    for j in 0..body.child_count() {
                        if let Some(mem) = body.child(j) {
                            match mem.kind() {
                                "function_definition" => {
                                    if is_test_fn(mem, source) {
                                        cb(mem);
                                    }
                                }
                                "decorated_definition" => {
                                    if let Some(f) = find_child_of_kind(mem, "function_definition")
                                    {
                                        if is_test_fn(f, source) {
                                            cb(f);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
fn is_test_fn(func: Node, source: &[u8]) -> bool {
    func.child_by_field_name("name")
        .is_some_and(|n| ntext(n, source).starts_with("test_"))
}

// ===========================================================================
// 11. fixed-wait
// ===========================================================================
fn check_fixed_wait(ts_lang: &tree_sitter::Language, tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let query = match query_engine::compile_query(ts_lang, CALL_EXPR_QUERY) {
        Ok(q) => q,
        Err(e) => {
            debug::debug(&format!("call query: {e}"));
            return Vec::new();
        }
    };
    let ns = query.capture_names();
    let oi = ns.iter().position(|n| *n == "obj");
    let mi_cap = ns.iter().position(|n| *n == "method");
    let ai = ns.iter().position(|n| *n == "args");
    let mti = ns.iter().position(|n| *n == "match");
    let tr = collect_test_ranges(tree.root_node(), source);
    let mut echoes = Vec::new();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = matches.next() {
        let (mut obj, mut method) = ("", "");
        let mut args: Option<Node> = None;
        let (mut line, mut start) = (0usize, 0usize);
        for cap in m.captures {
            let idx = cap.index as usize;
            if Some(idx) == oi {
                obj = ntext(cap.node, source);
            } else if Some(idx) == mi_cap {
                method = ntext(cap.node, source);
            } else if Some(idx) == ai {
                args = Some(cap.node);
            } else if Some(idx) == mti {
                line = cap.node.start_position().row + 1;
                start = cap.node.start_byte();
            }
        }
        if !is_in_ranges(start, &tr) {
            continue;
        }
        let is_sl = method == "sleep" && (obj == "time" || obj == "asyncio");
        let is_wt = method == "wait_for_timeout";
        if !is_sl && !is_wt {
            continue;
        }
        if is_sl {
            if let Some(a) = args {
                if is_zero(a, source) {
                    continue;
                }
            }
        }
        let msg = if is_sl {
            format!("{obj}.sleep() in test \u{2014} fixed waits are flaky.")
        } else {
            "wait_for_timeout() in test \u{2014} fixed waits are flaky.".into()
        };
        echoes.push(Echo {
            check: "fixed-wait".into(),
            line,
            message: msg,
            suggestion: "Use polling, retry loops, or event-based assertions instead.".into(),
            severity: Severity::Warn,
            fix: None,
        });
    }
    echoes
}

fn collect_test_ranges(root: Node, source: &[u8]) -> Vec<(usize, usize)> {
    let mut r = Vec::new();
    iter_test_fns(root, source, &mut |f: Node| {
        r.push((f.start_byte(), f.end_byte()));
    });
    r
}

fn is_zero(args: Node, source: &[u8]) -> bool {
    let mut cnt = 0;
    let mut z = false;
    let mut c = args.walk();
    if c.goto_first_child() {
        loop {
            if c.node().is_named() {
                cnt += 1;
                if c.node().kind() == "integer" && ntext_eq(c.node(), source, "0") {
                    z = true;
                }
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    cnt == 1 && z
}

// ===========================================================================
// 12. mock-spec-bypass
// ===========================================================================
fn check_mock_spec_bypass(tree: &Tree, source: &[u8]) -> Vec<Echo> {
    let mut echoes = Vec::new();
    let allowed: HashSet<&str> = ALLOWED_MOCK_ATTRS.iter().copied().collect();
    iter_test_fns(tree.root_node(), source, &mut |func: Node| {
        check_mock_fn(func, source, &allowed, &mut echoes);
    });
    echoes
}

fn check_mock_fn(func: Node, source: &[u8], allowed: &HashSet<&str>, echoes: &mut Vec<Echo>) {
    let body = match func.child_by_field_name("body") {
        Some(b) => b,
        None => return,
    };
    let mut specs: HashSet<String> = HashSet::new();
    collect_specs(body, source, &mut specs);
    if specs.is_empty() {
        return;
    }
    find_mock_assigns(body, source, &specs, allowed, echoes);
}

fn collect_specs(body: Node, source: &[u8], specs: &mut HashSet<String>) {
    let mut q: Vec<Node> = Vec::new();
    for i in 0..body.child_count() {
        if let Some(c) = body.child(i) {
            if c.is_named() {
                q.push(c);
            }
        }
    }
    while let Some(node) = q.pop() {
        if matches!(
            node.kind(),
            "function_definition" | "class_definition" | "decorated_definition"
        ) {
            continue;
        }
        let assign = if node.kind() == "expression_statement" {
            first_named_child(node).filter(|c| c.kind() == "assignment")
        } else if node.kind() == "assignment" {
            Some(node)
        } else {
            None
        };
        if let Some(a) = assign {
            if let (Some(t), Some(v)) = (
                a.child_by_field_name("left"),
                a.child_by_field_name("right"),
            ) {
                if t.kind() == "identifier" && is_mock_spec(v, source) {
                    specs.insert(ntext_owned(t, source));
                }
            }
        }
        for i in 0..node.child_count() {
            if let Some(c) = node.child(i) {
                if c.is_named() {
                    q.push(c);
                }
            }
        }
    }
}

fn is_mock_spec(node: Node, source: &[u8]) -> bool {
    if node.kind() != "call" {
        return false;
    }
    let f = match node.child_by_field_name("function") {
        Some(f) => f,
        None => return false,
    };
    let im = match f.kind() {
        "identifier" => MOCK_CLASSES.contains(&ntext(f, source)),
        "attribute" => f
            .child_by_field_name("attribute")
            .is_some_and(|a| MOCK_CLASSES.contains(&ntext(a, source))),
        _ => false,
    };
    if !im {
        return false;
    }
    if let Some(args) = node.child_by_field_name("arguments") {
        let mut c = args.walk();
        if c.goto_first_child() {
            loop {
                if c.node().kind() == "keyword_argument" {
                    if let Some(n) = c.node().child_by_field_name("name") {
                        let t = ntext(n, source);
                        if t == "spec" || t == "spec_set" {
                            return true;
                        }
                    }
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    false
}

fn find_mock_assigns(
    body: Node,
    source: &[u8],
    specs: &HashSet<String>,
    allowed: &HashSet<&str>,
    echoes: &mut Vec<Echo>,
) {
    let mut q: Vec<Node> = Vec::new();
    for i in 0..body.child_count() {
        if let Some(c) = body.child(i) {
            if c.is_named() {
                q.push(c);
            }
        }
    }
    while let Some(node) = q.pop() {
        if matches!(
            node.kind(),
            "function_definition" | "class_definition" | "decorated_definition"
        ) {
            continue;
        }
        let assign = if node.kind() == "expression_statement" {
            first_named_child(node).filter(|c| c.kind() == "assignment")
        } else if node.kind() == "assignment" {
            Some(node)
        } else {
            None
        };
        if let Some(a) = assign {
            if let Some(t) = a.child_by_field_name("left") {
                if t.kind() == "attribute" {
                    if let (Some(o), Some(at)) = (
                        t.child_by_field_name("object"),
                        t.child_by_field_name("attribute"),
                    ) {
                        if o.kind() == "identifier" {
                            let on = ntext_owned(o, source);
                            let an = ntext(at, source);
                            if specs.contains(&on) && !allowed.contains(an) {
                                echoes.push(Echo {
                                    check: "mock-spec-bypass".into(), line: a.start_position().row+1,
                                    message: format!("Setting .{an} on a Mock(spec=...) bypasses spec validation."),
                                    suggestion: "Use configure_mock() or check if the attribute exists on the spec class.".into(),
                                    severity: Severity::Warn, fix: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        for i in 0..node.child_count() {
            if let Some(c) = node.child(i) {
                if c.is_named() {
                    q.push(c);
                }
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EckoConfig;
    fn cnt(e: &[Echo], c: &str) -> usize {
        e.iter().filter(|x| x.check == c).count()
    }
    fn cfg() -> EckoConfig {
        EckoConfig::default()
    }

    #[test]
    fn bare_except_yes() {
        assert!(
            cnt(
                &run_checks("t.py", "try:\n    pass\nexcept:\n    pass\n", &cfg()),
                "bare-except"
            ) > 0
        );
    }
    #[test]
    fn bare_except_no() {
        assert_eq!(
            cnt(
                &run_checks(
                    "t.py",
                    "try:\n    pass\nexcept ValueError:\n    pass\n",
                    &cfg()
                ),
                "bare-except"
            ),
            0
        );
    }
    #[test]
    fn star_import_yes() {
        assert!(
            cnt(
                &run_checks("t.py", "from os import *\n", &cfg()),
                "star-imports"
            ) > 0
        );
    }
    #[test]
    fn unused_import_yes() {
        assert!(
            cnt(
                &run_checks("f.py", "import os\n\nx = 1\n", &cfg()),
                "unused-imports"
            ) > 0
        );
    }
    #[test]
    fn used_import_no() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "import os\n\nos.path.join('a','b')\n", &cfg()),
                "unused-imports"
            ),
            0
        );
    }
    #[test]
    fn from_unused() {
        assert!(
            cnt(
                &run_checks("f.py", "from os import path\n\nx = 1\n", &cfg()),
                "unused-imports"
            ) > 0
        );
    }
    #[test]
    fn from_used() {
        assert_eq!(
            cnt(
                &run_checks(
                    "f.py",
                    "from os import path\n\nresult = path.join('a','b')\n",
                    &cfg()
                ),
                "unused-imports"
            ),
            0
        );
    }
    #[test]
    fn alias_unused() {
        assert!(
            cnt(
                &run_checks("f.py", "import os as op\n\nx = 1\n", &cfg()),
                "unused-imports"
            ) > 0
        );
    }
    #[test]
    fn alias_used() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "import os as op\n\nop.path.join('a','b')\n", &cfg()),
                "unused-imports"
            ),
            0
        );
    }
    #[test]
    fn all_reexport() {
        assert_eq!(
            cnt(
                &run_checks(
                    "f.py",
                    "from os import path\n\n__all__ = ['path']\n",
                    &cfg()
                ),
                "unused-imports"
            ),
            0
        );
    }
    #[test]
    fn tc_import() {
        assert_eq!(
            run_checks(
                "f.py",
                "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    import foo\n\nx = 1\n",
                &cfg()
            )
            .iter()
            .filter(|x| x.check == "unused-imports" && x.message.contains("foo"))
            .count(),
            0
        );
    }
    #[test]
    fn from_alias_used() {
        assert_eq!(
            cnt(
                &run_checks(
                    "f.py",
                    "from os import path as p\n\nresult = p.join('a','b')\n",
                    &cfg()
                ),
                "unused-imports"
            ),
            0
        );
    }
    #[test]
    fn multi_from() {
        let e = run_checks(
            "f.py",
            "from os import path, getcwd\n\nresult = path.join('a','b')\n",
            &cfg(),
        );
        assert!(e
            .iter()
            .any(|x| x.check == "unused-imports" && x.message.contains("getcwd")));
        assert!(!e
            .iter()
            .any(|x| x.check == "unused-imports" && x.message.contains("path")));
    }
    #[test]
    fn eq_none() {
        assert!(
            cnt(
                &run_checks("f.py", "x = 1\nif x == None:\n    pass\n", &cfg()),
                "singleton-comparison"
            ) > 0
        );
    }
    #[test]
    fn ne_true() {
        assert!(
            cnt(
                &run_checks("f.py", "if x != True:\n    pass\n", &cfg()),
                "singleton-comparison"
            ) > 0
        );
    }
    #[test]
    fn is_none_ok() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "if x is None:\n    pass\n", &cfg()),
                "singleton-comparison"
            ),
            0
        );
    }
    #[test]
    fn eq_false() {
        assert!(
            cnt(
                &run_checks("f.py", "if x == False:\n    pass\n", &cfg()),
                "singleton-comparison"
            ) > 0
        );
    }
    #[test]
    fn mut_list() {
        assert!(
            cnt(
                &run_checks("f.py", "def f(x=[]):\n    pass\n", &cfg()),
                "mutable-default-args"
            ) > 0
        );
    }
    #[test]
    fn mut_dict() {
        assert!(
            cnt(
                &run_checks("f.py", "def f(x={}):\n    pass\n", &cfg()),
                "mutable-default-args"
            ) > 0
        );
    }
    #[test]
    fn mut_set_call() {
        assert!(
            cnt(
                &run_checks("f.py", "def f(x=set()):\n    pass\n", &cfg()),
                "mutable-default-args"
            ) > 0
        );
    }
    #[test]
    fn mut_list_call() {
        assert!(
            cnt(
                &run_checks("f.py", "def f(x=list()):\n    pass\n", &cfg()),
                "mutable-default-args"
            ) > 0
        );
    }
    #[test]
    fn none_default() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "def f(x=None):\n    pass\n", &cfg()),
                "mutable-default-args"
            ),
            0
        );
    }
    #[test]
    fn typed_mut() {
        assert!(
            cnt(
                &run_checks("f.py", "def f(x: list = []):\n    pass\n", &cfg()),
                "mutable-default-args"
            ) > 0
        );
    }
    #[test]
    fn shadow_param() {
        assert!(
            cnt(
                &run_checks("f.py", "def f(object):\n    pass\n", &cfg()),
                "builtin-shadowing"
            ) > 0
        );
    }
    #[test]
    fn shadow_allow() {
        assert_eq!(
            run_checks("f.py", "def f(type):\n    pass\n", &cfg())
                .iter()
                .filter(|x| x.check == "builtin-shadowing" && x.message.contains("type"))
                .count(),
            0
        );
    }
    #[test]
    fn shadow_dunder() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "def f(__doc__):\n    pass\n", &cfg()),
                "builtin-shadowing"
            ),
            0
        );
    }
    #[test]
    fn shadow_assign() {
        assert!(
            cnt(
                &run_checks("f.py", "object = 42\n", &cfg()),
                "builtin-shadowing"
            ) > 0
        );
    }
    #[test]
    fn ph_pass() {
        assert!(
            cnt(
                &run_checks("f.py", "def f():\n    pass\n", &cfg()),
                "placeholder-code"
            ) > 0
        );
    }
    #[test]
    fn ph_ellipsis() {
        assert!(
            cnt(
                &run_checks("f.py", "def f():\n    ...\n", &cfg()),
                "placeholder-code"
            ) > 0
        );
    }
    #[test]
    fn ph_nie() {
        assert!(
            cnt(
                &run_checks(
                    "f.py",
                    "def f():\n    raise NotImplementedError()\n",
                    &cfg()
                ),
                "placeholder-code"
            ) > 0
        );
    }
    #[test]
    fn ph_nie_no_call() {
        assert!(
            cnt(
                &run_checks("f.py", "def f():\n    raise NotImplementedError\n", &cfg()),
                "placeholder-code"
            ) > 0
        );
    }
    #[test]
    fn ph_abstract() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "@abstractmethod\ndef f():\n    pass\n", &cfg()),
                "placeholder-code"
            ),
            0
        );
    }
    #[test]
    fn ph_dunder() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "def __init__(self):\n    pass\n", &cfg()),
                "placeholder-code"
            ),
            0
        );
    }
    #[test]
    fn ph_test_file() {
        assert_eq!(
            cnt(
                &run_checks("test_f.py", "def f():\n    pass\n", &cfg()),
                "placeholder-code"
            ),
            0
        );
    }
    #[test]
    fn ph_protocol() {
        assert_eq!(
            cnt(
                &run_checks(
                    "f.py",
                    "class F(Protocol):\n    def b(self):\n        ...\n",
                    &cfg()
                ),
                "placeholder-code"
            ),
            0
        );
    }
    #[test]
    fn ph_real() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "def f():\n    return 42\n", &cfg()),
                "placeholder-code"
            ),
            0
        );
    }
    #[test]
    fn ph_doc_pass() {
        assert!(
            cnt(
                &run_checks("f.py", "def f():\n    \"\"\"doc\"\"\"\n    pass\n", &cfg()),
                "placeholder-code"
            ) > 0
        );
    }
    #[test]
    fn unreach_ret() {
        assert!(
            cnt(
                &run_checks("f.py", "def f():\n    return 1\n    x = 2\n", &cfg()),
                "unreachable-code"
            ) > 0
        );
    }
    #[test]
    fn unreach_raise() {
        assert!(
            cnt(
                &run_checks(
                    "f.py",
                    "def f():\n    raise ValueError()\n    x = 2\n",
                    &cfg()
                ),
                "unreachable-code"
            ) > 0
        );
    }
    #[test]
    fn unreach_sev() {
        assert_eq!(
            run_checks("f.py", "def f():\n    return 1\n    x = 2\n", &cfg())
                .iter()
                .find(|x| x.check == "unreachable-code")
                .unwrap()
                .severity,
            Severity::Error
        );
    }
    #[test]
    fn no_unreach() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "def f():\n    x = 1\n    return x\n", &cfg()),
                "unreachable-code"
            ),
            0
        );
    }
    #[test]
    fn yield_cm() {
        assert_eq!(
            cnt(
                &run_checks(
                    "f.py",
                    "@contextmanager\ndef f():\n    raise ValueError()\n    yield\n",
                    &cfg()
                ),
                "unreachable-code"
            ),
            0
        );
    }
    #[test]
    fn unreach_break() {
        assert!(
            cnt(
                &run_checks("f.py", "for x in [1]:\n    break\n    print(x)\n", &cfg()),
                "unreachable-code"
            ) > 0
        );
    }
    #[test]
    fn unreach_cont() {
        assert!(
            cnt(
                &run_checks(
                    "f.py",
                    "for x in [1]:\n    continue\n    print(x)\n",
                    &cfg()
                ),
                "unreachable-code"
            ) > 0
        );
    }
    #[test]
    fn dup_key() {
        assert!(
            cnt(
                &run_checks("f.py", "d = {\"a\": 1, \"b\": 2, \"a\": 3}\n", &cfg()),
                "duplicate-keys"
            ) > 0
        );
    }
    #[test]
    fn no_dup() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "d = {\"a\": 1, \"b\": 2, \"c\": 3}\n", &cfg()),
                "duplicate-keys"
            ),
            0
        );
    }
    #[test]
    fn dup_int() {
        assert!(
            cnt(
                &run_checks("f.py", "d = {1: 'a', 2: 'b', 1: 'c'}\n", &cfg()),
                "duplicate-keys"
            ) > 0
        );
    }
    #[test]
    fn test_cond_yes() {
        assert!(
            cnt(
                &run_checks(
                    "test_f.py",
                    "def test_f():\n    if True:\n        pass\n",
                    &cfg()
                ),
                "test-conditional"
            ) > 0
        );
    }
    #[test]
    fn test_cond_guard() {
        assert_eq!(cnt(&run_checks("test_f.py","def test_f():\n    if sys.platform == 'win32':\n        return\n    assert True\n",&cfg()),"test-conditional"),0);
    }
    #[test]
    fn test_cond_non_test() {
        assert_eq!(
            cnt(
                &run_checks(
                    "f.py",
                    "def test_f():\n    if True:\n        pass\n",
                    &cfg()
                ),
                "test-conditional"
            ),
            0
        );
    }
    #[test]
    fn test_cond_helper() {
        assert_eq!(
            cnt(
                &run_checks(
                    "test_f.py",
                    "def helper():\n    if True:\n        pass\n",
                    &cfg()
                ),
                "test-conditional"
            ),
            0
        );
    }
    #[test]
    fn test_cond_loop() {
        assert_eq!(cnt(&run_checks("test_f.py","def test_f():\n    for x in items:\n        if x > 0:\n            result.append(x)\n",&cfg()),"test-conditional"),0);
    }
    #[test]
    fn wait_sleep() {
        assert!(
            cnt(
                &run_checks("test_f.py", "def test_f():\n    time.sleep(1)\n", &cfg()),
                "fixed-wait"
            ) > 0
        );
    }
    #[test]
    fn wait_asyncio() {
        assert!(
            cnt(
                &run_checks(
                    "test_f.py",
                    "def test_f():\n    asyncio.sleep(0.5)\n",
                    &cfg()
                ),
                "fixed-wait"
            ) > 0
        );
    }
    #[test]
    fn wait_zero() {
        assert_eq!(
            cnt(
                &run_checks("test_f.py", "def test_f():\n    time.sleep(0)\n", &cfg()),
                "fixed-wait"
            ),
            0
        );
    }
    #[test]
    fn wait_non_test() {
        assert_eq!(
            cnt(
                &run_checks("f.py", "def test_f():\n    time.sleep(1)\n", &cfg()),
                "fixed-wait"
            ),
            0
        );
    }
    #[test]
    fn mock_bypass_yes() {
        assert!(
            cnt(
                &run_checks(
                    "test_f.py",
                    "def test_f():\n    m = Mock(spec=Foo)\n    m.bar = 1\n",
                    &cfg()
                ),
                "mock-spec-bypass"
            ) > 0
        );
    }
    #[test]
    fn mock_bypass_ok() {
        assert_eq!(
            cnt(
                &run_checks(
                    "test_f.py",
                    "def test_f():\n    m = Mock(spec=Foo)\n    m.return_value = 1\n",
                    &cfg()
                ),
                "mock-spec-bypass"
            ),
            0
        );
    }
    #[test]
    fn mock_no_spec() {
        assert_eq!(
            cnt(
                &run_checks(
                    "test_f.py",
                    "def test_f():\n    m = Mock()\n    m.bar = 1\n",
                    &cfg()
                ),
                "mock-spec-bypass"
            ),
            0
        );
    }
    #[test]
    fn mock_non_test() {
        assert_eq!(
            cnt(
                &run_checks(
                    "f.py",
                    "def test_f():\n    m = Mock(spec=Foo)\n    m.bar = 1\n",
                    &cfg()
                ),
                "mock-spec-bypass"
            ),
            0
        );
    }
    #[test]
    fn magic_mock() {
        assert!(
            cnt(
                &run_checks(
                    "test_f.py",
                    "def test_f():\n    m = MagicMock(spec_set=Foo)\n    m.bar = 1\n",
                    &cfg()
                ),
                "mock-spec-bypass"
            ) > 0
        );
    }
}
