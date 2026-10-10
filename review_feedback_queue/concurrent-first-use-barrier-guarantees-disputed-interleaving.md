# Concurrent-first-use barrier guarantees the disputed interleaving

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The first-use test could pass without exercising concurrent absent observations.

## Details

F283 — **possible** — `crates/farhelm-helm/src/provisioning.rs:10949` — Concurrent-first-use barrier guarantees the
disputed interleaving

Its only barrier is before directory lookup. One task can create the directory before the other checks existence, so
both succeed even if the AlreadyExists race-handling branch is broken. Final directory checks do not distinguish that
schedule. Control both absent observations before creation, then require the loser to encounter the conflicting creation
and succeed through the intended handling.

## Evidence and triage context

- provisioning.rs:10912–10920 identifies the absent-observation/AlreadyExists regression; :10960 places the only barrier
  before path(), and :10968–10985 checks only success and final directory properties. payloads.rs:232 invokes
  ensure_private_extracted_dir, whose :436–448 contains the separate observation/create/AlreadyExists boundary. No
  synchronization forces both absent observations. Under the user's explicit oracle rule this is a concrete
  test-correctness defect; retain separately from a production defect.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_06_cor:p1:C5`.

- `helm_state_provisioning_06_cor:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket
  as filed: not separately tagged in candidate list.
