---
kind: none
---

Nothing user-facing yet: a restarted supervisor now finds the terminal hosts of its sessions again and keeps those
sessions running with their screens, keeps retrying a host it could not use at first, and an orderly stop leaves each
host a fresh snapshot within its time limit. Only sessions created under an internal switch that no shipped supervisor
turns on run in terminal hosts, so every session still runs in tmux until a later change turns new terminals on; that
change carries the real entry.
