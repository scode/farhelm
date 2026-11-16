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
`a_relabel_withdraws_the_click_until_checked_again`. Not ported, by SPEC.md's own decision: a change to only an adjacent
row of a hovered link judged again (the warning "need not follow later redraws of other rows while the pointer rests",
"Terminal experience"). Superseded: a pointer move between a relabel and its paint (the browser's frame timing; the GPUI
view looks the hover up again as the output applies). Its attachment cases by
crates/gui/farhelm-gui/src/terminal/attachments/tests.rs: the paste through the attachment rules (a copied file winning
over the text the clipboard also carries, and its host path typed as a paste) by
`a_pasted_file_is_uploaded_and_its_host_path_typed`, and the drop that uploads the file and types its path by
`two_dropped_files_are_typed_as_two_words_in_order` (a native drop navigates nowhere, so that half needs no test).
Pending: the rest of the spec; refusing a link or tunnel click whose display another Farhelm surface covers (it waits
for the layer manager to say what covers a popover)

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

partly: typing by crates/gui/farhelm-gui/src/terminal/attached/tests.rs `program_keys_are_sent_to_the_live_attachment`,
`text_an_input_method_commits_reaches_the_program`, `a_dead_key_composes_with_the_next_key`; focus reporting by
`a_program_asking_for_focus_reports_hears_focus_change`, `a_program_that_starts_asking_hears_at_once_without_a_frame`;
the alternate screen's wheel by `the_wheel_goes_to_the_program_on_the_alternate_screen`; scrollback by
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
rounds to). Waiting: links.

### `tests/grid-flood.spec.ts`

Tests: 1. Covers: a flood's scrollback arrives whole and in order, through a viewer that stops reading.

not yet ported

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
`test_71_a_reopened_terminal_replays_its_screen_and_takes_typing`, and a resize reaching the program's terminal (its
"resize reaches the real terminal", triggered by a text size step rather than a window resize, which nothing checks end
to end yet) by `test_72_a_text_size_step_resizes_the_programs_terminal`. Waiting: the rest of the spec.

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
screen across a reconnect, and leaving mid-upload and reopening). The strip's look has no captured scene yet: new scenes
are app scenes, not committed goldens (the maintainer's decision, 2026-11-15), and an app scene would have to paste or
drop a file into the running app, which the bridge has no command for (headless Linux gives the app no clipboard
either), so it waits for such a command. A terminal that goes away hands its lines to the session's agent terminal, so
after a reset, or once a tab closes, the failures still on a tab's strip show in the agent terminal, not in the tab. The
zero-byte file and the file name full of URL syntax are the helm client's:
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
has no default action for a drop, so nothing can open the file; the text-field cases went with browser behavior H2);
"leaving mid-upload and reopening leaves exactly one set of hooks" (no DOM listeners to stack; its upload half is
above); and "a filename and a server error carrying markup render as literal text" (the desktop app draws no markup; the
strip draws host values through the escaping host-text element)

### `tests/terminal-clipboard.spec.ts`

Tests: 8. Covers: an OSC 52 write from the terminal's own process reaches the system clipboard, byte for byte.

not yet ported

### `tests/terminal-files.spec.ts`

Tests: 6. Covers: changed OSC display text starts a fresh hover at the same cells.

