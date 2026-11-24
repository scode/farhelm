# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight a883f2] `dynamic-announcements.md` — Add a signed central announcements feed on get.farhelm.io with helm polling, bell notice, Settings toggle, and P2 brick test
- [pending] `osc8-bracketed-url-warnings.md` — Restore terminal hyperlink mismatch warnings for nested bracketed and quoted URLs
- [pending] `light-mode-upgrade-pill-pulse.md` — Make the light-mode pending-update pill reminder pulse visible and bold its arrow in both themes
- [pending] `preserve-creds-on-lookup-failure.md` — Preserve remote agent-credentials on SQLite lookup failure and name the affected agent
- [pending] `custom-agents-cli-visibility.md` — Add `farhelm agent agents` discovery, include custom agents in unknown-agent refusals, forbid `--conversation` with recipes, and list valid built-in efforts
