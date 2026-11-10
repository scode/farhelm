# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `nonblocking-session-actions.md` — Pending Stop, Restart, Replace, Delete and launches no longer block switching sessions or acting on other ones; the acted-on session shows "Stopping…" and keeps its result after a switch
- [pending] `header-actions-compact.md` — The session header's actions compact only when the header is actually short of room, not whenever a tunnel pill is shown, with a browser test
- [pending] `podman-environments.md` — A rootless Podman container on one of your hosts as a session's own machine, built from a shared recipe with Farhelm inside and the checkout cloned inside; sandbox and container sessions share one provider abstraction in code; today's containerized sessions become delete-only (after `sandbox-sessions.md`)
- [pending] `environment-agent-operations.md` — Agents in sandbox and container sessions can ask Farhelm to act within their own environment, environments live until their last session is deleted, and agents elsewhere can launch sessions in new environments behind an always-asking approval card (after `podman-environments.md`)
