# The link-drag fixture can mistake a wrapped URL for a single-row URL

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The link-drag test could drag outside the link while claiming a single-row fixture.

## Details

F240 — **possible** — `e2e/tests/terminal-links.spec.ts:847–852` — The link-drag fixture can mistake a wrapped URL for a
single-row URL

Its tail search is not unique, and the drag endpoint is not bounded by measured terminal columns. A wrapped URL can
therefore pass the premise check while the gesture tests nonactivation outside the link rather than selection
suppression within it. Browser geometry and the passing counterexample remain unverified. Use measured columns, a unique
tail, and explicit checks that the complete URL and endpoints fit the row.

## Evidence and triage context

- e2e/tests/terminal-links.spec.ts:801–803 constructs a URL ending in 60 identical c characters.
- e2e/tests/terminal-links.spec.ts:139–148 returns the first viewport row containing a substring.
- e2e/tests/terminal-links.spec.ts:847–852 searches for 15 identical c characters and equates that row with the head
  row.
- e2e/tests/terminal-links.spec.ts:858 computes the endpoint from the full unwrapped string length.
- e2e/tests/terminal-links.spec.ts:304–315 converts that endpoint directly to pixels without checking it against
  geometry.cols.
- e2e/tests/terminal-links.spec.ts:868–889 checks a substantial URL substring and later positive click, but never
  validates that the earlier release point lies inside the same link.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:2359–2381 concerns OSC 8 hover-target disclosure, not the plain-link drag fixture's geometry or
  oracle.

Caveats:

- Actual configured-engine column counts and pointer outcomes were not measured.
- The faulty premise check is definite; a complete passing counterexample remains unverified.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_12_cor:p1:F3`.

- `test_infrastructure_12_cor:p1:F3`: confidence as filed: possible; suggested bucket as filed: other.
