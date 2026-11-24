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
