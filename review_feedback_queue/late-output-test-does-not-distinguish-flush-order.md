# The late-output regression does not distinguish flush order

Reviewed commit: a98147fb2eaaefb448473107529ada69114d7b03

## TLDR

The test for drawing an overdue terminal frame before processing new output would also pass if those steps were
reversed. No current product ordering defect is asserted.

## Details

`crates/farhelm-terminal-emulator/tests/sync.rs:90–96` opens a synchronized update containing `held`, feeds the plain
suffix ` after` after the deadline, then checks only the final `held after` screen and absence of a deadline. Processing
that plain suffix into the old buffered update before flushing yields the same observations.

The implementation currently has the required order at `src/emulator.rs:118–127`: flush an expired update before
advancing the parser. Strengthen the test with observations distinguishing an overdue frame already drawn from newly
arriving output still held in another frame. An absent-flush check alone does not protect flush-before-parse ordering.

Suggested bucket: `other`. Confidence: confirmed test-observability gap, with no missing source premise for that narrow
claim. No runtime mutation or product regression was demonstrated. Proposed drop: none. Required coverage searches found
no matching specification acceptance, Planned item, unlanded plan, known bug, queue item, ledger decision or filter. The
completed browser/tmux deadline-test fixes in the ledger concern different fixtures.

Provenance: g048/f1; Codex gpt-6.1-sol, high reasoning. Source inspected without executing tests or reading upstream
vendor trees. The finding corrects the initial reassurance that this test protected ordering.
