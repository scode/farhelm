---
kind: added
---

Enable conversation hooks for Grok and Antigravity from Settings → Agents or when you pick either agent in the session
launcher. This lets Restart resume the conversation after the first prompt. Farhelm installs its hooks in the agent's
own configuration on each machine where it starts that agent. The hooks also run outside Farhelm, where they only
answer the agent.

Turning the switch off leaves installed hooks in place. Settings names the file or directory to delete. If you
previously set up Antigravity by hand, you can delete the old `farhelm` entry from `~/.gemini/config/hooks.json`.
