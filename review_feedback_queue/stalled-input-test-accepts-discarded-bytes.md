# The stalled-input test accepts discarded input

Reviewed commit: d5a7f8de5ea949628ce146dcd7944852baa40db1

## TLDR

A terminal-host regression can pass when blocked typing is discarded instead of remaining pending. This is a gap in the
test, not an established product input-loss defect.

## Details

`crates/terminal-host/tests/host_lifecycle.rs:282–285` checks only that `InputAck.written` is less than the total. It
ignores `discarded`. The test at `305–358` therefore accepts all input being discarded while the subsequent control
operations succeed against an emptied queue. It also does not establish that the child remains alive through that
boundary.

Reject discarded input and establish both child liveness and unresolved input before using responsiveness as evidence
that controls work beside blocked input. The full-delivery unit test exercises a reading child, not this stalled-input
boundary.

Suggested bucket: `other`. Confidence: confirmed assertion gap; no missing source premise. No runtime schedule or
production regression was demonstrated. No exact coverage or applicable filter was found.

Provenance: g038/f1; Codex gpt-6.1-sol, high reasoning. Source tests and contracts were inspected without execution.
