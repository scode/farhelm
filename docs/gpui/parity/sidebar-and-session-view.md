# Parity: Sidebar and session view

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Session list, Status, Topology, Session view, Lifecycle operations.

The area covers the session list, the quick switcher, and the session view's chrome around the terminal: the header's
Stop, Restart, Replace, Clone and Delete, the details bar, and the interrupted and unreachable-host cards (the tab
strip, text size and tunnel pill in the same row are the terminal area's). The host rail's entries stay here too, since
the rail is this area's later work.

The Hosts attention list above the session list is this area's as well, Always update included, so the attention-list
cases of `tests/host-updates.spec.ts` sit here: its grouping, opening a host from it, dismissal, scrolling, Always
update, and how its changes treat an open session menu. That spec's host progress card, Update all and single Update
cases are in [Settings and hosts](settings-and-hosts.md), which names them. Likewise the quick switcher case of
`tests/host-picker.spec.ts` ("quick switcher draws a session's chosen host mark") is here, and its host picker cases are
in [Launcher](launcher.md).

The rail's host status follows the old stylesheet: an avatar's update badge is amber for an optional and a required
update alike ("The rail keeps its existing amber arrows; urgency is specific to this toggle"), and only the Hosts
control's `↑` turns red when some update is required.

### `tests/sidebar.spec.ts`

Tests: 116. Covers: the sidebar app bar explains the helm and window builds.

partly: crates/gui/farhelm-gui/src/session_list/tests.rs
(`rows_follow_the_pushed_order_and_the_selection_follows_its_session`,
`a_click_opens_a_session_remembers_it_and_marks_it_seen_once`, `an_empty_fleet_says_so_in_the_list_and_the_pane`,
`the_count_line_says_n_or_n_of_m_and_a_cut_list_warns`, `the_open_sessions_list_item_says_it_is_selected`, the pulse
tests), crates/gui/farhelm-gui/src/session_list/menu_tests.rs
`the_menu_opens_below_its_button_holds_focus_and_leaves_the_selection`; scripts/gui-e2e-smoke.py
`test_01_the_list_shows_every_seeded_session`, `test_03_a_rows_menu_opens_from_its_button_and_escape_closes_it`; the
seen-state cases: `an idle session that was never opened reads unseen, and opening it clears that` (tests.rs
`a_click_opens_a_session_remembers_it_and_marks_it_seen_once`), `a manual mark-unread on the open session sticks`
(crates/gui/farhelm-gui/src/session_list/seen_tests.rs
`a_manual_unread_of_the_open_session_sticks_until_the_user_returns_or_output_arrives`,
`the_seen_memory_survives_a_cut_listing_that_hides_the_open_session`,
`the_seen_memory_survives_a_host_filter_that_hides_the_open_session`,
`leaving_and_returning_while_the_session_is_hidden_marks_it_seen`),
`a manual read toggle reverses under a build mismatch` and
`the mark-seen item is reachable and operable by role and keyboard` (seen_tests.rs
`the_menu_item_follows_the_seen_state_and_sends_the_stamp_or_null`; one helm serves one window, so there is no build
mismatch to reverse under), `a failed manual mark closes on acceptance and preserves a newer menu` (seen_tests.rs
`a_refused_toggle_shows_on_the_row_until_the_next_operation`),
`the dot click marks a different row read without moving the selection` (seen_tests.rs
`the_dot_toggles_once_without_opening_its_row`),
`a manual mark-unread is visible to a second client that never touched the session` (the helm's shared state;
scripts/gui-e2e-smoke.py `test_36_mark_read_and_unread_from_the_row_menu` reads it back from the helm's listing);
`a dot with no seen-state toggle still opens the row on click` has no case to port, since only a live row draws a dot
and every live row offers the toggle. The bulk Mark read and Mark unread of marked rows:
crates/gui/farhelm-gui/src/session_list/marking_tests.rs `metadata_writes_go_once_per_eligible_row_and_clear_the_marks`
(Mark read sent; Mark unread offered but not sent there, its write being the row's own, seen_tests.rs
`the_menu_item_follows_the_seen_state_and_sends_the_stamp_or_null`); the menu-geometry case's items in order, Pin to top
or Unpin and Clone and Replace with… among them: menu_tests.rs
`the_menu_opens_below_its_button_holds_focus_and_leaves_the_selection`; the auto-select and visit-history cases:
tests.rs `the_automatic_choice_prefers_history_then_the_last_selection_then_the_newest`,
`a_selection_settles_only_on_evidence`, `a_selection_moves_only_once_a_complete_listing_shows_it_gone`,
`a_click_opens_a_session_remembers_it_and_marks_it_seen_once`, menu_tests.rs
`the_next_session_opens_only_once_the_delete_has_settled`, lifecycle_tests.rs
`a_pending_stop_or_replace_holds_no_automatic_choice_back`; their preference reread before the pick (`preferenceReads`,
a failed read's fallback) is superseded by the channel, which pushes every preference change before any later reply (the
migration's orchestrator, 2026-11-15; selection.rs's NOTE); the app bar (its first case):
crates/gui/farhelm-gui/src/sidebar/tests.rs `the_header_shows_the_wordmark_and_the_build_with_its_hover_help`,
`the_wordmark_is_the_file_for_the_appearances_ground`, crates/gui/farhelm-gui/src/assets.rs
`the_wordmark_paths_are_not_swapped`. Superseded: the helm and window skew half of that case ("The helm runs Farhelm
<helm>; this window is a development build of Farhelm.", and "... this window was built as Farhelm <window>." for a
release window) and the macOS build mismatch notice below the bar: the desktop app attaches only to the helm it embeds
(SPEC.md, "Non-goals for v1": "the desktop app attaches only to the helm it embeds"), so the window and its helm are one
build (the version readout's deviation below). The other macOS cases: the 48-pixel bar with room for the window buttons
is crates/gui/farhelm-gui/src/desktop/tests.rs
`the_band_rows_are_48_high_at_the_top_and_the_app_bar_keeps_the_lights_clear`, and the controls staying clear of the
buttons in a narrow window is `in_a_narrow_window_the_app_bar_runs_above_both_panes` (both built on macOS only, so only
a Mac run shows them); the reservation before the session shell exists is superseded, since the GPUI app has no sign-in
page before its shell and its connection screens draw a band strip across their top
(`desktop::title_band::Row::EmptyPane`). The bar staying put while the list scrolls holds by construction (the header is
outside the list's scroller) and has no test of its own

### `tests/sidebar.spec.ts` (its row-content cases)

Tests: 12. Covers: narrow rows align fixed facts and reserve only control-sized menu gutters.

partly: `a row with unbroken oversized fields stays contained and stacked in the sidebar`,
`a long cwd shows its tail with characters in logical order` and
`narrow rows align fixed facts and reserve only control-sized menu gutters`
(crates/gui/farhelm-gui/src/session_list/row_tests.rs
`a_long_title_directory_and_error_stay_inside_the_row_at_the_narrowest_sidebar`, real text system: the title cut before
the age, the ages on one right edge, the directory's tail kept, a long error wrapping inside the row; the agent glyphs:
crates/gui/farhelm-gui/src/session_list/row_marks_tests.rs
`the_agent_mark_follows_the_launch_and_inks_with_its_identity`, `the_agent_mark_draws_its_glyphs_picture`,
`every_row_draws_one_agent_mark_and_one_shield_named_by_their_description`; the compact half:
crates/gui/farhelm-gui/src/session_list/compact_tests.rs
`each_ended_status_draws_its_own_mark_named_and_helped_by_its_full_wording` (the exited and stopped marks named and
helped `Exited (code 17)` and `Exited (code unknown) — stopped by user`, the interrupted and error marks, no dot beside
them), `a_compact_stale_row_draws_a_clock_named_and_helped_by_its_cause` and
`compact_hides_the_last_line_and_the_ended_line_and_shortens_the_row` (no ended line, a shorter row); the menu gutter's
width has no test of its own), `the host/directory line aligns under the title as one continuous string` (row_tests.rs
`the_last_line_names_the_host_and_directory_without_a_dangling_separator`, `the_last_lines_parts_are_none_when_empty`,
`the_directory_is_drawn_in_the_monospace_face`; the line starts under the title by construction, with no test of its x),
`a /home/<user> cwd shows the folded form with the exact path in the card` (the folded half: tests.rs
`only_a_personal_home_with_a_path_after_it_folds` and row_tests.rs
`the_last_line_names_the_host_and_directory_without_a_dangling_separator`; the card:
crates/gui/farhelm-gui/src/session_list/summary_tests.rs
`a_running_sessions_card_names_its_agent_host_folder_and_times`, the directory in full),
`a local session's host line is provisional until the registry confirms it` (the name half: before a host list arrives
the row shows the helm's own name, row_tests.rs
`the_last_line_names_the_host_and_directory_without_a_dangling_separator`; the locality mark: row_marks_tests.rs
`a_rows_locality_is_local_remote_or_unplaced`, `an_unplaced_row_draws_no_host_mark_and_keeps_its_slot`,
`the_host_mark_names_its_locality_and_explains_it_on_hover`, `the_host_mark_draws_the_hosts_chosen_shape_and_color`),
`hostile identity and host text stay contained with simultaneous qualifiers` (partly: tests.rs
`a_rows_host_text_is_painted_and_named_escaped` and row_tests.rs `a_stale_row_names_its_cause_with_hover_help`; the
compact stale glyph: compact_tests.rs `a_compact_stale_row_draws_a_clock_named_and_helped_by_its_cause`), and
`aliasing a remote host renames it everywhere but the details view` (the row's half: tests.rs
`a_host_without_a_short_name_shows_its_registry_name`); the ages, ended words and stale causes these cases also read are
pinned in row_tests.rs `a_rows_age_shows_its_bucket_and_its_timestamp_as_hover_help`,
`an_age_moves_on_when_the_clock_ticks_even_in_an_inactive_window`,
`an_exit_without_a_code_says_the_code_is_unknown_on_the_row`, `each_host_state_names_its_cause`,
`the_selected_and_stale_rows_use_their_own_inks` and crates/gui/farhelm-gui/src/session_list/age.rs's tests.
`a bidi override in the invocation basename renders escaped and isolated` (the agent mark's half, its pinned text:
row_marks_tests.rs `the_description_escapes_the_command_it_quotes`; the menu header's half, the escaped basename, is not
covered here). `compact hides the second line and persists across client seeds`: compact_tests.rs
`compact_hides_the_last_line_and_the_ended_line_and_shortens_the_row` and
`a_pushed_change_of_the_preference_redraws_the_rows`, order/tests.rs
`compact_is_a_checkbox_after_the_orders_checked_as_the_preference_is` and
`choosing_compact_sends_its_sparse_write_and_the_rows_follow_it` (a second client's seed is the channel's snapshot, as
for the order; the path the old case read from the summary card is in the compact row's card, summary_tests.rs
`a_compact_row_shows_the_same_card_with_its_hidden_folder`, and in its description, a deviation below, compact_tests.rs
`a_compact_rows_help_says_what_its_hidden_lines_would`; the host mark stays on the first line by construction).
`an open menu tints its own row and presses its own toggle`: the pressed toggle is `Tier::RowMenu`'s expanded look, with
no test of its own, and the row's tint is not drawn (a deviation below); the managed checkout's form of the last line
(its `·` separator and repository mark) is not ported (a deviation below)

### `tests/tooltip.spec.ts` (its session-row cases)

Tests: 9. Covers: a session's title and open area share its summary card.

ported: crates/gui/farhelm-gui/src/session_list/summary_tests.rs and the goldens.
`a session's title and open area share its summary card`:
`a_rows_title_shows_its_summary_card_beside_the_sidebar_with_its_ring` (from the title, beside the sidebar with the
notch and the ring on the open control, the card's words in order),
`a_running_sessions_card_names_its_agent_host_folder_and_times`,
`an_idle_sessions_card_names_its_unseen_output_and_its_guarded_launch`,
`an_exited_sessions_card_says_exited_and_names_its_commands_program`,
`a_managed_checkouts_card_names_the_repository_and_the_checkouts_folder` and the `session-summary-card` golden (from the
open area's lower part), the compact density by `a_compact_row_shows_the_same_card_with_its_hidden_folder`, and end to
end scripts/gui-e2e-smoke.py `test_75_resting_on_a_rows_title_shows_its_summary_card`, and its keyboard focus half (Tab
onto the open control shows the same card beside the sidebar after the hover delay, the open row's with its on-fill
ring, and focus moving on or Escape takes it away) by
`tab_onto_a_rows_open_control_shows_its_card_until_focus_moves_on`;
`a session summary falls back to ordinary placement when the window is too narrow`:
crates/gui/farhelm-gui/src/layers/beside_tests.rs `card_help_falls_back_without_cues_in_a_window_too_narrow`;
`a stale card is dim while row marks keep their own hover`: `a_card_leaves_out_what_the_session_does_not_say` (the stale
status still named; its grey is `CardStatus`'s ink, which no test pins) and
`every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring` (the marks' own help);
`a legacy session card omits unavailable facts` and `an unplaced host keeps its name without a locality mark`:
`a_card_leaves_out_what_the_session_does_not_say`; `hovering a row mark shows its text beside the list after the delay`:
`every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring` and the `session-row-hover-beside` golden (the age's
ring and notch); `every visible session-row hover opts in, including the notification bell`:
`every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring` (the dot, host, agent, age, pin, bell, actions button
and stale cause, with the open row's on-fill ring) and `a_compact_row_shows_the_same_card_with_its_hidden_folder` (the
compact ended mark and clock), and on keyboard focus the marks that hold focus, the bell and the actions button, by
`a_focused_row_mark_shows_its_own_help_beside_the_sidebar_not_the_card` (each its own help beside the sidebar with its
ring, not the card); `moving to the next mark while a tooltip shows replaces it at once`: the layer manager's
crates/gui/farhelm-gui/src/components/tests/hover_help.rs
`moving_between_a_mark_and_its_row_shows_the_help_under_the_pointer` and
`one_controls_leave_never_hides_anothers_tooltip`; `a card target clones its hidden source into the shared tooltip`:
superseded (the card is a view the layer manager draws, `HoverHelp::card`).

### `tests/sort.spec.ts`

Tests: 21. Covers: session list ordering.

partly: crates/gui/farhelm-gui/src/session_list/order/tests.rs
`the_title_names_the_stored_order_and_follows_a_pushed_change` (`an unchosen client lists by activity` with the helm's
own `a_channel_opens_in_the_stored_list_order_or_most_recent_activity` in crates/farhelm-helm/src/ui_channel_tests.rs;
`a stored order this build does not know falls back to the default`),
`a_click_opens_the_menu_on_its_first_order_and_escape_returns_to_the_title`,
`down_and_up_open_the_menu_at_either_end_and_home_and_end_walk_it` and
`compact_is_a_checkbox_after_the_orders_checked_as_the_preference_is`
(`the sort title exposes checked choices and keyboard menu semantics`, End and Up reaching Compact; its closing press on
the main pane is the layer manager's outside press, crates/gui/farhelm-gui/src/layers_tests.rs
`a_popover_follows_its_anchor_and_closes_when_the_anchor_is_gone`),
`choosing_an_order_sends_only_its_sparse_write_and_rechoosing_sends_nothing`
(`switching to title order reorders the rows`, `re-sorting keeps the selected session selected`,
`the default order is never written, and re-choosing it re-reads nothing`, the window's half of
`a sort write and a selection write each preserve the other field`, whose merge is the helm's
`the_preference_routes_read_and_merge_one_shared_row` in crates/farhelm-helm/src/preferences.rs),
`a_refused_write_keeps_the_stored_order_and_shows_nothing` (`a preference write that fails`, a deviation below),
`the_open_menu_follows_a_pushed_order`; `an order and a host choice survive each other` holds by construction: the host
query and the order's write are separate commands, a keyless send and work on the preference row's key, so neither waits
for or resends the other, the host query never names a sort `filters/tests.rs`
`every_query_carries_both_the_host_and_the_status`, and the channel keeps following the stored order under a host,
`an_open_channel_follows_the_stored_order_until_the_gui_names_one` in crates/farhelm-helm/src/ui_channel_tests.rs; the
rows in the pushed order: crates/gui/farhelm-gui/src/session_list/tests.rs
`rows_follow_the_pushed_order_and_the_selection_follows_its_session`; scripts/gui-e2e-smoke.py
`test_46_the_order_menu_reorders_the_list_by_creation_and_by_title` (the helm storing each order and the window drawing
the helm's own order); the `session-order-menu` golden. Covered by construction, with no GUI test of their own:
`the chosen order survives a reload`, `nothing mounts before the preference seed` and `a preference read that fails` or
`stalls` (the channel's snapshot carries the preferences and the list already in the stored order, a reset's included:
`a_channel_opens_in_the_stored_list_order_or_most_recent_activity`; the GUI never reads the preferences on its own),
`a reply answering the previous order is refused after the order changes` and the multipage walk (no listing is a reply:
the helm pushes one list, re-sorted before the write's reply), and the two `fallback picks the newest session` cases
(the selection rules' newest-created choice from the rows in hand, tests.rs
`a_selection_moves_only_once_a_complete_listing_shows_it_gone`, which no request precedes). Compact's write and its
silent refusal: `choosing_compact_sends_its_sparse_write_and_the_rows_follow_it` and
`a_refused_compact_write_keeps_the_stored_density_and_shows_nothing`. A choice made while another preference write is in
flight waits for it, latest wins, as the old app's preference writer queued it:
`an_order_chosen_while_compact_is_saving_is_shown_at_once_and_sent_after_it`,
`two_quick_choices_of_the_order_send_only_the_latest`, `a_refused_queued_order_restores_the_stored_order`,
`a_choice_queued_behind_a_refused_write_is_still_sent`,
`two_fields_queued_behind_a_third_write_go_out_one_after_the_other` and
crates/gui/farhelm-gui/src/session_list/host_rail/expansion/tests.rs
`a_press_during_an_order_write_is_shown_at_once_and_sent_after_it`. The order hold's cases:
`the order holds under the pointer and catches up when it leaves` (crates/gui/farhelm-gui/src/session_list/pins_tests.rs
`the_pointer_holds_the_order_and_leaving_releases_it`), `a still pointer releases the held order after five seconds`
(pins_tests.rs `five_seconds_of_stillness_release_the_hold_and_movement_restarts_them`,
`a_scroll_over_the_rows_counts_as_movement`), `an open menu defers the release without losing it` (pins_tests.rs
`an_open_row_menu_keeps_the_hold`, `a_menu_closing_over_the_rows_keeps_the_hold`),
`a hold taken while a new order is loading does not freeze it` (crates/gui/farhelm-gui/src/session_list/hold.rs
`a_different_list_ends_the_hold`, a hold ending once the list on screen answers another query or order);
`a held delete that moves the open menu's row closes that menu` has no case to port, since a row's menu is anchored to
its row and follows it (crates/gui/farhelm-gui/src/layers_tests.rs
`a_popover_follows_its_anchor_and_closes_when_the_anchor_is_gone`).

### `tests/filters.spec.ts`

Tests: 13. Covers: permanent host filtering.

partly: crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs
`choosing_a_host_sends_its_query_once_and_all_sends_an_empty_one`,
`the_chip_names_the_chosen_host_and_removes_the_choice`, `clearing_the_filter_by_keyboard_or_click_focuses_all`,
`the_empty_state_names_the_answered_host_and_show_all_hosts_clears_it`,
`a_removed_chosen_host_keeps_a_pressed_inert_placeholder`, `a_host_list_never_read_shows_the_unavailable_status`,
`a_refused_query_keeps_the_previous_choice_and_shows_the_helms_words`, `a_reset_forgets_the_choice`,
`tab_from_all_reaches_the_first_avatar_and_space_chooses_it` (with
`clearing_the_filter_by_keyboard_or_click_focuses_all`, also the case's Tab from the status segments to the chip), for
the rail's Add host and the keyboard below its fold `add_host_is_the_rails_last_entry_with_its_name_help_and_role`,
`add_host_opens_the_dialog_and_escape_returns_focus_to_it` and
`tab_scrolls_the_last_avatar_and_add_host_into_view_below_the_fold` (the old page's scroll-into-view of a focused
entry), and for the foot's Hosts control and Settings gear
`the_foot_holds_hosts_above_the_settings_gear_with_their_names_and_help`,
`the_gear_opens_settings_at_its_first_switch_and_escape_returns_to_the_gear`,
`tab_reaches_the_foot_after_the_rail_and_the_lists_controls` and
`a_long_host_list_scrolls_above_the_foot_which_stays_put` (the Settings half of "rail scrolling leaves settings and help
reachable"; the Hosts swap's own tests are in the settings-and-hosts parity file, the `tests/hosts-panel.spec.ts` row);
crates/gui/farhelm-gui/src/session_list/filters/tests.rs, for the status halves of the empty sentence and Show all
statuses, `every_empty_answer_names_its_filters_with_a_link_per_filter`,
`each_link_clears_its_own_filter_and_focuses_all`, `a_cut_empty_answer_says_only_what_could_be_read`, and for the
query's bookkeeping `a_refusal_undoes_only_the_latest_choice`,
`a_refused_or_lost_query_puts_the_answered_choice_back_with_one_line` and
`only_a_narrowed_available_empty_answer_is_named`; scripts/gui-e2e-smoke.py
`test_35_choosing_the_local_host_narrows_the_list_and_its_chip_clears_it`,
`test_39_the_rail_foot_toggles_hosts_and_opens_settings` and
`test_41_the_rails_add_host_opens_its_dialog_and_escape_returns_to_it`; the `host-filter-empty`,
`filter-empty-both-links` and `buttons` goldens (crates/gui/farhelm-gui/src/goldens/); the sort beside a held listing
holds by construction (a held query is a keyless send that the order's write, work on the preference row's key, never
waits for, and the list draws its heading, the title in it, on every path of its render, a failed listing's included);
filters/tests.rs `the_heading_and_segments_are_drawn_before_the_first_listing` covers the heading before the first
listing. crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs
`a_failed_registry_refresh_names_the_avatars_state_as_last_read` covers the last-read wording of a failed refresh (the
case "unavailable and retained registry states neither guess local nor steal focus"). The rail's context menus:
crates/gui/farhelm-gui/src/session_list/host_rail/avatar_menu_tests.rs
`a_secondary_click_on_an_avatar_opens_its_hosts_menu_beside_it`, `shift_f10_on_a_focused_avatar_opens_its_menu`,
`the_expanded_rows_menu_opens_beside_its_icon_box`, and `a_second_secondary_click_on_the_same_avatar_leaves_one_menu`
(the opener, which `hosts`' own tests return focus to from Remove and Settings, is the avatar), and the screenshot
batch's `app-rail-host-context-menu` scene; the expanded rail's choosing and badges and the narrow-window geometry:
crates/gui/farhelm-gui/src/session_list/host_rail/expansion/tests.rs `choosing_and_badges_work_in_the_expanded_rail` and
`the_sidebar_widens_by_164_pixels_down_to_the_main_panes_floor` (real text system); scripts/gui-e2e-smoke.py
`test_77_the_rails_avatar_menu_and_expansion`. The chip at the minimum sidebar width with a long alias:
crates/gui/farhelm-gui/src/session_list/row_tests.rs
`a_chosen_hosts_long_alias_is_cut_inside_its_chip_at_the_narrowest_sidebar` (real text system: the segments and the chip
inside the list, the name cut with "…" and the ✕ inside the chip; the old case's empty sentence, which wraps the name in
full, has no layout check of its own here). Dropped: the ContextMenu key on an avatar, which GPUI names on neither
platform (a deviation below); the foot's notices bell and Help and the Help menu are the other-dialogs area's; the Add
host dialog's own cases are in the settings-and-hosts parity file

### `tests/status-filter.spec.ts`

Tests: 4. Covers: status segments query the helm, count matches and retain a hidden terminal.

ported: crates/gui/farhelm-gui/src/session_list/filters/tests.rs
`the_segments_are_a_radio_group_with_the_old_words_and_the_chip_after_them`,
`every_query_carries_both_the_host_and_the_status` (Idle's query, the arrow from Idle choosing Waiting),
crates/gui/farhelm-gui/src/session_pane/carried_tests.rs
`a_filtered_out_open_session_keeps_its_terminal_header_details_bar_and_rename` (the hidden open session keeps its
terminal and the pane still names it, while the list neither draws nor counts it),
`every_empty_answer_names_its_filters_with_a_link_per_filter` (the empty sentence per status, with and without a host,
and its links), `each_link_clears_its_own_filter_and_focuses_all` (Show all hosts keeping the status, Show all statuses
by Enter keeping the host, focus on the rail's All), `the_heading_and_segments_are_drawn_before_the_first_listing` and
`a_pick_is_hidden_by_the_answered_listing_or_else_by_its_row` (the switcher-pick rule, a current listing overriding the
switcher's stale status included); crates/gui/farhelm-gui/src/session_list/quick_switcher/tests.rs
`a_pick_the_filters_hide_resets_both_and_a_shown_pick_keeps_them` and
`a_pick_leaves_the_hosts_panel_and_a_refused_widening_keeps_both_choices`; the radio group's keys in
crates/gui/farhelm-gui/src/components/tests.rs; scripts/gui-e2e-smoke.py
`test_42_the_status_segments_narrow_the_list_to_one_status` (the helm's own status query and its "N of M"); the
`session-status-segments` and `filter-empty-both-links` goldens. Ended sessions under All only is the helm's exact
status match. The old app's "a refused pick keeps both choices" has no case: no pick is refused here (nor was one at the
reference commit); a refused widening query keeps both, as any refused query does. The query names the open session in
every send, so the helm keeps it in the pushed collection outside the list's order while a filter hides it
(crates/gui/farhelm-gui/src/session_list/filters/tests.rs `a_new_open_session_sends_the_query_in_force_naming_it`,
crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs `a_reset_forgets_the_choice`;
crates/gui/farhelm-gui/src/session_pane/carried_tests.rs)

### `tests/pins.spec.ts`

Tests: 5. Covers: pin from the menu groups the sidebar and unpin restores title order.

ported: `pin from the menu groups the sidebar and unpin restores title order`
(crates/gui/farhelm-gui/src/session_list/pins_tests.rs
`pin_and_unpin_regroup_the_list_with_the_glyph_and_the_separator`; the glyph's ink on the selected row is the
`session-list-pinned` golden; the helm's served order staying unpartitioned is the helm's own, which the GUI never asks
for; scripts/gui-e2e-smoke.py `test_70_pin_move_and_unpin_from_the_row_menu` reads each pin back from the helm's
listing), `menu moves share manual pin order, retain focus and ignore sort choices` (pins_tests.rs
`moves_name_the_displayed_neighbor_take_effect_under_the_hold_and_keep_focus`,
`a_pending_pin_request_refuses_the_items_and_a_refusal_shows_on_the_row`,
`a_narrowed_listing_keeps_its_pins_first_in_their_order`, crates/gui/farhelm-gui/src/session_list/pins.rs
`pins_lead_in_their_shared_order_and_the_rest_keep_the_helms`; the order surviving a reload and every sort choice holds
by construction, since the window groups the pins of whatever the helm serves; the second client and the blurred toggle
have nothing to port, since one helm serves one window and focus moves only at the Move's acceptance),
`new pins remain held across replies until the original hold ends` (pins_tests.rs
`a_pin_made_during_the_hold_waits_in_place_with_no_moves`; its grip half, no grip on a pin made during the hold until
the hold ends, is crates/gui/farhelm-gui/src/session_list/pin_drag_tests.rs
`movable_pins_have_a_named_grip_revealed_by_the_pointer`), `drag pins within their group and cancel interrupted moves`
(pin_drag_tests.rs `movable_pins_have_a_named_grip_revealed_by_the_pointer` for the grip on movable pins only, its name
and its reveal, with its help beside the sidebar in crates/gui/farhelm-gui/src/session_list/summary_tests.rs
`every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring`;
`a_drop_on_the_drop_line_sends_one_move_that_the_hold_shows_at_once` and
`a_drop_below_the_last_pins_middle_goes_after_it` for the lift, the drop line, the rows unchanged until the helm's
reply, the one request and the moved order under the hold; `escape_a_lost_release_and_a_release_outside_send_nothing`
for Escape, a lost capture, a release outside the rows and one below the separator;
`the_dragged_row_going_away_cancels_the_drag` for the deleted row, `a_retired_pin_has_no_grip_but_is_a_drop_target` for
a retired row, `the_window_going_inactive_ends_a_drag` for a lost capture the window never reports, and
`a_drag_longer_than_the_stillness_keeps_the_order_hold` for the hold through a long drag,
`the_apps_shortcuts_still_act_during_a_drag` for keys during a drag, which reach the grip the press focused as in the
old app, `escape_ends_a_drag_begun_with_a_rows_menu_open`, `escape_ends_a_drag_after_tab_moved_focus_off_the_grip` and
`the_escape_that_ends_a_drag_goes_no_further` for Escape wherever focus is in the sidebar;
crates/gui/farhelm-gui/src/session_list/pin_drag.rs `a_drop_names_the_pin_below_the_pointers_line_or_the_last_one` and
`a_release_outside_the_drawn_group_lands_nowhere` for where a drop lands; scripts/gui-e2e-smoke.py
`test_78_dragging_a_pins_grip_moves_it_in_the_helms_order` reads the move back from the helm's listing; the reload is
the helm's, as for Move, and there is no second client, since one helm serves one window; the keyboard way to rearrange
pins stays the row menu's Move up and Move down, since the old grip had no keys and no Tab stop),
`pin dragging survives a failed listing and replacement list mount` (pin_drag_tests.rs
`a_failed_listing_ends_a_drag_and_the_recovered_list_drags_again`: a failed read keeps the rows here, so a drag on them
ends by rule, where the old list's root went away with it)

### `tests/sidebar-resize.spec.ts`

Tests: 2. Covers: sidebar drag clamps, persists, resets and refits the program.

partly: crates/gui/farhelm-gui/src/sidebar/tests.rs `the_sidebar_draws_the_stored_width_and_follows_a_pushed_change`,
`a_window_opens_at_the_stored_width`, `the_divider_is_a_named_separator_whose_value_is_the_width`,
`a_drag_follows_every_move_writes_once_and_clamps_at_both_ends`, `a_drag_whose_release_was_lost_keeps_its_last_width`,
`a_double_click_resets_to_the_default_and_writes_it`, `the_arrows_step_clamp_write_each_press_and_keep_focus`,
`a_refused_write_shows_what_the_helm_stores`, `tab_meets_the_divider_between_the_list_and_the_pane`,
`a_press_on_the_grip_focuses_the_divider_under_either_click_rule`, `a_press_just_right_of_the_grip_reaches_the_terminal`
(a press beside the grip reaches the terminal), `a_window_too_narrow_for_both_panes_scrolls_sideways`,
`the_main_pane_keeps_its_floor_at_the_minimum_window_with_the_widest_sidebar` (real text system),
crates/gui/farhelm-gui/src/components/tests/divider.rs `a_divider_reports_its_drag_double_click_and_arrows`;
scripts/gui-e2e-smoke.py `test_37_the_focused_sidebar_divider_steps_the_helms_width`. The device storage cases
(`localStorage` holding the width less the rail, a reload keeping it) are superseded: the width is a helm preference
(SPEC.md, "Session list", decided by the maintainer on 2026-11-13), and the reload case is the pushed-change test. The
heading actions inside the sidebar at 296, 396 and 656: the heading now holds the order's title, Trash, Templates and
New (Add host is the rail's, Update all the attention list's), and at the narrowest width, the one that can run out of
room, crates/gui/farhelm-gui/src/session_list/row_tests.rs
`the_headings_title_and_actions_stay_whole_at_the_narrowest_sidebar` (real text system). The terminal's columns growing
after a narrower drag are the terminal area's, from its own fit of its pane. The second case's narrow-window chrome: the
fixed macOS app bar is the title band's 48 pixels (crates/gui/farhelm-gui/src/desktop/tests.rs
`the_band_rows_are_48_high_at_the_top_and_the_app_bar_keeps_the_lights_clear`, macOS only); its row-menu pointer cutoffs
are dropped, since the row menu draws no pointer (a deviation below). The shown width with the rail expanded is
crates/gui/farhelm-gui/src/session_list/host_rail/expansion/tests.rs
`the_shown_width_adds_164_pixels_within_the_main_panes_floor` and
`the_sidebar_widens_by_164_pixels_down_to_the_main_panes_floor` (real text system),
`the_divider_sits_at_the_shown_edge_and_drags_the_stored_width` (the grip at the shown edge, the separator's value and a
drag on the stored width) and, in `desktop/linux_title_bar/tests.rs`,
`the_content_width_leaves_out_kits_frame_on_each_untiled_side` (the content width the pane's floor is kept against in a
Linux window that draws its own frame)

### `tests/host-rail-warning.spec.ts`

Tests: 7. Covers: ${operation} warning acknowledges once, relatches a new failure and clears on correlated retry.

partly: crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs `a_failed_update_marks_the_avatar_until_hosts_opens`
(the drawn mark beside the dot, its words in the avatar's name and hover help, and Hosts acknowledging it) and
`a_warning_raised_outside_the_store_redraws_the_rail`, and scripts/gui-e2e-smoke.py
`test_74_a_setup_that_fails_with_hosts_closed_marks_its_avatar_until_hosts_opens` (a fake host's failed setup in the
running app); the warning's rules, when it latches, relatches and clears, are
crates/gui/farhelm-gui/src/hosts/status/tests.rs's. Not ported: the initial Add whose acceptance was malformed or lost,
and the held Update losing its target (see Deviations)

### `tests/host-short-names.spec.ts`

Tests: 7. Covers: short host captions preserve aliases, abbreviate FQDNs and truncate inside the collapsed rail.

partly: crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs
`the_rail_lists_all_then_registered_hosts_in_order_with_their_names`, `an_avatars_hover_help_is_its_name`,
`each_avatar_draws_its_dot_and_names_its_phase`, `a_hosts_names_are_shown_and_named_escaped`,
`identity_names_local_and_adds_a_destination_only_when_it_differs`,
`the_rail_takes_its_56_pixels_of_the_sidebar_and_cuts_long_captions` (real text system);
crates/gui/farhelm-gui/src/session_list/tests.rs `a_host_without_a_short_name_shows_its_registry_name` and
crates/gui/farhelm-gui/src/session_list/row_tests.rs
`the_last_line_names_the_host_and_directory_without_a_dangling_separator` (the rows' full-name fallbacks). The
expansion: crates/gui/farhelm-gui/src/session_list/host_rail/expansion/tests.rs
`the_expand_control_sits_below_help_with_the_old_toggles_words`,
`expanding_writes_the_preference_and_shows_the_choice_in_flight`,
`the_rail_follows_the_stored_preference_and_its_pushes` (the persistence case, superseded by the helm preference
SPEC_impl.md keeps, decided by the maintainer on 2026-11-13: a window opens as the helm stores it, and a pushed change
reaches it), `a_refused_expansion_leaves_the_rail_as_stored_and_shows_nothing`,
`tab_reaches_the_control_and_enter_and_space_toggle_it_keeping_focus` (keyboard expansion),
`the_expanded_rail_names_its_rows_and_keeps_every_mark_in_its_column` (expanded rows, the expanded labels, captions
gone, marks in their column and foot controls unmoved) and
`the_sidebar_widens_by_164_pixels_down_to_the_main_panes_floor` (the 164 pixels and a long name cut short, real text
system); the screenshot batch's `app-host-rail-expanded` scene; scripts/gui-e2e-smoke.py
`test_77_the_rails_avatar_menu_and_expansion`. Scrolling from the rail's padding holds by construction: the rail's
padding and gaps are inside its scrolling element (`host_rail::rail`'s docs), so a wheel over them is GPUI's own
scrolling of that element, with no test of a wheel over the padding

### `tests/rail-notices.spec.ts`

Tests: 4. Covers: rail notices quiet, alerting and resolved in ${palette}.

replaced: the bell and its panel are the other-dialogs area's, and [Other dialogs](other-dialogs.md), "The rail notices
bell and panel", names their tests (crates/gui/farhelm-gui/src/rail_notices/tests.rs and the `notices-alerting`,
`notices-empty` and `notices-bell-loud` goldens). This area's half is the bell's place at the top of the rail's foot,
above Hosts, and its Tab stop: crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs
`the_foot_holds_hosts_above_the_settings_gear_with_their_names_and_help` and
`tab_reaches_the_foot_after_the_rail_and_the_lists_controls`.

### `tests/host-updates.spec.ts`

Tests: 10. Covers: attention groups problems, work and updates and opens the exact host.

partly: crates/gui/farhelm-gui/src/session_list/attention/tests.rs.
`attention groups problems, work and updates and opens the exact host`
(`rows_group_problems_then_work_then_updates_in_registry_order`,
`each_problem_has_its_words_and_healthy_hosts_have_none`, `the_list_draws_one_row_per_host_with_its_words_and_action`,
`details_opens_hosts_at_the_host_with_its_toggle_focused`, which also checks the warning acknowledged and the filter
unchanged; the panel's half is crates/gui/farhelm-gui/src/hosts/tests.rs
`reveal_focuses_a_far_rows_toggle_and_scrolls_it_into_view`, and smoke step 76 drives Details end to end);
`Always update requests exactly N displayed hosts` and `is absent when automatic updates are already enabled`
(`always_update_asks_for_the_displayed_hosts_and_the_preference`);
`Always update retains a hidden save failure and recovers through Try again` and
`save failure retires a session menu measured while the save was pending`
(`a_failed_always_update_shows_its_line_until_try_again_saves`; recovery through Settings is
crates/gui/farhelm-gui/src/hosts/status/tests.rs `a_newer_choice_in_settings_or_elsewhere_retires_the_failed_save`);
`optional dismissal leaves other attention visible and revives only for new updates`
(`dismissal_hides_only_optional_updates_until_a_new_host_has_one`,
`dismissal_hides_optional_rows_until_another_host_gains_an_update`; the helm-build revival is a deviation below);
`progress-only publications preserve an open session menu` and
`attention insertion and removal retire existing compact session menus`
(`menus_below_close_when_the_list_moves_and_stay_through_progress`,
`the_layout_key_follows_copy_and_buttons_but_not_progress`, with ordinary rows rather than compact ones);
`many attention rows scroll above usable sessions in a short narrow sidebar` (`many_rows_scroll_inside_a_bounded_list`,
in a short window at the sidebar's ordinary width and one palette, the palettes being the goldens' to check). The
working row's step and bar are `a_working_row_names_its_step_and_fills_its_bar`, absence is
`the_list_is_absent_when_nothing_needs_attention_and_while_hosts_shows`, Update and Update all over the displayed hosts
are `update_and_update_all_send_exactly_the_displayed_hosts`, their wait for a launch in flight is
`update_actions_wait_visibly_while_a_launch_is_in_flight`, and a working row's progress card as its hover is
`a_working_rows_hover_is_the_progress_card_once_its_run_shows`; the reference scenes `host-setup-running` and
`host-setup-failed` (the list beside the rail's avatar during a setup and after it failed) wait for a bridge verb that
can script a fake host, since staging either takes the helm's host script, probe and provision calls, which no scene
step can make; meanwhile the tests above and smoke step 76 (a scripted setup failing with Hosts closed, its row, and
Details) cover the behaviour. `attention opens the menu header of an expanded host spanning the viewport` (Details on a
host whose open row is taller than the panel, its menu toggle scrolled out above) by the rule's two halves: the reveal
focuses that row's menu toggle itself (`reveal_focuses_a_far_rows_toggle_and_scrolls_it_into_view`), and the focus
coordinator scrolls a focused control into view from either side by the least distance, whatever holds it
(crates/gui/farhelm-gui/src/focus_tests.rs
`tab_to_a_field_below_the_fold_scrolls_it_into_view_and_shift_tab_scrolls_back` and
`tab_scrolls_the_least_distance_and_not_at_all_for_a_control_already_in_view`); no test draws a row taller than the
panel. The rest are in Settings and hosts. The old list's environment rows had no case of their own in this spec (the
old app's unit tests covered them), and are crates/gui/farhelm-gui/src/session_list/attention/tests.rs
`environment_phases_have_their_rows_and_routine_ones_none` (a stopped environment, a failed deletion and a failed resume
as problems, "Deleting" as work in progress, routine suspensions with no row),
`credentials_and_tunnels_join_a_rows_words` (undelivered credentials and failed tunnels added to a row's words, tunnels
only while connected), `environment_rows_offer_details_and_go_when_the_environment_recovers` and
`failed_tunnels_show_only_while_the_host_reads_connected`. Details on a "Resume failed" or "Stopped" row reaches no
Resume in Hosts yet, since Hosts' Resume is not ported (the session's own card offers one). SPEC.md, "Sandbox sessions",
asks the attention area for a failed deletion's reason and Retry; the old row offered only Details with that sentence
already in SPEC.md, so the GPUI migration's orchestrator decided on 2026-11-16 that the old app is its reference
reading: the row shows the failure and leads to Hosts, which shows the reason and Retry, and SPEC.md's wording is to be
revised to say so.

### `tests/work-start-order.spec.ts`

Tests: 1. Covers: active sessions lead across bursts, idle demotion, reload and rename.

replaced. The order is the helm's and the supervisor's (SPEC.md, "Session list": "The grouping and the key are
authoritative helm and supervisor data"): crates/farhelm-helm/src/aggregate.rs
`activity_order_moves_for_a_new_burst_or_a_group_change_only` (running first, a finished session dropping below on an
unchanged key, a new burst promoting) and `activity_keeps_the_work_start_order_and_tiebreaks_inside_each_group`, with
what starts a burst in crates/farhelm-supervisor/src/service/ticker.rs
`work_starts_and_activity_dates_follow_the_reading_transitions`. The window draws the order it is pushed and tells the
helm nothing: crates/gui/farhelm-gui/src/session_list/tests.rs
`rows_follow_the_pushed_order_and_the_selection_follows_its_session`. A row moving under its open Rename editor keeps
the editor's draft, caret and focus, and Save sends what was typed:
crates/gui/farhelm-gui/src/session_list/rename/tests.rs
`a_reorder_under_an_open_editor_keeps_its_draft_caret_and_focus`. The reload is the channel's snapshot in the stored
order (crates/farhelm-helm/src/ui_channel_tests.rs `a_channel_opens_in_the_stored_list_order_or_most_recent_activity`),
and the second client has nothing to port, since one helm serves one window.

### `tests/bulk-session-actions.spec.ts`

Tests: 14. Covers: marking retains New dispatch and launcher cancellation focus.

partly: crates/gui/farhelm-gui/src/session_list/marking_tests.rs ports the cases that need no batch, and
crates/gui/farhelm-gui/src/session_list/bulk_tests.rs the batch cases.
`marking retains New dispatch and launcher cancellation focus` (`the_new_action_opens_the_launcher_while_marked`, which
also cancels it back to the ⋯; the launcher's return rule in general is the launcher deviation below);
`retained mark keeps the open Trash dialog` (`the_trash_dialog_survives_marks_appearing`; focus after closing it is a
deviation below); `range and toggle keep the open session`
(`marking_clicks_open_nothing_and_a_plain_click_opens_and_clears`, `a_range_follows_the_order_on_screen_not_the_helms`,
with the range in the held order, a hidden anchor and the open row's toggle as pure tests; GPUI has no text selection to
keep, and the screenshots are the `session-list-marked` golden); `Escape yields to menus and the terminal`
(`escape_in_the_list_clears_the_marks_and_goes_on`, `escape_in_a_menu_keeps_the_marks`,
`escape_on_the_bar_hands_focus_to_the_order_title`; the terminal is outside the list's column, so it never sees the
list's Escape, and is not driven here); `metadata acts only on displayed eligible rows`
(`metadata_writes_go_once_per_eligible_row_and_clear_the_marks`,
`hidden_marks_survive_a_filter_and_hosts_and_are_never_counted`); `shortcuts act on the open session`
(`session_shortcuts_act_on_the_open_session_while_another_is_marked`). The handoff's other marking cases are there too:
`row_controls_neither_mark_nor_clear`, `a_failed_listing_offers_nothing_and_forgets_nothing`,
`only_a_complete_listing_forgets_a_vanished_mark`, `the_bar_replaces_the_heading_and_the_cut_band_stays`,
`the_count_is_focusable_but_not_a_tab_stop`, `a_busy_row_gets_the_not_started_line_until_its_next_operation` and
`the_marked_menu_closes_when_its_offers_change`. The batch cases: `Stop reports an engaged row and runs its sibling`
(`a_refused_row_keeps_its_line_and_its_siblings_go_on`, with an in-flight Rename as the engaged row, and
`a_press_on_the_bar_closes_a_rows_own_question_and_takes_the_row`: the old spec's scripted click left the row's own
Delete question open, but in the app a press on the bar closes it first, so the row is taken like any other);
`Delete rewords and shrinks without gaining targets` (`a_changed_target_rewords_the_question_which_never_gains_one`,
which also checks that the reword leaves focus where it was, `a_status_word_change_rewords_the_question_in_place`,
`a_reword_keeps_focus_where_the_user_put_it_in_the_question`, `the_question_closes_once_no_target_is_left` (and
`closing_the_question_leaves_a_warning_an_approval_card_opened_above_it`, which keeps the old app's closing of the bulk
dialog alone when an approval card's tunnel warning is above it), and
`a_bulk_delete_question_lists_each_target_and_cancel_keeps_the_marks` for the rows, the full host, the managed
checkout's line, the "Sessions affected" scroll stop and Cancel's focus on the ⋯; the hostile title's escaping is the
host-text element's, tested there; the marked bar's Stop and Delete are in the `session-list-marked` golden);
`completion preserves a later intent`, four variants (`completion_keeps_the_marks_of_a_later_question`,
`completion_keeps_a_target_marked_again`, `completion_keeps_the_marks_under_a_later_menu`,
`completion_keeps_the_marks_of_a_later_question_about_them`);
`Stop paces nine rows on one host while another progresses`
(`a_batch_sends_one_request_per_host_and_a_second_batch_keeps_its_own_pace`, with three rows on one host and two on
another, Clear and a second batch while the first runs);
`Delete keeps guards, row errors and cross-host selection exclusion`
(`automatic_selection_waits_for_every_admitted_delete` for the selection across hosts, the guards in
`a_bulk_stop_asks_about_tabs_and_freezes_each_rows_preconditions` and the Delete tests, and the focus handback to the
order's title when a finished batch empties the bar); `direct Delete with and without skip`
(`a_second_batch_skips_the_rows_the_first_still_holds` for ended rows deleted unasked and a second press refusing the
rows the first batch holds, `skip_session_confirmations_runs_bulk_stop_and_delete_unasked`). Also
`a_queued_row_is_pending_until_its_turn_and_free_after_it` (a queued row shows its Stop pending and its own menu refuses
it, as the old app's reservation at acceptance did), `a_stop_batch_holds_no_automatic_selection_back`,
`a_reset_drops_the_queued_rows` and `a_batch_acts_only_on_the_marks_shown`. Retired containerized sessions:
`a_bulk_stop_leaves_a_retired_session_out_and_delete_takes_it`,
`with_only_retired_sessions_marked_the_bar_draws_delete_and_no_stop` and
`with_no_menu_toggle_cancel_returns_focus_to_the_pressed_button`. `archive cues need a visible Trash`:
crates/gui/farhelm-gui/src/trash/cue/tests.rs `an_archive_while_sessions_are_marked_plays_nothing`.
`host attention movement closes the marked menu and reopening remeasures`, which the bulk work and the Hosts attention
list share: crates/gui/farhelm-gui/src/session_list/attention/tests.rs
`menus_below_close_when_the_list_moves_and_stay_through_progress`. The question's "Sessions affected" stop scrolls by
keyboard when its rows overflow under the dialog frame's rule, which the prompt reuses: the stop itself is
`a_bulk_delete_question_lists_each_target_and_cancel_keeps_the_marks`'s region, its keys
crates/gui/farhelm-gui/src/components/tests/dialog.rs `the_scroll_keys_scroll_a_dialog_body_through_its_stop` and
crates/gui/farhelm-gui/src/prompt_tests.rs `a_question_given_a_scroll_stop_lets_the_keyboard_reach_its_text`. Dropped: a
golden of the bulk question. The golden set takes no new pictures since 2026-11-16 (decided by the GPUI migration's
orchestrator), and an app scene, which stands in for one, cannot show the question: the bridge's command line
(`scripts/gui-agent.py`'s `click`, which passes on `gpui-agent`'s) sends no modifier keys, so it cannot mark a row, and
SPEC.md has no keyboard way to mark. The question's rows and words are
`a_bulk_delete_question_lists_each_target_and_cancel_keeps_the_marks`'s. The bridge's protocol does carry modifiers on a
mouse-down, so a modifier option on its `click` would make a scene and an end-to-end smoke step possible. Waiting for
bulk environment Delete: sandbox and recipe container rows' environment-wide cases.

### `tests/menu-highlight.spec.ts`

Tests: 5. Covers: session menu shares one highlight between hover and arrow keys.

partly (the session menu's case): crates/gui/farhelm-gui/src/session_list/menu_tests.rs
`the_pointer_and_the_arrows_move_one_highlight_and_enter_runs_it` (the pointer and the arrows move one highlight, the
pointer leaving keeps it, Enter runs the hovered Clone) and
`down_and_up_on_the_closed_button_open_the_menu_on_its_first_and_last_command`; the host, header caret, order and Help
menus draw the same item, whose fill is focus alone, but their cases and the hover help's pointer-rest case are not yet
ported

### `tests/retired-container-sessions.spec.ts`

Tests: 1. Covers: retired container rows offer Delete only while new containers keep normal controls.

ported: crates/gui/farhelm-gui/src/session_list/retired_tests.rs `a_retired_rows_title_says_so_in_either_density`
("Retired container · <title>" in the row's name and on screen, compact too),
`a_retired_rows_menu_holds_delete_alone_and_says_why` (the menu's one item, Delete, focused on opening; the header's
summary ending with the retired reason; the ordinary question and one unguarded `DELETE /api/sessions/<id>`; its refusal
on the row), `a_retired_row_draws_no_bell_and_no_seen_toggle`,
`opening_a_retired_session_stamps_nothing_and_rename_opens_nothing` (the old list's metadata fence),
`the_marked_bar_leaves_a_retired_row_out_of_every_batch` (Pin, Unpin, Mark read and Mark unread skip it, as the old
metadata batches did) and `an_ordinary_container_sessions_row_keeps_its_commands` (the spec's other half: a session on a
session-owned container host keeps every command); crates/gui/farhelm-gui/src/session_pane/retired_tests.rs
`a_retired_session_opens_on_its_card_with_no_terminal` (no stream, terminal or tab strip; the card with the reason,
between the top row and the details bar, holding focus), `the_header_offers_delete_alone_and_shows_its_refusal` (no
Stop, Restart, Replace, Clone, carets or text size; Delete asks, sends one Delete, and its refusal shows under the top
row), `a_row_that_turns_out_retired_drops_its_terminal`,
`an_unreachable_retired_session_shows_the_unavailable_card_alone` and
`an_ordinary_container_session_keeps_its_terminal_and_header` (its stream, tab strip and five actions);
crates/farhelm-proto/src/session_launch.rs `only_a_stored_container_choice_marks_a_launch_retired`. The ordinary
container session's Suspend and the rest of its environment controls are the environment sessions' (its menu's Suspend
and Stop and suspend are in `an_ordinary_container_sessions_row_keeps_its_commands`, and "Sidebar and session view cases
of specs listed in other areas' files" below). Leaving retired sessions out of a pin drag is
crates/gui/farhelm-gui/src/session_list/pin_drag_tests.rs `a_retired_pin_has_no_grip_but_is_a_drop_target`, and leaving
them out of the marked bar's Stop (Delete keeps them) is in `tests/bulk-session-actions.spec.ts`'s tests above. No
screenshot scene and no smoke step: the bridge seeds sessions only through the helm's create, which refuses the retired
launch, so no app run can show one. Recorded deviations: the card is a keyboard stop where focus rests, where the old
card was no stop; SPEC.md, "Where focus rests", names no home for a retired session, so the card was made home by
analogy with the other cards that stand where a terminal would be (the migration's orchestrator's brief of 2026-11-15);
and on an unreachable host the unavailable card stands alone in its place.

### `tests/ui-plan-regressions.spec.ts`

Tests: 5. Covers: header restart and replace refresh the sidebar under build skew.

partly (the header's Restart and Replace): crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`restart_asks_only_while_the_agent_works_and_sends_the_consent_it_was_given`,
`the_headers_replace_asks_the_rows_question_and_sends_its_guard`; scripts/gui-e2e-smoke.py
`test_38_restart_from_the_header_resumes_the_session`. The sidebar needs no refresh after them (the helm pushes the
change before it replies), and build skew has no counterpart (the app and its helm are one build); the tab, rename,
host-menu and host-permission cases are other areas' and stay unported

### `tests/session-confirmations.spec.ts`

Tests: 4. Covers: restoring confirmations reports save failure and recovers.

ported, its four tests as follows.

`skip session confirmations walks row header interrupted card and tab actions`: the row's Stop, Replace and Delete,
crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs
`skipped_confirmations_stop_and_replace_on_the_click_with_their_rows_guards` and
crates/gui/farhelm-gui/src/session_list/menu_tests.rs `skipped_confirmations_delete_on_the_click_with_the_rows_guard`;
the header's Restart, crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`a_skipped_restart_question_goes_ahead_with_consent_for_the_working_agent` (the header's Stop, Replace and Delete skip
their questions through the same row functions, not tested separately); the interrupted card's Replace,
crates/gui/farhelm-gui/src/session_pane/card_tests.rs `skipped_confirmations_replace_from_the_card_at_once`; and a tab's
Close, the terminal area's crates/gui/farhelm-gui/src/terminal/set/tests.rs
`skipped_questions_close_at_once_side_by_side_and_a_refusal_shows_on_its_tab`.

`Escape cancels header restart replace and delete questions and restores terminal focus`: header_tests.rs
`escape_returns_to_the_terminal_and_cancel_to_the_button`.

`restoring confirmations reports save failure and recovers`: Settings' Skip session confirmations switch saves its own
field (crates/gui/farhelm-gui/src/settings/tests.rs
`each_switch_saves_its_own_preference_field_under_the_preferences_key`), and a refused save puts the switch back with
its failure line (`a_refused_save_restores_the_stored_value_and_shows_the_error_beside_the_switch`, shown on another
switch under the same rule); the questions come back once the helm stores the choice, since each reads the stored
preference, a change from the old app that [Settings and hosts](settings-and-hosts.md) records among its deviations.

`superseded restoration cannot report an older failure` is superseded: Settings sends no second save while one is in
flight, refusing the press with its busy line, a second click on the saving switch included (settings/tests.rs
`a_press_during_another_settings_save_says_why_it_sent_nothing`), so no newer choice of the switch ever stands behind an
older save whose failure could show. Where the list's own preferences do queue a newer choice behind a write in flight,
a refused write shows nothing at all (`session_list/order/tests.rs`
`a_choice_queued_behind_a_refused_write_is_still_sent`).

### `tests/delete-precondition.spec.ts`

Tests: 3. Covers: an ${skip ? "opted-out tabs-only" : "unconfirmed"} delete sends the precondition and shows the
supervisor's refusal.

partly: crates/gui/farhelm-gui/src/session_list/menu_tests.rs
`each_delete_carries_the_precondition_of_what_its_row_showed`,
`a_question_answers_once_for_what_it_showed_even_after_the_session_changed`,
`skipped_confirmations_delete_on_the_click_with_the_rows_guard`,
`a_refusal_shows_on_the_row_until_the_next_delete_starts`; the header's question is
crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_headers_delete_asks_even_with_nothing_alive_and_its_progress_replaces_the_actions`

### `tests/replace.spec.ts`

Tests: 18. Covers: plain Replace retries an unknown outcome with the same key (${surface}).

partly (the row's plain Replace): crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs
`replace_always_asks_and_carries_the_delete_guard_of_its_row`,
`an_answer_sends_the_preconditions_its_question_was_drawn_from`,
`a_refusal_shows_on_the_row_and_in_its_menu_until_the_next_operation` (a refusal, and one without the helm's YOLO marker
even when its words speak of YOLO, as "Replace: …"), `a_yolo_refused_replace_asks_the_launchers_question_in_its_row`,
`the_one_off_answer_replaces_again_with_consent_exactly_once`,
`the_permanent_answer_writes_the_host_setting_then_replaces_with_consent`,
`cancel_drops_the_question_and_gives_focus_back_to_the_rows_button`,
`a_yolo_question_that_takes_focus_scrolls_its_row_into_view`,
`a_yolo_question_below_its_rows_last_line_is_scrolled_into_view_too`, `the_question_names_the_sources_host`,
`an_open_yolo_question_refuses_the_sessions_other_lifecycle_actions`,
`a_press_during_the_host_write_keeps_the_replacement_from_taking_focus`, `a_reset_drops_the_question`,
`a_replace_of_the_open_session_opens_its_replacement_and_focuses_it`,
`a_replace_confirmed_with_a_click_returns_focus_like_enter`,
`a_replace_after_the_user_moved_on_opens_its_replacement_without_focus`,
`a_replace_after_the_user_opened_another_session_leaves_them_there`,
`a_replace_refused_after_its_source_went_lets_the_selection_move_on`,
`a_replaces_delete_notice_shows_in_the_band_while_the_replacement_has_work_running` (the reply's `delete_notice` in the
Delete notice band, unprefixed, as the old app showed it); scripts/gui-e2e-smoke.py
`test_34_stop_and_replace_from_the_row_menu`. The retry-key cases are dropped by design (the GUI writes no retry keys:
SPEC_impl.md, "One failure path: the reset"); the row question's Cancel help is the old row's ("Cancel: keep this
session as it is"); the header's Replace is crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_headers_replace_asks_the_rows_question_and_sends_its_guard`,
`a_yolo_refused_replace_from_the_header_asks_below_its_top_row`,
`cancelling_the_headers_question_gives_the_terminal_its_height_back` and
`a_header_question_shows_at_its_row_while_its_session_is_not_open`; scripts/gui-e2e-smoke.py
`test_54_a_yolo_replace_asks_in_its_row_and_the_one_off_answer_replaces`; the Replace with triggers are
crates/gui/farhelm-gui/src/session_list/menu_tests.rs `the_menus_clone_and_replace_with_open_the_launcher_on_their_row`
and `the_menus_launcher_commands_wait_while_a_host_change_runs` (the row menu's) and
crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_with_choices_and_clone_open_the_launcher_and_return_focus_on_cancel` (the header's Replace menu), while the Replace
with cases themselves are the launcher area's, in [Launcher](launcher.md)'s section for other areas' specs; the
interrupted card's Replace and its YOLO question are crates/gui/farhelm-gui/src/session_pane/card_tests.rs
`the_cards_replace_asks_inline_and_sends_the_guard_it_froze` and `a_yolo_refused_replace_from_the_card_asks_in_the_card`

### `tests/restart-replace-progress.spec.ts`

Tests: 6. Covers: plain Restart shows host and elapsed time until a real refusal arrives.

partly: crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs
`a_replace_says_after_two_seconds_how_long_it_has_waited` (the row's Replace);
crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`a_restart_says_after_two_seconds_how_long_it_has_waited_and_then_its_refusal` (the header's Restart line, below the top
row, and "Restart: …" on the header and the row), `every_action_is_whole_and_reachable_in_a_650_pixel_pane` (with a
Restart's line naming a long host drawn below the row, every action stays whole at 650px). The header shows a Replace's
line from the same clock and function (`outcome::wait_line`), and only the header's copy is announced
(`only_the_headers_copy_of_a_wait_line_is_announced`); the interrupted card draws its Restart's line and refusal in the
card and the header leaves its copies out (crates/gui/farhelm-gui/src/session_pane/card_tests.rs
`the_cards_restart_is_one_click_and_its_wait_and_refusal_show_in_the_card` and
`the_cards_replace_shows_its_wait_and_refusal_in_the_card`); the 60-second "Still waiting" wording is not ported, since
the GUI has no per-command deadline and the helm answers a slow host within its 20-second bound

### `tests/session-action-completion.spec.ts`

Tests: 3. Covers: row Replace YOLO arrival respects ${focus} focus.

ported: crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs
`a_pending_stop_shows_beside_the_title_and_refuses_only_its_own_sessions_actions` ("Stopping…" on the row, the session's
actions usable again once the reply lands), `a_pending_stop_or_replace_holds_no_automatic_choice_back` (pending Stop and
Replace do not delay automatic selection), `a_yolo_question_after_the_user_moved_on_waits_without_focus` and
`a_yolo_refused_replace_asks_the_launchers_question_in_its_row` (the YOLO question's arrival, without and with focus;
the case is the launcher's in its parity file, whose row should point here, which the migration's orchestrator has asked
the launcher area to do). `Stop settles before its held status refresh`:
crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs `a_stops_reply_alone_frees_its_sessions_clone` (Clone works
once Stop's reply lands, with its notice on the row and no listing awaited); New terminal waits on the same session key
(crates/gui/farhelm-gui/src/terminal/set/tests.rs `new_terminal_is_refused_while_a_lifecycle_request_is_pending`), so it
is free again with Clone by construction, with no test of its own after the reply.
`bulk Stop completion leaves the bystander terminal focused`: crates/gui/farhelm-gui/src/session_list/bulk_tests.rs
`a_finished_stop_batch_leaves_the_open_sessions_terminal_focused`

### `tests/session-action-shortcuts.spec.ts`

Tests: 4. Covers: browser Delete chord shares header confirmation and restores terminal focus without forwarding input.

partly: crates/gui/farhelm-gui/src/session_pane/tests.rs
`the_delete_shortcut_from_a_focused_terminal_asks_and_never_reaches_the_program`,
crates/gui/farhelm-gui/src/session_list/menu_tests.rs `the_shortcut_does_nothing_with_no_session_open_or_a_menu_open`,
crates/gui/farhelm-gui/src/session_list/rename/tests.rs
`the_shortcut_opens_rename_for_the_open_session_and_escape_returns_to_its_terminal`,
`closing_a_shortcut_opened_editor_returns_focus_to_the_terminal`,
crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_delete_shortcut_asks_the_headers_question_even_with_nothing_alive` (the chord asks the header's Delete question,
even for an ended session with no tabs), crates/gui/farhelm-gui/src/session_list/menu_tests.rs
`the_shortcut_asks_about_the_open_session_and_waits_while_a_question_is_open`; the Rename chord on a filtered open
session (its untouched-save case aside, and its second-client case, since one helm serves one window) in
crates/gui/farhelm-gui/src/session_pane/carried_tests.rs
`a_filtered_out_open_session_keeps_its_terminal_header_details_bar_and_rename`

### `tests/header.spec.ts`

Tests: 12. Covers: native tunnel preview keeps ports and actions inside a 650px pane.

partly (the actions, their order, names, hover help, questions and the 650px floor, and the details bar: its status,
title, host, age, managed-checkout mark and copy fields): crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_row_draws_stop_restart_replace_and_delete_in_the_old_order`, `each_actions_hover_help_is_the_old_headers_tooltip`,
`the_headers_replace_asks_the_rows_question_and_sends_its_guard`,
`the_headers_delete_asks_even_with_nothing_alive_and_its_progress_replaces_the_actions` (the header Delete confirms in
place, and a committed one shows its progress until the reply lands),
`escape_returns_to_the_terminal_and_cancel_to_the_button`, `every_action_is_whole_and_reachable_in_a_650_pixel_pane`
(the real text system; with nothing in the left region yet it pins only that the four actions fit whole on one line, and
becomes a real guard once the tab strip lands); scripts/gui-e2e-smoke.py
`test_38_restart_from_the_header_resumes_the_session`; the header's half of "header actions compact only for measured
tunnel pressure and recover when it leaves" and "native tunnel preview keeps ports and actions inside a 650px pane":
crates/gui/farhelm-gui/src/session_pane/header_tests.rs `a_pill_at_650_pixels_condenses_the_actions_beside_it` (at 650
pixels with a pill on a long host, for an ordinary and an environment session: every action, both carets, the text size
group and Suspend whole on one line, drawn once, reachable by Tab, the pill whole at no less than its floor and left of
the actions, Stop condensed, the same look on the following frames, the ordinary look back once the same row is widened
to 1400 pixels and the condensed one again at 650) and `a_wide_pane_keeps_the_ordinary_actions_beside_a_pill` (1400
pixels: the ordinary look with the pill), the rule's arithmetic in crates/gui/farhelm-gui/src/session_pane/header/fit.rs
`the_actions_condense_only_when_a_pill_and_the_rest_do_not_fit`, and the condensed box in
crates/gui/farhelm-gui/src/components/tests/button.rs
`a_condensed_compact_button_gives_up_only_padding_text_size_and_line_height`;
crates/gui/farhelm-gui/src/session_pane/details_tests.rs
`the_bar_draws_status_title_host_and_age_in_order_for_each_kind_of_status` (the field order status, title, host, age for
a running, an idle, an exited and an unclassified session, the bar below the terminal, `Local (this machine)` with one
host mark, one badge, the ended badge's words), `each_parts_hover_help_is_its_whole_value` (the title's tooltip is the
whole title, the ended badge's the whole status), `a_stale_session_draws_no_badge_and_no_age`,
`no_host_name_draws_no_host_and_an_unplaced_host_no_mark`,
`a_remote_host_is_named_in_full_and_only_the_local_host_reads_local`, `the_age_moves_on_with_the_shared_clock`,
`an_oversized_title_is_clipped_and_the_mark_and_age_stay_whole` (the real text system: an oversized title clipped with
`…`, the live mark and the age whole, on one line); crates/gui/farhelm-gui/src/session_pane/carried_tests.rs
`a_filtered_out_open_session_keeps_its_terminal_header_details_bar_and_rename` (the actions and the bar of an open
session a filter hides) and `this_windows_delete_of_a_hidden_open_session_opens_the_next_session` and
`a_carried_session_deleted_elsewhere_moves_the_selection_on` (deleted elsewhere, by the command line or an agent), and
`a_carried_session_cut_away_on_its_own_host_stays_open` (an old open session its own host's cap left out, under a host
filter naming another host, stays open); crates/gui/farhelm-gui/src/session_pane/details_tests.rs
`the_copy_fields_show_name_help_and_copy_their_values` (the `~` fold, names and `— click to copy` tooltips, the
clipboard getting the exact `cwd`, no warning for an ordinary value),
`copying_hidden_characters_warns_upward_while_the_click_is_confirmed` (escaped shown value, no raw U+202E painted, raw
bytes copied, the warning's wording, above the bar, unclipped and inside the window, gone after 1.5 s),
`the_warning_survives_a_refresh_that_makes_the_value_plain` (the old safe-metadata-refresh case for the command; the
folder's warning wording is `copying_a_hidden_character_in_the_directory_warns_for_the_folder`),
`the_checkout_mark_names_the_repository_and_explains_the_checkout`, `the_copy_fields_follow_the_terminal_in_tab_order`,
`a_confirmation_does_not_follow_another_selection`, `an_oversized_title_is_clipped_and_the_mark_and_age_stay_whole` (a
long command cut too), `the_copy_fields_give_way_before_the_title_and_host` (SPEC's order, see the deviations),
`an_empty_or_blank_value_shows_the_old_marker_and_copies_the_raw_bytes`,
`the_checkout_mark_explains_an_environments_checkout`, `the_warning_wraps_inside_a_narrow_window`;
crates/gui/farhelm-gui/src/components/tests/copy_field.rs (the 1.5 s confirmation per click, restarted by a second
click, and Enter; the glyph under the pointer and keyboard focus); the `session-details-bar` (field order, free width)
and `session-details-copy-warning` goldens. The accessible descriptions are the hover help's text by construction
(`components/tests/hover_help.rs`), since GPUI's driver record has no description to read; copy end to end is not a
smoke step, since headless Linux has no clipboard. The interrupted card's half of "header pressure follows tab strips
added and replaced after interruption" (no terminal or strip while interrupted, new ones on recovery) is
crates/gui/farhelm-gui/src/session_pane/card_tests.rs `interruption_drops_the_terminal_and_recovery_mounts_a_new_one`.
The carets and Clone: crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`clone_and_the_split_carets_sit_in_the_old_order_with_the_old_words` (the nine controls' order, each caret joined to its
main part, names and hover help), `a_caret_menu_walks_its_choices_and_escape_or_tab_returns_to_the_caret` (the split
menus' keyboard case: Down, Up, Home and End, Escape and Tab back to the caret and the next Tab going on, Down or Up and
Enter opening the closed menu), `a_menus_plain_choice_asks_the_main_parts_question` (the repeated menu action keeps the
main part's question), `the_with_choices_and_clone_open_the_launcher_and_return_focus_on_cancel` (the "open the right
flows" half, and Cancel of Restart with back on the persistent caret),
`an_unavailable_choice_stays_in_its_menu_with_its_reason`, `clone_and_the_with_choices_wait_while_a_host_change_runs`
(Clone and the choices say why they wait), `a_caret_menu_closes_when_its_session_is_deleted_elsewhere`, the two
650-pixel tests with the carets and Clone in the row; crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs
`an_open_yolo_question_refuses_the_sessions_other_lifecycle_actions` (the carets and Clone refused with the rest);
scripts/gui-e2e-smoke.py `test_69_the_headers_replace_with_and_clone_open_the_launcher`; the `session-row-menu` golden.
Space on a caret and switching from one caret's open menu to the other's by a click are not pinned, nor are, of the
compact cases, a pill's content changing in place across the boundary, a pill hidden while mounted or closed (only the
pane's widening brings the ordinary look back in a test), a strip replaced after an interruption (the fit measures the
strip it is drawn with, every frame) and the condensed row's open menus and questions, which keep their size (they are
drawn by the layer manager, apart from the row). Waiting on the terminal area's named change: the condensed text size
buttons (the old `.btn` rule gave them 2 pixels of padding too; the terminal area, which owns them, agreed on 2026-11-16
to give the group's drawing a `compact: bool` that the header will pass into its condensed look). The terminal area's:
the text size buttons, the tab strip and the tunnel pill themselves, with the pill's ports, the tab strip's floor beside
a pill and the selected tab's reveal (the terminal area's); the bar's 32-pixel height, its position at the window's
bottom and the macOS title-band inset, which no test pins yet (the goldens and screenshots show them)

### `tests/stop-tabs.spec.ts`

Tests: 7. Covers: legacy HTTP Stop refuses tabs until tab consent is explicit.

partly (the row's Stop): crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs
`stop_asks_only_about_tabs_and_carries_what_its_row_showed`,
`an_answer_sends_the_preconditions_its_question_was_drawn_from`,
`a_refusal_shows_on_the_row_and_in_its_menu_until_the_next_operation`,
`a_stops_notice_stays_on_its_row_until_the_next_operation_starts`,
`a_pending_stop_shows_beside_the_title_and_refuses_only_its_own_sessions_actions`; the legacy HTTP bodies are the helm's
(`crates/farhelm-helm/`), the row question's Cancel help is the old row's ("Cancel: keep the agent and terminal tabs
running"), and the header's question and its "Stopping…" are crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_headers_stop_asks_the_rows_question_and_says_stopping_while_it_runs`

### `tests/terminal.spec.ts` (its row-action cases)

Tests: 18. Covers: a refused Stop closes the menu and shows the helm's words.

partly (Stop; the Delete cases are under `delete-precondition.spec.ts`):
crates/gui/farhelm-gui/src/session_list/lifecycle_tests.rs `stop_asks_only_about_tabs_and_carries_what_its_row_showed`
(no tabs asks nothing), `a_refusal_shows_on_the_row_and_in_its_menu_until_the_next_operation`,
`a_pending_stop_shows_beside_the_title_and_refuses_only_its_own_sessions_actions`; scripts/gui-e2e-smoke.py
`test_34_stop_and_replace_from_the_row_menu` (the helm lists the stopped session as exited). Two clicks landing in one
tick, and Delete and Stop clicked in one tick, are not ported as their own cases (the operation store refuses a second
request on a session); the header's "Stopping…" is crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_headers_stop_asks_the_rows_question_and_says_stopping_while_it_runs`; the rest of the file is the terminal area's
(`docs/gpui/parity/terminal.md`)

### `tests/terminal-restart.spec.ts`

Tests: 16. Covers: an interrupted session's view leads with the resume offer, and declining changes nothing.

partly (the header's plain Restart): crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`restart_says_why_it_is_unavailable_and_sends_nothing` (the offer's names and reasons, greyed and focusable, nothing
sent), `restart_asks_only_while_the_agent_works_and_sends_the_consent_it_was_given` (a working agent asks and
Enter-ready Confirm sends `stop_if_running: true`, an idle one restarts at once with `false`),
`a_skipped_restart_question_goes_ahead_with_consent_for_the_working_agent`;
crates/gui/farhelm-gui/src/session_list/restart.rs `the_hover_help_names_the_offer_then_says_why`;
components/tests/button.rs `a_busy_button_keeps_its_help_and_may_be_named_for_its_reason`,
`a_busy_filled_button_takes_the_retired_look_and_an_unfilled_one_keeps_its_own`; scripts/gui-e2e-smoke.py
`test_38_restart_from_the_header_resumes_the_session`. The case of a question that rewords itself to "The agent has
exited" while open does not arise: the prompt freezes its words when it opens, so the question only ever says the
working agent is stopped first, and its answer consents to that. The interrupted card:
crates/gui/farhelm-gui/src/session_pane/card_tests.rs `an_interrupted_session_opens_on_its_card_with_no_terminal` (no
terminal, no tab strip, the card's words, the details bar's Interrupted badge, focus on the card),
`the_cards_restart_is_one_click_and_its_wait_and_refusal_show_in_the_card` (exactly one unconfirmed Restart),
`a_restart_leaves_the_card_on_its_reply_until_the_list_catches_up` (the helm answers before its list stops saying
Interrupted, so a successful Restart marks the session, as the old `relaunched` did: the card goes and the terminal
attaches on the reply, nothing more is sent, and the mark ends with the first live status) and
`a_restarted_session_whose_host_drops_away_shows_its_card_again` (the mark's other end),
`a_restarted_session_that_leaves_the_listing_shows_its_card_again`,
`a_replace_from_the_card_holds_until_its_reply_and_opens_the_replacement` (no terminal for the removed source; the card
holds until the reply selects the replacement), `an_interrupted_session_whose_host_drops_away_mounts_no_terminal`,
`a_session_back_from_unreachable_already_live_mounts_its_terminal`, `a_reset_forgets_that_the_session_was_restarted`
(the mark is this window's own and goes with a reset), `the_cards_replace_asks_inline_and_sends_the_guard_it_froze` (the
inline question, Cancel, Enter on Confirm), `the_cards_open_question_leaves_the_headers_actions_unavailable`,
`the_cards_open_question_leaves_the_row_menus_actions_unavailable` (Rename, Clone, Replace with…, Replace, Stop and
Delete unavailable and sending nothing while it is open, available again after Cancel),
`the_cards_question_goes_when_its_session_is_deleted_elsewhere`,
`the_cards_question_goes_when_another_session_is_opened`, `the_cards_question_goes_when_its_host_drops_away` (as the old
card cleared its prompt once hidden), `the_cards_restart_and_replace_wait_for_a_host_change` (as the header's and the
row's do), `the_cards_replace_with_says_why_it_waits_while_a_launch_is_pending` (as the row's and the header's Replace
with do), `a_legacy_sessions_card_offers_replace_with_in_place_of_replace` (Replace with, no plain Replace; opening the
launcher is the launcher's), `the_card_is_home_enter_does_nothing_and_tab_reaches_restart_then_replace` (SPEC.md's rule
for the stopped session's card, which the old card never had), and its words per offer, legacy launches included,
crates/gui/farhelm-gui/src/session_pane/card.rs `the_cards_words_say_what_ended_and_what_each_choice_does`; the card's
Replace's YOLO question is in `tests/replace.spec.ts`'s row. No smoke step: no seeded session is interrupted, and
staging a reboot means quitting the shared run's app, killing its supervisor and every terminal host (every other step's
sessions with them), rewriting the supervisor's stored boot id and relaunching, so the card was driven by such a run of
its own instead (recorded in the PR). The old retired-container card is `tests/retired-container-sessions.spec.ts`'s
row. Recorded deviations: the card's question is the prompt's plain inline look, with its title line ("Replace
confirmation") and its answers at its end, where the old card drew the consequence and the two buttons in one wrapped
row; and while it is open the card's Replace keeps the primary fill, which is the pressed look too, where the old one
turned to the accent's hover fill. Other areas': Restart with… beyond its entry in the header's Restart menu is the
launcher's, in [Launcher](launcher.md) (the entry is crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`the_with_choices_and_clone_open_the_launcher_and_return_focus_on_cancel` and
`an_unavailable_choice_stays_in_its_menu_with_its_reason`), and the scrollback and lost-reply cases after a Restart are
the terminal area's. The old card's button padding of 4 pixels is not matched: the card's buttons are the ordinary
6-pixel ones (a deviation below). A restart's reattach (the terminal's half: the restart's detach reconnects by itself,
with no Reconnect offered, and the new run's replay replaces the old screen in one redraw) by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_restarts_detach_reattaches_by_itself_and_shows_the_new_run_in_one_redraw`. The detach-banner case after a Restart ("a
restarted session reconnects without a detach banner") by crates/gui/farhelm-gui/src/session_pane/tests.rs
`a_restart_reattaches_without_a_card_or_a_red_band` (no reconnect card, red band or status line at any step while the
Restart is pending), with `a_restarts_failed_reattach_shows_the_card` and `a_restart_does_not_hold_back_a_stall` for the
hold's ends.

### `tests/terminal-multihost.spec.ts` (stale)

Tests: 3. Covers: supervisor-outage: retain the screen behind one calm card and reattach promptly.

partly (its stale cases; the rest of the spec is in [Terminal](terminal.md)):
crates/gui/farhelm-gui/src/session_pane/unavailable_tests.rs
`a_session_whose_host_drops_away_keeps_its_terminal_inert_under_the_card` ("supervisor-outage": the retained terminal
and its strip inert under one card, its stream kept, focus going from the terminal to the card, which is home while it
shows, and given back to the terminal once the session reads reachable, the same stream live again; a behavior change
from the old app, which left nothing focused under its card because the browser blurred the inert terminal),
`a_layer_closed_after_the_host_drops_leaves_focus_on_the_card` (a question opened from the terminal and closed after the
host dropped leaves focus on the card, whose stop takes Page Down),
`a_tab_that_arrives_under_the_card_leaves_focus_on_the_card`, `focus_elsewhere_stays_as_the_card_comes_and_goes` (focus
left on another control is not taken, and recovery does not steal it),
`the_terminals_own_notices_stay_hidden_while_the_card_shows` (no reconnecting band or detach banner under the card),
`a_session_opened_unreachable_shows_the_card_and_mounts_no_terminal` ("stale-session-metadata-view": no terminal and no
strip, the heading, the explanation and the transport's error as diagnostics, the last-known status and age in the band
and not in the details bar, the card between the top row and the bar, focus on the card),
`a_session_opened_unreachable_mounts_its_terminal_when_the_host_answers`,
`choosing_an_unreachable_session_focuses_its_card`, `a_stale_row_that_goes_away_leaves_no_inert_terminal_under_nothing`,
`the_cards_spinner_turns`, `the_bands_age_moves_on_with_the_shared_clock`,
crates/gui/farhelm-gui/src/session_pane/card_tests.rs `a_session_opened_unreachable_takes_no_interrupted_card` (stale
beats interrupted: no stream, no strip, the unavailable card), `the_card_is_a_scroll_stop_that_tab_reaches` (Shift+Tab
from the first copy field reaches the card, and End and Home scroll it in a window of the app's minimum height), and the
words per cause, `the_cards_words_name_the_cause_the_remedy_and_the_evidence` and
`a_session_without_a_host_name_is_still_named_and_a_name_is_escaped`; the card's colors and layout, over the terminal's
ground and veil as the old overlay's scrim lay over the empty terminal area, by the `session-unavailable` golden. No
smoke step: a stale session needs its host unreachable, and in the smoke's one shared app run that means stopping the
run's only supervisor, which makes every other step's sessions stale too; the card was driven by a bridge run of its own
instead, which stops and restarts that run's supervisor (recorded in the PR). Not ported: the card's fixed, full-height
layout in a short window (`.outage-viewport`), since the window's 560-pixel minimum height leaves the card's region
several hundred pixels; the region scrolls instead. The case's "a Replace refusal shows" is the header's line, as for
any session; the case's wheel scrolling and "the document never scrolls" are GPUI's own scrolling. The Resume host
button for a suspended or stopped host is the environment sessions' (`the_suspended_card_says_so_and_resumes` and
`the_cards_words_and_resume_follow_the_host`, under "Sidebar and session view cases of specs listed in other areas'
files" below). "reboot-*", the interrupted card's cases, are in `tests/terminal-restart.spec.ts`'s row through
crates/gui/farhelm-gui/src/session_pane/card_tests.rs

### `tests/terminal.spec.ts` (its stale cases)

Tests: 2. Covers: stale-session-view-with-no-status-renders-metadata-and-no-badge.

partly (its stale cases; the rest of the spec is in [Terminal](terminal.md)):
"stale-session-view-with-no-status-renders-metadata-and-no-badge" by
crates/gui/farhelm-gui/src/session_pane/unavailable_tests.rs
`the_band_keeps_the_last_known_status_and_age_of_a_stale_session_only` (a session nobody classified has its age and no
status in the band) with crates/gui/farhelm-gui/src/session_pane/details_tests.rs for the bar's metadata; "opening a
stale session shows metadata and calm terminal recovery" by the same metadata tests for its metadata. Its terminal half,
a first attach the helm refuses or detaches ("no such session on this host entry") shown as the calm "Session
unavailable — reconnecting" card with no banner, is a deviation below: the GPUI terminal shows its red band with
Reconnect there, by the terminal area's crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`detached_and_refused_offer_reconnect` and `detached_refused_and_closed_streams_keep_the_red_band`, and keeps the card
for a drop it recovers from by itself (`a_dropped_stream_shows_the_reconnect_card_over_the_kept_screen`)

### `tests/session-view-claims.spec.ts`

Tests: 2. Covers: navigation survives another client deleting a session with a Restart prompt.

ported: crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`a_session_deleted_elsewhere_leaves_the_next_sessions_header_usable` (the open session deleted elsewhere, by the command
line or an agent, while its header Restart question is open, and while its Restart runs)

### `tests/quick-switcher.spec.ts`

Tests: 8. Covers: opening a template reports the launcher's catalog read failure.

partly: crates/gui/farhelm-gui/src/session_list/quick_switcher/tests.rs
`subsequences_keep_the_original_character_positions`, `title_matches_come_first_and_each_group_keeps_the_helms_order`,
`matching_reads_raw_values_not_their_escaped_spelling`,
`the_chord_from_a_terminal_opens_the_switcher_and_never_reaches_the_program`, `the_chord_does_nothing_over_a_menu`,
`the_chord_does_nothing_before_the_preferences_arrive`, `loading_picks_nothing_and_a_failed_read_says_so`,
`a_reply_is_listed_in_ranking_order_and_a_cut_reply_says_so`,
`a_reply_to_an_earlier_opening_never_reaches_a_later_switcher`, `the_arrows_move_the_selection_and_stop_at_both_ends`,
`the_selected_row_stays_in_view`, `a_pick_opens_the_session_remembers_it_and_focuses_its_terminal`,
`escape_and_close_return_focus_to_where_it_was`, `a_press_outside_closes_it_and_returns_focus`,
`marks_are_found_on_the_shown_spelling_of_a_matched_value`, and for a pick the sidebar's filters hide
`a_pick_the_filters_hide_resets_both_and_a_shown_pick_keeps_them` and
`a_pick_leaves_the_hosts_panel_and_a_refused_widening_keeps_both_choices`, and for the `New session named` row
`the_new_row_follows_the_matches_and_is_selected_when_nothing_matches`,
`loading_keeps_the_new_row_from_the_keys_and_a_failed_read_leaves_it_usable`,
`enter_on_the_new_row_opens_new_named_and_closing_it_returns_focus`,
`a_click_on_the_new_row_opens_new_and_returns_from_the_hosts_panel` and
`a_refused_new_closes_the_switcher_and_returns_focus`, and for the `tl:` templates section and the Quick launch row as
the switcher draws and reaches them `a_tl_query_lists_templates_in_the_helms_order_without_waiting_for_the_read`,
`a_failed_template_read_shows_its_reason_in_place_of_the_rows`,
`enter_on_a_template_opens_new_with_it_applied_and_closing_returns_focus`,
`a_click_on_a_template_opens_new_and_returns_from_the_hosts_panel`,
`the_quick_launch_row_follows_the_matches_and_names_the_mark` and
`the_quick_launch_row_opens_templates_or_launches_the_mark`, with the templates area's entry points they call (the `tl:`
label, the rows' matching and order, a failed template read's words, what a pick does in New, waiting for the catalog
and refusing in the old switcher's words, and the Quick launch row's label and action) in
crates/gui/farhelm-gui/src/templates/rows_tests.rs `a_leading_tl_label_makes_a_template_query`,
`rows_match_a_lowercased_substring_in_order_with_summaries_and_shortcuts`,
`unavailable_templates_give_the_reason_instead_of_rows`,
`a_switcher_pick_opens_new_with_the_template_and_never_launches`,
`a_switcher_picks_refusals_are_the_old_switchers_words`,
`the_quick_launch_row_names_the_mark_and_launches_or_opens_templates` and
`a_pick_applies_its_template_once_the_catalog_is_read`; crates/gui/farhelm-gui/src/actions_tests.rs (the held chord,
from a terminal too); scripts/gui-e2e-smoke.py `test_32_the_quick_switcher_opens_a_session_found_by_typing` and
`test_47_new_and_the_switchers_new_row_open_the_launcher`. Deviation: New's cancellable loading state for a template
pick never shows, since the launcher has none (the templates area's). Deviation: the Quick launch row's label may run on
past the title column into the host's, and its chord is never cut, where the old row cut both at their columns ("Quick
launch: Mark a te…", "⌘P / Ctrl+…" in the reference screen). Deviation: the `tl:` section has no "Loading templates…"
state, since templates are pushed state the window already holds when the switcher opens.

### `tests/host-picker.spec.ts`

Tests: 1. Covers: quick switcher draws a session's chosen host mark.

ported: crates/gui/farhelm-gui/src/session_list/quick_switcher/tests.rs
`a_row_draws_its_hosts_chosen_mark_before_the_hosts_name` (the rocket drawn beside the host's name) and
`an_unplaced_host_draws_no_mark_and_keeps_its_name` (its quick switcher case; the host picker cases are in Launcher)

### `tests/terminal-replay-rename.spec.ts`

Tests: 19. Covers: rename exposes peer title and refusal characters and seeds an escaped draft.

partly (the rename cases): crates/gui/farhelm-gui/src/session_list/rename/tests.rs
`an_untouched_save_sends_the_raw_title_and_success_closes_the_editor`,
`a_typed_title_is_sent_verbatim_and_an_empty_one_too`, `a_refusal_keeps_the_draft_and_shows_the_helms_words_escaped`,
`a_complete_listing_without_the_session_disables_save_and_keeps_the_draft`; scripts/gui-e2e-smoke.py
`test_05_rename_from_the_row_menu_retitles_the_session`; and Rename's naming mode after a quick launch, whose
quick-launch half the templates area ported: crates/gui/farhelm-gui/src/templates/naming_tests.rs
`an_unseen_quick_launch_opens_naming_and_a_typed_name_renames`, `unchanged_enter_and_escape_keep_the_created_name`,
`a_byte_identical_save_keeps_the_name_and_a_one_character_change_renames`,
`cancel_and_close_return_to_the_new_terminal`, `naming_opens_for_a_session_the_listing_leaves_out`,
`naming_is_off_when_its_preference_is`, `a_launch_that_revealed_new_is_not_named` and
`a_quick_launch_the_user_moved_on_from_is_not_named`. The terminal's catch-up cases: a tab's replay staged on its own
and shown at the bottom by crates/gui/farhelm-gui/src/terminal/set/tests.rs
`a_tabs_replay_is_staged_on_its_own_and_shown_at_the_bottom`; a reaped chosen tab falling back to the agent terminal
without taking focus from an open header question by `a_reaped_tab_leaves_focus_on_an_open_header_question`; a header
Delete or Replace question keeping the keyboard while the terminal goes live by
crates/gui/farhelm-gui/src/session_pane/tests.rs `a_delete_question_keeps_the_keyboard_while_the_terminal_goes_live` and
`a_replace_question_keeps_the_keyboard_while_the_terminal_goes_live`.

## Sidebar and session view cases of specs listed in other areas' files

These specs' entries sit in [Launcher](launcher.md) (`tests/tensorlake-sandbox-sessions.spec.ts`, wholesale) and
[Settings and hosts](settings-and-hosts.md) (`tests/podman-environments.spec.ts`); their session list and session view
cases are this area's. Each case is listed by its title at reference commit `24bc0580`. The tests are in-process over
the fake helm: no running-app check of an environment session is possible, since the end-to-end harness seeds no sandbox
or container (decided by the migration's orchestrator on 2026-11-15; one real container run happens at milestone M3's
stop). The quick switcher keeps drawing a session's host mark, not its environment's, as the old switcher did; the row
and the details bar draw the environment's.

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: Resume restores owned controls and Stop and suspend keeps its host-wide question.

partly: the card's Resume and its request, Suspend back in the header once connected, and, with skip on, the menu's Stop
and suspend asking "Stop and suspend sandbox?" after the activity read, sending one `stop-and-suspend` and no Suspend,
its refusal on the row, and the same refusal below the open session's header (the same test's last check). Replace
with's host is the launcher's.

- crates/gui/farhelm-gui/src/session_pane/environment_tests.rs `the_suspended_card_says_so_and_resumes`
- crates/gui/farhelm-gui/src/session_pane/environment_tests.rs `the_header_suspends_and_resumes_beside_stop`
- crates/gui/farhelm-gui/src/session_list/environment_tests.rs
  `stop_and_suspend_asks_after_its_read_and_sends_the_clicked_guards`

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: Stop and suspend sends the guards its click saw when the listing refreshes during its activity read.

ported:

- crates/gui/farhelm-gui/src/session_list/environment_tests.rs
  `stop_and_suspend_asks_after_its_read_and_sends_the_clicked_guards`

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: an owned session's pending Resume survives switching and excludes its competing actions.

partly: Delete and Clone refused while the Resume is pending and, after the 504 at the bound, while the host is pushed
resuming; Delete enabled while suspended and again after. Switching away and back during the Resume is not driven: the
pending state is the session's operation key and the host's pushed phase, which a switch does not touch.

- crates/gui/farhelm-gui/src/session_pane/environment_tests.rs `the_header_suspends_and_resumes_beside_stop`
- crates/gui/farhelm-gui/src/session_list/environment_tests.rs
  `a_resume_past_the_bound_stays_pending_and_a_refusal_shows`

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: owned suspended sessions keep Hosts visibility and mandatory whole-sandbox Delete.

ported: the card's explanation and "Suspended outside Farhelm, noticed at", the 30-minute sentence, Stop, Restart and
Replace unavailable while suspended, the row's Suspended badge, and "dev sandbox"; and, with skip on, the header's
Delete helped by "… whole sandbox and everything in it, after you confirm (…)", its question with the warning (every
session, sessions Farhelm does not list, the Resume hint, snapshots) and the sandbox's name, nothing sent before the
answer, and "Delete session and sandbox" sending one `?delete_sandbox=true` and no session-lane Delete. The rail leaving
the host out is the host rail's existing rule.

- crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs
  `a_suspended_sandboxs_delete_is_the_warned_whole_sandbox_delete`
- crates/gui/farhelm-gui/src/session_pane/environment_tests.rs `the_suspended_card_says_so_and_resumes`
- crates/gui/farhelm-gui/src/session_list/environment_tests.rs `the_suspended_detail_and_the_names_are_the_providers`
- crates/gui/farhelm-gui/src/session_pane/environment_tests.rs `the_header_suspends_and_resumes_beside_stop`
- crates/gui/farhelm-gui/src/session_list/environment_tests.rs
  `a_suspended_row_has_its_badge_and_a_stopped_container_never_does`
- crates/gui/farhelm-gui/src/session_list/environment_tests.rs
  `an_environment_rows_mark_and_host_name_are_its_providers`

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: a connected sandbox's Delete asks its Farhelm and reports the sandbox that stays; a sandbox destroyed elsewhere
retires its stays count but keeps the checkout notice; a row's unasked environment Delete offers the warned
whole-sandbox Delete; the header's unasked environment Delete reopens as the warned whole-sandbox Delete.

ported, in-process over the fake helm: with skip on and nothing alive, the menu's Delete described by the session lane's
help, its question (the both-outcomes consequence and the sandbox's name, the title, no checkout line, "Delete
session"), nothing sent before the answer, one `?delete_environment_session=true`, and the band's "The sandbox stays: 2
other sessions are in it"; the count retired once the hosts read no longer lists the sandbox, or lists it being deleted,
with the reply's checkout notice kept and a failed read changing nothing; a row's and the header's session-lane Delete
answered "unasked" asking the warned question again, whose answer sends one `?delete_sandbox=true`, and that Delete's
refusal shown on the row and below the header. The row leaving the list is the helm's pushed list, which the fake does
not push.

- crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs
  `a_connected_sandboxs_delete_asks_its_farhelm_and_says_what_stays`
- crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs
  `a_containers_count_retires_when_it_is_deleted_and_its_notice_stays`
- crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs
  `a_rows_unasked_delete_offers_the_whole_sandbox_delete`
- crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs
  `the_headers_unasked_delete_reopens_as_the_whole_sandbox_delete`
- crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs
  `the_band_keeps_a_count_until_its_environment_moves_on`

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: a session of a sandbox being deleted shows why its actions are unavailable.

partly (its row and header half): the row's "Deleting…" or "Deleting failed" in place of the stale cause with the
provider's reason as its help, and the same in the details bar in the age's place; every command of the row's menu and
the header refused, Clone's and Delete's descriptions and the header's Stop, Clone and Delete help giving the reason;
the Delete shortcut refused; nothing sent; and everything back once the host leaves Deleting. Bulk Stop and Delete leave
such a session out of their question and their batch with "Stop: " or "Delete: " and the reason on its row, as the old
batch did when it was drawn. Waiting for bulk environment Delete: the marked bar's Stop and Delete drawn unavailable,
with "The marked sessions' environments are being deleted, so their actions are unavailable", when every mark is such a
session.

- crates/gui/farhelm-gui/src/session_list/environment_tests.rs
  `a_row_being_deleted_shows_the_phase_in_place_of_its_cause`
- crates/gui/farhelm-gui/src/session_list/environment_tests.rs
  `every_menu_command_is_refused_while_the_environment_is_being_deleted`
- crates/gui/farhelm-gui/src/session_pane/environment_tests.rs
  `every_header_action_is_refused_while_the_environment_is_being_deleted`
- crates/gui/farhelm-gui/src/session_list/bulk_tests.rs `a_bulk_action_leaves_out_a_session_being_deleted_and_says_why`
- crates/gui/farhelm-gui/src/session_list/bulk_tests.rs `the_bulk_question_leaves_out_a_session_being_deleted`

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: bulk Delete of a connected sandbox's sessions sends one environment Delete each.

waiting for bulk environment Delete, the next environment piece; the single environment Delete it builds on is ported
(the cases above).

### `tests/tensorlake-sandbox-sessions.spec.ts` (launcher)

Case: a held sandbox Suspend and a launch on another host proceed side by side.

partly: the sidebar half, a pending Suspend holding no launch back, since Suspend takes no part in the launch and
registry exclusion (`Work::creates_a_session` and `changes_host_registry` are false): New stays available and opens the
launcher while it runs. Not ported: Suspend's refusal while this window's own launch targets the same host (a deviation
below).

- crates/gui/farhelm-gui/src/session_list/environment_tests.rs `a_suspend_in_flight_leaves_new_available`

### `tests/podman-environments.spec.ts` (settings and hosts)

Case: a connected container's Delete asks its Farhelm and reports the container that stays.

ported: the container's consequence ("the container goes too", no snapshots) and "container: owned-environment", one
`?delete_environment_session=true`, and "The container stays: 1 other session is in it" ahead of the reply's notice.

- crates/gui/farhelm-gui/src/session_list/environment_delete_tests.rs
  `a_containers_count_retires_when_it_is_deleted_and_its_notice_stays`

## Deviations from the old app

Where the port does not match the old app, and why. Each stays until the work named in it lands.

- The attention list's dismissal of optional update reminders lasts its UI session, where the old one also ended when
  the helm's build changed: the UI channel reports no helm build. In the desktop app the helm runs in the same process,
  so a helm on another build is a restarted helm, whose new UI session shows the reminders again anyway.
- An attention row's open control fills the row up to its button and draws the list rows' hover fill under the pointer;
  the old one had no fill. The row's open control is the one component the session rows use as well.
- While a session launch is in flight, the attention list's Update, Update all and Always update are drawn unavailable
  (still focusable) with "Wait for the pending launch to finish" as their hover help, and a press sends nothing; the old
  app queued such an Update until the launch was done. The port refuses it visibly instead, as the area's owner decided,
  since the Hosts panel's line that reports a refused Update is not drawn beside the list.
- The attention list closes the rows' menus and the marked menu when it moves them, and leaves the order menu open; no
  old case covers the order menu.
- The header's Stop, Replace and Delete ask the row's questions, through the same functions, so the two entry points
  cannot drift apart: the title quoted on a line of its own, and the row's answer labels ("Confirm stop", "Confirm
  replace", "Confirm delete") with Cancel first. The old header's questions quoted no title and labelled their answers
  "Stop", "Replace" and "Delete", with Cancel last. Restart, which only the header asks, keeps the old header question's
  words and quotes no title.
- A pending Delete shows the spinner the row and the scrim use beside its words in place of the header's actions; the
  old header showed a pulsing dot there.
- The version readout always shows this window's build, and its hover help never names the helm's: the GUI does not
  learn the helm's build yet. In the desktop app the helm runs in the same process, so the two cannot differ there; a
  remote helm would need it.
- A press and release on the divider that moved nothing writes nothing, where the old script saved the width on every
  release. Nothing changed, so the helm's stored width is the same either way.
- The old app marked the open session's row button `aria-current`, which GPUI cannot express. The open row's list item
  carries the selected state instead (AccessKit reports selection on item-like roles, not on a button), checked by
  `session_list/tests.rs` `the_open_sessions_list_item_says_it_is_selected`.
- The old app replaced the whole list with "Failed to load sessions: ..." when a read failed. The port keeps the rows it
  has (a failed read is no evidence that anything went) and shows that line above them, in the style of the old app's
  full-width failure lines (`.restart-error`), until the next listing the helm reads (`session_list/tests.rs`
  `a_failed_listing_keeps_its_rows_and_says_so_until_the_next_listing`). Since the rows stay, an open row menu stays
  too, and follows its button down past the line and back up; the old one closed with its row.
- An open row menu stays open and moves with its button when a pushed listing moves the row, where the old app closed it
  whenever its button moved (the old popover-movement specs; SPEC.md, "Things changing underneath", has the menu move
  with its session). A reorder, another row's removal and a host registry push are tested by
  `session_list/menu_tests.rs` `an_open_menu_follows_its_button_through_what_the_helm_pushes`, content growing above the
  rows by `session_list/delete_notice_tests.rs`
  `an_open_row_menu_stays_open_and_follows_its_button_when_the_band_appears`.
- The old page put every row in the document, so a screen reader could list all of them. The port draws only the rows in
  view and a few beyond (building every row on every redraw cost about 12% of a core at 200 sessions), so assistive
  technology and the test bridge see only those; the rest are reached by scrolling. This one is meant to stay.
- Tab from a row's control that a wheel scroll has moved out of view leaves the list (focus goes to the next painted
  stop, the terminal), where the old page scrolled the next row into view. The list keeps the row drawn so its control
  keeps focus. From its menu button Shift+Tab, Space and Enter bring the row back; Space or Enter on the row's open
  control opens the session without scrolling, and Tab's next stop is not in the painted order. Fixing it needs the
  focus coordinator to scroll before it steps (watch list).
- A click on a row's empty space (the gap beside its menu button, its border, a refusal line under it) keeps focus on
  whatever control holds it, where the old page sent focus to the body and so home to the terminal. The row's item
  tracks a focus handle so the row stays drawn while one of its controls holds focus, and the coordinator's press rule
  for such a container keeps focus in place. Harmless: it only matters when a control other than the terminal holds
  focus. A contain-only press kind would remove it (watch list).
- Not a deviation from the old app but a gap against SPEC.md that the port keeps from it: a bulk Delete of several
  sessions whose checkouts could not be archived shows only the last of their notices in the Delete notice band, each
  replacing the one before, where SPEC.md asks that each such outcome be visible. The single sandbox and container
  Delete kept the old one-notice band, so the bulk environment Delete decides whether one batch's notices are joined.
- A Replace refused after its source was already removed (the helm's "whether the original … was removed is unknown")
  has no row to show its words on, since they belong to the source; the selection moves on as for any removal. The old
  app showed them only in the header while the source stayed selected, which it does not here once the Replace settles.
- A plain Replace's YOLO question shows where the Replace was asked: in the source's row, under its lines, or below the
  open session's top row for the header's Replace (SPEC.md: "shown with the control or surface that started the
  launch"). A question asked from the header shows at the source's row instead while the source is not the open session,
  since the header is drawn only for the open session, so it is never shown nowhere. The old app drew every sidebar and
  header Replace question in one place, after the list's rows, headed "Replace <title>"; here the row or header it sits
  in names the session, and no heading is drawn. It is drawn on the window's ground, also on the selected row's accent
  fill.
- The old row menu's Replace, Stop, Mark read, Mark unread, Pin to top, Unpin, Move up and Move down had no tooltip,
  since SPEC.md exempts an item that shows a description line. The component layer has no form of hover help that stands
  for a visible description, so they carry one, as Rename and Delete already did. So do Clone and Replace with…, with
  the old header Clone's tooltip and the old interrupted card's Replace with tooltip.
- The header's caret menus open below their caret, as every popover of the layer manager does, where the old ones sat
  against the caret's right edge. Tab in one dismisses it as Escape does, so a second Tab at once goes on from the
  caret; a plain close would have swallowed it as a repeat of the closing press.
- The old order hold was also kept while a row's notification list was open (`popover-movement.spec.ts`, "a held served
  refusal reorder preserves an open bell"); here only a row's menu or a question one of its commands asked keeps it, as
  SPEC.md names. The pointer on an open notification list is not over the rows, so the order can catch up under it, and
  that is harmless: the list is anchored to its row's bell and moves with it, so a reorder cannot put a different
  session under the pointer. This one is meant to stay.
- A pinned row's title truncates beside its pin glyph, and the glyph's 16-pixel line box is held to its own size, so a
  pinned row is no taller than an unpinned one. In the old noncompact row the title could wrap under the glyph when the
  sidebar was narrow, and the glyph's taller line could add a few pixels to the row.
- A pin's grip takes focus on its press, as the old one did, through the focus coordinator with a handle that is no Tab
  stop, so the app's shortcuts still act during a drag and Tab moves focus on, as in the old app. A press made while a
  row's menu is open cannot focus the grip (the coordinator refuses a move out of an open layer), so focus goes back to
  the menu's button. Escape ends a drag wherever focus is in the sidebar's body (the host rail, the list and the rail's
  foot) and goes no further, as the old document-level listener had it; with a dialog open (Rename, opened by its
  shortcut mid-drag) Escape closes the dialog instead, by the layer manager's rule for Escape (SPEC_impl.md, "Overlays:
  the layer manager"), where the old listener ended the drag and left the dialog open, and with focus on the sidebar's
  width divider or the session pane's controls (only Tab reaches them mid-drag) or in a terminal it does not reach the
  drag. After a cancel focus stays on the grip. After a drop the grip is not drawn while the move is in flight, so the
  coordinator sends the lost focus home, where the old grip, disabled meanwhile, lost it to the page. A drag also ends
  when its window loses keyboard activation, where the old one ended when its pointer capture was lost. A row that
  cannot move just now (a request of its session in flight, a question of its row open, the listing unavailable) draws
  no grip, where the old one drew a hidden disabled button that kept the press; a press in that gutter opens the row. A
  retired containerized session's row has no grip, as the old one's was disabled, and stays a drop target, as it was;
  the old grip was also disabled while navigation was locked, which does not exist here. A drag also ends when its row
  is scrolled so far that the virtualized list no longer draws it, which the old list, drawing every row, never did. The
  dot field's fourth row, a one-pixel sliver of the old gradient, is not drawn.
- The old status dot was hidden from assistive technology (`aria-hidden`), its toggle reachable only through the row
  menu's item. The port's dot is a control with a role and a name, the action and the session's title ("Mark read:
  <title>"), because every component takes both; like the old dot it never takes focus, so the keyboard path is still
  the menu item.
- The old Replace progress line was hidden from assistive technology, with a visually hidden copy of its label announced
  once. The port's line is one status element named by that label (the seconds are not part of the name); whether a
  screen reader announces it once or on each second's redraw is not checked yet.
- While the window was not focused, the old app paused the running pulse (`animation-play-state: paused`), so a dot
  could stay frozen mid-dip at a level nobody chose. The port draws it at full opacity there instead. That is still,
  which is all SPEC.md asks ("stand still while the Farhelm window is not focused"), and it is the one still state that
  does not depend on timing: an exact freeze would need the ticker's shared count to stop for every view, which it
  cannot promise while anything else animates. This one is meant to stay; no later work removes it.
- Picking the session that is already open in the quick switcher focuses its terminal. The old switcher returned focus
  to whatever held it when the switcher opened; SPEC.md's focus rulebook ("Choosing a session, including the one already
  selected, focuses its terminal") takes precedence. In the old tests the opener was that terminal, so both read the
  same there. This one is meant to stay.
- The old quick switcher closed on a primary click's release on its backdrop; the port closes on a press of any button
  outside the card, as the layer manager's popovers do. The press is still taken by the veil, so nothing under it acts.
- The quick switcher's rows mark a session whose host the registry does not list (a removed host) with the default
  cloud, as the sidebar's row does. The old switcher drew a mark only for a host the registry listed, while the old row
  already drew the cloud for any host it could tell was not the local one.
- The old row's age and stale cause were plain text inside its open button. The port draws each as a status mark of its
  own, since GPUI names the open control by its title alone: the age is named by its whole timestamp (`Last activity …`)
  rather than read out as `2m`, and the cause by its words, with its hover help as its description.
- The old row's host, agent and permission marks were decorative pictures with hidden words inside its open button. The
  port draws them as status marks of their own, for the same reason as the age: the host mark is an image named `Local`
  or `Remote` with its locality's explanation as hover help, and the agent track is one image named by the pair's whole
  description, the old hidden text exactly, so a screen reader hears it once per row as before. The quick switcher's
  rows draw the same host mark, where the old switcher's had a tooltip but nothing for assistive technology.
- The age column is at least four characters of the age's own 12-pixel monospace face wide (28.8 pixels), where the old
  grid's `minmax(4ch, max-content)` counted four characters of the row's interface font. Every age under a thousand days
  still fits inside it, so the ages and the titles' ends line up as before.
- The quick switcher's agent column follows the row menu header's rule (the agent kind's name, else the command's
  program), as the migration's orchestrator decided for the port. The old one named the stored launch's harness first,
  and said `Command` for a command with no known agent; whether the two ever name a session differently has not been
  checked case by case.
- The quick switcher's search field is GPUI Kit's input at the app's input size and padding, not the old field's
  inherited 16 pixels and `10px 12px` padding.
- A pressed rail entry keeps its accent fill under the pointer and while it shows the keyboard ring. In the old sheet
  the hover fill outranked it (`--bg-2`, white in the light appearance), which left All's white word on a white fill.
  This one is meant to stay.
- The rail's Add host is never disabled, where the old one was while a launch or a change to the host registry was in
  flight (the old page's one token for both). Under the decision of 2026-11-14 each action refuses for itself, so the
  refusal belongs to the dialog's Add: pressed while a session launch, Restart or Replace is in flight, it sends nothing
  and says why in its error line, the fields kept (`hosts/add/tests.rs`
  `add_refuses_while_a_session_launch_restart_or_replace_is_in_flight`; the settings-and-hosts parity file's Add host
  deviations).
- The rail's expansion is the helm's `host_rail_open` preference, shared by every client (the maintainer decided on
  2026-11-13), where the old app kept it per device in `localStorage`; nothing carries over. It is written the way
  Compact is: a refused write is logged and shows nothing, the rail going back to what the helm stores. The old toggle
  always widened at once, since its write could not fail.
- An avatar's menu opens on a secondary click (a right press, or a Ctrl+click on macOS) and on Shift+F10, but not on the
  keyboard's ContextMenu key, which GPUI names on neither platform.
- The old count line's unsupported-filter wording (" — this helm does not support filtering, so the filter was ignored")
  is not ported, though SPEC.md still names the warning: it answered an old helm that ignored a filter, and the UI
  channel's helm applies its query or refuses it, so its listings carry nothing that could call for it (the `filters`
  module's NOTE).
- A refused write of the list's order is logged and shows nothing, and the list and its title stay in the stored order.
  SPEC.md, "Errors and diagnostics", counts the order among the best-effort preferences: a failed save is logged but
  stays silent, and the choice stays in effect only until the next read from the helm. The old page re-sorted itself and
  kept the order until its next read; under the channel the next read is immediate, since the helm pushes the list in
  the stored order before it answers, so the choice lasts no visible time (decided by the migration's orchestrator on
  2026-11-14). Keeping it in effect would take a session query that names the sort, which stops the channel following
  the shared order. Tested by `session_list/order/tests.rs` `a_refused_write_keeps_the_stored_order_and_shows_nothing`.
- Two rules stand side by side for a choice made while another preference write is in flight. The list's order, Compact
  and the rail's Expand and Collapse queue it: it is shown at once and sent when that write settles, the newest choice
  per field winning, and fields queued together go out one after the other. Settings refuses a press in the same
  situation with a visible line, "Not saved: another operation was in progress. Try again." (accepted by the migration's
  orchestrator on 2026-11-14; [Settings and hosts](settings-and-hosts.md) records it). The old app queued every field,
  Settings' included, and wrote different fields at once. The migration's orchestrator decided on 2026-11-16 to keep
  both rules, to be revisited if a user meets either. Tested by `session_list/order/tests.rs`
  `an_order_chosen_while_compact_is_saving_is_shown_at_once_and_sent_after_it`,
  `two_fields_queued_behind_a_third_write_go_out_one_after_the_other` and
  `a_choice_queued_behind_a_refused_write_is_still_sent`.
- The order menu opens below the list's title, where the old one opened beside the sidebar with a pointer; the layer
  manager places a popover below its anchor unless the anchor asks to stand beside it, as a row's menu's does. The title
  cannot report `aria-haspopup`, which GPUI has no way to express. Tab stays inside the open menu, as SPEC.md's
  "Keyboard focus" keeps focus inside every menu ("Inside a dialog or menu, focus stays inside"), where the old Tab on
  an item closed the menu; SPEC names Tab closing a menu only for the session view's Restart and Replace caret menus.
  Home and End are the menu's own keys, as the layer manager's menu context binds only Up and Down.
- A refused write of Compact is logged and shows nothing, and the rows and the item's check go back to the stored
  density, as for the order above and for the same reason: SPEC.md, "Errors and diagnostics", names the "compact layout"
  among the best-effort preferences whose choice "stays in effect only until the next read from the helm", and under the
  channel that read is the store the window already holds. The old client kept its optimistic choice until its next
  seed. Tested by `session_list/order/tests.rs` `a_refused_compact_write_keeps_the_stored_density_and_shows_nothing`.
- A compact row shows the same summary card as a noncompact one, with its directory in full, as the old app did. Its
  accessible description (the card's words, which the layer manager also reports as the tooltip's text) says what its
  hidden lines would have: the complete ended wording or the live word, a stale row's cause and the full directory, then
  "open this session" (`Exited (code 17) · Waiting for supervisor · /home/user/src/b — open this session`); a noncompact
  row's says its status word and "open this session". The old compact row put those words in hidden text inside its
  button, which joined its accessible name; here the name stays the title. Tested by `session_list/compact_tests.rs`
  `a_compact_rows_help_says_what_its_hidden_lines_would` and `session_list/summary_tests.rs`
  `a_compact_row_shows_the_same_card_with_its_hidden_folder`.
- Every row hover the old app placed beside the list is placed there, the notification bell's included
  (`session_list/summary_tests.rs` `every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring`), the pin grip's
  included; the managed-checkout mark is not ported, so it has no help to place.
- The summary card's agent line names a command launch's program by the first word of its command line, as the menu
  header does; the old card split the line as a shell would, so a quoted program name holding a space read whole.
- A row's menu and the questions its commands ask open beside its button, the menu's left edge 10 pixels past the
  button's right edge and its top 4 pixels above the button's, as the old flyout did, but through the layer manager's
  one beside placement, so they keep its clamp rather than the old flyout's: flush inside the window rather than 8
  pixels from its edges, moved up only as far as the menu's own height needs (the old one kept 528 pixels of room below
  the menu's top, so a short menu on a low row rose further), and never capped in height with a scrollbar (the full row
  menu fits in the minimum window). The old flyout's pointer notch toward its row is not drawn: the layer manager draws
  no notch for a popover; it draws one only for hover help beside the sidebar. Tested by `session_list/menu_tests.rs`
  `the_menu_opens_beside_its_button_holds_focus_and_leaves_the_selection` (the exact offset) and
  `delete_then_enter_asks_about_a_live_session_and_enter_deletes_it` (the question beside the row); the layer manager's
  own tests pin the placement and its clamp for any anchor.
- A row's menu button appears and goes at once, where the old one faded over 100 ms: a fade per row would need a ticker
  animation for each row in motion, which nothing else in the list holds. Touch screens get no always-shown button (the
  old `@media (hover: none), (any-pointer: coarse)` rule), since GPUI reports no coarse pointer to ask about. Its other
  reveal rules are the old ones, tested by `session_list/row_tests.rs`
  `the_menu_button_shows_for_hover_focus_its_open_menu_and_selection_and_none_latches`.
- A row whose menu is open draws no tint of its own; only its menu button takes the pressed look (`Tier::RowMenu`'s
  expanded look). The old row tinted itself as well (`tests/sidebar.spec.ts`, "an open menu tints its own row and
  presses its own toggle"). SPEC.md names a row tint only for marked rows and asks for none here; nothing in the port
  draws one yet.
- A managed checkout's row draws its last line as an ordinary folder's, without the old `·` separator and repository
  mark; no part of the port draws that form yet, and the details bar's checkout mark still says which sessions run in a
  managed checkout.
- The heading's actions, Trash, Templates and New, are all drawn. The old Templates button was disabled while a launch
  or a change was in flight (the old page's one token for both); this one opens the dialog whenever it is pressed, as
  the rail's Add host does under the decision of 2026-11-14 recorded above (each action refuses for itself). New is
  still disabled while New is unavailable (a launch pending or a host change in flight), as the old one was, since it is
  the launcher's own refusal. Tested by `session_list/order/tests.rs`
  `new_opens_the_launcher_and_closing_returns_focus_to_the_button`, `new_is_disabled_while_a_host_change_is_in_flight`
  and `the_new_action_leaves_the_hosts_panel_only_when_new_opens`.
- Marks belong to the UI session: a reset of the channel to the helm forgets them, as the old app lost them on a reload.
  They are never saved.
- Closing the Trash dialog while sessions are marked returns focus where the dialog's own rule sends it when its button
  is not drawn (what held focus when it opened, else home), where the old app focused the marked bar's ⋯ toggle. The
  Trash dialog names its button as where closing returns focus, and the button is not drawn while the bar is.
- A marked row a bulk action skipped because its own commands were refused says "Stop: not started, another operation
  was in progress. Try again." (the row's ordinary name for the work, then the operation store's shared reason), where
  the old app said "Stop: not started, another operation is in progress; try again". The name is the row's own line's:
  "Stop", "Delete" and "Pin" as before, but Unpin's line also says "Pin" and Mark read's and Mark unread's say "Seen",
  where the old app named the menu item. The line is the row's one line (the operation store's slot), so it goes when
  the session's next operation starts, and a refusal the busy request settles with replaces it, as the old error slot
  did. A marked row with a request in flight is skipped with that line too, where the old bulk metadata sent its write
  anyway and skipped only a row whose question or Rename was open: the port's row menu refuses Pin and the seen toggle
  while a request is in flight, and the bulk action follows it.
- The bulk question's rows show the status in words in the body's ink, where the old ones drew the colored status badge.
- When a bulk question closes because every target is proven gone, focus goes to the ⋯ while the frame still draws it,
  and once the ⋯ is gone it recovers to the open session's terminal, whatever held focus when the question opened; the
  old app focused the order's title.
- While the bar draws no ⋯ (every mark shown a retired session, which no metadata action takes), closing the bulk
  question returns focus to the bar's Delete (the only lifecycle control it draws then), where the old app named the
  order's title, which is not drawn while the bar is, so it focused nothing.
- The bulk question has no header Close (the prompt's centered question draws none), where the old one had a "×" named
  "Cancel bulk stop" or "Cancel bulk delete". Cancel and Escape do what it did.
- Escape clears the marks from anywhere in the list's column, where the old app cleared them only from a row, the status
  segments and the marked bar, and not from a row whose own question was open. So Escape on the order title, Trash,
  Templates or New (drawn while every mark is hidden) clears the hidden marks, and so does Escape on a row's control
  while that row's inline YOLO question is open below it (inside the question, the question takes Escape). SPEC.md's
  "Escape clears marks only from the session list" allows the wider scope.
- Over a listing the helm could not read, the port keeps the rows (above) but shows no marks and takes no marking click
  on them: SPEC.md has a failed listing expose no targets. The old app drew no rows then, so nothing could be marked
  either. The marks come back with the next listing it reads.
- Closing the launcher returns focus to what held focus when it opened, where the old app's Cancel always focused New,
  however the launcher had opened. This follows SPEC.md, "Opening and closing": "Closing something the user opened
  returns focus to where it was before it opened, on every platform, including those where clicking the button that
  opened it did not move focus to the button", which takes precedence over the old behavior ("Keyboard focus"). The two
  differ for a click on the heading's New on macOS, where a click focuses no button (focus goes back to whatever held
  it, usually the terminal); for Cmd+N (back to where focus was when it was pressed); and for the quick switcher's New
  row on every platform (back to where focus was when the switcher opened). A click on New on Linux focuses New, so both
  apps land on it there.
- With no session open and no sessions at all, focus rests on New (SPEC.md, "Where focus rests"), on the same proof the
  pane's "No sessions — create one" needs: an available, complete, unfiltered listing with no rows. Before the first
  listing, or over a failed or cut one, New is not home. While New is not drawn as a tab stop (disabled while a launch
  or a host change is in flight, or hidden with the list's column while the Hosts panel shows), focus rests on the
  window's root, where Tab and the other keys still work, and goes to New on the first frame that draws it, unless a
  control has taken focus meanwhile. Tested by `session_pane/tests.rs` `with_no_sessions_focus_rests_on_new`,
  `removing_the_last_session_sends_focus_to_new`, `a_listing_that_comes_back_empty_sends_focus_to_new`,
  `while_new_is_disabled_focus_rests_on_the_root_and_returns_to_new` and
  `a_first_launch_over_an_empty_fleet_ends_on_the_new_sessions_terminal`.
- The quick switcher's New row draws its top edge as a straight one-pixel line across the row, since GPUI gives an
  element one edge color for every side; the old top border followed the row's rounded corners.
- Every refused session query, a host's, a status's or the quick switcher's reset of both, is worded "Could not show
  those sessions: " and the helm's words, under the heading, with the previous choice put back (one lead, decided by the
  migration's orchestrator on 2026-11-14). The old app had no such line: a refused read failed into the list's "Failed
  to load sessions: ".
- The details bar's copy fields give up their width before the title and the host, as SPEC.md says ("ellipsize before
  the session name or host when the bar runs out of room"): they sit in one box that takes only the room the bar's other
  parts leave. The old bar's `flex: 0 3 auto` only shrank them faster, and since shrinking is weighted by width, a long
  command cut the old title too.
- The hidden-characters warning above the details bar is held inside the bar's width, 10 pixels clear of either edge,
  where the old one was capped at the smaller of 60 characters and 80% of the window. The bar is never wider than the
  window, so the warning always stays inside it and wraps where the old one could run past the pane.
- The command line's copy field shows `(empty)` or `(whitespace only: N characters)` for a value that would show
  nothing, as the old directory field did; the old command field showed nothing at all.
- Two of `tests/host-rail-warning.spec.ts`'s cases are not ported. An initial Add whose acceptance was malformed or lost
  has no warning on an avatar: a new host's setup refused before the helm registered it has no host to warn and shows
  its existing Add notice. A held Update losing its target (a destination or installation change) has no warning of its
  own: an Update left uncertain around such a change warns only through its own refusal, its run's failure or its run's
  withdrawal. `crate::hosts::status`'s module docs ("Not ported") give the reasons.
- The attention list's rows are reached by Tab, one control at a time. The old list's scroll area was a keyboard stop of
  its own (`tabindex="0"`) that scrolled with the arrow keys.
- An environment session's Stop question draws Stop and suspend between Cancel and Confirm stop, where the old one put
  it last: the prompt draws every extra answer between the two (`prompt.rs`). Its words and request are the old ones.
- An environment session's refused Suspend or Resume reads "Sandbox: " before the helm's words for a container too, as
  the old list's error slot wrote it for either provider, and so does a refused Resume host of an ordinary host on its
  session's row and menu (the card itself shows the bare words); a refused Stop and suspend reads "Stop: ".
- The helm answers Suspend, Resume and Stop and suspend within 20 seconds, answering work still running then with a 504
  that says it carries on (decided by the migration's orchestrator, 2026-11-15). The port does not show that 504 as a
  refusal: the row and header go on saying "Suspending…" or "Resuming…" from the host's pushed phase, and the commands
  of every session on that host, Delete and Clone included, stay refused while that phase shows the change running, as
  the old app's session claim lasted for the whole change. The old app waited for the provider itself. While the request
  is in flight only the clicked session is held, as the old per-session claim held only its own. A container's pause and
  unpause push the same changing phase as a sandbox's, so they show and hold the same way; but a container host checks
  its container over SSH before that phase is pushed, and when the check alone outlasts the bound the session looks idle
  from the 504 until the push (the helm's lifecycle lock refuses a second request meanwhile). The header's control keeps
  its label and width while pending, with the spinner in its glyph's slot and "Suspending…" or "Resuming…" as its
  accessible name and help, where the old button read the words.
- The unavailable card's "Resume host" has hover help, "Resume host: this session continues when its host resumes",
  where the old button had none; every GPUI control has help. A refused Resume shows on the card, and the lines above
  the terminal leave it out while the card shows it.
- While a Suspend or Resume waits for its provider, the unavailable card's evidence line says "Asked the environment
  provider to suspend at …" (or resume), the Hosts row's shared phase words (`hosts::identity::phase`), where the old
  card named who was asked: "Asked Tensorlake …" for a sandbox, "Asked the container host …" for a container. The
  per-provider words wait for an answer for them in the provider wording module (`environment_ui.rs`), which the Hosts
  details are to share; the card takes it up then, with a test of the exact words for a sandbox and a container.
- The interrupted card's Restart and Replace are the component layer's ordinary buttons, with 6 pixels of padding, where
  the old card's had 4. SPEC.md names no size for them; the component layer's condensed look, which gives up padding, is
  used only by the header, under a tunnel pill's pressure, which the card never meets.
- A session whose first attach the helm refuses or detaches (the old `terminal.spec.ts` case's "no such session on this
  host entry") shows the terminal's red band with Reconnect over an empty terminal, where the old app showed the calm
  "Session unavailable — reconnecting" card and no banner. The terminal keeps that card for a drop its stream recovers
  from by itself; a refusal or a detach waits for the user, and SPEC.md keeps "explicit banners" for such decisions
  (`terminal/attached/tests.rs` `detached_and_refused_offer_reconnect` and
  `detached_refused_and_closed_streams_keep_the_red_band`).
- Not yet ported, waiting for the launcher's environment work: Suspend and Resume refused while this window's own launch
  targets or is waking the same host (the old "A launch on this host is in progress; try again once it finishes"), and
  Clone's environment wording ("New session in a new environment, keep this one"), which would describe a Clone the
  launcher does not make yet. The helm's lifecycle lock refuses a collision meanwhile.
- While a session's environment is being deleted, its row menu's Rename, Mark read or Mark unread and pin items are
  unavailable too, where the old menu kept them while nothing else ran: every command refuses through the one refusal
  place (`session_list::menu::refused`), as it already did while a Suspend or Resume ran, which SPEC.md's "its actions
  unavailable" also says, and bulk Pin and Mark read leave such a session out with the reason. The Rename shortcut and
  the status dot do not consult that place, as they do not while a Suspend or Resume runs either, so they still act. A
  compact row keeps its stale clock and cause ("Host being deleted") rather than drawing the phase, as the old compact
  row had no being-deleted mark.
- The helm answers an environment session's Delete within 20 seconds and carries on past that, where the old app waited
  up to nine minutes (decided by the migration's orchestrator on 2026-11-16, and SPEC.md says so). The port shows the
  bound's 504 as no failure: the row and header drop their Delete progress and the pushed lists show how it ended. So a
  Delete of one session still unanswered then does not open the warned whole-sandbox question; the next Delete does,
  once the host reads not connected, and while it stays connected, Hosts' Delete sandbox is the way out. A success after
  the bound shows no "The sandbox stays" count, since its reply never reaches the window. A whole-sandbox Delete that
  fails after the bound leaves the sandbox listed in its ordinary state with the reason only in the helm's log, where
  the old app showed the reason on the row. Tested by `session_list/environment_delete_tests.rs`
  `a_504_at_the_bound_is_not_a_failure_but_a_refusal_shows`.
- An environment session's "unasked" answer asks the warned whole-sandbox question again only while the user has not
  moved on since answering the first, as every completion that would open a question does here. Otherwise the helm's
  words stay on the row, and the next Delete picks its lane from the hosts read again; the old question waited on the
  row's closed menu and opened with it. A header Delete's second question opens centered, since its Delete button is not
  drawn while the first Delete runs, where the old one hung under the button. Tested by
  `session_list/environment_delete_tests.rs` `an_unasked_answer_after_the_user_moved_on_stays_on_the_row`.
