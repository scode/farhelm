# Parity: Desktop integration and Linux packaging

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Desktop window chrome, Installation and updates, Desktop Quit, Status.

| Old spec                           | Tests | Covers                                                                   | Status         |
| ---------------------------------- | ----- | ------------------------------------------------------------------------ | -------------- |
| `tests/window-chrome.spec.ts`      | 6     | the macOS inset belongs to the merged top row and leaves details compact | not yet ported |
| `tests/update-readout-css.spec.ts` | 3     | version hint brightens and turns on hover and keyboard focus             | not yet ported |

## The old native app's own tests

The old desktop shell's Rust unit tests and JavaScript tests at the reference commit, which no Playwright spec covered,
and the old native smoke test's legs (`scripts/desktop-smoke.sh`).

| Old test                                                                            | Tests | Covers                                                                                              | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ----------------------------------------------------------------------------------- | ----- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/farhelm-ui/src/desktop/menu.rs` (`mac_menu_preserves_session_delete_chord`) | 1     | the Mac menu is the old arrangement, with no Close Window, so Cmd+W stays the open session's Delete | replaced: `desktop::tests::the_mac_menu_is_the_old_apps_with_no_close_window`, `desktop::tests::the_menus_commands_are_handled_app_wide_and_the_edit_items_by_a_focused_field`, `actions_tests::the_mac_menu_chords_run_their_actions_and_are_app_shortcuts_in_a_terminal` and `desktop::tests::closing_the_main_window_quits_and_closing_another_does_not`; and the real-window check on the test Mac (2026-11-14): the app menu and Edit show the old items, Cmd+Q and the window's close button each quit the app, and its local supervisor keeps running after |

## Deviations

- **The Edit menu does nothing for a focused terminal.** The Mac app's Edit items (Undo, Redo, Cut, Copy, Paste, Select
  All) dispatch the text field's own editing actions, so they act on a focused text field and are disabled anywhere
  else, a focused terminal included, except Copy: GPUI Kit's window root handles it too, so it stays enabled, and
  outside a text field it copies nothing. In the old app WebKit's own Copy, Paste and Select All items also acted on the
  terminal. The terminal's keys are unaffected (Cmd+C, Cmd+V and Cmd+A still copy, paste and select all there); only the
  menu items do not reach it.
- **macOS adds its own "Enter Full Screen".** The app menu shows macOS's own Enter Full Screen beside the old Toggle
  Full Screen item, and macOS 26's window tiling items (Fill, Center, Move & Resize, Full Screen Tile). The old app's
  toggle used AppKit's own full-screen command, which macOS recognizes, so it added no second item; GPUI's menu items
  cannot name that command.
- **No debug Help menu.** The old app's debug builds had a Help menu with the web view's developer tools and Float on
  Top; both went with the web view.
