# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `session-group-follow-ups.md` — Session groups: indent members, mark a collapsed group holding the open session, drag sessions into and out of groups
- [landing ed32b0] `submenus-open-on-hover.md` — Submenus open on hover, keeping their parent item highlighted
- [complete] `clone-keeps-invisible-characters.md` — Launcher fields keep invisible characters when edited; the suspended-host refusal matches the button
- [complete] `retire-old-terminal-announcement.md` — Remove the stale 0.37.0 old-terminal in-app announcement
- [complete] `quick-switcher-shot-paths.md` — Quick switcher shows staged folders in agent builds, so its screenshots pass the privacy check
- [complete] `model-families.md` — Known models with a latest model per family, replacing the retired-model special case
- [complete] `session-limit-notice.md` — Rail bell notice when a host or the helm hits the 500-session ceiling
- [complete] `update-survival-wording.md` — General survival statements stay general: SPEC.md records the principle and the old-terminal update note
- [complete] `reserved-resource-names.md` — SPEC.md: anything under Farhelm's reserved resource names or ownership labels may be treated as Farhelm's
- [pending] `drawn-frame-actions.md` — Actions act on what the user was shown (header, row menu, launcher, environment Stop and suspend, Wayland drop): one spec standard, one mechanism
- [complete] `secrets-logs-and-screen.md` — The WebSocket trace never logs the desktop credential; the Codex login editor never shows a chosen file
- [complete] `checkout-settings-stale-save.md` — Settings → Checkouts saves refuse a value the command line changed meanwhile, through one shared mechanism
- [complete] `concealed-link-target.md` — Hidden terminal text cannot change where a printed link goes
- [in-flight 441138] `triage-misc-nov18.md` — natural-detach input order, launching containers out of the leftovers list, a test fixture's group kill, two spec notes
- [in-flight 7b7d14] `triage-discards-nov18.md` — nine discards from the 2026-11-18 highest-bucket triage