partly: the hover and its refusals by crates/gui/farhelm-gui/src/terminal/files/tests.rs: "changed OSC display text
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
path disclose and download the whole file" by `both_halves_of_a_cut_path_check_and_save_the_whole_file`. Not ported
here: the forbidden OSC schemes, which are the web links' rule

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
pending hover and updates its newest link" is not pinned on its own (a redraw keeps a hover by the same link comparison,
whatever the check's state); "different paths, cell endpoints and unknown ranges start fresh checks", "changed and
unknown OSC display text cancels old evidence at the same target and cells", "partial wrapped redraw invalidates file
transfer evidence" by `a_relabel_withdraws_the_click_until_checked_again` and
`a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes`; the admission half of "a wrapped or
program-broken printed path cannot grow beyond its cached endpoint" by
`ready_for_requires_the_same_cells_file_and_label` (another end cell is not admitted); "file transfers refuse gestures
the terminal did not admit as clicks" by `a_drag_across_a_checked_path_is_never_admitted`. Superseded by the GPUI view,
which finds its hovered link again synchronously on every redraw and draws the pointer from it, with no deferred leave
and no cursor setters: "ready redraw cursor restoration waits for accessors and rejects retired links" and "leave
withdraws admission immediately and deferred teardown leaves another cursor alone"; the pointing hand that waits for a
ready check is pinned by `the_pointing_hand_waits_for_a_ready_check_on_a_printed_path`, through the predicate the
terminal draws it from. "disposal clears pending hover teardown and rejects late replies": dropping a terminal drops its
check's task, which is not tested on its own. The downloads by crates/gui/farhelm-gui/src/terminal/files/tests.rs:
"modifier opening is native-only and save outcomes survive opener refusal" and "a native modified click saves without
opening when hover offered only download" by `the_modifier_click_opens_only_what_the_helm_offered` (native only: the
desktop app's helm saves, so there is no browser half); "browser and native successes fade, while a failure waits for a
terminal click" and "replacement cancels an earlier notice deadline and retires a failure" by
`a_success_fades_and_a_failure_stays_until_the_next_press_or_download` (deviation: the success line vanishes at five
seconds rather than fading out over the old 200 ms, which the ticker's ten ticks a second would draw as two frames), and
a late outcome replacing a refusal, as the old message did, by `a_save_failing_behind_a_refusal_shows_its_failure` and
`a_refusal_takes_the_status_lines_place`. "terminal disposal clears notices and rejects a late native save result": by
`a_save_outlives_the_hover_but_not_the_terminal`. Waiting for the layer manager to say what covers a popover: "the cover
check measures every rendered marked surface and nothing else" and the covered half of "live approval coverage".
Superseded (no browser or DOM): the bounded Blob (the helm saves the desktop app's downloads), the browser timers'
receiver, and construction without a terminal element. Deviation: the old path link was one span, so a cut path's head
padding and tail indentation were underlined and hoverable; here only the path's own cells on each row are (pinned by
`a_path_cut_near_the_right_edge_joins_through_a_soft_wrap_and_a_program_break`)

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

partly: the notice's cases by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_plain_drag_under_mouse_reporting_raises_the_notice_after_the_grace_window`,
`no_notice_after_a_forced_drag_a_click_or_a_drag_without_mouse_reporting`,
`a_program_clipboard_write_in_the_grace_window_suppresses_the_notice`, `only_codex_has_its_own_copy_instruction` (the
text and the platform's key), `the_notice_sits_three_rows_from_the_release_with_its_fallbacks` (the five placement
cases, the row height and the clamp to the window's content area), `the_notice_goes_after_thirty_seconds` and
`the_notices_x_puts_it_away_keeping_focus_and_sending_nothing`; the copy decisions by the selection tests there
(`a_drag_selects_and_copies_and_a_click_does_not`, `under_mouse_reporting_only_the_forcing_modifier_selects`). Not
ported: a press on the scrollbar (no case yet), the narrowed viewport's remeasuring (GPUI lays the popover out on its
own, not at its last place), a retained selection under a plain tracked press (a reported press clears the selection
here), and the browser-global and listener-disposal cases, which have no counterpart

### `tests/terminal-flood.spec.ts`

Tests: 3. Covers: a terminal socket for an unknown session reports why.

partly: the paste of over a megabyte by `terminal/attached/tests.rs` `a_large_paste_arrives_whole_and_in_order` and
scripts/gui-e2e-smoke.py `test_30_a_large_paste_arrives_whole_inside_its_brackets` (macOS only; skipped on headless
Linux, which gives the app no clipboard); the unknown-session error and Backspace and Ctrl-C not yet ported

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
over. Font loading (the spec's font-gate cases) has no counterpart: the desktop app embeds its font

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
crates/gui/farhelm-grid-model/tests/links.rs `pinned_cell_links_and_label_join_oracle`. Pending: refusing a click whose
display another Farhelm surface covers (a hover tooltip already draws above popovers; approval cards will), which waits
for a way to ask the layer manager what covers a popover

### `tests/terminal-multihost.spec.ts`

Tests: 55. Covers: multi-host.

partly: none of its cases is the terminal's own; the spec is about hosts. Its three stale-session cases (a supervisor
outage keeping the screen behind one calm card and reattaching promptly) are the sidebar and session view area's, ported
with the unavailable card (docs/gpui/parity/sidebar-and-session-view.md, its `terminal-multihost.spec.ts` (stale)
entry). Its host Uninstall, Retry and Adopt cases are settings-and-hosts' (docs/gpui/parity/settings-and-hosts.md, its
Uninstall and Retry and Adopt sections); the rest (the host list, menus and settings, creating and cloning across hosts)
are not the terminal's either, and this entry tracks none of them

