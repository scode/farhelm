# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `spare-launch-checkout.md` — Keep one spare checkout per repository ready in a hidden folder inside the checkout folder, so a managed-checkout launch updates it from GitHub instead of re-cloning, with the background build, a Settings switch on by default, and Git cache consolidation as a standalone first PR (1 TODO entry).
- [complete] `yolo-confirm-launch-guarded.md` — Offer "start guarded session instead" in the YOLO confirmation of the session launcher and Restart with, switching the same launch to guarded in one click (1 TODO entry). (after `ui-peer-text-type.md`)
- [complete] `host-changed-update-warning.md` — Stop the false "host changed" warning after updating a host whose identity was learned during the update, and reword it and its submitting twin to name what changed, clearing once the host runs this helm's version (1 TODO entry). (after `ui-peer-text-type.md`)
