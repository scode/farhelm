---
kind: fixed
---

The session launcher now refuses a custom model id of `{codex:trusted-cwd}` or `{codex:untrusted-cwd}`, as it already
did for `{cwd}` and `{conversation}`. Before, such an id was accepted and then silently replaced at launch with a Codex
workspace-trust setting, which the agent then rejected as a model name.
