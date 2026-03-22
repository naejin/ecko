//! Universal checks -- apply to all file types.
//!
//! Checks: unicode-artifact, banned-pattern, import-layer.
//! These are language-agnostic and run on every file.

use std::collections::HashSet;
use std::path::Path;

use regex::Regex;

use crate::config::EckoConfig;
use crate::debug;
use crate::echo::{Echo, Severity};
use crate::lang::{self, Lang};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Unicode artifact characters that shouldn't appear in source code.
const ARTIFACTS: &[(char, &str)] = &[
    ('\u{2014}', "Em dash (\u{2014})"),
    ('\u{2013}', "En dash (\u{2013})"),
    ('\u{2018}', "Left single quote (\u{2018})"),
    ('\u{2019}', "Right single quote (\u{2019})"),
    ('\u{201c}', "Left double quote (\u{201c})"),
    ('\u{201d}', "Right double quote (\u{201d})"),
    ('\u{2026}', "Horizontal ellipsis (\u{2026})"),
    ('\u{2022}', "Bullet (\u{2022})"),
    ('\u{200b}', "Zero-width space"),
    ('\u{200c}', "Zero-width non-joiner"),
    ('\u{200d}', "Zero-width joiner"),
    ('\u{200e}', "Left-to-right mark"),
    ('\u{200f}', "Right-to-left mark"),
];

/// Prose extensions where em dashes, smart quotes, etc. are normal punctuation.
const PROSE_EXTENSIONS: &[&str] = &["md", "txt", "rst", "adoc", "rdoc"];

