# Template command editors hide meaningful characters and can silently remove newlines — Saved command editor

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Editing a saved launch command silently removes stored newlines.

## Details

F46 — **definite** — `crates/farhelm-ui/src/list/templates.rs:642` — Template command editors hide meaningful characters
and can silently remove newlines — Saved command editor

The saved launch-command field uses a raw single-line input. HTML sanitizes line breaks in that control, and an input
event replaces the entire stored field with its sanitized value. Editing one part of a multiline command can therefore
remove a meaningful newline elsewhere; hidden and directional characters also remain hard to inspect. Opening and saving
without editing does not itself prove corruption. Provide lossless multiline editing and visible unsafe characters,
preserving untouched raw values and deliberate round trips.

## Evidence and triage context

- crates/farhelm-ui/src/list/templates.rs:346–352: opening a template clones its raw fields into the draft.
- crates/farhelm-ui/src/list/templates.rs:642–643: the command uses an input without a multiline type and replaces
  fields.command with e.value().
- crates/farhelm-ui/src/list/templates.rs:651–652: the resume command uses the same raw input/whole-field replacement
  pattern.
- crates/farhelm-proto/src/launcher.rs:320–342: shape validation prohibits controls in the template name, but fields
  receive only a serialized-size check.
- crates/farhelm-ui/src/list/templates.rs:357–385: draft validation checks compatibility and empty values, not embedded
  line breaks.
- crates/farhelm-ui/src/list/templates.rs:933–958: Save validates and writes the resulting draft fields.
- crates/farhelm-proto/src/session_launch.rs:374–409: command and resume text are shell-split, allowing a quoted
  argument to preserve a literal newline.
- crates/farhelm-ui/src/list/create_form.rs:282–319,4662–4688 and crates/farhelm-ui/src/list/save_template.rs:65–69:
  Save as template can preserve raw relayed command and resume values.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:598–603 describes escaped display and raw-value preservation for cloned launcher controls; it does not
  accept raw rendering or newline corruption in the Templates editor.
- SPEC_impl.md:3904–3911 specifies direct draft fields and compatible partial templates, not lossy editing.

Caveats:

- Opening and saving without an input event does not by itself prove corruption: the Rust draft initially retains the
  original bytes.
- The newline consequence follows standard HTML text-input sanitization; no browser reproduction was performed.
- The editor does not execute commands automatically.
- Command and resume are separate affected controls within this single original finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_11_sec:p1:F2`.

- `ui_desktop_11_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
