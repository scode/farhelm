---
kind: changed
---

Non-YOLO launches are now called guarded, and Farhelm passes explicit flags for each agent's guarded mode instead of leaving it to the agent's configuration; Cursor remains on the agent's own setting with a grey dashed shield. Update remote hosts after upgrading before launching there.

Older Claude, Codex, Grok and Muse sessions started without permission flags keep their stored command, show the grey agent's own setting shield and count as YOLO on hosts that ask first until relaunched with Clone, Replace with or Restart with; plain Replace asks for confirmation there. Launches stored as guarded have no downgrade path to older Farhelm releases. Guarded can approve work automatically through a safety check; it does not promise to ask before every action or override your own allow rules and later mode switches.