/// JS/TS family extensions -- use JS string/comment scanner.
const JS_EXTENSIONS: &[&str] = &[
    "js", "jsx", "ts", "tsx", "mjs", "cjs", "css", "json", "jsonc",
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run universal checks on the given file.
///
/// `cwd` is used for glob filtering and relative paths in banned patterns
/// and import layer rules.
pub fn run_checks(
    file_path: &str,
    source: &str,
    lang: Lang,
    config: &EckoConfig,
    cwd: &str,
) -> Vec<Echo> {
    let mut echoes = Vec::new();

    echoes.extend(check_unicode_artifacts(file_path, source, lang));
    echoes.extend(check_banned_patterns(
        file_path,
        source,
        &config.banned_patterns,
        cwd,
    ));
    echoes.extend(check_import_layers(
        file_path,
        source,
        lang,
        &config.import_rules,
        cwd,
    ));

    echoes
}

// ===========================================================================
// Unicode artifact check
// ===========================================================================

/// Scan file for unicode artifacts, skipping string literals and comments.
///
/// Uses tree-sitter to identify string/comment node byte ranges, then scans
/// for artifact characters outside those ranges. Falls back to heuristic
/// skipping when tree-sitter parse is unavailable.
pub fn check_unicode_artifacts(file_path: &str, source: &str, lang: Lang) -> Vec<Echo> {
    // Skip prose files entirely
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if PROSE_EXTENSIONS.contains(&ext) {
        return Vec::new();
    }

    // Quick scan: bail if no artifact chars present
    let artifact_chars: HashSet<char> = ARTIFACTS.iter().map(|(c, _)| *c).collect();
    if !source.chars().any(|c| artifact_chars.contains(&c)) {
        return Vec::new();
    }

    // Collect skip ranges (byte ranges of strings/comments)
    let skip_ranges = get_skip_ranges(file_path, source, lang);

    // Build line-start offset table (CRLF-safe: finds \n positions in raw bytes)
    let line_starts = build_line_starts(source);

    let mut echoes = Vec::new();
    let mut seen_lines: HashSet<usize> = HashSet::new();

    for (byte_pos, ch) in source.char_indices() {
        if let Some(name) = artifact_name(ch) {
            if in_skip_range(byte_pos, &skip_ranges) {
                continue;
            }
            let line_num = offset_to_line(&line_starts, byte_pos);
            if seen_lines.contains(&line_num) {
                continue;
            }
            seen_lines.insert(line_num);
            echoes.push(Echo {
                check: "unicode-artifacts".to_string(),
                line: line_num,
                message: format!(
                    "{} found in source code. Likely from copy-pasting LLM output.",
                    name
                ),
                suggestion: "Replace with the ASCII equivalent.".to_string(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

/// Return the artifact name if `ch` is a known artifact character.
fn artifact_name(ch: char) -> Option<&'static str> {
    for &(c, name) in ARTIFACTS {
        if c == ch {
            return Some(name);
        }
    }
    None
}

/// Check if a byte offset falls within any skip range.
fn in_skip_range(offset: usize, ranges: &[(usize, usize)]) -> bool {
    for &(start, end) in ranges {
        if offset >= start && offset < end {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Skip range collection (tree-sitter primary, heuristic fallback)
// ---------------------------------------------------------------------------

/// Tree-sitter node kinds that represent strings/comments across languages.
const STRING_COMMENT_KINDS: &[&str] = &[
    // Python
    "string",
    "string_content",
    "comment",
    // JS/TS
    "string_literal",
    "template_string",
    "template_literal",
    // Go
    "raw_string_literal",
    "interpreted_string_literal",
    // Rust
    "string_literal",
    "raw_string_literal",
    "char_literal",
    "line_comment",
    "block_comment",
];

/// Collect byte ranges of string/comment nodes using tree-sitter.
/// Falls back to heuristic scanning when tree-sitter is unavailable.
fn get_skip_ranges(file_path: &str, source: &str, lang: Lang) -> Vec<(usize, usize)> {
    // Try tree-sitter first
    if lang != Lang::Unknown {
        if let Some(ranges) = tree_sitter_skip_ranges(source, lang) {
            return ranges;
        }
    }

    // Fallback: heuristic based on file extension
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if JS_EXTENSIONS.contains(&ext) {
        scan_js_skip_ranges(source)
    } else {
        scan_hash_skip_ranges(source)
    }
}

/// Use tree-sitter to find string/comment byte ranges.
fn tree_sitter_skip_ranges(source: &str, lang: Lang) -> Option<Vec<(usize, usize)>> {
    let mut parser = lang::create_parser(lang)?;
    let tree = parser.parse(source.as_bytes(), None)?;
    let root = tree.root_node();

    let mut ranges = Vec::new();
    let mut cursor = root.walk();
    collect_skip_nodes(&mut cursor, &mut ranges);

    Some(ranges)
}

/// Recursively walk the tree collecting byte ranges of string/comment nodes.
fn collect_skip_nodes(cursor: &mut tree_sitter::TreeCursor, ranges: &mut Vec<(usize, usize)>) {
    loop {
        let node = cursor.node();
        let kind = node.kind();

        if STRING_COMMENT_KINDS.contains(&kind) {
            ranges.push((node.start_byte(), node.end_byte()));
            // Don't descend into children -- the whole node is a skip range
        } else if cursor.goto_first_child() {
            collect_skip_nodes(cursor, ranges);
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

/// Heuristic: scan JS/TS source for string literals and comments.
/// Returns byte ranges to skip.
fn scan_js_skip_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut i = 0;

    while i < n {
        let c = bytes[i];

        // Single-line comment: //
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
            let start = i;
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            ranges.push((start, i));
            continue;
        }

        // Block comment: /* ... */
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < n {
                i += 2; // skip */
            }
            ranges.push((start, i));
            continue;
        }

        // String literals: ', ", `
        if c == b'"' || c == b'\'' || c == b'`' {
            let start = i;
            let quote = c;
            i += 1;
            while i < n {
                if bytes[i] == b'\\' && i + 1 < n {
                    i += 2; // skip escaped char
                } else if bytes[i] == quote {
                    i += 1;
                    break;
                } else if quote != b'`' && bytes[i] == b'\n' {
                    break; // unterminated single-line string
                } else {
                    i += 1;
                }
            }
            ranges.push((start, i));
            continue;
        }

        i += 1;
    }

    ranges
}

/// Heuristic: scan source for #-style line comments and basic string literals.
/// Used for shell scripts, YAML, TOML, and other non-JS/non-Python files.
fn scan_hash_skip_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut line_offset = 0;

    for line in source.split('\n') {
        let bytes = line.as_bytes();
        let n = bytes.len();
        let mut i = 0;

        while i < n {
            let c = bytes[i];

            // String literal -- skip to avoid treating # inside strings as comments
            if c == b'"' || c == b'\'' {
                let start = line_offset + i;
                let quote = c;
                i += 1;
                while i < n {
                    if bytes[i] == b'\\' && i + 1 < n {
                        i += 2;
                    } else if bytes[i] == quote {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                ranges.push((start, line_offset + i));
                continue;
            }

            // Hash comment -- rest of line
            if c == b'#' {
                ranges.push((line_offset + i, line_offset + n));
                break;
            }

            i += 1;
        }

        line_offset += line.len() + 1; // +1 for the '\n'
    }

    ranges
}

// ===========================================================================
// Banned patterns check
// ===========================================================================

/// Check file against banned regex patterns from config.
///
/// For each pattern rule: compile regex, search source, report matches with
/// line numbers. Uses bisect-style line number lookup (build line_starts
/// array, binary search for match offset).
pub fn check_banned_patterns(
    file_path: &str,
    source: &str,
    patterns: &[crate::config::PatternRule],
    cwd: &str,
) -> Vec<Echo> {
    if patterns.is_empty() || source.is_empty() {
        return Vec::new();
    }

    // Build line-start offset table for bisect lookup
    let line_starts = build_line_starts(source);

    let basename = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let rel_path = if !cwd.is_empty() {
        relative_path(file_path, cwd)
    } else {
        String::new()
    };

    let mut echoes = Vec::new();

    for rule in patterns {
        if rule.pattern.is_empty() {
            continue;
        }

        // Apply glob filter if specified
        if !rule.glob.is_empty() {
            let glob = &rule.glob;
            if !glob_matches(glob, basename)
                && (rel_path.is_empty() || !glob_matches(glob, &rel_path))
            {
                continue;
            }
        }

        // Compile regex with timeout-safe approach (skip invalid patterns)
        let regex = match Regex::new(&rule.pattern) {
            Ok(r) => r,
            Err(e) => {
                debug::debug(&format!(
                    "banned-pattern: invalid regex '{}': {}",
                    rule.pattern, e
                ));
                continue;
            }
        };

        let message = if rule.message.is_empty() {
            format!("Banned pattern `{}` found.", rule.pattern)
        } else {
            rule.message.clone()
        };

        for m in regex.find_iter(source) {
            let line_num = offset_to_line(&line_starts, m.start());
            echoes.push(Echo {
                check: "banned-pattern".to_string(),
                line: line_num,
                message: message.clone(),
                suggestion: String::new(),
                severity: Severity::Warn,
                fix: None,
            });
        }
    }

    echoes
}

/// Build an array of byte offsets where each line starts.
fn build_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// Convert a byte offset to a 1-based line number using binary search.
fn offset_to_line(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx, // idx is the insertion point; line is idx (1-based)
    }
}

/// Compute relative path (forward slashes).
fn relative_path(file_path: &str, cwd: &str) -> String {
    crate::git::relative_path(file_path, cwd)
}

/// Simple glob match (fnmatch-style: *, ?, []).
fn glob_matches(pattern: &str, text: &str) -> bool {
    let pat = globset::GlobBuilder::new(pattern)
        .literal_separator(false)
        .build();
    match pat {
        Ok(glob) => {
            let matcher = glob.compile_matcher();
            matcher.is_match(text)
        }
        Err(_) => false,
    }
}

// ===========================================================================
// Import layers check
// ===========================================================================

/// JS/TS import regex: `import X from 'mod'` and `require('mod')`.
fn js_import_regex() -> Regex {
    Regex::new(r#"(?:import\s+.*?\s+from\s+['"](.+?)['"]|require\s*\(\s*['"](.+?)['"]\s*\))"#)
        .expect("js import regex is valid")
}

/// Extract Python imports via tree-sitter AST.
/// Returns (module_name, line_number) pairs.
fn extract_python_imports(source: &str) -> Vec<(String, usize)> {
    let mut parser = match lang::create_parser(Lang::Python) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let tree = match parser.parse(source.as_bytes(), None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut imports = Vec::new();
    let root = tree.root_node();

    collect_python_imports(&root, source.as_bytes(), &mut imports);

    imports
}

/// Walk the AST collecting import module names.
fn collect_python_imports(
    node: &tree_sitter::Node,
    source: &[u8],
    imports: &mut Vec<(String, usize)>,
) {
    let kind = node.kind();

    if kind == "import_statement" {
        // `import foo, bar` -- extract dotted names
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i) {
                if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
                    let name_node = if child.kind() == "aliased_import" {
                        child.named_child(0)
                    } else {
                        Some(child)
                    };
                    if let Some(n) = name_node {
                        if let Ok(text) = std::str::from_utf8(&source[n.start_byte()..n.end_byte()])
                        {
                            imports.push((text.to_string(), n.start_position().row + 1));
                        }
                    }
                }
            }
        }
    } else if kind == "import_from_statement" {
        // `from foo import bar` -- extract the module name
        if let Some(module_node) = node.child_by_field_name("module_name") {
            if let Ok(text) =
                std::str::from_utf8(&source[module_node.start_byte()..module_node.end_byte()])
            {
                imports.push((text.to_string(), module_node.start_position().row + 1));
            }
        } else {
            // Fallback: look for dotted_name child
            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    if child.kind() == "dotted_name" {
                        if let Ok(text) =
                            std::str::from_utf8(&source[child.start_byte()..child.end_byte()])
                        {
                            imports.push((text.to_string(), child.start_position().row + 1));
                            break;
                        }
                    }
                }
            }
        }
    }

    // Recurse into children (only top-level statements)
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_python_imports(&child, source, imports);
        }
    }
}

/// Extract JS/TS imports via regex.
/// Returns (module_specifier, line_number) pairs.
fn extract_js_imports(source: &str) -> Vec<(String, usize)> {
    let re = js_import_regex();
    let mut imports = Vec::new();
    let mut current_line = 1usize;
    let mut last_offset = 0;

    for m in re.find_iter(source) {
        // Update line count
        current_line += source[last_offset..m.start()]
            .bytes()
            .filter(|&b| b == b'\n')
            .count();
        last_offset = m.start();

        // Check if inside a comment
        if is_in_js_comment(source, m.start()) {
            continue;
        }

        // Extract capture group
        let caps = re.captures(&source[m.start()..]);
        if let Some(caps) = caps {
            let module = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|c| c.as_str().to_string());
            if let Some(mod_name) = module {
                imports.push((mod_name, current_line));
            }
        }
    }

    imports
}

/// Check if offset falls inside a JS/TS comment (// or /* */).
fn is_in_js_comment(source: &str, offset: usize) -> bool {
    // Check for // line comment
    let line_start = source[..offset].rfind('\n').map(|p| p + 1).unwrap_or(0);
    if let Some(pos) = source[line_start..offset].find("//") {
        // Make sure the // is not inside a string
        let abs_pos = line_start + pos;
        if abs_pos < offset {
            return true;
        }
    }
    // Check for /* block comment */
    if let Some(last_open) = source[..offset].rfind("/*") {
        if source[last_open..offset].rfind("*/").is_none() {
            return true;
        }
    }
    false
}

/// Check if an import matches a denied module (separator-aware prefix match).
fn matches_deny(imp: &str, denied: &str, is_python: bool) -> bool {
    if imp == denied {
        return true;
    }
    let sep = if is_python { "." } else { "/" };
    imp.starts_with(&format!("{}{}", denied, sep))
}

/// Check file imports against architecture layer rules.
///
/// Extracts imports (tree-sitter for Python, regex for JS/TS), checks
/// against config.import_rules deny lists. Matches file path against
/// rule's `files` glob.
pub fn check_import_layers(
    file_path: &str,
    source: &str,
    lang: Lang,
    rules: &[crate::config::ImportRule],
    cwd: &str,
) -> Vec<Echo> {
    if rules.is_empty() {
        return Vec::new();
    }

    let is_python = matches!(lang, Lang::Python);
    let is_js = matches!(lang, Lang::JavaScript | Lang::TypeScript | Lang::Tsx);

    if !is_python && !is_js {
        return Vec::new();
    }

    // Compute relative path
    let rel = if !cwd.is_empty() {
        relative_path(file_path, cwd)
    } else {
        Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string()
    };

    let basename = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Extract imports
    let imports = if is_python {
        extract_python_imports(source)
    } else {
        extract_js_imports(source)
    };

    if imports.is_empty() {
        return Vec::new();
    }

    let mut echoes = Vec::new();

    for rule in rules {
        if rule.files.is_empty() {
            continue;
        }

        // Match against relative path and basename
        if !glob_matches(&rule.files, &rel) && !glob_matches(&rule.files, basename) {
            continue;
        }

        let message = if rule.message.is_empty() {
            "Import violates architecture layer rules".to_string()
        } else {
            rule.message.clone()
        };

        for (imp, lineno) in &imports {
            for denied in &rule.deny {
                if matches_deny(imp, denied, is_python) {
                    echoes.push(Echo {
                        check: "import-layer".to_string(),
                        line: *lineno,
                        message: format!("Import '{}' is denied by rule: {}", imp, message),
                        suggestion: format!("Remove or replace the import of '{}'.", denied),
                        severity: Severity::Warn,
                        fix: None,
                    });
                }
            }
        }
    }

    echoes
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- unicode artifact tests ---

    #[test]
    fn test_artifact_detection_basic() {
        let source = "let x = hello \u{2014} world;\n";
        let echoes = check_unicode_artifacts("test.rs", source, Lang::Rust);
        assert_eq!(echoes.len(), 1);
        assert_eq!(echoes[0].check, "unicode-artifacts");
        assert_eq!(echoes[0].line, 1);
        assert!(echoes[0].message.contains("Em dash"));
    }

    #[test]
    fn test_artifact_skip_prose() {
        let source = "Hello \u{2014} world\n";
        assert!(check_unicode_artifacts("readme.md", source, Lang::Unknown).is_empty());
        assert!(check_unicode_artifacts("notes.txt", source, Lang::Unknown).is_empty());
        assert!(check_unicode_artifacts("doc.rst", source, Lang::Unknown).is_empty());
        assert!(check_unicode_artifacts("doc.adoc", source, Lang::Unknown).is_empty());
        assert!(check_unicode_artifacts("doc.rdoc", source, Lang::Unknown).is_empty());
    }

    #[test]
    fn test_artifact_no_false_positive_clean_code() {
        let source = "fn main() {\n    println!(\"hello\");\n}\n";
        let echoes = check_unicode_artifacts("main.rs", source, Lang::Rust);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_artifact_skip_in_string_python() {
        // Em dash inside a Python string should be skipped
        let source = "msg = \"hello \u{2014} world\"\n";
        let echoes = check_unicode_artifacts("test.py", source, Lang::Python);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_artifact_skip_in_comment_python() {
        // Em dash in a Python comment should be skipped
        let source = "# This is a comment \u{2014} with dash\nx = 1\n";
        let echoes = check_unicode_artifacts("test.py", source, Lang::Python);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_artifact_detected_outside_string() {
        // Em dash in code (not in string/comment) should be detected
        let source = "x = 1\ny = hello \u{2014} world\n";
        let echoes = check_unicode_artifacts("test.py", source, Lang::Python);
        assert_eq!(echoes.len(), 1);
        assert_eq!(echoes[0].line, 2);
    }

    #[test]
    fn test_artifact_multiple_on_same_line() {
        // Only one echo per line
        let source = "x \u{2014} y \u{2013} z\n";
        let echoes = check_unicode_artifacts("test.rs", source, Lang::Rust);
        assert_eq!(echoes.len(), 1);
    }

    #[test]
    fn test_artifact_zero_width_chars() {
        let source = "let x\u{200b} = 1;\n";
        let echoes = check_unicode_artifacts("test.rs", source, Lang::Rust);
        assert_eq!(echoes.len(), 1);
        assert!(echoes[0].message.contains("Zero-width space"));
    }

    #[test]
    fn test_artifact_all_13_chars() {
        let chars = [
            '\u{2014}', '\u{2013}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2026}',
            '\u{2022}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{200e}', '\u{200f}',
        ];
        for ch in &chars {
            let source = format!("x{}y\n", ch);
            let echoes = check_unicode_artifacts("test.rs", &source, Lang::Rust);
            assert_eq!(
                echoes.len(),
                1,
                "Expected 1 echo for char U+{:04X}",
                *ch as u32
            );
        }
    }

    // --- heuristic skip range tests ---

    #[test]
    fn test_js_skip_ranges_line_comment() {
        let source = "// hello \u{2014} world\nlet x = 1;\n";
        let echoes = check_unicode_artifacts("test.js", source, Lang::Unknown);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_js_skip_ranges_block_comment() {
        let source = "/* hello \u{2014} world */\nlet x = 1;\n";
        let echoes = check_unicode_artifacts("test.js", source, Lang::Unknown);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_js_skip_ranges_string() {
        let source = "let x = \"hello \u{2014} world\";\n";
        let echoes = check_unicode_artifacts("test.js", source, Lang::Unknown);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_hash_skip_ranges_comment() {
        let source = "# comment \u{2014} here\nx = 1\n";
        let echoes = check_unicode_artifacts("test.sh", source, Lang::Unknown);
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_hash_skip_ranges_string() {
        let source = "x = \"\u{2014}\"\n";
        let echoes = check_unicode_artifacts("test.sh", source, Lang::Unknown);
        assert!(echoes.is_empty());
    }

    // --- banned patterns tests ---

    #[test]
    fn test_banned_patterns_basic() {
        use crate::config::PatternRule;
        let source = "console.log('debug');\nconsole.log('test');\n";
        let patterns = vec![PatternRule {
            pattern: "console\\.log".to_string(),
            message: "No console.log".to_string(),
            glob: String::new(),
        }];
        let echoes = check_banned_patterns("test.js", source, &patterns, "");
        assert_eq!(echoes.len(), 2);
        assert_eq!(echoes[0].line, 1);
        assert_eq!(echoes[1].line, 2);
        assert_eq!(echoes[0].message, "No console.log");
    }

    #[test]
    fn test_banned_patterns_invalid_regex() {
        use crate::config::PatternRule;
        let source = "hello world\n";
        let patterns = vec![PatternRule {
            pattern: "[invalid".to_string(),
            message: String::new(),
            glob: String::new(),
        }];
        let echoes = check_banned_patterns("test.js", source, &patterns, "");
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_banned_patterns_with_glob() {
        use crate::config::PatternRule;
        let source = "TODO: fix this\n";
        let patterns = vec![PatternRule {
            pattern: "TODO".to_string(),
            message: "No TODOs".to_string(),
            glob: "*.rs".to_string(),
        }];
        // Should NOT match .js file
        let echoes = check_banned_patterns("test.js", source, &patterns, "");
        assert!(echoes.is_empty());
        // Should match .rs file
        let echoes = check_banned_patterns("test.rs", source, &patterns, "");
        assert_eq!(echoes.len(), 1);
    }

    #[test]
    fn test_banned_patterns_empty_source() {
        use crate::config::PatternRule;
        let patterns = vec![PatternRule {
            pattern: "foo".to_string(),
            message: String::new(),
            glob: String::new(),
        }];
        let echoes = check_banned_patterns("test.js", "", &patterns, "");
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_banned_patterns_empty_patterns() {
        let echoes = check_banned_patterns("test.js", "hello", &[], "");
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_banned_patterns_default_message() {
        use crate::config::PatternRule;
        let source = "debugger;\n";
        let patterns = vec![PatternRule {
            pattern: "debugger".to_string(),
            message: String::new(),
            glob: String::new(),
        }];
        let echoes = check_banned_patterns("test.js", source, &patterns, "");
        assert_eq!(echoes.len(), 1);
        assert!(echoes[0]
            .message
            .contains("Banned pattern `debugger` found."));
    }

    #[test]
    fn test_offset_to_line() {
        let source = "line1\nline2\nline3\n";
        let starts = build_line_starts(source);
        assert_eq!(offset_to_line(&starts, 0), 1); // start of line1
        assert_eq!(offset_to_line(&starts, 3), 1); // middle of line1
        assert_eq!(offset_to_line(&starts, 6), 2); // start of line2
        assert_eq!(offset_to_line(&starts, 12), 3); // start of line3
    }

    // --- import layers tests ---

    #[test]
    fn test_import_layers_python_deny() {
        use crate::config::ImportRule;
        let source = "import os\nfrom django.db import models\n";
        let rules = vec![ImportRule {
            files: "*.py".to_string(),
            deny: vec!["django".to_string()],
            message: "No Django in utils".to_string(),
        }];
        let echoes = check_import_layers("utils.py", source, Lang::Python, &rules, "");
        assert_eq!(echoes.len(), 1);
        assert!(echoes[0].message.contains("django"));
        assert_eq!(echoes[0].check, "import-layer");
    }

    #[test]
    fn test_import_layers_python_allow() {
        use crate::config::ImportRule;
        let source = "import os\nimport sys\n";
        let rules = vec![ImportRule {
            files: "*.py".to_string(),
            deny: vec!["django".to_string()],
            message: "No Django".to_string(),
        }];
        let echoes = check_import_layers("utils.py", source, Lang::Python, &rules, "");
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_import_layers_js_deny() {
        use crate::config::ImportRule;
        let source = "import React from 'react';\nimport axios from 'axios';\n";
        let rules = vec![ImportRule {
            files: "*.js".to_string(),
            deny: vec!["axios".to_string()],
            message: "Use fetch instead".to_string(),
        }];
        let echoes = check_import_layers("app.js", source, Lang::JavaScript, &rules, "");
        assert_eq!(echoes.len(), 1);
        assert!(echoes[0].message.contains("axios"));
    }

    #[test]
    fn test_import_layers_no_rules() {
        let echoes = check_import_layers("test.py", "import os\n", Lang::Python, &[], "");
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_import_layers_unsupported_lang() {
        use crate::config::ImportRule;
        let rules = vec![ImportRule {
            files: "*".to_string(),
            deny: vec!["foo".to_string()],
            message: "no foo".to_string(),
        }];
        let echoes = check_import_layers("test.go", "import foo", Lang::Go, &rules, "");
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_import_layers_glob_no_match() {
        use crate::config::ImportRule;
        let source = "from django.db import models\n";
        let rules = vec![ImportRule {
            files: "api/*.py".to_string(),
            deny: vec!["django".to_string()],
            message: "No Django".to_string(),
        }];
        // File doesn't match glob
        let echoes = check_import_layers("utils.py", source, Lang::Python, &rules, "");
        assert!(echoes.is_empty());
    }

    #[test]
    fn test_import_layers_prefix_match() {
        use crate::config::ImportRule;
        let source = "from django.db import models\n";
        let rules = vec![ImportRule {
            files: "*.py".to_string(),
            deny: vec!["django".to_string()],
            message: "No Django".to_string(),
        }];
        // `django.db` starts with `django.` -- should match
        let echoes = check_import_layers("models.py", source, Lang::Python, &rules, "");
        assert_eq!(echoes.len(), 1);
    }

    #[test]
    fn test_matches_deny_python() {
        assert!(matches_deny("django.db", "django", true));
        assert!(matches_deny("django", "django", true));
        assert!(!matches_deny("djangotools", "django", true));
    }

    #[test]
    fn test_matches_deny_js() {
        assert!(matches_deny("react/dom", "react", false));
        assert!(matches_deny("react", "react", false));
        assert!(!matches_deny("react-dom", "react", false));
    }

    // --- helper tests ---

    #[test]
    fn test_in_skip_range() {
        let ranges = vec![(5, 10), (20, 30)];
        assert!(!in_skip_range(0, &ranges));
        assert!(in_skip_range(5, &ranges));
        assert!(in_skip_range(9, &ranges));
        assert!(!in_skip_range(10, &ranges));
        assert!(in_skip_range(25, &ranges));
        assert!(!in_skip_range(30, &ranges));
    }

    #[test]
    fn test_scan_js_skip_ranges() {
        let source = "// comment\nlet x = \"str\";\n";
        let ranges = scan_js_skip_ranges(source);
        assert!(ranges.len() >= 2); // comment + string
    }

    #[test]
    fn test_scan_hash_skip_ranges() {
        let source = "x = 1 # comment\ny = \"str\"\n";
        let ranges = scan_hash_skip_ranges(source);
        assert!(ranges.len() >= 2); // comment + string
    }
}
