---
kind: fixed
---

On Linux hosts, a restart interrupted at the wrong moment, for example by closing the browser while it was running, can no longer leave the host's supervisor unable to stop, delete, restart, or open terminals for any session until it is restarted. If the interrupted restart happened to be the first operation checking for a systemd user manager, that unfinished check used to block every later operation that needed the answer.
