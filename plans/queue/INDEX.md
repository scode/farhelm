# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `download-staging-sweep.md` — Remove hidden partial downloads a desktop Quit left behind, without touching another instance's save (1 triage outcome). (after `helm-lock-outlives-work.md`)
- [complete] `sidebar-bell-follows-row.md` — Make the notification list hand focus back on every close and close when its row moves (2 triage outcomes). (after `hosts-panel-menu-close.md`)
- [complete] `template-editor-and-listing.md` — Escape template names in the editor, stop an open Templates dialog undoing agent shortcut moves, and list custom flags in the CLI table (3 triage outcomes). (after `template-list-current-labels.md`)
- [complete] `replace-yolo-shared-question.md` — Make the header and sidebar Replace controls share one YOLO question per session (1 triage outcome).
- [complete] `tab-attach-no-wait.md` — Stop a tab attach during Stop from stalling every session on that host (1 triage outcome).
- [complete] `planning-probe-bound.md` — Bound a host planning probe's output wait so a lingering ssh helper cannot hold a planning slot forever, if super cheap (1 triage outcome).
- [complete] `repository-discovery-git-config.md` — Make repository suggestions ignore an inherited GIT_CONFIG, as the test fixture already does (1 triage outcome).
- [complete] `spec-scope-clarifications.md` — Write the introduction-is-a-hook and published-notes principles and three scope statements into the spec, with no code change (8 triage outcomes).
- [complete] `tooling-checks-oct19.md` — Fix the release-review page's link escaping and two instruction-checker gaps (3 triage outcomes).
- [complete] `test-hygiene-oct19.md` — Fix a macOS path comparison in checkout tests and three browser-test cleanups that stop early or arm late (4 triage outcomes). (after `host-entry-session-ids.md`)
- [complete] `triage-discards-oct19.md` — Close the four findings discarded in the 2026-10-19 triage, in one PR (4 triage outcomes).
- [in-flight 38a1a2] `structured-settings-dialog.md` — Restructure the Settings dialog into a sidebar of seven sections, like the Templates dialog (1 TODO entry).
- [complete] `host-settings-focus.md` — Keep keyboard focus inside host settings and SSH help when a save ends or SSH help's commands arrive (1 TODO entry).
- [complete] `launch-sequences.md` — Accurate quick-launch banners, one launchability rule for templates, and a centered amber launch-sequence pill (2 TODO entries).
- [complete] `menu-arrow-focus-race.md` — Drop a desktop menu focus move that would land after the user moved focus elsewhere (1 TODO entry).
- [in-flight cf3a9b] `launcher-enter-feedback.md` — Make Enter in the session launcher always launch, pick a visible row, or say why not (1 TODO entry).
- [complete] `select-newest-on-close.md` — When the open session goes away, open the most recently visited one, from a history kept on the helm (1 TODO entry). (after `host-entry-session-ids.md`)
- [pending] `supervisor-request-modes.md` — Declare how every supervisor request type runs in one exhaustive table with one owner for channel ids, test that each keeps the connection moving, and fix the requests that still stall it (creation and the checkout reconciliation before it). (after `tab-attach-no-wait.md`)
- [pending] `helm-owned-by-default.md` — Run every state-changing helm request on a helm-owned task by default, with an explicit exempt list for routes that must stay cancellable (structural refactor; accidental omissions fixed first). (after `host-entry-session-ids.md`, `transfer-leftover-cleanup.md`)
- [pending] `helm-ingress-types.md` — Make supervisor-reported session data, parsed agent verbs and the origin of helm errors types that only the checking code can construct (structural refactor, no behavior change). (after `helm-owned-by-default.md`, `launch-sequences.md`)
- [pending] `ui-peer-text-type.md` — Carry text from other machines in its own UI type with explicit escaped and raw accessors, so showing it unescaped stops compiling, and fix the unescaped sites it exposes. (after `host-entry-session-ids.md`, `terminal-link-and-copy-fixes.md`, `template-editor-and-listing.md`)
- [pending] `ui-session-actions.md` — Replace the sidebar's and header's copied Replace/YOLO flows with one controller and an outcome enum, and make the operation lock guard-only (structural refactor, no behavior change). (after `ui-peer-text-type.md`, `replace-yolo-shared-question.md`, `launcher-retry-identity.md`, `launcher-composer-drafts.md`, `launcher-enter-feedback.md`, `launch-sequences.md`, `select-newest-on-close.md`)
