# Parity: Terminal

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Terminal experience, Attachments, Session view, Keyboard focus.

### `grid-client/grid-attach.spec.ts`

Tests: 36. Covers: a terminal mounts the grid directly and keeps the catch-up, input and resize.

partly: the remote-localhost tunnel offer's hover hint and Cmd-click (its tunnel cases) by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_loopback_address_in_a_remote_session_offers_a_tunnel_on_a_mac`,
`a_cmd_click_asks_the_warning_then_opens_the_tunnel_through_the_helm`,
`a_hinted_address_whose_display_is_not_fully_visible_is_refused` and the other tunnel tests there, and the loopback
table by crates/gui/farhelm-grid-model/tests/links.rs
`tunnel_ports_are_offered_only_for_normalized_loopback_and_wildcard_hosts`. Driven end to end on the test Mac against a
real remote host (the hint under the real pointer, a Cmd-click, the warning, Allow, and the remote page served through
the tunnel on the Mac's 127.0.0.1), by a scripted run outside the smoke suite on 2026-11-14. Links by
`a_hyperlink_relabelled_under_the_pointer_is_judged_again`,
`a_hyperlink_relabelled_between_press_and_release_does_not_open` (a relabel of the same cells and target counts as
another link), `a_two_row_lookalike_hyperlink_warns_on_both_rows_and_still_opens`,
`hostile_hyperlink_text_and_targets_stay_data` (the old case's markup label has no counterpart: the desktop app draws no
HTML) and `a_printed_address_extended_on_the_next_row_opens_its_new_address`. The usable snapshot (its selection half),
now a stalled viewer's, since a displaced window shows no terminal (decided by the migration's orchestrator on
2026-11-15), by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_stalled_snapshot_still_selects_and_copies_and_sends_nothing`. A detach during the first replay (now a stall, for the
same reason), with no earlier screen, keeps what arrived of it as the snapshot by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_stall_during_the_first_replay_keeps_what_arrived` (it was
missing: the pane stayed blank). Its "a grid frame that breaks the format ends the attachment with a visible error"
case, whose harness had no reconnect policy, now with the old app's policy (the fault is a transport loss: a fresh
attach and a full replay, and the red band only once recovery stops): the faulted stream is closed and a new one
attaches at the fitted size after the ladder's first rung, its replay replacing the kept screen only once complete,
under the reconnect card with no red band and none of the fault's words, by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_protocol_fault_restarts_the_stream_and_its_replay_replaces_the_screen`; a fault on every replay waiting 0.5, 1, 2, 4,
8 and 15 s and the seventh showing "Terminal stream error: " with the model's words and Reconnect, with nothing
restarting after it, by `repeated_protocol_faults_back_off_and_end_in_the_red_band` (a recorded deviation from the old
app, which probed every 30 s forever: a peer that breaks the format will not be fixed by a full replay every 30 s, which
would also spend idle CPU for nothing; decided by the migration's orchestrator on 2026-11-16); Reconnect from there
attaching at once and counting from the first rung by `reconnect_after_the_stream_error_attaches_a_fresh_stream`;
Reconnect now skipping a restart's wait by `reconnect_now_skips_a_restarts_wait`; the count starting over only once a
restart has stayed live 15 s by `the_fault_count_starts_over_only_once_a_restart_has_held`; edge cases by
`a_fault_after_a_restart_went_live_keeps_climbing_past_the_hold`, `the_rest_of_a_faulted_streams_batch_is_not_applied`,
`a_resize_during_a_restarts_wait_waits_for_the_restart`, `a_resize_under_the_stream_error_opens_nothing`,
`a_restarts_first_screen_at_another_size_is_resized_once`, `a_reset_during_a_restarts_wait_opens_nothing` and
`a_reset_under_the_stream_error_leaves_reconnect_inert`. The fault's text goes to the log while recovering, which no
test reads, and the old case's bad foreground color stands in as a second hello, another message the model refuses; a
replay cut off by a dropped link instead reconnects and replays, which is the recovery work's. Its file-link cases by
crates/gui/farhelm-gui/src/terminal/files/tests.rs: "a printed path hovers with its host check and downloads on click"
by `a_click_saves_into_downloads_and_the_line_says_where`; "a redraw that changes the hovered path withdraws its click;
the same text redrawn keeps it" by `a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes`;
"in the desktop app, the platform's modifier-click opens an allowed file after saving it" by
`the_modifier_click_opens_only_what_the_helm_offered`; "a relabelled OSC 8 file link is checked afresh and downloads" by
`a_relabel_withdraws_the_click_until_checked_again`. Dropped, by SPEC.md's own decision: a change to only an adjacent
row of a hovered link judged again (the warning "need not follow later redraws of other rows while the pointer rests",
"Terminal experience"). Superseded: a pointer move between a relabel and its paint (the browser's frame timing; the GPUI
view looks the hover up again as the output applies). Its attachment cases by
crates/gui/farhelm-gui/src/terminal/attachments/tests.rs: the paste through the attachment rules (a copied file winning
over the text the clipboard also carries, and its host path typed as a paste) by
`a_pasted_file_is_uploaded_and_its_host_path_typed`, and the drop that uploads the file and types its path by
`two_dropped_files_are_typed_as_two_words_in_order` (a native drop navigates nowhere, so that half needs no test).

- "a terminal mounts the grid directly and keeps the catch-up, input and resize" by
  crates/gui/farhelm-gui/src/terminal/attached/tests.rs `the_stream_opens_at_the_fitted_size` (measured before the
  attach, opened at the fitted size, behind the catch-up placeholder), `a_replay_is_shown_only_once_complete`,
  `program_keys_are_sent_to_the_live_attachment` and `a_text_size_step_resizes_every_open_terminal` (one resize per
  step); its page-wide record and hooks are superseded (the desktop app has no page globals).
- "a program's clipboard write reaches the system clipboard once whole" by
  `a_programs_clipboard_write_reaches_the_clipboard` and crates/gui/farhelm-grid-model/tests/model.rs
  `clipboard_parts_are_atomic_and_primary_is_ignored` (delivered only with its last part).
- "page shortcuts pressed in a grid terminal act and never reach the program" by
  crates/gui/farhelm-gui/src/actions_tests.rs
  `every_physical_shortcut_runs_from_a_focused_terminal_and_never_reaches_it` and
  `app_shortcuts_bound_in_farhelms_context_still_work_from_a_terminal`, and
  crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_held_app_shortcut_never_reaches_the_program`.
- "a socket closing during a grid terminal's catch-up reveals what arrived, under the banner": like the format fault
  above, its harness had no reconnect policy; a close is now a dropped link that reconnects and replays, and a replay
  cut off is not shown, by `a_stall_during_the_first_replay_keeps_what_arrived` and
  `a_replay_is_shown_only_once_complete`; a closed stream's "Connection closed" band by
  `detached_refused_and_closed_streams_keep_the_red_band`.
- "a completed selection reaches the system clipboard through the page's queue" by
  `a_drag_selects_and_copies_and_a_click_does_not` (the page's write queue is the browser's and has no counterpart).
- "a program's primary-selection write suppresses grid drag guidance without copying it" by
  `a_program_clipboard_write_in_the_grace_window_suppresses_the_notice` (a primary-selection write in the grace window)
  and model.rs `clipboard_parts_are_atomic_and_primary_is_ignored` (never copied).
- "a withheld grid link drag dismisses guidance and never raises it again" by
  `a_withheld_press_on_a_web_link_never_raises_the_notice` and `the_next_press_in_the_terminal_puts_the_notice_away`
  (any press puts the notice away).
- "links in a grid terminal open under today's rules" by `a_printed_web_address_hovers_and_opens_on_a_click`,
  `a_program_hyperlink_shows_its_destination_and_opens_on_a_click`, `the_link_choice_decides_who_gets_a_click_on_a_link`
  (the program choice opens nothing) and `hostile_hyperlink_text_and_targets_stay_data` (a `javascript:` target opens
  nothing).
- "under mouse reporting a press on a web link is withheld from the program by default" by
  `the_link_choice_decides_who_gets_a_click_on_a_link` (intercept, the default).
- The recovery half of "a dropped connection keeps the grid's last screen while it recovers, and a takeover keeps it as
  a usable snapshot" by `a_dropped_stream_shows_the_reconnect_card_over_the_kept_screen`; its takeover half is the
  stalled viewer's snapshot above.
- "a recovered grid terminal replays into a new grid and drops the held screen" by
  `a_replay_is_shown_only_once_complete`, `the_card_stays_through_the_reconnects_catch_up_and_goes_when_it_is_live` and
  `program_keys_are_sent_to_the_live_attachment` (typing reaches the new attachment).
- "restarted anonymous links hover as a whole address while a different target still warns" by
  crates/gui/farhelm-grid-model/tests/links.rs `osc_precedence_and_generated_continuation_identity` (id-less runs with
  one target join as one link) and `pinned_cell_links_and_label_join_oracle` (the warning), in the model; no view test
  draws it.
- "a terminal's unmount removes every page-level listener its mount installed": superseded, as there are no DOM
  listeners to leave behind (the same reason as terminal-attachments' "exactly one set of hooks").
- "a grid attachment detached as stalled shows why and does not reconnect" by `detached_and_refused_offer_reconnect` and
  `detached_refused_and_closed_streams_keep_the_red_band` (Detached with its reason, waiting for Reconnect), and
  `a_stalled_detach_attaches_again_only_when_the_user_asks` (no second attach opens until Reconnect is pressed).
- "a host's file name with format characters is shown with them visible" by
  crates/gui/farhelm-gui/src/terminal/files/tests.rs
  `a_hosts_file_name_with_format_characters_is_shown_with_them_visible`.
- "a container session offers only the paths its host admits": superseded. The GPUI app checks every hovered path and
  shows the host's refusal ("Not found", in red) by files/tests.rs
  `folders_unreadable_and_other_refusals_say_why_and_admit_nothing`, as SPEC.md's file-path rule says and as a container
  session's file links follow "the ordinary supervisor rules inside their container" (SPEC.md, "Session view"). The
  hover that hid a refusal belonged to the retired legacy containerized sessions, whose supervisor sat outside the
  container; their check before a path was offered was removed with them in #3330, before the reference commit, so at
  `24bc0580` the old app no longer read the `containerized: true` the spec still passed and showed "Not found" too.
- "a container file hover survives unrelated neighboring output (single row)" by files/tests.rs
  `a_single_row_paths_hover_survives_output_past_a_blank_row` and
  `a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes` (a rewrite of the row below starts
  no check); its cut variant by `a_cut_paths_hover_survives_output_past_a_blank_row`.
- "a new neighboring continuation withdraws a container path's old hover" by
  crates/gui/farhelm-grid-model/tests/paths.rs `a_printed_path_extended_on_its_next_row_is_a_different_link` and
  files/tests.rs `ready_for_requires_the_same_cells_file_and_label`.
- "a click whose file hover is covered is refused with an explanation" and "a web link or tunnel click whose target
  display is covered is refused with an explanation": `a_real_approval_card_covering_the_link_panel_refuses_the_click`,
  `a_covered_tunnel_hint_refuses_the_modified_click` and files/tests.rs
  `a_covered_file_panel_refuses_the_transfer_even_if_the_press_dismisses_the_cover`. A display cut off by the window's
  content area is also refused (`a_click_whose_destination_is_not_fully_visible_is_refused`,
  `a_hinted_address_whose_display_is_not_fully_visible_is_refused`, files/tests.rs
  `a_click_whose_file_details_are_not_fully_visible_is_refused`).

### `grid-client/grid-copy.spec.ts`

Tests: 1. Covers: a copy across a wide character's wrap leaves out the empty edge cell, on the screen and from history.

replaced: crates/gui/farhelm-grid-model/tests/model.rs `selection_uses_cells_original_widths_and_farhelm_trimming` (the
wide wrap's empty edge cell, for history rows; the screen and a history-to-screen crossing use the same line-numbered
rule but have no case of their own yet) and crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_drag_selects_and_copies_and_a_click_does_not` (the copy path)

### `grid-client/grid-fit.spec.ts`

Tests: 3. Covers: sizing after real pane changes.

replaced: crates/gui/farhelm-gui/src/terminal/attached/tests.rs `the_fit_is_the_old_apps`, `a_new_size_is_sent_once` (an
unchanged pane keeps its first fitted size), `a_resize_history_ends_at_the_size_a_fresh_owner_fits` (a fractional
advance at scale 1.5) and `the_fit_is_the_documented_formula_at_every_text_size_and_scale` (the bundled font at 9 to 28
points, scales 1, 1.5 and 2)

### `grid-client/grid-terminal.spec.ts`

Tests: 41. Covers: a recorded attach draws its text, wide boxes, styles and cursor in their cells.

replaced: typing by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`program_keys_are_sent_to_the_live_attachment`, `text_an_input_method_commits_reaches_the_program`,
`a_dead_key_composes_with_the_next_key`; focus reporting by `a_program_asking_for_focus_reports_hears_focus_change`,
`a_program_that_starts_asking_hears_at_once_without_a_frame`; the alternate screen's wheel by
`the_wheel_goes_to_the_program_on_the_alternate_screen`; scrollback by
`the_wheel_scrolls_history_and_input_returns_to_the_bottom`, `the_page_keys_page_through_history_and_send_nothing`,
`the_scrollbar_shows_on_hover_and_for_a_moment_after_a_scroll`,
`the_terminals_scrollbar_moves_the_view_and_sends_nothing`, crates/gui/farhelm-gui/src/terminal/tests.rs
`the_view_scrolls_into_history_and_stays_on_its_lines`, `trimmed_history_shows_its_notice_above_the_oldest_line`,
crates/gui/farhelm-grid-model/tests/model.rs `the_notice_line_sits_above_history_that_starts_at_line_zero`,
crates/gui/farhelm-gui/src/components/tests/scrollbar.rs
`a_press_on_the_track_jumps_there_and_a_drag_follows_the_pointer` and scripts/gui-e2e-smoke.py
`test_33_the_wheel_scrolls_history_and_typing_returns`; the exit line by
`a_replayed_exit_shows_its_line_with_the_screen`. Selection by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_drag_selects_and_copies_and_a_click_does_not`, `a_double_click_copies_the_word_and_a_triple_click_the_line`,
`under_mouse_reporting_only_the_forcing_modifier_selects`, `a_drag_held_past_the_top_scrolls_and_selects_history`,
`select_all_and_copy_copy_the_whole_terminal`, `a_drag_delivered_between_frames_still_selects`,
`shift_click_extends_a_word_drag_grows_and_a_lost_release_ends`,
`a_drag_ends_when_its_selection_is_reset_or_its_view_replaced`, `alt_drag_selects_a_rectangle_off_a_mac` and
crates/gui/farhelm-gui/src/terminal/tests.rs `a_selection_names_lines_and_select_all_covers_history_and_screen`,
`a_selection_is_trimmed_by_dropped_history_and_cleared_by_a_reset`. Alt-click cursor moves by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_quick_alt_click_moves_the_programs_cursor`,
`an_alt_click_in_the_history_moves_nothing`. Mouse reporting by `the_program_gets_the_mouse_reports_its_mode_asks_for`
and `a_reported_press_clears_the_selection_and_a_lost_release_is_reported` (the right click's browser context menu has
no desktop counterpart). The fit at a new text size by `a_text_size_step_resizes_every_open_terminal`. Drawing by
crates/gui/farhelm-gui/src/terminal/tests.rs `non_ascii_runs_draw_as_one_box_at_their_wire_column`,
`a_hidden_cursor_is_not_drawn_and_comes_back_when_shown` and
`right_to_left_text_and_an_override_keep_their_wire_columns`; a first screen at another size than the pane fits by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_first_screen_at_another_size_is_resized_once` (it was missing:
the size is now corrected once). Pointer and scrollback cases by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`the_wheel_under_mouse_reporting_is_the_programs_on_the_primary_screen`,
`a_reported_drag_released_outside_the_grid_ends_at_its_last_cell`,
`a_reported_press_brings_a_scrolled_back_view_to_the_live_screen`, `a_paste_clears_the_selection`,
`a_stalled_snapshot_still_selects_and_copies_and_sends_nothing`, `a_live_exit_shows_its_line` (a later state without an
exit removes the line) and crates/gui/farhelm-gui/src/terminal/tests.rs
`a_wheel_step_after_the_cap_dropped_lines_moves_by_the_step_alone`,
`a_hovered_link_stays_hovered_across_output_to_another_row`. The selection pressed exactly mid-cell by
crates/gui/farhelm-input-mapping/src/pointer.rs `selection_midpoints_cells_and_captured_edges` (the boundary a press
rounds to).

- "a recorded attach draws its text, wide boxes, styles and cursor in their cells" by
  crates/gui/farhelm-grid-model/tests/fixtures.rs `recorded_attach_and_scroll_streams` (the recordings' screen, styles
  and cursor), crates/gui/farhelm-gui/src/terminal/tests.rs `non_ascii_runs_draw_as_one_box_at_their_wire_column`,
  `styles_resolve_with_the_old_apps_precedence` and `a_cursor_covers_a_wide_characters_whole_box`, and the
  `terminal-styles` and `terminal-cursors` goldens (the pixels).
- "the cursor follows focus, shape and visibility" by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
  `the_cursor_blinks_only_while_focused` (drawn outlined while unfocused), terminal/tests.rs
  `a_hidden_cursor_is_not_drawn_and_comes_back_when_shown` and the `terminal-cursors` golden (the shapes; no unit test
  draws the bar).
- "a click on a link reports it; a drag across one never does; a withheld press reaches the program not at all" by
  `the_link_choice_decides_who_gets_a_click_on_a_link`, `a_printed_web_address_hovers_and_opens_on_a_click` (a drag
  opens nothing) and `a_withheld_press_on_a_web_link_never_raises_the_notice` (a drag pressed on a link reaches the
  program not at all).
- "a large history is applied in slices: the screen draws at once and the rest fills in": superseded (decided by the
  migration's orchestrator on 2026-11-16): delivery is bounded per main-thread turn rather than sliced within one
  message, and one message is applied whole. The bounded batches that yield by
  `a_flood_is_delivered_in_bounded_turns_that_yield`, the redraws by terminal/tests.rs
  `shown_output_redraws_and_hidden_output_does_not`, and a large history accepted whole by
  crates/gui/farhelm-grid-model/tests/model.rs
  `large_valid_history_append_is_not_rejected_by_a_client_only_count_limit`.
- "history trimmed after falling behind keeps a scrolled-back view at the oldest line" by model.rs
  `viewport_anchors_survive_output_and_clamp_after_eviction` and
  `large_valid_history_append_is_not_rejected_by_a_client_only_count_limit`.
- "a history link keeps its answer and hover across lazy reads and unrelated output" by terminal/tests.rs
  `a_hovered_link_stays_hovered_across_output_to_another_row` and crates/gui/farhelm-gui/src/terminal/files/tests.rs
  `folders_unreadable_and_other_refusals_say_why_and_admit_nothing` (a refused hover kept across output).
- "a late link answer still hovers after output that did not touch its line" by files/tests.rs
  `a_pending_check_survives_output_that_does_not_touch_its_line` (a check still pending across a cursor move and a
  rewrite of another row) and `a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes`
  (answered checks).
- "a late history link answer is refused after a same-number same-text reset" by files/tests.rs
  `a_pending_history_check_is_not_used_after_a_same_number_same_text_reset`. It departs from the old case: the reset
  returns the view to the live screen, which drops the pending check, where the old view stayed in history and the old
  grid refused the late answer by comparing the stored line (the test's NOTE says why).
- "a late link answer is not used for a line the view scrolled away from the pointer" by files/tests.rs
  `a_pending_check_is_not_used_for_a_line_scrolled_away`.
- "a column selection whose first lines drop at the cap keeps its columns" by
  crates/gui/farhelm-gui/src/terminal/tests.rs `a_column_selection_trimmed_at_the_cap_keeps_its_columns`.
- "a bad history row found after the frame that carried it is reported as a protocol error": superseded (decided by the
  migration's orchestrator on 2026-11-16): the model checks a message whole as it applies it (model.rs
  `invalid_frame_leaves_committed_and_pending_state_unchanged`, `stream_ordering_is_enforced_before_model_mutation`), so
  no row is checked after its frame, and a fault restarts the stream
  (`a_protocol_fault_restarts_the_stream_and_its_replay_replaces_the_screen`).
- "redraws ask for links never with nothing hovered, and once per change to a hovered link's rows": its web-link half
  superseded, as in terminal-files.test.js's entry (the view finds its hovered link again synchronously on every
  redraw); its host-check half by `a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes` (no
  check on an equal rewrite or on another row).
- "a late link answer is dropped when a row of a long printed URL beyond the path detector's reach changes": superseded
  for the same reason, since a web address has no answer to wait for; a changed link text drops its hover by
  terminal/tests.rs `a_hovered_link_is_underlined_and_dropped_when_its_text_changes`.
- "disposing the grid mid-drag removes the drag's document listeners": superseded (no document listeners); the drag's
  end by attached/tests.rs `a_drag_ends_when_its_selection_is_reset_or_its_view_replaced`.
- "the context menu finds the input under the pointer, holding the selection": superseded with the browser context menu
  (the right-click deviation below).
- "history keeps being applied after the page is hidden": superseded: the browser throttled a hidden page's frames, and
  the desktop app delivers output on main-thread turns, not frames (`a_flood_is_delivered_in_bounded_turns_that_yield`).

### `tests/grid-flood.spec.ts`

Tests: 1. Covers: a flood's scrollback arrives whole and in order, through a viewer that stops reading.

replaced: the old case drove an 800,000-line flood end to end through a viewer that stopped reading; here each hop's
flow control is pinned on its own, as terminal-keys' entry does for its bytes (the end-to-end flood replaced hop by hop,
decided by the migration's orchestrator on 2026-11-16).

- The window's delivery in bounded turns that yield by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
  `a_flood_is_delivered_in_bounded_turns_that_yield`.
- The helm client's bounded event queue, which stops reading its socket while the window is behind, by
  crates/farhelm-helm-client/src/terminal/tests.rs `a_caller_that_falls_behind_slows_the_stream_instead_of_breaking_it`.
- The helm pausing and resuming the supervisor's grid stream for a reader that falls behind, every frame arriving in
  order, by crates/farhelm-helm/src/terminal.rs `a_slow_browser_pauses_and_resumes_its_grid_stream` and
  `a_slow_browser_survives_large_rows_arriving_after_the_pause`.
- The supervisor withholding a flood from a paused viewer and then delivering its history with no line lost or repeated
  and no history reset by crates/farhelm-supervisor/src/service/grid_stream/tests.rs
  `a_paused_flood_is_withheld_and_loses_no_history_line`.

### `tests/grid-terminal.spec.ts`

Tests: 1. Covers: a host terminal draws as a grid and takes typing.

replaced: scripts/gui-e2e-smoke.py `test_02_the_open_sessions_terminal_takes_typing`

### `tests/terminal.spec.ts`

Tests: 58. Covers: renders the session and the agent's TUI output.

partly: the host-colored terminal frame (its "host-colored terminal frame follows local appearance and Settings" case)
by crates/gui/farhelm-gui/src/terminal/attached/tests.rs `the_frame_takes_the_hosts_color_while_the_preference_is_on`.
Ghostty's colors and the pane ground beyond the last cell are pinned by the GUI goldens (`terminal-styles`,
`terminal-cursors`, `terminal-status`), which run through scripts/check-gui-goldens.py rather than `cargo test`.
Switching while the first stream still attaches closes it, so nothing it sends later has a reader, by
crates/gui/farhelm-gui/src/session_pane/tests.rs `switching_while_the_first_stream_attaches_closes_it`. A terminal
switched away from and back to replaying its screen and taking typing again (the reselect half of its "switching
sessions tears down the mounted terminal; reselecting mounts a fresh one" and, with a session switch in place of the
page reload, "reload reattaches with replayed scrollback") by scripts/gui-e2e-smoke.py
`test_72_a_reopened_terminal_replays_its_screen_and_takes_typing`, and a resize reaching the program's terminal (its
"resize reaches the real terminal", triggered by a text size step rather than a window resize, which nothing checks end
to end yet) by `test_73_a_text_size_step_resizes_the_programs_terminal`. Every case the terminal owns is replaced or
superseded below; the rest are other areas', listed below with their files, whose entries say how far each is ported.

- "renders the session and the agent's TUI output" and "input round-trips through the real terminal path" by
  scripts/gui-e2e-smoke.py `test_02_the_open_sessions_terminal_takes_typing` (a click opens the row, the terminal shows
  `FAKE-AGENT READY`, and a typed line comes back as its `echo:`); the first case's details title is the session view's.
- The teardown half of "switching sessions tears down the mounted terminal; reselecting mounts a fresh one" by
  crates/gui/farhelm-gui/src/terminal/set/tests.rs `choosing_another_session_ends_every_terminal_of_the_open_one`.
- "a final detach banner is held while a delete is in flight and paints if it fails" by
  crates/gui/farhelm-gui/src/session_pane/tests.rs `the_terminal_holds_back_what_the_delete_does_to_it` (nothing drawn
  while the Delete runs) and `a_refused_delete_shows_the_notices_it_held` (the held exit line and reconnect card after a
  refusal; the old case's stall banner is the reconnect card here), with the detach reason's invisible characters
  escaped by crates/gui/farhelm-gui/src/terminal/attached/tests.rs `detached_and_refused_offer_reconnect`.
- "second client takes over; first shows the detach banner": superseded, as one UI holds the helm at a time and a
  displaced window shows no terminal (decided by the migration's orchestrator on 2026-11-15; SPEC_impl.md, "The seat:
  one UI per helm").
- "terminal mounting waits for the clipboard naming helper", "a failed mount rolls back cleanly; the same session can be
  mounted again" and "session readiness rejects a reused primary island until the requested session opens": superseded,
  being the browser's island mounting and its test oracle; the desktop app mounts no islands and loads no helper script
  (as terminal-tabs.spec.ts's entry says of its mount cases).
- Owned by sidebar and session view, tracked in docs/gpui/parity/sidebar-and-session-view.md (its
  `tests/terminal.spec.ts` (its row-action cases) entry): the details title half of "renders the session and the agent's
  TUI output"; "list renders the session row with title, cwd, invocation, and the status the API reports"; "keyboard
  activation opens the session, matching a real click"; "alive delete opens an inline confirming state with the
  is-still-running wording and the session title"; "confirming an inline delete prompt deletes the session with exactly
  one DELETE request"; "exited session deletes immediately with no confirming state"; "delete confirmation safely
  displays a title containing executable HTML without ever parsing it as markup"; "a legal multi-KB, unbroken title
  keeps the consequence text intact and clips only the title, without disturbing the confirm/cancel buttons"; "an inline
  confirming state survives a listing refresh; cancel still works afterward"; "one row's confirming state does not
  affect another row's controls"; "an alive-to-exited status change under an open confirm prompt keeps confirming, with
  the fallback wording"; "a failed listing read while confirming does not clear the confirming state";
  "create-shows-no-badge-until-a-status-is-classified"; both "details preserve ..." cases (a missing legacy host, an
  unknown host); "deleting a session with unknown status confirms first, with wording that admits uncertainty"; "stop
  and delete failures surface in the row's own error line, without disturbing the rest of the list"; "a failed action's
  error is keyed to its own session, not shared across rows"; "stop's in-flight guard keeps navigation and another row's
  actions usable"; "rapid stop/delete clicks on the same row never let a confirmed delete silently vanish"; "dispatching
  cancel and confirm in the same tick never deletes the session"; "the confirm prompt focuses delete; held Enter waits
  for a new press"; "list refreshes an existing row from alive to exited, then drops it on delete"; "an interrupted
  session shows its badge and deletes without confirming"; "an error session shows its badge with detail and deletes
  without confirming"; "a long error detail wraps completely and compact ended rows stay one line"; "compact rows retain
  distinct ended and harness glyphs within two characters"; "agent marks keep identity colors and invert on selection in
  ${palette}"; "an error detail containing executable HTML renders literally in the badge"; "a live session draws a dot
  with a hidden word and a relative age"; "an idle session already seen uses its row's status foreground with the plain
  word"; "an idle session with unseen output uses its row's new-output foreground and accessible words"; "only a
  reachable running dot pulses in ${palette}"; "truncation banner shows when the listing reports truncated"; "list polls
  and picks up a session created elsewhere".
- Owned by sidebar and session view, tracked in docs/gpui/parity/sidebar-and-session-view.md (its
  `tests/terminal.spec.ts` (its stale cases) entry): "opening a stale session shows metadata and calm terminal recovery"
  and "stale-session-view-with-no-status-renders-metadata-and-no-badge".
- Owned by the launcher, tracked in docs/gpui/parity/launcher.md (its `tests/terminal.spec.ts` (terminal) entry): "the
  create form prefills the working directory with ~ when no session is selected"; "multi-session flow: create two, open
  and type in one, stop and delete the other, then delete the first with confirmation"; "create dialog surfaces a
  precondition failure, preserves the form, and creates no session"; "create dialog disables the submit control while a
  create is in flight"; "submitting the create form twice while one create is in flight produces exactly one session";
  "a blank title creates a session titled after the working directory's basename, not an empty string".
- The helm HTTP API's cases, by the helm's own tests rather than a GUI port (its routes are still served): "create API
  reports a precondition failure containing the supervisor's own text" by crates/farhelm-helm/src/sessions_tests.rs
  `create_session_error_reply_maps_to_bad_request_status` (400, with the supervisor's words in the body) and
  crates/farhelm/tests/e2e/session_lifecycle.rs `create_in_missing_directory_errors` (the real supervisor's refusal of a
  missing working directory); "stop and delete a session through the HTTP API" by sessions_tests.rs
  `stop_session_happy_path_returns_200_with_empty_object_body` and
  `delete_session_happy_path_returns_200_with_the_archive_fact`, with the stop and delete themselves against a real
  supervisor by e2e session_lifecycle.rs `stop_kills_the_whole_process_tree` and
  `delete_removes_session_terminal_and_row`; "requests from a foreign origin are refused" by
  crates/farhelm-helm/src/middleware.rs `any_origin_is_refused`, sessions_tests.rs
  `foreign_origin_is_refused_on_the_stop_route` and crates/farhelm-helm/src/lib.rs
  `foreign_host_is_rejected_but_still_carries_build_stamp_header`.
- "responses carry the anti-framing headers": superseded (decided by the migration's orchestrator on 2026-11-16): it
  guarded a browser page against framing, and the desktop app has no browser page to frame; the helm sends neither
  `X-Frame-Options` nor `frame-ancestors` now.

### `tests/terminal-attachments.spec.ts`

Tests: 36. Covers: a file dropped on the agent terminal uploads and inserts its host path at the cursor.

replaced, by crates/gui/farhelm-gui/src/terminal/attachments/tests.rs:
`two_dropped_files_are_typed_as_two_words_in_order` ("a file dropped on the agent terminal uploads and inserts its host
path at the cursor", "two dropped files insert both paths, each its own word"),
`a_pasted_file_is_uploaded_and_its_host_path_typed` (the paste half of the first, and "a completed upload keeps focus
where it was and inserts through bracketed paste"), `a_drop_on_a_tab_reaches_only_that_tab` ("a file dropped on a tab
inserts its path in that tab", "an attachment failure stays in the terminal that received it"),
`a_pasted_image_is_uploaded_under_a_generated_name`, `pasted_text_that_looks_like_a_path_is_typed`,
`a_dropped_directory_is_refused_visibly` and `a_pasted_directory_is_refused_visibly` (the dropped directory),
`typing_goes_on_during_an_upload_and_the_path_lands_after_it` (typing during an upload),
`pasted_files_upload_one_at_a_time_and_a_failure_stops_none` (two real files in one payload, a failure on the first of
two), `each_failure_says_whether_the_file_was_attached` (an upload that fails, a dead request and a malformed reply,
error text carrying replacement tokens, an item whose bytes cannot be read; each line now also says whether the file was
attached or that this cannot be told, which SPEC.md requires), `a_typed_path_is_one_shell_word` (a path whose
directories need quoting, through `sh` directly rather than a tab's shell),
`an_upload_that_lands_with_no_live_terminal_shows_its_path` (an upload that outlives its socket; the line no longer says
"copy it from here", since the strip's text cannot be selected),
`a_paste_or_drop_into_a_terminal_that_is_not_live_is_refused_until_it_is_live` ,
`a_drop_on_an_inert_terminal_is_refused_until_the_veil_lifts` and `a_drop_on_the_reconnect_card_is_refused` (a payload
dropped on a detached terminal, the refusal clearing on reconnect, and the pane half of a file dropped where no live
terminal takes it), `the_strip_never_resizes_the_terminal`, `the_strip_names_what_is_running_and_counts_what_waits` (the
indicator naming the upload still running; its count is the uploads running and waiting, "Attaching 3 files…", where the
old line counted those running at once, because uploads now run one at a time),
`going_live_takes_away_the_refusal_and_keeps_a_failure`, `a_reset_reports_a_cut_off_upload_in_the_new_agent_terminal`,
`a_closed_tab_reports_its_cut_off_uploads_in_the_agent_terminal` (uploads cut off by a reconnect: here a terminal goes
away whole, when its tab closes, its session is left or the window's connection resets, while a stream's own reconnect
keeps its uploads running) and `a_failure_and_a_cut_off_upload_outlive_their_terminal` (an upload failure staying on
screen across a reconnect; an upload cut off by leaving its session is reported as a notice instead). The strip's look
has no captured scene yet: new scenes are app scenes, not committed goldens (the maintainer's decision, 2026-11-15), and
an app scene would have to paste or drop a file into the running app, which the bridge has no command for (headless
Linux gives the app no clipboard either), so it waits for such a command. A terminal that goes away hands its lines to
the session's agent terminal, so after a reset, or once a tab closes, the failures still on a tab's strip show in the
agent terminal, not in the tab. The zero-byte file and the file name full of URL syntax are the helm client's:
crates/farhelm-helm-client/src/transfers/tests.rs `upload_streams_the_whole_file_with_its_content_length` (an empty
file's empty body, the proposed name percent-encoded in the query) and crates/farhelm-proto/src/ui_channel/path.rs
`query_values_are_encoded_on_their_own` (a value's `&` and `=` encoded, so a name cannot add a parameter). Superseded:
the two `upload-skew-surfaces-on-the-terminal` cases, because the desktop app embeds its helm, so the window and the
helm are always one build (decided by the terminal area's owner on 2026-11-15); "clipboard facts survive a failed File
projection" and both "a file reference beside raw image data reports the omitted image" cases, which are the browser's
clipboard objects (GPUI hands over paths, image bytes and text, and its macOS read never reports a file and an image
together); "pasted and dropped text cannot close bracketed paste early", "file and text pastes are intercepted before
the terminal sees the event", "a payload with files but no items takes the same path", "a text-only drop is prevented,
inserted, and leaves the page where it was" and "an unsupported drop leaves an earlier failure on screen", the browser's
event plumbing (a native drop carries files only, an empty one is ignored, and stripping an embedded end marker is the
supervisor's, `grid_input.rs`); the page-wide half of "a file dropped where no live terminal takes it" (a native window
has no default action for a drop, so nothing can open the file; the text-field cases went with item H2 of
docs/gpui/browser-behaviors.md, at commit b97cd023, since removed); "leaving mid-upload and reopening leaves exactly one
set of hooks" (no DOM listeners to stack; its upload half is above); and "a filename and a server error carrying markup
render as literal text" (the desktop app draws no markup; the strip draws host values through the escaping host-text
element)

### `tests/terminal-clipboard.spec.ts`

Tests: 8. Covers: an OSC 52 write from the terminal's own process reaches the system clipboard, byte for byte.

replaced: the old file drove each case through a real pty and the browser's clipboard; here the hops are pinned one by
one, in crates/gui/farhelm-gui/src/terminal/attached/tests.rs unless named otherwise.

- "an OSC 52 write from the terminal's own process reaches the system clipboard, byte for byte" by
  `a_programs_clipboard_write_reaches_the_clipboard`, crates/gui/farhelm-grid-model/tests/model.rs
  `clipboard_parts_are_atomic_and_primary_is_ignored` (delivered whole, only with its last part),
  crates/farhelm-supervisor/src/grid_output/tests.rs `clipboard_writes_split_into_parts_that_fit` (cut at character
  boundaries into parts that rejoin to the write) and crates/farhelm-supervisor/src/service/grid_stream/tests.rs
  `clipboard_writes_arrive_whole_and_latest_per_target`.
- "an OSC 52 read query never receives real clipboard contents" by crates/farhelm-terminal-emulator/tests/queries.rs
  `color_and_clipboard_queries` (a clipboard read is never answered).
- "a real drag-select copies the local selection to the system clipboard exactly" by
  `a_drag_selects_and_copies_and_a_click_does_not`.
- "a drag released outside the terminal element still copies" by `a_drag_held_past_the_top_scrolls_and_selects_history`
  (released above the screen, and what was selected is copied) and
  `shift_click_extends_a_word_drag_grows_and_a_lost_release_ends` (a release the window never saw ends the drag at the
  next move, which copies).
- "reselecting identical text re-copies after an external clipboard change" by
  `a_drag_selects_and_copies_and_a_click_does_not` (the same selection copies again over another owner's text).
- "under mouse reporting: a plain drag copies nothing, Shift+drag copies exactly" by
  `under_mouse_reporting_only_the_forcing_modifier_selects` (Shift off a Mac, Option on one).
- "a click without a drag does not clobber the system clipboard" by `a_drag_selects_and_copies_and_a_click_does_not`.
- Both "an OSC 52 write survives navigator.clipboard.writeText that ${behavior.label}" cases (rejects, never resolves):
  superseded (decided by the migration's orchestrator on 2026-11-16), as failure modes of the browser's asynchronous
  clipboard API, which the desktop app does not use.

### `tests/terminal-files.spec.ts`

Tests: 6. Covers: changed OSC display text starts a fresh hover at the same cells.

replaced: the hover and its refusals by crates/gui/farhelm-gui/src/terminal/files/tests.rs: "changed OSC display text
starts a fresh hover at the same cells" by `a_relabel_withdraws_the_click_until_checked_again`; "file hover shows
decimal size tiers and a red cap refusal" by `sizes_and_tiers_use_decimal_boundaries_including_the_exact_limit` (the
sizes, tiers and their colors), `folders_unreadable_and_other_refusals_say_why_and_admit_nothing` (the cap refusal) and
the `terminal-file-disclosures` golden (the look); "leaving cancels lookup and the next hover checks again" by
`each_hover_checks_afresh_and_leaving_cancels`; the refusal half of "file refusals, OSC file save and forbidden OSC
schemes" by `folders_unreadable_and_other_refusals_say_why_and_admit_nothing`; the hover half of "file hover and
complete browser download" by `a_hover_shows_checking_then_the_hosts_answer_and_a_click_is_admitted`, against the fake
transfers served through the UI session. As in the old app, a file click is Farhelm's whatever the link choice and its
press is not withheld from a program under mouse reporting (pinned by
`a_file_click_is_farhelms_whatever_the_link_choice` and
`under_mouse_reporting_a_file_click_is_admitted_and_still_reported`). The saves, against the fake transfers (the helm's
own tests cover the save on the host): the local and SSH downloads by
`a_click_saves_into_downloads_and_the_line_says_where` (the session's host is the helm's to reach, so the two are one
case here); the OSC file save by `a_relabel_withdraws_the_click_until_checked_again`; the failure after the file
vanished by `a_success_fades_and_a_failure_stays_until_the_next_press_or_download`; "both halves of a program-broken
path disclose and download the whole file" by `both_halves_of_a_cut_path_check_and_save_the_whole_file`. The forbidden
OSC schemes are the web links' rule, pinned there: a `javascript:` hyperlink shows no display and a click opens nothing
by crates/gui/farhelm-gui/src/terminal/attached/tests.rs `hostile_hyperlink_text_and_targets_stay_data`, and only an
http or https target with a host reaches the browser by crates/gui/farhelm-grid-model/tests/links.rs
`prose_detection_and_native_browser_scheme_boundary`

### `terminal-files.test.js` (unit tests)

Tests: 35. Covers: the old JS unit tests (`crates/farhelm-ui/js-tests/`): path detection, file hovers and transfers.

partly: detection by crates/gui/farhelm-grid-model/tests/paths.rs
`paths_exclude_diagnostic_locations_prose_punctuation_urls_and_versions`,
`bare_digit_leading_extensions_keep_the_numeric_token_exclusions`, `a_long_numeric_looking_file_name_is_a_path` and
`version_spellings_stay_out_including_a_long_near_match` (both without the old time bounds, which guarded against
backtracking the regex crate cannot do), `program_hyperlinks_give_decoded_absolute_file_paths_only`,
`path_cells_map_through_wide_combining_and_wrapped_cells`,
`a_path_cut_near_the_right_edge_joins_through_a_soft_wrap_and_a_program_break`,
`cut_paths_keep_delimiters_column_limits_and_the_two_row_boundary` and
`a_plain_word_cannot_donate_a_head_to_a_cut_path`; the detection half of "a wrapped or program-broken printed path
cannot grow beyond its cached endpoint" by `a_printed_path_extended_on_its_next_row_is_a_different_link`, and the range
comparison in "different paths, cell endpoints and unknown ranges start fresh checks" by
`same_range_compares_both_endpoints_and_fails_closed`. The file hover by
crates/gui/farhelm-gui/src/terminal/files/tests.rs: "only a visible fully fitting disclosure can authorize a transfer"
and "clipping refuses transfer and shows failure after its own click" by
`a_click_whose_file_details_are_not_fully_visible_is_refused` (the fit itself is the web links' shared check); "file
sizes and tiers use decimal boundaries including the exact cap" by
`sizes_and_tiers_use_decimal_boundaries_including_the_exact_limit`; "file icons distinguish documents, PDFs, images,
archives and folders" by `icons_distinguish_documents_pdfs_images_archives_and_folders`; "hover preserves both path
pieces and advertises open only for native admitted types" by
`hints_offer_opening_only_where_the_helm_does_with_the_platforms_modifier` and
`a_hover_shows_checking_then_the_hosts_answer_and_a_click_is_admitted`; "same-task redraw preserves answered ready and
refused hovers" by `a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes` (ready) and
`folders_unreadable_and_other_refusals_say_why_and_admit_nothing` (refused); "real departure aborts and hides, and a
later same-path hover checks afresh" by `each_hover_checks_afresh_and_leaving_cancels`; "same-task redraw preserves a
pending hover and updates its newest link" by `a_pending_check_survives_output_that_does_not_touch_its_line` (a redraw
keeps a hover by the same link comparison, whatever the check's state, and the pending check is answered and admitted);
"different paths, cell endpoints and unknown ranges start fresh checks", "changed and unknown OSC display text cancels
old evidence at the same target and cells", "partial wrapped redraw invalidates file transfer evidence" by
`a_relabel_withdraws_the_click_until_checked_again` and
`a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes`; the admission half of "a wrapped or
program-broken printed path cannot grow beyond its cached endpoint" by
`ready_for_requires_the_same_cells_file_and_label` (another end cell is not admitted); "file transfers refuse gestures
the terminal did not admit as clicks" by `a_drag_across_a_checked_path_is_never_admitted`. Superseded by the GPUI view,
which finds its hovered link again synchronously on every redraw and draws the pointer from it, with no deferred leave
and no cursor setters: "ready redraw cursor restoration waits for accessors and rejects retired links" and "leave
withdraws admission immediately and deferred teardown leaves another cursor alone"; the pointing hand that waits for a
ready check is pinned by `the_pointing_hand_waits_for_a_ready_check_on_a_printed_path`, through the predicate the
terminal draws it from. "disposal clears pending hover teardown and rejects late replies": superseded with the
JavaScript disposal it tested: dropping a terminal drops its check's task, which is not tested on its own. The downloads
by crates/gui/farhelm-gui/src/terminal/files/tests.rs: "modifier opening is native-only and save outcomes survive opener
refusal" and "a native modified click saves without opening when hover offered only download" by
`the_modifier_click_opens_only_what_the_helm_offered` (native only: the desktop app's helm saves, so there is no browser
half); "browser and native successes fade, while a failure waits for a terminal click" and "replacement cancels an
earlier notice deadline and retires a failure" by `a_success_fades_and_a_failure_stays_until_the_next_press_or_download`
(deviation: the success line vanishes at five seconds rather than fading out over the old 200 ms, which the ticker's ten
ticks a second would draw as two frames), and a late outcome replacing a refusal, as the old message did, by
`a_save_failing_behind_a_refusal_shows_its_failure` and `a_refusal_takes_the_status_lines_place`. "terminal disposal
clears notices and rejects a late native save result": by `a_save_outlives_the_hover_but_not_the_terminal`. The vendored
`hitbox_occlusion` test measures the covering regions in the rendered frame. The grid-attach cases above exercise the
actual approval card and click refusals. Superseded (no browser or DOM): the bounded Blob (the helm saves the desktop
app's downloads), the browser timers' receiver, and construction without a terminal element. Deviation: the old path
link was one span, so a cut path's head padding and tail indentation were underlined and hoverable; here only the path's
own cells on each row are (pinned by `a_path_cut_near_the_right_edge_joins_through_a_soft_wrap_and_a_program_break`)

### `clipboard-name.test.js` (unit tests)

Tests: 11. Covers: the old JS unit tests (`crates/farhelm-ui/js-tests/`): a pasted clipboard file's name and a pasted
image's extension.

replaced: the extension derivation by crates/gui/farhelm-gui/src/terminal/attachments/tests.rs
`an_images_extension_comes_from_its_type` (with the old Rust alias table, so `image/x-icon` is `ico`), a copied file and
a non-image file keeping their own names by `a_pasted_file_is_uploaded_and_its_host_path_typed`, and a nameless or
freshly synthesized clipboard image getting a generated name by `a_pasted_image_is_uploaded_under_a_generated_name`.
Superseded: the two tests of capturing the paste event's facts, and the four of the placeholder-name heuristic (an old
or fresh `image.png`, the documented false positive, a nameless non-image, the files-only fallback), because GPUI hands
a pasted image over as bytes with no file name, so every pasted image gets a generated name and no name is judged
(decided by the terminal area's owner on 2026-11-15)

### `copy-on-select.test.js` (unit tests)

Tests: 21. Covers: the old JS unit tests (`crates/farhelm-ui/js-tests/`): gesture copying and the drag-copy notice.

replaced: the notice's cases by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_plain_drag_under_mouse_reporting_raises_the_notice_after_the_grace_window`,
`no_notice_after_a_forced_drag_a_click_or_a_drag_without_mouse_reporting`,
`a_program_clipboard_write_in_the_grace_window_suppresses_the_notice`, `only_codex_has_its_own_copy_instruction` (the
text and the platform's key), `the_notice_sits_three_rows_from_the_release_with_its_fallbacks` (the five placement
cases, the row height and the clamp to the window's content area), `the_notice_goes_after_thirty_seconds` and
`the_notices_x_puts_it_away_keeping_focus_and_sending_nothing`; the copy decisions by the selection tests there
(`a_drag_selects_and_copies_and_a_click_does_not`, `under_mouse_reporting_only_the_forcing_modifier_selects`). A
retained selection under a plain tracked press is never copied again, because a reported press clears the selection here
(`a_reported_press_clears_the_selection_and_a_lost_release_is_reported`). Superseded: the narrowed viewport's
remeasuring (GPUI lays the popover out on its own, not at its last place), and the browser-global and listener-disposal
cases, which have no counterpart

- The scrollbar case of "no notice for a gesture that copied or never tried to" (a press outside the terminal screen, on
  its scrollbar, under mouse reporting) by `a_drag_on_the_scrollbar_under_mouse_reporting_raises_no_notice`, with
  `the_terminals_scrollbar_moves_the_view_and_sends_nothing` (the press sends the program nothing under drag reporting
  too).

### `tests/terminal-flood.spec.ts`

Tests: 3. Covers: a terminal socket for an unknown session reports why.

replaced: the paste of over a megabyte by `terminal/attached/tests.rs` `a_large_paste_arrives_whole_and_in_order` and
scripts/gui-e2e-smoke.py `test_30_a_large_paste_arrives_whole_inside_its_brackets` (macOS only; skipped on headless
Linux, which gives the app no clipboard). The old file drove its other two cases through a real helm and pty; here, as
for terminal-keys, the hops are pinned one by one.

- "a terminal socket for an unknown session reports why" by crates/farhelm/tests/e2e/session_lifecycle.rs
  `attach_to_unknown_session_errors_and_connection_survives` (the supervisor's error names the session),
  crates/farhelm-helm/src/terminal.rs `only_a_missing_tab_is_reported_as_tab_closed` (the agent terminal's refusal
  reaches the client as a `detached` notice) and `term_ws_with_unknown_tab_id_surfaces_the_supervisors_not_found_error`
  (with the supervisor's words, for a tab), and crates/gui/farhelm-gui/src/terminal/attached/tests.rs
  `detached_and_refused_offer_reconnect` (a refusal shows "Connection error" and offers Reconnect).
- "real backspace erases; real ctrl-c kills the fake agent": the bytes each key sends, through the supervisor's real
  encoder, by crates/gui/farhelm-input-mapping/src/tests.rs `meaningful_native_chords_have_exact_supervisor_bytes`
  (Ctrl-C as `03`) and `source_modeled_native_us_subset_matches_decisions_and_real_bytes` (Backspace among the named
  keys); what the pty's line discipline then does with them is the kernel's.

### `tests/terminal-font.spec.ts`

Tests: 3. Covers: terminal text size ${size} keeps input focus at its limit.

replaced: crates/gui/farhelm-gui/src/session_pane/header_tests.rs
`a_text_size_press_leaves_focus_on_the_terminal_under_the_macs_click_rule` (a press at the limit keeps the terminal
focused under the Mac's click rule; under Linux's a click focuses the button and the terminal does not take it back, per
SPEC.md "Keyboard focus") and `the_text_size_stops_at_the_ends_of_its_range` (nothing is sent past either end). The rest
of the text size by `the_text_size_buttons_step_the_shared_size_and_the_readout_follows`,
`a_refused_text_size_falls_back_to_the_stored_one`, `the_text_size_shortcut_steps_only_while_a_terminal_is_open`,
`the_text_size_shortcut_does_nothing_without_a_terminal`, `a_terminal_opens_at_the_current_text_size` and
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_text_size_step_resizes_every_open_terminal`. Deviation: the
size is the helm's shared preference instead of each browser's local storage, so the old per-device size is not carried
over. Superseded: the spec's font-gate cases ("a rejected font download leaves the terminal unmounted and logs exactly
one Farhelm font error" and "a held font request keeps the terminal unmounted past the old 3 s deadline, then mounts
once released"), because the desktop app embeds its font and downloads none

### `tests/terminal-keys.spec.ts`

Tests: 5. Covers: Shift+Enter.

replaced: the old file proved the bytes end to end through a real pty; here the hops are pinned one by one. Shift+Enter
as one key message by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`program_input_routes_keys_to_the_program_or_text_input`, and a plain Enter after it by
`a_plain_enter_after_shift_enter_is_sent_without_shift`; a key message's bytes (Shift+Enter as ESC CR, Enter as a bare
CR, Ctrl+Shift+Enter without the ESC) in one input frame by crates/farhelm-supervisor/src/grid_input/tests.rs
`shift_enter_is_esc_cr_in_one_atomic_input_frame`, with every modifier set of Enter by
crates/gui/farhelm-input-mapping/src/tests.rs `source_modeled_native_us_subset_matches_decisions_and_real_bytes` and
`meaningful_native_chords_have_exact_supervisor_bytes`; that pair as one input to the host by
crates/farhelm-supervisor/src/hosts/client/tests.rs `shift_enter_is_one_input`, and a small input as one pty write by
crates/terminal-host/src/host.rs `a_small_input_is_offered_in_one_write`

### `tests/terminal-links.spec.ts`

Tests: 15. Covers: an OSC 8 ${kind} span ignores a selection drag and still accepts a click.

partly: the plain-text URL click, hover and drags that open nothing by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_printed_web_address_hovers_and_opens_on_a_click` and
`the_link_choice_decides_who_gets_a_click_on_a_link`, the hover following redraws by
crates/gui/farhelm-gui/src/terminal/tests.rs `a_hovered_link_is_underlined_and_dropped_when_its_text_changes`, and
detection (wraps, punctuation, schemes) by crates/gui/farhelm-grid-model/tests/links.rs. a program's hyperlink's
destination display, its lookalike warning and click admission by
`a_program_hyperlink_shows_its_destination_and_opens_on_a_click`, `a_lookalike_hyperlink_shows_the_warning` and
`a_click_whose_destination_is_not_fully_visible_is_refused`, the label join and lookalike judgement by
crates/gui/farhelm-grid-model/tests/links.rs `pinned_cell_links_and_label_join_oracle`.

- "clicking an OSC 8 hyperlink opens it in a new tab with no confirm dialog" by
  crates/gui/farhelm-gui/src/terminal/attached/tests.rs
  `a_program_hyperlink_shows_its_destination_and_opens_on_a_click`.
- "plain-text http and https URLs open in a new tab with no confirm dialog" by
  `a_printed_web_address_hovers_and_opens_on_a_click`.
- "a URL wrapped across rows activates from its continuation row with the full target" by
  crates/gui/farhelm-grid-model/tests/links.rs `plain_urls_join_wraps_and_keep_unicode_cell_offsets` (the continuation
  row's link is the whole address) and `a_printed_address_extended_on_the_next_row_opens_its_new_address`.
- "surrounding punctuation is trimmed but in-URL characters are kept" by links.rs
  `prose_detection_and_native_browser_scheme_boundary`.
- "a drag starting and ending inside one URL selects and copies without opening" and "dragging from inside a URL into
  surrounding text selects without opening" by `a_drag_over_a_printed_web_address_selects_and_copies_it` (both copy
  exactly what was dragged over and open nothing), `a_printed_web_address_hovers_and_opens_on_a_click` (a drag opens
  nothing) and `the_release_looks_up_the_hover_again` (a drag from a link released on plain text).
- "an OSC 8 link whose text matches its target, or is not a URL, hovers quietly" by
  `a_program_hyperlink_shows_its_destination_and_opens_on_a_click` (a label that is not an address gets the ordinary
  display), `a_lookalike_hyperlink_shows_the_warning` (the loud control) and links.rs
  `pinned_cell_links_and_label_join_oracle` (what each label claims).
- "an OSC 8 span wins over its https display text, beside a working bare URL" by links.rs
  `osc_precedence_and_generated_continuation_identity` (the program's target beats prose recognition of its text) and
  `a_lookalike_hyperlink_shows_the_warning` (the display shows the text beside the real target).
- "forbidden and malformed schemes are neither decorated nor clickable" by links.rs
  `prose_detection_and_native_browser_scheme_boundary` and `hostile_hyperlink_text_and_targets_stay_data` (a
  `javascript:` hyperlink shows nothing and opens nothing).
- "an OSC 8 ${kind} span ignores a selection drag and still accepts a click": its web kind by
  `a_drag_across_a_program_hyperlink_copies_and_opens_nothing` (the drag copies the label and opens nothing, and a click
  then opens the target), and its file kind by crates/gui/farhelm-gui/src/terminal/files/tests.rs
  `a_drag_across_a_checked_path_is_never_admitted` (a drag is never admitted, a click is).
- "a URL beginning above the viewport activates from its visible continuation" by
  `an_address_begun_above_the_view_hovers_and_opens_whole`.
- "a wrapped URL still activates after a resize reflows it" by `an_address_reflowed_by_a_resize_hovers_and_opens_whole`.
- "a URL scrolled out of view activates after scrolling back to it" by
  `an_address_scrolled_away_and_back_hovers_and_opens`.
- "an http substring inside a forbidden scheme opens only the http part" by
  `an_address_inside_a_forbidden_scheme_opens_only_the_address`.
- "a replayed plain-text URL activates after a reload with no second gate": superseded (decided by the migration's
  orchestrator on 2026-11-16): the desktop app has no page reload, and a replay into a fresh terminal goes through the
  same link detection as live output.
- Refusing a click whose display another Farhelm surface covers uses the layer manager's rendered hitboxes, as in the
  grid-attach cases above. `adjacent_and_pass_through_popovers_do_not_refuse_a_link` checks both exceptions.

### `tests/terminal-multihost.spec.ts`

Tests: 55. Covers: multi-host.

partly: none of its cases is the terminal's own; the spec is about hosts. Its three stale-session cases (a supervisor
outage keeping the screen behind one calm card and reattaching promptly) are the sidebar and session view area's, ported
with the unavailable card (docs/gpui/parity/sidebar-and-session-view.md, its `terminal-multihost.spec.ts` (stale)
entry). Its host Uninstall, Retry and Adopt cases are settings-and-hosts' (docs/gpui/parity/settings-and-hosts.md, its
Uninstall and Retry and Adopt sections); the rest (the host list, menus and settings, creating and cloning across hosts)
are not the terminal's either. Every case is listed in its owner's file, so the terminal area has nothing left to port
here; how far each is ported is in its owner's entry:

- Owned by settings and hosts, tracked in docs/gpui/parity/settings-and-hosts.md (its
  `tests/terminal-multihost.spec.ts`, host cases section): "host-list-states"; "host-remove-escapes-the-sidebar-clip";
  "host-remove-dialog-fits"; "a failed removal stays visible with details collapsed";
  "host-menu-survives-the-longest-phase-word"; "add-host-form-closes-a-stray-host-menu";
  "add-host-form-closes-a-stray-session-menu"; "host-menu-keyboard-contract"; "automatic-setup-menu-order"; both
  "host-appearance" cases; "host-settings-yolo-without-asking"; "host-settings-commands-without-asking";
  "host-settings-own-sessions-without-asking"; "host-settings-dialog-lifecycle"; both "host-settings-pinned-controls"
  cases; "host-settings-refused-save"; "host-settings-backdrop-escape"; "host-menu-busy-guards"; "automatic setup
  refuses forced activation while another host operation is busy"; "local-host-always-listed"; "add-host-discovers";
  "add-host-sends-no-install-paths"; "remove-and-re-add-host"; "host-list-phase-table"; the "host-inline-update" cases;
  "host-retry-click" and "a refused Retry opens a host dialog"; "remove-cancel"; the "Remove keeps its dialog open
  through a refused request" cases; "a refused Remove without confirmation opens a host dialog";
  "remove-skip-confirmation".
- Owned by the launcher, tracked in docs/gpui/parity/launcher.md (its `tests/terminal-multihost.spec.ts` (terminal)
  entry): "create-dialog-host-selector"; "clone-cross-host"; "clone-cross-host-explicit-edit";
  "create-dialog-selector-disappearance"; "create-dialog-selector-disabled-in-flight"; "create-on-unreachable-refused";
  "create-intent-key-rebinds-on-host-change"; "create-intent-key-rebinds-on-retarget".
- Owned by sidebar and session view, tracked in docs/gpui/parity/sidebar-and-session-view.md (its
  `tests/terminal-multihost.spec.ts` (stale) entry): "unreachable-host-goes-stale"; "op-refused-on-unreachable"; "a
  refused Stop leaves the remote agent live after reconnect"; "reboot-while-open"; "reboot-already-interrupted";
  "reboot-restart".

### `tests/terminal-reconnect.spec.ts`

Tests: 24. Covers: socket-killed-reconnects-and-lands-at-tail.

replaced: `manual-reconnect-is-offered-during-the-first-rung`'s card and button: a dropped stream shows the calm
reconnect card ("Session unavailable — reconnecting", its explanation and a neutral Reconnect now with the old hover
help) over the kept screen, with no red band and without taking focus, by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_dropped_stream_shows_the_reconnect_card_over_the_kept_screen`;
one press sends one reconnect-now request by `reconnect_now_sends_one_request_per_press`, and the keyboard reaches and
presses it by `reconnect_now_is_reached_and_pressed_from_the_keyboard`; the card stays through the reconnect's catch-up
and goes when it is live by `the_card_stays_through_the_reconnects_catch_up_and_goes_when_it_is_live`; stalls, refusals
and closes keep the red band by `detached_refused_and_closed_streams_keep_the_red_band`, while a protocol fault in the
reconnect's catch-up keeps the card and restarts the stream by `a_fault_during_a_reconnects_catch_up_restarts_it`; the
card's look, and its cut-short scrolling pane, by no golden since the repository stopped taking new ones (decided by the
migration's orchestrator on 2026-11-15), and by no app scene yet, since the screenshot batch cannot drop a terminal's
stream. A Restart reattaching without the card or a red band, held for the whole recovery the Restart began because the
helm answers it before the reattach, by crates/gui/farhelm-gui/src/session_pane/tests.rs
`a_restart_reattaches_without_a_card_or_a_red_band`, and the card once an attempt fails, or the reattach drops before it
goes live, by `a_restarts_failed_reattach_shows_the_card` and `a_restarts_reattach_that_drops_mid_replay_shows_the_card`
(decided by the migration's orchestrator on 2026-11-15). A stuck Restart stays silent for at most about 35 s: the first
attempt's socket must open within the stream's 30 s watchdog, and an opened one that sends no grid message for 5 s is
ended as a failed attempt by the catch-up's idle bound, ported from the old app, after which the second attempt shows
the card, by crates/farhelm-helm-client/src/terminal/tests/catch_up.rs
`a_reattach_that_sends_no_grid_message_climbs_the_ladder`. A reattach that attaches and then stalls in its replay while
its link keeps answering pings ends its catch-up 5 s after its last grid message, by
`a_replay_that_goes_quiet_partway_ends_its_catch_up_on_what_arrived`, with the caller's own delays not counted by
`the_bound_waits_while_the_caller_is_behind`; the terminal then shows what arrived, a whole screen or not, and goes
live, as the old one did, while the supervisor's own end of replay on a partial screen is still a protocol fault, by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_replay_that_went_quiet_shows_what_arrived_and_goes_live`. Tab
terminals are not held, since a Restart detaches no tab, by crates/gui/farhelm-gui/src/terminal/set/tests.rs
`a_restart_does_not_hold_a_tabs_reconnect_card`; an inert terminal draws no card by crates/gui/farhelm-
gui/src/terminal/attached/tests.rs `an_inert_terminal_draws_no_reconnect_card`, and the card keeps presses from the
screen behind it by `a_drag_on_the_card_reaches_nothing_behind_it_and_the_scrim_lets_presses_through`. The card's phase
and attempt line, from what the fake stream reports: the old card's four templates word for word, with the scheduled
wait rounded as the old card rounded it, by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`the_phase_line_says_the_old_cards_words` (the wait is not counted down, as there, because the line is redrawn only on
the stream's reports; no test pins that); `retry-exhaustion-shows-reprobe-phase`'s probing phase ("Retrying every", the
attempt count past six, Reconnect now still drawn), reached through every rung of the ladder, by
`the_phase_line_follows_the_ladder_into_probing`; `manual-reconnect-is-offered-during-the-first-rung`'s "attempt 1 of 6"
and its press starting that attempt, which the line keeps through the catch-up, by
`reconnect_now_in_the_first_wait_shows_attempt_1_under_way`; the line's look by no golden either. Its takeovers during
backoff no longer apply: one UI holds the helm at a time, and a displaced window shows no terminal (decided by the
migration's orchestrator on 2026-11-15).

- "socket-killed-reconnects-and-lands-at-tail" by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
  `a_dropped_stream_shows_the_reconnect_card_over_the_kept_screen`,
  `the_card_stays_through_the_reconnects_catch_up_and_goes_when_it_is_live` (it recovers by itself) and
  `a_replay_is_shown_only_once_complete` (the replay replaces the kept screen in one redraw); landing at the tail is
  pinned for a tab, by crates/gui/farhelm-gui/src/terminal/set/tests.rs
  `a_tabs_replay_is_staged_on_its_own_and_shown_at_the_bottom`.
- "wedged-socket-detected" by crates/farhelm-helm-client/src/terminal/tests.rs
  `an_unanswered_ping_reconnects_the_stream` (an open socket whose pings go unanswered reconnects by itself).
- "a-non-decision-detach-keeps-the-ladder-climbing" by helm-client
  `each_detach_code_stops_or_reattaches_as_the_policy_says` (`other` waits its rung and reattaches) and
  `a_successful_attachment_starts_the_ladder_over`.
- "leaving-a-session-cancels-a-pending-reconnect" by crates/farhelm-helm-client/src/terminal/tests/ladder.rs
  `letting_go_of_a_stream_during_its_reconnect_wait_cancels_the_reconnect` (closing a stream, or dropping its handles or
  its feed, during a reconnect wait makes no later attempt) and set/tests.rs
  `leaving_a_session_cancels_its_terminals_pending_reconnects` (choosing another session ends both a stream's reconnect
  wait and a faulted stream's restart wait), with set/tests.rs
  `choosing_another_session_ends_every_terminal_of_the_open_one`, crates/gui/farhelm-gui/src/session_pane/tests.rs
  `switching_while_the_first_stream_attaches_closes_it`, and helm-client `dropping_the_event_feed_ends_the_stream` and
  `a_stream_closed_while_waiting_for_the_user_still_reports_closed`.
- "a-stall-during-recovery-stays-detached-across-syncs" by set/tests.rs `a_stalled_terminal_holds_back_no_other` (a list
  push sends a stalled terminal's stream nothing, and it still offers Reconnect), attached/tests.rs
  `detached_refused_and_closed_streams_keep_the_red_band` and `a_stalled_detach_attaches_again_only_when_the_user_asks`,
  helm-client `each_detach_code_stops_or_reattaches_as_the_policy_says` (a stall waits for the user) and
  session_pane/tests.rs `a_restart_does_not_hold_back_a_stall`.
- "takeover-does-not-bounce-back", "a-recovering-view-does-not-take-the-session-back-for-a-new-tab",
  "a-tab-opened-unaware-of-a-takeover-does-not-take-the-session-back" and "manual-reconnect-takes-the-session-back":
  superseded with the takeovers during backoff, for the same reason; a `taken_over` detach reattaching is pinned by
  helm-client `every_attach_takes_the_one_route_and_a_takeover_reattaches` and
  `each_detach_code_stops_or_reattaches_as_the_policy_says`.
- "latched-skew-revokes-automatic-reconnect", both "skew-revocation-disarms-the-heartbeat" cases,
  "a-rolled-back-helm-gets-no-automatic-attach", "client-helm-skew-prompts-reload" and
  "absent-build-stamp-is-skew-and-silences-the-heartbeat": superseded, because the desktop app embeds its helm, so the
  window and the helm are always one build (decided by the terminal area's owner on 2026-11-15, as for
  terminal-attachments' skew cases).
- "bfcache-restore-lets-a-terminal-reconnect-again": superseded: the browser's back-forward cache has no counterpart in
  a desktop window.
- "retry-exhaustion-shows-reprobe-phase" by attached/tests.rs `the_phase_line_follows_the_ladder_into_probing` (the
  card's probing phase, above), helm-client ladder.rs `an_unreachable_terminal_climbs_the_ladder_then_probes_every_30_s`
  (the ladder's waits, then a probe every 30 s without giving up) and helm-client
  `a_probe_past_the_retries_recovers_by_itself_and_starts_the_ladder_over` (past the six retries over real sockets, each
  retry failing as a real connection that drops during its handshake).
- "manual-reconnect-is-offered-during-the-first-rung"'s Page Up scrolling the cut-short card from its focused button by
  attached/tests.rs `the_page_keys_scroll_a_cut_card_from_reconnect_now`.
- "background-probes-recover-without-a-click" by helm-client
  `a_probe_past_the_retries_recovers_by_itself_and_starts_the_ladder_over` (a probe past the sixth rung attaches with no
  request, and the next drop starts the ladder over).
- "silent-attach-is-a-failed-attempt-not-a-recovery" by helm-client
  `a_reattach_the_helm_accepts_and_never_answers_is_a_failed_attempt` (a reattach answering no ping fails by the
  stream's watchdog and climbs the ladder), and, for a reattach whose link answers pings but which sends no replay, by
  crates/farhelm-helm-client/src/terminal/tests/catch_up.rs `a_reattach_that_sends_no_grid_message_climbs_the_ladder`,
  `a_replay_that_goes_quiet_partway_ends_its_catch_up_on_what_arrived` and `the_bound_waits_while_the_caller_is_behind`,
  with attached/tests.rs `a_replay_that_went_quiet_shows_what_arrived_and_goes_live` (the catch-up's idle bound).
- "view-changes-do-not-postpone-a-recovery": superseded (decided by the migration's orchestrator on 2026-11-16): the
  ladder's timer is the stream worker's, which choosing a tab, moving focus or a list push never touches.
- "a-departed-tombstone-is-buried-and-its-terminal-comes-back": superseded (decided by the migration's orchestrator on
  2026-11-16): there is no tombstone state; a terminal that leaves the view is dropped and a fresh one is built on
  return (`choosing_another_session_ends_every_terminal_of_the_open_one`,
  `tabs_opened_and_closed_elsewhere_come_and_go`, scripts/gui-e2e-smoke.py
  `test_72_a_reopened_terminal_replays_its_screen_and_takes_typing`).
- "late-evidence-cannot-confirm-a-closing-socket": superseded (decided by the migration's orchestrator on 2026-11-16):
  one select loop ends a socket, and nothing read after that is applied to it.
- "heartbeat-stays-idle-under-output": a deviation (decided by the migration's orchestrator on 2026-11-16). The helm
  client pings every interval whatever the output, where the old heartbeat stayed idle while output flowed; a ping is a
  control frame the helm answers and the program never sees, so no user can see the difference.

### `tests/terminal-replay-rename.spec.ts` (terminal)

Case: its four catch-up cases, "replay-unconnected: a stalled first mount retries on the reconnect ladder",
"replay-unconnected: without recovery the banner reports it instead of revealing a dead terminal", "replay-starved: a
watchdog expiry that fires late does not end a live catch-up" and "replay-stale-mount: a torn-down island's deferred
ending cannot touch its replacement". Only the terminal's cases are listed here; the spec's entry, with its other cases,
sits in docs/gpui/parity/sidebar-and-session-view.md.

replaced: as follows.

- "replay-unconnected: a stalled first mount retries on the reconnect ladder" by
  crates/farhelm-helm-client/src/terminal/tests.rs
  `a_first_attach_the_helm_accepts_and_never_answers_is_retried_on_the_ladder` (the stream's half) and
  crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_first_attach_that_fails_shows_the_reconnect_card_and_retries`
  (the pane's: the reconnect card, no red band, and typing sent nowhere). The old case kept focus out of such a
  terminal; here choosing a session focuses its terminal whatever its stream is doing (SPEC.md, "Where focus rests").
- "replay-unconnected: without recovery the banner reports it instead of revealing a dead terminal" by attached/tests.rs
  `a_first_attach_that_is_refused_says_so_instead_of_catching_up`. The app has no switch that turns recovery off, so a
  refused first attach, which stops the stream for good, stands in for it.
- "replay-starved: a watchdog expiry that fires late does not end a live catch-up" by helm-client
  `a_watchdog_overdue_behind_a_starved_runtime_does_not_end_an_answered_attach`.
- "replay-stale-mount: a torn-down island's deferred ending cannot touch its replacement": superseded, as for
  terminal-tabs.spec.ts's mount cases: the desktop app mounts no islands.

### `tests/terminal-restart.spec.ts` (terminal)

Case: its two terminal cases, "a restarted session's terminal still shows the previous run's scrollback above the new
one" and "a restart whose response is lost still recovers the terminal". Only the terminal's cases are listed here; the
spec's entry sits in docs/gpui/parity/sidebar-and-session-view.md, which also maps its "a restarted session reconnects
without a detach banner".

replaced: as follows.

- "a restarted session's terminal still shows the previous run's scrollback above the new one" by
  crates/gui/farhelm-gui/src/terminal/attached/tests.rs
  `a_restarts_detach_reattaches_by_itself_and_shows_the_new_run_in_one_redraw` (the client's half: the restart's detach
  reattaches by itself, offers no Reconnect, and shows the new run in one redraw with the carried-over history under the
  wheel) and crates/farhelm/tests/e2e/host_relaunches.rs
  `a_restart_starts_a_new_host_that_keeps_the_history_on_a_blank_screen` (the supervisor's: the new run's terminal keeps
  the first run's history).
- "a restart whose response is lost still recovers the terminal" by crates/gui/farhelm-gui/src/session_pane/tests.rs
  `a_restart_whose_answer_never_comes_still_recovers_the_terminal` (the pane's half: the reattach goes live with no card
  or red band while the Restart is still unanswered) and crates/farhelm-helm/src/sessions_tests.rs
  `a_reply_carrying_unknown_never_erases_a_known_status` (the helm's: what it records from the restart's reply).

### `tests/terminal-switch-geometry.spec.ts`

Tests: 1. Covers: session switching keeps full-width text and rules still at ${font}px.

replaced: crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_fresh_owner_on_the_same_pane_asks_for_the_last_size_and_resizes_nothing` (at 13 points, after a resize history)

### `tests/terminal-tabs.spec.ts`

Tests: 36. Covers: a tab runs a real shell in the session's working directory, leaving the agent terminal alone.

replaced: crates/gui/farhelm-gui/src/terminal/set/tests.rs
`every_tab_attaches_under_the_views_lease_at_the_shown_terminals_size` (positional labels, one lease, hidden tabs
attached at real geometry), `choosing_a_tab_shows_and_focuses_its_terminal`,
`tabs_opened_and_closed_elsewhere_come_and_go`, `a_reaped_tab_goes_silently_and_the_agent_terminal_takes_focus`,
`a_stalled_terminal_holds_back_no_other` (a stopped stream holds back no other terminal; the old takeover latch over new
tabs no longer applies, as one UI holds the helm at a time, decided by the migration's orchestrator on 2026-11-15),
`tabs_past_the_bound_are_listed_but_not_attached`, `a_tab_moving_under_the_bound_attaches`,
`a_hidden_tab_is_resized_with_the_shown_terminal`, `choosing_another_session_ends_every_terminal_of_the_open_one`,
`an_inert_set_makes_every_terminal_and_the_strip_inert`, `a_tab_keyboard_focus_reaches_is_scrolled_into_view`,
`the_tabs_share_the_agent_terminals_tunnels`, `a_remote_sessions_terminals_read_their_tunnels_once`, and
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `a_tab_closed_before_it_was_ever_attached_explains_itself`. Driven
on Linux through the bridge (two tabs opened through the helm API, a command typed into Terminal 1, `exit` in Terminal 2
reaping it). New terminal and Close by `new_terminal_opens_a_tab_and_shows_it`,
`new_terminal_sends_once_and_a_refusal_says_why`, `closing_a_tab_asks_first_then_sends_one_delete`,
`skipped_questions_close_at_once_side_by_side_and_a_refusal_shows_on_its_tab`,
`new_terminal_is_refused_while_a_lifecycle_question_is_open`,
`new_terminal_is_refused_while_a_lifecycle_request_is_pending`. Deviation: the close question keeps the tab's name from
when it opened rather than renumbering with the strip (it acts on the tab's id). Superseded: the browser-only mount
cases (a pending mount cancelled, a failed mount rolled back), which have no counterpart, as the desktop app mounts no
islands.

- "a tab runs a real shell in the session's working directory, leaving the agent terminal alone" by
  crates/farhelm/tests/e2e/terminal_tabs.rs `a_tab_runs_a_shell_in_the_sessions_working_directory` and
  `input_reaches_only_the_terminal_it_was_sent_to`, with the bridge drive above.
- "the strip labels tabs positionally and gives the agent terminal no close control": the labels ("Agent", "Terminal 1",
  ...) by `every_tab_attaches_under_the_views_lease_at_the_shown_terminals_size`, and the agent terminal's tab with no ×
  by `the_agent_terminals_tab_has_no_close_control`.
- "a tab survives a reload, reattaching with its scrollback while still unselected" by
  `every_tab_attaches_under_the_views_lease_at_the_shown_terminals_size` (hidden tabs attached) and
  `a_tabs_replay_is_staged_on_its_own_and_shown_at_the_bottom`; a reload's counterpart, the window's connection reset,
  by crates/gui/farhelm-gui/src/session_pane/tests.rs `a_reset_reattaches_the_surviving_selection`.
- The desktop app remembers the tab the user chose for each session. Set tests cover switching back to a waiting agent's
  session, sending input to the restored tab, clearing the choice with Agent, and falling back when a tab is absent or
  past the attach cap (`reopening_a_session_focuses_the_remembered_tab_until_agent_is_chosen`,
  `a_missing_tab_falls_back_to_agent_without_erasing_the_choice`, and
  `tabs_past_the_bound_are_listed_but_not_attached`). Pane test `a_reset_restores_the_chosen_tab_on_its_new_stream`
  checks the same focus and input path after a helm reconnect.
- Set test `an_interrupted_upload_is_reported_once_across_a_tab_restore` checks that switching away reports the
  interrupted upload in a banner and restoring the tab adds no strip line for it. The quick switcher tests
  `a_hidden_picks_first_row_restores_the_remembered_tab` and `a_hidden_picks_late_row_keeps_agent_after_the_user_types`
  cover a pick whose row arrives after the pane opens.
- "closing a tab confirms in-page, then kills its shell and drops it from the session" by
  `closing_a_tab_asks_first_then_sends_one_delete` and e2e terminal_tabs.rs
  `closing_a_tab_kills_its_shell_and_daemonized_child_and_nothing_else`.
- "a second view of the same session detaches every terminal of the first" and "a displaced view discovers the winner's
  new tab without attaching it, until take control": superseded, as one UI holds the helm at a time and a displaced
  window shows no terminal (decided by the migration's orchestrator on 2026-11-15, as above); the helm's half, a second
  lease taking over both terminals of one session only, by crates/farhelm/tests/e2e/tab_lifecycle_edges.rs
  `a_second_lease_takes_over_both_terminals_of_one_session_only`.
- "restarting the agent rebuilds only the agent island; a tab keeps its socket" by
  `a_restart_does_not_hold_a_tabs_reconnect_card` and e2e terminal_tabs.rs
  `restarting_the_agent_leaves_a_tab_attached_running_and_unswept`.
- "retargeting an open tab-close question focuses the new answer" by
  `a_second_tabs_close_takes_over_the_question_and_enter_closes_that_tab` (the second ×'s own question, focus on its
  confirm, and one DELETE for that tab on Enter); a press outside an anchored question closes it unanswered by
  crates/gui/farhelm-gui/src/prompt_tests.rs `an_anchored_question_sits_below_its_anchor_and_closes_like_a_menu`, and a
  question starts on Confirm by `closing_a_tab_asks_first_then_sends_one_delete`.
- "cancel and confirm in the same tick close nothing" by prompt_tests.rs `a_confirmation_acts_only_on_what_it_showed`
  (Cancel then Confirm sends nothing, in the anchored presentation the tab close uses).
- "a failed close surfaces on that tab and leaves it attached" by
  `a_refused_close_leaves_its_tab_listed_attached_and_live` (the refusal on its tab, the tab still listed and attached,
  typing reaching it, and its × working again); the old case's other half, a later close of the other tab leaving the
  refusal shown, in part by `skipped_questions_close_at_once_side_by_side_and_a_refusal_shows_on_its_tab` (the sibling's
  close succeeding alongside the refused one).
- "terminal text size steps every terminal, reaches the pty, and is remembered" by
  `a_hidden_tab_is_resized_with_the_shown_terminal`, crates/gui/farhelm-gui/src/terminal/attached/tests.rs
  `a_text_size_step_resizes_every_open_terminal`, scripts/gui-e2e-smoke.py
  `test_73_a_text_size_step_resizes_the_programs_terminal` and crates/gui/farhelm-gui/src/session_pane/header_tests.rs
  `the_text_size_buttons_step_the_shared_size_and_the_readout_follows`.
- "terminal text size readout survives switching sessions with fixed button positions" by header_tests.rs
  `the_text_size_buttons_keep_their_places_as_the_readout_and_the_session_change` (the buttons' positions) and
  `a_terminal_opens_at_the_current_text_size` (the size, a shared preference).
- "terminal text size uses Cmd, not Ctrl, on a Mac" by crates/gui/farhelm-gui/src/actions_tests.rs
  `every_physical_row_answers_its_own_chord_on_both_platforms`.
- "a tab opened after closing another starts with a clean island" by
  `every_tab_attaches_under_the_views_lease_at_the_shown_terminals_size` (a tab attaches at the shown terminal's size,
  so the attach-time resize that tore the old snapshot does not happen); the snapshot itself is the supervisor's.
- "a tab opened and closed before any poll observes it does not come back" and "a tab closed elsewhere before this view
  ever listed it stops being shown" by `tabs_opened_and_closed_elsewhere_come_and_go` and
  `new_terminal_opens_a_tab_and_shows_it` (the reply and the push arriving in either order): the strip is the helm's
  pushed list, with no optimistic entries or polls.
- "a confirm prompt renumbers with the strip while still targeting its own tab" by
  `a_close_question_closes_its_own_tab_after_the_strip_renumbers` (one DELETE, for its own tab, after a renumber); its
  name is the deviation above.
- "a keyboard-focused selected tab keeps its resting accent even while hovered" by
  crates/gui/farhelm-gui/src/components/tests/button.rs
  `the_shown_terminal_tab_keeps_its_resting_accent_under_keyboard_focus`.
- "opening the close confirmation refits the terminals below it": superseded (decided by the migration's orchestrator on
  2026-11-16): the question is an anchored popover over the pane and takes no layout room, so nothing below it resizes.
- "a failing detail read leaves the tabs and their terminals alone" and "a session the helm stops listing is reported as
  stale, not torn down": superseded (decided by the migration's orchestrator on 2026-11-16): the app reads no session
  detail route, as the UI channel pushes the rows; an unreachable host's terminals by
  `an_inert_set_makes_every_terminal_and_the_strip_inert`.

### `tests/terminal-unicode.spec.ts`

Tests: 2. Covers: a narrowed text-presentation glyph owns one grid cell.

replaced: the narrowed glyph's one cell by crates/gui/farhelm-gui/src/terminal/tests.rs
`the_cache_keeps_wire_columns_and_the_placeholder_hides_the_replay` and
crates/farhelm-terminal-emulator/tests/snapshot.rs `selector_changed_widths_round_trip_and_continue_under_current_rules`
(columns only: the goldens bundle no fallback font, so its pixels are not checked); the reconnect by
crates/farhelm-supervisor/src/grid_output/tests.rs `a_vs16_emoji_replays_two_cells_wide_with_the_cursor_after_it`

### `tests/mouse-modes.spec.ts`

Tests: 3. Covers: mouse-modes-restored-on-reattach.

replaced: `mouse-modes-restored-on-reattach`'s client half by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`mouse_reporting_survives_a_reattach` (the replay carrying the mode is the supervisor's); the two drag-copy notice tests
by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_plain_drag_under_mouse_reporting_raises_the_notice_after_the_grace_window`,
`no_notice_after_a_forced_drag_a_click_or_a_drag_without_mouse_reporting`,
`a_program_clipboard_write_in_the_grace_window_suppresses_the_notice` (the copied drag),
`the_drawn_notice_sits_three_terminal_rows_from_its_release`,
`the_notices_x_puts_it_away_keeping_focus_and_sending_nothing`,
`a_press_on_the_notice_reaches_the_terminal_and_puts_it_away`, `the_notice_goes_after_thirty_seconds` and
`a_codex_sessions_notice_names_codexs_own_copy_key`; the hidden-tab case is a deviation below (a notice whose terminal
is hidden is closed for good)

- The drag-copy notice ×'s hover help, in "drag-copy notice: shown only for a drag the program kept without copying", by
  `the_notices_x_is_a_named_button_with_its_hover_help`.

### `tests/ime-composition.spec.ts`

Tests: 8. Covers: overlap boundary leaves the next key ordinary (${order}, ${keyCode}).

superseded: none of its cases touches the terminal. Each replays a browser engine's composition event order into a field
elsewhere (the launcher's template name, the rename dialog, the model combobox, a window shortcut), which is the DOM's
composition guard that item E6 of docs/gpui/browser-behaviors.md (at commit b97cd023, since removed) records only and
the GPUI app does not build. The terminal's committed-text path is covered by
crates/gui/farhelm-gui/src/terminal/attached/tests.rs `text_an_input_method_commits_reaches_the_program`,
`a_composition_reaches_the_program_only_when_it_commits` and `a_dead_key_composes_with_the_next_key`,
crates/gui/farhelm-input-mapping/src/tests.rs `composition_and_app_routing_have_one_owner`, and scripts/gui-e2e-smoke.py
`test_31_text_an_input_method_commits_reaches_the_program`. Input-method gaps, the same as the old grid client's (its
`compositionend` handler at the reference commit says showing the composition at the cursor was left out as not worth
its complexity): a composition in progress is not drawn in the terminal, only in the input method's own window if it
shows one; and the terminal tells the input method neither where its cursor is nor which character a point is over
(`bounds_for_range` and `character_index_for_point` in `terminal/attached.rs` answer nothing), so the candidate window
sits where the operating system puts it, not at the cursor. Input-method support stays at what GPUI gives, decided by
the maintainer on 2026-11-12, so these are recorded, not built.

### `tests/ui-plan-regressions.spec.ts` (terminal)

Case: "a newly discovered tab waits for intent under build skew". Only the terminal's case is listed here; the spec's
entry sits in docs/gpui/parity/sidebar-and-session-view.md.

superseded: the desktop app embeds its helm, so the window and the helm are always one build and there is no skew to
wait under (decided by the terminal area's owner on 2026-11-15, as for terminal-attachments' skew cases).

## Scheduled for M3

- The Linux primary selection: a selection setting it, a program's OSC 52 write to it, and middle-click pasting it. The
  migration's orchestrator moved these from M2 to M3 on 2026-11-14 (the Linux app has no users yet, and M2 is the Mac
  daily loop). A program's OSC 52 write to the ordinary clipboard is honored now.

## Deviations from the old app

Where the port does not match the old app, and why. Each stays until the work named in it lands, or says it is meant to
stay.

- A program's hyperlink's destination display shortens the link's text to the old budget before escaping invisible
  characters, so a text padded with them can draw longer than the old cap; and the text is not forced left to right (the
  old `bidi-override`): GPUI orders right-to-left letters within it, isolated from the app's own words. While a display
  shows, Escape pressed with focus outside the terminal closes it instead of reaching the control (the layer manager's
  Escape rule); it stays closed until the pointer moves to another cell. Escape from a focused terminal still reaches
  the program.
- A refused click on a program's hyperlink explains itself on the terminal's top line at once, where the old line faded
  in over 200 ms (not ported yet, like the scrollbar's fade). The destination display sits flush inside the window's
  content area when clamped (inside the frame and title bar around it, on Linux when the window draws its own frame),
  where the old one kept 4 pixels from the page's edge: the layer manager's shared clamp. A click counts the display as
  fully visible only inside that area.
- The tunnel warning has no busy state: Allow closes it, then the consent save and the tunnel's opening run, and a
  refused save shows in the tunnel notice, where the old warning stayed up, disabled, reading "Saving…", until the save
  answered. The prompt closes every question before its answer runs, unless the answer is an inline one's extra answer.
  Its header has no tunnel glyph yet: the app's icon set has none until the tunnel pill brings one. The old warning's
  third paragraph, about links from other `127.0.0.1` pages into the standalone helm, is left out for good: it was about
  the browser interface, which is gone.
- Every tunnel failure, opening included, shows in the one tunnel notice, under the top row or inside the open list,
  where the old app showed an opening's failure in a separate notice at the window's bottom right. SPEC.md places read
  and close failures below the top row and says nothing of opening's; one place for all of them is meant to stay.
- The tunnel notice takes the pointer over the terminal's top rows while it shows: text under it is not hovered,
  selected or scrolled through it, and a link under it opens nothing. The old app treated its notice as covering the
  links under it too; meant to stay.
- A session on the local host reads its tunnels only the first time the pointer rests on a loopback address on a Mac, so
  that hover's display gains its tunnel hint a moment later, once the read answers; a session on a remote host reads
  them as it opens, for its pill. The old app read every session's.
- The pill's listing follows this terminal's own tunnel commands and changes of the shared consent, not every change the
  helm makes: the old app read it again on every event the helm sent. A tunnel the helm opens for an agent's approved
  link ("Tunnel and open", the approvals area's) is read again once that answer is sent
  (`with_consent_the_answer_goes_at_once_and_the_tunnels_are_read_again`).
- With several tunnels the pill's hover help and accessible name give their count ("2 open tunnels from this Mac"), as
  the old pill's did; SPEC.md's "full mapping" applies to the one-tunnel pill.
- With **Color terminal frames by host** on, a session whose host the registry does not list gets no frame, where the
  old app drew one in the ordinary text color. SPEC.md names only listed hosts' colors; a registry that has not been
  read yet would otherwise draw a frame that changes color a moment later.
- The scrollback's scrollbar appears and disappears at once, where the old one faded in over 100 ms and out over 800 ms.
  The fade is not ported yet; the animation clock now tells a control whether motion is reduced
  (`ticker::reduced_motion`), so nothing blocks it. It still hides 800 ms after the last scroll or after the pointer
  leaves the terminal, as the old one finished fading then.
- Remote-localhost tunnels (the Cmd-click offer, the tunnel pill and the consent switch) are the Mac app's only for now
  (the maintainer's decision, 2026-11-15). Porting them to the Linux app is the TODO.md "Near term" entry "Tunnels from
  remote sessions on Linux".
- Linux has no copy chord in the terminal (the old app had no Linux window, and SPEC.md names none); every completed
  selection is copied as it is made, as on every platform. A right click selects nothing: the old app's right-click word
  selection fed its browser context menu's Copy, which the desktop app does not have, and the migration does not port
  it.
- The scrollbar shows hover help ("Drag to scroll back through the terminal's history"), which the old one did not: the
  GPUI component layer gives every control a label, hover help and a role. Meant to stay.
- The drag-copy notice does not open after a key press or a click anywhere in the window during its grace window: the
  layer manager refuses an open once the user has moved on since the release, where the old one showed regardless. Nor
  does it open while its terminal's link destination display is open as the grace window ends (chosen in the notice's
  code review on 2026-11-16). The old app drew the display above the notice; here both are popovers in stack order, so
  the notice would cover the display, and the display's cover check, to be a hit test, would not see a popover that lets
  the pointer through. A notice whose terminal is hidden (another tab chosen) is closed and does not come back with the
  terminal, where the old one, hidden with its pane, did. A notice taller than the window is cut at the window's content
  area's height less 8 pixels where the old one scrolled. (`attached.rs`, "The drag-copy notice".)
- The block elements (U+2580 to U+259F: full, half, eighth and quadrant blocks, and the three shades) are drawn as
  filled rectangles of their cell, not from the font as in the old app, so stacked blocks meet at every text size; the
  font's block glyph is shorter than the cell at most sizes, which left a stripe of background under every row. The
  shades are flat fills at a quarter, half and three quarters of the foreground's opacity rather than a dot pattern. Box
  drawing (U+2500 to U+257F) stays a font glyph. Decided by the maintainer on 2026-11-14. (`terminal/blocks.rs`.)
- Beside a tunnel pill, the tab strip keeps the old 112-pixel floor only as far as the top row has room: when the width
  the pill needs to stay whole and the actions leave less, the strip gets what is left and scrolls. At 650 pixels,
  beside one tunnel's pill, it comes out below 112 pixels for both environment and remote sessions. While a Delete runs,
  the row has no actions to fit and the strip has no floor at all beside a pill, where the old rule still applied it.
  Decided with the session view's owner on 2026-11-16; meant to stay. The rule and why are in the module docs of
  `crates/gui/farhelm-gui/src/session_pane/header/fit.rs`.
- The text size control (zoom out, the size readout, zoom in) is exactly as tall as the session header's buttons beside
  it, in the ordinary look and in the condensed one beside a tunnel pill, where the old control was shorter than them.
  Its magnifier buttons take the header buttons' compact geometry. Decided by the maintainer on 2026-11-16, who counts
  the old height as a bug; meant to stay. (`terminal/text_size.rs`.)
- A first attach whose socket opened and then sent no grid message within the catch-up's 5 s idle bound goes onto the
  reconnect ladder as a failed attempt, where the old app revealed an empty, live terminal. The helm client announces an
  attachment only with its first output, so there is nothing to reveal and no attachment for typing to reach; a
  reconnect that sends nothing already went onto the ladder in the old app. What it costs: in the old app, output that
  came after the 5 s still filled the revealed terminal, while here a first attach whose first grid message takes longer
  than that is dropped and retried, so a host that slow on every attempt never shows and keeps climbing the ladder. That
  is unlikely, since the supervisor's own wait for the attach's size is at most 1 s. (`farhelm-helm-client`'s
  `terminal.rs`, "The catch-up's idle bound".)
- The helm client pings the helm every interval whatever the output, where the old heartbeat stayed idle while output
  flowed. A ping is a control frame the helm answers and the program never sees, so no user can see the difference.
  Decided by the migration's orchestrator on 2026-11-16; meant to stay. (`farhelm-helm-client`'s `terminal.rs`.)
