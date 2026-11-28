# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`delete-dead-replace-checkout-ui`](reports/delete-dead-replace-checkout-ui.report.md) landed 2026-11-27 in #5160: Delete dead fresh-checkout Replace UI remnants and close matching review finding
- [`file-hover-too-large-hint`](reports/file-hover-too-large-hint.report.md) landed 2026-11-27 in #5162: Show "too large to copy contents" on terminal file hover cards over the 10 MB copy cap
- [`agent-hosts-tailscale-column`](reports/agent-hosts-tailscale-column.report.md) landed 2026-11-27 in #5161: Add a TAILSCALE column to plain `farhelm agent hosts` table output
- [`cli-flag-parsing-audit`](reports/cli-flag-parsing-audit.report.md) landed 2026-11-27 in #5163, #5165: Stop free-text CLI options and positionals from swallowing flags and clean up clap parsing quirks
- [`drift-base-isolation-and-turns`](reports/drift-base-isolation-and-turns.report.md) landed 2026-11-27 in #5167, #5168: Add Codex process-tree isolation and Cursor/OpenCode interactive turns to the base drift category
- [`drift-permission-footer`](reports/drift-permission-footer.report.md) landed 2026-11-27 in #5169: Extend permission_modes drift checks with Guarded launches and tri-state permission-footer verification
- [`terminal-tab-shortcuts`](reports/terminal-tab-shortcuts.report.md) landed 2026-11-27 in #5164, #5166: Add Cmd+T/Cmd+W terminal-tab shortcuts and always confirm Delete when shell tabs are open
- [`drift-conversation-rotation`](reports/drift-conversation-rotation.report.md) landed 2026-11-27 in #5170, #5171: Add conversation_rotation drift category replacing standalone rotation tests and covering seven tracked harnesses
- [`drift-persistent-hooks`](reports/drift-persistent-hooks.report.md) landed 2026-11-27 in #5173: Add persistent_hooks drift category verifying Grok and Antigravity hooks inside and outside Farhelm
- [`terminal-tab-overflow-ux`](reports/terminal-tab-overflow-ux.report.md) landed 2026-11-28 in #5176, #5177: Pin the main Agent tab and add auto-scroll, edge fades, arrows, and an all-tabs menu for overflowing tabs
