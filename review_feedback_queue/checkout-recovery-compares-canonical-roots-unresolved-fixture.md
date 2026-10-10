# Checkout recovery compares canonical roots with unresolved fixture paths

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Checkout recovery tests reject correctly resolved directory roots.

## Details

F137 — **definite** — `crates/farhelm/tests/e2e/github_checkouts.rs:918` — Checkout recovery compares canonical roots
with unresolved fixture paths

The recovery preview returns canonical directory roots, but both expected roots retain the fixture's unresolved path
spelling. A symlink component therefore makes correct output fail before the recovery scenario completes. Compute
canonical expected values for both roots so the assertions test recovery behavior rather than incidental
temporary-directory spelling.

## Evidence and triage context

- github_checkouts.rs:225–226 obtains unresolved fixture roots. :918–921 compares canonical_root with root_a.path(), and
  :1014–1017 independently compares it with root_b.path(). service/core.rs:7387–7410 canonicalizes the root before
  returning its text.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2249–2251 explicitly requires root expansion/canonicalization. No exact existing coverage found.

Caveats:

- No native macOS execution. Both assertion sites are retained within this single input finding; neither is silently
  omitted.
- No native macOS execution. Both independent assertion sites need preservation in the finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p1:F6`.

- `cli_installation_05_cor:p1:F6`: confidence as filed: definite; suggested bucket as filed: other.
