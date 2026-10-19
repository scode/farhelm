# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight f1276d] `workspace-trust-destination.md` — Inherited spawns recompute workspace trust for their destination; launch cards show trust plainly; a replaced checkout folder loses trust (3 triage outcomes).
- [complete] `agent-spawned-session-rules.md` — Refuse empty agent-to-agent prompts, call a prompt card's permission shield a snapshot, and say a wrapper is not tied to an agent type (3 triage outcomes).
- [pending] `host-entry-session-ids.md` — Identify sessions by a helm-assigned host-entry UUID plus the session ID, so approvals and confirmations cannot reach another host entry (2 triage outcomes, plus one triage already folded in). (after `agent-spawned-session-rules.md`, `workspace-trust-destination.md`)
- [in-flight 86c982] `launch-enters-working-dir.md` — Agent launches, Restart, Replace and terminal tabs enter the session folder themselves and fail with a clear error instead of starting in HOME (1 triage outcome).
- [complete] `link-hover-covers.md` — The tunnel-error notice counts as covering a link hover, the spec accepts the refusal banner over a card, and a modified click opens only when the hover offered it (3 triage outcomes).
- [in-flight cb6b05] `session-header-and-row-fixes.md` — Automatic terminal recovery leaves an open Restart/Replace/Delete question focused; the copy warning follows the copied value; accessible names escape titles (3 triage outcomes).
- [pending] `queued-launch-snapshot.md` — At Enter, an armed quick-launch sequence launches the templates its pill showed, not whatever the keys are bound to by then (1 triage outcome).
- [pending] `payload-lock-reproduction.md` — Reproduce whether old payload work overlapping a successor helm can leave a bad cached program copy, and record the evidence for a later decision (1 triage outcome).
- [pending] `archive-recovery-reproduction.md` — Reproduce whether a cancelled startup's archive recovery can leave a trashed checkout unlisted, and record the evidence for a later decision (1 triage outcome).
- [pending] `triage-discards-oct18.md` — Remove the five feedback items the 2026-10-18 triage discarded, with their ledger updates, in one PR (5 triage outcomes).
- [pending] `supervisor-colon-path-refusal.md` — The supervisor and the desktop app refuse to start when the folder sessions get on PATH contains ':' (1 triage outcome).
- [pending] `tunnel-port-consent-warning.md` — The one-time localhost tunnel warning says a tunnel uses the remote service's port number on the Mac when free (1 triage outcome).
