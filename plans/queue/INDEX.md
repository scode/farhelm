# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `template-name-escape.md` — Template editor: an older template's unsafe session name shows escaped and read-only, like model, folder and commands
- [pending] `replace-retry-identity.md` — Replace retries: one retry identity per source, and a fresh attempt after a definite refusal (2 outcomes, each gated)
