# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [blocked] `drawn-frame-actions.md` — Actions act on what the user was shown (header, row menu, launcher, environment Stop and suspend, Wayland drop): one spec standard, one mechanism
- [complete] `covered-link-refusal.md` — A terminal link or file click is refused, with its reason raised above the card, when an approval card, menu or dialog covers its details panel
- [pending] `exited-session-no-terminal.md` — An exited session whose terminal is gone shows Restart and Replace instead of reconnecting forever (protocol bump)
- [in-flight ce95bc] `shift-drag-marking.md` — Shift+drag across the session list marks the rows it sweeps (after `session-group-follow-ups.md`)
- [complete] `demo-video-capture.md` — The README demo video's capture runs on the desktop app again (mechanism only)
- [complete] `replace-with-checkout-hint.md` — Replace with on a managed checkout says the replacement keeps its checkout, and warns before a fresh clone
- [in-flight 7e75ca] `one-helm-per-supervisor.md` — Each host records its owning helm, refuses others, and is taken over only on request (protocol bump)
- [in-flight 48ff3e] `model-family-shorthand.md` — Typing a model family word in the launcher picks that family's latest model (after `model-families.md`)
- [complete] `unusable-folder-refusals.md` — A folder that cannot be entered and a zero-size terminal are refused clearly; six tests lost with tmux come back
