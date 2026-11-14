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

| Old test                                                                            | Tests | Covers                                                                                              | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------------------------------------------------------------------------- | ----- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/farhelm-ui/src/desktop/menu.rs` (`mac_menu_preserves_session_delete_chord`) | 1     | the Mac menu is the old arrangement, with no Close Window, so Cmd+W stays the open session's Delete | replaced: `desktop::tests::the_mac_menu_is_the_old_apps_with_no_close_window`, `desktop::tests::the_menus_commands_are_handled_app_wide_and_the_edit_items_by_a_focused_field`, `actions_tests::the_mac_menu_chords_run_their_actions_and_are_app_shortcuts_in_a_terminal` and `desktop::tests::closing_the_main_window_quits_and_closing_another_does_not`; and the real-window check on the test Mac (2026-11-14): the app menu and Edit show the old items, Cmd+Q and the window's close button each quit the app, and its local supervisor keeps running after                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `crates/farhelm-ui/src/desktop/window_state.rs`                                     | 10    | the saved frame's format and restore rules, and following the live window                           | replaced: the format and restore tests moved unchanged with their code (`window_state::tests`, in `crates/farhelm-desktop`), the following is `window_state::tests::reports_only_make_the_frame_pending_and_a_flush_writes_the_last_one` and the other `FrameKeeper` tests beside it, `desktop::tests::the_hosts_placement_is_where_the_window_opens`, `desktop::tests::following_the_window_reports_its_frame_at_once_and_on_every_change` and `desktop::tests::the_host_hears_once_that_the_window_settled_after_it_stops_changing`; the real-window check on the test Mac passed (2026-11-14: a moved and resized window reopens where it was with no growth, a zoom is saved with the ordinary frame from before it and reopens zoomed, a window reopened zoomed and left alone stays zoomed, an unzoom is saved, a frame off every display falls back; not covered: a second Mac display, Linux displays, KDE); a relaunch on the same state directory on Linux is part of the Linux lifecycle check still to come |

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
- **The saved window frame starts over once.** It is now saved in logical pixels, where the old app saved physical ones,
  because GPUI reports no display scale factor before a window opens; the old app's saved frame is ignored at the first
  launch after the switch, which opens at the default frame.
- **A window on another Mac display reopens on the primary one.** GPUI reports every Mac display at the same origin and
  window positions relative to the window's own display, and the app's startup names no display, so only positions on
  the primary display are saved; the old app saved global positions and reopened on the display the window was last on.
- **A change in the last second before a crash can be lost.** The frame is saved a second after the window stops
  changing and at quit, where the old app saved once, when its window closed.
- **No debug Help menu.** The old app's debug builds had a Help menu with the web view's developer tools and Float on
  Top; both went with the web view.
