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
- [`session-details-bottom`](reports/session-details-bottom.report.md) landed 2026-10-18 in #2253: session view: identity and host in a bar under the terminal, tabs and actions share the top row
- [`codex-trust-prompts`](reports/codex-trust-prompts.report.md) landed 2026-10-18 in #2255, #2256: Codex, Muse and Pi workspace trust move from the launcher to per-agent Settings switches for managed checkouts
- [`prompt-running-agent`](reports/prompt-running-agent.report.md) landed 2026-10-18 in #2257, #2265: farhelm command to type a prompt into another session's running Claude or Codex
- [`launcher-enter-focus`](reports/launcher-enter-focus.report.md) landed 2026-10-18 in #2267: dialogs keep focus on empty-space clicks, and Enter on the dialog itself presses its main button
- [`host-terminal-colors`](reports/host-terminal-colors.report.md) landed 2026-10-18 in #2266, #2269: local host picks its icon and color, red for every host, optional host-colored terminal frame
- [`agent-wrappers`](reports/agent-wrappers.report.md) landed 2026-10-18 in #2271, #2272: optional command prefix for agent launches, treated exactly like the real agent
- [`agent-quick-launch-templates`](reports/agent-quick-launch-templates.report.md) landed 2026-10-18 in #2273, #2274: agents can set the quick-launch template and number keys through the farhelm command
- [`json-download-open`](reports/json-download-open.report.md) landed 2026-10-18 in #2296, #2299: Cmd-click opens downloads of the types listed in a new setting (md, txt, json by default), with loud warnings for types not known to be safe
- [`launcher-typed-guarded`](reports/launcher-typed-guarded.report.md) landed 2026-10-18 in #2297, #2301: typing guarded in the launcher selects Guarded like yolo, plus typed routes for the remaining permission and effort choices
- [`session-hover-text`](reports/session-hover-text.report.md) landed 2026-10-18 in #2293, #2300: hovering a session row shows one styled summary card: host, checkout, agent, times
