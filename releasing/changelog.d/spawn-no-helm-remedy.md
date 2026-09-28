---
kind: fixed
---

When `farhelm spawn --agent` or `--profile-id` fails because no helm is attached to the session, the error now tells you to use `--inherit-agent` instead. It used to suggest leaving out `--agent`, which the command rejects.
