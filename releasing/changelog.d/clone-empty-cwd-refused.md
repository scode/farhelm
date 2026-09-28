---
kind: fixed
---

`farhelm agent clone --cwd ""` is now refused right away with a message saying to leave `--cwd` out to use the source session's directory. Before, the empty directory replaced the source's and the clone failed only on the destination host with a confusing "working directory is not absolute" error, and an idempotency key passed with it could no longer be reused.
