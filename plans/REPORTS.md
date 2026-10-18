# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`keyboard-scroll-audit`](reports/keyboard-scroll-audit.report.md) landed 2026-10-17 in #2222, #2226: keyboard reach for the remaining scroll regions
- [`quick-launch-chords`](reports/quick-launch-chords.report.md) landed 2026-10-17 in #2220, #2225, #2228: compose quick launch templates with a ⌘X launch key sequence
- [`session-action-shortcuts`](reports/session-action-shortcuts.report.md) landed 2026-10-17 in #2218, #2221, #2230: keyboard shortcuts to delete and rename the open session (⌘W, ⌘L)
- [`tunnel-localhost-links`](reports/tunnel-localhost-links.report.md) landed 2026-10-17 in #2219, #2227, #2229, #2231: open a remote session's localhost link through a tunnel in the Mac app
- [`menu-focus-source`](reports/menu-focus-source.report.md) landed 2026-10-17 in #2232: menu arrows follow focus moved while arrow presses are landing
- [`name-session-while-starting`](reports/name-session-while-starting.report.md) landed 2026-10-17 in #2233, #2234: name a quick-launched session in a rename dialog while it starts
- [`one-way-yolo-shield`](reports/one-way-yolo-shield.report.md) landed 2026-10-18 in #2251: the observed YOLO shield only ever moves toward YOLO within an agent run
- [`covered-link-hover-refusal`](reports/covered-link-hover-refusal.report.md) landed 2026-10-18 in #2254: terminal file and web links refuse a click while an approval card covers their hover
