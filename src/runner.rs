//! Runner -- dispatches checks for each mode.
//!
//! `run_post_tool_use` implements the full Layer 1 + Layer 2 flow.
//! `run_stop` implements Layer 2 re-sweep across modified files.
//! `run_dry_run` lists applicable checks without executing.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rayon::prelude::*;

use crate::checks;
use crate::config;
use crate::debug;
use crate::echo::{self, Echo};
use crate::external;
use crate::formatter;
use crate::git;
use crate::hints;
use crate::lang::{self, Lang};
use crate::ledger;
use crate::suppress;

// ---------------------------------------------------------------------------
// EditContext -- diff-scoped checking
// ---------------------------------------------------------------------------

/// Edit tool context extracted from hook stdin JSON.
///
/// Contains the `new_string` from an Edit tool invocation. Used to compute
/// which lines the agent actually changed, so we only report echoes within
/// the changed range.
pub struct EditContext {
    pub new_string: String,
}

/// Compute the set of 1-based line numbers covered by `new_string` in `source`.
///
/// Finds the first occurrence of `new_string` in the post-edit source and returns
/// the line numbers it spans. Returns `None` if `new_string` is empty or not found
/// (defensive fallback: caller should not filter echoes).
fn compute_changed_lines(source: &str, edit: &EditContext) -> Option<HashSet<usize>> {
    if edit.new_string.is_empty() {
        // Deletion -- fall back to full-file checking. A deletion can expose bugs
        // (removing a guard clause makes subsequent code reachable, removing an
        // import makes its uses undefined).
        return None;
    }

    let start_byte = source.find(&edit.new_string)?;
    // end_byte is exclusive (one past last byte). Use saturating_sub(1) to get the
    // last inclusive byte so a trailing newline doesn't bleed into the next line.
    let last_byte = start_byte + edit.new_string.len().saturating_sub(1);

    // Build line-start offset table (same approach as universal.rs:build_line_starts).
    let mut line_starts = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }

    // Convert byte range to 1-based line numbers.
    let start_line = match line_starts.binary_search(&start_byte) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    };
    let end_line = match line_starts.binary_search(&last_byte) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    };

    let lines: HashSet<usize> = (start_line..=end_line).collect();
    Some(lines)
}

// ---------------------------------------------------------------------------
// StopResult -- structured output from run_stop_inner()
// ---------------------------------------------------------------------------

/// Structured result from stop-mode analysis.
///
/// Returned by `run_stop_inner()` so both the CLI hook and MCP tool
/// can consume the same data without reimplementing the logic.
pub struct StopResult {
    pub all_echoes: HashMap<String, Vec<Echo>>,
    pub elapsed: f64,
    pub corrections: HashMap<String, i32>,
    pub session_entries: Vec<ledger::LedgerEntry>,
    pub file_count: usize,
    pub config: config::EckoConfig,
}

/// Type alias for adapter thread results to reduce type complexity.
type AdapterResult = (&'static str, HashMap<String, Vec<Echo>>);

// ---------------------------------------------------------------------------
// Exclude directories
// ---------------------------------------------------------------------------

const DEFAULT_EXCLUDE_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    "dist",
    "build",
    ".next",
    ".nuxt",
    "coverage",
    "vendor",
    ".venv",
    "venv",
    "target",
    ".tox",
    "fixtures",
    "__fixtures__",
    "__snapshots__",
    ".ecko-reverb",
    ".ecko-session",
];

/// Build a compiled GlobSet from user exclude patterns.
///
/// Returns `None` if the list is empty or all patterns are invalid.
pub fn build_exclude_set(user_excludes: &[String]) -> Option<globset::GlobSet> {
    if user_excludes.is_empty() {
        return None;
    }
    let mut builder = globset::GlobSetBuilder::new();
    for pat in user_excludes {
        if let Ok(glob) = globset::Glob::new(pat) {
            builder.add(glob);
        }
    }
    builder.build().ok()
}

/// Check if a file should be excluded from linting.
///
/// A file is excluded if:
/// - Any path component matches a default exclude directory name.
/// - The file matches a user-configured glob pattern in `exclude`.
pub fn is_excluded(file_path: &str, cwd: &str, user_excludes: &[String]) -> bool {
    let set = build_exclude_set(user_excludes);
    is_excluded_with_globset(file_path, cwd, set.as_ref())
}

