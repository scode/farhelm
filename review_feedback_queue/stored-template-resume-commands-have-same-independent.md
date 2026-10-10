# Stored template resume commands have the same independent display defect

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The template resume seed conceals bytes kept for later execution.

## Details

F209 — **definite** — `crates/farhelm-ui/src/list/create_form.rs:508` — Stored template resume commands have the same
independent display defect

The resume-command setter independently bypasses safe display seeding, so its apparent command can omit or reorder
meaningful bytes retained for a future restart. Correcting the initial command setter does not repair this site. Seed
present resume commands through escaped display and a separate raw value, preserving untouched bytes and the explicit
null meaning that Resume is off.

## Evidence and triage context

- crates/farhelm-ui/src/list/create_form.rs:505 enables Resume and :508 stores its raw command while clearing seed
  tracking.
- crates/farhelm-ui/src/list/create_form.rs:5777 renders a single-line text input; :4003 submits the retained string.
- crates/farhelm-proto/src/session_launch.rs:134 requires an agent and placeholders but permits quoted newlines; :387
  derives resume arguments from the retained string.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage. SPEC_impl.md:598 supplies the analogous clone treatment; SPEC.md:2178–2185 prevents treating an
  already-authorized template writer as an unauthorized execution actor.

Caveats:

- A usable resume command still needs a declared agent and the required placeholders.
- Execution requires a later resumable restart.
- No runtime reproduction or unauthorized template-write bypass.
- Requires a valid agent declaration and required placeholders.
- No browser reproduction or additional authority bypass established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_09_cor:p1:F2`.

- `ui_desktop_09_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
