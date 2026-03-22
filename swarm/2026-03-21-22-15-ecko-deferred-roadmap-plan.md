# Swarm: ecko-deferred-roadmap-plan

**Date**: 2026-03-21 22:15
**Configuration**: 7 diverge → 4 synthesis → 1 arbiter
**Lenses**: Pragmatist, Critic, Architect, Contrarian, Completionist, User, Minimalist

---

## Input

### Task
Design a detailed, implementation-ready plan for all remaining deferred roadmap items for the ecko Claude Code plugin. Do NOT defer any item unless it is genuinely irrelevant to ecko's mission.

### Goal
A complete, implementation-ready plan covering all 11 deferred items, grouped into logical milestones with clear ordering, specific file changes, code patterns, and testing strategies.

### Context
Ecko v0.9.1: 448 tests, runner.py 617 lines, 6 tool adapters, 7 custom checks, zero Python dependencies. Three-layer architecture. Deferred items from v0.9.0 swarm include biome configurability, runner decomposition, session command, severity levels, JSON output, Go/Rust support, and more.

---

## Phase 1: Independent Exploration

### Agent 1 — The Pragmatist
**Theme: "Ship What Matters"**
5 milestones (v1.0-v1.4), ~547 total tests. v1.0 Session Dashboard (classify.py extraction, /ecko:session, session stats in stop, dry-run). v1.1 Configurability (biome both approaches, ruff_use_project_config, ruff_disabled_rules post-filter). v1.2 Machine-Readable (severity with user-configurable defaults, JSON output). v1.3 New Languages (Go/Rust Layer 3). v1.4 Intelligence (fingerprint.py with dep file scanning). Key: biome gets both project config + extra_rules. Severity has _DEFAULT_SEVERITY map with check_severity config. Go/Rust adapters with separate resolve functions.

### Agent 2 — The Critic
**Theme: "What Could Go Wrong"**
4 milestones, ~534 total tests. CRITICAL: ruff_use_project_config must always --no-fix (project config fix=true creates infinite hook loop). Biome project detection ONLY (no extra_rules — temp config merge is fragile). Severity should DEFER until after JSON ships (touches 13 files). JSON needs OutputCollector pattern. Go/Rust: BOTH Layer 3 only (per-file vs per-package mismatch is fundamental for golangci-lint). Empty --select edge case for ruff_disabled_rules. YAML parser is a "ticking time bomb" — all new keys must be flat.

### Agent 3 — The Architect
**Theme: "Structure for the Long Term"**
4 milestones, ~644 total tests. 5-concern model: orchestration, detection, filtering, presentation, memory. filtering.py extraction (~120 lines). biome gets BOTH approaches (project config + extra_rules via temp config in .ecko-session/biome-config/). Severity default "warn", user overrides via severity_overrides mapping. JSON with output_format config key. Runner trajectory tracking: 617→470→535→575→605. Detailed file change matrix across milestones.

### Agent 4 — The Contrarian
**Theme: "Challenge Every Assumption"**
4 milestones, ~525 total tests. Key contrarian takes: biome via CLI --rule flags (third approach nobody else considered). NO user severity config — "severity is ecko's opinion." Replace ruff_disabled_rules with source_code field on Echo + expand disabled_checks to match raw codes. JSON output should ship BEFORE severity. /ecko:session outputs JSON to stdout. Fingerprint as project.py feeding /ecko:tune only, never auto-config. Ecko's value is the echo-correction loop, not the specific rules.

### Agent 5 — The Completionist
**Theme: "Cover Every Edge Case"**
4 milestones, ~554 total tests. Most thorough on edge cases: encoding="utf-8" on all new open() calls, re-export backward compat, _KNOWN_KEYS updates for every key, golangci-lint capital-I "Issues" in JSON, clippy streaming JSON (one object per line). severity_threshold config for filtering (show only warn+error). ruff_disabled_rules with _RUFF_RULE_RE validation. Fingerprint with _MARKERS list and suggestions dict feeding /ecko:tune and /ecko:status. Suggested show_severity toggle.

