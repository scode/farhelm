# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`shared-placement-picker`](reports/shared-placement-picker.report.md) landed 2026-11-19 in #4130, #4133: Shared placement picker drops its no-op press and moves to a shared module, with the template editor test
- [`submenus-open-on-hover`](reports/submenus-open-on-hover.report.md) landed 2026-11-19 in #4139: Submenus open on hover, keeping their parent item highlighted
- [`clone-keeps-invisible-characters`](reports/clone-keeps-invisible-characters.report.md) landed 2026-11-19 in #4131, #4132: Launcher fields keep invisible characters when edited; the suspended-host refusal matches the button
- [`retire-old-terminal-announcement`](reports/retire-old-terminal-announcement.report.md) landed 2026-11-19 in #4140: Remove the stale 0.37.0 old-terminal in-app announcement
- [`update-survival-wording`](reports/update-survival-wording.report.md) landed 2026-11-19 in #4145: General survival statements stay general: SPEC.md records the principle and the old-terminal update note
- [`quick-switcher-shot-paths`](reports/quick-switcher-shot-paths.report.md) landed 2026-11-19 in #4138: Quick switcher shows staged folders in agent builds, so its screenshots pass the privacy check
- [`session-group-follow-ups`](reports/session-group-follow-ups.report.md) landed 2026-11-19 in #4135, #4143, #4144, #4146, #4147: Session groups: indent members, mark a collapsed group holding the open session, drag sessions into and out of groups
- [`model-families`](reports/model-families.report.md) landed 2026-11-19 in #4149, #4152: Known models with a latest model per family, replacing the retired-model special case
- [`session-limit-notice`](reports/session-limit-notice.report.md) landed 2026-11-19 in #4150: Rail bell notice when a host or the helm hits the 500-session ceiling
- [`reserved-resource-names`](reports/reserved-resource-names.report.md) landed 2026-11-19 in #4148: SPEC.md: anything under Farhelm's reserved resource names or ownership labels may be treated as Farhelm's
- [`secrets-logs-and-screen`](reports/secrets-logs-and-screen.report.md) landed 2026-11-19 in #4151, #4153: The WebSocket trace never logs the desktop credential; the Codex login editor never shows a chosen file
