# Qualified hostname scrubbing leaves the private domain

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Screen capture could leave the private domain of a qualified hostname.

## Details

F91 — **possible** — `scripts/capture-agent-screens.py:118` — Qualified hostname scrubbing leaves the private domain

The scrubber retains only the hostname portion before the first dot. Replacing that prefix in standalone
qualified-hostname output leaves the domain behind, and fixture validation has no complete-hostname check. This requires
such output outside an email-shaped string; no capture or disclosure was observed. Replace and reject the full hostname
before applying the short-hostname replacement.

## Evidence and triage context

- scripts/capture-agent-screens.py:118 truncates the hostname; :93–151 configures scrub rules.
- crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:200–237 does not independently reject hostname/domain
  text.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- docs/agent-screen-fixtures.md:43–46 asks for manual inspection, but does not expressly waive handling the obtainable
  full hostname.

Caveats:

- Requires a qualified hostname shown outside an email-shaped string. Email-shaped output is generally removed
  separately. No actual capture or disclosure observed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_08_cor:p2:F2`.

- `automation_website_08_cor:p2:F2`: confidence as filed: possible; suggested bucket as filed: highest.
