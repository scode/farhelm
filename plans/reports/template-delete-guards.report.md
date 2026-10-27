# Template Delete keeps its promises

## What this was about

Two template-deletion failures needed fixes. After Retry, the desktop app could install permanent deletion while the
button still showed plain “delete” and promised Undo. A deletion whose reply was lost could also leave its deleted
template in the list with a raw transport error. The third outcome was an accepted selection race: selecting template B
and immediately clicking Delete can delete B while the editor still shows A. The maintainer decided that the target
follows the delivered click in that brief gap; the button's explicit Undo offer still has to hold.

The stack binds each delete button to its own fixed Undo promise. A missing Delete reply now reports that deletion may
have happened and reloads the list, without Undo. The accepted selection race received only feedback cleanup and its
execution record. All three PRs remain drafts for the landing monitor.

## Things you should know

- This preserves the Undo offer, not a new restoration mechanism. Undo still saves through ordinary template validation,
  which can refuse an invalid stored value.
- An uncertain deletion clears any earlier Undo notice in the same dialog. Only the template list is refreshed;
  quick-launch marks and number keys are not reconciled. Known helm refusals retain their existing error handling,
  including the rename path's handling of its internal deletion.
- A sign-in refusal or timeout before sending may also produce the uncertainty message. The plan explicitly accepts that
  conservative classification.
- The stack was rebased over the installer, restart-trust, reporter-preparation and terminal-warning fixes that landed
  during execution. Their behavior does not interact with Templates. The only conflict was adjacent feedback-index
  removals; both sets of removals were kept. No product correction was needed.

## Open questions and possible follow-ups

No execution block remains. The existing TODO to treat the GUI–helm connection as infallible may replace the local
lost-reply stopgap. This plan does not decide whether desktop recovery crashes or restarts the interface, or whether
request deadlines remain.

## The PRs

