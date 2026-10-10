# The checkout-validation test cannot construct its fixture on APFS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

APFS rejects the validation fixture before its intended checks run.

## Details

F201 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:29230` — The checkout-validation test cannot
construct its fixture on APFS

The test tries to create a non-UTF-8 directory name that ordinary APFS cannot represent, panicking during fixture setup.
Neither that refusal case nor the later portable checkout-validation cases are exercised. Separate or gate the
invalid-name filesystem scenario while retaining the portable checks on macOS, so an unavailable fixture shape does not
block unrelated validation.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:29219 has no macOS gate; line 29230 unwraps creation of a name
  containing byte 0xff.
- crates/farhelm-supervisor/src/service/core.rs:29234 places the intended refusal and portable validation assertions
  after that creation.
- crates/farhelm-supervisor/src/service/core.rs:27076 documents APFS refusal and gates the analogous fixture; line 28723
  gates another invalid-name filesystem fixture.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The non-UTF-8 product-path refusal decision does not authorize a supported-platform test setup failure. No matching
  fixture item found.

Caveats:

- No macOS execution.
- The failure depends on the fixture residing on APFS or another filesystem that rejects the name.
- Conditional on APFS or another filesystem rejecting invalid UTF-8 names.
- Separate or gate only the unconstructible fixture portion so portable assertions remain active.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_05:p1:F1`.

- `gap_supervisor_state_sec_05:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
