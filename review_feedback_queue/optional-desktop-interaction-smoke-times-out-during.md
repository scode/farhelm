# Optional desktop interaction smoke times out during normal shell deletion

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The optional desktop smoke times out during normal deletion.

## Details

F273 — **definite** — `scripts/desktop-smoke.sh:1318` — Optional desktop interaction smoke times out during normal shell
deletion

Its client deadline equals the normal five-second graceful-stop period. An interactive shell can consume that entire
grace, leaving no time for escalation and the response even when deletion works correctly. Use the existing
thirty-second deletion budget so healthy teardown is not reported as an interaction regression.

## Evidence and triage context

- scripts/desktop-smoke.sh:1262 creates Bash, :1303–1316 exercises its terminal, and :1318 imposes --max-time 5 on
  deletion. The identical mechanism is documented with a previous observed failure at :968–975. service/sweep.rs:50
  defines a five-second grace; :1117–1126 and :1661–1664 restrict early SIGHUP handling to tabs, not the agent Bash
  process.
- scripts/desktop-smoke.sh:1262 selects Bash and :1318 gives DELETE five seconds. Lines 968–977 document and avoid the
  same deadline defect in the default leg. service/sweep.rs:50 sets five seconds; :1134 waits the grace; :1661 limits
  accelerated SIGHUP to tab units.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Applies to DESKTOP_SMOKE_LEGACY_INTERACTION=1 with an ordinary interactive Bash; no runtime test was run.
- Optional interaction leg only; normal Bash signal behavior is the fixture premise.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_09_cor:p1:F3`,
`automation_website_09_sec:p1:F3`.

- `automation_website_09_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `automation_website_09_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
