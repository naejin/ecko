//! Built-in checks -- tree-sitter query-based and regex-based.
//!
//! Layer 2 dispatch hub. Each language has its own submodule with check
//! implementations. Universal checks (banned patterns, unicode artifacts)
//! apply to all files.

pub mod custom;
pub mod dead_code;
pub mod go;
pub mod javascript;
pub mod python;
pub mod rust_checks;
pub mod universal;

use crate::config::EckoConfig;
use crate::echo::Echo;
use crate::lang::Lang;

/// Run all applicable Layer 2 checks for a file.
///
/// Dispatches to language-specific and universal check modules.
/// Returns unsorted echoes -- the caller handles ordering.
pub fn run_layer2_checks(
    file_path: &str,
    lang: Lang,
    source: &str,
    cwd: &str,
    config: &EckoConfig,
) -> Vec<Echo> {
    let mut echoes = Vec::new();

    // Language-specific checks
    match lang {
        Lang::Python => {
            echoes.extend(python::run_checks(file_path, source, config));
        }
        Lang::JavaScript | Lang::TypeScript | Lang::Tsx => {
            echoes.extend(javascript::run_checks(file_path, source, lang, config));
        }
        Lang::Go => {
            echoes.extend(go::run_checks(file_path, source, config));
        }
        Lang::Rust => {
            echoes.extend(rust_checks::run_checks(file_path, source, config));
        }
        _ => {}
    }

    // Universal checks (all languages)
    echoes.extend(universal::run_checks(file_path, source, lang, config, cwd));

    // Custom tree-sitter checks from config
    echoes.extend(custom::run_checks(file_path, source, lang, config));

    echoes
}

/// List check names applicable to a given language (for dry-run mode).
pub fn list_applicable_checks(lang: Lang) -> Vec<String> {
    const PYTHON: &[&str] = &[
        "bare-except",
        "star-imports",
        "unused-imports",
        "singleton-comparison",
        "mutable-default-args",
        "builtin-shadowing",
        "placeholder-code",
        "unreachable-code",
        "duplicate-keys",
        "test-conditional",
        "fixed-wait",
        "mock-spec-bypass",
    ];
    const JS_TS: &[&str] = &[
        "debugger-statement",
        "no-var",
        "unused-imports",
        "unreachable-code",
        "duplicate-keys",
        "empty-block-statements",
        "useless-catch",
        "placeholder-code",
    ];
    const GO: &[&str] = &[
        "unused-imports",
        "empty-error-check",
        "unreachable-code",
        "placeholder-code",
    ];
    const RUST: &[&str] = &[
        "unused-imports",
        "todo-macro",
        "unreachable-code",
        "placeholder-code",
    ];
    const UNIVERSAL: &[&str] = &[
        "trailing-whitespace",
        "unicode-artifacts",
        "banned-patterns",
        "import-layers",
        "obsolete-terms",
    ];

    let lang_checks: &[&str] = match lang {
        Lang::Python => PYTHON,
        Lang::JavaScript | Lang::TypeScript | Lang::Tsx => JS_TS,
        Lang::Go => GO,
        Lang::Rust => RUST,
        _ => &[],
    };

    lang_checks
        .iter()
        .chain(UNIVERSAL.iter())
        .map(|s| s.to_string())
        .collect()
}
