---
kind: fixed
---

When an agent never starts, the error now names the failed startup step, includes its exit code when known, and may
keep a short terminal excerpt. Shell startup failures also show error on hosts without systemd scopes, including
macOS.
