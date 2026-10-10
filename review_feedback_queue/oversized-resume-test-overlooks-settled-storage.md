# Oversized-resume test overlooks settled storage

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The oversized-resume test can accept prohibited settled storage.

## Details

F162 — **definite** — `crates/farhelm-supervisor/src/service/handlers.rs:7802` — Oversized-resume test overlooks settled
storage

A rejected oversized resume request can still create a permanent Failed reservation without changing the test's queried
sets. At least the final refusal can therefore consume its key and retain oversized input while all assertions pass. Use
distinct keys and check that each individual reservation is absent after refusal at this separate test site.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/handlers.rs:7732–7740: the test promises refusal before oversized permanent
  reservation writes.
- crates/farhelm-supervisor/src/service/handlers.rs:7751–7796: both refusals reuse key and check error kind/message.
- crates/farhelm-supervisor/src/service/handlers.rs:7798–7806: the storage oracle again checks only sessions and pending
  reservations.
- crates/farhelm-supervisor/src/store.rs:4294–4297: settled reservations are excluded.
- crates/farhelm-supervisor/src/service/handlers.rs:375–406: current size checks return before durable create work.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No current oversized production write is established.
- The shared query defect does not make this the same editable assertion site as F2.
- No current production oversized write is established.
- No mutation test was run.
- Do not merge this with the invalid-key assertion site.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_06:p1:F3`.

- `gap_supervisor_state_cor_06:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
