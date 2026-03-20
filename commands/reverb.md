---
description: "Capture a reverb note about what went wrong in this session"
allowed-tools: ["Bash", "Read", "Write", "Glob"]
---

Write a reverb note — a structured reflection on what went wrong in the current session. You have full conversation context, so use it.

Arguments: $ARGUMENTS

Steps:
1. Create the reverb directory if it doesn't exist:
   ```
   mkdir -p .ecko-reverb
   ```

2. Run ecko stop mode to gather current echoes:
   ```
   python3 ${CLAUDE_PLUGIN_ROOT}/checks/runner.py --file . --mode stop --cwd $(pwd) --plugin-root ${CLAUDE_PLUGIN_ROOT} 2>&1
   ```

3. Generate a slug from the arguments or conversation context. Use 2-4 lowercase words joined by hyphens (e.g., `wrong-import-layer`, `missed-type-error`). Derive from arguments if provided, otherwise infer from the conversation.

4. Write the reverb note to `.ecko-reverb/{YYYY-MM-DD}-{slug}.md`:

   ```markdown
   # Reverb: {title}

   ## Echo summary
   {ecko stop-mode output, or "No echoes found" if clean}

   ## What went wrong
   {reflection on root cause, not just symptoms}

   ## User observations
   <!-- Add your own notes here about what surprised you or what you'd like ecko to catch next time -->

   ```

5. Tell the user the file was created and that `/ecko:tune` can turn reverb notes into ecko.yaml rules.
