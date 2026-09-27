---
kind: fixed
---

The `agent` column of `farhelm agent sessions` no longer shows anything taken from a session's command line. A session started as `API_KEY=… claude` used to show the key itself in that column, to every agent on every host, until the failed session was deleted. The column now shows the profile's name, the agent Farhelm recognized (`claude`, `codex`, `goose`, `pi`, `omp`, `grok`), or `custom`. Sessions the session launcher starts with Goose settings now show `goose` instead of `env`.

Until a remote host is updated, its sessions that were not started from a profile show `custom` there.