- [#2745](https://github.com/scode/farhelm/pull/2745/changes): keep the painted delete button's Undo offer through
  Retry.
- [#2746](https://github.com/scode/farhelm/pull/2746/changes): discard the accepted selection-race feedback; no code or
  specification change.
- [#2748](https://github.com/scode/farhelm/pull/2748/changes): report uncertain template deletion and refresh the list.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(list::templates::tests::)'`: 29 passed, no runtime
  skips, recorder `9a3fb082-fdfc-4dc6-9526-4246073e0bf2`. This includes the failed-read/Retry/native-listener regression
  in both kind directions. The focused regression also passed in `deb2683d-4f35-41c2-b87c-ec0c981d1ba8`.
- The required old-behavior negative control failed at the intended callback assertion: Permanent instead of Restorable,
  recorder `c95b2de4-8142-4bd9-9f79-c0699b2c35c1`. The fixed source was restored before the 29-test pass. Two initial
  development attempts failed on fixture compilation/setup and visibility lookup
  (`4da7b382-26fe-4adb-b858-241f8208078b`, `5b62fca7-7e6f-4c86-aa4b-b5af1384da4d`); their evidence was retained and the
  fixture corrected.
- `npx playwright test 'templates\.spec\.ts'`: 118 passed, 59 per engine on Chromium and WebKit, zero skips, recorder
  `088a958a-4518-4993-a898-522f789573e9`. The final lost-reply, refusal and existing rename/delete/Undo selection passed
  six cases, three per engine, recorder `bea26812-121d-4a9d-9b54-901b497899d8`. The lost-reply case verifies the real
  helm committed Delete before the reply is dropped.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` and desktop production-library Clippy passed on the final
  implementation. Desktop all-targets Clippy failed on an existing type-complexity warning in an untouched approval-card
  test; the same signature was verified on base `db3ec9fa`. That broader lint check is not claimed as passing.
- Cargo formatting, selected dprint checks, changelog format, whitespace checks and the test-delay checker passed; 322
  delays were inspected with zero missing rationales. The agent-instruction checker passed with two existing
  heading-reference warnings. PR2 needed only source/document checks.
- The CLI and browser fixture builds passed. The Dioxus release web build passed for each code change. An initial build
  invocation selected a different tool named `dx`; using the installed Dioxus 0.7.10 executable resolved that setup
  error.
- The 29 unit cases and full browser run cover PR1's runtime source, represented by original PR1 head `a63baa97`. Their
  controls, helpers and selectors are unchanged in PR3; its new API branches and both callers are covered by the final
  six browser cases. The backend and fixture builds were reused because PR2/3 change neither their code nor their
  version. After rebase onto `513c905b`, each PR's product diff remained unchanged; the upstream audit found no changed
  premise or interaction invalidating these checks. No runtime rerun followed solely from rebase or URL bookkeeping.
- The full workspace, full browser suite and desktop-shell smoke were skipped: the affected control boundary has a
  native event-dispatch regression, and the affected HTTP paths have browser coverage on both engines. No live
  installation was changed.

## Review gate's outcome

Each PR passed its required single fresh-context gpt-6.1-sol reviewer at high effort, with no actionable findings. The
PR1 reviewer also checked the fixture attribute-group correction. Commit/PR wording passed cold reads. A separate
read-only audit checked all upstream changes against the stack before rebase; it found no semantic correction or extra
runtime validation needed.

### Landing

Landed as built. Template deletion in the Templates dialog could break its promises in two ways: after Retry, the
desktop app could delete permanently while the button still read plain "delete" and promised Undo, and a deletion whose
reply was lost could leave the deleted template in the list beside a raw connection error. The dialog now keeps two
separate buttons, "delete" (with Undo) and "delete permanently", and shows the permanent one only for a template that
fails the current save rules (Undo would have to save it again), so a Retry that re-reads the template can no longer
turn the visible "delete" into a permanent one; and a lost reply now says the deletion may have happened and reloads the
list, without Undo. The third finding, that selecting another template and clicking Delete at once can delete the newly
selected one while the editor still shows the old one, you decided is acceptable; only its review-queue entry was
removed.

- #2745: keep the painted delete button's Undo offer through Retry.
- #2746: close the accepted selection-race finding, with no code change.
- #2748: report an uncertain template deletion and reload the list.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained only hidden checkout Trash (#2744), which touches nothing in the Templates dialog.
A fresh-context reviewer and I read this plan against it and against earlier template work already on main (the Steel
restyle, quick launch marks, the template panel's Escape handling, Restart with's template refusal) and found nothing
that interacts. The second, always-hidden delete button is safe for every other test: they find the button by its name
or only look at the visible one.

#### Fixes made at landing

None.

#### Things you should know

The report already says that a lost reply clears an earlier Undo notice in the same dialog and does not update quick
launch marks. The landing review found a few more consequences of the lost-reply path, all rare:

- After a lost reply, the editor stays open on the template that may have been deleted. Saving re-creates it, and Delete
  then shows a message asking to reload the template list.
- SPEC.md, which this plan did not change, says a click on plain "delete" always offers Undo. The lost-reply path offers
  none; that exception rests on SPEC.md's general rule that an operation whose outcome is unknown reports that, and on
  your triage decision. A possible follow-up is to name the exception in that sentence; the existing TODO entry about
  treating the connection between the app and the helm as infallible would remove the stopgap instead.
- When a reload changes whether the selected template passes the save rules (it was edited elsewhere, say), the visible
  delete button swaps, and keyboard focus on the old one is lost.
- #2748 rewrote a shared comment in the UI's request code and dropped two details about provisioning from it; restoring
  them is optional cleanup.

#### Checks

This round landed only this plan, on main after hidden checkout Trash:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason, `dprint check` across the repository, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (two warnings, both about citations main already had), the workspace debug build
  and the UI's release web build: all passed. The desktop build's all-targets lint was not run; the report says it fails
  on an existing warning in code this plan does not touch.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, with the pinned test runner, run
  `8ad1ff47-c4aa-4b34-b73d-7f889990460a`: all 784 passed. Its desktop-build tests, which include the plan's Retry test
  (that fix is desktop-only), `cargo nextest run -p farhelm-ui --features desktop`, run
  `c059a870-fc64-43f8-a371-1228e218ca8b`: 964 passed and 1 failed, "a hung save releases waiters at their deadline", an
  open flake already on TODO.md's Deflake list (a waiter released a moment before its deadline), in save-ordering code
  this plan does not touch. The UI's JavaScript and stylesheet tests (`node --test` in `crates/farhelm-ui/js-tests`),
  run `9a77c0c3-15d6-44f3-9e91-5d8d64086a26`: 358 passed.
- `npx playwright test` in `e2e/` with `tests/templates.spec.ts`, `tests/quick-launch.spec.ts`,
  `tests/template-list-shortcut.spec.ts` and `tests/popover-template-movement.spec.ts`, on Chromium and WebKit, three
  workers each with its own helm and supervisors, no retries, run `8bbc6de3-211c-4954-88e6-626cc0e501d6`: all 232
  passed.
- Reused: none. Skipped: the rest of the Rust workspace (nothing outside the UI changed, and only the desktop app
  depends on it), the full browser suite (the four specs cover the template list), and running the desktop app itself
  (its Retry fix is covered by the desktop-build test above).

The report's browser run covered the templates spec; this landing also ran the specs that share the template list.
Nothing in the executor's report became untrue at landing.
