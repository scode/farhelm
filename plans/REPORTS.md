# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`deliberate-setup-spec`](reports/deliberate-setup-spec.report.md) landed 2026-10-15 in #2075, #2077: SPEC.md: proportionate precautions against deliberate local setup (OS-trusted proxy, retargeted symlink)
- [`trash-unchecked-wording`](reports/trash-unchecked-wording.report.md) landed 2026-10-15 in #2080: Trash cleanup's unchecked-entry text warns that verifiable entries are deleted permanently
- [`triage-layout-filter-and-devbuild`](reports/triage-layout-filter-and-devbuild.report.md) landed 2026-10-15 in #2076, #2079, #2081: Review queue: add the oversized-dialog filter and drop two layout findings under it, discard the devbuild finding
- [`adopt-identity-escaping`](reports/adopt-identity-escaping.report.md) landed 2026-10-15 in #2083: Adopt prompt: distinct installation identities never render alike
- [`claude-trust-decimals`](reports/claude-trust-decimals.report.md) landed 2026-10-15 in #2089: Claude checkout trust edits keep unrelated numbers in Claude's settings exact
- [`checkout-recovery-host-binding`](reports/checkout-recovery-host-binding.report.md) landed 2026-10-15 in #2086: Agent creates into managed checkouts: retry replay bound to the asking host
- [`template-editor-unsafe-values`](reports/template-editor-unsafe-values.report.md) landed 2026-10-15 in #2087: Template editor shows hidden characters in model and folder, read-only like commands
