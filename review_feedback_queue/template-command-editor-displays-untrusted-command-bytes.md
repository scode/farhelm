# Template command editor displays untrusted command bytes directly

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The template launch-command editor hides meaningful remote-derived characters.

## Details

F56 — **definite** — `crates/farhelm-ui/src/list/templates.rs:642` — Template command editor displays untrusted command
bytes directly

Cloning to a template preserves raw command bytes and opens an editor that places them directly in its text control.
Invisible or directional characters can make the apparent command differ from the bytes retained for a later launch. The
launcher's escaped display does not protect this separate editor, and opening it does not execute anything. Give the
field escaped initial display, separate ownership of raw bytes, and an explicit edited flag following the launcher's
model.

## Evidence and triage context

- crates/farhelm-ui/src/list/create_form.rs:2583-2588 seeds Clone's invocation through reseed_cloned_field; lines
  1676-1697 separate escaped display from original submission bytes.
- crates/farhelm-ui/src/list/create_form.rs:306-310 snapshots the original command; lines 4662-4668 pass that snapshot
  to Candidate::from_state.
- crates/farhelm-ui/src/list/save_template.rs:65-69 retains the command; lines 183-186 save and forward the actual
  template.
- crates/farhelm-ui/src/list/view.rs:3420-3427 closes the launcher and opens that template; templates.rs:863-870
  initializes the editor from it.
- crates/farhelm-ui/src/list/templates.rs:642-643 renders the raw command and edits it directly.
- crates/farhelm-proto/src/launcher.rs:615-616 copies the stored command when applying the template.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:598-603 requires escaped display with preserved raw bytes for Clone fields; it does not authorize raw
  template-editor display.

Caveats:

- Requires presentation-unsafe command bytes and user interaction.
- Opening the editor alone does not execute the command.
- No browser reproduction was performed.
- Retain definite presentation mismatch as correctness/other. Supplied evidence establishes raw invisible/directional
  text, not execution or privilege escalation; template writers are already authorized to change commands. Lossless
  newline-editing findings remain separate in highest.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_cor:p1:F2`.

- `ui_desktop_11_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
