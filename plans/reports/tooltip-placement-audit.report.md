### What this was about

Hovering Announcements in the Help menu could hide Keyboard shortcuts or Release notes. Other lists and dialogs had
similar overlaps. Help for the sidebar divider and terminal scrollbar could appear near a window edge far from the
pointer. The maintainer chose positions for ten kinds of controls using mockups based on actual Farhelm layouts. This
stack implements those choices and records them in the tooltip standard.

### Things you should know

Menu help tries the right and then the left, with a notch pointing to the highlighted choice. Open submenus retain the
space they need. The host rail uses its own edge, and groups, attention rows and host rows use the sidebar edge.
Session-row help continues to use the sidebar edge. Toolbar and dialog button help prefers below, which keeps the prompt
or explanatory text visible. Lists use their right edge, with ordinary placement as a fallback. Where an open menu would
still be covered, the help waits for room.

The audit placed Recent setups, browsed folders, queued approvals and Settings navigation with the other lists. Approval
answers and actions beneath session or connection status messages follow the dialog-footer rule. Help for Close in
Keyboard shortcuts follows the same placement as other dialog-header Close buttons. Small inline help uses ordinary
above-first placement. In launcher suggestions and the quick switcher, arrow keys move the selection while the field or
picker button holds keyboard focus.

Help for the divider, terminal scrollbar and approval scroll boxes uses pointer height on hover and the middle of the
visible part on keyboard focus. A notch points to the control. The divider and scroll box already have a keyboard focus
outline, and another full-height line beside the terminal scrollbar would compete with the text.

### Open questions and possible follow-ups

None.

### PRs