### `tests/terminal-reconnect.spec.ts`

Tests: 24. Covers: socket-killed-reconnects-and-lands-at-tail.

partly: `manual-reconnect-is-offered-during-the-first-rung`'s card and button: a dropped stream shows the calm reconnect
card ("Session unavailable — reconnecting", its explanation and a neutral Reconnect now with the old hover help) over
the kept screen, with no red band and without taking focus, by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_dropped_stream_shows_the_reconnect_card_over_the_kept_screen`; one press sends one reconnect-now request by
`reconnect_now_sends_one_request_per_press`, and the keyboard reaches and presses it by
`reconnect_now_is_reached_and_pressed_from_the_keyboard`; the card stays through the reconnect's catch-up and goes when
it is live by `the_card_stays_through_the_reconnects_catch_up_and_goes_when_it_is_live`; stalls, refusals and closes
keep the red band by `detached_refused_and_closed_streams_keep_the_red_band`, while a protocol fault in the reconnect's
catch-up keeps the card and restarts the stream by `a_fault_during_a_reconnects_catch_up_restarts_it`; the card's look,
and its cut-short scrolling pane, by no golden since the repository stopped taking new ones (decided by the migration's
orchestrator on 2026-11-15), and by no app scene yet, since the screenshot batch cannot drop a terminal's stream. A
Restart reattaching without the card or a red band, held for the whole recovery the Restart began because the helm
answers it before the reattach, by crates/gui/farhelm-gui/src/session_pane/tests.rs
`a_restart_reattaches_without_a_card_or_a_red_band`, and the card once an attempt fails, or the reattach drops before it
goes live, by `a_restarts_failed_reattach_shows_the_card` and `a_restarts_reattach_that_drops_mid_replay_shows_the_card`
(decided by the migration's orchestrator on 2026-11-15). A stuck Restart stays silent for at most about a minute: the
first attempt's socket must open within the stream's 30 s watchdog, and an opened one that never attaches is ended by
the same watchdog, after which the second attempt shows the card. The bound does not cover a reattach that attaches and
then stalls in its replay while its link keeps answering pings: that one stays hidden until it fails, drops or
completes. Tab terminals are not held, since a Restart detaches no tab, by
crates/gui/farhelm-gui/src/terminal/set/tests.rs `a_restart_does_not_hold_a_tabs_reconnect_card`; an inert terminal
draws no card by crates/gui/farhelm-gui/src/terminal/attached/tests.rs `an_inert_terminal_draws_no_reconnect_card`, and
the card keeps presses from the screen behind it by
`a_drag_on_the_card_reaches_nothing_behind_it_and_the_scrim_lets_presses_through`. The card's phase and attempt line,
from what the fake stream reports: the old card's four templates word for word, with the scheduled wait rounded as the
old card rounded it, by crates/gui/farhelm-gui/src/terminal/attached/tests.rs `the_phase_line_says_the_old_cards_words`
(the wait is not counted down, as there, because the line is redrawn only on the stream's reports; no test pins that);
`retry-exhaustion-shows-reprobe-phase`'s probing phase ("Retrying every", the attempt count past six, Reconnect now
still drawn), reached through every rung of the ladder, by `the_phase_line_follows_the_ladder_into_probing`;
`manual-reconnect-is-offered-during-the-first-rung`'s "attempt 1 of 6" and its press starting that attempt, which the
line keeps through the catch-up, by `reconnect_now_in_the_first_wait_shows_attempt_1_under_way`; the line's look by no
golden either. Waiting: PageUp scrolling the cut-short card from its focused button; retry exhaustion against a real
helm (failing real handshakes rather than the fake's); and the rest of the spec (wedged sockets, version skew). Its
takeovers during backoff no longer apply: one UI holds the helm at a time, and a displaced window shows no terminal
(decided by the migration's orchestrator on 2026-11-15)

### `tests/terminal-switch-geometry.spec.ts`

Tests: 1. Covers: session switching keeps full-width text and rules still at ${font}px.

replaced: crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_fresh_owner_on_the_same_pane_asks_for_the_last_size_and_resizes_nothing` (at 13 points, after a resize history)

