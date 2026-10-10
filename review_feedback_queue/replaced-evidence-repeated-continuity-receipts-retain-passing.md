# Replaced evidence or repeated continuity receipts retain a passing verdict

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Evidence replaced during digest capture could inherit a passing validation.

## Details

F321 — **possible** — `releasing/mac-vm-test/brick_test.py:444–479` — Replaced evidence or repeated continuity receipts
retain a passing verdict

The brick-test recorder validates bytes read earlier, then rereads referenced files to compute retained digests.
Replacement between those steps can make a different file become the bound evidence without repeating validation; later
matching-digest checks accept it. Concurrent replacement was not observed. Hash the same bytes that were validated or
otherwise bind validation and digest capture to one snapshot. Repeated continuity receipts are separately rejected.

## Evidence and triage context

- brick_test.py:444 reads evidence into contents; :447–472 validates those bytes. Line 479 rereads each reference to
  compute the retained digest instead of hashing the validated contents. A concurrent replacement after validation but
  before :479 can become the bound file; verdict at :525–527 then accepts its matching digest without rerunning the GUI
  or artifact-verification checks. Continuity replay is separately refuted by :240–252 and continuity.py:86–92. The
  workflow lock at brick_test.py:345–351 serializes workflow transitions, not independent writes to evidence files.
  Concurrent evidence replacement was not observed; review_feedback_queue/publisher-copy-race.md concerns different
  files and a different publisher and is not exact coverage.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_07_sec:p1:C5`.

- `automation_website_07_sec:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
