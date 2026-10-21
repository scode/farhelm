# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 7f4bcc] `spare-launch-checkout.md` — Keep one spare checkout per repository ready in a hidden folder inside the checkout folder, so a managed-checkout launch updates it from GitHub instead of re-cloning, with the background build, a Settings switch on by default, and Git cache consolidation as a standalone first PR (1 TODO entry).
