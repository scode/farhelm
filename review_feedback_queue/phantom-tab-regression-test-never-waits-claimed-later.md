# The phantom-tab regression test never waits for its claimed later reconciliation

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The phantom-tab test checks absence before witnessing later reconciliation.

## Details

F249 — **definite** — `e2e/tests/terminal-tabs.spec.ts:2079` — The phantom-tab regression test never waits for its
claimed later reconciliation

Repeating an already-satisfied tab-count assertion does not establish that a post-close detail response has been
applied. A separate API request neither drives nor observes that browser reconciliation, and the never-observed-open
premise is also uncontrolled. Control opening and closing detail responses, then witness a post-close response being
applied before asserting the tab stays absent.

## Evidence and triage context

- e2e/tests/terminal-tabs.spec.ts:2073–2076 opens, closes and observes zero tabs.
- e2e/tests/terminal-tabs.spec.ts:2079 immediately repeats the same condition with a larger timeout; :2080–2081 checks
  server state through the request fixture.
- crates/farhelm-ui/src/session_view.rs:899–903 retires optimistic corrections when a detail response is applied.
- crates/farhelm-ui/src/session_view.rs:1600–1609 currently retires a locally closed optimistic tab.
- terminal-tabs.spec.ts:2051-2059 names reappearance after reconciliation as the regression, but :2079 can return
  immediately after :2076. session_view.rs:1600-1609 suppresses a closed tab and retires its optimistic entry; :899-902
  later prunes corrections. A stale optimistic entry can remain invisible until that later update. The separate API GET
  at :2080 does not prove the browser applied a later update.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No current phantom-tab product defect is established.
- SPEC_impl.md:1288–1303 describes reconciliation hints and the polling backstop; it does not accept this missing test
  boundary.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_14_cor:p1:F2`,
`test_infrastructure_14_sec:p1:C3`.

- `test_infrastructure_14_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_14_sec:p1:C3`: confidence as filed: definite; suggested bucket as filed: not separately tagged in
  candidate list.