### Agent 6 — The User
**Theme: "What Does the User Actually Need?"**
4 milestones, ~572 total tests. Key UX insights: empty session should say "No session data yet" not show zeros. ruff_use_project_config should emit note when no ruff config found. biome flood warning needed in docs. ruff_disabled_rules via --extend-ignore (operates at ruff CLI level). severity_overrides as list-of-dicts (like banned_patterns). JSON should NOT apply echo caps (caps are display-only). Separate session_stats.py module for session command.

### Agent 7 — The Minimalist
**Theme: "Subtract to Multiply"**
3 milestones, ~512 total tests. ELIMINATED 3 items: Runner decomposition (617 lines is fine, add section comments only). ruff_disabled_rules (disabled_checks already works, document the path). Project fingerprinting (no behavior to customize, just expand vulture _ALWAYS_SKIP ~3 lines). severity_overrides flat mapping with has_errors() helper. biome project detection ONLY. Total ~427 new lines across all releases. Strongest argument: "resist scope creep, ship minimal version."

---

## Phase 2: Synthesis

### Synthesizer 1
**Theme: "Minimal Config Surface"**
4 milestones (v1.0-v1.3), ~526 total tests, only 2 new config keys. DESCOPED runner decomposition to section comments. ELIMINATED ruff_disabled_rules (source_code on Echo + disabled_checks handles it). NO severity user config. Key insight: source_code field on Echo enables disabled_checks to match raw ruff codes without a new config key. Runner trajectory: 617→680 lines. New files: commands/session.md, checks/fingerprint.py, 2 Go/Rust adapters, 2 fixture files.

### Synthesizer 2
**Theme: "Balanced Completeness"**
3 milestones (v1.0-v1.2), ~550 total tests, 5 new config keys. INCLUDED runner decomposition as filtering.py. ruff_disabled_rules as post-filter with rule_code on Echo. severity.py with overrides parsed as "key: value" strings (no YAML parser changes). Detailed session_stats.py module. Project fingerprinting with _FRAMEWORK_VULTURE_SKIPS. Echo dataclass gains rule_code and severity fields. 7 new production files, 3 new test files.

