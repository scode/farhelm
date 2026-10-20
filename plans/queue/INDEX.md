# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 7ddbf5] `host-entry-session-ids.md` — Identify sessions by a helm-assigned host-entry UUID plus the session ID, so approvals and confirmations cannot reach another host entry (2 triage outcomes, plus one triage already folded in). (after `agent-spawned-session-rules.md`, `workspace-trust-destination.md`)
- [in-flight bd2ebd] `helm-lock-outlives-work.md` — Keep the helm's state-directory lock until every task on its runtime has finished, so a successor cannot claim the directory while old install work still writes the release cache (1 triage outcome).
- [complete] `summary-card-observed-yolo.md` — Make the session hover summary say when YOLO has been observed instead of only the stored "guarded" launch (1 triage outcome).
- [in-flight 1d63af] `hosts-panel-menu-close.md` — Close open sidebar menus when the hosts panel changes height, so a failing expanded setup cannot leave a menu beside the wrong row (1 triage outcome).
- [pending] `template-list-current-labels.md` — Reword the Cmd-J spec so a pick follows the name's current template, and refresh the list's shortcut labels when it opens (2 triage outcomes).
