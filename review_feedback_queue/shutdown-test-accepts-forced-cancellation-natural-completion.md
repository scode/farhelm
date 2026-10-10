# Shutdown test accepts forced cancellation as natural completion

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The shutdown test accepts cancellation of a writer that never finishes.

## Details

F146 — **definite** — `crates/farhelm-supervisor/src/service/connection.rs:2484` — Shutdown test accepts forced
cancellation as natural completion

A writer can deliver the queued frame and then remain stuck or panic. The helper cancels it, while the test treats frame
delivery and cleanup as sufficient success. This masks the shutdown behavior the regression is intended to protect.
Await the writer directly under the intended short timeout and require successful completion; reserve abort-and-await
for failure cleanup.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/connection.rs:2440 promises natural completion despite retained sender clones.
- crates/farhelm-supervisor/src/service/connection.rs:2484 calls drain_writer rather than requiring successful task
  completion.
- crates/farhelm-supervisor/src/service/connection.rs:764 accepts any completed join result; :778 aborts a stalled
  writer and returns normally.
- crates/farhelm-supervisor/src/service/connection.rs:841 closes receivers; removing a close leaves a retained sender
  able to keep the writer pending.
- crates/farhelm-supervisor/src/service/connection.rs:2440–2445 explicitly promises natural completion before the drain
  window.
- crates/farhelm-supervisor/src/service/connection.rs:2454–2455 retains both sender handles. Lines 2475–2485 signal
  closure, assert receipt of one frame, call drain_writer with a one-millisecond window, and assert only that one frame
  completed.
- crates/farhelm-supervisor/src/service/connection.rs:763–781 returns normally after a no-progress timeout, aborting and
  awaiting the writer without exposing cancellation to its caller.
- crates/farhelm-supervisor/src/service/connection.rs:834–848 currently closes both receivers on the shutdown signal.
  Removing that transition while continuing ordinary receives lets the queued frame arrive but leaves the writer
  waiting; the test's drain helper then cancels it successfully.
- crates/farhelm-supervisor/src/service/connection.rs:715–728 is the production caller: it signals writer closure after
  handlers finish, then uses drain_writer as a fallback. Reusing that fallback in the test hides whether the signal
  itself works.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact existing coverage. shutdown-expiry.md concerns a different production teardown acknowledgement.
- review_feedback_queue/shutdown-expiry.md:14–24 concerns supervisor process exit before tmux output clients reach their
  safe shutdown boundary. This finding concerns a protocol-writer test accepting cancellation; its trigger, consequence,
  and independently editable site differ.
- BUGS.md:8–44 concerns abrupt supervisor death crashing tmux, not this test oracle.
- TODO.md:33–43 plans moving session creation off the connection reader; it does not cover writer-test completion.

Caveats:

- Inspection only; no mutation test was run.
- This establishes a defective test oracle, not a broken production shutdown path.
- review_feedback_queue/shutdown-expiry.md:14-24 concerns supervisor exit before tmux output-client safety
  acknowledgement, a different trigger and consequence.
- Test defect only; production receiver closure is present.
- No current production writer-shutdown failure was established.
- No runtime or mutation test was performed.
- The existing progress-drain test has a separate natural-completion sentinel; that does not repair this test's oracle.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_cor_01:p1:F1`,
`gap_supervisor_runtime_sec_01:p1:F1`.

- `gap_supervisor_runtime_cor_01:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_runtime_sec_01:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
