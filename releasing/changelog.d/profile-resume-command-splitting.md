---
kind: fixed
---

A profile's resume command is now split into arguments the same way as its invocation. Before, a backslash inside double
quotes was dropped only in the resume command, so `--append-system-prompt "match \d+"` restarted a session with
`match d+`. A word starting with `#` now begins a comment there too, as it already did in the invocation, and a trailing
backslash is kept instead of refused. Saved profiles are unchanged until their resume command is edited.
