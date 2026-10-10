# PID-only anchoring can admit an earlier launch’s durable report

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A retained report could be mistaken for a later launch after process-number reuse.

## Details

F80 — **possible** — `crates/farhelm-supervisor/src/procs.rs:487`; `crates/farhelm-supervisor/src/procs.rs:483` —
PID-only anchoring can admit an earlier launch’s durable report

A durable report can be assigned the current launch generation when its recorded ancestor number matches a later pane
and no usable current start token distinguishes them. If the report survives relaunch long enough for number reuse, it
could replace the conversation binding and make Restart resume earlier work. Normal draining and subsequent reports
narrow the opportunity; no full sequence was reproduced. Retain a launch discriminator comparable after pane exit and
refuse live-process read failures rather than weakening identity.

## Evidence and triage context

- crates/farhelm-supervisor/src/procs.rs:483–486 matches PID without a start-token comparison when pane.start is None.
- crates/farhelm-supervisor/src/service/core.rs:14498–14504 supplies None for dead panes and also for unsuccessful
  live-process reads.
- crates/farhelm-supervisor/src/hook_report.rs:142–160 records ancestry but no originating launch generation.
- crates/farhelm-supervisor/src/service/report_files.rs:311–323 anchors first, then supplies the current row generation.
- crates/farhelm-supervisor/src/service/report_files.rs:243–250,517–522 retains transiently failed reports;
  service/core.rs:10049 continues after the drain.
- crates/farhelm-supervisor/src/procs.rs:483–486 accepts a matching PID when the current anchor lacks a start token.
- crates/farhelm-supervisor/src/service/core.rs:14498–14504 constructs such an anchor.
- crates/farhelm-supervisor/src/hook_report.rs:142–160 has no originating generation field.
- crates/farhelm-supervisor/src/service/report_files.rs:311–323 attaches the current generation only after anchoring.
- crates/farhelm-supervisor/src/service/report_files.rs:517–522 restores retrying reports, while service/core.rs:10049
  proceeds after the void-returning capture pass.
- Independent decisions are sr_data:p1:F1 KEEP, sr_data:p1:F2 KEEP, and sr_data:p1:F3 DROP as covered by
  review_feedback_queue/omp-bun-pane-proof.md:9–50. No input formal entry was merged or removed.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:1842–1845 specifies PID-only anchoring after exit but also claims that anchoring ties reports to their
  launches.
- SPEC_impl.md:1973–1974 says earlier-launch reports fail anchoring and are discarded.
- SPEC_impl.md:2194–2205 and TRIAGE_OUTCOMES.md:6505–6533 accept bounded reuse windows, expressly excluding bare numbers
  stored for later.
- SPEC_impl.md:1842–1845 prescribes PID-only dead-pane anchoring but asserts that it ties reports to launches.
- SPEC_impl.md:1973–1974 promises earlier-launch reports are discarded.
- TRIAGE_OUTCOMES.md:6523–6528 excludes process numbers stored for later from the accepted bounded-reuse policy.

Caveats:

- Requires retention across relaunch, PID reuse, and admission without a usable current start token.
- Normal Restart draining and subsequent reports substantially narrow the opportunity.
- No end-to-end reproduction establishes the complete sequence.
- No reproduction establishes a retained report surviving long enough for the required PID reuse.
- Normal draining, replacement reports, and live start-token matching prevent many candidate sequences.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_data:p1:F2`, `sr_lifecycle:p1:F2`, `sr_data:p1:C13`.

- `sr_data:p1:F2`: confidence as filed: Possible; suggested bucket as filed: highest.
- `sr_lifecycle:p1:F2`: confidence as filed: Possible; suggested bucket as filed: highest.
- `sr_data:p1:C13`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
