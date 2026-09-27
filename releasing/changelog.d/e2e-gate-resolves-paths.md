---
kind: none
---

The test-only provisioning backend switch now resolves symlinks and `..` before checking that its directory is inside the helm state directory. Nothing changes for normal use.
