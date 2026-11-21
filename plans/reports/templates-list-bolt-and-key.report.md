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

### Landing

#### What landed with it

hosts-small-fixes (Hosts, Add host and attention list fixes) landed just before this plan in the same round. Since the
executor last rebased this plan onto main, main had gained only the terminal's Connecting color fix and TODO entries. An
independent reviewer and I read both plans against each other; they change no file in common. The shared button style
this plan changed has one user, the Templates list, so no other button looks or behaves differently; only the Templates
list's reference pictures changed, in light and dark. The landing made no fixes of its own to this plan.

#### Checks

This round landed two plans together: hosts-small-fixes (the helm's host deletion and the desktop app's Hosts, Add host
and attention list) and templates-list-bolt-and-key (the Templates list, a shared button style and four reference
pictures). The landing ran the helm crate's and the desktop app GUI's test suites, the app's end-to-end smoke test and
the main lint (clippy) on a separate arm64 Linux test machine, and the screenshot comparisons and small checks locally
on x86_64, on both plans stacked together on main:

- `cargo nextest run -p farhelm-helm`, on the test machine (recorder run `049389f8-eae4-4d17-88ce-879c0fcf4801`): all
  1790 tests passed. The two skipped tests need a real Mac host and are always skipped in this run.
- `cargo nextest run -p farhelm-gui`, on the test machine (recorder run `c296b28a-18cb-4854-a8a3-11b58412dd4b`): all
  3227 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (recorder run `e99d643b-a6fc-4676-ada5-f59e1299049b`):
  every step passed but paste, which skips on Linux as it always does. Its steps include the Hosts attention list and
  host menus.
- `cargo clippy --all-targets -- -D warnings`, on the test machine, not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py` (the screenshot comparisons), locally (recorder run
  `149c1a19-7377-4aa8-b6a0-dd70d3d438c1`): passed, with templates-list-bolt-and-key's four re-captured Templates
  pictures.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format` (formatting, test and changelog lints), locally: passed.

After hosts-small-fixes' landing fix (#4430), which changes one hover text in the Tensorlake Add host form and three
spec sentences:

- `cargo nextest run -p farhelm-gui`, on the test machine (recorder run `db8c7155-3cde-46b1-a498-179efe457023`): all
  3227 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test machine: passed. `cargo fmt --all -- --check` and
  `dprint check`, locally: passed.

The helm, smoke and screenshot runs above still apply to the fix, since it changes no helm code and no hover text is in
a picture. Skipped: the rest of the workspace suite, since nothing outside the helm and the GUI changed; and Mac runs,
since nothing here is Mac-specific.

#### Things you should know that the report does not say

When no template is marked with the lightning bolt, the list keeps a blank band about 21 points tall at its foot. It
holds room for the key line that appears under a marked template, so that marking the first one does not resize the
dialog and move the bolt out from under the pointer. The report mentions reserving the line but not that it shows as
blank space while nothing is marked; the reviewer found it necessary rather than excessive.

#### What you should still do

Nothing.
