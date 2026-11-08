# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight d2b5ac] `tensorlake-credentials-polish.md` — Settings → Credentials shows one plain-text storage warning for the whole section and labels its key field "Tensorlake API key"
- [in-flight ec23f9] `recipe-bases.md` — A Tensorlake recipe gets a base (Tensorlake Ubuntu, your image or your snapshot) that Farhelm always builds on with packages, Claude Code and Codex, checking the base's requirements first
- [pending] `prepared-snapshots.md` — Farhelm-managed prepared snapshots per recipe (supervisor installed, no identity or secrets, stale after 24 h or an update, prepared lazily); new Tensorlake hosts start from them (after `recipe-bases.md`)
- [pending] `sandbox-sessions.md` — Launch a session in a named Tensorlake sandbox of its own from a recipe (launcher, templates, Clone), with explicit Suspend/Resume, Stop and suspend, Delete removing the sandbox, and agent operations refused (after `prepared-snapshots.md`)
