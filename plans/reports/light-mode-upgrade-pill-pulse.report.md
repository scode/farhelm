## What this was about

The sidebar pill for an update waiting for restart barely changed color during its light-mode reminder. It was
particularly easy to miss when the sidebar was too narrow for the arrow beside it. In light mode the pill now pulses
solid red with white version text and a white warning mark when present. The reminder arrow is bolder and slightly
larger in both themes.

## Things you should know

- The stronger light-mode hover tint uses darker red for the version and darker amber for the warning. The existing
  amber on the stronger tint would have dropped from roughly 4.7:1 to 4.25:1 contrast. The darker amber measures roughly
  5.18:1 against the new tint.
- The light-mode version text has roughly 4.74:1 contrast at rest, 5.30:1 on hover and 6.07:1 in the pulse. Tests check
  that the version and warning both reach 4.5:1. The full render changed just the two screenshots of the update-ready
  pill. Both show the held reminder frame with a warning present.

## Open questions and possible follow-ups

None.

## Pull request

- Make the sidebar update reminder easier to see. [#4747](https://github.com/scode/farhelm/pull/4747/changes)

## Checks

- `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/^(theme::|update_readout::|components::tests::button::)/)'`
  passed all 56 selected tests on `9ddd644977af` (run `1577a238-4554-4ce2-afd3-03ae7ee7b1a2`). The selection filtered
  out 3,313 tests and had no runtime skips. The candidate's source is identical to the draft except for the two
  reference IDs added after the captures were published.
- On `b806c31ca299`, `python3 scripts/check-gui-goldens.py --update --scene version-readout-update-ready -j 3` passed
  and produced the two visually inspected theme variants (run `e7d87292-e5ae-432e-a398-29ca9e4c1114`). On
  `9ddd644977af`, `python3 scripts/check-gui-goldens.py --update -j 3` passed with four renderer workers (run
  `fa4b1009-254f-45c1-999f-4d2a9becfeda`). Both final PNGs were byte-identical to the inspected images. The theme
  correction between runs concerns hover. This scene depicts rest, focus and a held pulse. An initial comparison was
  interrupted before a renderer verdict (run `3b1ffe33-b146-44c4-aa51-28fd7eefd5de`). After publishing the references,
  `python3 scripts/check-gui-goldens.py -j 3` passed in all four workers (run `38a49ec1-2d51-47a0-a4bf-84d8c1fe16da`) on
  `24b2ae6453c6`. The draft has that same tree.
- `cargo clippy --locked -j 3 -p farhelm-gui --all-targets -- -D warnings` passed on `9ddd644977af`.
  `python3 -B scripts/check-gui-lints.py` passed all 120 demonstrations, and
  `python3 -B scripts/check-crate-layering.py` found zero violations.
- `cargo fmt --all -- --check`, `dprint check -- SPEC.md SPEC_impl.md TODO.md`,
  `python3 releasing/check-changelog.py format` and `python -B scripts/check-test-sleeps.py` with the pinned parser
  environment all passed on `9ddd644977af`. Only the reference manifest changed afterward.
- The focused GUI selection covers the affected readout and button. The full workspace battery would exercise unrelated
  behavior. Native macOS rendering requires a Mac GUI session, and this local validation ran on Linux.

## Review gate

A fresh Sol review found the weaker amber contrast on light-mode hover. The fix reuses the darker warning ink and checks
the warning's contrast on every fill. All findings were resolved, and the commit wording passed a separate cold read.

### Landing

#### What landed with it

The plan's PR ([#4747](https://github.com/scode/farhelm/pull/4747/changes)) and one landing fix,
[#4750](https://github.com/scode/farhelm/pull/4750/changes), were the only plan landed this time. While the plan was
being built, main gained only the terminal hyperlink warning work (osc8-bracketed-url-warnings, in the terminal's link
model) and plan bookkeeping. Neither touches the sidebar or its colors, so nothing interacted with this plan. The rebase
met one conflict, in TODO.md, where this plan and that one each removed their own entry; both removals were kept.

#### Fixes made while landing

- **No change to anything drawn.** The plan chose the warning mark's color on hover and during the reminder by having
  the code that draws the pill ask whether the theme is dark, while every other color, including the version text beside
  it, comes from the theme's palette. #4750 gives the mark two palette colors of its own, with the same values, so each
  theme's colors are defined and tested in one place again. The version readout's screenshots still match. The fix is
  the change the landing's independent reviewer suggested after reading the plan's PR; the fix itself was not reviewed
  again, and its commit message went through a cold read.
- SPEC_impl.md had lost the sentence saying the reminder deepens the pill's tint, which left dark mode's reminder
  undescribed; it is back. SPEC.md now says hover deepens the tint in both themes (it read as light mode only) and that
  the light reminder flashes the pill between its tint and solid red. A few code comments that still described the old
  look were updated.

#### Checks

The plan changes how the desktop app draws its sidebar's update pill, so the landing ran the desktop GUI's tests and
those of the two crates built with it (the terminal link model and input mapping), clippy with warnings as errors over
the workspace and over the app binary, a full comparison of the app's screenshots with their stored reference pictures,
and the formatting, test-delay and changelog-format checks. Worker runs were on a separate arm64 Linux test worker; the
rest ran locally on x86_64 Linux. No check failed.

On the plan's PR rebased onto main, before the landing fix:

- `cargo nextest run -p farhelm-grid-model -p farhelm-gui -p farhelm-input-mapping`, on the test worker (recorder run
  `797ebb2a-19e1-46d3-a6ea-f274c665a846`): all 3456 passed. Nextest skipped four tests marked to run only on demand:
  three that compare screenshots, which run separately below, and a helper that regenerates icon images.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `80e2871d-e8cd-4870-b5f6-e1b2069620a4`): every
  screenshot matched its reference, including the two updated update-ready pictures. I looked at both: in light mode the
  reminder frame is solid red with white text and a white warning mark, and dark mode is unchanged apart from the bolder
  arrow.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4750):

