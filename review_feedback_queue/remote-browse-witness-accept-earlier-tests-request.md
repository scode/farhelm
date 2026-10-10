# The remote-browse witness can accept an earlier test’s request

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A remote-routing witness can reuse a previous test's receipt.

## Details

F247 — **definite** — `e2e/tests/sidebar.spec.ts:5114–5120` — The remote-browse witness can accept an earlier test’s
request

The witness searches historical receipt bytes for a reused directory path. An earlier successful remote browse can
therefore satisfy a later execution whose action was incorrectly handled locally. Checking the browser's outgoing host
field does not prove the helm forwarded it correctly. Require a receipt appended after an action-specific baseline,
handling truncation, or use unique directories and receipts per execution.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:5097 reuses remote_state_dir; :5110–5112 checks the outgoing request; :5117–5118 searches
  the entire log without an offset or unique receipt.
- e2e/start-stack.sh:362–364 starts the remote supervisor with one log file; :381–391 registers its stable
  state-directory path.
- e2e/playwright.config.ts:85–100 shares one stack serially; :126–140 runs ordinary specs across both engines before
  remote teardown.
- crates/farhelm-supervisor/src/service/handlers.rs:2399–2406 emits the receipt when that supervisor receives
  BrowseDirectory.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- A concrete test-oracle defect, not a demonstrated product routing failure.
- The stale-witness trigger requires an earlier matching receipt.
- SPEC.md:453–461 requires browsing the target host; no matching Planned, BUGS, queue or ledger disposition was found.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_10_cor:p2:F1`.

- `test_infrastructure_10_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
