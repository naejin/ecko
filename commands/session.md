---
description: "Show session echo summary -- files touched, top checks, self-corrections"
allowed-tools: ["Bash", "Read"]
---

Show a summary of the current coding session's echo activity.

Steps:
1. Run the session stats command:
   ```bash
   ${CLAUDE_PLUGIN_ROOT}/scripts/run.sh --mode session-stats --cwd $(pwd) --plugin-root ${CLAUDE_PLUGIN_ROOT} 2>&1
   ```

2. Present the output to the user. The command shows:
   - Number of ledger entries in the session window
   - Files touched in the session
   - Self-corrections (echoes that were fixed after being flagged)

3. If no session data exists, let the user know that ecko records session data as it runs -- they'll see stats after writing some code.
