# Initial result-publication failure erases the observed child result

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A failed first result write discards known test-run facts.

## Details

F252 — **definite** — `scripts/record-test-run.py:1868` — Initial result-publication failure erases the observed child
result

After the child finishes, failure publishing the first final manifest sends execution into an exception path that
replaces observed status, duration, and cleanup facts with defaults. A successful fallback then durably records
unavailable or false information. Preserve the observed command result across that boundary and retain its fields
alongside the publication error; exercise a one-time first-write failure.

## Evidence and triage context

- scripts/record-test-run.py:1746 obtains CommandResult; :1770-1779 passes its child status, duration and cleanup facts
  to finalize.
- scripts/record-test-run.py:1578-1599 gives those arguments null/false defaults, assigns them into the manifest before
  writing, and does not preserve previous values.
- scripts/record-test-run.py:671-685 performs fallible open/write/fsync/replace operations. A failure escapes the
  initial finalize call.
- scripts/record-test-run.py:1855-1874 catches that failure and calls finalize without any observed CommandResult
  fields. A successful second write therefore publishes the overwritten values.
- scripts/record-test-run.py:1803-1815 separately handles later report-publication failures without this reset; that
  protection does not encompass :1770.
- scripts/test_run_summary.py:192-207 classifies the resulting null child status as unavailable, losing the previously
  observed failure or signal classification.
- scripts/test-record-test-run.py:1067-1073 injects publication failure only after a complete runner report exists; it
  does not exercise the earlier publication boundary.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/FILTER.md:24-42, Rare, self-correcting glitches and imprecise diagnostics: the trigger resembles
  an evidence-IO failure, but the consequence includes a durably incorrect record, expressly excluded at :40.
- SPEC.md:2021-2031, Healthy local filesystems: accepts filesystem-induced failure or stalled progress on a product
  host. It does not explicitly accept replacing already observed test results with incorrect fields during a successful
  fallback publication; scope and consequence do not fully match.
- review_feedback_queue/publisher-copy-race.md:14-22 concerns concurrent screenshot replacement between verification and
  copying. It does not cover command-result erasure after a manifest-write exception.

Caveats:

- Requires the initial publication to fail and the fallback publication to succeed.
- The recorder returns 125; this is not a false successful test run.
- Source inspection only; no runtime reproduction performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_15_cor:p1:F1`.

- `test_infrastructure_15_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
