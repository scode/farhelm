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

### Landing

#### What landed with it

It landed alone in its round. Since its base, main gained exited-session-no-terminal (a recovery card when a session's
terminal is gone), gui-ux-desktop-fixes (keyboard, contrast and refusal-wording fixes in the desktop app) and
gui-ux-backend-fixes (fixes in the helm and the supervisor from the same triage). The independent reviewer checked every
sentence this plan corrects against what those plans changed, and none of them made a corrected sentence false again or
contradicts it. The triage ledger's entries from those plans and this one were kept side by side; each of this plan's 21
outcomes now appears once in the archive, with its feedback file and index line removed.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The reviewer checked every
sentence the plan adds or changes, and each is true of the code. The only changes a user sees are the Settings Close
tooltip and one new row in the keyboard shortcuts overview, with its light and dark reference screenshots; the rest is
docs, specs and comments. Six texts were true but read badly or dropped something still true, and I reworded them below.

#### Fixes made while landing

- **The Settings Close tooltip** read "Settings may ask about unsaved recipe edits before closing. Closing discards
  other unsaved fields." It now reads "Close: asks first about unsaved recipe edits and discards other unsaved fields",
  in the "Close: …" form every other dialog's close button uses.
- **The new keyboard shortcuts row**, for Cmd or Ctrl plus a number key during a launch sequence, read "In a sequence:
  queue any unfinished word, then the numbered template". It now reads "In a sequence: add a numbered template after any
  unfinished word", phrased like the rows beside it. I published the re-rendered keyboard shortcuts screenshots and
  looked at both.
- **The Launch templates docs page** described bare number keys during a sequence starting from an edge case ("When a
  number key pressed on its own types a character…"). It now says: inside a word, a number key adds its character; with
  no word started, it queues that key's template; and on keyboards where Shift types digits, Shift and a number key can
  start a name with a digit.
- **A SPEC.md sentence** said "An update can grow a file past that limit", which reads like updating Farhelm. It now
  says that applying agent credentials to a sandbox host can, with its own write, push an agent settings file that is
  just under the 1 MiB limit over it.
- **Two comments.** One about the sidebar's version had lost a caveat that is still true (the desktop app never learns
  the helm's build, so the sidebar always shows the app's own version); it is back. One about the supervisor's live
  status had lost its point (a wrong guess between running, waiting and idle never moves a session between live and
  exited); it says so again.

#### Things you should know that the report does not say

- The ⚡ chord also adds any unfinished word before its template during a sequence, as the number chords do, but its row
  in the keyboard shortcuts overview still says only "add the ⚡ template"; the Keyboard shortcuts docs page and SPEC.md
  say both do. Possible follow-up: the same "after any unfinished word" wording on that row.
- The report's checks ran on the earlier wording of the Close tooltip and the shortcuts row, and its published shortcuts
  screenshots were replaced; the checks below cover the wording that landed.
- The report's open question, the smoke's approval step failing once, is unchanged: the step passed in the landing's
  smoke run, but one more pass does not explain the earlier failure.

#### Checks

The plan changes comments in the supervisor and the desktop app's GUI crate, two strings the app shows, SPEC.md,
SPEC_impl.md and one docs page. The landing ran the lints, the GUI crate's whole suite, the app's end-to-end smoke, the
reference screenshots (goldens) and the website build. Builds ran locally; the GUI suite and the smoke ran on boat
sandboxes (cloud VMs), from binaries built locally.

On the plan as delivered, stacked on main with the triage ledger's entries kept side by side:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy --locked -p farhelm-gui --all-targets --features agent -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui`, from a locally built archive, split in three across boats: 1040, 1099 and 1035
  passed, 3174 in all, with no runtime skips (`d0a22440-648b-4d76-8c0f-97bd9ec72b20`,
  `c9f91454-8b79-4e12-ac2b-edf90d91741d`, `4c309b8d-f57c-4730-8804-1ce37ad1e3be`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, and the clipboard
  paste step was skipped, as always on Linux (`51917e62-5b89-4471-bb24-f60562dc6cfc`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`f09025bf-cbe5-42b3-8de0-31cdffc05876`).
- The docs website build (`bun install --frozen-lockfile && bun run build` in `website/`): passed.

On the fixed tree (the six rewordings above):

- `cargo fmt --all -- --check`, `dprint check` (after rewrapping the reworded SPEC.md sentence) and
  `cargo clippy --all-targets -- -D warnings`: passed.
- The Settings and shortcuts tests,
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^settings::/) | test(/^shortcuts::/)'`, locally: 217 passed
  (`58784106-7639-40e6-b12c-3d268c9b4461`).
- The docs website build: passed.
- `python3 scripts/check-gui-goldens.py --update`, locally (`3403df1d-9114-461d-ace4-e24b1ee0cec2`): every capture but
  the two keyboard shortcuts pictures matched its reference byte for byte. I published those two, and the goldens then
  passed against the published references (`65e245e0-fa33-472a-a810-66a32e69acb2`).

The GUI suite on the boats and the smoke were reused for the fixed tree: after them only wording changed, and the
focused tests and the goldens cover the two strings the app shows.

Skipped:

- The rest of the Rust workspace: outside the GUI crate the plan changes only comments.
- The Mac-only checks: no code that only builds for the Mac changed.
