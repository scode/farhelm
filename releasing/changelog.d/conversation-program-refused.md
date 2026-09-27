---
kind: fixed
---

Starting a session whose command begins with `{conversation}` is now refused. For agents like Claude or Codex, Farhelm builds the resume command from the launch command, and this one produced a resume command the supervisor refuses to load, so once such a session existed, the host's supervisor failed to start after its next restart, upgrade, or reboot until the session was removed from its database by hand.