/// Check exclusion with a pre-built GlobSet (avoids recompiling per file).
pub fn is_excluded_with_globset(
    file_path: &str,
    cwd: &str,
    exclude_set: Option<&globset::GlobSet>,
) -> bool {
    // Check default exclude directories: any path component match.
    let rel = Path::new(file_path)
        .strip_prefix(cwd)
        .unwrap_or_else(|_| Path::new(file_path));

    for component in rel.components() {
        if let std::path::Component::Normal(seg) = component {
            let seg_str = seg.to_string_lossy();
            if DEFAULT_EXCLUDE_DIRS.iter().any(|&d| d == seg_str.as_ref()) {
                debug::debug(&format!("excluded (default dir): {file_path}"));
                return true;
            }
        }
    }

    // Check user-configured glob patterns.
    if let Some(set) = exclude_set {
        let rel_str = rel.to_string_lossy();
        if set.is_match(rel_str.as_ref()) {
            debug::debug(&format!("excluded (user glob): {file_path}"));
            return true;
        }
    }

    false
}

// ---------------------------------------------------------------------------
// PostToolUse
// ---------------------------------------------------------------------------

/// PostToolUse mode -- per-file Layer 1 + Layer 2 checks after Write/Edit.
///
/// When `edit_ctx` is `Some`, only echoes on changed lines are reported (diff-scoped).
/// When `edit_ctx` is `None` (Write tool or missing context), all echoes are reported.
///
/// Returns 1 if echoes were found, 0 if clean.
pub fn run_post_tool_use(
    file_path: &str,
    cwd: &str,
    plugin_root: &str,
    edit_ctx: Option<EditContext>,
) -> i32 {
    debug::debug(&format!(
        "run_post_tool_use: file={file_path}, cwd={cwd}, plugin_root={plugin_root}, has_edit_ctx={}",
        edit_ctx.is_some()
    ));

    // 1. Load config
    let config = config::load_config(cwd);
    let disabled = config::get_disabled_checks(&config);
    let json_output = config::is_output_json(&config);

    // 2. Check file exists
    if !Path::new(file_path).exists() {
        debug::debug(&format!("file does not exist: {file_path}"));
        return 0;
    }

    // 3. Skip stubs (.pyi, .test-d.ts)
    if lang::is_skippable_stub(file_path) {
        debug::debug(&format!("skipping stub: {file_path}"));
        return 0;
    }

    // 4. Skip excluded files
    if is_excluded(file_path, cwd, &config.exclude) {
        return 0;
    }

    // 5. Detect language
    let language = lang::detect_language(file_path);
    debug::debug(&format!("detected language: {language:?}"));

    // 6. Read source
    let _source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            debug::debug(&format!("failed to read file: {e}"));
            return 0;
        }
    };

    // 7. Layer 1: autofix (if configured)
    formatter::autofix(file_path, language, &config);

    // Re-read source after autofix (formatters may have modified it).
    let source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            debug::debug(&format!("failed to re-read file after autofix: {e}"));
            return 0;
        }
    };

    // 7a. Compute changed lines for diff-scoped checking (after autofix re-read).
    let changed_lines = edit_ctx
        .as_ref()
        .and_then(|ctx| compute_changed_lines(&source, ctx));
    if let Some(ref lines) = changed_lines {
        debug::debug(&format!("diff-scope: changed lines = {:?}", lines));
    }

    // 8. Layer 2: run checks
    let echoes = checks::run_layer2_checks(file_path, language, &source, cwd, &config);

    // 8a. Diff-scope filter: only keep echoes on changed lines (Edit tool).
    let echoes = if let Some(ref lines) = changed_lines {
        let before = echoes.len();
        let filtered: Vec<Echo> = echoes
            .into_iter()
            .filter(|e| lines.contains(&e.line))
            .collect();
        debug::debug(&format!(
            "diff-scope: {} echoes filtered to {} on changed lines",
            before,
            filtered.len()
        ));
        filtered
    } else {
        echoes
    };

    // 9. Filter suppressed (ecko:ignore)
    let echoes = suppress::filter_suppressed(echoes, file_path);

    // 10. Filter disabled checks
    let mut echoes: Vec<Echo> = echoes
        .into_iter()
        .filter(|e| !disabled.contains(&e.check))
        .collect();

    // 10a. Enrich echoes with contextual suggestions.
    for e in &mut echoes {
        if e.suggestion.is_empty() {
            let hint = hints::contextual_suggestion(&e.check, file_path, &e.message);
            if !hint.is_empty() {
                e.suggestion = hint;
            }
        }
    }

    // 10b. Strip fix suggestions if disabled in config.
    if !config.fix_suggestions {
        echoes.iter_mut().for_each(|e| e.fix = None);
    }

    // 10c. Apply per-check echo cap.
    let echoes = echo::apply_per_check_cap(echoes, config.echo_cap_per_check);

    // 11. Record to session ledger (best-effort, never blocks the hook)
    if config.session_hours > 0.0 {
        let mut echo_counts: HashMap<String, usize> = HashMap::new();
        for e in &echoes {
            *echo_counts.entry(e.check.clone()).or_insert(0) += 1;
        }
        ledger::append(cwd, file_path, "post-tool-use", &echo_counts);
    }

    // 12. Output
    if echoes.is_empty() {
        debug::debug("clean -- 0 echoes");
        return 0;
    }

    let rel = relative_path(file_path, cwd);

    if json_output {
        let output = echo::format_file_echoes_json(&rel, &echoes, &[]);
        echo::emit(&output);
    } else {
        let output = echo::format_file_echoes(&rel, &echoes);
        echo::emit(&output);
    }

    // 12a. Session pattern detection: emit behavioral directives AFTER echoes
    // so the agent sees the immediate fix first, then the behavioral nudge.
    if config.pattern_threshold > 0 && config.session_hours > 0.0 {
        if let Some(directives) = detect_session_patterns(cwd, &rel, &config) {
            for d in directives {
                echo::emit(&d);
            }
        }
    }

    1
}

