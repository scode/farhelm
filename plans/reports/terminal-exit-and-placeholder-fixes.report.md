# Terminal scrollback after exit and Connecting contrast

## What this was about

On a full terminal pane, the exit notice can push the oldest saved output or the missing-output notice above the visible
area. Scrolling all the way back still leaves that line hidden. While opening a terminal in the light appearance, the
Connecting text was almost as dark as the pane behind it.

The Connecting label now uses the dark palette’s dim foreground in both appearances. The pane has a fixed dark
background while it waits for its first screen. The light-appearance contrast rises from 1.6:1 to 7.9:1.

## Things you should know

The scrollback issue moves to Near term in TODO.md. The maintainer set a triage limit of roughly 100 changed lines for
each fix. The prototype that exposed the oldest row and kept the exit notice below the text already changed 137 lines of
Rust, including tests. Select All still needed work to include that history when an agent exited in the terminal's
alternate screen, the separate full-screen view some agents use. The clipping remains in the app.

## Open questions and possible follow-ups

The TODO tracks the scrollback fix, including the missing-output notice, exits on the alternate screen, drag selection
and Select All. The exit notice needs to stay below the text while every retained row stays reachable. The archived
triage outcome retains the request for a scrolled-back regression test.

## Pull requests

- Track the clipped scrollback in Near term. [#4414](https://github.com/scode/farhelm/pull/4414/changes)
- Make Connecting readable in light appearance. [#4415](https://github.com/scode/farhelm/pull/4415/changes)

## Checks

- The Linux GPUI screenshot suite compared all 300 captures successfully with `python3 -B scripts/check-gui-goldens.py`
  in recorder run `bce4898d-b24c-43ce-a3a6-d9cb188cbe06` on `0a46161e` plus the Connecting patch. I reused the run after
  restacking. The intervening container and snapshot changes are test-only and the desktop source and screenshot inputs
  stayed byte-for-byte the same. The existing scenes show settled terminals and do not capture Connecting.
- `cargo test --locked -p farhelm-gui --lib terminal::attached::tests::the_stream_opens_at_the_fitted_size -- --exact`
  passed on Linux in recorder run `2bccdba7-253d-4459-bdd0-152c82f815ca` at `88808176`. It checks that Connecting
  appears before terminal output. The color contrast follows from the palette values and was checked in source review.
- For the scrollback investigation,
  `cargo test --locked -p farhelm-gui --lib terminal::attached::tests::an_exited_terminal_reaches_the_start_of_its_history -- --exact`
  reproduced the clipping at `5885c857` with the exploratory test in run `64515f11-6fa7-4672-bfca-3c7ed7966b56`. Four
  focused tests passed at the same base plus the prototype in run `ba5361d5-07c8-4352-acbc-d8ca28427500`. Both runs were
  exploratory. The scrollback draft changes Markdown only.

  ```sh
  cargo test --locked -p farhelm-gui --lib -- --exact --test-threads=1 \
    terminal::attached::tests::an_exited_terminal_reaches_the_start_of_its_history \
    terminal::attached::tests::a_live_exit_shows_its_line \
    terminal::attached::tests::the_pointer_names_the_cell_drawn_under_it_on_a_lifted_screen \
    terminal::attached::tests::the_wheel_goes_to_the_program_on_the_alternate_screen
  ```

- `cargo fmt --all -- --check`,
  `dprint check TODO.md TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md`,
  `python3 releasing/check-changelog.py format`, `git diff --check 34b2a0132 88808176`, and
  `python3 scripts/check-agents-md.py` passed. The instruction checker reported one existing archive reference to a
  renamed test-authoring heading. `cargo clippy --locked -p farhelm-gui --lib -- -D warnings` passed at `88808176`.
- Skipped the workspace and end-to-end suites. The focused checks cover the attachment placeholder and existing desktop
  rendering. The scrollback follow-up has no product code to exercise.

## Review

Both drafts passed independent source review. The scrollback review found the alternate-screen Select All gap in the
prototype. The final review of that draft confirmed the TODO and archive carry the full scope. The Connecting review
confirmed that the label draws on the default terminal background and that the existing screenshot scenes settle after
Connecting has disappeared. Both final commit messages passed cold reads.

### Landing

#### What landed with it

Nothing else landed in this round. Since this plan's work started, main gained the gui-launcher-feedback-fixes plan
(launcher, Restart wording and feedback dialog fixes) and planning changes to TODO.md. An independent reviewer and I
read this plan against them; none touches the terminal pane or the exit notice. The landing made no fixes of its own to
this plan.

#### Checks

The plan changes one line of the desktop app's terminal pane (the color of its "Connecting" text) and adds a TODO.md
entry, plus a changelog fragment and the triage records. The landing ran the desktop app's GUI test suite and the main
lint (clippy) on a separate arm64 Linux test machine, and the screenshot comparisons and small checks locally on x86_64,
on the plan stacked on main:

- `cargo nextest run -p farhelm-gui`, on the test machine (recorder run `50eedd58-1508-486f-8e63-a85d76a9eb2f`): all
  3226 tests passed. One of them checks that "Connecting" appears; none checks its color.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (recorder run
  `21e329f2-7741-4dd9-a96b-d3caf0d4b67b`): passed. No reference picture shows the Connecting pane, so they do not cover
  the new color.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

Skipped: the workspace suite and the end-to-end smoke test, since only one color in the GUI changed; and Mac runs, since
nothing here is Mac-specific.

#### Things you should know that the report does not say

- Deferring the scrollback fix is what your triage decision allowed: "if any of these spiral into something very
  complicated, the executor should make a near term todo item to follow up on it instead of adding tons of complexity",
  with a size limit of about 100 changed lines, which the prototype passed at 137 before it was finished. The new Near
  term entry says what is wrong, what done means and the edge cases.
- The Connecting text's contrast on the pane's dark background works out at 7.9:1, up from 1.6:1 in the light appearance
  and unchanged in the dark one. That figure is computed from the two colors; no test or screenshot shows the new color
  on screen.
- The color is set by naming the dark palette directly, the one place outside tests where a view does that instead of
  asking for the current palette. The plan asked for exactly this, and it matches the pane, which stays dark in both
  appearances until its first screen arrives.
- The new TODO entry does not say where the details live: the code locations and the request for a test that scrolls
  back after an exit are only in the archived triage outcome (`exit-notice-hides-oldest-scrollback-row.md` in
  `TRIAGE_OUTCOMES_ARCHIVE.md`).

#### What you should still do

Nothing is required. Whoever picks up the scrollback TODO should read that archived triage outcome first; adding a
one-line pointer to it in the TODO entry would make that hard to miss.
