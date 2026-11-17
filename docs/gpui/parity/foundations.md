# Parity: Foundations (M1)

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Keyboard focus, Things changing underneath, Losing the connection to the helm, One GUI at a time; and
SPEC_impl.md, GUI architecture.

NOTE: The screenshot goldens named here (`buttons`, `session-list` and the rest) do not run in nextest. They are ignored
in ordinary test runs because they need the real renderer (`crates/gui/farhelm-gui/src/goldens/mod.rs`), and run through
`scripts/check-gui-goldens.py` instead. Every test named by its function runs in the workspace's ordinary nextest suite.

The popover-movement specs asserted that a menu closes when its anchor moves. SPEC.md now says an open menu moves with
its session, so those rows are superseded. The case they keep, a menu whose session or anchor is gone closing (SPEC.md,
"Keyboard focus"), is ported for every kind of menu:

- crates/gui/farhelm-gui/src/layers_tests.rs `a_popover_follows_its_anchor_and_closes_when_the_anchor_is_gone` (any
  popover of the layer manager, which draws the host menus and the bell's notification list as well as the session
  menus)
- crates/gui/farhelm-gui/src/session_list/menu_tests.rs `a_pushed_removal_of_the_row_closes_its_menu` (a row menu whose
  row a pushed listing removes)
