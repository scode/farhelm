---
kind: changed
---

Sessions and terminals that Farhelm starts on a recipe host now get the recipe's Claude Code credentials when they
start. A running session picks up a replacement after Restart.

Farhelm removes the keys it previously delivered to Claude's settings once during upgrade. Claude started in a shell
outside Farhelm uses that shell's own configuration. Codex continues to use its existing recipe credential delivery.

Claude settings already in a recipe's base image, such as a base URL, take precedence over the recipe's credentials, so
Claude would send the recipe's token to that URL; choose **None** for such an image.
