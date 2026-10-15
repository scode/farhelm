# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 60c6cd] `terminal-link-safety.md` — Terminal links act only on a real click, on the target the hover honestly shows (four triage outcomes, three complexity gated)
- [landing 63b9e1] `adopt-identity-escaping.md` — Adopt prompt: distinct installation identities never render alike
- [in-flight 5ba68a] `template-editor-unsafe-values.md` — Template editor shows hidden characters in model and folder, read-only like commands
- [complete] `checkout-recovery-host-binding.md` — Agent creates into managed checkouts: retry replay bound to the asking host
- [in-flight 74ab98] `claude-trust-decimals.md` — Claude checkout trust edits keep unrelated numbers in Claude's settings exact
- [pending] `ntfy-token-server-change.md` — Waiting alerts: changing the ntfy server's origin drops the saved token, and Settings says so
- [pending] `codex-guide-hook-warning.md` — Codex docs page stops calling the hook warning harmless
