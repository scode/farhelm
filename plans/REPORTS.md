# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`covered-link-hover-refusal`](reports/covered-link-hover-refusal.report.md) landed 2026-10-18 in #2254: terminal file and web links refuse a click while an approval card covers their hover
- [`session-details-bottom`](reports/session-details-bottom.report.md) landed 2026-10-18 in #2253: session view: identity and host in a bar under the terminal, tabs and actions share the top row
- [`codex-trust-prompts`](reports/codex-trust-prompts.report.md) landed 2026-10-18 in #2255, #2256: Codex, Muse and Pi workspace trust move from the launcher to per-agent Settings switches for managed checkouts
- [`launcher-enter-focus`](reports/launcher-enter-focus.report.md) landed 2026-10-18 in #2267: dialogs keep focus on empty-space clicks, and Enter on the dialog itself presses its main button
- [`host-terminal-colors`](reports/host-terminal-colors.report.md) landed 2026-10-18 in #2266, #2269: local host picks its icon and color, red for every host, optional host-colored terminal frame
- [`agent-wrappers`](reports/agent-wrappers.report.md) landed 2026-10-18 in #2271, #2272: optional command prefix for agent launches, treated exactly like the real agent
- [`agent-quick-launch-templates`](reports/agent-quick-launch-templates.report.md) landed 2026-10-18 in #2273, #2274: agents can set the quick-launch template and number keys through the farhelm command
- [`json-download-open`](reports/json-download-open.report.md) landed 2026-10-18 in #2296, #2299: Cmd-click opens downloads of the types listed in a new setting (md, txt, json by default), with loud warnings for types not known to be safe
- [`launcher-typed-guarded`](reports/launcher-typed-guarded.report.md) landed 2026-10-18 in #2297, #2301: typing guarded in the launcher selects Guarded like yolo, plus typed routes for the remaining permission and effort choices
- [`session-hover-text`](reports/session-hover-text.report.md) landed 2026-10-18 in #2293, #2300: hovering a session row shows one styled summary card: host, checkout, agent, times
- [`launcher-template-keys`](reports/launcher-template-keys.report.md) landed 2026-10-18 in #2304, #2306, #2310, #2316: template shortcuts apply templates inside the open launcher, through one shared shortcut definition, and Cmd-J launches a template by name
- [`host-uninstall-dialogs`](reports/host-uninstall-dialogs.report.md) landed 2026-10-19 in #2298, #2303, #2305, #2307, #2311, #2317: host setup, update and uninstall show their steps as a stepper in the dialog and in the row's progress, refusals open in dialogs, and the post-uninstall notice shrinks to one line
