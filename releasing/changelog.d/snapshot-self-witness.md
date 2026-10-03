---
kind: fixed
---

On a host without a usable systemd user manager, Stop and Delete find a session's processes by reading the process
table. If that read silently came back empty, they reported the session stopped or deleted even though its processes
might still be running. Such a read now counts as a failure to look, so the operation reports that it could not confirm
the processes are gone.
