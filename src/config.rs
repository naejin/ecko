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
// Guard metadata (from .ecko-guard.yaml)
// ---------------------------------------------------------------------------

/// Metadata about active `.ecko-guard.yaml` guard rules.
///
/// Populated during config merge, not from ecko.yaml deserialization.
#[derive(Debug, Clone)]
pub struct GuardMeta {
    /// Unix epoch seconds when the guard file was created.
    pub created: f64,
    /// Human-readable task description (e.g., "auth-refactor").
    pub task: String,
    /// Check names sourced from the guard file (for friction detection).
    /// Uses actual check names as emitted by checks (e.g., "banned-patterns", "import-layers").
    pub guard_check_names: HashSet<String>,
}

/// Deserialization target for `.ecko-guard.yaml`.
///
/// Same rule fields as `EckoConfig` plus guard-specific metadata.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
struct EckoGuardConfig {
    created: f64,
    #[serde(default)]
    task: String,
    #[serde(default)]
    banned_patterns: Vec<PatternRule>,
    #[serde(default)]
    obsolete_terms: Vec<PatternRule>,
    #[serde(default)]
    blocked_commands: Vec<PatternRule>,
    #[serde(default)]
    import_rules: Vec<ImportRule>,
    #[serde(default)]
    custom_checks: Vec<CustomCheck>,
}

impl Default for EckoGuardConfig {
    fn default() -> Self {
        Self {
            created: 0.0,
            task: String::new(),
            banned_patterns: Vec::new(),
            obsolete_terms: Vec::new(),
            blocked_commands: Vec::new(),
            import_rules: Vec::new(),
            custom_checks: Vec::new(),
        }
    }
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
    /// Guard metadata -- populated by merge logic, not from ecko.yaml.
    #[serde(skip)]
    pub guard_meta: Option<GuardMeta>,
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
            guard_meta: None,
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

/// Load `ecko.yaml` from `cwd`, then merge `.ecko-guard.yaml` if present.
///
/// Returns `EckoConfig::default()` when `ecko.yaml` is absent or unparseable.
/// Guard rules are appended to the main config arrays; guard metadata is stored
/// in `guard_meta` for lifecycle detection (age nudge, friction).
pub fn load_config(cwd: &str) -> EckoConfig {
    let path = Path::new(cwd).join("ecko.yaml");
    let mut cfg = if !path.exists() {
        debug::debug("ecko.yaml not found, using defaults");
        EckoConfig::default()
    } else {
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                debug::debug(&format!("failed to read ecko.yaml: {e}"));
                return EckoConfig::default();
            }
        };
        match serde_yaml::from_str::<EckoConfig>(&contents) {
            Ok(c) => {
                debug::debug("ecko.yaml loaded successfully");
                c
            }
            Err(e) => {
                echo::emit(&format!(
                    "~~ ecko ~~ warning: failed to parse ecko.yaml: {e}"
                ));
                EckoConfig::default()
            }
        }
    };

    // Merge .ecko-guard.yaml if present
    merge_guard_config(&mut cfg, cwd);

    validate_custom_checks(&cfg);
    cfg
}

