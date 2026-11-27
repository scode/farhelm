# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [held] `file-hover-too-large-hint.md` — Show "too large to copy contents" on terminal file hover cards over the 10 MB copy cap
- [held] `cli-flag-parsing-audit.md` — Stop free-text CLI options and positionals from swallowing flags and clean up clap parsing quirks
- [held] `agent-hosts-tailscale-column.md` — Add a TAILSCALE column to plain `farhelm agent hosts` table output
- [held] `delete-dead-replace-checkout-ui.md` — Delete dead fresh-checkout Replace UI remnants and close matching review finding
- [held] `terminal-tab-shortcuts.md` — Add Cmd+T/Cmd+W terminal-tab shortcuts and always confirm Delete when shell tabs are open
- [held] `terminal-tab-overflow-ux.md` — Pin the main Agent tab and add auto-scroll, edge fades, arrows, and an all-tabs menu for overflowing tabs
- [held] `blocked-notify-attention-and-debounce.md` — Move blocked-agent alerts to the red waiting indicator and sound with an ntfy awareness grace period
- [held] `ntfy-quiet-hours-urgency.md` — Add local-time quiet hours and quiet-hours priority/off setting for ntfy alerts

