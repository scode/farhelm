# Parity: Foundations (M1)

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Keyboard focus, Things changing underneath, Losing the connection to the helm, One GUI at a time; and
SPEC_impl.md, GUI architecture.

The popover-movement specs asserted that a menu closes when its anchor moves. SPEC.md now says an open menu moves with
its session, so those rows are superseded. One closing rule still stands: changes in the Hosts attention list retire
session menus anchored below it (SPEC.md, "Session list"); that case belongs to the attention-list row of
`tests/host-updates.spec.ts` in [Sidebar and session view](sidebar-and-session-view.md).

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

partly: screenshot goldens `buttons--{light,dark}`

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

- crates/gui/farhelm-gui/src/session_list/row_tests.rs `the_directory_is_drawn_in_the_monospace_face` (interface and
  technical values: a row's directory in the monospace face beside its title in the proportional interface face)
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

Not yet ported:

- of the first case, the row's host name and the list heading's New button in the interface face (this area's); the
  launcher's search field in the interface face, its Folder and Agent command fields in the monospace face, and its
  search's "Use this path" and "Browse this path" rows, words in the interface face and paths in the monospace face (the
  launcher area's); and the version readout (desktop integration's `update_readout/view/tests.rs`, pending there)
- the second case, the Trash dialog's archive times (other dialogs')
- of the third, the number keys' equal-width digits (templates and quick launch's)

"A bundled face loaded" has no case here, since the fonts are built into the app (`assets.rs`). The row's pin glyph is
not measured: no bundled face but the monospace one has it, so the text system draws it from that face whichever face
the row asks for, and a width cannot say which one did; the row asks for the monospace face, as the old stylesheet did.

### `tests/modal-layout.spec.ts`

Tests: 22. Covers: overflowing ${family} keeps vertical margins at width ${width}.

partly: crates/gui/farhelm-gui/src/components/tests/dialog.rs `a_long_dialog_body_scrolls_and_the_answers_stay_visible`,
`a_frame_too_tall_for_the_window_scrolls_as_a_whole`

- crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs `every_text_only_state_gives_the_body_a_stop_that_tab_reaches`
  (text-only SSH help scrollports have keyboard entry points)
- crates/gui/farhelm-gui/src/hosts/remove/tests.rs `the_removal_questions_body_has_a_stop_that_tab_reaches` (remove-host
  scrollports have keyboard entry points)

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
  crates/gui/farhelm-gui/src/focus_tests.rs `tab_in_a_dialog_of_buttons_stays_put_where_tab_skips_buttons` (busy
  Templates contains Tab with no enabled control: the premise does not arise, since the dialog's controls turn busy, not
  disabled, while a write runs and keep focus, and a dialog none of whose controls Tab reaches keeps focus where it is)

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

### `tests/tooltip.spec.ts`

Tests: 18. Covers: a session's title and open area share its summary card.

partly: crates/gui/farhelm-gui/src/components/tests/hover_help.rs tooltip tests; the session-row cases are in the
sidebar's parity file

- the nine session-row cases, from "a session's title and open area share its summary card" to "moving to the next mark
  while a tooltip shows replaces it at once" and the superseded "a card target clones its hidden source into the shared
  tooltip": [Sidebar and session view](sidebar-and-session-view.md), `tests/tooltip.spec.ts` (its session-row cases)
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

Not yet ported: "a card follows only its marked step list after every clone", which in the GPUI app means the host
progress card keeping the current step whole as the run moves on; it is the settings and hosts area's, in
`hosts/progress/card/tests.rs`.

Deviation, decided by the migration's orchestrator on 2026-11-16: after Escape puts a control's help away, the next
pointer move over the control brings it back after the delay, where the old app kept it down until the pointer left the
control. GPUI ends every hover on a key press, so the move after Escape begins a new hover, as the pointer arriving
would. crates/gui/farhelm-gui/src/layers/tooltip.rs's first NOTE lists it with the other ways a dismissal differs.

