---
kind: changed
---

On Linux, stopping a session, closing a terminal tab or deleting a session avoids scanning every process when its
systemd groups confirm cleanup. Programs that deliberately move out of those groups, or only carry a session's
environment marker, are no longer stopped by that cheaper cleanup.

Jobs you start under your own tmux inside a session can keep running after Stop, Restart, Replace, Delete or tab close,
as they can outside Farhelm; the tmux server itself still ends. Delete keeps the full scan when it cannot establish
complete tab discovery, including when the session has no terminal or its old tmux session is gone. Unscoped or
uncertain containment retains the existing scan.
