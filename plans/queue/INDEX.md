# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 8bd3d9] `delete-one-trashed-checkout.md` — a delete button on each checkout in the trash dialog, deleting it at once without a confirmation
- [in-flight 3951aa] `update-check-feedback.md` — show a user-started update check's progress and outcome inline on the desktop app's version readout
