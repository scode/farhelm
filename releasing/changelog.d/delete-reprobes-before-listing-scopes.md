---
kind: fixed
---

Deleting a session on a Linux host with a systemd user manager no longer sometimes leaves a background process from an earlier run of that session, or from a closed terminal tab, running after the session is gone. If Farhelm had wrongly concluded at startup that the host had no usable user manager, Delete skipped asking systemd for the session's older units before correcting that conclusion, and so never stopped the processes only those units could reach.
