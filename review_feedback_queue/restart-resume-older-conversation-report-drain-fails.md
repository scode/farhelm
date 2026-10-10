# Restart can resume an older conversation after its report drain fails transiently

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Restart could choose an older conversation while a replacement report remains unsettled.

## Details

F81 — **possible** — `crates/farhelm-supervisor/src/service/report_files.rs:151` — Restart can resume an older
conversation after its report drain fails transiently

A transient admission failure restores the pending conversation report for retry but does not tell Restart that the
session's drain remains unsettled. If later lifecycle checks succeed before that report is applied, Restart can stop the
current conversation and resume the previously stored one. A concurrent successful drain can prevent this; the sequence
was not reproduced. Propagate per-session unsettled status and refuse Restart before target selection while its pending
report remains unjudged.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:14479–14488 maps a failed pane-process query to Internal.
- crates/farhelm-supervisor/src/service/report_files.rs:243–250 classifies Internal as Retry; lines 517–522 restore the
  report and return.
- crates/farhelm-supervisor/src/service/report_files.rs:126–156 returns no per-session settlement status.
- crates/farhelm-supervisor/src/service/capture.rs:108–117 proceeds to refresh without a drain result.
- crates/farhelm-supervisor/src/service/core.rs:10049,10071–10074,10145–10150 drains, reads the durable snapshot, and
  constructs/verifies its existing Resume target.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- service/core.rs:13945–13953 describes a failed replacement store write, not a transient pane-query failure with a
  valid replacement report already on disk.
- TRIAGE_OUTCOMES.md:5835–5868 addresses reports rejected after capture-claim contention, a different mechanism.
- SPEC_impl.md:1971–1979 requires retry restoration and explains why Restart waits for pending reports; it does not
  explicitly permit Restart to proceed after such a retry.

Caveats:

- Requires an earlier valid capture, a pending replacement report, and an admission failure that clears before later
  Restart operations.
- A concurrent successful drain can prevent the outcome.
- No runtime reproduction or transcript deletion is established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_data:p2:F1`.

- `sr_data:p2:F1`: confidence as filed: Likely, conditional by inspection; suggested bucket as filed: highest.
