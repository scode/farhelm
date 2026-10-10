# Template-supplied session names rendered raw

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A template's saved session name could be misleading in the launcher.

## Details

F300 — **possible** — `crates/farhelm-ui/src/list/create_form.rs:615` — Template-supplied session names rendered raw

The stored name is marked edited and rendered directly in a single-line input. Presentation-unsafe Unicode can therefore
conceal or reorder the apparent name. This is narrowed to editable-name correctness; an independent wrong-target or
security consequence is not established. Use safe display seeding with separate raw-byte ownership and explicit editing
semantics, preserving stored content until actual input.

## Evidence and triage context

- create_form.rs:535-536 converts the stored name into a Name action; :615-620 stores it raw and marks it edited.
  :3143-3148 returns that raw title when no clone default applies; :5374-5386 renders it in a single-line input.
  farhelm-proto/src/launcher.rs:336-342 places only a size bound on template fields, so presentation-unsafe Unicode can
  reach this path. Escaped checkout paths at create_form.rs:3106 and :5202 refute the stronger
  concealed-filesystem-target claim, not the raw-name discrepancy. No exact acceptance or queue coverage was found.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_09_sec:p1:C4`.

- `ui_desktop_09_sec:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed:
  not separately tagged in candidate list.
