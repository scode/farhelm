---
kind: fixed
---

Linux setup and remote host setup now refuse a Farhelm program path containing `$`, with advice to install at another path, instead of writing a service that cannot start. Uninstall refuses a service changed to a type other than `simple`, because it cannot safely identify which installation owns it. Stale install-lock recovery commands now preserve spaces, quotes, non-ASCII text and shell-sensitive characters in paths; paths that cannot be printed safely get manual recovery advice instead.
