# Swarm: ecko-v080-session-memory

**Date**: 2026-03-21 16:30
**Configuration**: 5 diverge → 3 synthesis → 1 arbiter
**Lenses**: Pragmatist, Critic, Architect, Contrarian, Minimalist

---

## Input

### Task
Design the v0.8.0 milestone for ecko — session ledger, self-correction tracking, cross-file echo cap.

### Goal
Implementation-ready plan with files, code patterns, testing strategy.

### Context
Three deferred items from v0.7.0 swarm. Each hook is a fresh Python process. Zero Python dependencies. 347 tests, runner.py ~700 lines.

---

## Phase 1: Independent Exploration

### Agent 1 — The Pragmatist
**Theme: "Session Memory"**
`.ecko-session/ledger.jsonl`, staleness=6h, `ECKO_SESSION_HOURS` env var. Self-correction via (check, line) set diff. Cross-file cap per-check, default 15. `checks/session.py` ~120 lines. No locking, no session IDs. ~40 tests.

### Agent 2 — The Critic
**Theme: "What Could Go Wrong"**
Enumerated 10 risks. Session marker file with `O_CREAT|O_EXCL`. Self-correction: net count per check, flagged metric as misleading. Cross-file cap per-check, default 0. Proposed "first-pass clean" rate. Flagged runner.py growth to 800+. ~50 tests.

### Agent 3 — The Architect
**Theme: "Session Memory Architecture"**
`.ecko/session.jsonl` universal state dir. 30-min gap heuristic for session boundary. Rotation: truncate to current session on stop. Self-correction: subtraction model. Cross-file cap: total, default 25, unseen-files-first priority. Ledger as supplementary file source in `_get_modified_files`. File locking with fcntl/msvcrt. ~40 tests.

### Agent 4 — The Contrarian
**Theme: "Category Change"**
`.ecko-ledger/` with per-session JSONL files, session token. Self-correction: (check, line) with +/-2 line tolerance, separate `checks/correction.py`. Cross-file cap: static priority ranking (type-error=1, var-declarations=8). Replace --since=4h with ledger. `/ecko:ledger` slash command. File locking, 72h cleanup. ~58 tests.

### Agent 5 — The Minimalist
**Theme: "Minimal Machinery"**
Single `.ecko-session.jsonl` file (no directory). No session IDs, no markers. Prune-on-write, 4h window matching --since=4h. Self-correction: pure function. Always record 0-echo entries. Cross-file cap: total per stop run, default 0. `checks/ledger.py` ~80 lines. ~31 tests.

---

## Phase 2: Synthesis

### Synthesizer 1
`.ecko-session/ledger.jsonl`, 4h rolling window prune-on-write, per-check count delta self-correction, per-check cross-file cap default 15. Record 0-echo entries. Flat config keys. ~40 tests.

### Synthesizer 2
Same location, 6h staleness prune-on-read. High-water-mark self-correction. Total cross-file cap default 0. One-line-per-(file,check) schema. First-pass-clean rate. Ledger supplements _get_modified_files. Nested config. ~45 tests.

### Synthesizer 3
Same location, 4h window prune-on-write with `ECKO_SESSION_HOURS` override. Per-check count delta self-correction, stop only. Per-check cross-file cap default 0. Dict-per-entry schema `{"echoes": {"check": count}}`. Record clean files as `{}`. Flat `cross_file_echo_cap`, nested `session.*`. ~45 tests.

---

## Consensus

# v0.8.0 "Session Memory" — ledger + self-correction + cross-file cap

## Recommendation

Three features, one new module (`checks/ledger.py` ~130 lines), ~40 new tests (target ~387).

1. **Session ledger:** `.ecko-session/ledger.jsonl`, JSONL append-only, 4h rolling window, prune-on-write. Schema: `{"ts": float, "file": "rel/path", "mode": "post-tool-use"|"stop", "echoes": {"check": count}}`. Clean files recorded as `{"echoes": {}}`. No locking, no session IDs.

2. **Self-correction:** per-(file, check) count delta between first and last post-tool-use entry. Stop entries excluded. Single summary line: `~~ ecko ~~ self-corrections: 3 fixed, 1 persisted`.

3. **Cross-file echo cap:** per-check across all files in stop mode, default 0 (off). Config: `echo_cap_cross_file`. Display-only in `format_stop_echoes()`.

## Key Agreements

1. `.ecko-session/ledger.jsonl` location — unanimous
2. JSONL format — unanimous
3. Rolling time window (not session IDs) — unanimous
4. Count-based deltas (not line-level) — unanimous
5. No file locking — unanimous
6. Record clean files — unanimous
7. New standalone module — unanimous
8. All ledger I/O guarded by try/except — unanimous

## Resolved Trade-offs

- **4h vs 6h window:** 4h (matches `--since=4h`)
- **Schema:** dict-per-entry `{"echoes": {"check": count}}` (compact, delta-friendly)
- **Prune timing:** on-write (bounds file growth continuously)
- **Config style:** flat keys (matches `echo_cap_per_check` pattern)
- **Cross-file cap:** per-check not total (ensures all check types get visibility)
- **Cap default:** 0/off (new feature, least surprise)
- **Module name:** `ledger.py` (names the core abstraction)
- **No env var override** for session hours (config belongs in ecko.yaml)

## Open Questions

1. Should `session_hours: 0` disable the ledger?
2. Should self-correction summary include a percentage?
3. Ledger as supplementary file source in `_get_modified_files` (defer to v0.8.1)

## Confidence Assessment

**Rock-solid:** Location, format, schema, window, pruning, no locking, pure data module, test count.
**Provisional:** Exact self-correction wording, session_hours=0 behavior, cross-file cap overflow format.
