# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `uninstall-identity-display.md` — Uninstall refusals for a changed host identity stop quoting the identities, which could look alike, and point to the host list instead
- [complete] `template-panel-escape.md` — Escape in the launcher's save-as-template panel closes only the panel even after focus was lost, and focus returns to the name field after a refused save
- [complete] `ssh-close-start-quoting.md` — Delete the helm's unreachable "closed before the handshake" start suggestion; real SSH connections already get the corrected start command
- [complete] `replace-yolo-cancel-race.md` — Cancel on a session's Replace YOLO question is refused while "don't ask again" saves, so the outcome follows the first answer and a failed save shows its reason
- [complete] `report-failed-settings-saves.md` — Report and revert failed saves of frame colors, quick-launch naming, link-click choice, trust hints and the ⚡ mark; the welcome card's seen save joins the silent list
- [complete] `standard-link-style.md` — One readable link style everywhere: a new link blue, always underlined, the same for link-styled buttons, with ↗ on links that leave the app
- [complete] `restart-with-template-refusal.md` — Restart with and Replace with apply templates whose held fields repeat the session's own values, refusing only real changes
