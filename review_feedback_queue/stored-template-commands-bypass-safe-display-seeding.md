# Stored template commands bypass safe display seeding

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The launcher can display a template command differently from its retained bytes.

## Details

F208 — **definite** — `crates/farhelm-ui/src/list/create_form.rs:495` — Stored template commands bypass safe display
seeding

Stored template commands bypass escaped display seeding. A quoted newline can disappear from the single-line input while
remaining in the submitted command, and invisible or directional characters remain active in display. This is a definite
review/editing mismatch; unauthorized execution is not established because template writers already have command
authority. Use escaped display with a separate raw seed, preserving original bytes until actual input.

## Evidence and triage context

- crates/farhelm-ui/src/list/create_form.rs:495 stores raw template command text and discards raw-seed tracking.
- crates/farhelm-ui/src/list/create_form.rs:2739 copies that text for the single-line input at :5669.
- crates/farhelm-ui/src/list/create_form.rs:3994 submits the retained text through submitted_field, whose :1676 fallback
  returns it unchanged.
- crates/farhelm-proto/src/session_launch.rs:119 validates shell splitting and :374 derives execution arguments from the
  retained command.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:2178–2185 authorizes permissioned template writers but explicitly preserves GUI correctness. SPEC_impl.md:598
  documents analogous safe clone seeding. TRIAGE_OUTCOMES.md:2290 fixes a different session-header site. No existing
  item covers this setter.

Caveats:

- No unauthorized template-writing route is claimed.
- No browser reproduction.
- This is executable-text misrepresentation, not proven HTML or JavaScript injection.
- No unauthorized template-writing route or privilege escalation established.
- Keep separate from the resume setter.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_09_cor:p1:F1`.

- `ui_desktop_09_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
