# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 7ddbf5] `host-entry-session-ids.md` — Identify sessions by a helm-assigned host-entry UUID plus the session ID, so approvals and confirmations cannot reach another host entry (2 triage outcomes, plus one triage already folded in). (after `agent-spawned-session-rules.md`, `workspace-trust-destination.md`)
- [pending] `archive-recovery-reproduction.md` — Reproduce whether a cancelled startup's archive recovery can leave a trashed checkout unlisted, and record the evidence for a later decision (1 triage outcome).