- `cargo nextest run -p farhelm-gui --lib -E 'test(/^(theme::|update_readout::|components::tests::button::)/)'`, locally
  (recorder run `6ce37354-6c86-4e21-9ad2-e1e3bd1dff30`): all 56 passed, including the contrast test, which now also pins
  the warning mark's new palette colors.
- `python3 scripts/check-gui-goldens.py --scene 'version-readout-*'`, locally (recorder run
  `d511e8f4-3458-4472-a630-8fd9dd729a71`): every version readout screenshot still matched.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, the formatter, the sleep check, `dprint check` and the
  changelog format check, locally: passed.

Reused for the fix: the rest of the GUI, link model and input mapping tests, the workspace lints and the other
screenshots above, since the fix only renames where two existing colors come from inside the GUI crate. Skipped: the
rest of the workspace suite, since nothing outside the desktop GUI changed; the end-to-end desktop smoke, whose steps do
not reach the update pill; macOS, since the change is colors and a font weight in shared drawing code, and no Mac was
used (the bolder, larger arrow is the one change whose look depends on how a Mac renders text, a small cosmetic risk);
and CI, which runs only on demand.

#### Things you should know that the report does not say

- **The light-mode reminder is a flash, not a steady fill.** The report says the pill pulses, but the TODO entry and the
  screenshots (which hold one solid frame) make it look like a steady fill. In fact, during its two and a half seconds
  the pill alternates three times between the pale tint with red text and solid red with white text, about once every
  0.8 seconds. That is well under the common accessibility limit of three flashes a second, and it is what makes the
  reminder hard to miss. No screenshot or check shows the motion. Dark mode's reminder alternates the same way between
  two red tints, as before. I recommend keeping it and judging it on a devbuild before the release; if it feels too
  loud, holding the solid fill for the whole reminder is a small change.
- The light hover state, with its deeper tint and darker red and amber text, has no screenshot. Its colors are pinned by
  the contrast test, which checks every fill and ink pair at 4.5:1 or better, so I suggest no screenshot for it.

#### Possible follow-ups

None beyond judging the flash, above.
