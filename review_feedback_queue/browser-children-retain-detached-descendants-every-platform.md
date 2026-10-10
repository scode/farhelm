# Browser children retain detached descendants on every platform

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Non-Linux supervision could leave detached browser descendants alive.

## Details

F315 — **possible** — `e2e/harness-tests/supervise-child.py:154-157` — Browser children retain detached descendants on
every platform

Process-group cleanup runs everywhere, but detached-descendant cleanup depends on Linux subreaping. The all-platform
allegation is refuted; the remaining non-Linux possibility is not an observed leak and establishes no user-work loss.
Settle acceptance of that supported-platform limitation, and if cleanup is required, provide ownership or containment
that covers detached descendants rather than relying only on the original process group.

## Evidence and triage context

- supervise-child.py:64-73 enables subreaping only on Linux; :152-157 performs group cleanup everywhere and
  detached-child cleanup only when subreaping is enabled. e2e/harness-tests/README.md:16-19 discloses the limitation. No
  concrete user-work loss or independently verified non-Linux leak was established.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_04_cor:p2:C5`.

- `test_infrastructure_04_cor:p2:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
