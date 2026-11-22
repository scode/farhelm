# Parity: Desktop integration and Linux packaging

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Desktop window chrome, Installation and updates, Desktop Quit, Status.

### `tests/window-chrome.spec.ts`

Tests: 6. Covers: the macOS inset belongs to the merged top row and leaves details compact.

replaced: `desktop::tests::presses_on_empty_band_background_drag_and_controls_and_text_keep_theirs`,
`desktop::tests::a_double_click_zooms_only_when_both_presses_were_on_the_same_rows_background`,
`desktop::tests::the_window_options_give_the_band_the_title_bar_only_on_macos` and, run on the test Mac (2026-11-17,
with the two bullets below, all passing),
`desktop::tests::mac::the_real_rows_drag_from_their_background_and_leave_their_controls_and_text_alone`,
`desktop::tests::mac::with_no_session_open_the_empty_panes_strip_drags`,
`desktop::tests::mac::the_band_rows_are_48_high_at_the_top_and_the_app_bar_keeps_the_lights_clear`,
`desktop::tests::mac::with_no_session_open_a_band_strip_covers_the_panes_top` and
`desktop::tests::mac::in_a_narrow_window_the_app_bar_runs_above_both_panes`; the web-shell case is superseded (there is
no web shell), and "text stays selectable" is superseded by labels no longer being selectable (decided by the maintainer
on 2026-11-13; see Deviations); and the real-window checks on the test Mac passed (2026-11-14: dragging the wordmark or
the session top row's empty background moves the window, a drag that starts on the version text does not, a double-click
on the band zooms and a second one restores; 2026-11-17, with a shell tab open: a drag starting in the gap between the
Agent and Terminal 1 tabs moves the window, as does one starting just above the shell tab, a drag starting on the shell
tab does not, and no press resizes the window). The `app-title-band` scene, captured by the screenshot batch on the test
Mac (2026-11-14, light, narrow), matches the reference `macos-window-chrome`: the inset kept clear for the traffic
lights, the wordmark and version, and the 48 px top row across both panes; the batch captures the app's own drawing, so
the lights themselves show only in a screen capture of the real window. Not covered in a real window: a press on a stale
session's inert tab (covered in process by the last bullet). Not covered by any test: a press on the text-size stepper.
Not checked: a Force Touch trackpad, a second display

- `desktop::tests::mac::the_connection_screens_strip_keeps_the_lights_clear_and_drags`: while the window is not
  connected, a 48-point band strip runs across its top and a press on it drags.
- `desktop::tests::mac::the_top_rows_tabs_keep_their_presses_live_or_inert_and_the_strip_around_them_drags`: with two
  shell tabs, a press on a tab chooses it and does not drag, a press in the gap between two tabs and one above a tab
  drag, and once the session is listed stale a press on its inert tab does not drag.

### `tests/update-readout-css.spec.ts`

Tests: 5. Covers: version hint brightens and turns on hover and keyboard focus.

replaced: `update_readout::view::tests::hover_and_keyboard_focus_brighten_the_version_and_turn_the_glyph`,
`the_idle_version_is_a_named_check_button`, `a_visible_check_spins_the_glyph_and_shows_its_words`,
`an_installed_update_keeps_focus_on_the_readout_only_if_it_had_it` (the three focus cases),
`the_spinner_runs_only_while_something_spins_and_stands_still_under_reduced_motion`, and the goldens of every state,
light and dark (`version-readout-check`, `version-readout-progress`, `version-readout-outcomes`,
`version-readout-update-ready`, `version-readout-menu`, `version-readout-tooltip`). The hover transitions are not ported
(see Deviations). Per their scene titles in `docs/gpui/screenshot-scenes.json`, `-check` shows the check button at rest,
lit as under the pointer, with a reinstall warning, that warning lit, and with keyboard focus; `-progress` a check and
an install, alone and beside the update toggle; `-outcomes` Up to date, Check failed, Check failed beside the toggle,
and the failure button focused; `-update-ready` the update toggle plain, with a withdrawal warning, and focused; `-menu`
the update menu open; `-tooltip` the wider paragraph help for a reinstall readout. The goldens are Linux only: no Mac
golden shows the readout in the band

## The old native app's own tests

The old desktop shell's Rust unit tests and JavaScript tests at the reference commit, which no Playwright spec covered,
and the old native smoke test's legs (`scripts/desktop-smoke.sh`).

