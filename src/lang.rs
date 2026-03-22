//! Language detection and tree-sitter parser management.
//!
//! Maps file extensions to language variants and provides tree-sitter
//! parser construction for each supported language.

use std::path::Path;
use tree_sitter::{Language, Parser};

/// Supported language variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    Rust,
    Unknown,
}

/// Detect language from file extension.
///
/// Uses filename extension only -- never inspects file contents.
pub fn detect_language(file_path: &str) -> Lang {
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "py" | "pyi" => Lang::Python,
        "js" | "jsx" | "mjs" | "cjs" => Lang::JavaScript,
        "ts" => Lang::TypeScript,
        "tsx" => Lang::Tsx,
        "go" => Lang::Go,
        "rs" => Lang::Rust,
        _ => Lang::Unknown,
    }
}

/// Return the tree-sitter `Language` for a given `Lang` variant.
///
/// Returns `None` for `Lang::Unknown`.
pub fn get_tree_sitter_language(lang: Lang) -> Option<Language> {
    match lang {
        Lang::Python => Some(tree_sitter_python::LANGUAGE.into()),
        Lang::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        Lang::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Lang::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        Lang::Go => Some(tree_sitter_go::LANGUAGE.into()),
        Lang::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        Lang::Unknown => None,
    }
}

/// Create a configured tree-sitter `Parser` for the given language.
///
/// Returns `None` for `Lang::Unknown` or if setting the language fails.
pub fn create_parser(lang: Lang) -> Option<Parser> {
    let ts_lang = get_tree_sitter_language(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&ts_lang).ok()?;
    Some(parser)
}

/// Parse source code for a given language. Returns the tree-sitter Language and parsed Tree.
/// Returns None if the language is unsupported or parsing fails.
pub fn parse_for_checks(lang: Lang, source: &str) -> Option<(Language, tree_sitter::Tree)> {
    let ts_lang = get_tree_sitter_language(lang)?;
    let mut parser = create_parser(lang)?;
    let tree = crate::query_engine::parse_source(&mut parser, source)?;
    Some((ts_lang, tree))
}

/// Filename-only test-file detection.
///
/// Returns `true` if the file is a Python test file:
/// - starts with `test_` and ends with `.py`
/// - ends with `_test.py`
/// - is exactly `conftest.py` or `conftest.pyi`
pub fn is_test_file(file_path: &str) -> bool {
    let filename = match Path::new(file_path).file_name().and_then(|f| f.to_str()) {
        Some(name) => name,
        None => return false,
    };

    if filename == "conftest.py" || filename == "conftest.pyi" {
        return true;
    }

    if filename.starts_with("test_") && filename.ends_with(".py") {
        return true;
    }

    if filename.ends_with("_test.py") {
        return true;
    }

    false
}

/// Returns `true` for stub/declaration files that should be skipped from linting.
///
/// Matches `.pyi` (Python type stubs) and `.test-d.ts` (tsd assertion files).
pub fn is_skippable_stub(file_path: &str) -> bool {
    let filename = match Path::new(file_path).file_name().and_then(|f| f.to_str()) {
        Some(name) => name,
        None => return false,
    };

    // .pyi type stubs
    if filename.ends_with(".pyi") {
        return true;
    }

    // .test-d.ts tsd assertion files
    if filename.ends_with(".test-d.ts") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_python() {
        assert_eq!(detect_language("foo.py"), Lang::Python);
        assert_eq!(detect_language("bar.pyi"), Lang::Python);
        assert_eq!(detect_language("/abs/path/module.py"), Lang::Python);
    }

    #[test]
    fn test_detect_language_javascript() {
        assert_eq!(detect_language("app.js"), Lang::JavaScript);
        assert_eq!(detect_language("component.jsx"), Lang::JavaScript);
        assert_eq!(detect_language("util.mjs"), Lang::JavaScript);
        assert_eq!(detect_language("legacy.cjs"), Lang::JavaScript);
    }

    #[test]
    fn test_detect_language_typescript() {
        assert_eq!(detect_language("app.ts"), Lang::TypeScript);
        assert_eq!(detect_language("component.tsx"), Lang::Tsx);
    }

    #[test]
    fn test_detect_language_go_rust() {
        assert_eq!(detect_language("main.go"), Lang::Go);
        assert_eq!(detect_language("lib.rs"), Lang::Rust);
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(detect_language("readme.md"), Lang::Unknown);
        assert_eq!(detect_language("Makefile"), Lang::Unknown);
        assert_eq!(detect_language(""), Lang::Unknown);
    }

    #[test]
    fn test_get_tree_sitter_language() {
        assert!(get_tree_sitter_language(Lang::Python).is_some());
        assert!(get_tree_sitter_language(Lang::JavaScript).is_some());
        assert!(get_tree_sitter_language(Lang::TypeScript).is_some());
        assert!(get_tree_sitter_language(Lang::Tsx).is_some());
        assert!(get_tree_sitter_language(Lang::Go).is_some());
        assert!(get_tree_sitter_language(Lang::Rust).is_some());
        assert!(get_tree_sitter_language(Lang::Unknown).is_none());
    }

    #[test]
    fn test_create_parser() {
        assert!(create_parser(Lang::Python).is_some());
        assert!(create_parser(Lang::TypeScript).is_some());
        assert!(create_parser(Lang::Tsx).is_some());
        assert!(create_parser(Lang::Unknown).is_none());
    }

    #[test]
    fn test_is_test_file() {
        assert!(is_test_file("test_foo.py"));
        assert!(is_test_file("/path/to/test_bar.py"));
        assert!(is_test_file("foo_test.py"));
        assert!(is_test_file("conftest.py"));
        assert!(is_test_file("conftest.pyi"));
        assert!(is_test_file("/deep/path/conftest.py"));

        assert!(!is_test_file("test_foo.js"));
        assert!(!is_test_file("foo.py"));
        assert!(!is_test_file("testfoo.py"));
        assert!(!is_test_file("conftest.yaml"));
    }

    #[test]
    fn test_is_skippable_stub() {
        assert!(is_skippable_stub("types.pyi"));
        assert!(is_skippable_stub("/path/to/module.pyi"));
        assert!(is_skippable_stub("utils.test-d.ts"));

        assert!(!is_skippable_stub("module.py"));
        assert!(!is_skippable_stub("app.ts"));
        assert!(!is_skippable_stub("test.d.ts"));
    }
}
