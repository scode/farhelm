# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `file-hover-too-large-hint.md` — Show "too large to copy contents" on terminal file hover cards over the 10 MB copy cap
- [complete] `cli-flag-parsing-audit.md` — Stop free-text CLI options and positionals from swallowing flags and clean up clap parsing quirks
- [complete] `agent-hosts-tailscale-column.md` — Add a TAILSCALE column to plain `farhelm agent hosts` table output
- [in-flight 871c57] `terminal-tab-shortcuts.md` — Add Cmd+T/Cmd+W terminal-tab shortcuts and always confirm Delete when shell tabs are open
- [pending] `terminal-tab-overflow-ux.md` — Pin the main Agent tab and add auto-scroll, edge fades, arrows, and an all-tabs menu for overflowing tabs
- [in-flight 9c521b] `blocked-notify-attention-and-debounce.md` — Move blocked-agent alerts to the red waiting indicator and sound with an ntfy awareness grace period
- [pending] `ntfy-quiet-hours-urgency.md` — Add local-time quiet hours and quiet-hours priority/off setting for ntfy alerts
- [in-flight 23f6b1] `drift-base-isolation-and-turns.md` — Add Codex process-tree isolation and Cursor/OpenCode interactive turns to the base drift category
- [pending] `drift-persistent-hooks.md` — Add persistent_hooks drift category verifying Grok and Antigravity hooks inside and outside Farhelm (after `drift-base-isolation-and-turns.md`)
- [in-flight 37690c] `drift-conversation-rotation.md` — Add conversation_rotation drift category replacing standalone rotation tests and covering seven tracked harnesses
- [pending] `drift-permission-footer.md` — Extend permission_modes drift checks with Guarded launches and tri-state permission-footer verification
