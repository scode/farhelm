# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 85c393] `session-group-follow-ups.md` — Session groups: indent members, mark a collapsed group holding the open session, drag sessions into and out of groups
- [landing d604da] `model-families.md` — Known models with a latest model per family, replacing the retired-model special case
- [landing 96f90c] `session-limit-notice.md` — Rail bell notice when a host or the helm hits the 500-session ceiling
- [landing b7a330] `reserved-resource-names.md` — SPEC.md: anything under Farhelm's reserved resource names or ownership labels may be treated as Farhelm's
- [in-flight cf10b9] `drawn-frame-actions.md` — Actions act on what the user was shown (header, row menu, launcher, environment Stop and suspend, Wayland drop): one spec standard, one mechanism
- [complete] `secrets-logs-and-screen.md` — The WebSocket trace never logs the desktop credential; the Codex login editor never shows a chosen file
- [complete] `checkout-settings-stale-save.md` — Settings → Checkouts saves refuse a value the command line changed meanwhile, through one shared mechanism
- [complete] `concealed-link-target.md` — Hidden terminal text cannot change where a printed link goes
- [complete] `triage-misc-nov18.md` — natural-detach input order, launching containers out of the leftovers list, a test fixture's group kill, two spec notes
- [complete] `triage-discards-nov18.md` — nine discards from the 2026-11-18 highest-bucket triage
- [complete] `covered-link-refusal.md` — A terminal link or file click is refused, with its reason raised above the card, when an approval card, menu or dialog covers its details panel
- [pending] `exited-session-no-terminal.md` — An exited session whose terminal is gone shows Restart and Replace instead of reconnecting forever (protocol bump)
- [pending] `shift-drag-marking.md` — Shift+drag across the session list marks the rows it sweeps (after `session-group-follow-ups.md`)
- [complete] `demo-video-capture.md` — The README demo video's capture runs on the desktop app again (mechanism only)
- [complete] `replace-with-checkout-hint.md` — Replace with on a managed checkout says the replacement keeps its checkout, and warns before a fresh clone
- [in-flight 7e75ca] `one-helm-per-supervisor.md` — Each host records its owning helm, refuses others, and is taken over only on request (protocol bump)
- [pending] `model-family-shorthand.md` — Typing a model family word in the launcher picks that family's latest model (after `model-families.md`)
- [in-flight cc131e] `unusable-folder-refusals.md` — A folder that cannot be entered and a zero-size terminal are refused clearly; six tests lost with tmux come back
