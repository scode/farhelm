---
kind: fixed
---

A `claude` that a Claude session starts through its shell, such as a sub-agent it shells out to, can no longer replace the conversation a restart resumes, even when that child has a Farhelm reporting hook of its own. Only the session's own foreground Claude can report its conversation. If you launch Claude through a wrapper, the wrapper must start Claude directly: Claude sessions under a chain of two launchers (a wrapper that runs a script, which in turn runs `claude` without `exec`) now fall back to matching the conversation from Claude's saved records instead of Claude reporting it.
