# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `browser-test-cleanup.md` — Browser tests clean up their sessions, templates, preferences and fixtures even when an earlier step fails (20 triage outcomes, each gated)
- [complete] `rust-test-portability.md` — Rust tests stop failing on macOS temp paths and APFS names, wait for real completion, keep Git config out of fixtures, and the test-state sweep cannot hang (6 outcomes, each gated)
- [pending] `launcher-fixes.md` — New-session dialog fixes: retry key after a template resets permissions, Launch after a repository pick, the setup field after the folder is cleared, custom flags kept on a model choice, and the active repository suggestion kept visible (5 outcomes, each gated)
- [landing 65c0d8] `copy-guidance-resize.md` — The "this drag did not copy" notice is measured at its origin so narrowing the window does not collapse it (gated)
- [landing 65c0d9] `brick-test-feedback-evidence.md` — The release brick test keeps the final guest when failed feedback checks' exported evidence is missing (gated)
- [complete] `managed-checkout-row-accessibility.md` — Full-size session rows for managed checkouts expose the working folder and trash note to screen readers (gated)
- [complete] `download-long-name-copies.md` — A second desktop download of a file with a near-limit name shortens the stem so the numbered copy fits (gated)
- [complete] `save-template-ime-escape.md` — Escape during input-method composition no longer closes the save-template panel and its draft (gated)
- [complete] `helm-start-without-ca-bundle.md` — A Linux helm without CA certificates starts, with downloads refusing, instead of exiting at startup (gated)
- [complete] `stopped-session-launch-error.md` — A cached launch error no longer overrides a session's recorded stopped status (gated)
- [complete] `trash-totals-unchecked.md` — A host's trash totals are marked partial or unknown when unchecked entries exist, instead of showing 0 B (gated)
- [complete] `checkout-override-error-wording.md` — A checkout failure caused by a per-host folder override names that override instead of pointing at Settings (gated)
