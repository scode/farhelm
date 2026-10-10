# Recreation test assumes distinct birth timestamps

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Immediate directory recreation could preserve the identity the test expects to differ.

## Details

F288 — **possible** — `crates/farhelm-supervisor/src/working_copies.rs:3374` — Recreation test assumes distinct birth
timestamps

The fixture requires birth-time support but does not prove that recreation changed either the reused inode or a coarse
birth timestamp before demanding DifferentObject. If both match, the accepted comparison correctly reports the same
identity and the test falsely fails. No equal-timestamp failure was reproduced. Establish a distinguishable observed
identity before that assertion, or explicitly handle the equal-timestamp case without misreporting a production defect.

## Evidence and triage context

- crates/farhelm-supervisor/src/working_copies.rs:3350–3384: the test requires birth-time support, immediately
  removes/recreates the directory, observes inode reuse, and unconditionally expects DifferentObject.
- crates/farhelm-supervisor/src/working_copies.rs:558–564: equal inode and recorded birth time satisfy same_directory.
- FLAKES.md:127–130: the historical record explicitly identifies same-coarse-clock-tick residual exposure for this test.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": ["TRIAGE_OUTCOMES.md:1574–1608 covers production identity checks and fallback, not
  acceptance of an unverified test premise.", "review_feedback_queue/snapshot-root.md:20–24 concerns replacing a mounted
  root with a snapshot, not immediate directory recreation.", "review_feedback_queue/birth-oracle.md addresses the
  skip/probe failure branch, not equal observed birth timestamps."]}

Caveats:

- No equal-timestamp failure was reproduced.
- Requires actual reuse of both inode and birth timestamp.
- Independent editable test from working_copies.rs:3983.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_09:p1:C2`.

- `gap_supervisor_state_cor_09:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
