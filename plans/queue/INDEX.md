# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `grid-repeated-link-warning.md` — Grid terminals: stop a link a program starts twice from showing a false "does not match" warning
- [in-flight 91102f] `readopt-unavailable-hosts.md` — Re-adopt terminal hosts (agents and tabs) that become unavailable while the supervisor runs
- [in-flight 6d0a5b] `faster-snapshot-encoder.md` — Make terminal checkpoint encoding faster with byte-identical output, measured by a new benchmark
- [in-flight 7e86ec] `stop-checkpoint-burst.md` — Gentler checkpoint burst at an orderly supervisor stop: skip small tails, half the cores, biggest first
- [pending] `vs16-emoji-width.md` — Grid terminals: size emoji variation sequences (U+FE0F, U+FE0E) the way Ghostty does
- [pending] `colored-harness-marks.md` — Color the sidebar's harness marks where brand terms allow, official colors first
- [pending] `discord-link-in-app.md` — Add "Chat on Discord" to the ? menu and the feedback dialog