// ---------------------------------------------------------------------------
// Session pattern detection
// ---------------------------------------------------------------------------

/// Detect checks that have fired on `threshold` distinct files in this session.
///
/// Returns formatted directive strings for checks that hit exactly the threshold
/// (emits once per check per session). Returns `None` on ledger read failure.
fn detect_session_patterns(
    cwd: &str,
    current_file: &str,
    config: &config::EckoConfig,
) -> Option<Vec<String>> {
    let entries = ledger::read_session(cwd, config.session_hours);
    if entries.is_empty() {
        return None;
    }

    let threshold = config.pattern_threshold;

    // Count distinct files per check (exclude current file -- it was just appended).
    let mut check_files: HashMap<String, HashSet<String>> = HashMap::new();
    for entry in &entries {
        if entry.mode != "post-tool-use" || entry.file == current_file {
            continue;
        }
        for check_name in entry.echoes.keys() {
            check_files
                .entry(check_name.clone())
                .or_default()
                .insert(entry.file.clone());
        }
    }

    let mut directives = Vec::new();
    for (check, files) in &check_files {
        if files.len() == threshold {
            let hint = hints::pattern_directive(check);
            directives.push(format!(
                "~~ ecko ~~ pattern: '{}' flagged in {} files this session -- {}",
                check, threshold, hint
            ));
        }
    }

    if directives.is_empty() {
        None
    } else {
        directives.sort(); // Deterministic output order
        Some(directives)
    }
}

// ---------------------------------------------------------------------------
// Stop
// ---------------------------------------------------------------------------

/// Stop mode -- deep analysis across modified files.
///
/// Thin wrapper over `run_stop_inner()` that formats output and returns an exit code.
pub fn run_stop(cwd: &str, plugin_root: &str, files_override: Option<Vec<String>>) -> i32 {
    debug::debug(&format!(
        "run_stop: cwd={cwd}, plugin_root={plugin_root}, files_override={files_override:?}"
    ));

    let result = run_stop_inner(cwd, files_override);

    let json_output = config::is_output_json(&result.config);
    let cross_cap = result.config.echo_cap_cross_file;

    // Session ledger: self-correction + session stats
    let correction_line = echo::format_correction_summary(&result.corrections);
    let session_line = if !result.session_entries.is_empty() {
        let json_values = ledger::entries_to_json_values(&result.session_entries);
        echo::format_session_stats(&json_values, &result.corrections)
    } else {
        String::new()
    };

    // Format and emit output
    if json_output {
        let output = echo::format_stop_echoes_json(
            &result.all_echoes,
            result.elapsed,
            &[],
            &result.corrections,
        );
        echo::emit(&output);
        emit_guard_lifecycle(&result);
        return if result.all_echoes.is_empty() { 0 } else { 1 };
    }

    if result.all_echoes.is_empty() {
        if result.file_count > 0 {
            let file_word = if result.file_count == 1 {
                "file"
            } else {
                "files"
            };
            echo::emit(&format!(
                "~~ ecko ~~ clean sweep \u{2014} 0 echoes across {} {file_word} ({:.1}s)",
                result.file_count, result.elapsed
            ));
        }
        if !correction_line.is_empty() {
            echo::emit(&correction_line);
        }
        if !session_line.is_empty() {
            echo::emit(&session_line);
        }
        emit_guard_lifecycle(&result);
        return 0;
    }

    let output = echo::format_stop_echoes(&result.all_echoes, cross_cap);
    echo::emit(&output);
    echo::emit(&format!("~~ ecko ~~ finished in {:.1}s", result.elapsed));
    if !correction_line.is_empty() {
        echo::emit(&correction_line);
    }
    if !session_line.is_empty() {
        echo::emit(&session_line);
    }
    emit_guard_lifecycle(&result);

    1
}

