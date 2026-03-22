---
description: "Verify ecko binary and optionally install external analysis tools"
allowed-tools: ["Bash", "Read", "AskUserQuestion"]
---

Help the user verify their ecko installation and optionally install external deep analysis tools.

Steps:
1. Verify the ecko binary is available:
   ```bash
   ${CLAUDE_PLUGIN_ROOT}/scripts/run.sh --version 2>&1
   ```
   If this fails, the script will attempt to build from source or download the binary automatically.

2. Run a quick self-check to verify ecko works:
   ```bash
   ${CLAUDE_PLUGIN_ROOT}/scripts/run.sh --mode dry-run --file dummy.py --cwd $(pwd) --plugin-root ${CLAUDE_PLUGIN_ROOT} 2>&1
   ```

3. Check which optional external tools are installed by running `which` for each:
   - `pyright` -- Python type checking (deep analysis)
   - `tsc` -- TypeScript type checking (deep analysis)
   - `golangci-lint` -- Go linting (deep analysis)
   - `clippy` -- Rust linting (comes with rustup)

4. For any missing tools the user wants, suggest installation:

   Python:
   - `pip install pyright`

   Node/TypeScript:
   - `npm install -g typescript`

   Go:
   - `go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest`

   Rust:
   - `rustup component add clippy`

5. Ecko's 28 core checks are native and need no external tools. External tools are optional and only enhance Layer 3 deep analysis on stop.

Important: Ask before installing anything. Some users may prefer project-local installs or different package managers.
