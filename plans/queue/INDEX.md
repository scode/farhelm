# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `hidden-trash-folder.md` — Deleted managed checkouts go into a hidden `.farhelm-trash` folder, and each host's existing trash moves there on its own
- [complete] `steel-foundation.md` — Steel redesign, part 1: blue-gray and periwinkle colors with a light counterpart and an appearance setting, Inter for the interface, solid selection, shared controls, and visual-review captures (after `standard-link-style.md`)
- [pending] `steel-sidebar.md` — Steel redesign, sidebar: host rail with All and host states, hosts panel, sort order as the list title with the session count, status filter (after `steel-foundation.md`, `uninstall-identity-display.md`)
- [pending] `steel-session-chrome.md` — Steel redesign, session toolbar: icon-and-label actions, split Restart and Replace buttons with their "with" menus, tunnel pill beside the tabs (after `steel-foundation.md`)
- [pending] `steel-dialogs.md` — Steel redesign, dialogs: settings with section icons and switches, the launcher's labeled rows and pinned footer, every confirmation and menu on one anatomy (after `steel-foundation.md`, `report-failed-settings-saves.md`, `template-panel-escape.md`, `restart-with-template-refusal.md`)
- [pending] `steel-sentence-case.md` — Steel redesign, last part: sentence case for every label in the app, SPEC.md, tests and the docs website's text (after `steel-sidebar.md`, `steel-session-chrome.md`, `steel-dialogs.md`)
- [pending] `template-delete-guards.md` — Template Delete keeps its promises: Retry can no longer permanently delete behind a plain "delete", and a lost reply says the delete may have happened (three triage outcomes)
- [in-flight 5e699e] `installer-link-noreplace.md` — The installer never overwrites a file that appears at the Terminal command's location mid-install, and the spec accepts older releases' own installers (two triage outcomes)
- [complete] `restart-with-trust-identity.md` — Restart with checks the checkout folder's identity before trusting a Codex or Pi agent automatically (triage outcome)
- [in-flight b28153] `link-refusal-confirm-layer.md` — A refused-link explanation never covers an open session confirmation's warning (triage outcome)
- [in-flight c41902] `named-restart-reporter-clear.md` — Restart with into another Pi or OMP conversation clears the old one when the conversation reporter cannot be prepared (triage outcome)
