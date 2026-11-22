# Keyboard shortcuts chord opens the dialog during terminal IME composition

Reviewed commit: 10973c17ec82250ca2f7c862d376055c2a4f9394

## TLDR

Pressing the Keyboard shortcuts chord (Cmd+/ on macOS or Ctrl+Shift+/ on Linux) while an input method is composing in a
focused terminal still opens the Keyboard shortcuts dialog instead of ignoring the chord.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/main_view.rs:289`.

SPEC.md ("Desktop window chrome", lines 3891-3892) requires: "Cmd+/ on macOS, Ctrl+Shift+/ on Linux, opens **Keyboard
shortcuts**, including from a focused terminal. Matching uses the physical Slash key, with no extra modifiers; repeat
and composition do not open it." Commit `2ce86b87983be8d24c9cb2abfe8f566305f670bb` added
`SessionPane::terminal_is_composing` (`crates/gui/farhelm-gui/src/session_pane.rs:515-522`) and wired it into
`DeleteSession` and `RenameSession` in `crates/gui/farhelm-gui/src/main_view.rs:250-266`, while commit
`10973c17ec82250ca2f7c862d376055c2a4f9394` explicitly updated `crates/gui/farhelm-gui/src/shortcuts.rs:27-31` and
`crates/gui/farhelm-gui/src/actions.rs:95-96` to note that `ShowKeyboardShortcuts` (`main_view.rs:289-291` ->
`shortcuts::open_from_chord`) still does not check `terminal_is_composing` (or a focused text field's composition). Fix
direction: guard `ShowKeyboardShortcuts` in `main_view.rs:289-291` with
`!main.session_pane.read(cx).terminal_is_composing(window, cx)` (and a text-input composition query when available),
matching `DeleteSession` and `RenameSession`.

Reviewer metadata (g03-triage-gui-shortcuts-launcher-1/f2): suggested bucket: other (not highest: opens the read-only
Keyboard shortcuts overview dialog; cannot lose user data, credentials, or sessions); confidence: confirmed; possible
cover: docs/gpui/parity/other-dialogs.md (cited in crates/gui/farhelm-gui/src/shortcuts.rs:30-31 as a recorded parity
gap; not read per reviewer rules).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g03-triage-gui-shortcuts-launcher-1/f2`.
