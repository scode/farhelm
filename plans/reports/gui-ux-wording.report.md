# GUI wording and specification corrections

## What this was about

The November 18 triage selected 21 outcomes and kept the existing app behavior. The Settings Close tooltip said closing
would discard unsaved fields. It now explains that Settings can ask what to do with an unfinished recipe edit first. The
Keyboard shortcuts dialog omitted what a number shortcut does during a launch sequence. It now explains that the
shortcut adds the unfinished word as the next template name, then queues the template bound to that number. Pressing
Enter later launches the queued sequence in that order. The other corrections bring the guides, specifications, and
comments into line with the app.

## Things you should know

The save rules now say that saving a template or applying credentials can leave the attempted draft editable after a
refusal. The app waits for the user to correct and save it again. Checkout folder edits already revert and show the
refusal following [#4160](https://github.com/scode/farhelm/pull/4160/changes). If the latest save of sidebar width,
terminal text size, or navigation rail expansion fails, the control can return to its saved value with no error shown.
If Settings closes before an ntfy save is refused, that instance of Settings loses the refusal. The next dialog reads
the saved settings when Notifications first opens.

Custom command launches first apply a literal shell-word check. For example, an unquoted `{cwd}` passed as its own
argument can be filled in, while `--directory={cwd}` is refused. Commands that pass the checks retain Bash syntax. The
Launch templates guide now explains that a keyboard layout that types a digit with Shift can start a template name with
that digit during a launch sequence.

The OMP permission correction concerns new requests, which default to Guarded. A permission omitted in an older stored
launch retains its historical YOLO interpretation. The conversation comment now describes OMP's early save on `/new`.
The security section records the decision to trust the user's own configuration. On Linux, Farhelm looks up the
notification sound player `pw-play` on the user's PATH. Flags supplied in a custom Muse command can conflict with
Farhelm's hooks. Applying credentials on a sandbox host can grow an agent settings file beyond 1 MiB. Subsequent reads
refuse the file until the user reduces its size.

## Open questions and possible follow-ups

The full Linux desktop smoke had one approval-step failure. After Allow, the focused terminal still showed a basic fake
agent. The test timed out waiting for the spawning agent's completion marker. It passed alone on the same build. Both
Settings focus and Keyboard shortcuts focus checks passed in the full run. The evidence retains the initial failure and
the passing retry. The cause remains undetermined.

- A template can save a command that the launcher later refuses for a misplaced placeholder. The remaining product
  decision is where that refusal belongs when templates can contain only part of a launch setup.
- Anyone inspecting the helm's attachment debug logs can see a host-supplied session ID whose formatting characters may
  affect the displayed log line. There is a separate feedback item about escaping the diagnostic copy while preserving
  the session's actual ID.

## Pull requests

- [#4253](https://github.com/scode/farhelm/pull/4253/changes) corrects the terminal address reference in a session store
  test.
- [#4254](https://github.com/scode/farhelm/pull/4254/changes) explains what happens to unsaved recipe edits when you
  close Settings.
- [#4255](https://github.com/scode/farhelm/pull/4255/changes) describes when automatic tab cleanup retries a failed
  close.
- [#4256](https://github.com/scode/farhelm/pull/4256/changes) describes what the agent settings file size checks
  establish.
- [#4257](https://github.com/scode/farhelm/pull/4257/changes) explains the test for a late activity sample after
  Restart.
- [#4258](https://github.com/scode/farhelm/pull/4258/changes) clarifies which read errors can fail a session listing.
- [#4259](https://github.com/scode/farhelm/pull/4259/changes) explains when the supervisor observes a recorded exit and
  how it can fill in an exit code after Stop.
- [#4260](https://github.com/scode/farhelm/pull/4260/changes) describes the literal word checks on custom command
  launches and their placeholders.
- [#4261](https://github.com/scode/farhelm/pull/4261/changes) describes where the helm bounds and escapes diagnostic
  copies of host text.
- [#4262](https://github.com/scode/farhelm/pull/4262/changes) documents number shortcuts during a launch sequence in the
  Keyboard shortcuts dialog.
- [#4263](https://github.com/scode/farhelm/pull/4263/changes) explains Shift-typed digits in template names in the spec
  and Launch templates guide.
- [#4264](https://github.com/scode/farhelm/pull/4264/changes) records OMP's new-request and stored-launch permission
  defaults.
- [#4265](https://github.com/scode/farhelm/pull/4265/changes) corrects the comment about when OMP can save a new
  conversation.
- [#4266](https://github.com/scode/farhelm/pull/4266/changes) corrects stale desktop and terminal client comments while
  preserving the relevant behavior comparisons.
- [#4267](https://github.com/scode/farhelm/pull/4267/changes) clarifies failed-save exceptions and records the checkout
  discrepancy that [#4160](https://github.com/scode/farhelm/pull/4160/changes) already resolved.
- [#4268](https://github.com/scode/farhelm/pull/4268/changes) records the accepted behavior when saving a layout
  preference fails.
- [#4269](https://github.com/scode/farhelm/pull/4269/changes) records the ntfy save error that can arrive after Settings
  closes and when Notifications reads the saved settings again.
- [#4270](https://github.com/scode/farhelm/pull/4270/changes) documents the agreed local configuration boundary for
  three grouped outcomes.

## Checks run, reused, and skipped

- Targeted `cargo nextest run -p farhelm-gui --lib` on `d2dc70338cb8` passed all 23 selected Keyboard shortcuts,
  Settings close, and recipe draft tests (run `aafa9004-1f61-47c8-980f-91cf5ff8bcac`). This revision has both final UI
  strings.
- `python3 scripts/check-gui-goldens.py --update` passed on `d2dc70338cb8` and generated all 268 Linux reference images
  (run `f6a143c5-e0cb-4a90-821c-0dd441bc6133`). A byte comparison found only the expected light and dark Keyboard
  shortcuts changes. Both were visually checked and published through `scripts/gui-goldens.py`. The earlier capture (run
  `135e7476-588d-4400-9628-31964c4765ef`) was interrupted after 209 images and was retained.
- `bun install --frozen-lockfile` and `bun run build` built the revised Launch templates page and the other 36 pages.
  Internal link validation passed. Astro warned about its empty `i18n` collection and missing `404` content entry.
- `rustfmt --edition 2024 --check` on changed Rust files, `dprint check` on changed prose, whitespace and changelog
  checks passed. `scripts/check-test-sleeps.py` found 306 annotated delays and no missing annotations.
  `scripts/check-agents-md.py` reported zero errors and the same four pre-existing citation warnings.
- Reused `cargo clippy --locked -p farhelm-gui --lib --tests -- -D warnings` on `c9cb12cf`. The final shortcut edit
  updated one label and the corresponding expected strings. The exact-label tests above cover that edit.
- Reused the 209 passing tests from `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::/)'` on `6c8ed168` (run
  `6ee53e1c-1e67-437e-a483-7a74046df63c`). The narrower run above covers Settings close and recipe behavior with the
  final help text.
- `python3 -B scripts/gui-e2e-smoke.py` on `c9cb12cf` passed 48 steps and skipped the Mac-only clipboard step. The
  approval step failed in that run (`6b4aec6f-384d-4c32-84d1-ef7760dcb3e4`) and passed in a fresh app on the same build
  when selected alone (`e22a5742-5975-42bd-acf2-7c9a52a66e60`). The Settings and Keyboard shortcuts steps opened and
  closed the relevant dialogs and returned focus. The subsequent shortcut label edit does not change those paths.
- Runtime checks were selected for the two UI wording changes. Spec and comment corrections used static checks and
  source review. Native app and screenshot validation ran on Linux, and the shortcut table tests checked both platforms'
  labels.

## Review

All 18 PRs passed independent source review and a cold title read. The migration-comment PR also passed a scope
reassessment. Before delivery, the audit checked all 21 archived outcomes against their recorded decisions and verified
the 18 draft PRs' bases, heads, and outcome links. The stack is awaiting landing. The landing monitor will add its
verification and the merge details to this report.
