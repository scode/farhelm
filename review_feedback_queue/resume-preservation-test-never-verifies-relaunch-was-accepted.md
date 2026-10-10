# Resume-preservation test never verifies that relaunch was accepted

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The resume-preservation test can pass when relaunch is refused.

## Details

F167 — **definite** — `crates/farhelm-supervisor/src/store.rs:7192` — Resume-preservation test never verifies that
relaunch was accepted

The test checks retained conversation capture and provenance without requiring the valid relaunch request to be
accepted. A refusal leaves exactly that data intact, and the later stale request can still receive its expected refusal.
Require the existing Claimed outcome and a generation advance before checking preservation, so refusal cannot masquerade
as a successful restart.

## Evidence and triage context

- crates/farhelm-supervisor/src/store.rs:7177–7195: the test discards the successful Result's RelaunchDecision.
- crates/farhelm-supervisor/src/store.rs:7201–7226: it checks preserved capture/provenance and the later stale claim
  only.
- crates/farhelm-supervisor/src/store.rs:4057–4060: OfferChanged is an Ok refusal before mutation.
- crates/farhelm-supervisor/src/store.rs:4073–4099: accepted relaunch advances generation and returns Claimed.
- crates/farhelm-supervisor/src/service/core.rs:10420–10429: the real caller treats OfferChanged as a conflict with no
  relaunch.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:1873–1875 requires preservation through relaunch and stale-provenance refusal; it does not accept refusal
  of the valid relaunch.

Caveats:

- No current production refusal was demonstrated.
- No mutation test was run.
- This is a concrete false-positive oracle, not a request for additional generic assertions.
- No current production refusal regression was demonstrated.
- No runtime reproductions were performed for this adjudication.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_08:p1:F2`.

- `gap_supervisor_state_cor_08:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
