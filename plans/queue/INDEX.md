# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `prepared-snapshots.md` — Farhelm-managed prepared snapshots per recipe (supervisor installed, no identity or secrets, stale after 24 h or an update, prepared lazily); new Tensorlake hosts start from them (after `recipe-bases.md`)
- [pending] `sandbox-sessions.md` — Launch a session in a named Tensorlake sandbox of its own from a recipe (launcher, templates, Clone), with explicit Suspend/Resume, Stop and suspend, Delete removing the sandbox, and agent operations refused (after `prepared-snapshots.md`)
- [pending] `tensorlake-name-ownership.md` — Tensorlake: adopt or delete a sandbox by name only when this helm asked for it, or spec that shared state is unsupported
- [in-flight cce2fa] `tensorlake-settings-prompts.md` — Tensorlake settings: bind the leftover Delete question, word the recipe Delete question honestly
