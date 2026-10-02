---
kind: breaking
---

Farhelm now starts Cursor as `cursor-agent` instead of `agent`, from the launch composer and from the `cursor` and
`cursor-yolo` built-in profiles, so `cursor-agent` must be on the PATH of each host that runs Cursor. YOLO detection for
custom command lines and profiles now recognizes Cursor only by that name, and also counts its short `-f` flag. A
command that starts with the generic name `agent` is no longer treated as Cursor, because other tools install a command
by that name too. If you wrote your own profile or command as `agent --force` (or `agent -f`, `agent --yolo`), it now
starts without the YOLO confirmation on a host that asks first and shows no YOLO badge: change `agent` to `cursor-agent` in it to keep both.

Sessions started earlier from the `cursor-yolo` profile or a typed `agent --force` command lose their YOLO badge and
Cursor mark in the sidebar, and cloning or replacing a typed-command one is no longer checked for YOLO. Sessions started
from the launch composer are unaffected.
