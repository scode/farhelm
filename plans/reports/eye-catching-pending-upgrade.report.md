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

### Landing

#### What landed with it

The plan's two PRs (#4706, #4719) and one landing fix (#4722) landed alone in their round. Since its base, main had
gained the desktop button restyle (the button-restyle plan: #4691, #4699, #4704, #4711, #4712 and #4720), which was
built at the same time and changed the same button component and the same version readout pictures. The rebase met that
in two ways. The button component conflicted in four places, each a line both sides edited (a comment naming a renamed
height function, the corner radius and focus ring choices for each kind of button, and the corner rounding of joined
buttons); the resolution keeps both sides' additions. And each plan had republished the version readout's reference
pictures from its own code alone, so neither set matched the combined app; #4722 replaces them with pictures rendered
from the combined code.

The landing's independent reviewer confirmed the conflict resolutions draw what each side drew before, that the
restyle's medium label weight reaches the pill but the pill's own bold text wins, and found no bugs in the reminder's
timers, its handling of Reduce Motion, window activation and input, its pause while the window is disconnected, or the
menu's anchor.

#### Fixes made while landing

- The combined code did not pass the lints: a line the restyle added still referred to the version readout's kind of
  button in its old form, which this plan simplified. The landing corrected that line inside #4706, the PR that made the
  change; it changes nothing the app does.
- #4722 republishes the four version readout pictures (the readout's menu, its row of outcomes, its progress rows and
  the ready pill) from the combined code, each compared by eye with both plans' own versions. The ready pill's picture
  shows the pill in four rows (plain, with a warning, focused, and with the reminder arrow) and also needed a taller
  frame: the plan added the reminder row without enlarging the picture, so the rows were cut off at top and bottom, in
  the plan's own published picture too.
- SPEC_impl.md's description of the sidebar header, the screenshot catalog's titles and the notes comparing the desktop
  app with the older one still described the old red text toggle; #4722 describes the pill and its reminder. The plan
  had updated SPEC.md only, though it said both specs would follow.

#### Checks

Every change is in the desktop window's crate and its docs. The plan ran filtered selections of that crate's tests
before the button restyle reached main, so the landing ran the crate's whole suite, the lints (after the fixes also
those of the desktop app's outer program, the separate crate that starts the app and embeds the window), the end-to-end
smoke, which draws the sidebar header, and every reference picture, on the plan stacked on main. The worker runs were on
a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux.

On the plan stacked on main, with the republished version readout pictures:

- `cargo nextest run -p farhelm-gui`, on the test worker (recorder run `ef0d6592-b42e-4427-afbc-b96db8035f00`): all 3354
  passed and none printed a runtime `SKIPPED`.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `53a40ba0-e361-4f92-a002-1ab4d80d8ad7`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py --update`, locally (recorder run `1ed31c49-440a-4aa0-890a-de7287bb5fc6`): only
  the four version readout scenes changed, which were inspected and published; a comparison against the new manifest
  (recorder run `7d1c7d92-7d94-4ed7-b781-50c5a32ffc68`) matched every picture.
- `cargo clippy --all-targets -- -D warnings` on the test worker failed on the line the landing corrected inside #4706
  (see the fixes above), which stopped that run before the outer program's lints.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fixes:

- `python3 scripts/check-gui-goldens.py --update`, locally (recorder run `13379a2a-f32d-4baf-847c-ea5f558b9bb6`): only
  the ready pill's picture changed, now with its fourth row whole; inspected and published.
- `cargo clippy --all-targets -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` runs (with and without `--features agent`), on the test
  worker: passed.
- `python3 -B scripts/test-gui-screenshot-batch.py` (a self-test of a script that starts nothing, which the validation
  guide lists without the recorder; the screenshot catalog's titles changed), `cargo fmt --all -- --check`,
  `dprint check` and `python3 scripts/check-agents-md.py`, locally: passed.

Reused for the fixes: the crate's tests and the smoke above, since the fixes change one match pattern, one picture's
frame size, the picture manifest and documentation. Skipped: the outer program's own tests, since this plan does not
touch that crate; macOS, where the reminder's timing and window activation follow the same code but the pill's look is
unchecked; CI, which runs only on demand.

#### Things you should know that the report does not say

- **In the light theme the reminder barely shows.** The pill's resting tint, its hover tint and its pulse differ by one
  to four color levels per channel there, so in the light picture the pulsing reminder row looks the same as the resting
  pill. The report's "the pill brightens" and "hover is softer than the pulse" hold only in the dark theme. Where the
  arrow is also hidden (a sidebar near its narrowest, or a status such as Checking… beside the pill), a light theme
  reminder shows almost nothing. A deeper tint would bring the red text below the usual 4.5:1 contrast, so this needs a
  visual decision rather than a landing fix.
- The reminder's arrow is a regular-weight 12-pixel → beside the bold pill, which makes it faint; drawing it bold would
  be a cheap way to make it stand out.
