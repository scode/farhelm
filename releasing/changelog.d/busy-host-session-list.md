---
kind: fixed
---

Typing into sessions and refreshing the session list keep working on a host while many of its sessions are being
stopped, restarted or deleted at once. Each of those can take several seconds while a session's programs are given time
to exit, and once eight were in progress on one host, the next session-list refresh stopped keystrokes to every session
on that host until one of them finished; if that lasted long enough, the app dropped its connection to the host.
