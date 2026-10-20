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

### Landing

Landed as #2428, #2431 and #2433, merged bottom-up. It was the third of six plans in this landing round, after
download-staging-sweep and sidebar-bell-follows-row. Its content is what the report describes, rebased onto this round's
main, except for one landing fix to #2431 that changes behavior, described below; please read that one.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 108's sixteen plans and documentation and plan bookkeeping. A fresh-context
reviewer and I read them and the other plans of this round against this plan. The one that touches the same code is
launch-sequences, from round 108:

- It reworded the quick-launch bolt's explanation for a template that cannot be marked, and kept a lit bolt clickable on
  such a template so that its mark can still be cleared. This plan's #2431 changed the same bolt to read the current
  shortcuts before writing. The two conflicted textually; the landing kept launch-sequences' wording and this plan's new
  click handling.
- The interaction behind the landing fix below also involves launch-sequences: before the fix, a click meant to clear a
  lit bolt could re-mark a template that launch-sequences had made ineligible for the mark.

menu-arrow-focus-race, later in this round, changes the number-key menu's focus handling in the same file and test; the
combined test passed on both engines. The other conflicts were in the review feedback queue's index, where each plan
removes its own entries; all removals are kept.

#### Fixes made during the landing

- #2431, the quick-launch bolt (⚡) in an open Templates dialog. The plan made a bolt click read the current shortcuts
  before writing, but the click carried only the template's name and toggled whatever it read. So whenever the dialog
  was out of date, the click did the opposite of what the user saw:
  - a click on an unlit bolt for a template an agent had just marked cleared that mark;
  - a click on a lit bolt after an agent had moved the mark elsewhere or cleared it put the mark back on this template,
    even one launch-sequences had made ineligible for it.

  Both undo an agent's change made while the dialog was open, which your recorded decision for this plan rules out (a
  GUI write never undoes an agent or command-line change made while the view was open). The plan's own browser check
  asserted the wrong outcome: that the first click cleared the agent's mark. The fix makes the click carry what the user
  asked for, mark or unmark, as the bolt showed it, and writes nothing when the freshly read shortcuts already match
  what the user asked for. The check now expects the agent's mark to survive. The number-key menu already worked this
  way and is unchanged.

  The plan's rule for this PR was to block if it went over about 100 changed lines. The report's three size figures (66,
  100 and 30 lines) are one per PR, so #2431 measured exactly 100; with the fix it is 113 (specification, code and
  tests), mostly the corrected click handling and a comment explaining it. I did not block, because your decision
  already settles the behavior. Undoing only the fix would bring back the clicks that undo an agent's change; the
  realistic alternatives are to keep it, or to revert #2431 as a whole, which returns to an open dialog overwriting
  newer shortcut assignments.

#### Checks

Every plan in this round was executed while round 108's sixteen plans were still unlanded, so none had been tested
against them. Together the six change a menu focus check that every menu uses, which session a page opens on load, the
desktop app's quit path, helm preferences and storage, and a browser-test helper many specs share. That is why this
round ran the whole Rust workspace and the full browser suite. All of the following ran on all six plans stacked
together on main, including the fixes made while landing them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `scripts/check-desktop-assets.sh`: all passed (formatting and the test-sleep check again after the last test fixes).
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `8c534e03-3c81-4f66-99e3-0e6ffdb526e8` (kept): 3951 passed, 3 failed. Two were
  helm database tests fixed during this landing (see select-newest-on-close's notes); the helm's checkout configuration
  test module then passed, 28 tests, run `faf4d80d-ec6d-4a36-9220-9d522e8f38d0`. The third was
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a known open flake under load (FLAKES.md and
  the TODO deflake entry "Shallow pause resets under load"); no plan here changes terminal streaming or the supervisor.
  Five repeats of that test by itself, with the browser suite running at the same time, gave four passes and one failure
  (run `e34c4af0-729c-4f62-b1bb-db1c6d9e97e2`, kept).
- `cargo nextest run -p farhelm-ui --features desktop`, run `03589e5a-0f7e-423b-ab65-5c3bdd33c33c`: 720 passed; this and
  the desktop compile check above cover the desktop quit change, which lives in the UI crate's desktop code.
  `node --test` in `crates/farhelm-ui/js-tests`, run `c600b345-76c4-42a8-a879-fce21765c8d6`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, in two parts.
  The first run, `68484e27-6c23-45a6-aae5-5539e8c275b2` (kept), was cut short part-way by my own mistake: a narrow
  re-run I started in the same checkout removed files the running suite depends on, so every test it started afterwards
  failed before testing anything. Its results stand for the 64 projects (one engine and one spec file each) whose tests
  all started before that point and passed. Every other project, and the spec files changed by later landing fixes, ran
  again on both engines in run `49ead42e-f608-4e76-a833-6ff945dfb1f8`: 1538 passed, 12 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), and 5 failed. Those 5, and the
  4 the first run had shown before that point, were all tests that assumed how a page behaves on load (which session it
  opens, or how many preference reads and saves it makes) before select-newest-on-close changed it. They are 6 distinct
  tests, all fixed in that plan and passing in narrow runs on both engines (listed in its notes).
- Skipped: the workspace and desktop doctests (no plan adds or changes a documentation example),
  `cargo check -p farhelm-desktop` and the desktop smoke test (the desktop app's own crate is unchanged; its quit path
  change is in the UI crate covered above), the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 108's landings, so its statement that no rebase was needed and its
focused checks no longer describe what landed; the combined runs above supersede them. Apart from the bolt behavior and
size described above, nothing else in the executor's report became untrue at landing.
