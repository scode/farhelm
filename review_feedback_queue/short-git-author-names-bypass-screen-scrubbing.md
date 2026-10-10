# Short Git author names bypass screen scrubbing

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Screen capture could retain a short configured Git author name.

## Details

F90 — **possible** — `scripts/capture-agent-screens.py:125–128` — Short Git author names bypass screen scrubbing

Only author-name components with at least three characters are added to screenshot replacements and refusal checks. A
configured name made entirely of shorter components can therefore survive if it appears in a capture and another
identity rule does not remove it. No actual capture or disclosure was observed. Scrub and check the complete configured
author name, retaining component replacement as a separate measure.

## Evidence and triage context

- scripts/capture-agent-screens.py:125–128 filters author-name components.
- scripts/capture-agent-screens.py:263–288 saves captures;
  crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:200–237 independently checks only email and absolute home
  paths.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- docs/agent-screen-fixtures.md:43–46 requires manual checks for identities the tool does not know; this identity is
  explicitly available to the scrubber, so coverage is unclear.

Caveats:

- Requires such a configured name to appear in a capture and not be removed by another identity rule. No real identity,
  capture, test or published disclosure observed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_08_cor:p2:F1`.

- `automation_website_08_cor:p2:F1`: confidence as filed: possible; suggested bucket as filed: highest.
