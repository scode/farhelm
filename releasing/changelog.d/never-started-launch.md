---
kind: fixed
---

When an agent never starts, the error now identifies whether your login shell or the host's systemd scope wrapper
exited first, with its exit code when known. A login shell that exits before starting the agent also shows an error on
hosts without systemd scopes, including macOS.
