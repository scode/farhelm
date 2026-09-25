---
kind: fixed
---

The `--payload-dir` provisioning source now removes crash-orphaned staging files during later payload use. Active materializations, completed payload snapshots, unrelated temporary files, and legacy `.extracted` contents remain intact.
