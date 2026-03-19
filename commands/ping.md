---
description: "Run ecko checks on a file and echo back any issues found"
allowed-tools: ["Bash", "Read"]
---

Run ecko checks on the specified file (or the most recently edited file if none given).

Arguments: $ARGUMENTS

Steps:
1. Determine the target file. If the user provided a file path in the arguments, use that. Otherwise, find the most recently modified tracked file using `git diff --name-only HEAD` or `git log -1 --name-only`.
2. Run the ecko checker:
   ```
   python3 ${CLAUDE_PLUGIN_ROOT}/checks/runner.py --file <FILE> --mode post-tool-use --cwd $(pwd) --plugin-root ${CLAUDE_PLUGIN_ROOT}
   ```
3. Show the output to the user. If there are no echoes, say the file is clean.
