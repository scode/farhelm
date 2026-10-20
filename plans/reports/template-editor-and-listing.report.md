# Templates editor and listing

## What this was about

Template names containing invisible formatting were escaped in the list but concealed those characters when opened for
editing. An already-open Templates dialog could overwrite a newer shortcut assignment saved by an agent or the command
line. The `farhelm agent templates` table also said only “permissions custom”, without showing the arguments it applies.

The three agreed fixes now have separate draft PRs. The editor shows unsafe names as escapes and retains their original
bytes through unrelated saves; “replace name” clears the name for literal editing. Shortcut edits read current bindings
before deciding the change, and the specification states that GUI writes must not undo agent or command-line changes
made while a view was open. The CLI table shows custom permission arguments with the same quoting as wrapper prefixes.

## Things you should know

Existing names containing emoji joiners remain usable. Their escaped name field is read-only until explicitly replaced;
no new name-validation rule was added. Shortcut displays may still lag until the next interaction, as agreed, and a
failed binding read sends no shortcut write. Existing unsaved template drafts stay intact.

The measured code, test and documentation changes are 66, 100 and 30 lines respectively, excluding ledger, queue removal
and changelog fragments. All three meet their 100-line gates; none was discarded or blocked.

## Open questions and possible follow-ups

None required for this plan.

## PRs

- [#2428](https://github.com/scode/farhelm/pull/2428/changes): expose invisible template-name characters while
  preserving stored names until explicit replacement.
- [#2431](https://github.com/scode/farhelm/pull/2431/changes): preserve current shortcut assignments when editing from
  an already-open Templates dialog, with the agreed specification principle.
- [#2433](https://github.com/scode/farhelm/pull/2433/changes): show custom permission arguments in the template table.

## Checks run, reused and skipped

Runtime commands below ran through the recorder with zero retries, four nextest slots or one browser worker. Browser
selections used Chromium and WebKit. Nextest and tmux were pinned; renderer units do not use tmux.

- `cargo nextest run -p farhelm-ui --lib -E 'test(list::templates::)'`: 17 passed in recorder run
  `b5d454c9-6861-477d-ba78-1b8b458f3e71`.
- `cargo nextest run -p farhelm-ui --lib -E 'test(list::templates::) | test(fresh_bindings_refresh) | test(template_binding)'`:
  after the shortcut change, 18 passed, including the shared binding-refresh test, in
  `4328b729-4961-4491-9b7b-91baa4fcc33a`.
- `npx playwright test tests/templates.spec.ts`: `60239565-7d6a-4354-9405-e19eea0b5fd1` recorded 94 passes and two
  failures in the new name test's cleanup. The test called the existing cleanup helper with the wrong arguments. The
  failed run is retained.
- `npx playwright test tests/templates.spec.ts -g 'template name escapes formatting until explicitly replaced'`: after
  correcting only that call and its failure handling, the selected name case passed on both engines in
  `2e23b839-a5a0-48e6-b8e2-07335b90e470`; the other 94 cases were unchanged.
- `npx playwright test tests/templates.spec.ts -g 'number keys move|template name escapes formatting|the Templates dialog creates, edits'`:
  after the shortcut change, the external-binding regression, name preservation and editor lifecycle passed on both
  engines, six cases in `c4f08a32-7711-43fd-914c-d3d6af5f96c3`. This covers the interaction between the two UI fixes.
- `cargo nextest run -p farhelm --bin farhelm -E 'test(render::)'`: six passed in
  `fea64c13-0c82-4182-8b06-33530c11f4b5`, including the space-bearing argument regression and existing output-escaping
  tests. This covers the third PR's renderer change on parent `2e068576`.
- `cargo build`, two web release builds, `cargo fmt --all -- --check`, UI-library and shipped-CLI-binary Clippy with
  warnings denied, changed Markdown formatting, changelog format, the test-sleep checker and `check-agents-md.py`
  passed. The sleep checker inspected 305 delays with no missing rationale.
- The UI checks cover the first two PRs through shortcut head `2e068576`; the CLI-only change does not alter their
  inputs. The unchanged CLI/helm/supervisor build from the name fix was reused for the shortcut browser run, which used
  the rebuilt web bundle. Later main changes were only other plans' state and blocked-question bookkeeping, with no
  interaction requiring a rebase or more runtime tests.
- Workspace-wide, desktop, installer and unrelated browser suites were skipped: the changed UI paths have direct
  two-engine coverage, and the CLI change uses the existing renderer and quoting helpers. No hosted CI run was
  requested.

## Review gate

Each PR received its required single fresh-context gpt-6.1-sol review at high effort. No unresolved findings remain. The
first name review missed the invalid test-cleanup call; the same reviewer inspected the correction and confirmed it
resolved. Commit messages and PR wording passed fresh cold reads; the shortcut message was rewritten once to remove an
ambiguity.
