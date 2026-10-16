# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`deliberate-setup-spec`](reports/deliberate-setup-spec.report.md) landed 2026-10-15 in #2075, #2077: SPEC.md: proportionate precautions against deliberate local setup (OS-trusted proxy, retargeted symlink)
- [`trash-unchecked-wording`](reports/trash-unchecked-wording.report.md) landed 2026-10-15 in #2080: Trash cleanup's unchecked-entry text warns that verifiable entries are deleted permanently
- [`triage-layout-filter-and-devbuild`](reports/triage-layout-filter-and-devbuild.report.md) landed 2026-10-15 in #2076, #2079, #2081: Review queue: add the oversized-dialog filter and drop two layout findings under it, discard the devbuild finding
- [`adopt-identity-escaping`](reports/adopt-identity-escaping.report.md) landed 2026-10-15 in #2083: Adopt prompt: distinct installation identities never render alike
- [`claude-trust-decimals`](reports/claude-trust-decimals.report.md) landed 2026-10-15 in #2089: Claude checkout trust edits keep unrelated numbers in Claude's settings exact
- [`checkout-recovery-host-binding`](reports/checkout-recovery-host-binding.report.md) landed 2026-10-15 in #2086: Agent creates into managed checkouts: retry replay bound to the asking host
- [`template-editor-unsafe-values`](reports/template-editor-unsafe-values.report.md) landed 2026-10-15 in #2087: Template editor shows hidden characters in model and folder, read-only like commands
- [`codex-guide-hook-warning`](reports/codex-guide-hook-warning.report.md) landed 2026-10-15 in #2091: Codex docs page stops calling the hook warning harmless
- [`terminal-link-safety`](reports/terminal-link-safety.report.md) landed 2026-10-16 in #2078, #2085, #2092, #2095: Terminal links act only on a real click, on the target the hover honestly shows (four triage outcomes, three complexity gated)
- [`ntfy-token-server-change`](reports/ntfy-token-server-change.report.md) landed 2026-10-16 in #2098: Waiting alerts: changing the ntfy server's origin drops the saved token, and Settings says so
- [`remote-uninstall-fixes`](reports/remote-uninstall-fixes.report.md) landed 2026-10-16 in #2096, #2100, #2102: Remote uninstall finishes on hosts with uutils coreutils and after an interrupted run, and its fixture test runs off GNU systems (3 outcomes, each gated)
- [`update-failure-reporting`](reports/update-failure-reporting.report.md) landed 2026-10-16 in #2105, #2108: The version hover names why an automatic update install failed, and the install guide stops saying failures are only logged (2 outcomes, each gated)
- [`docs-accuracy-fixes`](reports/docs-accuracy-fixes.report.md) landed 2026-10-16 in #2104, #2106, #2109: The paste guide and resume docs stop overpromising, and the docs preview refreshes release notes on edit (3 outcomes, each gated)
- [`host-picker-dotted-i`](reports/host-picker-dotted-i.report.md) landed 2026-10-16 in #2107: Typing İ repeatedly in the host picker cycles through hosts whose names start with it (gated)
- [`setup-advice-state-directory`](reports/setup-advice-state-directory.report.md) landed 2026-10-16 in #2110: Setup's sign-in advice names the state directory when it came from the environment (gated)
- [`copy-guidance-resize`](reports/copy-guidance-resize.report.md) landed 2026-10-16 in #2111: The "this drag did not copy" notice is measured at its origin so narrowing the window does not collapse it (gated)
- [`brick-test-feedback-evidence`](reports/brick-test-feedback-evidence.report.md) landed 2026-10-16 in #2112: The release brick test keeps the final guest when failed feedback checks' exported evidence is missing (gated)
- [`managed-checkout-row-accessibility`](reports/managed-checkout-row-accessibility.report.md) landed 2026-10-16 in #2113: Full-size session rows for managed checkouts expose the working folder and trash note to screen readers (gated)
- [`download-long-name-copies`](reports/download-long-name-copies.report.md) landed 2026-10-16 in #2114: A second desktop download of a file with a near-limit name shortens the stem so the numbered copy fits (gated)
