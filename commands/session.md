---
description: "Show session echo summary — files touched, top checks, self-corrections"
allowed-tools: ["Bash", "Read"]
---

Show a summary of the current coding session's echo activity.

Steps:
1. Run the session stats script:
   ```bash
   python3 ${CLAUDE_PLUGIN_ROOT}/checks/session_stats.py --cwd $(pwd)
   ```

2. Present the output to the user. The script shows:
   - Files touched in the session window
   - Total echoes detected
   - Self-corrections (echoes that were fixed after being flagged)
   - Clean first-pass rate (files with no echoes on first check)
   - Top 5 most frequent check types

3. If no session data exists, let the user know that ecko records session data as it runs — they'll see stats after writing some code.