// ---------------------------------------------------------------------------
// Guard lifecycle (age nudge + friction detection)
// ---------------------------------------------------------------------------

/// Emit guard lifecycle warnings when `.ecko-guard.yaml` is active.
///
/// 1. Age nudge: warns when guard file is older than 7 days.
/// 2. Friction detection: warns when guard-sourced checks fire on 3+ files
///    in the current session (signal that rules may be stale).
fn emit_guard_lifecycle(result: &StopResult) {
    let meta = match &result.config.guard_meta {
        Some(m) => m,
        None => return,
    };

    // --- Age nudge ---
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    if meta.created > 0.0 {
        let age_days = ((now - meta.created) / 86400.0) as i64;
        if age_days >= 7 {
            let task_info = if meta.task.is_empty() {
                String::new()
            } else {
                format!(" (task: {})", meta.task)
            };
            echo::emit(&format!(
                "~~ ecko ~~ note: .ecko-guard.yaml is {age_days} days old{task_info}. Run /ecko:guard --review or --clear."
            ));
        }
    }

    // --- Friction detection ---
    if result.session_entries.is_empty() || meta.guard_check_names.is_empty() {
        return;
    }

    let mut check_files: HashMap<String, HashSet<String>> = HashMap::new();
    for entry in &result.session_entries {
        if entry.mode == "post-tool-use" {
            for check_name in entry.echoes.keys() {
                if meta.guard_check_names.contains(check_name) {
                    check_files
                        .entry(check_name.clone())
                        .or_default()
                        .insert(entry.file.clone());
                }
            }
        }
    }

    let mut friction_checks: Vec<&String> = check_files
        .iter()
        .filter(|(_, files)| files.len() >= 3)
        .map(|(check, _)| check)
        .collect();

    if !friction_checks.is_empty() {
        friction_checks.sort();
        echo::emit(&format!(
            "~~ ecko ~~ note: guard rule(s) fired on 3+ files this session: {}. Still relevant? /ecko:guard --review",
            friction_checks.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
}

/// Core stop-mode logic -- returns structured `StopResult`.
///
/// Used by both the CLI hook (`run_stop`) and the MCP tool (`check_workspace`).
/// Single codepath for all workspace-level checking.
pub fn run_stop_inner(cwd: &str, files_override: Option<Vec<String>>) -> StopResult {
    let t0 = std::time::Instant::now();

    // --- 1. Load config once ---
    let config = config::load_config(cwd);
    let disabled = config::get_disabled_checks(&config);
    let session_hours = config.session_hours;

    // --- 2. Get modified files ---
    let raw_files: Vec<String> = if let Some(override_files) = files_override.as_ref() {
        override_files
            .iter()
            .map(|f| git::normalize_path(f, cwd))
            .collect()
    } else {
        git::get_modified_files(cwd, session_hours)
    };

    // --- 3. Read session ledger once (used for scoping AND corrections/stats) ---
    let session_entries = if session_hours > 0.0 {
        ledger::read_session(cwd, session_hours)
    } else {
        Vec::new()
    };

    // Scope to ledger-tracked files when available.
    // Prevents flooding with pre-existing issues on first use of existing projects:
    // get_modified_files includes git log --since files the agent never touched.
    let raw_files = if files_override.is_none() && !session_entries.is_empty() {
        let ledger_files: HashSet<String> = session_entries
            .iter()
            .filter(|e| e.mode == "post-tool-use" && !e.file.is_empty())
            .map(|e| git::normalize_path(&e.file, cwd))
            .collect();

        if ledger_files.is_empty() {
            raw_files
        } else {
            let scoped: Vec<String> = raw_files
                .into_iter()
                .filter(|f| ledger_files.contains(f))
                .collect();
            debug::debug(&format!(
                "stop: scoped to {} ledger-tracked files (from {} ledger entries)",
                scoped.len(),
                ledger_files.len()
            ));
            scoped
        }
    } else {
        raw_files
    };

    // --- 4. Filter excluded files and stubs ---
    let exclude_set = build_exclude_set(&config.exclude);
    let modified: Vec<String> = raw_files
        .into_iter()
        .filter(|f| {
            Path::new(f).is_file()
                && !lang::is_skippable_stub(f)
                && !is_excluded_with_globset(f, cwd, exclude_set.as_ref())
        })
        .collect();

    if modified.is_empty() {
        debug::debug("stop: no modified files after filtering");
        return StopResult {
            all_echoes: HashMap::new(),
            elapsed: t0.elapsed().as_secs_f64(),
            corrections: HashMap::new(),
            session_entries,
            file_count: 0,
            config,
        };
    }

    debug::debug(&format!(
        "stop: {} modified files after filtering",
        modified.len()
    ));

    // --- 5. Run Layer 2 checks in parallel (rayon) ---
    let layer2_results: Vec<(String, Vec<Echo>)> = modified
        .par_iter()
        .filter_map(|file_path| {
            let source = match std::fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(_) => return None,
            };
            let language = lang::detect_language(file_path);
            let echoes = checks::run_layer2_checks(file_path, language, &source, cwd, &config);
            if echoes.is_empty() {
                None
            } else {
                let norm = git::normalize_path(file_path, cwd);
                Some((norm, echoes))
            }
        })
        .collect();

    // Merge Layer 2 results into all_echoes
    let mut all_echoes: HashMap<String, Vec<Echo>> = HashMap::new();
    for (path, echoes) in layer2_results {
        all_echoes.entry(path).or_default().extend(echoes);
    }

    // --- 6. External adapters (Layer 3) ---
    let adapter_echoes = run_external_adapters(&modified, cwd, &config);
    for (file, echoes) in adapter_echoes {
        all_echoes.entry(file).or_default().extend(echoes);
    }

    // --- 6b. Dead code analysis (cross-file, replaces vulture/knip) ---
    {
        let dead_code_results = checks::dead_code::run_dead_code_analysis(&modified, cwd, &config);
        for (file, echoes) in dead_code_results {
            all_echoes.entry(file).or_default().extend(echoes);
        }
    }

    // --- 7. Deduplicate echoes per file (same check + line) ---
    for echoes in all_echoes.values_mut() {
        let mut seen = HashSet::new();
        echoes.retain(|e| seen.insert((e.check.clone(), e.line)));
    }

    // Apply suppression, exclusion, and disabled-check filters
    let keys: Vec<String> = all_echoes.keys().cloned().collect();
    for path in keys {
        if is_excluded_with_globset(&path, cwd, exclude_set.as_ref()) {
            all_echoes.remove(&path);
            continue;
        }
        if let Some(echoes) = all_echoes.get_mut(&path) {
            *echoes = suppress::filter_suppressed(std::mem::take(echoes), &path);
            echoes.retain(|e| !disabled.contains(&e.check));
            if echoes.is_empty() {
                all_echoes.remove(&path);
            }
        }
    }

    // --- 7a. Strip fix suggestions if disabled in config ---
    if !config.fix_suggestions {
        for echoes in all_echoes.values_mut() {
            echoes.iter_mut().for_each(|e| e.fix = None);
        }
    }

    // --- 7b. Apply per-check cap per file ---
    if config.echo_cap_per_check > 0 {
        for echoes in all_echoes.values_mut() {
            *echoes = echo::apply_per_check_cap(std::mem::take(echoes), config.echo_cap_per_check);
        }
        all_echoes.retain(|_, v| !v.is_empty());
    }

    // --- 8. Compute timing + corrections ---
    let elapsed = t0.elapsed().as_secs_f64();
    let corrections = if !session_entries.is_empty() {
        ledger::compute_self_corrections(&session_entries)
    } else {
        HashMap::new()
    };
    let file_count = modified.len();

    StopResult {
        all_echoes,
        elapsed,
        corrections,
        session_entries,
        file_count,
        config,
    }
}

// ---------------------------------------------------------------------------
// External adapters (Layer 3)
// ---------------------------------------------------------------------------

/// Spawn external adapter tools in parallel, collect results.
///
/// Each adapter runs in its own thread. Results are merged into a single map.
fn run_external_adapters(
    modified: &[String],
    cwd: &str,
    config: &config::EckoConfig,
) -> HashMap<String, Vec<Echo>> {
    let has_python = modified
        .iter()
        .any(|f| lang::detect_language(f) == Lang::Python);
    let has_typescript = modified
        .iter()
        .any(|f| matches!(lang::detect_language(f), Lang::TypeScript | Lang::Tsx));
    let has_go = modified
        .iter()
        .any(|f| lang::detect_language(f) == Lang::Go);
    let has_rust = modified
        .iter()
        .any(|f| lang::detect_language(f) == Lang::Rust);

    let mut handles: Vec<std::thread::JoinHandle<AdapterResult>> = Vec::new();

    // Pyright -- Python type checking
    if has_python && config::is_deep_enabled(config, "pyright") {
        let py_files: Vec<String> = modified
            .iter()
            .filter(|f| lang::detect_language(f) == Lang::Python)
            .cloned()
            .collect();
        let cwd_c = cwd.to_string();
        handles.push(std::thread::spawn(move || {
            ("pyright", external::pyright::run_pyright(&py_files, &cwd_c))
        }));
    } else if has_python {
        debug::debug("external: pyright skipped (deep_analysis.pyright not enabled)");
    }

    // tsc -- TypeScript type checking
    if has_typescript && config::is_deep_enabled(config, "tsc") {
        let cwd_c = cwd.to_string();
        let modified_set: std::collections::HashSet<String> = modified.iter().cloned().collect();
        handles.push(std::thread::spawn(move || {
            let results = external::tsc::run_tsc(&cwd_c);
            // Post-filter to modified files only (tsc reports all project errors)
            let filtered: HashMap<String, Vec<Echo>> = results
                .into_iter()
                .filter(|(path, _)| {
                    let abs_path = if std::path::Path::new(path).is_absolute() {
                        path.clone()
                    } else {
                        format!("{}/{}", cwd_c, path)
                    };
                    modified_set.contains(path) || modified_set.contains(&abs_path)
                })
                .collect();
            ("tsc", filtered)
        }));
    } else if has_typescript {
        debug::debug("external: tsc skipped (deep_analysis.tsc not enabled)");
    }

    // golangci-lint -- Go linting
    if has_go && config::is_deep_enabled(config, "golangci-lint") {
        let cwd_c = cwd.to_string();
        let modified_c: Vec<String> = modified.to_vec();
        handles.push(std::thread::spawn(move || {
            (
                "golangci-lint",
                external::golangci::run_golangci(&cwd_c, &modified_c),
            )
        }));
    } else if has_go {
        debug::debug("external: golangci-lint skipped (deep_analysis.golangci-lint not enabled)");
    }

    // clippy -- Rust linting
    if has_rust && config::is_deep_enabled(config, "clippy") {
        let cwd_c = cwd.to_string();
        let modified_c: Vec<String> = modified.to_vec();
        handles.push(std::thread::spawn(move || {
            ("clippy", external::clippy::run_clippy(&cwd_c, &modified_c))
        }));
    } else if has_rust {
        debug::debug("external: clippy skipped (deep_analysis.clippy not enabled)");
    }

    // Collect results from all adapter threads
    let mut all_echoes: HashMap<String, Vec<Echo>> = HashMap::new();
    for handle in handles {
        match handle.join() {
            Ok((tool_name, results)) => {
                if results.is_empty() {
                    debug::debug(&format!("external: {tool_name} returned 0 echoes"));
                } else {
                    debug::debug(&format!(
                        "external: {tool_name} returned echoes in {} files",
                        results.len()
                    ));
                }
                for (file, echoes) in results {
                    all_echoes.entry(file).or_default().extend(echoes);
                }
            }
            Err(_) => {
                debug::debug("external: adapter thread panicked");
            }
        }
    }
    all_echoes
}

// ---------------------------------------------------------------------------
// DryRun
// ---------------------------------------------------------------------------

/// DryRun mode -- list applicable checks without running any tools.
///
/// Prints to stdout (informational, not a hook).
pub fn run_dry_run(file_path: &str, cwd: &str, plugin_root: &str) -> i32 {
    debug::debug(&format!(
        "run_dry_run: file={file_path}, cwd={cwd}, plugin_root={plugin_root}"
    ));

    let config = config::load_config(cwd);
    let disabled = config::get_disabled_checks(&config);

    if !Path::new(file_path).exists() {
        println!("ecko dry-run: file not found: {file_path}");
        return 0;
    }

    if lang::is_skippable_stub(file_path) {
        println!("ecko dry-run: skipped (stub file): {file_path}");
        return 0;
    }

    if is_excluded(file_path, cwd, &config.exclude) {
        println!("ecko dry-run: excluded: {file_path}");
        return 0;
    }

    let language = lang::detect_language(file_path);
    let rel = relative_path(file_path, cwd);
    println!("ecko dry-run: {rel} ({language:?})");

    let applicable = checks::list_applicable_checks(language);
    if applicable.is_empty() {
        println!("  (no built-in checks for this language)");
    } else {
        for check_name in &applicable {
            let status = if disabled.contains(check_name) {
                "disabled"
            } else {
                "enabled"
            };
            println!("  [{status}] {check_name}");
        }
    }

    0
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn relative_path(file_path: &str, cwd: &str) -> String {
    git::relative_path(file_path, cwd)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- compute_changed_lines tests ---

    #[test]
    fn test_changed_lines_simple_edit() {
        let source = "line1\nline2\nline3\nline4\nline5\n";
        let edit = EditContext {
            new_string: "line3".to_string(),
        };
        let lines = compute_changed_lines(source, &edit).unwrap();
        assert!(lines.contains(&3));
        assert!(!lines.contains(&2));
        assert!(!lines.contains(&4));
    }

    #[test]
    fn test_changed_lines_multiline_edit() {
        let source = "aaa\nbbb\nccc\nddd\neee\n";
        let edit = EditContext {
            new_string: "bbb\nccc\nddd".to_string(),
        };
        let lines = compute_changed_lines(source, &edit).unwrap();
        assert!(lines.contains(&2));
        assert!(lines.contains(&3));
        assert!(lines.contains(&4));
        assert!(!lines.contains(&1));
        assert!(!lines.contains(&5));
    }

    #[test]
    fn test_changed_lines_edit_at_start() {
        let source = "NEW\nline2\nline3\n";
        let edit = EditContext {
            new_string: "NEW".to_string(),
        };
        let lines = compute_changed_lines(source, &edit).unwrap();
        assert!(lines.contains(&1));
        assert!(!lines.contains(&2));
    }

    #[test]
    fn test_changed_lines_edit_at_end() {
        let source = "line1\nline2\nNEW\n";
        let edit = EditContext {
            new_string: "NEW".to_string(),
        };
        let lines = compute_changed_lines(source, &edit).unwrap();
        assert!(lines.contains(&3));
        assert!(!lines.contains(&2));
    }

    #[test]
    fn test_changed_lines_empty_new_string_falls_back() {
        let source = "line1\nline2\n";
        let edit = EditContext {
            new_string: String::new(),
        };
        assert!(
            compute_changed_lines(source, &edit).is_none(),
            "deletion should return None (full-file fallback)"
        );
    }

    #[test]
    fn test_changed_lines_not_found() {
        let source = "line1\nline2\n";
        let edit = EditContext {
            new_string: "NONEXISTENT".to_string(),
        };
        assert!(
            compute_changed_lines(source, &edit).is_none(),
            "should return None when new_string not found"
        );
    }

    #[test]
    fn test_changed_lines_multiple_occurrences_uses_first() {
        let source = "dup\nline2\ndup\n";
        let edit = EditContext {
            new_string: "dup".to_string(),
        };
        let lines = compute_changed_lines(source, &edit).unwrap();
        assert!(lines.contains(&1), "should match first occurrence");
        // Line 3 also has "dup" but we should only match line 1
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_changed_lines_trailing_newline_no_bleed() {
        // "aaa\n" covers only line 1, should NOT include line 2
        let source = "aaa\nbbb\nccc\n";
        let edit = EditContext {
            new_string: "aaa\n".to_string(),
        };
        let lines = compute_changed_lines(source, &edit).unwrap();
        assert!(lines.contains(&1));
        assert!(
            !lines.contains(&2),
            "trailing newline must not bleed into next line"
        );
    }

    #[test]
    fn test_changed_lines_single_char() {
        let source = "a\nb\nc\n";
        let edit = EditContext {
            new_string: "b".to_string(),
        };
        let lines = compute_changed_lines(source, &edit).unwrap();
        assert_eq!(lines, HashSet::from([2]));
    }

    // --- detect_session_patterns tests ---

    #[test]
    fn test_detect_patterns_below_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_str().unwrap();

        // Record 2 files with bare-except (below threshold of 3)
        let mut echoes = HashMap::new();
        echoes.insert("bare-except".to_string(), 1_usize);
        ledger::append(cwd, &format!("{}/a.py", cwd), "post-tool-use", &echoes);
        ledger::append(cwd, &format!("{}/b.py", cwd), "post-tool-use", &echoes);

        let config = config::EckoConfig {
            pattern_threshold: 3,
            ..config::EckoConfig::default()
        };
        let result = detect_session_patterns(cwd, "c.py", &config);
        assert!(
            result.is_none(),
            "2 files < threshold 3, no directive expected"
        );
    }

    #[test]
    fn test_detect_patterns_at_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_str().unwrap();

        let mut echoes = HashMap::new();
        echoes.insert("bare-except".to_string(), 1_usize);
        ledger::append(cwd, &format!("{}/a.py", cwd), "post-tool-use", &echoes);
        ledger::append(cwd, &format!("{}/b.py", cwd), "post-tool-use", &echoes);
        ledger::append(cwd, &format!("{}/c.py", cwd), "post-tool-use", &echoes);

        let config = config::EckoConfig {
            pattern_threshold: 3,
            ..config::EckoConfig::default()
        };
        // current_file is "d.py" (not in the ledger, so all 3 count)
        let result = detect_session_patterns(cwd, "d.py", &config);
        assert!(
            result.is_some(),
            "3 files == threshold 3, directive expected"
        );
        let directives = result.unwrap();
        assert_eq!(directives.len(), 1);
        assert!(directives[0].contains("bare-except"));
        assert!(directives[0].contains("pattern"));
    }

    #[test]
    fn test_detect_patterns_above_threshold_no_repeat() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_str().unwrap();

        let mut echoes = HashMap::new();
        echoes.insert("bare-except".to_string(), 1_usize);
        for name in &["a.py", "b.py", "c.py", "d.py"] {
            ledger::append(cwd, &format!("{}/{}", cwd, name), "post-tool-use", &echoes);
        }

        let config = config::EckoConfig {
            pattern_threshold: 3,
            ..config::EckoConfig::default()
        };
        let result = detect_session_patterns(cwd, "e.py", &config);
        assert!(
            result.is_none(),
            "4 files > threshold 3, directive should NOT repeat"
        );
    }

    #[test]
    fn test_detect_patterns_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_str().unwrap();

        let mut echoes = HashMap::new();
        echoes.insert("bare-except".to_string(), 1_usize);
        for name in &["a.py", "b.py", "c.py"] {
            ledger::append(cwd, &format!("{}/{}", cwd, name), "post-tool-use", &echoes);
        }

        let config = config::EckoConfig {
            pattern_threshold: 0,
            ..config::EckoConfig::default()
        };
        // threshold 0 means caller should never call detect_session_patterns,
        // but if called, it uses 0 which no check can match
        let result = detect_session_patterns(cwd, "d.py", &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_patterns_excludes_current_file() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_str().unwrap();

        let mut echoes = HashMap::new();
        echoes.insert("bare-except".to_string(), 1_usize);
        ledger::append(cwd, &format!("{}/a.py", cwd), "post-tool-use", &echoes);
        ledger::append(cwd, &format!("{}/b.py", cwd), "post-tool-use", &echoes);
        // Third entry is the current file -- should be excluded
        ledger::append(cwd, &format!("{}/c.py", cwd), "post-tool-use", &echoes);

        let config = config::EckoConfig {
            pattern_threshold: 3,
            ..config::EckoConfig::default()
        };
        let result = detect_session_patterns(cwd, "c.py", &config);
        assert!(
            result.is_none(),
            "only 2 files after excluding current file, below threshold"
        );
    }
}
