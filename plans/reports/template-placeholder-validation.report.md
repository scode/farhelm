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

### Landing

#### What landed with it

Two other plans landed just before this one in the same round: antigravity-workspace-trust (an Antigravity workspace
trust switch and Waiting for its trust question, protocol 82) and model-family-cleanup (launcher model family shortcuts
and a shorter wrong-agent refusal). Since this plan's work started, main had gained only the Add host takeover wording
fix and two TODO entries. An independent reviewer and I read the three plans' changes against each other.
Model-family-cleanup changes a different part of the same launch code. Antigravity adds a placeholder that only an
Antigravity launch fills with the working directory; the save check treats it as ordinary text, as Command launches do,
and a template carries no trust setting to check. The landing changed nothing in this plan.

#### Checks

This round landed three plans together: antigravity-workspace-trust, model-family-cleanup and
template-placeholder-validation. Between them they change the protocol (to 82), the helm's database (to version 93), how
the supervisor reads Antigravity's screen, the helm's launches, templates and preferences, the desktop app (Settings,
the note above the terminal, the launcher's model field and search, the template editor), screenshots, the docs website,
SPEC.md and SPEC_impl.md. So the landing ran the whole workspace suite, the desktop app's tests, the app's end-to-end
smoke test and the main lint (clippy) on a separate arm64 Linux test machine, and the screenshot comparisons, the
website build and the small checks locally on x86_64. All of these ran on the three plans stacked together on main.

On the plans as delivered:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`8bcf674c-ac40-4153-932e-8c1088275db5`): all 8182 tests passed, and no test reported skipping itself because the
  machine lacked something it needs (a systemd user session, ssh to itself, Podman).
- `cargo nextest run -p farhelm-desktop`, on the test machine (`6780e911-a7be-4097-ba35-431188b6f40b`): all 180 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (`084e7104-a303-472d-8b1b-04d8906d56b6`): every step passed
  but paste, which skips on Linux as it always does.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (`2980a7fb-c943-402e-99cf-5ac085118d90`):
  passed, with Antigravity's two new Settings pictures.
- `python3 -B scripts/test-gui-screenshot-batch.py` (which validates the screenshot list), the website build
  (`bun run build`), `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

After the landing fixes, #4362 for Antigravity (one function in the app's Settings code, which still shows the same help
text, plus SPEC.md, SPEC_impl.md and a changelog fragment) and #4363 for model-family-cleanup (a docs page):

- `cargo nextest run -p farhelm-gui`, on the test machine (`c609aff2-d2f1-4346-9cb8-16f37e4a1266`): all 3226 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed.
- The website build, `cargo fmt --all -- --check`, `dprint check` and `python3 releasing/check-changelog.py format`,
  locally: passed.

Reused for the fixes: the workspace suite, the desktop tests, the smoke test and the screenshot comparisons, since the
Settings help shows the same text and nothing else that runs changed. Skipped: Mac runs, since no macOS-specific code
changed; the Antigravity note's devbuild check covers the Mac by hand.

#### Things you should know that the report does not say

Nothing beyond the report. One small wording point: SPEC.md now says a misplaced placeholder "is refused when saving a
launch or a template". A launch is started rather than saved, so "when starting a launch or saving a template" would
read better; the meaning is clear as it is.

#### What you should still do

Nothing. The tests cover the save refusal, an older template with a misplaced placeholder staying readable, and its
**Delete permanently** label.