### `crates/farhelm-ui/src/desktop/menu.rs` (`mac_menu_preserves_session_delete_chord`)

Tests: 1. Covers: the Mac menu is the old arrangement, with no Close Window, so Cmd+W stays the open session's Delete.

replaced: `desktop::tests::the_mac_menu_is_the_old_apps_with_no_close_window`,
`desktop::tests::the_menus_commands_are_handled_app_wide_and_the_edit_items_by_a_focused_field`,
`actions_tests::the_mac_menu_chords_run_their_actions_and_are_app_shortcuts_in_a_terminal` and
`desktop::tests::closing_the_main_window_quits_and_closing_another_does_not`; and the real-window check on the test Mac
(2026-11-14): the app menu and Edit show the old items, Cmd+Q and the window's close button each quit the app, and its
local supervisor keeps running after

### `crates/farhelm-ui/src/desktop/window_state.rs`

Tests: 10. Covers: the saved frame's format and restore rules, and following the live window.

replaced: nine of the ten format and restore tests moved unchanged with their code (`window_state::tests`, in
`crates/farhelm-desktop`); the tenth, `maximized_transitions_preserve_last_normal_bounds`, was removed with the unused
helper it tested (2026-11-14), and its case is the bullet below; the following is
`window_state::tests::reports_only_make_the_frame_pending_and_a_flush_writes_the_last_one` and the other `FrameKeeper`
tests beside it, `desktop::tests::the_hosts_placement_is_where_the_window_opens`,
`desktop::tests::following_the_window_reports_its_frame_at_once_and_on_every_change` and
`desktop::tests::the_host_hears_once_that_the_window_settled_after_it_stops_changing`; the real-window check on the test
Mac passed (2026-11-14: a moved and resized window reopens where it was with no growth, a zoom is saved with the
ordinary frame from before it and reopens zoomed, a window reopened zoomed and left alone stays zoomed, an unzoom is
saved, a frame off every display falls back; not covered: a second Mac display, Linux displays, KDE); on Linux,
`scripts/gui-lifecycle.py` steps 04, 09 and 10 (the frame saved at the window's size while the app runs; a saved size
back after a relaunch, written between launches since the bridge cannot resize a window on headless Mutter; and a window
maximized by the title bar's Maximize saved maximized with its ordinary frame kept, reopened maximized, and once quit
again still saved maximized with that frame)

- `window_state::tests::maximize_keeps_the_ordinary_frame_and_fullscreen_is_not_saved`: the removed test's case, the
  ordinary frame kept while maximized, now as a `FrameKeeper` test.

### `crates/farhelm-ui/src/window_chrome.rs` (`press_decision_covers_buttons_reports_and_fullscreen`, `click_report_header_parsing_rejects_non_reports`)

Tests: 2. Covers: which press on the band drags, zooms or does nothing, and the press report the JavaScript bridge sent.

replaced: `desktop::tests::the_press_decision_drags_zooms_or_leaves_the_press_alone`; the report parsing is superseded,
since presses reach the band through GPUI's own propagation and no report crosses a transport

### `crates/farhelm-ui/js-tests/click-detail.test.js`

Tests: 20. Covers: which presses inside a drag surface count as empty background, and when a repeat may zoom.

