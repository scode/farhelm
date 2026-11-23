# Make a pending upgrade more eye-catching

## What this was about

The desktop app showed a ready update as red version text in the sidebar. That was easy to miss in a window that stays
open for hours. The version and up-arrow now sit in a tinted red pill. When the update is ready, the pill brightens and
a small bouncing arrow points to it for about two and a half seconds.

The maintainer chose the tinted red pill and the arrow-and-pulse reminder, with a steady pill under Reduce Motion. The
arrival reminder plays as soon as the update is ready in an active window, or when the window next becomes active. The
periodic reminder comes due every 15 minutes and plays if the window is active and has received a click or keystroke in
the last five minutes. While the header is visible, activating the window can play a due reminder even if the last input
is old. A window that stays active but idle waits for a click or keystroke. Turning on Reduce Motion also stops a
reminder already playing.

## Things you should know

The arrow uses the space between the wordmark and the pill. If that space is too small, or an update status such as
Checking is beside the pill, the pill alone brightens. The pill stays in place during the animation, including its
keyboard focus ring and menu anchor. Hover has a softer tint than the pulse so the reminder can still be seen under the
pointer.

A disconnected window has no header to animate. It holds no reminder timer. It preserves a reminder that was already
due, so an arrival can play when the active header returns. A due periodic reminder can still wait for input after an
idle reconnect.

## Open questions and possible follow-ups

None identified.

## PRs

- Tinted version pill for a ready update. [#4706](https://github.com/scode/farhelm/pull/4706/changes)
- Arrival and periodic reminders, live Reduce Motion handling, and the frozen reminder screenshot.
  [#4719](https://github.com/scode/farhelm/pull/4719/changes)

## Checks run, reused and skipped

- PR 1 passed 60 tests at `a46aeac0` (run `fe73d132-23c5-4f08-bc76-dc900ae0cf16`). The pinned
  `cargo nextest run -p farhelm-gui --lib` selected the update readout, button and sidebar modules. Reviewed eight
  intentional Linux screenshot changes, four scenes in both themes. The full `python3 scripts/check-gui-goldens.py`
  comparison passed after publishing the references at `271a9a65` (run `473b7833-ba0b-4a82-b720-53ae964868fc`).
- At `f1341a94`, the pinned `cargo nextest run -p farhelm-gui --lib` passed seven focused cases for arrival, idle and
  inactive deferral, Reduce Motion, arrow clearance, reconnect and pill tint (run
  `f6ebb38d-e5ca-407a-b93d-e312cd23dc8f`). It then passed all 142 selected tests in the update readout, button, sidebar,
  focus and ticker modules (run `aca2fe58-7d40-4251-94ea-4a4dc74c0e35`).
- `python3 scripts/check-gui-goldens.py --update` captured the full Linux catalog. The only changes were the
  ready-update scene in dark and light, both reviewed before publication (run `24d39870-fce4-4264-af14-f74cf5245212`).
  The full `python3 scripts/check-gui-goldens.py` comparison then passed at `1d778262` (run
  `9dcf628d-dde5-4919-a12b-cd0e790d1231`).
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, `python3 -B scripts/check-gui-lints.py`,
  `python -B scripts/check-test-sleeps.py` in the pinned interpreter environment, `dprint check SPEC.md TODO.md` and
  `python3 releasing/check-changelog.py format` passed.
- Reused both PRs' targeted tests, screenshot comparisons and Rust lint results after the final rebase. Main changed SSH
  host removal and separate queue and TODO entries. The stack's changed GUI source and screenshot references stayed
  byte-identical. Reran the changed Markdown check on the rebased stack.
- Skipped the full workspace test suite because the selected tests cover the changed GUI paths. This run used Linux,
  with a headless test of the compact Mac layout using the native control inset. Native macOS rendering and preference
  delivery require a Mac run.

## Review gate

The independent source reviewer found that a disconnected header kept its timer, the hover tint could mask the pulse,
and the compact layout test bypassed the real header. The final changes stop the hidden timer and retain reminders
already due, keep the two tints distinct, and test the measured gap on the production layout. Both PRs passed source and
wording review.
