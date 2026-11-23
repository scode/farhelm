# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 9814e8] `host-auto-update-after-2fa.md` — Prompt for SSH sign-in and resume automatic remote-host updates (after `remove-host-signin-lock.md`)
- [in-flight 826f22] `custom-agents.md` — Add user-registered custom agents, the custom-agent contract, CLI reporter, and docs
- [complete] `positional-hook-attribution.md` — Attribute Grok and Codex hook reports to the process that ran the hook, as the default rule for every agent
