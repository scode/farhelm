---
kind: fixed
---

On Linux, a systemd user manager that is merely slow to answer no longer turns off per-session cgroup scopes until the
supervisor restarts. A launch that meets a slow manager still runs without its own scope, but a later launch checks
again, so scopes come back once the manager answers. A manager that refuses to start a scope is now recognized at once
instead of after the full 15-second check.
