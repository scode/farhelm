---
kind: changed
---

On Linux, stopping a session, closing a terminal tab or deleting a session avoids scanning every process when its
systemd groups confirm cleanup. Programs that deliberately move out of those groups, or only carry a session's
environment marker, are no longer stopped by that cheaper cleanup.

The full scan remains when a group contains a tmux server, so programs in its panes still end with the session or tab
that started it, even when tmux puts them in separate groups. Delete also keeps the full scan when it cannot establish
complete tab discovery, including when the session has no terminal or its old tmux session is gone. Unscoped or
uncertain containment retains the existing scan.
