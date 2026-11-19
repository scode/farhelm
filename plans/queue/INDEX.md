# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing d04a88] `drawn-frame-actions.md` — Actions act on what the user was shown (header, row menu, launcher, environment Stop and suspend, Wayland drop): one spec standard, one mechanism
- [pending] `exited-session-no-terminal.md` — An exited session whose terminal is gone shows Restart and Replace instead of reconnecting forever (protocol bump)
- [landing 8e3744] `one-helm-per-supervisor.md` — Each host records its owning helm, refuses others, and is taken over only on request (protocol bump)
- [landing dbb006] `gui-ux-discards.md` — Fifty discards from the 2026-11-18 GUI UX triage, in one PR
- [landing 8e536e] `gui-ux-backend-test-gaps.md` — Fifteen small supervisor, terminal-host and e2e test fixes from the 2026-11-18 GUI UX triage
- [landing 1d4040] `gui-ux-gui-test-gaps.md` — Five small desktop app, helm and release-test fixes from the 2026-11-18 GUI UX triage
- [pending] `gui-ux-wording.md` — Comment, spec and docs corrections from the 2026-11-18 GUI UX triage, two spec acceptances and the user's-own-configuration principle
- [pending] `gui-ux-desktop-fixes.md` — Header question anchors, Close all after navigation, selected-template contrast, host text in tooltips and refusals, wrong action names, approval keyboard tab stops
- [pending] `gui-ux-backend-fixes.md` — Bounded shell-tab requests, Tensorlake suspension attribution, late approval answers, tunnels for retired sessions, the Mac update wait, endpoint @ paths, braced command display
- [complete] `gui-ux-enter-and-focus.md` — Presses after the user's own action count as moving on; the half-second fence is for Escape only; Confirm-first for trash deletion; focus home after bulk actions
- [pending] `antigravity-support.md` — Support Google Antigravity (agy): structured launches, sidebar mark, user-installed hook conversation tracking, and docs (protocol bump)
- [complete] `settings-app-version.md` — Settings → App shows the running version with a Check for updates button that shows a running check
