# A session menu opened with a right press stays put when its row moves

Reviewed commit: c1163ff06cd61d3aacae8b8e65c0aca51ee1bdd7

## TLDR

When a session's menu, or the combined menu for marked sessions, is opened with a right press and the list shifts under
it (a session above is deleted elsewhere, a row above grows or shrinks, the list scrolls a little), the menu stays where
the press was while its session's row moves away. The menu can end up over a neighbouring session's row, still acting on
the session it was opened for. The same menu opened from the ⋯ button moves with its row, and SPEC.md says a row's open
menu moves with its session.

## Details

Provenance: trailing review through `dbb153ec`, group g10 (session context menu), finding f1; group g11 (what the menu
acts on), candidate c6, showed the combined marked menu shares the route and is folded in here. Suggested bucket:
`other`. Confidence: confirmed for the code behavior; that the spec sentence covers row shifts other than a reorder is
the reviewer's reading.

- `popover_origin`'s new arm `(Some(_), Placement::AtPoint(at)) => *at` (`crates/gui/farhelm-gui/src/layers.rs:2691` at
  the tip) ignores the anchor's bounds on every frame. The `Placement::AtPoint` docs (`layers.rs:399-404`) say so on
  purpose: "The point does not follow the anchor: a list that scrolls under an open context menu leaves the menu where
  it opened."
- SPEC.md, "Keyboard focus", "Things changing underneath": "Focus follows the thing, not the position. When the session
  list reorders, a focused row, or its open menu, moves with its session." The order hold (`session_list/hold.rs` module
  docs) blocks a reorder while a row menu is open, but not every shift: "a session that disappears goes at once", and
  rows update in place, so rows below the change move.
- The combined marked menu is placed the same way (`crates/gui/farhelm-gui/src/session_list/marking.rs:950`); it closes
  only when what it offers changes or its row is no longer drawn.
- Nothing acts on the wrong session: the menu's header names its session (`RowMenu::contents`, `menu.rs` ~986) and its
  commands act on the key it was opened for.

Fix: store the press as an offset from the anchor's (or row's) bounds at open time and place at anchor origin plus
offset each frame, still clamped, covering both call sites; or decide in SPEC.md that a pointer-placed menu stays where
it opened. A GUI test can open a context menu on a row, push a listing without the row above it, and compare the menu's
origin with the row's ⋯ button before and after.

Possible cover: none known in SPEC.md. The code documents the choice (`layers.rs:403-404`), and the new SPEC.md "Session
list" paragraph says only that the menu opens at the pointer.
