# foreign-destination fixture can reuse the identity it expects to reject

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The foreign-directory test could accidentally recreate the original identity.

## Details

F169 — **possible** — `crates/farhelm-supervisor/src/working_copies.rs:3983` — foreign-destination fixture can reuse the
identity it expects to reject

The fixture removes the original checkout before creating its supposed foreign replacement. On a filesystem without
birth times, reuse of the original inode can satisfy the accepted identity fallback, making recovery correctly accept
the directory that the test demands it reject. Reuse was not reproduced. Create and establish the distinct foreign
identity while the original still exists, then remove the original and test recovery.

## Evidence and triage context

- crates/farhelm-supervisor/src/working_copies.rs:3976–3989: the test removes the original directory before constructing
  the purported stranger and unconditionally expects IdentityMismatch.
- crates/farhelm-supervisor/src/working_copies.rs:558–564: identity falls back to device/inode when no birth time was
  recorded.
- crates/farhelm-supervisor/src/working_copies.rs:1813–1820: a matching destination yields MetadataComplete.
- crates/farhelm-supervisor/src/working_copies.rs:3959–3965: the neighboring test establishes a distinct replacement
  while the original still exists.
- FLAKES.md:107–130: historical failures established equal source/destination identity and explicitly preserve
  no-birth-time exposure.
- crates/farhelm-supervisor/src/working_copies.rs:3979–3985 records an allocated identity, removes that directory, then
  creates the journaled destination.
- crates/farhelm-supervisor/src/working_copies.rs:3987–3989 unconditionally requires IdentityMismatch.
- crates/farhelm-supervisor/src/working_copies.rs:558–564 compares device/inode when no birth time was recorded; equal
  birth times also fail to distinguish an inode reuse.
- crates/farhelm-supervisor/src/working_copies.rs:1813–1820 accepts a matching destination as MetadataComplete before
  checking source absence.
- crates/farhelm-supervisor/src/working_copies.rs:3962–3965 demonstrates the adjacent fixture's safer
  coexistence-before-removal construction.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": ["TRIAGE_OUTCOMES.md:1574–1608 accepts the production fallback, not a test premise that
  successive directories must have different fallback identities.", "FLAKES.md:107–134 records history and residual
  exposure but does not dispose of this current fixture issue.", "review_feedback_queue/birth-oracle.md:14–27 concerns
  an external capability probe; this destination test does not call that probe."]}
- FLAKES.md:107–130 names this exact test and records historical matching-identity failures plus the remaining
  no-birth-time/coarse-clock exposure. It is supporting evidence, not a listed suppression basis.
- TRIAGE_OUTCOMES.md:1574–1608 accepts production identity fallback for historical records. Its trigger and scope do not
  settle whether this test must establish a distinct-object premise.

Caveats:

- No current failure was reproduced.
- The open premise is inode reuse for this destination on a filesystem without birth times.
- This finding concerns test reliability, not a new production work-loss claim.
- Requires missing recorded birth time and actual inode reuse.
- No new production work-loss claim is established.
- No current-substrate failure was reproduced.
- The unresolved premise is inode reuse without a distinguishing birth time.
- This finding concerns false test failures, not newly established loss of user work.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_09:p1:F1`,
`gap_supervisor_state_sec_12:p1:F2`.

- `gap_supervisor_state_cor_09:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.
- `gap_supervisor_state_sec_12:p1:F2`: confidence as filed: possible; suggested bucket as filed: other.
