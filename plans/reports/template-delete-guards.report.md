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