/// Load and merge `.ecko-guard.yaml` into the main config.
///
/// Appends guard rules to `banned_patterns`, `import_rules`, `custom_checks`,
/// and `blocked_commands`. Populates `guard_meta` with lifecycle metadata.
fn merge_guard_config(cfg: &mut EckoConfig, cwd: &str) {
    let guard_path = Path::new(cwd).join(".ecko-guard.yaml");
    if !guard_path.exists() {
        return;
    }

    let contents = match std::fs::read_to_string(&guard_path) {
        Ok(c) => c,
        Err(e) => {
            debug::debug(&format!("failed to read .ecko-guard.yaml: {e}"));
            return;
        }
    };

    let guard: EckoGuardConfig = match serde_yaml::from_str(&contents) {
        Ok(g) => g,
        Err(e) => {
            echo::emit(&format!(
                "~~ ecko ~~ warning: failed to parse .ecko-guard.yaml: {e}"
            ));
            return;
        }
    };

    // Collect guard check names for friction detection.
    // Names must match the actual check names emitted by the checks.
    let mut guard_check_names = HashSet::new();
    if !guard.import_rules.is_empty() {
        guard_check_names.insert("import-layers".to_string());
    }
    if !guard.banned_patterns.is_empty() {
        guard_check_names.insert("banned-patterns".to_string());
    }
    // Note: obsolete_terms is merged but has no Rust v2 check yet.
    // When a check is implemented, add its name to guard_check_names here.
    for cc in &guard.custom_checks {
        guard_check_names.insert(cc.name.clone());
    }

    // Merge rule arrays
    cfg.banned_patterns.extend(guard.banned_patterns);
    cfg.obsolete_terms.extend(guard.obsolete_terms);
    cfg.import_rules.extend(guard.import_rules);
    cfg.custom_checks.extend(guard.custom_checks);
    cfg.blocked_commands.extend(guard.blocked_commands);

    let check_count = guard_check_names.len();
    cfg.guard_meta = Some(GuardMeta {
        created: guard.created,
        task: guard.task,
        guard_check_names,
    });

    debug::debug(&format!(
        ".ecko-guard.yaml merged: {check_count} guard check types"
    ));
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

    // --- Guard config merge tests ---

    #[test]
    fn guard_merge_with_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let ecko_yaml = r#"
banned_patterns:
  - pattern: "TODO"
    message: "No TODOs"
"#;
        let guard_yaml = r#"
created: 1711111800.0
task: "auth-refactor"
banned_patterns:
  - pattern: "fetch\\("
    glob: "*.tsx"
    message: "Use hooks for API calls"
import_rules:
  - files: "components/*.tsx"
    deny: [api]
    message: "Components must not import api directly"
"#;
        std::fs::write(dir.path().join("ecko.yaml"), ecko_yaml).unwrap();
        std::fs::write(dir.path().join(".ecko-guard.yaml"), guard_yaml).unwrap();
        let cfg = load_config(dir.path().to_str().unwrap());

        // ecko.yaml pattern + guard pattern = 2 total
        assert_eq!(cfg.banned_patterns.len(), 2);
        // Guard import rules merged
        assert_eq!(cfg.import_rules.len(), 1);
        // Guard metadata populated
        let meta = cfg.guard_meta.as_ref().unwrap();
        assert_eq!(meta.task, "auth-refactor");
        assert!(meta.created > 0.0);
        assert!(meta.guard_check_names.contains("import-layers"));
        assert!(meta.guard_check_names.contains("banned-patterns"));
    }

    #[test]
    fn guard_merge_only_guard_file() {
        let dir = tempfile::tempdir().unwrap();
        let guard_yaml = r#"
created: 1711111800.0
task: "test-task"
blocked_commands:
  - pattern: "npm publish"
    message: "Do not publish during refactor"
"#;
        // No ecko.yaml -- only guard file
        std::fs::write(dir.path().join(".ecko-guard.yaml"), guard_yaml).unwrap();
        let cfg = load_config(dir.path().to_str().unwrap());

        assert_eq!(cfg.blocked_commands.len(), 1);
        assert!(cfg.guard_meta.is_some());
        assert_eq!(cfg.guard_meta.as_ref().unwrap().task, "test-task");
    }

    #[test]
    fn guard_merge_invalid_guard_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ecko.yaml"), "echo_cap_per_check: 3").unwrap();
        std::fs::write(dir.path().join(".ecko-guard.yaml"), "{{{{ not yaml").unwrap();
        let cfg = load_config(dir.path().to_str().unwrap());

        // ecko.yaml loaded fine, guard skipped gracefully
        assert_eq!(cfg.echo_cap_per_check, 3);
        assert!(cfg.guard_meta.is_none());
    }

    #[test]
    fn no_guard_meta_when_no_guard_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ecko.yaml"), "echo_cap_per_check: 7").unwrap();
        let cfg = load_config(dir.path().to_str().unwrap());

        assert_eq!(cfg.echo_cap_per_check, 7);
        assert!(cfg.guard_meta.is_none());
    }
}
