### What this was about

When a visible host first needs an SSH answer or reports a setup failure, an amber light travels from the center of the
Farhelm window and flashes its row. Farhelm read the desktop's Reduce Motion setting at launch. On Linux, the GUI
toolkit followed the desktop portal while the app was open. If those readings differed, Farhelm could start a moving
attention cue that the toolkit skipped to its transparent last frame. The user saw no cue at all. The plan called for
one live setting and for clocks and deadlines to keep running when animations pause.

### Things you should know

- On Linux, Farhelm follows changes from the desktop portal. On macOS, the app refreshes the setting when its window
  regains focus, before any queued attention cue starts.
- An attention cue that is already flying becomes a stationary flash through its original expiry. That cue stays put if
  motion is enabled again. The launcher banner and quick-launch pill keep their original animation start time so a
  completed entrance cannot play again after a preference change.
- With reduced motion on, the terminal cursor stays visible, status pulses steady, and spinners pause. The trash icon
  loses its lift or tilt from an animation. The icon still shows when space is being freed, and ages, counters, and
  one-shot deadlines keep running.

### Open questions and possible follow-ups

An optional native macOS check is to switch away from Farhelm, change Reduce Motion while an attention cue is waiting,
and return to check that the cue uses the new setting. The activation order was verified in source, and the open-window
toggle tests ran on Linux.

### PRs

- Adds the GUI toolkit's live preference observer. [#4681](https://github.com/scode/farhelm/pull/4681/changes)
- Makes the desktop app follow that preference and covers changes in an open window.
  [#4687](https://github.com/scode/farhelm/pull/4687/changes)

### Checks run, reused and skipped

- Passed 93 focused GUI tests and 723 terminal and session-list tests with pinned nextest on Linux. The runs cover
  changing the preference in an open window, keeping both clocks and deadlines live, drawing the stationary attention
  flash, clearing the trash animation, and restoring motion without replaying a completed entrance. Runs
  `d9a6d774-df6b-4d8a-a0bf-771d432b0432` and `f214578a-cbc6-4c45-b32c-bb1ef8124048` cover source `15b7b93c`. The later
  change to a screenshot fixture does not enter these tests.
- The first run exposed a session-list test that counted the preference-change redraw as animation, run
  `85a588bb-3658-48a3-8f05-ce6662cfbf50`. The test now checks that redraw before measuring a quiet second.
- Ran `python3 scripts/check-gui-goldens.py`. In the first full comparison, `1af1a337-1298-4db6-9f1b-05fad04437f4`, two
  Delete screenshots showed ordinary session rows because a redraw could stage them before the fake helm had connected.
  The scene now waits for the live fixture. The focused light and dark comparison passed in
  `47b2dc5c-67e7-4ca6-85e4-2fb72296bb4e`, then the full Linux golden catalog matched its references in
  `4e6d9d9e-f9ef-469f-9aef-8cc0dd4e24e9`.
- Recreated the vendored GPUI tree from its checked archive and replayed its patches, matching the checked-in source.
  Reused six standalone GPUI test results from `7c0c3eab-813c-406c-a13c-6ea4bb6408a8` at `45bab938`. The later observer
  change was a documentation comment and the rebase kept the patch intact. The vendored terminal suites also passed,
  with 62, 166 and 3 tests in `91c64b48-c4eb-4550-a15e-1904b9f37035`, `220b478c-af43-45b7-a4f5-6dc55e348e05` and
  `09fc77f8-7857-480a-8772-f72898f6a822`.
- Passed `cargo check --locked -p farhelm-desktop`, strict Clippy for the workspace defaults and separately for the
  desktop crate, Rust and Markdown formatting, changelog format, and the test-delay checker. The instruction checker
  passed with one existing archived-heading warning. The desktop checks preceded the screenshot fixture change and still
  cover the desktop binary.
- Native macOS runtime coverage requires a Mac. The Linux runs targeted the affected desktop behavior and vendor
  patches. The full Rust workspace battery and CI would add unrelated coverage.

### Review gate

The requested independent source reviewer checked both PRs. The observer review clarified the callback contract and
registered its new test in the recorded suite. The desktop review caught replay of a finished transition when motion was
restored. Its retained animation state fixes that, and the reviewer verified tests for the visible cue, immediate timer
cancellation, session-list redraw, and screenshot fixture readiness. The final source-review disposition is pass for
both drafts. Both PR descriptions passed a separate cold read.
