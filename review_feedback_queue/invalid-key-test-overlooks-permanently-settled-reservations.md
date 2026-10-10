# Invalid-key test overlooks permanently settled reservations

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Invalid-key tests miss permanently stored failed reservations.

## Details

F161 — **definite** — `crates/farhelm-supervisor/src/service/handlers.rs:7682` — Invalid-key test overlooks permanently
settled reservations

The assertions inspect sets that exclude settled Failed reservations. An implementation can persist every rejected key
as Failed while both sets remain empty, consuming those keys and retaining prohibited input. Keep each submitted key and
assert directly that its reservation is absent immediately after rejection, so the no-storage contract is actually
checked.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/handlers.rs:7604–7613: the contract forbids reaching permanent reservation
  storage.
- crates/farhelm-supervisor/src/service/handlers.rs:7632–7687: final storage checks cover session rows and pending
  reservations only.
- crates/farhelm-supervisor/src/store.rs:4281–4297: pending_reservations excludes every settled reservation.
- crates/farhelm-supervisor/src/service/handlers.rs:326–355: current invalid-key rejection precedes admission.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Current production ordering is correct.
- This finding concerns the test oracle, not an established invalid-key write.
- Current production rejection ordering is correct.
- This is a test defect, not a demonstrated invalid-key write.
- Separate assertion site from the oversized-resume test.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_06:p1:F2`.

- `gap_supervisor_state_cor_06:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
