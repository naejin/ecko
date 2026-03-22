//! Custom tree-sitter checks from ecko.yaml config.
//!
//! Users can define custom checks with a query, message, severity, and
//! language filter in their ecko.yaml. This module compiles and runs those.

use crate::config::EckoConfig;
use crate::debug;
use crate::echo::{Echo, Severity};
use crate::lang::{self, Lang};
use crate::query_engine::{self, QueryCheck};

/// Run user-defined custom checks from config.
///
/// Phase 1: basic implementation -- compiles and runs custom queries.
pub fn run_checks(_file_path: &str, source: &str, lang: Lang, config: &EckoConfig) -> Vec<Echo> {
    if config.custom_checks.is_empty() {
        return Vec::new();
    }

    let (ts_lang, tree) = match lang::parse_for_checks(lang, source) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let source_bytes = source.as_bytes();
    let lang_str = lang_to_str(lang);
    let mut echoes = Vec::new();

    for custom in &config.custom_checks {
        // Check language filter.
        if !custom.languages.is_empty() && !custom.languages.iter().any(|l| l == lang_str) {
            continue;
        }

        let severity = match custom.severity.as_str() {
            "error" => Severity::Error,
            _ => Severity::Warn,
        };

        match query_engine::compile_query(&ts_lang, &custom.query) {
            Ok(query) => {
                let check = QueryCheck {
                    name: custom.name.clone(),
                    query,
                    message: custom.message.clone(),
                    severity,
                    capture_name: "match".to_string(),
                };
                echoes.extend(query_engine::run_query(&check, &tree, source_bytes));
            }
            Err(e) => {
                crate::echo::emit(&format!(
                    "~~ ecko ~~ warning: custom check '{}' has invalid query: {e}",
                    custom.name
                ));
                debug::debug(&format!("custom check '{}' query failed: {e}", custom.name));
            }
        }
    }

    echoes
}

/// Map Lang enum to a string for matching against config language filters.
fn lang_to_str(lang: Lang) -> &'static str {
    match lang {
        Lang::Python => "python",
        Lang::JavaScript => "javascript",
        Lang::TypeScript => "typescript",
        Lang::Tsx => "tsx",
        Lang::Go => "go",
        Lang::Rust => "rust",
        Lang::Unknown => "unknown",
    }
}
