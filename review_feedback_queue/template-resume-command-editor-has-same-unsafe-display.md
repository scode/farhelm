# Template resume-command editor has the same unsafe display boundary

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The template resume-command editor can misrepresent its retained command.

## Details

F57 — **definite** — `crates/farhelm-ui/src/list/templates.rs:651` — Template resume-command editor has the same unsafe
display boundary

The resume-command field has its own raw rendering and edit path for remote-derived command bytes. Presentation-unsafe
characters can conceal or reorder what the user sees while the original bytes remain available for later execution. This
is a distinct inspection boundary from the start command; execution requires a later launch and resume operation. Apply
escaped initial display with separate raw-byte ownership to this field, rather than relying on launcher escaping.

## Evidence and triage context

- crates/farhelm-ui/src/list/create_form.rs:2594-2605 seeds a command launch's resume command through the
  escaped-display/raw-seed helper.
- crates/farhelm-ui/src/list/create_form.rs:313-318 snapshots the original resume bytes.
- crates/farhelm-ui/src/list/save_template.rs:69 retains those bytes in TemplateFields; lines 183-186 forward the saved
  template.
- crates/farhelm-ui/src/list/view.rs:3420-3427 opens the saved template.
- crates/farhelm-ui/src/list/templates.rs:651-652 renders and edits the raw resume command.
- crates/farhelm-proto/src/launcher.rs:621-622 preserves the stored resume command during template application.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Distinct editable field from the start command.
- Execution requires a later launch and resume operation.
- No browser reproduction was performed.
- Retain definite presentation mismatch as correctness/other. Supplied evidence establishes raw invisible/directional
  text, not execution or privilege escalation; template writers are already authorized to change commands. Lossless
  newline-editing findings remain separate in highest.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_cor:p1:F3`.

- `ui_desktop_11_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.
