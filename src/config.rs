//! Configuration loading and access for ecko.yaml project configs.
//!
//! Uses serde_yaml for deserialization. Falls back to `EckoConfig::default()`
//! when the config file is missing or unparseable.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::debug;
use crate::echo;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct PatternRule {
    pub pattern: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub glob: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImportRule {
    pub files: String,
    pub deny: Vec<String>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CustomCheck {
    pub name: String,
    pub languages: Vec<String>,
    pub query: String,
    pub message: String,
    #[serde(default = "default_severity")]
    pub severity: String,
}

fn default_severity() -> String {
    "warn".to_string()
}

// ---------------------------------------------------------------------------
// EckoConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct EckoConfig {
    pub disabled_checks: Vec<String>,
    pub exclude: Vec<String>,
    pub banned_patterns: Vec<PatternRule>,
    pub obsolete_terms: Vec<PatternRule>,
    pub blocked_commands: Vec<PatternRule>,
    pub autofix: HashMap<String, bool>,
    pub deep_analysis: HashMap<String, bool>,
    pub echo_cap_per_check: usize,
    pub echo_cap_cross_file: usize,
    pub session_hours: f64,
    pub output_format: String,
    pub reverb: HashMap<String, bool>,
    pub builtin_shadow_allowlist: Option<Vec<String>>,
    pub import_rules: Vec<ImportRule>,
    pub custom_checks: Vec<CustomCheck>,
    pub fix_suggestions: bool,
}

impl Default for EckoConfig {
    fn default() -> Self {
        Self {
            disabled_checks: Vec::new(),
            exclude: Vec::new(),
            banned_patterns: Vec::new(),
            obsolete_terms: Vec::new(),
            blocked_commands: Vec::new(),
            autofix: HashMap::new(),
            deep_analysis: HashMap::new(),
            echo_cap_per_check: 5,
            echo_cap_cross_file: 0,
            session_hours: 4.0,
            output_format: "text".to_string(),
            reverb: HashMap::new(),
            builtin_shadow_allowlist: None,
            import_rules: Vec::new(),
            custom_checks: Vec::new(),
            fix_suggestions: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Default builtin shadow allowlist (20 names)
// ---------------------------------------------------------------------------

const DEFAULT_SHADOW_ALLOWLIST: &[&str] = &[
    "type", "help", "input", "format", "id", "repr", "ascii", "hash", "hex", "oct", "dir", "next",
    "map", "filter", "list", "dict", "set", "max", "min", "range",
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load `ecko.yaml` from `cwd`. Returns `EckoConfig::default()` when the file
/// is absent or contains invalid YAML.
pub fn load_config(cwd: &str) -> EckoConfig {
    let path = Path::new(cwd).join("ecko.yaml");
    if !path.exists() {
        debug::debug("ecko.yaml not found, using defaults");
        return EckoConfig::default();
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            debug::debug(&format!("failed to read ecko.yaml: {e}"));
            return EckoConfig::default();
        }
    };

    match serde_yaml::from_str::<EckoConfig>(&contents) {
        Ok(cfg) => {
            debug::debug("ecko.yaml loaded successfully");
            validate_custom_checks(&cfg);
            cfg
        }
        Err(e) => {
            echo::emit(&format!(
                "~~ ecko ~~ warning: failed to parse ecko.yaml: {e}"
            ));
            EckoConfig::default()
        }
    }
}

/// Validate custom check queries at load time.
///
/// Attempts to compile each custom check's tree-sitter query against a
/// representative language. Invalid queries emit a user-facing warning
/// immediately rather than failing silently during per-file checks.
fn validate_custom_checks(config: &EckoConfig) {
    use crate::lang;
    use crate::query_engine;

    for custom in &config.custom_checks {
        // Pick first specified language, or fall back to Python as a representative.
        let test_lang = custom
            .languages
            .first()
            .and_then(|l| match l.as_str() {
                "python" => Some(lang::Lang::Python),
                "javascript" => Some(lang::Lang::JavaScript),
                "typescript" => Some(lang::Lang::TypeScript),
                "tsx" => Some(lang::Lang::Tsx),
                "go" => Some(lang::Lang::Go),
                "rust" => Some(lang::Lang::Rust),
                _ => None,
            })
            .unwrap_or(lang::Lang::Python);

        if let Some(ts_lang) = lang::get_tree_sitter_language(test_lang) {
            if let Err(e) = query_engine::compile_query(&ts_lang, &custom.query) {
                echo::emit(&format!(
                    "~~ ecko ~~ warning: custom check '{}' has invalid query: {e}",
                    custom.name
                ));
            }
        }
    }
}

/// Set of disabled check names (kebab-case).
pub fn get_disabled_checks(config: &EckoConfig) -> HashSet<String> {
    config.disabled_checks.iter().cloned().collect()
}

/// Whether autofix is enabled for `tool` (default: false).
pub fn is_autofix_enabled(config: &EckoConfig, tool: &str) -> bool {
    config.autofix.get(tool).copied().unwrap_or(false)
}

/// Whether deep analysis is enabled for `tool` (default: false).
pub fn is_deep_enabled(config: &EckoConfig, tool: &str) -> bool {
    config.deep_analysis.get(tool).copied().unwrap_or(false)
}

/// Builtin shadow allowlist -- returns the user-configured list if present,
/// otherwise the 20-name default set.
pub fn get_builtin_shadow_allowlist(config: &EckoConfig) -> HashSet<String> {
    match &config.builtin_shadow_allowlist {
        Some(user_list) => user_list.iter().cloned().collect(),
        None => DEFAULT_SHADOW_ALLOWLIST
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    }
}

/// Whether output format is JSON.
pub fn is_output_json(config: &EckoConfig) -> bool {
    config.output_format == "json"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config_missing_file_returns_defaults() {
        let cfg = load_config("/tmp/ecko-no-such-dir-test");
        assert_eq!(cfg.echo_cap_per_check, 5);
        assert_eq!(cfg.session_hours, 4.0);
        assert!(cfg.disabled_checks.is_empty());
        assert!(cfg.banned_patterns.is_empty());
        assert_eq!(cfg.output_format, "text");
        assert!(cfg.fix_suggestions);
    }

    #[test]
    fn load_config_valid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
disabled_checks:
  - bare-except
echo_cap_per_check: 10
session_hours: 2.0
output_format: json
"#;
        std::fs::write(dir.path().join("ecko.yaml"), yaml).unwrap();
        let cfg = load_config(dir.path().to_str().unwrap());
        assert_eq!(cfg.disabled_checks, vec!["bare-except"]);
        assert_eq!(cfg.echo_cap_per_check, 10);
        assert_eq!(cfg.session_hours, 2.0);
        assert!(is_output_json(&cfg));
    }

    #[test]
    fn load_config_invalid_yaml_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ecko.yaml"), "{{{{ not yaml").unwrap();
        let cfg = load_config(dir.path().to_str().unwrap());
        // Should fall back to defaults
        assert_eq!(cfg.echo_cap_per_check, 5);
    }

    #[test]
    fn get_disabled_checks_roundtrips() {
        let mut cfg = EckoConfig::default();
        cfg.disabled_checks = vec!["foo".into(), "bar".into()];
        let set = get_disabled_checks(&cfg);
        assert!(set.contains("foo"));
        assert!(set.contains("bar"));
        assert!(!set.contains("baz"));
    }

    #[test]
    fn autofix_and_deep_defaults_to_false() {
        let cfg = EckoConfig::default();
        assert!(!is_autofix_enabled(&cfg, "black"));
        assert!(!is_deep_enabled(&cfg, "pyright"));
    }

    #[test]
    fn autofix_enabled_when_set() {
        let mut cfg = EckoConfig::default();
        cfg.autofix.insert("black".into(), true);
        assert!(is_autofix_enabled(&cfg, "black"));
        assert!(!is_autofix_enabled(&cfg, "prettier"));
    }

    #[test]
    fn shadow_allowlist_default_has_20_names() {
        let cfg = EckoConfig::default();
        let set = get_builtin_shadow_allowlist(&cfg);
        assert_eq!(set.len(), 20);
        assert!(set.contains("type"));
        assert!(set.contains("id"));
    }

    #[test]
    fn shadow_allowlist_user_replaces_default() {
        let mut cfg = EckoConfig::default();
        cfg.builtin_shadow_allowlist = Some(vec!["myname".into()]);
        let set = get_builtin_shadow_allowlist(&cfg);
        assert_eq!(set.len(), 1);
        assert!(set.contains("myname"));
        assert!(!set.contains("type"));
    }

    #[test]
    fn output_format_json_detection() {
        let mut cfg = EckoConfig::default();
        assert!(!is_output_json(&cfg));
        cfg.output_format = "json".into();
        assert!(is_output_json(&cfg));
    }
}
