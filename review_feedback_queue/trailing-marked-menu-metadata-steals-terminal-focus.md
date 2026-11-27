# Metadata actions from the right-press marked menu move focus away from the terminal

Reviewed commit: c1163ff06cd61d3aacae8b8e65c0aca51ee1bdd7

## TLDR

With several sessions marked, a user typing in a terminal right-clicks a marked row and picks Pin, Mark read, Mute or
Move to group. The action runs, but keyboard focus then lands on the session list's sort-order title button instead of
going back to the terminal, so the next keystrokes go to the sidebar (Enter or Space opens the sort menu) rather than to
the agent. Picking New group… and then cancelling the name prompt sends focus to the marked bar's ⋯ button rather than
back to the terminal.

## Details

Provenance: trailing review through `dbb153ec`, group g11 (what the right-press menu acts on), finding f1 (confidence
raised to confirmed in synthesis); group g10 (session context menu), candidate c6, confirmed from the menu side that the
close path restores focus and the metadata handler then overrides it. Suggested bucket: `other`. Confidence: confirmed
from source; not observed at runtime.

- The commit opens the existing marked menu from a right press with `focus::ReturnTo::Opener`
  (`crates/gui/farhelm-gui/src/session_list/marking.rs:939-970`, `open_marked_context_menu`), so closing it returns
  focus where it was, per its own docstring ("a right press moves no focus, so that is usually the terminal").
- The metadata items still go through `apply_metadata` unchanged (`marking.rs:1047-1130`). It closes the menu (line
  1055, returning focus to the opener), takes a fresh `Mark` (line 1056), and calls
  `focus::focus_control(&self.order_focus, &mark, ...)` after clearing marks (line 1129, and line 1087 for an
  existing-group Move). `layers::close` removes the layer synchronously (`layers.rs:1467`, 1497-1510), and `Mark::take`
  (`focus.rs:278`) counts only user key and click presses, so the move is admitted (`focus.rs:554-562`).
- That focus move exists for the bar's ⋯ menu, whose return target disappears with the bar once marks clear (docstring
  at 1040-1046). That reason does not hold for a right press's menu, whose return target (usually the terminal)
  survives. The New group… branch (line 1066) hard-codes `ReturnTo::Control(self.marking.menu_focus)`, the bar's ⋯.
- SPEC.md, "Keyboard focus", "Opening and closing": "Closing something the user opened returns focus to where it was
  before it opened". SPEC.md, "Session list", says the combined menu's actions act "as the bar's control does", which
  covers what they act on, not a focus jump the right press never made. The commit records a decision for its lifecycle
  items' bulk question returning to the bar, but none for the metadata items.
- A row's or group header's own right-press menu does not have this pattern (g10 checked rename, New group naming and
  the lifecycle questions).

Fix: remember at open time whether the open marked menu is a right press's (`Kept::menu_lifecycle` already does) and,
for that menu, skip the order-title focus move and give New group… a return target of what held focus before the press.
Test: focus the terminal, mark two rows, right-press one, choose Pin, and assert focus is the terminal.

Possible cover: none known.