replaced: the classification and pairing cases by
`desktop::tests::presses_on_empty_band_background_drag_and_controls_and_text_keep_theirs` and
`desktop::tests::a_double_click_zooms_only_when_both_presses_were_on_the_same_rows_background`; the endpoint, header and
synchronous-POST cases are superseded (no JavaScript bridge); "a press over an inert descendant" (the old case pressed a
stale session's inert tab) is the bullet below, run on the test Mac on 2026-11-17: the terminal tabs are now in the
session top row and go inert with the terminal under a stale session's card, and the inert tab is covered in process
only, while a live tab and the strip's empty part beside and above one were also checked in a real window that day
(`window-chrome.spec.ts`'s entry); a positioned popover over the band is covered by the layer manager's occluding
popovers, which a press never passes through; "a press inside a surface but outside its box" is GPUI's hit testing,
which hands a row only presses inside it; "a replaced or disconnected spacer" is covered by pairing presses by row, not
by element

- `desktop::tests::mac::the_top_rows_tabs_keep_their_presses_live_or_inert_and_the_strip_around_them_drags`: a press on
  a live tab and on a stale session's inert tab keeps the tab's, and the strip's empty part around the tabs drags.

### The old native smoke test's legs

`scripts/desktop-smoke.sh` at the reference commit (`git show 24bc0580:scripts/desktop-smoke.sh`) booted the old
browser-UI app's WebKitGTK window under Xvfb and checked it through its helm API, its state directory and the process
table, one leg after another in one script. Its default phase is the first rows; the optional appearance,
missing-initialize and pixel-driven phases are the last three. The Linux GPUI app's lifecycle check,
`scripts/gui-lifecycle.py`, replaces the legs about launching, quitting and relaunching; its steps are cited by number.
The supervisor decisions moved unchanged with their unit tests (in `crates/farhelm-desktop` unless another crate is
named).

#### "a bootstrap failure exits 1 with a plain message, no panic"

Tests: 1. Covers: a launch whose state directory is a plain file exits 1 with one `farhelm-desktop: ` line and no panic.

replaced: `scripts/gui-lifecycle.py` step 02

#### "booting Xvfb, openbox, and the self-contained app"

Tests: 1. Covers: the embedded helm comes up and the local host connects; no browser UI at `/`, no `/api/auth/token`,
the web view authenticates, no stored credential or device row.

replaced: the boot and connection by `scripts/gui-lifecycle.py` step 01 (the window draws the listing it read over its
UI channel, the helm lists its local host as connected, and the app's own supervisor is recorded and running); the web
view's authentication is superseded (the window reaches the helm in process with an in-memory credential); the 404s for
`/` and `POST /api/auth/token` are the first bullet below; the check for no stored credential or device row is
superseded, since the helm keeps credentials only in memory and writes none to `helm.db`. That rests on the design (the
helm's `auth` module has no store), not on a test: no test asserts that minting writes no row, and the other two bullets
test only its consequences

- `router_boundary_tests::the_helm_serves_no_page_and_no_sign_in` (in `farhelm-helm`): `/` and `POST /api/auth/token`
  answer 404.
- `auth::tests::credentials_are_scoped_to_the_minting_process`: a credential another helm process minted is refused.
- `auth::tests::a_device_row_left_by_an_older_helm_does_not_authenticate`: a `device_sessions` row an older helm stored
  authenticates nothing.

#### "a second desktop launch refuses the owned state directory"

Tests: 1. Covers: a second launch on an open app's state directory exits 1 and the open app keeps serving.

superseded: on Linux a second launch now brings the open app forward (settled by the migration's coordinator on
2026-11-13), `scripts/gui-lifecycle.py` step 05 and
`second_launch::tests::a_second_launch_hands_over_only_to_a_live_app_and_carries_its_token`; the refusal stays for a
launch that cannot reach the open app, `the_launch_lock_refuses_beside_an_open_app_and_waits_out_a_brief_holder`

- `desktop::tests::each_second_launch_activates_the_main_window_with_its_token`: the open app raises its main window
  once per second launch, with that launch's activation token.

#### "a second launch with another program refuses without stopping the open app's supervisor"

Tests: 1. Covers: a second launch whose program differs refuses at the open app's lock before it stops anything.

superseded the same way: a second launch forwards before it reads or decides anything about a supervisor, and step 05
checks the supervisor is the same afterwards; beside an open app that cannot be reached, the lock refuses before any
supervisor decision (`the_launch_lock_refuses_beside_an_open_app_and_waits_out_a_brief_holder`)

#### "waiting for the first-boot listing request through the smoke hook"

Tests: 1. Covers: the web view's first session listing asks for the default order.

superseded: a hook into the web view's REST listing; the window's listing and its order are the sidebar area's

#### "waiting for the client-log marker" (both launches)

Tests: 2. Covers: the web view's console reached the helm's log.

dropped: there is no web view or JavaScript console

#### "asset handler served ... requests, 0 missing"

Tests: 1. Covers: every web asset the window loaded came from the bundle compiled into the binary.

dropped: the GPUI app loads no web assets; its fonts and icons are compiled in and drawn by the goldens

#### "creating two bundle-substrate sessions before the hard restart"

Tests: 1. Covers: two sessions created through the helm's API, each with a live agent terminal running in a terminal
host, set up for the restart leg.

replaced: `scripts/gui-lifecycle.py` seeds one session through the helm's API (`gui-agent.py start --sessions 1`) for
steps 07 and 08, which is all the restart legs need; that a created session's agent runs in a terminal host is the
bullet below. The second session let the old relaunch show that the window reopened on the remembered session rather
than the newest, which is the sidebar area's to port

- `host_creates::a_create_starts_its_agent_in_a_terminal_host` (the e2e suite,
  `crates/farhelm/tests/e2e/host_creates.rs`): a created session's row is a terminal-host row naming the host's socket,
  and the session lists as live.

#### "waiting for the window and nudging it to render"

Tests: 1. Covers: resizing the Xvfb window until it reported a sane size, so the web view would paint.

test infrastructure: a workaround for the old browser-UI app's WebKitGTK window under Xvfb; the bridge reports the GPUI
window's size and waits for drawn text

#### "killing the app without Rust cleanup and relaunching it"

Tests: 1. Covers: after SIGKILL the supervisor survives and the relaunch reuses it, mints a new credential and refuses
the old one, and keeps the sessions (their terminals alive) and the helm's preferences.

replaced: `scripts/gui-lifecycle.py` step 08 (SIGKILL, the same supervisor running and reused, the seeded session listed
again as running, waiting or idle) and step 07 (the same after a quit, and a new credential with the old one refused
with 401). The old leg's live terminal was a terminal host still listening on its socket, not typed input; a session
whose terminal was lost lists as interrupted, which the steps' live-state check refuses, and step 08 also types into the
kept session in the relaunched window and sees it echoed (`assert_session_takes_typing`). The remembered selection and
sort are the helm's preferences, which the sidebar area restores, and this entry does not claim them; the web view's
re-authentication and attachment trace are superseded

#### "rotating the token: the internal desktop credential carries on"

Tests: 1. Covers: rotating the helm token neither revokes the app's credential nor re-authenticates the web view or
closes its terminal socket.

superseded: the web view's re-authentication and socket are gone, and so are the `farhelm helm token rotate` command and
the durable token it rotated (removed with the browser interface; see `releasing/changelog.d/gpui-desktop.md`), so no
rotation can happen; the window's credential is minted per launch and kept in memory, and a relaunch's new credential,
with the old one refused, is `scripts/gui-lifecycle.py` step 07

#### the eval-bridge watchdog stays silent

Tests: 1. Covers: the web view watchdog's death message never fired during a healthy run.

dropped: the watchdog watched the web view's eval bridge, which is gone

#### the close chord quits the app

Tests: 1. Covers: Alt+F4 through the window manager closes the window and the app exits successfully, leaving the
supervisor running.

partly: a click on the Linux title bar's Close quits the app the orderly way with the supervisor still running,
`scripts/gui-lifecycle.py` steps 06 and 10 (steps 09 and 10 also quit by SIGTERM with the same checks), and
`desktop::tests::closing_the_main_window_quits_and_closing_another_does_not` (in `crates/gui/farhelm-gui`) closes the
window in process; a close the compositor asks for (Alt+F4, KWin's own Close), which reaches the app by another path
than the drawn button, is not yet ported; the pending KDE check covers KWin's

#### "a launch with a replaced program replaces the app's own supervisor"

Tests: 1. Covers: a program file replaced the way an install does it makes the next launch stop the app's own supervisor
and start one from the new file, with sessions kept and driveable.

replaced: the decision by `the_app_replaces_only_its_own_supervisor_and_only_when_it_must`, and a replaced file counting
as another program by `managed_supervisor::tests::the_same_path_with_a_replaced_file_is_a_different_program` and
`a_running_supervisor_matches_after_its_program_is_replaced` (in `farhelm-supervisor`); the end-to-end replacement with
a session kept and typed into is the bullet below

- `scripts/gui-lifecycle.py` step 11: with the app quit and its supervisor running, a launch from a copy of the program
  in the run, and then one after that copy was replaced the way an install does it (a new file renamed over the path),
  each stop the old supervisor the orderly way and start a new one running the new file, and the seeded session stays
  live and takes typing.

#### "a replacement that would fail its prerequisites stops nothing"

Tests: 1. Covers: a replacement whose program folder holds `:` refuses with exit 1 while the old supervisor keeps
serving and its record stays.

replaced: a replacement program that does not run refuses before anything is stopped
(`a_replacement_refuses_a_program_that_does_not_run`), and the folder rule itself is
`service::core::tests::a_colon_in_the_session_program_folder_is_refused_at_startup` (in `farhelm-supervisor`); the
desktop's own check is the first bullet below, and that the launch refuses before it stops the old supervisor is the
second

- `tests::a_program_folder_holding_a_colon_fails_the_desktops_prerequisites` (in `crates/farhelm-desktop`): a program in
  a folder holding `:` refuses, naming the folder and that it cannot be one PATH entry, while a plain folder or a colon
  only in the file name passes; in the versioned Mac layout the forwarder's folder is the one checked.
- `scripts/gui-lifecycle.py` step 12: with the app quit and its supervisor running, a launch from a copy of the program
  in a folder named `replace:refused` exits 1 with the folder's refusal, and the same supervisor is still recorded,
  running and accepting at its socket.

#### "a launch replaces the app's own supervisor when it never answers"

Tests: 1. Covers: a frozen supervisor of the app's own is given the startup wait, stopped, killed after the grace, and
replaced.

replaced: `waiting_for_the_apps_own_silent_supervisor_ends_on_answer_exit_or_deadline`,
`a_failed_discovery_counts_as_not_answering_only_for_the_apps_own_supervisor` and the not-answering cases of
`the_app_replaces_only_its_own_supervisor_and_only_when_it_must`

#### "an answering supervisor the app did not start is reused"

Tests: 1. Covers: a hand-started supervisor on the state directory is reused, not stopped, and the app records none of
its own.

replaced: `the_app_replaces_only_its_own_supervisor_and_only_when_it_must` (without the app's own record, an answering
supervisor is reused); the leg's teardown of a planted terminal host was harness cleanup (test infrastructure)

#### the appearance leg (`DESKTOP_SMOKE_APPEARANCE=1`)

Tests: 4. Covers: a fresh and a stored launch in dark and light GTK themes paint the web view in the matching palette.

superseded: it drove the web view's `matchMedia` from GTK themes; the GPUI app's light and dark palettes are its
goldens, light and dark, which run only under `scripts/check-gui-goldens.py`. The remaining case, a launch in the
system's palette that a stored choice overrides, is the Settings area's tests below

- `settings::tests::the_stored_appearance_paints_the_window_from_the_first_snapshot_on`: the window starts in the
  system's palette and the stored choice applies from the first snapshot on.
- `settings::tests::system_follows_the_operating_system_and_an_explicit_choice_does_not`: System follows the operating
  system, and a stored Light or Dark wins over it.
- `settings::tests::an_unset_or_unknown_appearance_reads_as_system`: no stored choice reads as System.

#### the missing-initialize leg (`DESKTOP_SMOKE_APPEARANCE=no-initialize`)

Tests: 1. Covers: a web view document that never initializes is refused within its creation deadline, and the supervisor
keeps running detached.

superseded: the fault was in the web view's document; a GPUI window that cannot open reports through the same pre-window
refusal as a bootstrap failure (`Host::on_startup_failure`; the bullet below), and the supervisor is detached from the
app in any case (`scripts/gui-lifecycle.py` steps 06 and 08)

- `bootstrap::tests::a_window_that_cannot_open_reaches_the_hosts_startup_failure_once` (in `crates/gui/farhelm-gui`): on
  a test platform whose window renderer cannot be made, the startup fails at the window and its error reaches
  `on_startup_failure` once, with no window left open. `run`'s one-line call into that branch and the desktop binary's
  own startup-failure handler are not run by any test; `scripts/gui-lifecycle.py` step 02 shows the same refusal (one
  `farhelm-desktop: ` line, exit 1) from the pre-window path.

#### the pixel-driven create and terminal round trip (`DESKTOP_SMOKE_LEGACY_INTERACTION=1`)

Tests: 2. Covers: a session created through the create form, then a command typed into its terminal.

superseded: the bridge drives the app by its controls rather than by pixels; typing into a session's terminal is
`scripts/gui-e2e-smoke.py` step 02, and the create form is the launcher area's

## Behavior with no old test

- **The Mac Edit menu's Copy, Paste and Select All reach a focused terminal.** They act as Cmd+C, Cmd+V and Cmd+A do
  there, and in a focused text field as the field's own keys do; Undo, Redo and Cut act on a text field only and are
  disabled over a terminal, where the old app's did nothing useful either. With neither focused, all six are disabled.
  In the old app the menu's Paste reached the terminal, but its Select All most likely did not (the old terminal
  selected everything only from its Cmd+A key handler), and whether its Copy did without a right-click first is
  uncertain. Tested by the menu description test (`desktop::tests::the_mac_menu_is_the_old_apps_with_no_close_window`),
  which also pins the chords the three items show to the terminal's key rows, the enabled-state test
  (`desktop::tests::the_menus_commands_are_handled_app_wide_and_the_edit_items_by_a_focused_field`),
  `components::tests::text_input::the_edit_menus_copy_paste_and_select_all_act_on_every_kind_of_field` and
  `terminal::attached::tests::the_edit_menus_copy_paste_and_select_all_reach_a_focused_terminal`. Checked on the test
  Mac in a real window (2026-11-17), over a focused terminal: Undo, Redo and Cut are disabled and Copy, Paste and Select
  All enabled, the menu's Paste reaches the terminal as a bracketed paste, and Select All then Copy copies the
  terminal's text. A separate native Cmd+C check on the test Mac (2026-11-17) kept focus on the New session button and
  left the system output device idle. A control beep activated the same device.
- **The Open-Source Notices window** (new: the old app had no notices surface; the maintainer decided on 2026-11-13 that
  the notices live in it). Tested by `desktop::tests::the_notices_item_opens_one_window_and_brings_it_forward` (one
  window, the text drawn as a virtualized list, a second choice brings it forward, also when made from the notices
  window itself, and a choice after its close opens a new one),
  `desktop::tests::the_notices_are_wrapped_at_the_column_and_keep_every_character`,
  `desktop::tests::the_notices_window_is_titled_and_fits_its_column`, the menu description test and the
  `open-source-notices` golden. On Linux the window opens from the host rail's `?` Help menu, whose Open-Source Notices
  item dispatches `actions::OpenSourceNotices`
  (`help_menu::tests::open_source_notices_dispatches_the_app_menus_action_on_linux`); `farhelm-desktop --licenses` still
  prints the same text. Checked on the test Mac in a real window: the item sits between Minimize and Quit; choosing it
  again while the notices window is key, behind the main window or minimized brings that one window forward; Cmd+W there
  does nothing; closing it leaves the app running; Cmd+Q quits with it open.
- **The updater's window side** (the old app's updater wiring had no test of its own; its pure rules moved with
  `update_readout`'s tests, now in the GUI crate). Tested by `desktop::updates::tests` (no updater means nothing runs
  and nothing is offered; the window shows the updater's latest readout; the commands reach the updater and a failed
  automatic-updates change comes back; Restart to update quits only when the restart started) and
  `tests::the_updater_feed_reports_each_change_once_and_ends_with_the_updater` in `crates/farhelm-desktop`. That only
  the installed release bundle starts one is `updater::tests::the_updater_runs_only_for_the_installed_release_bundle`.
  The entry points other areas call are `desktop::updates::runs`, `check_now`, `automatic_updates` and
  `set_automatic_updates`.
- **The Mac app comes forward when it starts.** As soon as its main window opens, the app asks macOS to make it the
  active app, however it was started; macOS does that by itself only for an app Launch Services opens, so a build run
  from a terminal otherwise opened behind the app in front. The old app asked too, once its window was visible, with no
  test. Tested by `desktop::tests::the_mac_brings_the_main_window_forward_at_startup`, which sees only the window's
  half: the request to macOS does nothing on GPUI's test platform, and was checked on a test Mac (started from a shell
  with Finder in front, the app was frontmost). On Linux the compositor decides.
- **The Linux window's own title bar** (new: the old app's Linux build was never released). Under a compositor that
  draws no window decorations (GNOME's Mutter) the main window and the notices window draw a 34 px bar with the title
  and Minimize, Maximize or Restore, and Close; a drag on its background moves the window and a double-click maximizes
  or restores it, through the macOS band's press rule, and the drag leaves keyboard focus where it was. Where the
  compositor draws its own title bar (KDE) the window draws none. The minimum size grows by GPUI Kit's frame and the bar
  so the app's content keeps 720 by 560. Tested by `desktop::linux_title_bar::tests` (when it shows, the minimum and
  opening allowances, the named buttons, their commands, the background's drag and zoom and focus) and the screenshot
  batch's crop test (`scripts/test-gui-screenshot-batch.py`,
  `test_the_linux_windows_own_title_bar_is_cropped_with_the_frame`); driven through the bridge on headless Mutter (the
  bar inside Kit's frame, its buttons, Maximize and its relabelling to Restore, Close ending the app; Close and Maximize
  also in `scripts/gui-lifecycle.py` steps 06 and 10); the `linux-title-bar` golden shows it ordinary and maximized.
  Pending: a drag, a double-click zoom and Minimize on a real GNOME session (headless Mutter has no pointer drag and no
  visible minimize), and the KDE check (KWin's own bar, no drawn bar, no Kit margin, the 722x596 minimum, drag, resize
  and close), which cannot run here.
- **The Linux app refuses an X11-only session** (new: the old app's Linux build was never released). Without a Wayland
  display the app prints `farhelm-desktop: the desktop app needs a Wayland session ...` and exits 1 before starting
  anything, rather than starting its helm and supervisor behind a window that never shows. A second launch from such a
  shell still brings an open app forward, since that needs no display. On Linux the refusal reaches only stderr (a
  terminal or the journal): the native alert the startup refusal path shows is the Mac's alone, so a launch from a
  desktop menu shows nothing. Tested by `tests::a_linux_launch_without_wayland_refuses_with_a_clear_line` in
  `crates/farhelm-desktop` (which environments refuse) and `scripts/gui-lifecycle.py` step 03 (a launch without one
  exits 1 with the line and creates no state directory) and step 05 (a second launch without one is handed over).
- **Linux installation and updates: not built.** The maintainer decided on 2026-11-15 that the migration builds no Linux
  package, desktop entry install or Linux auto-update; the Linux app runs from a build, shows no update UI (no updater
  runs there), and is tested for real on Linux by the bridge, `scripts/gui-e2e-smoke.py` and `scripts/gui-lifecycle.py`.
  The packaging work is the "Linux installation and updates for the desktop app" entry in `TODO.md`'s "Near term".

## Deviations

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
- **A window sized by hand to fill a Mac display counts as zoomed.** A window within 2 points of its display's visible
  area is saved as zoomed, because GPUI's own check wants an exact match and a window GPUI zoomed after showing it came
  out a point short on 2026-11-14 (GPUI now zooms before showing the window, which has not been checked on a Mac); so a
  window dragged by hand to fill the screen reopens zoomed rather than at that frame.
- **A change in the last second before a crash can be lost.** The frame is saved a second after the window stops
  changing and at quit, where the old app saved once, when its window closed.
- **A notice banner sits below a strip of the title band.** While the connection-lost notice (new in this app, so with
  no old counterpart) is posted on macOS, an empty band strip above the banners holds the traffic lights, and the
  sidebar's top row below them keeps its room for the lights.
- **The version readout changes at once.** Its hover and focus brightening and the glyph's quarter turn happen
  instantly, where the old sheet faded the color over 150 ms and turned the glyph over 300 ms: no GPUI component
  animates a hover. The status words carry the readout's hover (a status mark needs one; the old status had none), and
  the screen-reader live region and hidden "Update ready" announcement are not built (a known gap). Closing the update
  menu when the status beside it changed width is not ported (the old menu was placed from a snapshot; a GPUI popover
  follows its toggle), nor is moving focus to the Settings gear after Check failed is dismissed (SPEC.md now sends focus
  home), nor opening the menu with an arrow key. Its keyboard focus ring is the shared control ring, 1 px outside the
  control, where the old sheet drew the readout's ring 2 px out.
- **The Linux window draws a title bar of its own where the compositor draws none.** SPEC.md says the Linux window
  layout keeps its appearance and gains no drag or zoom; under GNOME's Mutter, which draws no decorations, the window
  would otherwise have no title bar at all and could not be moved with the pointer (settled by the migration's
  coordinator on 2026-11-13). The bar is inert while a dialog is open, as the macOS band is, and under KDE the minimum
  is 722 by 596, since it is set before the compositor says whether it draws the bar.
- **Text in the title band is not selectable.** In the old app a double-click on the band's text (the version, say)
  selected a word of it; GPUI's labels are not selectable, and the maintainer decided on 2026-11-13 that, apart from the
  terminal and text fields, text is copyable only through the copy buttons and copy menu items the old app had, so no
  text in the band can be selected or copied.
- **No debug Help menu.** The old app's debug builds had a Help menu with the web view's developer tools and Float on
  Top; both went with the web view.
- **The version readout's hover is the wider paragraph help.** The old app showed it in the short tooltip, 280 px wide
  and cut at eight lines, which hid the command at the end of the reinstall, upgrade-floor and interrupted install or
  uninstall advice (`curl -fsSL https://get.farhelm.io/install.sh | sh`, for the user to retype). It is now up to 440 px
  wide with no line limit, so that command shows; the maintainer decided this on 2026-11-16.