- crates/gui/farhelm-gui/src/session_pane/header_tests.rs `a_caret_menu_closes_when_its_session_is_deleted_elsewhere`
  (the session header's caret menus)
- crates/gui/farhelm-gui/src/templates/dialog_tests.rs `the_key_menu_closes_when_its_template_is_gone` (the Templates
  dialog's number-key menu)

One closing rule still stands: changes in the Hosts attention list retire session menus anchored below it (SPEC.md,
"Session list"); that case belongs to the attention-list row of `tests/host-updates.spec.ts` in
[Sidebar and session view](sidebar-and-session-view.md), which names
`crates/gui/farhelm-gui/src/session_list/attention/tests.rs`
`menus_below_close_when_the_list_moves_and_stay_through_progress`.

The screenshot batch (`docs/gpui/screenshot-batch.md`) captures the first slice (session list, the open session's
terminal, row menu, Delete question, rename dialog, empty fleet) beside the foundation goldens; of its reference scenes,
the Delete question and the rename dialog are ported, and the rest wait for the parts of their pictures that no area has
built yet. `scripts/gui-e2e-smoke.py` drives the same slice end to end through the bridge.

The old app's selects had no spec of their own; the shared select (`crates/gui/farhelm-gui/src/components/select.rs`,
tested in `components/tests/select.rs` and pinned by the `select` and `select-open` goldens) replaces the native
`<select>` with two recorded deviations. The native pop-up was the platform's own menu, which on macOS opened over the
control with the chosen option on top of it; the GPUI select draws its list as an anchored menu of the layer manager,
below the control (above it when there is no room below). And the native menu was at least as wide as its control and
grew to its longest option, where the GPUI list is exactly the control's width and cuts a longer option with an ellipsis
(the migration's orchestrator chose "exactly" for the launcher's floating lists on 2026-11-14). Smaller differences that
follow from drawing a menu: the arrow keys wrap from the last option to the first, as in every menu here; a placeholder
("Choose a recipe") shows in the closed control only, not also as a disabled first entry of the list; and typing a
letter does not jump to an option.

### `tests/buttons.spec.ts`

Tests: 5. Covers: unavailable filled actions stay quiet and compact questions keep answer order.

replaced: screenshot goldens `buttons--{light,dark}`

- crates/gui/farhelm-gui/src/session_pane/header_tests.rs `restart_says_why_it_is_unavailable_and_sends_nothing`
  (unavailable filled actions stay quiet and compact questions keep answer order: the unavailable Restart is named and
  helped as unavailable, and a click sends nothing) and crates/gui/farhelm-gui/src/components/tests/button.rs
  `a_busy_button_keeps_its_help_and_may_be_named_for_its_reason` (it stays a Tab stop, since it is drawn busy, not
  disabled, so its reason can be read from the keyboard)
- crates/gui/farhelm-gui/src/components/tests/button.rs
  `a_busy_filled_button_takes_the_retired_look_and_an_unfilled_one_keeps_its_own` (the quiet look of that unavailable
  action in both themes, at rest and under hover: a busy neutral button, the unavailable Restart's state, takes exactly
  the retired colors, hover fill included, in both palettes); the migration's orchestrator decided on 2026-11-16 that
  the goldens cover this look beyond that test: the `buttons` golden draws the retired colors, and the look under
  keyboard focus is the shared focus ring, which only the `focus-ring` golden pins on a button
- crates/gui/farhelm-gui/src/session_list/menu_tests.rs
  `delete_then_enter_asks_about_a_live_session_and_enter_deletes_it` and
  crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs
  `replace_always_asks_and_carries_the_delete_guard_of_its_row` (the row's Delete and Replace questions open with focus
  on Confirm), the `row-delete-confirmation` golden (Cancel before Confirm), and
  crates/gui/farhelm-gui/src/components/tests/dialog.rs
  `a_dialogs_answers_wrap_in_a_narrow_window_in_every_footer_shape` (the answers wrap in a narrow window)
- the `buttons` golden ("normal, neutral, danger, and exempt buttons keep their deliberate tiers": its scene, `buttons`
  in `crates/gui/farhelm-gui/src/goldens/foundations.rs`, draws every tier but the outlined danger one, which the
  `inline-prompt` golden pins at rest)
- `launcher::view_tests::the_footer_keeps_launch_primary_and_reset_neutral` checks the real launcher's painted fills and
  borders in both appearances. Launch is primary when ready, and Reset choices is neutral.
- crates/gui/farhelm-gui/src/components/tests/button.rs `a_heading_button_takes_the_heading_bands_padding` and the
  `session-list` golden (sidebar heading buttons retain their hierarchy and compact sizing)
- crates/gui/farhelm-gui/src/session_list/row_tests.rs
  `the_menu_button_shows_for_hover_focus_its_open_menu_and_selection_and_none_latches` (the row kebab is hidden at rest
  and revealed by hover, focus, selection, or its own open menu)
- "the row kebab stays revealed at rest under Playwright's touch emulation": a recorded deviation of
  [Sidebar and session view](sidebar-and-session-view.md), "Deviations from the old app": touch screens get no
  always-shown button, since GPUI reports no coarse pointer to ask about

### `tests/chrome.spec.ts`

Tests: 3. Covers: the three-level surface ladder renders distinct, correctly grouped levels.

replaced

- screenshot goldens `session-list`, `session-details-bar` and `session-row-menu` (the three-level surface ladder: the
  sidebar and the details bar on the raised level over the ground, and the floating menu)
- selected session identity stays legible while Rename disables its open button: nothing to port, since the GPUI app
  never disables the row under Rename; the layer manager blocks the page behind the dialog instead
  (crates/gui/farhelm-gui/src/layers_tests.rs `the_page_behind_a_dialog_takes_no_clicks_and_empty_space_keeps_focus`),
  so the row draws unchanged
- crates/gui/farhelm-gui/src/components/tests/list_row.rs
  `keyboard_focus_rings_a_row_in_the_accent_and_the_selected_row_in_on_fill_and_a_click_does_not` (keyboard focus uses
  accent on ordinary controls and inverse on selection; a mouse click does not: here the ordinary control is an
  unselected session row, which rings in the accent, the selected row rings in the on-fill ink, and a click shows no
  ring under either click rule; a button's accent ring is pinned by the `focus-ring` golden)

### `tests/interface-fonts.spec.ts`

Tests: 3. Covers: interface and technical values load their own bundled font roles.

partly

- crates/gui/farhelm-gui/src/session_list/row_tests.rs
  `the_directory_is_drawn_in_the_monospace_face_and_the_host_in_the_interface_face` (interface and technical values: a
  row's directory in the monospace face beside its title and its host's name in the proportional interface face)
- crates/gui/farhelm-gui/src/session_list/row_tests.rs `the_ages_and_the_command_line_are_drawn_in_the_monospace_face`
  (the same case: a row's age, and the details bar's command line and age)
- crates/gui/farhelm-gui/src/session_list/row_tests.rs `the_menu_summary_draws_a_commands_program_in_the_monospace_face`
  (the same case: the row menu's summary draws a command session's program in the monospace face)
- crates/gui/farhelm-gui/src/host_text.rs `a_mono_part_is_drawn_in_the_monospace_face` (the shared sentence's monospace
  host parts) and crates/gui/farhelm-gui/src/terminal/tests.rs
  `the_cell_is_the_bundled_fonts_advance_by_its_ascent_plus_descent` (the terminal's bundled face)
- crates/gui/farhelm-gui/src/templates/summary_tests.rs `template_values_are_host_text` (template summaries split
  command fonts: the wrapper prefix and flags as monospace parts beside labels in words, in the Templates dialog and the
  quick switcher alike)
- crates/gui/farhelm-gui/src/templates/dialog_tests.rs `the_number_keys_digits_are_drawn_at_equal_widths` (the same
  case's "shortcut digits keep equal widths": the Templates dialog's number-key chords in the monospace face, each as
  many monospace advances wide as it has characters)
- crates/gui/farhelm-gui/src/trash/dialog/tests.rs `a_known_archive_time_alone_is_drawn_in_the_monospace_face` (the
  second case, archive metadata keeps labels in the interface face and only known timestamps in the monospace face: the
  Trash dialog's "Archived" before a time in the monospace face, and "Archive time unknown" in the interface face)
- crates/gui/farhelm-gui/src/launcher/view_tests.rs
  `the_launchers_paths_and_command_are_monospace_beside_words_in_the_interface_face` (the first case, in the launcher:
  the Folder and Agent command fields in the monospace face, the search field in the interface face, and the search's
  "Use this path" and "Browse this path" rows, words in the interface face and paths in the monospace face)

Not yet ported:

- of the first case, the version readout in the monospace face; belongs to desktop integration
  ([Desktop integration and Linux packaging](desktop-integration-and-linux-packaging.md)), in
  crates/gui/farhelm-gui/src/update_readout/view/tests.rs

The first case's New button in the interface face has no port: the list heading's New is an icon button (SPEC.md,
"Session list"), whose words are its accessible name and hover help, and it paints no text for a face to draw.

"A bundled face loaded" has no case here, since the fonts are built into the app (`assets.rs`). The row's pin glyph is
not measured: no bundled face but the monospace one has it, so the text system draws it from that face whichever face
the row asks for, and a width cannot say which one did; the row asks for the monospace face, as the old stylesheet did.

### `tests/modal-layout.spec.ts`

Tests: 22. Covers: overflowing ${family} keeps vertical margins at width ${width}.

superseded by the shared dialog frame, decided by the migration's orchestrator on 2026-11-16: the six families the old
spec measured one by one (host settings, Templates, Trash, Rename, "Restart with…", which opens the launcher, and the
launcher itself) all draw the one `DialogFrame` (`crates/gui/farhelm-gui/src/components/dialog.rs`), so its margins, its
scrolling body and its body stop are tested once, on the frame: crates/gui/farhelm-gui/src/components/tests/dialog.rs
`a_long_dialog_body_scrolls_and_the_answers_stay_visible`, `a_frame_too_tall_for_the_window_scrolls_as_a_whole`

- crates/gui/farhelm-gui/src/components/tests/dialog.rs `a_dialog_at_its_height_limit_fits_the_linux_content_area` and
  `without_a_frame_the_dialog_limits_are_the_windows` ("overflowing ${family} keeps vertical margins at width ${width}",
  the twelve cases: the frame's height limit leaves a margin above and below it)
- crates/gui/farhelm-gui/src/components/tests/dialog.rs
  `a_dialog_body_has_its_own_stop_whenever_it_holds_nothing_to_focus` and
  `the_scroll_keys_scroll_a_dialog_body_through_its_stop` ("overflowing ${family} scrollports have keyboard entry
  points", the six cases: a body with nothing to focus gets a Tab stop of its own, and the scroll keys scroll it)
- crates/gui/farhelm-gui/src/feedback/tests.rs `after_a_send_the_thanks_has_a_body_stop_of_its_own` ("sent-feedback
  scrollports have keyboard entry points"; [Other dialogs](other-dialogs.md), `tests/feedback.spec.ts`, records it too)
- "host-settings scrollports stay reachable while saving": superseded, since no host settings control goes unavailable
  while a save runs, so the body never loses its last stop ([Settings and hosts](settings-and-hosts.md) records the same
  after its host settings tests)
- crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs `every_text_only_state_gives_the_body_a_stop_that_tab_reaches`
  (text-only SSH help scrollports have keyboard entry points)
- crates/gui/farhelm-gui/src/hosts/remove/tests.rs `the_removal_questions_body_has_a_stop_that_tab_reaches` (remove-host
  scrollports have keyboard entry points)
- crates/gui/farhelm-gui/src/hosts/settings/tests.rs `tab_walks_down_the_body_while_a_destination_save_is_held`
  (host-settings scrollports stay reachable while saving: no control goes unavailable while a save runs, so Tab reaches
  each of the body's controls during a held destination save and scrolls it into view, where the old body took a stop of
  its own once its controls were disabled)

### `tests/modal-trap.spec.ts`

Tests: 5. Covers: the ${mode} switcher wraps search and close, excluding result options.

replaced: crates/gui/farhelm-gui/src/focus_tests.rs `tab_stays_inside_the_topmost_dialog`,
crates/gui/farhelm-gui/src/layers_tests.rs `tab_stays_inside_an_open_menu`

- crates/gui/farhelm-gui/src/session_list/quick_switcher/tests.rs
  `tab_walks_the_search_field_and_close_and_skips_the_rows_in_either_mode` (the sessions and the templates switcher wrap
  search and Close, excluding result options: one test for both modes, under both answers of macOS's Keyboard navigation
  setting, which makes Tab skip Close while it is off)
- crates/gui/farhelm-gui/src/session_list/rename/tests.rs `tab_walks_close_the_field_cancel_and_save_and_wraps` (Rename
  wraps Close through Save: under both answers of macOS's Keyboard navigation setting, which leaves only the field a Tab
  stop while it is off), and crates/gui/farhelm-gui/src/layers_tests.rs `an_approval_card_stays_usable_over_a_dialog`
  (the real approval stays usable meanwhile;
  [Approvals and agent-spawned sessions](approvals-and-agent-spawned-sessions.md) lists that case too)
- crates/gui/farhelm-gui/src/layers_tests.rs `the_page_behind_a_dialog_takes_no_clicks_and_empty_space_keeps_focus`,
  crates/gui/farhelm-gui/src/launcher/view_tests.rs `cancel_and_the_host_button_are_tab_stops` and
  crates/gui/farhelm-gui/src/session_list/order/tests.rs
  `new_opens_the_launcher_and_closing_returns_focus_to_the_button` (ordinary New isolates the page and wraps its
  controls; the late sibling made inert is the browser's DOM, with nothing to port)
- crates/gui/farhelm-gui/src/templates/dialog_tests.rs `a_write_in_flight_leaves_focus_and_the_dialog_alone` and
  crates/gui/farhelm-gui/src/focus_tests.rs `tab_in_a_dialog_of_buttons_stays_put_where_tab_skips_buttons` ("busy
  Templates contains Tab on its root with no enabled descendant": the premise does not arise, since the dialog's
  controls turn busy, not disabled, while a write runs and keep focus, and a dialog none of whose controls Tab reaches
  keeps focus where it is)

### `tests/modal-focus.spec.ts`

Tests: 1. Covers: rename keeps focus in its field when the selected tab goes away.

replaced: crates/gui/farhelm-gui/src/focus_tests.rs
`focus_goes_home_when_nothing_holds_it_and_never_takes_it_from_a_control` (home never takes focus from a control)

- crates/gui/farhelm-gui/src/focus_tests.rs `choosing_a_terminal_while_a_dialog_is_open_leaves_focus_in_the_dialog` (a
  terminal chosen while a dialog is open leaves focus in the dialog)
- crates/gui/farhelm-gui/src/terminal/set/tests.rs `a_reaped_tab_leaves_focus_on_an_open_header_question` (a reaped tab
  leaves focus on an open header question, the case's twin)
- crates/gui/farhelm-gui/src/terminal/set/tests.rs `a_reaped_tab_leaves_focus_in_an_open_rename_field` (the case itself:
  with a shell tab chosen and Rename open from the row menu with a draft typed, the tab's reap chooses the agent
  terminal and leaves focus in the title field; further typing lands in the draft that Enter saves, and the agent
  terminal gets no key)

### `tests/keyboard-scroll.spec.ts`

Tests: 19. Covers: help menu keeps keyboard focus through a late terminal reveal.

partly

- crates/gui/farhelm-gui/src/focus_tests.rs `focus_control_moves_focus_unless_the_user_moved_on_or_a_layer_is_open`,
  `a_chosen_terminal_that_arrives_after_the_user_moved_on_only_becomes_home` (help menu keeps keyboard focus through a
  late terminal reveal)
- crates/gui/farhelm-gui/src/hosts/settings/tests.rs `the_destination_saves_as_typed_and_a_refusal_stays_in_place` (host
  settings keep usable focus after a refused destination save)
- crates/gui/farhelm-gui/src/layers_tests.rs
  `focus_on_an_approval_card_outlives_the_dialog_and_goes_home_with_the_card`,
  `an_approval_card_stays_usable_over_a_dialog` (SSH command arrival preserves an independent approval owner)
- crates/gui/farhelm-gui/src/feedback/tests.rs `after_a_send_the_thanks_has_a_body_stop_of_its_own`,
  crates/gui/farhelm-gui/src/components/tests/dialog.rs
  `a_dialog_body_has_its_own_stop_whenever_it_holds_nothing_to_focus` (sent feedback is keyboard-scrollable without
  changing the editing Tab order)
- crates/gui/farhelm-gui/src/feedback/tests.rs
  `tab_walks_the_dialog_while_a_send_is_out_and_a_refusal_gives_the_fields_back` (sending feedback gains a body stop and
  refusal restores editor entry: the stop's premise does not arise, since the fields stay focusable and read-only while
  a send is out, and so remain the body's way in; Tab walks the dialog meanwhile from the busy Send, which keeps focus
  through the refusal, and after it Shift+Tab reaches the message, which takes keys again, and Tab goes on to the
  contact)
- crates/gui/farhelm-gui/src/components/tests/scroll_box.rs
  `an_approvals_complete_initial_prompt_is_keyboard_scrollable` (an approval's complete initial prompt is
  keyboard-scrollable)
- crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs `every_text_only_state_gives_the_body_a_stop_that_tab_reaches` (SSH
  help remains keyboard-scrollable with loading, error, no hosts and no commands)
- crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs `a_long_command_wraps_inside_the_dialog` (long SSH commands wrap
  while Copy preserves their exact bytes; the bytes are pinned by
  `the_menu_entry_shows_each_hosts_two_commands_and_copies_their_exact_bytes`)
- crates/gui/farhelm-gui/src/hosts/remove/tests.rs `the_removal_questions_body_has_a_stop_that_tab_reaches` (the
  remove-host explanation is keyboard-scrollable)
- crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs `the_commands_arriving_leave_focus_usable` (SSH command arrival
  keeps usable focus from body, dismiss and backdrop; from the body, focus goes to the closest remaining control, the
  Sessions Copy, where the old app went to the footer Close, and a veil press keeps Close focused, where the old
  backdrop click left nothing focused, both as SPEC.md, "Opening and closing", has it)
- crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs `a_reply_to_a_closed_opening_leaves_a_reopened_one_alone` (SSH help
  closed before its reply leaves a reopened region independent)
- "host settings remain keyboard-scrollable while every setting is saving", "host settings keep usable focus after a
  successful checkbox save" and "host settings automatic busy recovery remembers the commands checkbox": superseded,
  decided by the migration's orchestrator on 2026-11-16. All three rest on the old dialog disabling every control while
  a save ran, which took focus away and needed the body stop and the recovery; no host settings control goes unavailable
  during a save ([Settings and hosts](settings-and-hosts.md) records the same), so a save never disables the control
  that holds focus. The saves themselves are crates/gui/farhelm-gui/src/hosts/settings/tests.rs
  `escape_while_a_save_is_in_flight_keeps_the_field_open`,
  `a_press_during_another_save_on_the_host_says_why_it_sent_nothing`,
  `saves_on_the_host_wait_for_each_other_without_locking_the_folder` and
  `each_permission_saves_its_own_route_and_field`. The keyboard paths the old cases drove are
  `tab_walks_down_the_body_while_a_destination_save_is_held` (Tab walks the body's controls down to its last switch
  while a save is held, each scrolled into view) and `a_space_toggled_switch_keeps_focus_through_its_save` (a switch
  stays live through its save, so focus never leaves it and Tab goes on to the next switch; the refused destination half
  is `the_destination_saves_as_typed_and_a_refusal_stays_in_place`)
- "sending feedback gains a body stop and refusal restores editor entry", its body stop: superseded, decided by the
  migration's orchestrator on 2026-11-16. The old dialog disabled its controls while a send was out, and the body stop
  kept it scrollable; here only Cancel, the header Close and the "Re-use for future feedback" checkbox are disabled,
  while the message and contact fields turn read-only and Send busy, so focus always has a control to stay on
  (crates/gui/farhelm-gui/src/feedback/tests.rs `nothing_closes_the_dialog_or_sends_again_while_a_send_is_out`, in which
  a click focuses the read-only message field while the send is out)

The same case's refusal half, where a refused send puts focus back in the message field itself, is replaced by
crates/gui/farhelm-gui/src/feedback/tests.rs `a_refused_send_clicked_on_a_mac_gives_focus_back_to_the_message` (a
pointer send under the macOS click rule leaves focus on the dialog, and the refusal moves it to the message) and
`a_refused_send_leaves_focus_on_the_send_that_holds_it` (focus already on a control stays there); with a keyboard send,
`tab_walks_the_dialog_while_a_send_is_out_and_a_refusal_gives_the_fields_back` (listed above) keeps focus on the busy
Send, and `a_failed_send_says_so_and_keeps_the_text_for_a_retry` checks the text. The dialog is other dialogs'
([Other dialogs](other-dialogs.md)).

### `tests/tooltip.spec.ts`

Tests: 18. Covers: a session's title and open area share its summary card.

replaced: crates/gui/farhelm-gui/src/components/tests/hover_help.rs tooltip tests; the session-row cases are in the
sidebar's parity file

- the nine session-row cases, the eight from "a session's title and open area share its summary card" to "moving to the
  next mark while a tooltip shows replaces it at once" (the version case between them aside) and the superseded "a card
  target clones its hidden source into the shared tooltip": [Sidebar and session view](sidebar-and-session-view.md),
  `tests/tooltip.spec.ts` (its session-row cases)
- crates/gui/farhelm-gui/src/components/tests/hover_help.rs
  `pointer_help_at_the_top_edge_sits_below_its_control_clear_of_the_pointer` (the version at the top edge gets its
  tooltip below, clear of the cursor: pointer help is placed against its control, centered above it, or 28 pixels below
  it when there is no room above, as help shown for keyboard focus already was)
- crates/gui/farhelm-gui/src/components/tests/hover_help.rs
  `moving_inside_a_control_while_its_help_waits_starts_the_wait_again` and
  `a_pointer_moving_over_a_control_leaves_a_wait_keyboard_focus_began` (the delay restarts while the pointer keeps
  moving inside a target, and only for a wait the pointer began)
- crates/gui/farhelm-gui/src/components/tests/hover_help.rs
  `a_press_puts_a_controls_help_away_while_the_pointer_rests_on_it` and
  `a_press_in_the_same_update_as_the_move_onto_a_mark_puts_the_marks_help_away` (a press or Escape dismisses the tooltip
  until the pointer leaves: the press half; the Escape half is a deviation, below)
- crates/gui/farhelm-gui/src/components/tests/hover_help.rs
  `keyboard_focus_shows_a_controls_help_after_the_delay_and_a_click_does_not` (keyboard focus shows the focused
  control's tooltip)
- crates/gui/farhelm-gui/src/components/tests/hover_help.rs
  `focus_help_goes_with_focus_and_escape_keeps_it_down_until_focus_moves` (an Escape dismissal ends when focus leaves
  the control)
- touch input never shows a tooltip, and the page carries no native title tooltips: nothing to port, since the app takes
  no touch input, and GPUI draws no native tooltips while every control constructor takes its hover help
  (`tests/tooltip-coverage.spec.ts` below)
- quiet focus survives asset reinstall and allows the first later hover: the reinstall was the browser reloading its
  scripts, with nothing to port; quiet focus itself is a recorded deviation
  (crates/gui/farhelm-gui/src/layers/tooltip.rs, the second NOTE: every keyboard landing shows help)

"A card follows only its marked step list after every clone" is, in the GPUI app, the host progress card keeping its
current step in view. crates/gui/farhelm-gui/src/hosts/progress/card/tests.rs
`a_short_window_previews_only_diagnostics_that_cannot_fit` draws the current step whole in a 300-pixel window
([Settings and hosts](settings-and-hosts.md) records it for "An open Update card follows the new current step in a short
viewport"), and `the_current_step_stays_whole_as_the_run_moves_it` keeps it whole after every clone, in a window too
short for its steps, as the helm moves that step from late in the run to early. The unmarked list was the old tooltip's
own markup, with nothing to port.

Deviation, decided by the migration's orchestrator on 2026-11-16: after Escape puts a control's help away, the next
pointer move over the control brings it back after the delay, where the old app kept it down until the pointer left the
control. GPUI ends every hover on a key press, so the move after Escape begins a new hover, as the pointer arriving
would. crates/gui/farhelm-gui/src/layers/tooltip.rs's first NOTE lists it with the other ways a dismissal differs.

### `tests/tooltip-coverage.spec.ts`

Tests: 3. Covers: template launcher rows and search carry hover help.

superseded: the component constructors (`Button::new`, `TextInput::new`, `Checkbox::new`, the radio group's and the tab
strip's) require hover help, so a control without it does not compile, and `crates/gui/clippy.toml` bans GPUI's and GPUI
Kit's own tooltip methods. Nothing is left to check at run time. The template cases' words are tested in
[Templates and quick launch](templates-and-quick-launch.md), `tests/tooltip-coverage.spec.ts`.

### `tests/popover-movement.spec.ts`

Tests: 18. Covers: Sort dismisses an open ${kind} before Compact moves its anchor.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); the case that remains, a menu
whose session or anchor is gone closing, is ported by the tests listed at the top of this file

- crates/gui/farhelm-gui/src/session_list/menu_tests.rs `an_open_menu_follows_its_button_through_what_the_helm_pushes`
  (a keyboard-opened row menu stays open and follows its button through a pushed reorder that moves its row down or up
  by changing its index, and stays in place through a permutation wholly above it; the two refusal-crossing cases and
  the prefix-permutation case, for the row menu)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `an_open_row_menu_stays_open_and_follows_its_button_when_the_band_appears` (the crossing cases' own movement, content
  above a row that stays at its index growing and moving its button, which an open row menu follows)
- crates/gui/farhelm-gui/src/notices_bell/tests.rs `an_open_list_follows_its_bell_through_what_changes_underneath` (the
  bell cases, for a row's notification list opened by its keys: the row above rewrapping its ended line moves the bell
  down and the list follows it, which is also what a refusal or a stale cause line growing on that row does; a change
  compact hides moves nothing; a pushed reorder that moves the bell's row down or up across a row above it moves the
  list with it, and a permutation wholly above it moves nothing)

The held served reorder with the row's notification list open is a recorded deviation: the list does not keep the order
hold ([Sidebar and session view](sidebar-and-session-view.md), "Deviations from the old app").

### `tests/popover-header-movement.spec.ts`

Tests: 10. Covers: a completed Delete notice moves the anchor and closes an open ${kind}.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); the case that remains, a menu
whose session or anchor is gone closing, is ported by the tests listed at the top of this file

- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `an_open_row_menu_stays_open_and_follows_its_button_when_the_band_appears` (the band's appearance moves an open row
  menu with its button and leaves it open, with focus on its command)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `a_delete_whose_checkout_was_not_archived_shows_the_helms_words_in_the_band` (the band a completed Delete's notice
  inserts)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `a_later_notice_replaces_the_band_and_one_with_nothing_to_report_keeps_it` (a Delete with no notice keeps the band)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `dismiss_takes_the_band_away_and_focus_goes_home_to_the_open_sessions_terminal` (Dismiss takes the band away; with no
  menu open)
- crates/gui/farhelm-gui/src/session_list/menu_tests.rs `an_open_menu_follows_its_button_through_what_the_helm_pushes`
  (another row's removal leaves an open row menu open and in place, the menu part of "a Delete with no notice"; a failed
  listing keeps the rows and the menu, which follows its button down past the failure line and back up when the next
  listing is read, inverting "recovering from a failed listing closes an open menu")
- crates/gui/farhelm-gui/src/notices_bell/tests.rs `an_open_list_follows_its_bell_through_what_changes_underneath` (the
  bell cases: an open notification list stays open and follows its bell when a completed Delete's notice brings the
  band, when the listing is cut and its count becomes a warning line, and through a failed listing and its recovery;
  another row's removal moves nothing. Dismissing the band under an open list is unreachable: a press on Dismiss is a
  press outside the list, which closes it, and Tab out of the list closes it too)

### `tests/popover-host-movement.spec.ts`

Tests: 8. Covers: a concealed same-height host reorder leaves the session ${kind} open.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); the case that remains, a menu
whose session or anchor is gone closing, is ported by the tests listed at the top of this file

- crates/gui/farhelm-gui/src/session_list/menu_tests.rs `an_open_menu_follows_its_button_through_what_the_helm_pushes`
  (a pushed host registry reorder leaves an open session row menu open and in place)
- crates/gui/farhelm-gui/src/hosts/menu/tests.rs `an_open_menu_follows_its_row_and_closes_when_its_host_goes` (the six
  host menu cases, the two that closed the menu inverted: an open host menu stays open and moves with its toggle when a
  registry reorder moves its row down, and stays in place through a reorder below it and a permutation above it; it
  moves with its toggle when the details of a row above it grow as a row below shrinks, and stays in place when the rows
  wholly above or wholly below it trade the lengths of their details; and the remaining case, a host menu closing when
  the registry stops listing its host)
- crates/gui/farhelm-gui/src/notices_bell/tests.rs `an_open_list_follows_its_bell_through_what_changes_underneath` (the
  same for an open notification list, "a concealed same-height host reorder leaves the session bell open" among them)

### `tests/popover-expanded-host-movement.spec.ts`

Tests: 6. Covers: expanded progress growth moves and closes the lower host menu.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); the case that remains, a menu
whose session or anchor is gone closing, is ported by the tests listed at the top of this file

- crates/gui/farhelm-gui/src/layers_tests.rs `a_popover_follows_its_anchor_and_closes_when_the_anchor_is_gone` (a host
  menu is a popover of the layer manager, so it follows its button when growth above moves it, and closes only when the
  button is gone)
- crates/gui/farhelm-gui/src/hosts/menu/tests.rs `an_open_menu_follows_its_row_and_closes_when_its_host_goes` (all six
  cases, the four that closed the menu inverted: an open host menu moves with its toggle when a running Update's step
  message grows in a row above it, also with the rows kept under a failed registry read's line, when the local host's
  row above it grows, and when the helm's late refusal of a save under its host's key (as a closed settings dialog's
  would be) shows under a row above it; it stays in place when a run below it only moves on a step, and when a finished
  run above it changes only what the row does not draw: its run id, a step's id, paths and explanation)

The old local case grew the manual setup command under the local row. The GPUI row shows the helm's dial failure and a
fixed remedy instead (the settings and hosts area's decision of 2026-11-14, `hosts.rs` module docs), so the line that
grows there is the dial failure.

### `tests/popover-identity-movement.spec.ts`

Tests: 9. Covers: pin wrapping above closes the lower actions menu.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); the case that remains, a menu
whose session or anchor is gone closing, is ported by the tests listed at the top of this file

- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `an_open_row_menu_stays_open_and_follows_its_button_when_the_band_appears` and
  crates/gui/farhelm-gui/src/layers_tests.rs `a_popover_follows_its_anchor_and_closes_when_the_anchor_is_gone` (content
  above a row growing, as a wrapping pinned title or a growing activity track does, moves an open row menu with its
  button and leaves it open)
- crates/gui/farhelm-gui/src/session_list/seen_tests.rs
  `the_menu_item_follows_the_seen_state_and_sends_the_stamp_or_null` (pin wrapping preserves its own actions menu: a
  push to the menu's own row keeps the menu open and relabels its command)
- crates/gui/farhelm-gui/src/session_list/attention/tests.rs
  `menus_below_close_when_the_list_moves_and_stay_through_progress` and
  crates/gui/farhelm-gui/src/session_list/menu_tests.rs `an_open_menu_follows_its_button_through_what_the_helm_pushes`
  (pin wrapping below, compact pin, one identity contribution exchange wholly above, and the same displayed title with a
  new raw spelling: a push that moves nothing leaves the menu open and in place, as does a permutation above it)
- unpinned delete style, with compact off and on: nothing to port, since the header's Delete cannot start while a row
  menu is open: a press on it is a press outside the menu, which closes it first, and the Delete shortcut does nothing
  over a menu (crates/gui/farhelm-gui/src/session_list/menu_tests.rs
  `the_shortcut_does_nothing_with_no_session_open_or_a_menu_open`); the old test focused the header's button from script

### `tests/popover-template-movement.spec.ts`

Tests: 6. Covers: rename ${moves ? "closes a displaced" : "keeps an unchanged"} template number popup.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); the case that remains, a menu
whose session or anchor is gone closing, is ported by the tests listed at the top of this file

- crates/gui/farhelm-gui/src/templates/dialog_tests.rs `the_key_menu_follows_its_button_through_what_the_helm_pushes`
  (the six cases, inverted: a pushed ⚡ transfer or rename that moves the number-key menu's button leaves the menu open
  and moves it with the button)
- crates/gui/farhelm-gui/src/templates/dialog_tests.rs `the_key_menu_closes_when_its_template_is_gone` (the remaining
  case; [Templates and quick launch](templates-and-quick-launch.md), `tests/popover-template-movement.spec.ts`, records
  the same)

### `tests/popover-width-movement.spec.ts`

Tests: 8. Covers: the actual activity clock moves a 9m bell at 10m and closes it.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); the case that remains, a menu
whose session or anchor is gone closing, is ported by the tests listed at the top of this file

- crates/gui/farhelm-gui/src/layers_tests.rs `a_popover_follows_its_anchor_and_closes_when_the_anchor_is_gone` (a bell's
  notification list and a row menu are popovers of the layer manager, so a growing age or a Trash count that wraps the
  header moves them with their buttons and leaves them open)
- crates/gui/farhelm-gui/src/notices_bell/tests.rs `an_open_list_follows_its_bell_through_what_changes_underneath` (the
  bell cases: the clock turning the bell's own age from 9m to 10m moves the bell left and its open list with it; 10m to
  11m, as wide, and another row's age widening move nothing. The Trash count's bell cases are the heading above the rows
  growing and a redraw that moves nothing, which the same test makes with the cut listing's warning line and the steps
  that move nothing, not with a Trash count)

### `tests/shell-scroll.spec.ts`

Tests: 1. Covers: a wheel over the header or the sidebar edge never scrolls the document.

superseded, decided by the migration's orchestrator on 2026-11-16: the GPUI window's root is not a scroll container, so
there is no document for a wheel to scroll, and the old bug (absolutely positioned screen-reader text growing the page)
cannot arise. No test replaces it.

### `tests/feed.spec.ts`

Tests: 11. Covers: the invalidation feed.

superseded by the UI channel (SPEC_impl "The UI channel")

- crates/gui/farhelm-gui/src/main_view_tests.rs `an_idle_window_sends_the_helm_nothing` (a healthy feed performs no
  periodic reads on the list, with a session view open, and with a stale session view open: one test, in which an idle
  window with no session open, with a session open, and with a session on an unreachable host open sends the helm
  nothing for ten minutes)
- crates/gui/farhelm-gui/src/session_list/tests.rs `rows_follow_the_pushed_order_and_the_selection_follows_its_session`
  ("reader snapshots retain queued work and follow session selection": there are no reader snapshots, and the pushed
  rows and the selection follow their sessions)
- crates/farhelm-helm/src/ui_channel_tests.rs `taking_the_seat_displaces_the_holder_and_its_release_keeps_the_successor`
  ("a rename in one client appears in another without a refresh": superseded by SPEC.md, "One GUI at a time", since a
  helm serves one window), and `a_preference_write_outside_the_channel_reaches_an_idle_connected_ui`,
  `a_template_saved_outside_the_channel_reaches_an_idle_connected_ui`,
  `dismissing_an_announcement_outside_the_channel_moves_it_to_history_on_the_ui` and
  `marking_a_notice_read_outside_the_channel_reaches_an_idle_connected_ui` (a write made outside the window's channel
  still reaches it unasked)
- crates/farhelm-helm-client/tests/real_helm.rs `a_dropped_connection_reconnects_with_a_consistent_snapshot` and
  crates/farhelm-helm/src/ui_channel_tests.rs `the_snapshot_on_open_holds_every_collection_as_served` ("a dead feed
  falls back to polling and recovers on its own" and "a mutation that lands while the socket is down is seen at the
  handshake": there is no polling fallback, since SPEC.md, "Losing the connection to the helm", resets the window
  instead, and the reconnect brings a whole fresh snapshot, so a write made during the outage is in it; the second test
  pins that a snapshot holds every collection as served)
- crates/farhelm-helm/src/ui_channel_tests.rs `a_feed_bump_pushes_changes_and_a_failed_read_removes_nothing` and
  `a_provisioning_step_transition_reaches_the_connected_ui` (a status transition arrives through the feed without a
  refresh)
- "build skew stops the feed and the fallback poll both", "a skewed page still reads after an explicit host choice" and
  "a page that latched skew before the feed asset loaded never subscribes": superseded, decided by the migration's
  orchestrator on 2026-11-16. The desktop app runs its own embedded helm from the same binary, so a window and a helm of
  different builds cannot meet, and the window checks no build (SPEC_impl.md, "Version and skew"). The same goes for the
  old build-skew banner, whose reference scene `build-skew-banner` in `docs/gpui/screenshot-scenes.json` has no picture
  to port; the scene list records only whether a scene is ported, so the batch page still lists it as not yet ported.

### `tests/readers.spec.ts`

Tests: 2. Covers: read ordering and recovery.

superseded by the UI channel's ordered pushes; its remaining acceptance case, a failed listing that keeps its rows, says
so and clears once the next listing is read, is ported

- crates/gui/farhelm-gui/src/session_list/tests.rs `a_failed_listing_keeps_its_rows_and_says_so_until_the_next_listing`
  ("a failed stop refetch is handed to the reader, which brings the list back"; the handoff itself has no counterpart,
  since the helm pushes the listing)

### `tests/listing-mutation-fence.spec.ts`

Tests: 2. Covers: a pre-${operation} listing cannot close the newly selected session.

superseded by the UI channel's serial admission: the window reads no listings, and every change a command causes reaches
it before the command's reply (SPEC.md, "Losing the connection to the helm"), so a listing from before a create or a
replace cannot arrive after it. The remaining case, the selection leaving a session only once a complete listing shows
it gone, is ported.

- crates/farhelm-helm/src/ui_channel_tests.rs `a_command_waits_for_the_one_before_it_while_heartbeats_are_answered` and
  `the_changes_a_command_causes_arrive_before_its_reply` (the helm admits one command at a time and sends its changes
  before its reply)
- crates/farhelm-helm-client/src/channel/tests.rs `a_reply_resolves_only_after_the_changes_sent_before_it` and
  crates/farhelm-helm-client/tests/real_helm.rs `a_command_s_changes_are_applied_before_its_reply_resolves` (the client
  applies those changes before it resolves the reply, against a fake and against the real helm)
- crates/gui/farhelm-gui/src/session_list/tests.rs `a_selection_moves_only_once_a_complete_listing_shows_it_gone` (the
  open session is dropped only when a complete listing leaves it out, never on a truncated one)

### `tests/m6-5-debts.spec.ts`

Tests: 4. Covers: the M6.5 test debts.

superseded by the UI channel for its first three cases, and replaced for the fourth

- "a listing read older than a stop cannot resurrect the pre-stop status", "a detail read older than a restart cannot
  describe the previous run" and "a detail read that starts during a restart cannot describe the previous run": the
  window issues no reads, so a stale read has nothing to overwrite; the ordering they guarded is pinned by the tests
  listed under `tests/listing-mutation-fence.spec.ts` above
- crates/gui/farhelm-gui/src/host_text.rs `every_spoofing_string_is_shown_escaped_and_kept_raw_for_editing`,
  `a_drawn_mono_part_is_escaped_and_isolated_as_a_host_part` and
  `a_right_to_left_host_part_reorders_only_within_its_isolate`, and the `host-text` golden ("a peer value renders
  isolated in its own element with its controls escaped": host-supplied text is drawn escaped and isolated by the shared
  host-text sentence)

### `tests/auth.spec.ts`

Tests: 6. Covers: an unauthenticated browser gets an in-page token prompt.

dropped: browser sign-in removed (#3401)

### `tests/scratch.spec.ts`

Tests: 1. Covers: scratch dirs land in the stack state dir and keep their prefix.

test infrastructure: a helper of the old suite; no port

### `tests/real-agent.spec.ts`

Tests: 8. Covers: the real-agent helper's pacing and marker-waiting work against the fake agent.

test infrastructure: a helper of the old suite; scripts/gui-agent.py seeds fake agents

### `tests/f20_visual_capture.spec.ts`

Tests: 1. Covers: F20 visual capture matrix.

replaced by scripts/gui-screenshot-batch.py

### `docs-shots/add-a-remote-host.spec.ts`

Tests: 4. Covers: add button.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/first-session.spec.ts`

Tests: 3. Covers: this machine.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/launch-templates.spec.ts`

Tests: 6. Covers: templates button.

docs media: rebuilt on the GPUI tooling at cutover (M4)

- On the test Mac (2026-11-17) both Launch templates shots reached the size check: the window fitted to 1600×972 points
  at 2×, yielding 3200×1944 pixels against the required 3200×2000, and the runner refused both shots.

### `docs-shots/manage-hosts.spec.ts`

Tests: 5. Covers: host menu.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/release-notes.spec.ts`

Tests: 70. Covers: v0.38.0 container launch choice.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/session-list.spec.ts`

Tests: 6. Covers: a row.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/start-a-session.spec.ts`

Tests: 12. Covers: new button.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/stop-restart-resume.spec.ts`

Tests: 2. Covers: header actions.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/visual-review.spec.ts`

Tests: 4. Covers: ${colorScheme} ${width === "main-650" ? "window-650" : width}.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/work-in-a-session.spec.ts`

Tests: 3. Covers: header.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `readme-hero/capture.spec.ts`

Tests: 1. Covers: capture the README hero.

docs media: rebuilt on the GPUI tooling at cutover (M4)

- Captured on the test Mac (2026-11-17): the 1280×800-point window rendered at 2× and the runner fetched a 2560×1600
  PNG.

### `readme-video/capture.spec.ts`

Tests: 1. Covers: record the README demo video.

docs media: rebuilt on the GPUI tooling at cutover (M4)
