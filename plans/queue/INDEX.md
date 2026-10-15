# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `terminal-link-safety.md` — Terminal links act only on a real click, on the target the hover honestly shows (four triage outcomes, three complexity gated)
- [landing 64a1b2] `template-editor-unsafe-values.md` — Template editor shows hidden characters in model and folder, read-only like commands
- [landing 64c3d4] `checkout-recovery-host-binding.md` — Agent creates into managed checkouts: retry replay bound to the asking host
- [landing 64e5f6] `claude-trust-decimals.md` — Claude checkout trust edits keep unrelated numbers in Claude's settings exact
- [complete] `ntfy-token-server-change.md` — Waiting alerts: changing the ntfy server's origin drops the saved token, and Settings says so
- [landing 64a7b8] `codex-guide-hook-warning.md` — Codex docs page stops calling the hook warning harmless
- [pending] `browser-test-cleanup.md` — Browser tests clean up their sessions, templates, preferences and fixtures even when an earlier step fails (20 triage outcomes, each gated)
- [complete] `rust-test-portability.md` — Rust tests stop failing on macOS temp paths and APFS names, wait for real completion, keep Git config out of fixtures, and the test-state sweep cannot hang (6 outcomes, each gated)
- [complete] `remote-uninstall-fixes.md` — Remote uninstall finishes on hosts with uutils coreutils and after an interrupted run, and its fixture test runs off GNU systems (3 outcomes, each gated)
- [pending] `launcher-fixes.md` — New-session dialog fixes: retry key after a template resets permissions, Launch after a repository pick, the setup field after the folder is cleared, custom flags kept on a model choice, and the active repository suggestion kept visible (5 outcomes, each gated)
- [complete] `update-failure-reporting.md` — The version hover names why an automatic update install failed, and the install guide stops saying failures are only logged (2 outcomes, each gated)
- [complete] `docs-accuracy-fixes.md` — The paste guide and resume docs stop overpromising, and the docs preview refreshes release notes on edit (3 outcomes, each gated)
- [complete] `host-picker-dotted-i.md` — Typing İ repeatedly in the host picker cycles through hosts whose names start with it (gated)
- [complete] `setup-advice-state-directory.md` — Setup's sign-in advice names the state directory when it came from the environment (gated)
- [complete] `copy-guidance-resize.md` — The "this drag did not copy" notice is measured at its origin so narrowing the window does not collapse it (gated)
- [complete] `brick-test-feedback-evidence.md` — The release brick test keeps the final guest when failed feedback checks' exported evidence is missing (gated)
- [in-flight a6d39e] `managed-checkout-row-accessibility.md` — Full-size session rows for managed checkouts expose the working folder and trash note to screen readers (gated)
- [in-flight e0ad5d] `download-long-name-copies.md` — A second desktop download of a file with a near-limit name shortens the stem so the numbered copy fits (gated)
- [pending] `save-template-ime-escape.md` — Escape during input-method composition no longer closes the save-template panel and its draft (gated)
- [pending] `helm-start-without-ca-bundle.md` — A Linux helm without CA certificates starts, with downloads refusing, instead of exiting at startup (gated)
- [pending] `stopped-session-launch-error.md` — A cached launch error no longer overrides a session's recorded stopped status (gated)
- [pending] `trash-totals-unchecked.md` — A host's trash totals are marked partial or unknown when unchecked entries exist, instead of showing 0 B (gated)
- [pending] `checkout-override-error-wording.md` — A checkout failure caused by a per-host folder override names that override instead of pointing at Settings (gated)
