# Checkout discovery strips valid trailing path characters

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The test recorder identifies the wrong checkout when its name ends in whitespace.

## Details

F334 — **definite** — `scripts/record-test-run.py:641` — Checkout discovery strips valid trailing path characters

Generic text trimming removes legal trailing whitespace from Git's reported root path. Source probes and the
evidence-storage fence then use the trimmed path; if its sibling is another checkout, metadata can fully identify that
other tree while execution stays in the original. Remove only Git's terminating newline and validate the discovered root
against the invoking directory before using it.

## Evidence and triage context

- scripts/record-test-run.py:624–642 handles probe output and checkout discovery.
- scripts/record-test-run.py:994–1006 runs source probes in the discovered root; :1626–1630 skips fallback when that
  value is non-null.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None found by the originating reviewer.

Caveats:

- Requires a legal checkout name ending in whitespace; a complete wrong-tree fingerprint also requires a trimmed sibling
  checkout. No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_15_cor:p2:F1`.

- `test_infrastructure_15_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
