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
- [`checkout-settings-stale-save`](reports/checkout-settings-stale-save.report.md) landed 2026-11-19 in #4160: Settings → Checkouts saves refuse a value the command line changed meanwhile, through one shared mechanism
- [`concealed-link-target`](reports/concealed-link-target.report.md) landed 2026-11-19 in #4154: Hidden terminal text cannot change where a printed link goes
- [`triage-misc-nov18`](reports/triage-misc-nov18.report.md) landed 2026-11-19 in #4156, #4159, #4163, #4169, #4170: natural-detach input order, launching containers out of the leftovers list, a test fixture's group kill, two spec notes
- [`triage-discards-nov18`](reports/triage-discards-nov18.report.md) landed 2026-11-19 in #4157, #4158, #4161, #4162, #4164, #4165, #4166, #4167, #4168: nine discards from the 2026-11-18 highest-bucket triage
- [`covered-link-refusal`](reports/covered-link-refusal.report.md) landed 2026-11-19 in #4175, #4180, #4186: A terminal link or file click is refused, with its reason raised above the card, when an approval card, menu or dialog covers its details panel
- [`demo-video-capture`](reports/demo-video-capture.report.md) landed 2026-11-19 in #4178, #4184, #4185: The README demo video's capture runs on the desktop app again (mechanism only)
- [`replace-with-checkout-hint`](reports/replace-with-checkout-hint.report.md) landed 2026-11-19 in #4177, #4179: Replace with on a managed checkout says the replacement keeps its checkout, and warns before a fresh clone
- [`shift-drag-marking`](reports/shift-drag-marking.report.md) landed 2026-11-19 in #4199: Shift+drag across the session list marks the rows it sweeps
- [`model-family-shorthand`](reports/model-family-shorthand.report.md) landed 2026-11-19 in #4198: Typing a model family word in the launcher picks that family's latest model
- [`unusable-folder-refusals`](reports/unusable-folder-refusals.report.md) landed 2026-11-19 in #4188, #4190, #4194: A folder that cannot be entered and a zero-size terminal are refused clearly; six tests lost with tmux come back
- [`drawn-frame-actions`](reports/drawn-frame-actions.report.md) landed 2026-11-19 in #4176, #4193, #4204, #4181, #4183: Actions act on what the user was shown (header, row menu, launcher, environment Stop and suspend, Wayland drop): one spec standard, one mechanism
- [`one-helm-per-supervisor`](reports/one-helm-per-supervisor.report.md) landed 2026-11-19 in #4187, #4195, #4200: Each host records its owning helm, refuses others, and is taken over only on request (protocol bump)
- [`gui-ux-discards`](reports/gui-ux-discards.report.md) landed 2026-11-19 in #4203: Fifty discards from the 2026-11-18 GUI UX triage, in one PR
- [`gui-ux-backend-test-gaps`](reports/gui-ux-backend-test-gaps.report.md) landed 2026-11-19 in #4211, #4213, #4214, #4215, #4218, #4220, #4221, #4222, #4223, #4224, #4225, #4226, #4227, #4228, #4229: Fifteen small supervisor, terminal-host and e2e test fixes from the 2026-11-18 GUI UX triage
- [`gui-ux-gui-test-gaps`](reports/gui-ux-gui-test-gaps.report.md) landed 2026-11-19 in #4207, #4209, #4212, #4217, #4219: Five small desktop app, helm and release-test fixes from the 2026-11-18 GUI UX triage
- [`gui-ux-enter-and-focus`](reports/gui-ux-enter-and-focus.report.md) landed 2026-11-19 in #4206, #4231, #4216, #4230: Presses after the user's own action count as moving on; the half-second fence is for Escape only; Confirm-first for trash deletion; focus home after bulk actions
- [`settings-app-version`](reports/settings-app-version.report.md) landed 2026-11-19 in #4233: Settings → App shows the running version with a Check for updates button that shows a running check
- [`exited-session-no-terminal`](reports/exited-session-no-terminal.report.md) landed 2026-11-19 in #4235, #4244: An exited session whose terminal is gone shows Restart and Replace instead of reconnecting forever (protocol bump)