### `tests/terminal-tabs.spec.ts`

Tests: 36. Covers: a tab runs a real shell in the session's working directory, leaving the agent terminal alone.

partly: crates/gui/farhelm-gui/src/terminal/set/tests.rs
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
when it opened rather than renumbering with the strip (it acts on the tab's id). Pending: the spec's retargeting and
same-tick cancel-and-confirm cases for the tab question; the browser-only mount cases (a pending mount cancelled, a
failed mount rolled back) have no counterpart, as the desktop app mounts no islands

### `tests/terminal-unicode.spec.ts`

Tests: 2. Covers: a narrowed text-presentation glyph owns one grid cell.

replaced: the narrowed glyph's one cell by crates/gui/farhelm-gui/src/terminal/tests.rs
`the_cache_keeps_wire_columns_and_the_placeholder_hides_the_replay` and
crates/farhelm-terminal-emulator/tests/snapshot.rs `selector_changed_widths_round_trip_and_continue_under_current_rules`
(columns only: the goldens bundle no fallback font, so its pixels are not checked); the reconnect by
crates/farhelm-supervisor/src/grid_output/tests.rs `a_vs16_emoji_replays_two_cells_wide_with_the_cursor_after_it`

### `tests/mouse-modes.spec.ts`

Tests: 3. Covers: mouse-modes-restored-on-reattach.

partly: `mouse-modes-restored-on-reattach`'s client half by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`mouse_reporting_survives_a_reattach` (the replay carrying the mode is the supervisor's); the two drag-copy notice tests
by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`a_plain_drag_under_mouse_reporting_raises_the_notice_after_the_grace_window`,
`no_notice_after_a_forced_drag_a_click_or_a_drag_without_mouse_reporting`,
`a_program_clipboard_write_in_the_grace_window_suppresses_the_notice` (the copied drag),
`the_drawn_notice_sits_three_terminal_rows_from_its_release`,
`the_notices_x_puts_it_away_keeping_focus_and_sending_nothing`,
`a_press_on_the_notice_reaches_the_terminal_and_puts_it_away`, `the_notice_goes_after_thirty_seconds` and
`a_codex_sessions_notice_names_codexs_own_copy_key`; the ×'s hover help and the hidden-tab case are not covered (a
notice whose terminal is hidden is closed for good, a deviation below)

### `tests/ime-composition.spec.ts`

Tests: 8. Covers: overlap boundary leaves the next key ordinary (${order}, ${keyCode}).

superseded: none of its cases touches the terminal. Each replays a browser engine's composition event order into a field
elsewhere (the launcher's template name, the rename dialog, the model combobox, a window shortcut), which is the DOM's
composition guard that docs/gpui/browser-behaviors.md E6 records only and the GPUI app does not build. The terminal's
committed-text path is covered by crates/gui/farhelm-gui/src/terminal/attached/tests.rs
`text_an_input_method_commits_reaches_the_program`, `a_composition_reaches_the_program_only_when_it_commits` and
`a_dead_key_composes_with_the_next_key`, crates/gui/farhelm-input-mapping/src/tests.rs
`composition_and_app_routing_have_one_owner`, and scripts/gui-e2e-smoke.py
`test_31_text_an_input_method_commits_reaches_the_program`. Input-method gaps, the same as the old grid client's (its
`compositionend` handler at the reference commit says showing the composition at the cursor was left out as not worth
its complexity): a composition in progress is not drawn in the terminal, only in the input method's own window if it
shows one; and the terminal tells the input method neither where its cursor is nor which character a point is over
(`bounds_for_range` and `character_index_for_point` in `terminal/attached.rs` answer nothing), so the candidate window
sits where the operating system puts it, not at the cursor. Input-method support stays at what GPUI gives, decided by
the maintainer on 2026-11-12, so these are recorded, not built.

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
  edge when clamped, where the old one kept 4 pixels from it: the layer manager's shared clamp.
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
  link ("Tunnel and open", which the approvals area ports) will not show in the pill until the next read; that port adds
  its own read.
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
