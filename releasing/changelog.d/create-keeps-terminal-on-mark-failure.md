---
kind: fixed
---

When tmux fails partway through starting a new session (after creating it, while labelling its agent window), the session that is kept now opens normally. Before, its agent could be running but the session had no terminal to open until the host's supervisor restarted.
