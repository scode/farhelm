---
kind: changed
---

On the Mac, quitting Farhelm now leaves its supervisor, the part that keeps your sessions running, running in the
background, so your sessions' terminals keep being served while the app is closed. Opening Farhelm again reconnects to
it, and an update replaces it with the new version's. `farhelm uninstall` stops it for you.

Context for curation: the agents' `farhelm spawn` and `farhelm agent` commands still fail while the app is closed,
because they need the helm, which lives in the app. Nothing restarts a supervisor that crashes while the app is closed,
and nothing starts it at login; opening the app does. The supervisor's log moved from the app's own log to
`supervisor.log` in the state directory (`~/.local/state/farhelm/supervisor.log`). This also applies to the Linux
desktop app. The update handover (an older app's supervisor being replaced by the new version's) is first exercised by
the next release's brick test.
