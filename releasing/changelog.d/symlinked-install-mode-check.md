---
kind: fixed
---

Re-running Update on a remote host whose `farhelm` binary or supervisor unit file is a symlink no longer changes the linked file's permissions and reports a mode repair on every run, or fails with a permission error when that file belongs to another user. Farhelm now reads the permissions of the file the symlink points to, as it already did on the helm's own machine.
