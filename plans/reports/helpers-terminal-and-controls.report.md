# Shared terminal input and control helpers

## What this was about

Farhelm kept separate copies of several desktop rules. A later change to one copy could make similar controls behave
differently or tell users to press a key that the terminal handles another way. The plan shares the existing rules for
terminal input, keyboard focus, local validation messages and animations. It also consolidates the test input steps. The
changes preserve the existing behavior and appearance.

## Things you should know

Hold Option on macOS or Shift on Linux to select text in Farhelm when a program in the terminal handles the mouse. The
drag-copy notice now gets its key name from the rule the gesture uses. A keypad digit still cancels an active template
sequence and reaches its original destination. The terminal still uses the key's logical value to decide whether to send
a keypad code.

The keyboard ring around the sidebar's resize grip uses the same two-pixel border as the other controls the plan named.
Each control chooses when to draw its ring and how the ring fits. The feedback form also shares the existing rule that
uppercases an initial ASCII letter in a local validation message. The shared focus check uses the latest drawn frame.
The pending move into a newly opened dialog still waits for the dialog's first frame, and the window must still exist
for that move.

The trash can and progress spinners keep their own animation phases, including when Reduce Motion pauses them. Closing a
Trash confirmation first dismisses the question and leaves the dialog open. When the original Delete all button is still
present, focus goes back to it. When a host has disappeared, the dialog handles focus as it does for other disappearing
controls. The shared key-event test helpers preserve each test's choice of when to check the effect of a press or
release.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4595](https://github.com/scode/farhelm/pull/4595/changes) shares terminal modifier rules.
- [#4596](https://github.com/scode/farhelm/pull/4596/changes) shares keypad digit recognition.
- [#4599](https://github.com/scode/farhelm/pull/4599/changes) shares the keyboard focus ring.
- [#4602](https://github.com/scode/farhelm/pull/4602/changes) shares capitalization for form errors.
- [#4604](https://github.com/scode/farhelm/pull/4604/changes) shares the check for drawn focus targets.
- [#4606](https://github.com/scode/farhelm/pull/4606/changes) shares animation ticks and Trash confirmation focus.
- [#4613](https://github.com/scode/farhelm/pull/4613/changes) shares key event test helpers and clears this plan's TODO
  entries.

## Checks run, reused and skipped

- On rebased revision `4317481cac6b`, reran `cargo nextest run -p farhelm-gui --lib` through the recorder with the
  launch, recents, Restart with, search, checkout and environment view modules selected. All 161 passed with no runtime
  skips. Run `211af8d5-8a5c-4965-afe8-c7f67c9de01a`.
- On the same revision, reran `python3 scripts/check-gui-goldens.py` through the recorder in compare mode. All four
  render jobs matched the full Linux catalog, including the new managed checkout references. Run
  `6aed79e1-ea03-41ca-8370-9467fb56c4b3`.
- The delivered tip `ce90751828cd` has the same files and changes as that revision, on main `db1aa0408209`. Reused the
  pre-rebase `cargo nextest run -p farhelm-input-mapping -p farhelm-gui --lib` selection on `fe3a422b9823`. It ran the
  input-mapping package and the affected GUI modules, with all 819 passing and no runtime skips, including both new
  key-helper self-tests. Run `41fe6d52-59ac-4bce-b1d8-0cecaf11649b`. Each draft's diff stayed the same after rebasing.
  Main's compact layout and live preview announcements affect the checkout view, covered by the launcher rerun and the
  new screenshot comparison.
- Earlier focused nextest runs through the recorder covered the attached terminal (180 cases,
  `dc38d402-fc94-4c22-9deb-d071a9de3dc5`), keypad and template sequence (39, `c5a080bf-1537-4302-8ebc-fbb67e56460d`),
  components (155, `0c90d6e1-67ba-4d88-9658-72c280da35e3`), forms (127, `5bb4397a-bc67-45f9-9509-7bb7bec035cc`), focus
  and sign-in (69, `fb2f1f65-deb6-41a4-b576-ac4f35afae51`) and Trash and spinners (81,
  `9655fc4f-5f01-434b-b368-a310017ff182`). Earlier Linux comparisons matched after the modifier, focus-ring,
  form-wording and animation drafts in runs `05f90cf4-0772-457a-b098-4051ff866855`,
  `41c1f368-a4d3-4f56-a50c-892a1185274b`, `09e8a2ef-7c8d-4712-a46c-85ad297426d5` and
  `207428d4-1b06-4508-9fcb-a0c8e8739822`. The rebased checks above cover the new layout and references.
- All recorded nextest runs used four slots with zero retries.
  `cargo fmt -p farhelm-gui -p farhelm-input-mapping -- --check`, `dprint check TODO.md` and `git diff --check` passed
  again after the rebase. `python -B scripts/check-test-sleeps.py` inspected 309 delays with zero missing reasons. The
  delay result still applies after the helper parameter type correction and the rebase because the tests' delays stayed
  the same.
- Skipped the full workspace suite because the selected tests cover the changed paths. Tests exercise the macOS and
  Linux key rules with simulated inputs. The visual checks ran on Linux.

## Review gate

Passed with no open findings. Each draft received an independent source review and a cold wording check. Review
clarified which modifier bits mouse reports carry, found a template row that needed the common capitalization rule, and
preserved Trash dismissal when the host button has disappeared. The final draft adds the matching launcher and shortcut
test callers found during review. Two harness self-tests verify the invariant check after a single key press or release.
