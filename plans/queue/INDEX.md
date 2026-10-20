# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight b966e6] `host-entry-session-ids.md` — Identify sessions by a helm-assigned host-entry UUID plus the session ID, so approvals and confirmations cannot reach another host entry (2 triage outcomes, plus one triage already folded in). (after `agent-spawned-session-rules.md`, `workspace-trust-destination.md`)
- [blocked] `launch-enters-working-dir.md` — Agent launches, Restart, Replace and terminal tabs enter the session folder themselves and fail with a clear error instead of starting in HOME (1 triage outcome).
- [complete] `payload-lock-reproduction.md` — Reproduce whether old payload work overlapping a successor helm can leave a bad cached program copy, and record the evidence for a later decision (1 triage outcome).
- [in-flight e244b8] `archive-recovery-reproduction.md` — Reproduce whether a cancelled startup's archive recovery can leave a trashed checkout unlisted, and record the evidence for a later decision (1 triage outcome).
- [complete] `triage-discards-oct18.md` — Remove the five feedback items the 2026-10-18 triage discarded, with their ledger updates, in one PR (5 triage outcomes).
- [complete] `supervisor-colon-path-refusal.md` — The supervisor and the desktop app refuse to start when the folder sessions get on PATH contains ':' (1 triage outcome).
- [complete] `tunnel-port-consent-warning.md` — The one-time localhost tunnel warning says a tunnel uses the remote service's port number on the Mac when free (1 triage outcome).
