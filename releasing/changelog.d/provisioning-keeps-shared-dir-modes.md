---
kind: fixed
---

Adding or updating a remote host no longer changes the permissions of existing directories Farhelm only puts a file into, such as `~/.config/systemd/user` or the directory holding a `farhelm` binary you registered (which can be your home directory). Before, provisioning reset them to world-readable `0755`, exposing whatever else was in them to other accounts on that host. Farhelm's own directories are still created and kept at their intended permissions.