### Synthesizer 3
**Theme: "Architecture First"**
4 milestones (v1.0-v1.3), ~560 total tests, 6 new config keys. Severity + JSON in FIRST milestone (they're coupled). ruff_disabled_rules via --extend-ignore (prevents ruff from even reporting). severity_overrides as list-of-dicts (proven pattern from banned_patterns). severity_threshold config. Session command deferred to LAST milestone. Most detailed risk register. Runner trajectory: 617→530→575→610→610.

### Synthesizer 4
**Theme: "Session Intelligence First"**
4 milestones (v1.0-v1.3), ~628 total tests. ELIMINATED ruff_disabled_rules (document disabled_checks instead). Session + ruff config + dry-run in first milestone. JSON before severity (Contrarian's insight). Severity prefix only for [error] (warn is implicit — reduces noise). classify.py extraction. Most detailed test counts per item. Risk registry with severity/mitigation.

---

## Consensus

# Consensus: Four milestones (v1.0 through v1.3) covering all 11 deferred items, adding 3 new config keys and ~120 new tests

## Recommendation

### Milestone Map

| Milestone | Theme | New Config Keys | Est. New Tests |
|---|---|---|---|
| **v1.0** | Project Config + Session UX | `ruff_use_project_config`, `biome_use_project_config` | ~40 |
| **v1.1** | Severity + Machine Output | `output_format` | ~35 |
| **v1.2** | Go + Rust | (none) | ~30 |
| **v1.3** | Fingerprinting + Dry-Run | (none) | ~15 |

Total trajectory: 448 tests to ~568 tests. Runner stays under 700 lines (section comments only, no further extraction).

---

### v1.0 "Project Config" (next release)

**Items covered:** (1) ruff_use_project_config, (2) biome_use_project_config, (3) /ecko:session command, (4) session stats in stop output, (5) runner decomposition phase 2 (descoped to comments only)

#### 1a. `ruff_use_project_config` (P0)

**What:** Boolean flag. When `true`, ruff adapter drops `--select` and defers to the project's own `ruff.toml` / `pyproject.toml [tool.ruff]`. Always passes `--no-fix` (safety invariant) and `--output-format json`. Emits a note when enabled but no project config is found.

**Files:**

- `checks/config.py` -- add `get_ruff_use_project_config(config) -> bool`, add `"ruff_use_project_config"` to `_KNOWN_KEYS`
- `checks/tools/ruff_adapter.py` -- add `use_project_config: bool = False` parameter to `run_ruff()`. When true, build command as `[*cmd, "check", "--output-format", "json", "--no-fix", file_path]` (no `--select`). When false, current behavior unchanged. Echoes from project config still go through RULE_MAP where possible; unmapped codes use `code.lower()` as check name (existing behavior from `ruff_extra_rules`).
- `checks/runner.py` -- read the config value once before the file loop, pass to `_run_layer2_checks` and through to `run_ruff()`
- `ecko.yaml.example` -- add commented-out `ruff_use_project_config: false`

**Tests (~8):**
- Config getter: default false, explicit true, explicit false
- Adapter: with project config flag, `--select` is absent
- Adapter: `--no-fix` always present regardless of flag
- Adapter: note emitted when flag true but no ruff.toml found (mock subprocess)
- Integration: echoes still filtered by `disabled_checks`

#### 1b. `biome_use_project_config` (P1)

**What:** Boolean flag. When `true`, biome adapter drops `--config-path` (ecko's bundled config) and uses the project's own `biome.json` / `biome.jsonc`. Unknown biome rule names are mapped to ecko check names via `_to_kebab()` conversion (e.g., `noDoubleEquals` becomes `no-double-equals`). Known rules still use RULE_MAP. Falls back to ecko's config if project config not found.

**Files:**

- `checks/config.py` -- add `get_biome_use_project_config(config) -> bool`, add to `_KNOWN_KEYS`
- `checks/tools/biome_adapter.py` -- add `use_project_config: bool = False` parameter. Add `_to_kebab(name: str) -> str` helper (insert `-` before each uppercase letter, lowercase all). When true and project biome.json exists, run without `--config-path`. Modify the rule mapping: `check = RULE_MAP.get(rule_name) or _to_kebab(rule_name)`. When true and no project config found, fall back to ecko's config and emit a note.
- `checks/runner.py` -- read config, pass through `_run_layer2_checks`
- `ecko.yaml.example` -- add commented-out `biome_use_project_config: false`

**Tests (~8):**
- `_to_kebab`: `"noUnusedImports"` -> `"no-unused-imports"`, `"noVar"` -> `"no-var"`, already-kebab passthrough
- Config getter: default false, explicit true
- Adapter: without flag, `--config-path` present (current behavior)
- Adapter: with flag and project biome.json present, `--config-path` absent
- Adapter: with flag and no project config, fallback to ecko config with note
- Integration: unknown rules get kebab-case check name, can be disabled via `disabled_checks`

#### 1c. Session stats in stop output (P1)

**What:** After the self-correction summary in the stop hook, emit a one-line session summary: `~~ ecko ~~ session: 47 echoes across 8 files, 12 self-corrected`. Uses existing `read_session()` and `compute_self_corrections()` from ledger.py. No new data structures needed.

**Files:**

- `checks/result.py` -- add `format_session_stats(entries, corrections) -> str` that computes totals from entries and returns the summary line (or empty string if no data)
- `checks/runner.py` -- in `run_stop()`, after the existing `correction_line` block, call `format_session_stats` and emit it

**Tests (~5):**
- Empty entries -> empty string
- Entries with echoes -> correct totals
- Entries with corrections -> includes correction count
- Output appears after correction line in stop output

#### 1d. `/ecko:session` command (P2)

**What:** Slash command that reads the session ledger and presents a structured summary: files touched, total echoes, self-correction rate, top echoes by check name.

**Files:**

- `commands/session.md` -- new slash command. Runs `python3 ${CLAUDE_PLUGIN_ROOT}/checks/session_stats.py --cwd <cwd>` via Bash tool, then presents the output.
- `checks/session_stats.py` -- new module (~50 lines). Imports `ledger.read_session`, `ledger.compute_self_corrections`, `config.load_config`, `config.get_session_hours`. Computes and prints: files touched, total echoes, top 5 checks, self-correction count, clean-first-pass files. Output is plain text, not stderr (this is a command, not a hook).

**Tests (~5):**
- session_stats with empty ledger
- session_stats with populated ledger
- Top-5 check ranking
- Clean-first-pass counting

#### 1e. Runner decomposition phase 2 -- DESCOPED

Runner is 617 lines. After adding project config passthrough it will be ~640-650. This is well within maintainability bounds. All 4 syntheses agree the extraction candidates (`filter_suppressed`, `is_excluded`, `detect_language`) are tightly coupled to runner flow. The cost of a new module (import complexity, re-exports, test updates) exceeds the readability benefit.

**Action:** Add section comments (`# --- Filtering ---`, `# --- Tool availability ---`) to delineate logical sections. No extraction.

---

### v1.1 "Severity + Machine Output"

**Items covered:** (6) severity levels, (7) structured JSON output, (8) ruff_disabled_rules (eliminated)

#### 1.1a. Severity on Echo (P0)

**What:** Add a `severity` field to the Echo dataclass. Default `"warn"`. Internal defaults only in this release -- no user configuration.

**Design decisions:**

- **No user config initially.** Ship the data model first, add user overrides in a future release once the defaults prove themselves.
- **No severity_threshold.** There is no behavioral mechanism to attach it to -- Claude Code hooks have binary exit codes (0/1/2). A threshold without teeth is misleading.
- **Text output prefix: `[error]` only.** Warn is the implicit default. Only error-severity echoes get a prefix. This reduces noise in the common case.

**Files:**

- `checks/result.py` -- add `severity: str = "warn"` field to Echo dataclass. In `format_file_echoes` and `format_stop_echoes`, prepend `[error] ` to the check name when `severity == "error"`. Add `has_errors(echoes: list[Echo]) -> bool` helper.
- `checks/tools/ruff_adapter.py` -- set `severity="error"` for `bare-except` (E722) and `star-imports` (F403)
- `checks/tools/biome_adapter.py` -- set `severity="error"` for biome's `error`-category diagnostics
- `checks/tools/pyright_adapter.py` -- set `severity="error"` for pyright errors
- `checks/custom/unreachable_code.py` -- set `severity="error"`

**Tests (~12):**
- Echo dataclass: default severity is "warn"
- format_file_echoes: error echoes show `[error]` prefix
- format_file_echoes: warn echoes show no prefix
- has_errors: true when any error, false when all warn
- Adapter-specific: ruff bare-except is error, singleton-comparison is warn

#### 1.1b. Structured JSON output (P0)

**What:** `output_format` config key. When set to `"json"`, runner emits JSON to stderr instead of text. Schema version 1. No echo caps applied in JSON mode.

**Schema:**

```json
{
  "schema_version": 1,
  "mode": "post-tool-use",
  "file": "src/app.py",
  "echoes": [
    {
      "check": "unused-imports",
      "line": 3,
      "message": "...",
      "suggestion": "...",
      "severity": "warn"
    }
  ],
  "skipped_tools": ["biome"]
}
```

Stop mode: `{"schema_version": 1, "mode": "stop", "files": {...}, "elapsed": 1.2, "skipped_tools": [...]}`

**Files:**

- `checks/config.py` -- add `get_output_format(config) -> str` (returns "text" or "json"), add to `_KNOWN_KEYS`
- `checks/result.py` -- add `format_file_echoes_json()` and `format_stop_echoes_json()`. No echo caps applied.
- `checks/runner.py` -- read output_format from config, branch on format at final emit stage

**Tests (~12):**
- Config getter: default "text", explicit "json"
- JSON post-tool-use: valid JSON, schema_version present, all echoes included (no cap)
- JSON stop: valid JSON, files structure, elapsed present
- JSON: severity field included on each echo
- Text mode: unchanged behavior (regression)

#### 1.1c. ruff_disabled_rules -- ELIMINATED

`disabled_checks` already handles suppression via the ecko check name layer. The ruff adapter maps codes to ecko check names; `disabled_checks: [c901]` suppresses `C901`. The case where E711 and E712 both map to `singleton-comparison` but user wants only one suppressed is extremely rare.

**Action:** Document the interaction in `ecko.yaml.example` comments. If users report friction, add source_code field to Echo in a future release.

---

### v1.2 "Go + Rust"

**Items covered:** (10) Go support, (11) Rust support

Both are Layer 3 only (project-level tools, not per-file). Both follow the established adapter pattern.

#### 1.2a. Go support via golangci-lint (P0)

**Files:**

- `checks/tools/golangci_adapter.py` (~60 lines) -- `run_golangci(cwd, modified_files) -> dict[str, list[Echo]]`. Runs `golangci-lint run --out-format json ./...`. Parses JSON (capital-I "Issues"). Check names: `go-{linter}` (e.g., `go-errcheck`). Post-filters to modified files. Timeout: 120s.
- `checks/runner.py` -- add `".go": "go"` to `LANG_MAP`. Add golangci-lint to Layer 3 thread pool. Add install hint.
- `ecko.yaml.example` -- add `golangci-lint: true` under `deep_analysis`

**Tests (~15):**
- Adapter: parse JSON, timeout handling, OSError handling, post-filter, tool not found
- Runner: .go language detection, dispatch, skipped tool messages

#### 1.2b. Rust support via clippy (P1)

**Files:**

- `checks/tools/clippy_adapter.py` (~70 lines) -- `run_clippy(cwd, modified_files) -> dict[str, list[Echo]]`. Runs `cargo clippy --message-format=json`. Streaming JSON (one object per line). Filter `reason == "compiler-message"`. Check names: `rust-{lint}`. Post-filters to modified files. Timeout: 120s.
- `checks/runner.py` -- add `".rs": "rust"` to `LANG_MAP`. Add clippy to Layer 3 (gated on Cargo.toml). Add install hint.
- `ecko.yaml.example` -- add `clippy: true` under `deep_analysis`

**Tests (~15):**
- Adapter: streaming JSON parsing, timeout, OSError, post-filter, Cargo.toml gate, tool not found

---

### v1.3 "Intelligence"

**Items covered:** (12) project fingerprinting, (13) dry-run mode

#### 1.3a. Project fingerprinting (P1)

**What:** `checks/fingerprint.py` (~80 lines). Detects frameworks from marker files and dependency content. Results feed `/ecko:tune` suggestions and vulture adapter skip lists. No auto-configuration.

**Files:**

- `checks/fingerprint.py` -- `detect_frameworks(cwd) -> set[str]`. Scans requirements.txt, pyproject.toml, package.json for dependency names. 10KB cap per file.
- `checks/tools/vulture_adapter.py` -- `_FRAMEWORK_VULTURE_SKIPS` dict (FastAPI: db, session, request; Flask: app, g, request; Django: request, queryset)
- `commands/status.md` / `commands/tune.md` -- show detected frameworks, framework-aware suggestions

**Tests (~8):**
- detect_frameworks: Django, Flask, FastAPI, Next.js, no markers
- Vulture integration: framework-specific skips applied

#### 1.3b. Dry-run mode (P2)

**What:** `--mode dry-run` lists which checks would run for a given file without executing any tools.

**Files:**

- `checks/runner.py` -- add "dry-run" to mode choices, new `run_dry_run()` function (~40 lines). Lists: language detection, applicable checks, tool availability, config values.

**Tests (~7):**
- Python file lists ruff/custom/pyright/vulture
- JS file lists biome/custom/tsc/knip
- Missing tools shown as "not found"
- Disabled checks shown as disabled

---

## Key Agreements

1. **`ruff_use_project_config` always passes `--no-fix`.** Non-negotiable safety invariant.
2. **`biome_use_project_config` with `_to_kebab()` name mapping.** Only viable approach for unknown biome rules.
3. **Go and Rust are Layer 3 only.** golangci-lint and clippy are project-level analysis tools.
4. **JSON output: no echo caps, schema version field.** Machine consumers need complete data.
5. **Severity as Echo dataclass field, default "warn".** All syntheses agree on the data model.
6. **Fingerprinting feeds display and filters, never auto-configuration.** Conservative scope.
7. **`ruff_disabled_rules` is unnecessary.** `disabled_checks` already handles suppression via ecko check names.

## Resolved Trade-offs

**Runner decomposition: comments only, no extraction.** Runner is 617 lines, will grow to ~680. The extraction candidates are 5-15 lines each and tightly coupled. Section comments provide navigability at zero cost. Revisit if runner exceeds 800 lines.

**ruff_disabled_rules: eliminated.** disabled_checks already works. The rare E711-vs-E712 edge case doesn't justify a new config key + implementation. Document the interaction instead.

**Severity user config: none initially.** Ship the data model, let JSON consumers use the severity field, add user overrides when there's behavioral mechanism (CI mode, Claude Code protocol evolution).

**Milestone ordering: project config before severity/JSON.** ruff_use_project_config and biome_use_project_config are the most requested features from the v0.9.0 deferral. Session stats use existing infrastructure. Ship them first for immediate user value.

**Number of new config keys: 3 total.** ruff_use_project_config (bool), biome_use_project_config (bool), output_format (string). Minimal config surface.

**Dry-run in v1.3, not eliminated.** Genuinely different from ECKO_DEBUG=1 (which still runs tools). Placed last so it can accurately list all checks including Go/Rust.

## Open Questions

1. **golangci-lint resolution pattern.** It's a Go binary, not Python/Node. Use simple `shutil.which()` PATH check.
2. **Clippy requires Rust toolchain.** No package manager fallback. Graceful skip if cargo not found.
3. **JSON output and exit codes.** Keep identical exit codes regardless of output format. JSON consumers inspect the echoes array.
4. **Session stats accuracy across blended sessions.** Acceptable given existing session_hours design.
5. **Fingerprint marker maintenance.** Built-in only for v1.3, no user-extensible markers.

## Confidence Assessment

**Rock-solid:**
- ruff_use_project_config design (--no-fix invariant, drop --select)
- biome_use_project_config with _to_kebab() fallback
- Severity as Echo field with internal defaults
- JSON output schema with schema_version: 1 and no echo caps
- Go/Rust as Layer 3 only
- Elimination of ruff_disabled_rules
- Runner decomposition descoped to comments

**High confidence:**
- Session stats format and placement in stop output
- /ecko:session command structure
- Dry-run output format
- Test counts per milestone (~40, ~35, ~30, ~15)

**Provisional:**
- Fingerprint marker table completeness — validate against 10-repo suite
- Vulture framework skip lists — false negative risk if too broad
- golangci-lint and clippy timeout values (120s) — may need tuning
- v1.2 vs v1.3 ordering — if Go/Rust demand is low, fingerprinting could move earlier
