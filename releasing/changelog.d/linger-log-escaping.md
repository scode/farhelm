---
kind: fixed
---

When enabling linger on a remote host fails while adding or updating it, the host's error output is now written to the
helm's log with terminal control characters escaped, so a remote host can no longer put sequences into that log that
redraw your terminal or change your clipboard when you read it.
