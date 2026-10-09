---
kind: breaking
---

Relaunch Codex sessions started before this update before relying on Resume for a conversation begun with `/clear`: those running sessions cannot confirm the new conversation until relaunched. Farhelm now checks Codex and Grok saved files when you choose Restart instead of repeatedly reading them in the background. A deleted file can leave Resume available until that click; Restart then refuses and adds a notification. A passing read error keeps Resume available so you can retry.
