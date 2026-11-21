# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 454f61] `gui-launcher-feedback-fixes.md` — show recent setups' wrapper prefix, the inline default-folder field for a new sandbox checkout, a correct Restart explanation for older Muse sessions and the Discord link's ↗, closing four triage outcomes
- [in-flight be1841] `hosts-small-fixes.md` — disable container-only recipes in Add host's Tensorlake form, record a repeat host deletion's failure, say "start" for a starting container and name attention-list Update buttons, closing four triage outcomes
- [in-flight b1aebc] `terminal-exit-and-placeholder-fixes.md` — let an exited agent's oldest line scroll into view and make the connecting placeholder readable in light appearance, closing two triage outcomes
- [landing 0d8c3e] `desktop-updater-unreadable-optout.md` — treat an unreadable automatic-update setting as off, closing its triage outcome
- [in-flight bf7f7b] `container-delete-and-link-group-tests.md` — add regression tests for legacy container Delete's removal and snapshot link grouping, closing two triage outcomes
