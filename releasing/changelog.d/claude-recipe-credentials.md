---
kind: changed
---

Claude Code sessions and terminals that Farhelm starts on a recipe host now get the recipe's credentials when they start.
A running session picks up a replacement after Restart.

Farhelm removes the keys it previously delivered to Claude's settings once during upgrade. Claude started in a shell
outside Farhelm uses that shell's own configuration. Codex continues to use its existing recipe credential delivery.
