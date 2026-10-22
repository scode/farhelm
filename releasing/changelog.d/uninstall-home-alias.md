---
kind: fixed
---

Uninstall from the hosts panel now works on hosts whose home folder is reachable at two paths, for example through a
bind mount, where it used to refuse saying the service was loaded from somewhere else. Shared unit folders and shared
installations are still refused, and uninstall keeps the host's data.
