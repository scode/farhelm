# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `readopt-unavailable-hosts.md` — Re-adopt terminal hosts (agents and tabs) that become unavailable while the supervisor runs
- [in-flight 6d0a5b] `faster-snapshot-encoder.md` — Make terminal checkpoint encoding faster with byte-identical output, measured by a new benchmark
- [complete] `stop-checkpoint-burst.md` — Gentler checkpoint burst at an orderly supervisor stop: skip small tails, half the cores, biggest first
- [pending] `vs16-emoji-width.md` — Grid terminals: size emoji variation sequences (U+FE0F, U+FE0E) the way Ghostty does
- [in-flight 56bf16] `colored-harness-marks.md` — Color the sidebar's harness marks where brand terms allow, official colors first
- [in-flight 66b8cd] `discord-link-in-app.md` — Add "Chat on Discord" to the ? menu and the feedback dialog
- [in-flight c96b93] `grid-drop-and-drag-notice.md` — Grid terminals: accept dropped files and show the drag-copy notice, placed 3 rows off the pointer
- [pending] `in-app-announcements.md` — An announcement queue the app shows once per helm, fed for now by announcements built into releases
- [pending] `launch-progress.md` — Show a live "Launching on host… 14 s" status while a launch, Restart or Replace waits
