# The tilde-replay test equates display and canonical paths

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The tilde-replay test rejects correct canonical paths.

## Details

F136 — **definite** — `crates/farhelm/tests/e2e/create_idempotency.rs:830` — The tilde-replay test equates display and
canonical paths

The test compares the displayed workspace spelling directly with the canonical path returned for replay. When the
temporary root contains a symlink, as ordinary macOS temporary paths do, those spellings legitimately differ and
validation stops before checking replay. Canonicalize the expected home/workspace path for the canonical-directory
assertion while keeping the separate display-path expectation unchanged.

## Evidence and triage context

- create_idempotency.rs:802–804 derives accepted from the unresolved fixture path and :828–832 uses it for both display
  and canonical expectations. farhelm-teststate/src/lib.rs:65,640–653,673–681 retains the /tmp spelling.
  service/core.rs:7303–7312 canonicalizes existing destinations.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2301–2303 distinguishes accepted canonical destination from submitted display spelling. No matching
  Planned, BUGS.md, queue, ledger, or filter coverage found.

Caveats:

- No native macOS execution. The trigger is a noncanonical fixture-root spelling, not every possible macOS filesystem
  layout.
- No native macOS execution. The trigger is a noncanonical fixture-root spelling, not every conceivable macOS layout.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p1:F5`.

- `cli_installation_05_cor:p1:F5`: confidence as filed: definite; suggested bucket as filed: other.
