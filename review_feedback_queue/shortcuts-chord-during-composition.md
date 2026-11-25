# The Keyboard shortcuts chord can still interrupt composition

Reviewed commit: 10973c17ec82250ca2f7c862d376055c2a4f9394

## TLDR

The overview can still open when native input handling clears terminal composition before the shortcut reaches Farhelm,
or when the focused text field has no composition query.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/main_view.rs`, the `ShowKeyboardShortcuts`
handler.

SPEC.md, "Desktop window chrome", says composition does not open the overview. The main view checks for marked text in
the focused terminal when it receives the action. On Linux, the Wayland and X11 backends feed the native key to XKB
compose first. In the cancellation path they commit the pre-edit text before dispatching the same key with its physical
position and modifiers. `AttachedTerminal::commit_text` clears the mark, so the action can open the overview for a key
pressed during composition. The action needs the composition state from when the key arrived.

Other focused text fields have no composition query for the action. This applies where the chord is otherwise available.
A dialog or menu holding the keyboard suppresses it. Both cases are based on source inspection.

This follows `trailing-shortcuts-chord-opens-during-terminal-ime.md`, which covers the terminal mark still present at
dispatch. The earlier review is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g03-triage-gui-shortcuts-launcher-1/f2`.
