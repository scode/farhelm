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
