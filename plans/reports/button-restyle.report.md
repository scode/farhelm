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

### Landing

#### What landed with it

The plan's five PRs (#4691, #4699, #4704, #4711, #4712) and one landing fix (#4720) landed alone in their round. Since
its base, main had gained only the change that lets a host be removed during an SSH sign-in (#4714, #4718), which
touched the desktop's Hosts menu, its Remove confirmation and its record of operations in flight, none of which this
plan touches or draws differently; the rebase met no conflicts. The plan's republished reference pictures already
include main's latest ones, and every picture matched on the combined stack.

The landing's independent reviewer confirmed that the new font file matches the checksum and version that
THIRD_PARTY_NOTICES.md records, with the same weight and vertical metrics as the two faces already bundled, and that its
license notice now covers all three faces.

#### Fixes made while landing

- The landing's full run of the desktop window's tests failed one launcher test the plan's focused runs did not include:
  it still required the Launch button to paint the accent outline the restyle removed. The product was right; #4720
  makes the test expect no outline, while it still checks that Launch has the primary fill beside a quiet Reset.
- Several design notes and code docs still described the toolbar from before the restyle (split buttons with no gap
  between their parts, the session header condensing only beside a tunnel pill, a font list without the Medium face).
  #4720 corrects them.

#### Checks

Everything the plan changed is in the desktop window's crate, its design notes and the bundled fonts. The plan ran 259
focused tests of that crate, so the landing ran the crate's whole suite, the lints (including those of the desktop app's
outer program, the separate crate that starts the app, manages its supervisor and embeds the window, which builds the
window's crate too), the end-to-end smoke, which clicks through the restyled buttons, and every reference picture, on
the plan stacked on main. The worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64
Linux.

On the plan as delivered, stacked on main:

- `cargo nextest run -p farhelm-gui`, on the test worker (recorder run `dfe4bc9d-83ed-4f64-a1bc-e74606681a8c`): 3346 of
  3347 passed and none printed a runtime `SKIPPED`. The failure is the launcher test the landing fix corrects; the
  failed run is kept.
- `cargo clippy --all-targets -- -D warnings` and the desktop app's two `cargo clippy -p farhelm-desktop --all-targets`
  runs (with and without `--features agent`), on the test worker: passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `ee9aabdd-01d7-4608-ae97-ed191b8648d3`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `a1f75822-3e87-48c1-8a71-94d6697bf496`): every picture
  matched its reference.
- `python3 -B scripts/test-gui-screenshot-batch.py` (a self-test of a script that starts nothing, which the validation
  guide lists without the recorder), `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`,
  `dprint check` and `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix:

- `cargo nextest run -p farhelm-gui -E 'test(/^launcher::view_tests::/)'`, locally: a first version of the corrected
  test still failed (recorder run `d1988f6a-2ad6-4d94-9662-058639cd9ddf`, kept), since it looked for the transparent
  edge as an exact painted color; the version that landed asserts that no visible outline is painted, and all 73
  launcher tests passed (recorder run `81f73767-41e2-47c2-8b75-1e66e171e63c`).
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, on the test worker, `cargo fmt --all -- --check` and
  `dprint check`, locally: passed.

Reused for the fix: the crate's other tests, the smoke and the reference pictures above, since the fix changes one test
and documentation. Skipped: the outer program's own tests, since this plan does not touch that crate; macOS, where the
bundled font renders through another text engine (the report already names this as unverified); CI, which runs only on
demand.

#### Things you should know that the report does not say

- **Stop is wider at rest, and keeping it there took a new layout rule.** Stop now always reserves the width of
  "Stopping…", so that the other actions never move when it changes label. At rest it is therefore about 30 pixels wider
  than its label needs, with more empty space around its label than Clone and Delete have. With that width, the session
  toolbar no longer fit a 650-pixel pane, so the plan added a rule for the narrowest rows: the tab strip may borrow up
  to 6 pixels of the row's side margins, and the gaps between tabs, tunnel pill and actions shrink to as little as 2
  pixels. The plan's own fallback for a 650-pixel miss was to condense earlier, then narrow the toolbar's gaps, then ask
  you; the gap narrowing matches its second step, but borrowing the margins is in none of the steps, and the report does
  not say whether condensing earlier was tried. It says there are no unresolved decisions. Worth a look at the session
  toolbar's reference picture. The alternative is to let Stop widen while it stops, as the old header did, at the cost
  of its neighbors shifting for the moment it says "Stopping…", and to drop the narrow-row rule if it is then no longer
  needed.
- A split button's caret whose choices are all unavailable no longer changes on hover, though a click still opens its
  menu, where the reasons are. The plan's reviewers chose this; giving it back a quiet hover would keep the hint that it
  is clickable.
- The selected tab in a tab strip (the launcher's Agent and Command, say) is now Medium like the others, where it was
  SemiBold, while the chosen part of a segmented control stays SemiBold, so the two kinds of control now disagree.
- The × on the terminal's drag-copy notice now sits 6 pixels below the first line of the notice's text, where it was
  level with it. No reference picture shows the notice.
- In dark mode, the primary button's new hover color gives its white label a contrast of 4.31:1, below the 4.5:1 that
  accessibility guidelines ask for at this text size; the resting color gives 4.94:1 and the old hover 5.61:1. The
  colors are the ones you approved; a slightly darker hover would fix it.
- The Medium weight also reaches controls the plan did not list, since it applies to anything drawn inside a button: the
  session list's group headings, the launcher's host picker value and the tunnel pill's text.
- The marked-sessions bar's Stop and Delete grew, so at the narrowest sidebar the bar may now wrap onto a second line.
  Nothing is cut off, and its only reference picture is at a wider sidebar.
