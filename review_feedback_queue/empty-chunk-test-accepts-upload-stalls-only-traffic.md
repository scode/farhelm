# The empty-chunk test accepts an upload that stalls only after traffic stops

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The empty-chunk upload test can pass the wrong timeout behavior.

## Details

F129 — **definite** — `crates/farhelm/tests/e2e/attachment_uploads.rs:2291` — The empty-chunk test accepts an upload
that stalls only after traffic stops

The regression sends a finite stream of empty chunks and observes abort afterward. An implementation that incorrectly
counts those chunks as progress can postpone abort until the stream stops and still satisfy the assertions. The test
therefore cannot enforce its stated progress-accounting contract. Observe abort concurrently with sending and require it
while empty traffic is still being supplied.

## Evidence and triage context

- attachment_uploads.rs:2274–2276 selects a 300 ms progress window; :2285–2291 completes sixty empty sends paced at 50
  ms before reading the outcome at :2293. :2297 asserts only the stalled reason. next_outcome at :218–223 discards
  acknowledgements. uploads.rs:605–610 currently rearms only for nonempty chunks.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:99–112 concerns a helm-side empty-body busy loop, not this supervisor test oracle.
  .agents/test-authoring.md:16–17 requires a distinguishing observable. No exact Planned, BUGS.md, queue, ledger, or
  filter coverage found.

Caveats:

- No mutation test was run. This establishes a test defect, not a shipped upload defect.
- No mutation test run. This is a test proof defect, not evidence of a shipped upload defect.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_04_cor:p1:F1`.

- `cli_installation_04_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
