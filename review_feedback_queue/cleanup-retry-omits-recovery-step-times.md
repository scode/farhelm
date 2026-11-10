# Cleanup Retry omits recovery-step timing

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

After a cleanup failure, Retry can report zero time for recovery steps that actually ran, weakening its diagnostic
timing report.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `ed9bdff39748ff551ca08cc02ff6c0f129677f50`. Original source location:
`crates/farhelm-supervisor/src/checkout_trash.rs:659-664`. Locations below use that revision unless another is named;
the assessment was checked through the reviewed tip.

Retry performs recovery work without adding that work to the per-step counters used in its timing report. A prior
cleanup refusal, including a permission failure, supplies the retry state; this does not require a crash or an already
successful delete. Ordinary initial cleanup accounting does not repair the retry path.

Trace the recovery open/check steps and the final timing report together, and charge those steps to the same counters
used by ordinary cleanup. Source inspection confirms the omission; neither measured duration nor a runtime diagnostic
sample is claimed. The diagnostic filter does not clearly cover the entire trigger because permission refusal is not
inherently a narrow timing event. Retain other, with confirmed source confidence; g001-S4 corroborates the counter
boundary.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g004/g004-001`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
