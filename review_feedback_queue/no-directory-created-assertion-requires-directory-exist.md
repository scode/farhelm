# “No directory created” assertion requires the directory to exist

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The no-directory-creation test requires the directory to have been created.

## Details

F205 — **definite** — `crates/farhelm-supervisor/src/working_copies.rs:4009` — “No directory created” assertion requires
the directory to exist

Its oracle reads the archive directory and accepts an empty result. An absent directory fails that read, so the test
passes when recovery creates an empty directory and cannot pass when it remains absent. Settle the intended invariant:
assert absence and defer creation if creation is forbidden, or explicitly document and test the narrower empty-directory
contract.

## Evidence and triage context

- crates/farhelm-supervisor/src/working_copies.rs:3993–4003 specifies that nothing is created and removes the source
  before reconciliation.
- crates/farhelm-supervisor/src/working_copies.rs:4009–4012 compares entries(archive_root) with an empty vector while
  claiming that reconciliation invented no directory.
- crates/farhelm-supervisor/src/working_copies.rs:2102–2108 implements entries through read_dir(...).expect(...), so an
  absent archive directory fails the assertion setup.
- crates/farhelm-supervisor/src/working_copies.rs:1787 calls ensure_archive_root before source/destination absence is
  resolved; :1525–1531 creates and syncs the directory, and :1920 subsequently returns SourceMissing.
- crates/farhelm-supervisor/src/store.rs:3250–3265 and service/core.rs:5496–5520 show that this reconciliation path is
  also used for startup recovery.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- SPEC.md:700–720 and SPEC_impl.md:1602–1615 do not explicitly prohibit creating an empty archive directory in this
  case.
- The definite defect is the contradictory test oracle; whether production should defer directory creation requires
  settling the intended contract.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_12:p1:F1`.

- `gap_supervisor_state_sec_12:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
