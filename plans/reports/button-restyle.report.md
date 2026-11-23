# Desktop button restyle

## What this was about

Many desktop actions had outlined buttons, and the session toolbar used compact controls even in wide panes. The
maintainer wanted a softer look and more space in the toolbar. The desktop now uses softly rounded corners, tinted fills
and one-pixel joins for adjacent parts of the same control. It keeps Inter and bundles its Medium face for button
labels. Fields, the Settings switch, the segmented-control container and deliberately outlined actions retain their
outlines.

## Things you should know

- The ordinary session toolbar and the marked-sessions actions share 26-pixel buttons, roomier icons and Medium labels.
  Narrow layouts condense. At 650 pixels the actions and both tunnel ports stay whole, and the tab strip scrolls to
  reveal the focused control. The longer “Stopping…” label has room to appear without shifting adjacent actions. A
  frozen environment gives that space back to the tabs while Stop is unavailable.
- In the 2× Linux references the Restart, Clone and Delete capitals sit at their button fill centers in both themes. The
  toolbar icons are vertically centered, with the trash icon half a device pixel below center. The live Stop icon and
  label group was one device pixel from horizontal center in the measured outcome scene.
- The terminal's drag-copy notice has a larger dismiss target. Closing it leaves focus in the terminal and sends no
  input to the running program.
- The font comes from the static Inter Medium TTF in the Inter 4.1 release archive. Its version matches the bundled
  faces. The last draft removes the completed TODO, and one changelog fragment covers the restyle.

## Open questions and possible follow-ups

The docs website screenshots and README hero will show the previous look until their own refreshes. There are no
unresolved decisions for this stack.

## PRs

- [#4691](https://github.com/scode/farhelm/pull/4691/changes) Bundle Inter Medium.
- [#4699](https://github.com/scode/farhelm/pull/4699/changes) Capture the original session toolbar and button states.
- [#4704](https://github.com/scode/farhelm/pull/4704/changes) Restyle buttons, joins and choice controls.
- [#4711](https://github.com/scode/farhelm/pull/4711/changes) Give the session toolbar larger buttons and preserve the
  narrow layout.
- [#4712](https://github.com/scode/farhelm/pull/4712/changes) Restyle the drag-copy notice dismiss control and remove
  the finished TODO.

## Checks

- On Linux at `af8b3c0a`, 259 focused GUI tests passed (`92e3caa3-203a-4a41-bef2-6906c71a41a8`), including 650-pixel
  fit, keyboard reach, Stop progress, drag-copy dismissal and the rebased reduced-motion and remembered-tab changes.
- The 47 scene-catalog tests (`9686e33c-e201-4ab2-a10c-a882fe20583c`), full 414-picture Linux comparison
  (`8ae8fe15-c9e0-431f-99ce-18cb790ef4ad`) and strict GUI Clippy passed at that revision. Formatting, whitespace, 310
  test-delay rationale checks and changelog format checks passed during development.
- Main updated the recipe editor and covered terminal link and added a stale launcher choice scene. The rebase
  regenerated the conflicted references around those changes. The Inter Medium update
  (`3d9ea2bf-f28d-434d-bdef-0737abdb1589`) and button styling update (`0518f44d-8288-4aab-b080-eadc1f57492b`) reproduced
  the previously reviewed pictures outside those scenes. Compared both themes for every newly differing scene, then
  published the updated reference images.
- Draft tip `00ca1ca7` has the same code, tests and reference files as `af8b3c0a`. Its final rebase adds two unrelated
  TODO entries from main. The maintainer opted for Linux rendering, so macOS and packaged-font rendering remain
  unverified. The executor ran targeted checks, with the workspace suite left to landing.

<details>
<summary>Final recorded validation commands</summary>

The runtime commands ran through the test recorder. Nextest used four slots and zero retries.

```sh
cargo nextest run -p farhelm-gui --lib -E 'test(/^(components::tests::(button|checkbox|radio_group|tab_strip|transition)|session_pane::(header_tests|header::fit)|session_list::(bulk_tests|marking_tests)|terminal::(text_size|set::tests)|ticker_tests)::/) | test(/^terminal::attached::tests::.*(notice|drag|clipboard_write)/)'
python3 -B scripts/test-gui-screenshot-batch.py
python3 scripts/check-gui-goldens.py
cargo clippy -p farhelm-gui --all-targets -- -D warnings
```

</details>

## Review

Each draft passed its independent source review. Review corrected button weights in menus, muted caret tint, toolbar
fixture assertions, Stop's centering and tab visibility in a crowded tunnel row. The first fit run found two failures
(`9284d98d-ba62-4d3d-ad87-da70ccac42dd`). The corrected toolbar passed all 144 focused cases
(`701dc00b-d067-466f-87b3-ac17ed29bdac`), and the post-rebase selection repeated that coverage. The drag notice review
reported no findings.
