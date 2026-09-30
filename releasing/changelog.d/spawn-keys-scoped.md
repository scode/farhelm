---
kind: fixed
---

An `--idempotency-key` passed to `farhelm spawn` now belongs to the session that ran the command. Before, another
session on the same host reusing the same key and request, such as a sibling or a child re-running its parent's command,
got the first session's child back as its own new child (or, for the child itself, an error). Keys used before this
release are not recognized afterwards, so a retry spanning the upgrade spawns a new session.
