# Commit-race test can exercise only the pre-commit abort path

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The upload-abort test could finish before commit is actually in flight.

## Details

F281 — **possible** — `crates/farhelm-helm/src/uploads.rs:1534` — Commit-race test can exercise only the pre-commit
abort path

The fixture injects abort without first establishing that the peer received CommitUpload. An earlier pre-commit check
can produce the accepted error and sentinel without exercising cancellation of an unanswered commit. That false-pass
schedule on the current-thread runtime was not demonstrated. Require and assert CommitUpload before aborting, withhold
its reply, and retain bounded response and sentinel checks.

## Evidence and triage context

- crates/farhelm-helm/src/uploads.rs:1532: the peer receives one data frame, checks its length, and immediately sends
  UploadAborted at line 1534; it never receives CommitUpload.
- crates/farhelm-helm/src/rest_harness.rs:1427: recv_frame reads only the next frame; neither this helper nor send at
  line 1442 establishes commit admission.
- crates/farhelm-helm/src/uploads.rs:338: the real HTTP relay sends its pending tail, then calls upload.commit at
  line 344.
- crates/farhelm-helm/src/client.rs:1909: the demultiplexer records UploadAborted in the upload's retained state.
- crates/farhelm-helm/src/client.rs:3649: commit returns the retained abort reason before sending CommitUpload; lines
  3672–3683 separately watch for an abort during the exchange.
- crates/farhelm-helm/src/uploads.rs:1555: the test bounds response completion and checks the sentinel at line 1566, but
  does not distinguish those two paths.
- crates/farhelm-helm/src/client.rs:7248: the neighboring before-commit test explicitly waits for the abort before
  calling commit, demonstrating the competing path's intended contract.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"basis": "review_feedback_queue/upload-cancellation-drops-final-reply.md:22", "comparison": "This covers supervisor
  cancellation discarding required messages under backpressure, leaving a healthy caller waiting. The present finding
  concerns a helm test receiving an abort without proving commit admission. Trigger, mechanism, and editable site
  differ."}
- {"basis": "TODO.md:446, Hand-rolled fake supervisors in tests", "comparison": "This concerns migrating duplicated
  peers and two window-test reads, not establishing this commit-race premise. It is also outside Planned."}

Caveats:

- No runtime or mutation test was run.
- The runtime defaults to current-thread execution; an early-abort false pass requires a scheduling opportunity before
  commit's initial check. Its occurrence in this exact fixture was not established.
- Current production code contains the in-flight abort watcher.
- SPEC.md:1303–1316 permits uncertain publication after cancellation and requires visible failures; it does not accept
  this test-proof gap. TODO.md:33–43, BUGS.md, FILTER.md, and targeted ledger searches supplied no matching coverage.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_lifecycle:p3:F1`.

- `hc_lifecycle:p3:F1`: confidence as filed: Possible; missing commit observation confirmed by inspection, false pass
  scheduling-dependent; suggested bucket as filed: other.
