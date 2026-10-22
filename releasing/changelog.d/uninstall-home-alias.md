---
kind: fixed
---

Uninstall from the hosts panel now works on hosts whose home folder is reachable at two paths, for example through a
bind mount, where it used to refuse saying the service was loaded from somewhere else. Shared unit folders and shared
installations are still refused, and uninstall keeps the host's data.

A follow-up fix makes uninstall refuse one contrived layout this change briefly allowed: the second home path hid that
the data folder was inside the folder uninstall removes, so the data could have been deleted with it. It needs a bind
mount of the home that leaves out a mount nested inside it, plus a folder above Farhelm's install folder (such as
`~/.local/lib`) that is a symlink into that bind.
No release shipped the gap, so this needs no entry of its own.
