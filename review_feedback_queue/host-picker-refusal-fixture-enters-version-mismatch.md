# Host-picker refusal fixture unintentionally triggers version mismatch

Reviewed commit: 164101bdffff7d0e7dfef79652a6abe9f22014f7

## TLDR

The new browser test checks recovery from a failed launch with ordinary background refresh disabled, weakening its
evidence for normal launcher behavior.

## Details

Suggested bucket: `other`. Confidence: confirmed by static tracing; not reproduced. Source: dedicated `gpt-6.1-sol` high
adversarial review. Proposed drop: none.

e2e/tests/host-picker.spec.ts:187–191 fabricates a 500 response without x-farhelm-build. Releasing the held response
therefore latches Skew::Silent through api.rs:1249 and skew.rs:183–185,207–220. The recovery assertions at
host-picker.spec.ts:208–210 subsequently run under version mismatch, which withdraws the feed and unattended behavior
rather than representing an ordinary same-build refusal. SPEC_impl.md:3642–3645 requires every helm reply to carry its
stamp. Use fulfillAsHelm from helpers/terminal-suite.ts:113–127 and assert that no mismatch notice appears. Scope:
test-fixture correctness; no confirmed shipped-product regression.

Possible cover: TRIAGE_OUTCOMES.md:10291–10307, heading yolo-refusal-fixture-unintentionally-puts-page-into.md, records
the same defect in yolo-guard.spec.ts. Its fixture scope does not cover this newly added host-picker test. No exact
queue, Planned, BUGS, or filter coverage found.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: host-picker.spec.ts:187–191 and
skew.rs:183–220 retain the relevant behavior. The test file has no subsequent modifying commit; no fixing commit found.

Review coverage: full commit diff read: true. No commit diff hunk unread. Unchanged surrounding modules and test bodies
outside the listed ranges were not exhaustively read. The full 13,089-line triage ledger was not read linearly; relevant
sections were selected through searches. No browser geometry or assistive-technology behavior was executed. Relevant
truncated outputs were reopened before reliance.

Runtime evidence: None, as requested by default. Read-only static review; no checkout files created, modified, or
deleted.
