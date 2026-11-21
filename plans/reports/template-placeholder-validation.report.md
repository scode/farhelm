# Check command placeholders when saving launch templates

## What this was about

Farhelm used to save a launch template containing a quoted or joined placeholder and reject it only when you tried to
launch it. An example is a command containing `"{cwd}"` or `--dir={cwd}`, where the placeholder needs to stand as its
own unquoted word. Farhelm now refuses the save if either the launch command or the resume command has a placement
error. Any previously saved version stays in place.

## Things you should know

The planning decision was to validate placeholder placement on save and check rules that need the assembled setup at
launch. A partial template can still leave choices such as the agent type to another template or the launcher. Existing
templates with invalid placement remain readable and editable. Their delete button says **Delete permanently** because
**Undo** would have to save the invalid command again.

## Open questions and possible follow-ups

No open decisions.

## PRs

- Check placeholders on save and close the matching TODO and review finding.
  [#4355](https://github.com/scode/farhelm/pull/4355/changes)

## Checks

- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib` with filters for command placeholders and
  the template save, apply, edit and delete modules passed all 172 selected tests (recorded run
  `51135922-bf9a-4e48-bc59-caa5f3794642`, draft revision `41205bdb`). The checks verify that a refusal leaves the saved
  template untouched, a partial template can still save, and an older template remains readable and gets the right
  Delete label after a replacement under the same name. These results still apply after the Clippy correction, which
  compares the same stored template by reference in one assertion.
- Passed `cargo clippy -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib --tests -- -D warnings`,
  `cargo fmt --all -- --check`, `dprint check <changed Markdown files>`, `python3 releasing/check-changelog.py format`
  and `python -B scripts/check-test-sleeps.py`. `python3 scripts/check-agents-md.py` had zero errors and one existing
  archived heading warning.
- Skipped the full Rust and desktop smoke suites. The selected tests cover the shared validator, actual HTTP save and
  read, and the desktop editor. Skipped screenshot capture because the reference scenes contain no invalid placeholders
  and the existing Delete control's label is covered by the editor test.

## Review gate

The required independent review passed after correcting the cost of updating the Delete label. The editor keeps the
label current when a saved template changes and avoids reparsing the command on redraws. The review also led to a
storage-level test for older templates and more precise implementation spec wording. The commit message passed a
separate cold read.