- Menus and dropdowns, including the Announcements case. [#4285](https://github.com/scode/farhelm/pull/4285/changes)
- Sidebar groups, attention rows, Hosts and the host rail. [#4288](https://github.com/scode/farhelm/pull/4288/changes)
- Below-first help for toolbar controls and dialog buttons. [#4289](https://github.com/scode/farhelm/pull/4289/changes)
- Help beside launcher, dialog and Settings lists. [#4290](https://github.com/scode/farhelm/pull/4290/changes)
- Pointer-height help for tall controls and the final placement audit.
  [#4292](https://github.com/scode/farhelm/pull/4292/changes)

### Checks

- On rebased source `1d85799d06d5`, recorded
  `cargo nextest run --locked -p farhelm-gui --lib -E <focused GUI selection>` passed 554 tests
  (82580830-55ef-4a58-a2d9-e54490ad42b4). The selection covered tall pointer and keyboard placement, keeping menus
  readable, and the interactions with the landed text-field editing and template truncation changes in the launcher,
  quick switcher and Templates.
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, `rustfmt --check` for changed Rust files,
  `dprint check` for changed Markdown, `git diff --check`, `python3 releasing/check-changelog.py format`, and
  `python -B scripts/check-test-sleeps.py` also passed.
- Ran recorded `python3 scripts/check-gui-goldens.py --update` and inspected the Linux additions in both themes for each
  PR. Final run 77ac780f-d109-49bd-9aa1-d381a95ef226 on `1d85799d06d5` captured all 278 images. The 276 existing
  references, including the updated Settings and Templates references from main, matched byte-for-byte. The two new
  images show approval scroll-box help beside its lower rows.
- Reused four earlier recorded `cargo nextest run -p farhelm-gui --lib -E <focused GUI selection>` results. Menu help
  passed 164 tests on `d26375d07326` (1671362f-9ea8-4824-9db2-2bf305795a2d). Sidebar and rail passed 381 on
  `61a56d8d981d` (674dadff-cf10-4cb4-96a5-59c6d451442b). Toolbar and dialogs passed 130 on `054937334bc6`
  (c69b4262-5bdb-4542-8783-6a28b2c7f26c). List help passed 401 on `eb58fffa1d96` (74b8ddac-c57c-43d4-802d-d71e4e6cf2cd).
  The rebase replayed the same placement changes and the final run repeated the areas that interacted with main, so the
  earlier passes still cover their remaining surfaces.
- The screenshot references are Linux. macOS rendering was outside this local run. The backend, installer and release
  batteries were outside the changed GUI behavior. The documentation screenshot that shows terminal download help uses a
  separate display path and required no recapture.

### Review gate

All five PRs passed independent review after corrections. The fixes leave open menus readable, keep the ring visible on
selected controls and follow their corners, and recenter a visible scroll-box tooltip when keyboard focus takes over.
The focused tests cover those cases.

### Landing

#### What landed with it

It landed alone in its round. Since its base, main gained stop-agent-podman-containers (supervisor cleanup of Podman
containers) and the scripts for running slow tests on a test machine of your own; neither touches the desktop app. The
independent reviewer also checked the earlier template-summary ellipsis and Emacs text-field keys against this plan's
Templates dialog and text-field changes, and found no interaction.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. I looked at the new reference
screenshots: help for a quick switcher row now sits beside the list instead of over the rows, and help for a Help menu
item sits beside the menu.

#### Fixes made while landing

- **Help for waiting approval requests stopped avoiding the list once it scrolled.** The list's help edge was drawn
  inside the list that scrolls, and the app moves a scrolling list's contents, the edge included, by the scroll
  distance. With four or more waiting requests scrolled to the bottom, a header's help fell back to covering the header
  above it, the overlap this plan removes elsewhere. The edge now sits on a wrapper that does not scroll.
- **Three descriptions of "beside" placement were stale.** The GPUI foundation notes, the sidebar's and the component
  layer's comments still said only the sidebar draws an edge, once per window, and that help without one shows at the
  pointer. They now say that lists and dialogs draw their own edges. Help sits against the nearest edge that contains
  its control, and takes ordinary placement when there is none.

#### Checks

The plan changes only the desktop app's GUI crate (with new reference screenshots). So the landing ran the GUI crate's
whole test suite, the app's end-to-end smoke, both GUI clippy runs and the reference screenshots (goldens). The suite,
the smoke and clippy ran on the maintainer's own test machine, an arm64 Linux VM that builds and tests the tree itself.
The goldens and the small checks ran locally on x86_64.

On the plan as delivered, rebased on main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop -E 'package(farhelm-gui)'`, on the test machine
  (`c613c98e-640b-4ae1-a2fb-a4ce44729915`): all 3193 passed, with no runtime skips.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on the test machine: 49 of its 50 steps passed, and the
  clipboard paste step was skipped, as always on Linux (`0624e572-a629-47b4-a9fc-ca106a8613a5`).
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings` and the same with `--features agent`, on the test
  machine: passed.
- `python3 scripts/check-gui-goldens.py`, locally: passed with the plan's new references
  (`686a9c1f-cc2c-4a73-b205-8e8441abe284`).

On the fixed tree:

- `cargo fmt --all -- --check`, `dprint check` and `python3 scripts/check-agents-md.py`, locally: passed.
- The GUI crate's whole suite again, on the test machine (`97598ef7-5e48-4ef8-86f3-0fcb43e3adc1`): all 3193 passed.
- `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, on the test machine: passed.
- `python3 scripts/check-gui-goldens.py`, locally, since the approval list's layout changed: passed
  (`a4d9af91-8dc3-4ecc-be6c-d758d10a6534`).

The smoke was reused for the fixed tree: the fixes moved one help edge onto a wrapper and changed comments.

Skipped:

- Anything on a Mac: hover help placement was not tried on macOS.
- A test of the scrolled approval list's help: none exists, and I did not add one.

#### Things you should know that the report does not say

- **There is no single rule for help near an open menu.** Help beside an edge treats open menus three ways: sidebar rows
  ignore them, the host rail hides its card, and lists fall back to ordinary placement. Menus, below-first help and tall
  controls each have their own variant again. In one case this shows: the host row's ⋯ button, whose menu opens beside
  it, can put its help over the top of that open menu when the pointer leaves and comes back. The rail fixed and tested
  its identical case. Possible follow-up: one rule. A panel that contains the control is its surface, every other open
  panel is an obstacle, and help that has no room hides. That would also remove two easy mistakes: a popover row added
  without its `in_popover` flag silently loses its help, and a later style call can turn rail help back into sidebar
  placement.
- **Two new tests pass even when no help card is drawn.** The sort menu test and the template ⚡ help test only check
  the card's position if one is found, so a regression that stopped drawing it would pass.
- **The Help menu's help mostly repeats each item's own description** ("Announcements: show the announcements Farhelm
  has shown you before…" beside "Shows previously shown announcements again"). That predates this plan; only its
  placement changed here.
