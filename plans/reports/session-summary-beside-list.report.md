## What this was about

Hovering a session showed a summary card over neighbouring sessions, making the list harder to browse. The card now
appears six pixels to the right of the sidebar, level with the row, and moves up when needed to fit the window. This
applies to hover and keyboard focus in both list densities, on web and desktop.

## Things you should know

A window without enough room beside the list keeps the previous above-or-below placement. A showing card notices a
window resize even when its row does not move. Ordinary tooltips, the host-update progress popup, card contents, timing,
dismissal and accessibility keep their existing behavior.

The specifications and changelog describe the placement, and the source TODO entry is removed. No published screenshot
or release capture changed. Terminal host recovery landed during execution; the change was rebased onto it without
conflicts and with an empty comparison against the reviewed patch. Subsequent checkpoint-stop changes affect separate
supervisor functions and specification paragraphs, so they did not require another rebase or tooltip test run.

## Open questions and possible follow-ups

None identified. Native macOS appearance was not checked; WebKit browser checks cover the shared placement script.

## The PRs

- [#2972: move session summaries beside the list](https://github.com/scode/farhelm/pull/2972/changes) — draft,
  `plan/session-summary-beside-list/01-card-beside-list`, head `dce09156e385628a3d6d9768deedd9435625ecf3`.

## Checks run, reused and skipped

Run for this change:

- From the UI JavaScript test directory, recorder → `node --test`: 512 passed, none skipped, run
  `a08b938a-87e1-4a6f-a29f-bc7af031b986`. This includes the four new pure placement cases.
- From `e2e`, controlled recorder → `npx playwright test 'tooltip\.spec\.ts|popover-identity-movement\.spec\.ts'`: 50
  passed, 25 each Chromium and WebKit, none skipped, run `a8ecf416-ef74-4c31-acb4-3e80bdab6924`. Checks cover both list
  densities, the resting-pointer resize fallback, ordinary tooltip placement and existing popover movement.
- With `FARHELM_E2E_SUITE_TERMINALS=host` intentionally retained through the recorder, the same browser selection: 50
  passed, 25 each Chromium and WebKit, none skipped, run `069c9fe0-1dad-4a04-b04c-fbab481596cc`.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo fmt --all -- --check`, and the release web UI build passed.
- `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/session-card-beside-list.md`,
  `python3 releasing/check-changelog.py format` and whitespace checks passed. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 397 delays with no missing rationale.

Both browser records have complete source identity and output, validated runner policy, zero retries and the matching
pinned tmux 3.7c. The final JavaScript run, lint, desktop compilation, web build and formatting evidence from the
reviewed working revision `02853b81` remain applicable after the rebase: the patch comparison was empty and the UI
source and assets are identical. Markdown and whitespace checks also passed on the rebased tree. The existing CLI and
fixture binaries were reused: the intervening backend change concerns recovery from terminal host failures, which these
healthy-stack tooltip scenarios do not exercise. This is placement evidence, not recovery coverage.

Skipped the wider Rust, browser, installer and release suites because the change adds one row attribute and shared
tooltip placement, covered by compilation, pure-function and page tests. Skipped the staged-fleet visual capture because
the actual browser geometry assertions cover the placement and no documentation screenshot needs refreshing. CI was not
dispatched; this repository runs it only on demand.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in correctness, design, idiomatic
code or the acceptance criteria. Its prompt included the full goal and test-authoring rules. The separate documentation
pass covered every touched file. A fresh wording reader understood the exact commit and PR text with no contradictions
or convention issues. No scope departure or corrective review round was needed.
