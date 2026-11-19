# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `ssh-askpass-2fa.md` — SSH sign-in prompts: relay what ssh asks (2FA codes, passwords, host keys) to an attention row and popover, inline in Add host; Sign-in needed after cancel or expiry; keepalives; remove SSH needs 2FA?
- [pending] `auto-update-hosts-default.md` — Update remote hosts automatically on by default for new and upgraded installs (pre-change opt-outs cleared); brick test step 5 observes the automatic update; devbuilds start with it off or get a TODO follow-up
