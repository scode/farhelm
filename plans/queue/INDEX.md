# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `settings-search.md` — Search field in Settings: section list narrows to matches, pane lists matches, jump to the setting (after `docs-shot-desktop-fixes.md`)
- [pending] `ssh-askpass-2fa.md` — SSH sign-in prompts: relay what ssh asks (2FA codes, passwords, host keys) to an attention row and popover, inline in Add host; Sign-in needed after cancel or expiry; keepalives; remove SSH needs 2FA?
