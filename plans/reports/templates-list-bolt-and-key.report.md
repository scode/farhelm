# Align quick launch and number keys in the Templates list

## What this was about

The lightning bolt sat above the center of each template row. The placeholder for an unassigned number shortcut looked
clipped and pushed some template names farther right. The maintainer picked a fixed-width key cap and asked to center
both controls on the template name and summary.

Each cap now reserves the same width. A dashed plus button opens the number menu for an unassigned template. The cap
shows one digit on Linux and Cmd plus the digit on macOS. Hover help and the number menu still spell out the full key.

## Things you should know

Clicking the lightning bolt chooses the template launched by Cmd+P on macOS or Ctrl+Shift+P on Linux. That key appears
below the chosen template's summary. The cap opens a menu to assign a separate number shortcut. Reserving one line in
the list keeps the controls in place when the first bolt is selected. If you move the bolt selection from a row above,
the earlier row loses its key line and the rows below it move up.

## Open questions and possible follow-ups

None.

## Pull requests

- The Templates row alignment and shortcut cap fix. [#4424](https://github.com/scode/farhelm/pull/4424/changes)

## Checks

- All 333 selected Linux GUI tests passed with
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^templates::/) | test(/^components::tests::button::/)'`. Recorded
  run `990b61e1-202d-4d82-9452-6995b8482a5d` used `14083acb` plus the final layout. The tests pinned the digit label,
  fixed cap width, menu anchoring, and first-mark position in a long list.
- The complete Linux screenshot capture passed with `python3 scripts/check-gui-goldens.py --update` on the same source
  in run `f7129b16-c94a-4134-8778-f351e3be4999`. I visually checked all eight changed images across the list, editor,
  number menu and leave prompt in both appearances. The other 292 images matched the previous manifest, including the
  separate template launcher. After publishing and rebasing, all 300 merged references fetched, and the eight updated
  images matched the inspected captures byte for byte.
- I reused those runs on `d8d9e539` because the rebase kept the tested Rust diff byte for byte. Main changed recent
  launcher rows, Restart explanations and the feedback dialog, and its four updated feedback references were preserved.
- Rust and Markdown formatting, GUI lints, test-delay and changelog checks passed. I skipped native macOS rendering
  because the validation environment and the screenshot references are Linux. HarfBuzz measured 12 pixels for the Mac
  label in the bundled font, within the 18-pixel content width. I skipped the full workspace suite because the changes
  touch the Templates rows and their existing buttons.

## Review

Independent source review caught a stale test label and a layout shift in longer lists. The revised test covers the cap
label and the longer-list geometry. The reviewer verified the corrections. The commit wording passed a cold read.