### `tests/tooltip-coverage.spec.ts`

Tests: 3. Covers: template launcher rows and search carry hover help.

replaced: component constructors require hover help (a missing one does not compile)

### `tests/popover-movement.spec.ts`

Tests: 18. Covers: Sort dismisses an open ${kind} before Compact moves its anchor.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

- crates/gui/farhelm-gui/src/session_list/menu_tests.rs `an_open_menu_follows_its_button_through_what_the_helm_pushes`
  (a keyboard-opened row menu stays open and follows its button through a pushed reorder that moves its row down or up
  by changing its index, and stays in place through a permutation wholly above it; the two refusal-crossing cases and
  the prefix-permutation case, for the row menu)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `an_open_row_menu_stays_open_and_follows_its_button_when_the_band_appears` (the crossing cases' own movement, content
  above a row that stays at its index growing and moving its button, which an open row menu follows)

The held served reorder with the row's notification list open is a recorded deviation: the list does not keep the order
hold ([Sidebar and session view](sidebar-and-session-view.md), "Deviations from the old app").

### `tests/popover-header-movement.spec.ts`

Tests: 10. Covers: a completed Delete notice moves the anchor and closes an open ${kind}.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

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

### `tests/popover-host-movement.spec.ts`

Tests: 8. Covers: a concealed same-height host reorder leaves the session ${kind} open.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

- crates/gui/farhelm-gui/src/session_list/menu_tests.rs `an_open_menu_follows_its_button_through_what_the_helm_pushes`
  (a pushed host registry reorder leaves an open session row menu open and in place)

### `tests/popover-expanded-host-movement.spec.ts`

Tests: 6. Covers: expanded progress growth moves and closes the lower host menu.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-identity-movement.spec.ts`

Tests: 8. Covers: pin wrapping above closes the lower actions menu.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-template-movement.spec.ts`

Tests: 1. Covers: rename ${moves ? "closes a displaced" : "keeps an unchanged"} template number popup.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-width-movement.spec.ts`

Tests: 6. Covers: the actual activity clock moves a 9m bell at 10m and closes it.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/shell-scroll.spec.ts`

Tests: 1. Covers: a wheel over the header or the sidebar edge never scrolls the document.

not yet ported

### `tests/feed.spec.ts`

Tests: 11. Covers: the invalidation feed.

superseded by the UI channel (SPEC_impl "The UI channel")

- crates/gui/farhelm-gui/src/main_view_tests.rs `an_idle_window_sends_the_helm_nothing` (a healthy feed performs no
  periodic reads on the list, with a session view open, and with a stale session view open: one test, in which an idle
  window with no session open, with a session open, and with a session on an unreachable host open sends the helm
  nothing for ten minutes)

### `tests/readers.spec.ts`

Tests: 2. Covers: read ordering and recovery.

superseded by the UI channel's ordered pushes; its remaining acceptance case, a failed listing that keeps its rows, says
so and clears once the next listing is read, is ported

- crates/gui/farhelm-gui/src/session_list/tests.rs `a_failed_listing_keeps_its_rows_and_says_so_until_the_next_listing`
  ("a failed stop refetch is handed to the reader, which brings the list back"; the handoff itself has no counterpart,
  since the helm pushes the listing)

### `tests/listing-mutation-fence.spec.ts`

Tests: 1. Covers: a pre-${operation} listing cannot close the newly selected session.

superseded by serial admission (helm ui_channel tests); verify the case

### `tests/m6-5-debts.spec.ts`

Tests: 4. Covers: the M6.5 test debts.

superseded by the UI channel; verify each case

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

Tests: 2. Covers: ${colorScheme} ${width === "main-650" ? "window-650" : width}.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/work-in-a-session.spec.ts`

Tests: 3. Covers: header.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `readme-hero/capture.spec.ts`

Tests: 1. Covers: capture the README hero.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `readme-video/capture.spec.ts`

Tests: 1. Covers: record the README demo video.

docs media: rebuilt on the GPUI tooling at cutover (M4)
