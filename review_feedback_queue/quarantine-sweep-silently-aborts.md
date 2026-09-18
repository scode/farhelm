# The quarantine sweep silently stops at the first unreadable entry

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A single transient disk hiccup during startup cleanup can silently leave whole deleted-session folders behind, with no
warning anywhere that the cleanup gave up partway.

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: definite. Three lenses agreed (state-lifecycle,
general, data-flow). Coordinator confirmed the silent arm and the warning siblings; the restater corrected which sibling
is which (below) and the coordinator verified the correction.

`discard_quarantine_root` (`crates/farhelm-supervisor/src/attachments.rs:446-458`) iterates with
`while let
Ok(Some(entry))` (455): the first `Err` — transient I/O error, a file vanishing mid-scan — silently ends the
whole pass with no warning and no marker of partial completion. Every sibling warns: the staging sweeper matches
explicitly with warn-and-break (475-489), and the top-level reconciliation loop warns and breaks on error (435-439).
Even within the quarantine function, the `read_dir` arm warns on failure (449-452) — only the iterator arm, covering
every subsequent entry, swallows errors. Startup reconciliation is the backstop every lifecycle path names; reporting it
complete when it bailed early is the wrong answer from the last line of defense.

Suggested fix: match explicitly like `sweep_staging` — warn-and-break (or warn-and-continue per entry) on `Err`, so a
partial sweep is at least visible.
