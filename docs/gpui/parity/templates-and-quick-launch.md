# Parity: Templates and quick launch

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Creation (the Templates dialog, quick launch, number keys, launcher sequences and the Cmd+J template list).

### `tests/templates.spec.ts`

Tests: 48. Covers: template bolts use a fixed outline or filled drawing for their mark.

partly, with some cases or parts of them not yet ported and some superseded (both after the bullets): the list,
summaries, ⚡ and number keys (`templates::dialog_tests`, `templates::shortcuts_tests`, `templates::summary_tests`); the
editor, its fields, Save, rename and Duplicate (`templates::editor_tests`, `templates::fields_tests`); leaving and
deleting (`templates::leave_tests`); Save as template from New and Clone, its checklist's defaults, Enter, Escape, the
name refusals, a refused save and the saved template opening in the editor (`templates::save_tests`), focus back in the
search box after Escape or Cancel and left where the user put it by a refusal
(`escape_or_cancel_puts_focus_back_in_the_search_box`, `a_refused_save_leaves_focus_where_the_user_put_it`), its GitHub
repository row (`a_chosen_repository_is_offered_checked_and_the_folder_is_not`), and a template's repository destination
choosing that repository (`templates::apply_tests::a_repository_destination_chooses_that_repository`); `tl:` in the
launcher's search: rows only under `tl:`, by a case-insensitive part of the name, the exact name preselected, between
Harnesses and Models (`launcher::search_tests::templates_are_offered_only_under_tl`,
`the_exact_template_name_is_preselected_by_unicode_lowercase`,
`template_rows_sit_between_harnesses_and_models_in_the_old_words`), a taken row applying its template and keeping the
launch kind (`launcher::search_view_tests::a_tl_row_applies_its_template_and_keeps_the_launch_kind`), a refused one
changing nothing and saying why above the box (`a_refused_tl_row_changes_nothing_and_says_why_above_the_box`), and a
failed read as the reason in the results (`launcher::search_tests::unavailable_templates_are_the_reason_under_tl`,
`launcher::search_view_tests::unavailable_templates_are_the_reason_in_the_results`); a ⚡ or number-key write refused
while the bindings cannot be read, the helm's preferences unavailable or not arrived
(`templates::dialog_tests::unreadable_bindings_refuse_a_key_or_mark_write`,
`bindings_that_never_arrived_refuse_a_key_write`); stacking several templates from `tl:` is the sequence's; read-only
resume commands and session names still refusing an unrelated save
(`templates::editor_tests::read_only_resume_and_session_name_still_refuse_an_unrelated_save`), an escaped folder and
prompt saved raw and a hidden model suggestion drawn escaped
(`escaped_folder_and_prompt_save_raw_and_a_hidden_suggestion_is_escaped`), the managed checkout's hint
(`the_name_kind_and_destination_kind_are_saved`), the quick-launch line
(`the_quick_launch_line_says_whether_the_template_holds_the_mark`); a repaired draft still deleting permanently
(`templates::leave_tests::a_repaired_draft_of_a_template_failing_save_rules_still_deletes_permanently`); "This template"
in the key menu, a refused key write, a second click closing the menu and a long summary inside its row
(`templates::dialog_tests::the_key_menu_names_the_templates_own_key_as_this_template`,
`a_refused_key_write_is_reported_and_the_stored_keys_stay`, `a_second_click_on_the_key_button_closes_its_menu`,
`a_long_summary_stays_inside_its_row`); the prompt's stacking, a resume command and an `effort: null` reset, an old
template's trust ignored and a retired container refused
(`templates::apply_tests::an_unrelated_template_leaves_a_dormant_prompt_and_an_empty_one_clears_it`,
`a_template_fills_the_resume_command_and_a_null_effort_resets_the_choice`,
`an_old_templates_workspace_trust_is_ignored_when_applied`,
`a_retired_container_template_is_refused_and_changes_nothing`); a recent setup keeping the typed prompt
(`launcher::draft_tests::a_recent_setup_keeps_the_typed_prompt`; a prompt cannot leak into the next Clone or Replace
with, since each opening builds a new draft from a session, which holds none); Launch held during Save as template's
save with focus kept on it (`templates::save_tests::launch_sends_nothing_while_the_save_is_out_and_keeps_its_focus`).

- `templates::dialog_tests::the_bolt_marks_and_clears_through_the_preferences` ("template bolts use a fixed outline or
  filled drawing for their mark", in its marking half; the drawing itself is checked only by the `templates-dialog`
  golden, which an ordinary test run skips)
- `templates::dialog_tests::a_refused_write_is_reported_and_nothing_changes` ("a refused quick-launch mark restores the
  stored mark", with the deviation "A refused ⚡ or number-key write says it was not saved")
- `templates::dialog_tests::an_ineligible_bolt_is_unavailable_unless_it_is_lit` ("model-only number key opens New with a
  temporary composition explanation", the list's half)
- `templates::dialog_tests::the_key_menu_offers_every_key_and_moves_the_chosen_one` ("number keys move, follow rename,
  and stay freed after undo")
- `templates::dialog_tests::escape_from_the_key_menu_returns_focus_to_its_button` (the same case)
- `templates::dialog_tests::closing_returns_focus_to_the_templates_button` ("unsaved departures support keep editing,
  discard and save before leaving", its closing)
- `templates::shortcuts_tests::a_rename_moves_only_the_old_names_key` ("number keys move, follow rename")
- `templates::shortcuts_tests::a_bolt_click_writes_what_the_drawn_bolt_meant` (the same case)
- `templates::shortcuts_tests::writes_patch_only_their_own_field` (the same case)
- `templates::shortcuts_tests::hover_help_names_the_behavior_and_the_choice` ("model-only number key opens New", the key
  button's hover help)
- `templates::editor_tests::save_writes_the_whole_template_once_and_keeps_focus` ("template Save follows unsaved changes
  and retains focus after saving"; also "the Templates dialog creates, edits, renames, deletes and undoes")
- `templates::editor_tests::an_escaped_name_saves_raw_until_replaced` ("template name escapes formatting until
  explicitly replaced")
- `templates::editor_tests::replace_name_clears_the_name_and_focuses_it` (the same case)
- `templates::editor_tests::an_escaped_field_keeps_its_raw_value_until_replaced` ("unsafe template model and folder stay
  raw until explicitly replaced")
- `templates::editor_tests::the_initial_prompt_keeps_its_line_breaks` ("initial prompt template field preserves
  multiline text and agent compatibility")
- `templates::editor_tests::a_field_that_no_longer_fits_stays_and_refuses_saving` (the same case)
- `templates::editor_tests::add_field_offers_unset_fields_and_escape_closes_only_its_menu` (the same case, and "model
  suggestions and phone departure controls remain reachable")
- `templates::editor_tests::new_template_opens_an_empty_draft` ("the Templates dialog creates, edits, renames, deletes
  and undoes")
- `templates::editor_tests::a_rename_with_nothing_bound_goes_straight_to_the_deletion` (the same case)
- `templates::editor_tests::save_refusals_come_in_the_old_order_and_send_nothing` ("long summaries stay reachable and
  taken names refuse save and undo")
- `templates::editor_tests::duplicate_opens_an_unsaved_copy_of_the_saved_template` ("session name kind survives
  duplicate and undo and base saves stay clean")
- `templates::editor_tests::saving_a_base_name_leaves_no_unsaved_changes` (the same case)
- `templates::editor_tests::a_kindless_template_opens_with_its_inferred_switch_unsaved` ("legacy switches and agent
  choices remain explicit at phone width")
- `templates::editor_tests::a_row_opens_its_template_pressed_with_focus_on_its_name` ("reselecting the current template
  preserves edits and renames", its first half: reopening the row keeps a typed name)
- `templates::editor_tests::a_new_name_is_refused_while_the_templates_are_unavailable` ("a phone draft can retry a
  failed template list without discarding", what is left of it)
- `templates::editor_tests::an_edit_while_saving_changes_nothing` ("templates busy root stays keyboard-scrollable", with
  the deviation "While a save is in flight")
- `templates::editor_tests::a_refused_save_is_reported_kept_and_never_retried` (the same case)
- `templates::fields_tests::new_and_renamed_names_must_be_free_and_unspaced` ("template name whitespace is refused only
  for a new name")
- `templates::fields_tests::the_open_name_always_saves_and_a_rename_must_be_free` (the same case)
- `templates::fields_tests::spaced_historical_names_keep_their_undo` (the same case)
- `templates::fields_tests::duplicate_names_are_free_unspaced_and_fit_the_limit` (the same case)
- `templates::fields_tests::duplicate_replaces_unicode_whitespace` (the same case)
- `templates::fields_tests::unsafe_text_makes_a_field_read_only` ("older template ${field} stays read-only without
  bypassing save refusal")
- `templates::fields_tests::a_retired_container_field_refuses_until_removed` ("retired container templates load, refuse
  application and allow explicit removal")
- `templates::fields_tests::removing_a_retired_container_field_makes_the_draft_saveable` (the same case)
- `templates::fields_tests::opening_infers_the_switch_and_drops_trust_in_the_draft` ("old template trust ${trust} is
  ignored on launch and dropped on ${write}")
- `templates::fields_tests::incompatible_fields_refuse_saving_but_any_model_text_fits` ("initial prompt template field
  preserves multiline text and agent compatibility")
- `templates::fields_tests::explicit_permissions_stay_saveable_and_partial_templates_offer_all` ("an approvals-only
  template saves every offered approval choice" and "templates save explicit guarded and default-YOLO permissions")
- `templates::fields_tests::a_session_name_owns_its_kind` ("session name kind survives duplicate and undo")
- `templates::fields_tests::opening_infers_the_switch_from_each_field_kind` ("legacy switches and agent choices remain
  explicit")
- `templates::fields_tests::changing_the_agent_flags_approvals_it_does_not_offer` (the same case)
- `templates::leave_tests::every_departure_from_an_unsaved_draft_asks_first` ("unsaved departures support keep editing,
  discard and save before leaving")
- `templates::leave_tests::discard_goes_where_the_user_was_heading` (the same case)
- `templates::leave_tests::save_leaves_only_once_the_save_succeeds` (the same case)
- `templates::leave_tests::keep_editing_by_pointer_returns_to_the_name_field_on_a_mac` (the same case; it runs on every
  platform)
- `templates::leave_tests::delete_takes_effect_at_once_and_undo_restores_the_loaded_copy` ("number keys ... stay freed
  after undo" and "the Templates dialog creates, edits, renames, deletes and undoes")
- `templates::leave_tests::a_template_failing_save_rules_is_deleted_permanently` ("older template ${field} deletion
  warns before permanently deleting")
- `templates::leave_tests::a_refused_delete_is_reported_and_keeps_the_editor` ("a refused template Delete keeps the helm
  error and stored template")
- `templates::leave_tests::undo_refuses_a_taken_name_and_keeps_a_spaced_one` ("taken names refuse save and undo")
- `templates::leave_tests::undo_lasts_ten_seconds_a_later_delete_replaces_it_and_closing_ends_it` ("undo expires
  independently of earlier deletions and ends on close")
- `templates::leave_tests::the_question_is_scrolled_into_view` ("model suggestions and phone departure controls remain
  reachable", the departure question's half)
- `templates::save_tests::a_spaced_or_taken_name_is_refused_without_a_write` ("save as template requires a name without
  whitespace" and "launcher template save refuses a taken name")
- `templates::save_tests::new_checks_its_choices_and_offers_passive_values_unchecked` ("save as template captures the
  ${kind} setup without launching"; the checkboxes' geometry is the deviation "Save as template's checklist values")
- `templates::save_tests::hand_edits_check_permissions_wrapper_host_folder_and_name` (the same case)
- `templates::save_tests::the_command_tab_offers_only_its_own_fields` (the same case)
- `templates::save_tests::enter_saves_the_checked_fields_and_opens_the_new_template` (the same case)
- `templates::save_tests::unchecking_a_field_and_choosing_fixed_change_what_is_saved` (the same case, and "initial
  prompt launcher template stacking and save selection")
- `templates::save_tests::the_initial_prompt_is_checked_by_its_first_line_where_it_can_be_sent` ("initial prompt
  launcher template stacking and save selection")
- `launcher::view_tests::agent_launch_text_holding_invisible_characters_shows_escaped` ("initial prompt launcher
  template stacking and save selection", its invisible-character half: a prompt holding U+202E and a line break shown
  escaped and read-only until cleared)
- `templates::save_tests::escape_closes_the_panel_and_keeps_the_launcher` ("Escape keeps the launcher", and "save as
  template handles body Escape before the launcher", with focus in the panel: Escape closes the panel, not New, and the
  next Escape closes New)
- `templates::save_tests::a_refused_save_is_reported_and_keeps_the_launcher` ("launcher template save refuses a taken
  name and Escape keeps the launcher")
- `templates::save_tests::escape_or_cancel_puts_focus_back_in_the_search_box` ("save as template handles body Escape
  before the launcher", with focus in the panel: Escape and Cancel each put focus back in the search box, which is not
  checked to be in view)
- `templates::save_tests::opening_the_panel_brings_its_name_field_into_view` (the same case, its reopening step)
- `templates::save_tests::replace_with_offers_no_save_as_template` ("Replace with accepts its own template host and
  refuses another host atomically")
- `templates::apply_tests::replace_with_drops_an_equal_host_and_refuses_another` (the same case)
- `templates::launcher_modes_tests::replace_with_refuses_a_host_change_whole_and_takes_a_later_press` (the same case)
- `templates::apply_tests::the_prompt_follows_the_shared_fold` ("initial prompt launcher template stacking")
- `templates::apply_tests::templates_stack_with_the_later_winning_where_they_overlap` ("tl: applies templates, stacks
  them, and refuses one that does not apply")
- `templates::apply_tests::a_refusal_anywhere_in_the_batch_leaves_the_draft_unchanged` (the same case)
- `templates::apply_tests::the_launch_kind_applies_before_the_agent_type` ("a template fills the command tab and the
  folder")
- `templates::dispatch_tests::a_partial_number_key_opens_new_with_the_prefix_banner_for_six_seconds` ("model-only number
  key opens New with a temporary composition explanation"; its held catalog read showing "Loading session setup…" is the
  deviations "A switcher template pick shows no loading state" and "Every quick-launch banner expires", which count the
  banner's seconds from when the templates applied)

Not yet ported:

- the approval-card half of "save as template refusal respects ${focusOwner} focus" (a refusal leaving focus on an
  approval card the user moved to while the save was out); its other halves are above, and its lost-focus half is
  superseded
- of "number keys move, follow rename, and stay freed after undo", a change to the bindings pushed while the dialog is
  open, drawn on its key button and kept by the next key write
  (`templates::dialog_tests::the_key_menu_offers_every_key_and_moves_the_chosen_one` builds its write from bindings
  pushed before the dialog opened)
- of "save as template handles body Escape before the launcher", Escape with focus outside the panel (in the launcher,
  for example after a click in its search box, the counterpart of the old page body) closing the panel and not New, and
  the search box back in view after Escape and after Cancel
- of "reselecting the current template preserves edits and renames", reopening the row after a save, and an edit to a
  field other than the name kept across reopening
- of "templates busy root stays keyboard-scrollable", scrolling the busy editor by keyboard (Home back to the top); the
  deviation "While a save is in flight" covers its Tab stop, not its scrolling
- of "model-only number key opens New with a temporary composition explanation", a click on the banner not counting as
  an edit, and a button choice (the Command tab) taking the banner away

Superseded: "save as template leaves composing Enter with the name editor" and "save as template keeps the draft on
composing Escape", by the terminal file's `tests/ime-composition.spec.ts` entry, which supersedes that spec whole: these
cases replay a browser engine's composition event order into a field, a DOM guard the GPUI app does not build, and
input-method support stays at what GPUI gives (the maintainer's decision of 2026-11-12). "delete waits for the saved
template list before offering undo", by the deviation "No list reload after a save": there is no reload to wait for. "a
phone draft can retry a failed template list without discarding", by the Superseded entries "The phone-width one-pane
layout" and "Retry beside a failed template list". The 390-pixel widths of "templates busy root stays
keyboard-scrollable" and "save as template handles body Escape before the launcher" and the phone halves of "session
name kind survives duplicate and undo", "legacy switches and agent choices remain explicit at phone width" and "model
suggestions and phone departure controls remain reachable", by the phone layout's entry. The composing cases, the Delete
wait and the phone draft's retry were settled on 2026-11-16 by the GPUI migration's coordinator.

### `tests/quick-launch.spec.ts`

Tests: 32. Covers: quick launch submits the wrapper prefix saved in its agent template.

partly, with parts of three cases not yet ported (after the bullets): the ⚡ chord's direct launch through New's
prefill, the catalog and the launcher's submit path, unseen, the mark kept to success and cleared by a reveal, a slow
one showing the launcher's progress pill until its reply
(`templates::dispatch_tests::the_chord_launches_the_marked_template_unseen`,
`a_revealed_quick_launch_is_no_longer_unseen`, `actions_tests::the_launch_chords_run_once_per_press_from_a_terminal`),
an incomplete mark opening New with its choices and the needs-more-choices banner, which an edit takes away
(`templates::dispatch_tests::an_incomplete_mark_opens_new_with_its_choices_and_banner`), a model-only template picking
its agent type and a model the agent type does not offer refused
(`a_model_only_template_picks_its_agent_type_from_the_catalog`, `a_model_the_agent_type_does_not_offer_is_refused`), a
refused mark opening New with the refusal (`a_refused_mark_opens_new_unchanged_with_the_refusal`), a draft the launcher
refuses shown with its reason, the offline-host case (`a_draft_the_launcher_refuses_is_shown_with_its_reason`), a
missing or unusable mark opening Templates (`a_missing_or_unusable_mark_opens_templates_with_its_hint`), a failed
template read refused (`unavailable_templates_refuse_a_press_on_the_page`), unreadable bindings refused for the ⚡
chord, the number keys and a sequence's keys, but not its words
(`unreadable_bindings_refuse_the_chord_and_number_keys_on_the_page`,
`bindings_that_never_arrived_refuse_the_chord_on_the_page`, `unreadable_bindings_refuse_a_press_in_the_launcher`,
`a_sequence_on_unreadable_bindings_refuses_its_keys_but_not_its_words`), a press consumed while a refused launch waits
behind its notice (`a_press_waits_while_a_refused_launch_waits_behind_its_notice`), presses during a quick launch's
preparation waiting (New, a second ⚡ and the sequence prefix) and its result after the user moved on offered by the
notice (`presses_during_a_preparation_wait_and_its_result_respects_the_moved_on_rule`), a reset dropping a preparation
(`a_reset_drops_a_preparation`), the sequence prefix refused while a launch is pending, as the old app refused it
(`the_chord_launches_the_marked_template_unseen`), other dialogs keeping the keyboard, for the ⚡ chord and the number
keys (`the_chord_and_number_keys_do_nothing_under_another_dialog`), a number key launching its template unseen
(`a_number_key_launches_its_template_unseen`), a held number key acting once
(`actions_tests::a_held_number_key_acts_once_and_a_held_text_size_chord_repeats`), an unbound or unusable number key
opening Templates with its hint (`templates::dispatch_tests::an_unbound_number_key_opens_templates_with_its_hint`), a
partial template's number key opening New with the prefix banner, which goes six seconds after the templates applied
(`a_partial_number_key_opens_new_with_the_prefix_banner_for_six_seconds`), and a sequence's Enter launching or opening
New with its banner or refusal (`a_sequences_enter_launches_unseen`, `a_sequence_that_cannot_launch_opens_new`); the
sequence's prefix, key capture, cancels and pill (`templates::armed_tests`,
`components::tests::sequence_pill::a_click_on_the_sequence_pill_reaches_what_it_covers`). A number key's refusal takes
the ⚡ chord's path and is tested there; the ordinary YOLO question is revealed over the template's draft when the helm
refuses a ⚡ or number-key launch for YOLO, and its answer retries with consent
(`templates::dispatch_tests::the_chords_yolo_refusal_reveals_the_ordinary_question_over_its_draft`,
`a_number_keys_yolo_refusal_reveals_the_ordinary_question_over_its_draft`). Delete waits for a ⚡ write in flight
(`templates::leave_tests::delete_waits_for_a_bolt_write_in_flight`). Naming after an unseen success: the editor over the
new session with the name selected, a typed name renamed, unchanged Enter and Escape sending nothing, focus to the new
terminal (`templates::naming_tests::an_unseen_quick_launch_opens_naming_and_a_typed_name_renames`,
`unchanged_enter_and_escape_keep_the_created_name`,
`a_byte_identical_save_keeps_the_name_and_a_one_character_change_renames`,
`cancel_and_close_return_to_the_new_terminal`), a session the listing leaves out named from the create's reply
(`naming_opens_for_a_session_the_listing_leaves_out`), the preference turning it off
(`naming_is_off_when_its_preference_is`), and none after a revealed launch or for a launch the user moved on from
(`a_launch_that_revealed_new_is_not_named`, `a_quick_launch_the_user_moved_on_from_is_not_named`). The quick switcher's
Quick launch row: its label and action
(`templates::rows_tests::the_quick_launch_row_names_the_mark_and_launches_or_opens_templates`,
`the_quick_launch_row_before_the_preferences_arrive_says_mark_a_template_and_refuses`,
`the_quick_launch_row_keeps_the_last_mark_and_refuses_while_the_bindings_are_unreadable`), and as the sidebar draws it
(`session_list::quick_switcher::tests::the_quick_launch_row_follows_the_matches_and_names_the_mark`,
`the_quick_launch_row_opens_templates_or_launches_the_mark`,
`the_quick_launch_row_leaves_the_hosts_panel_only_for_a_launch`). A quick launch waiting out of sight for the host's
last folder, and showing New when the host's recent setups cannot be read: the launcher's
(`launcher::folder_default_tests::a_quick_launch_waits_out_of_sight_for_the_hosts_last_folder`,
`a_quick_launch_without_the_hosts_history_shows_new`), and through a template's ⚡, waiting only when the template
leaves the folder alone (`templates::dispatch_tests::a_quick_launch_without_a_folder_starts_in_the_hosts_last_folder`,
`a_quick_launch_of_a_template_with_a_folder_reads_no_history`). Superseded (see below): the old dispatcher's fences and
the lost reply's retry. A deviation (see below): a quick launch while the host list is unavailable. Also: a template's
wrapper prefix through the ⚡ chord (`templates::dispatch_tests::the_chord_launches_a_templates_wrapper_prefix`), a
sequence of a partial number key and ⚡ launching the stacked setup
(`a_sequence_of_a_partial_key_and_the_mark_launches_the_stacked_setup`), and Escape and Cancel on a changed name in
naming (`templates::naming_tests::escape_and_cancel_on_a_changed_name_send_no_rename`).

- `templates::armed_tests::the_prefix_from_a_terminal_arms_a_sequence_that_takes_every_key` ("typed launch proceeds
  before the prefix catalog returns and owns terminal input"; the template read the old case held at the prefix has no
  counterpart, since the helm pushes the templates and nothing reads them then)
- `templates::armed_tests::bindings_are_copied_when_the_prefix_starts` ("a sequence honors a ${binding} changed after
  this window loaded")
- `templates::armed_tests::a_non_sequence_key_cancels_and_reaches_the_program` ("launch sequence cancellation leaves
  ordinary terminal input and modals in control")
- `templates::armed_tests::escape_cancels_and_never_reaches_the_program` (the same case)
- `templates::armed_tests::a_dialog_keeps_the_keyboard_from_the_prefix_and_an_armed_sequence` (the same case)
- `templates::armed_tests::the_prefix_with_no_session_open_centers_the_pill_on_the_main_area` (the same case's pill
  place; the Mac's lower pill is the deviation "The sequence pill's place follows the selected session", checked by
  reading the code, not by a test)
- `templates::armed_tests::the_launcher_pill_stays_above_an_open_sessions_details_bar` (the same case)
- `templates::armed_tests::a_text_field_keeps_the_prefix` ("launch prefix keeps text fields and resolves an unbound
  digit only at Enter")
- `templates::armed_tests::enter_with_an_unbound_key_opens_templates_with_its_hint` (the same case)
- `templates::armed_tests::the_pill_shows_chips_the_word_and_its_candidates` ("typed candidates stay bounded and
  pointer-transparent without moving the pill"; its 390-pixel width has no counterpart, since the window cannot be
  narrower than 720 pixels)
- `templates::sequence_tests::the_candidate_cap_never_limits_ambiguity` (the same case)
- `templates::sequence_tests::a_missing_captured_name_refuses_the_whole_sequence` ("typed page refusals are atomic and
  empty Enter leaves later bindings paired", and "a sequence naming a deleted template reveals New with the missing
  name")
- `templates::sequence_tests::a_word_refusal_wins_over_any_key_refusal` ("typed page refusals are atomic"; its pairing
  of later bindings after an empty Enter is superseded by "The old quick-launch dispatcher's fences")
- `templates::sequence_tests::enter_queues_the_word_and_an_empty_sequence_is_nothing` (the same case)
- `templates::sequence_tests::a_completed_name_is_matched_again_at_enter` ("a sequence naming a deleted template")
- `templates::sequence_tests::key_refusals_choose_a_hint_and_a_launcher_sentence` ("a bound Bolt and unbound number show
  only the number binding hint", with the deviation "One binding hint for a sequence's unbound keys")
- `templates::sequence_tests::an_unbound_reference_refuses_naming_the_first` (the same case)
- `templates::dialog_tests::a_refused_write_is_reported_and_nothing_changes` ("quick launch discards a failed local mark
  against the shared read", with the deviation "⚡ is lit once the helm stores it")
- `templates::dialog_tests::the_bolt_marks_and_clears_through_the_preferences` (the same case, and "the template mark
  moves, survives rename, and stays cleared after undo")
- `templates::dialog_tests::a_write_in_flight_leaves_focus_and_the_dialog_alone` ("quick launch honors a held local mark
  against the shared read": Escape and Space do not close the dialog while the mark's write is held, though the test
  never presses the header Close, the old case's control; the bolt not lit until the helm stores the mark is the
  deviation "⚡ is lit once the helm stores it")
- `templates::dispatch_tests::the_chord_launches_the_marked_template_unseen` (the same case: the chord afterwards
  launches the mark the helm stored)
- `templates::editor_tests::a_rename_saves_moves_the_mark_and_key_then_deletes_the_old_name` ("the template mark moves,
  survives rename")
- `templates::leave_tests::delete_takes_effect_at_once_and_undo_restores_the_loaded_copy` (the same case's "stays
  cleared after undo")
- `templates::dialog_tests::an_ineligible_bolt_is_unavailable_unless_it_is_lit` ("a marked template that became
  ineligible can still be unmarked")
- `templates::naming_tests::the_created_name_stays_selected_through_pushes_of_its_row` ("repeated naming selects the
  title and returns focus after immediate Enter and Save")
- `launcher::launch_tests::a_refusal_after_the_host_moved_installation_is_generic` ("a refusal after target change
  reveals the spent quick-launch draft": a quick launch's refusal comes back through the launcher's own submit path, so
  the launcher's test of that refusal replaces it, decided on 2026-11-16 by the GPUI migration's coordinator)
- `templates::dispatch_tests::unreadable_bindings_refuse_the_chord_and_number_keys_on_the_page` ("a quick preparation
  error survives New's first host binding", its error half: unreadable bindings open New with "Not launched. Couldn't
  prepare quick launch:" and the helm's words)

Not yet ported:

- the Replace with half of "hidden preparation isolates keys and a queued Replace with requires Launch" (Replace with
  opened while a quick launch is being prepared out of sight, leaving Launch to the user); its key half is
  `templates::dispatch_tests::presses_during_a_preparation_wait_and_its_result_respects_the_moved_on_rule`, above
- the host-binding half of "a quick preparation error survives New's first host binding" (New binding its first host
  after the preparation error without clearing it)
- of "quick launch honors a held local mark against the shared read", the header Close not closing the dialog while the
  mark's write is held

### `tests/launcher-template-shortcuts.spec.ts`

Tests: 8. Covers: back-to-back bolt and number chords stack on New without submitting.

replaced: back-to-back ⚡ presses applying in order to the open launcher's draft, each its own refusal unit, with Launch
left to the user (`templates::dispatch_tests::presses_apply_to_the_open_launcher_in_order`), a press waiting for the
launcher's catalog read (`a_press_waits_for_the_open_launchers_catalog`), a Launch press dropping presses still waiting
for it (`launch_drops_the_batches_waiting_for_the_catalog`), a missing mark or failed read refused there without opening
Templates (`a_missing_mark_or_failed_read_refuses_in_the_launcher`), a shortcut's refusal shown over a press's reason
and a later press's reason shown over it (`a_shortcut_refusal_shows_over_a_press_reason`,
`a_press_after_a_shortcut_refusal_shows_its_own_reason`), and a sequence in the launcher applying, or refusing a word or
an unbound key there (`a_sequence_in_the_launcher_applies_or_refuses_there`,
`templates::armed_tests::the_launcher_form_says_apply_and_sits_at_the_bottom`), a sequence begun from a launcher text
field with nothing selected and the chord kept with a selection
(`launcher::view_tests::the_prefix_in_a_launcher_field_begins_a_sequence_only_with_nothing_selected`; other fields keep
it: `templates::armed_tests::a_text_field_keeps_the_prefix`); number keys applying there, refusing a template that does
not apply, and refusing an unbound key there without opening Templates (`number_keys_apply_to_the_open_launcher`); a
number key on Clone's draft (`templates::launcher_modes_tests::a_number_key_applies_to_clones_draft`), Replace with
refusing a host change, alone or in a sequence, and taking a later press
(`replace_with_refuses_a_host_change_whole_and_takes_a_later_press`), Tab completion in the launcher's form keeping the
search box's query and focus (`tab_completes_in_the_launchers_sequence_and_enter_applies_it`); a shortcut clearing a
quick launch's YOLO question keeping focus and the selection in the Name field
(`templates::dispatch_tests::a_shortcut_clearing_the_yolo_question_keeps_focus_and_selection`), and a key doing nothing
while a dialog sits over the launcher, the old Replace with case's "yields to another modal" half
(`templates::launcher_modes_tests::a_number_key_does_nothing_while_a_dialog_is_open_over_the_launcher`), and a list that
never takes focus, as the Host list, not stopping a key, as the old shortcuts gave way only to a modal dialog
(`templates::launcher_modes_tests::a_number_key_applies_while_the_host_list_is_open`).

- `templates::sequence_tests::words_match_exact_then_lowercase_exact_then_prefix` ("typed launcher matching prefers
  exact names and refuses whole ambiguous batches")
- `templates::sequence_tests::matching_names_are_ordered_by_tier_then_catalog` (the same case)
- `templates::sequence_tests::a_word_refusal_wins_over_any_key_refusal` (the same case's whole-batch refusal)
- `templates::sequence_tests::keys_queue_entries_in_typing_order` ("typed launcher words mix with digits and chords in
  input order")
- `templates::sequence_tests::enter_applies_in_order_through_the_captured_bindings` (the same case)
- `templates::sequence_tests::backspace_edits_only_the_unfinished_word_by_scalar` (the same case)
- `templates::sequence_tests::tab_needs_known_names_and_a_word` ("typed launcher Tab completion is immediate and
  preserves spaced names", the spaced names)
- `templates::sequence_tests::tab_completes_only_a_unique_match` (the same case)

### `tests/template-list-shortcut.spec.ts`

Tests: 4. Covers: Cmd-J exact name launches through the fresh quick-launch path.

replaced, as a templates-only mode of the sidebar's quick switcher: the exact name over an earlier match, the arrows
taking that away, the chips, Enter's unseen launch, and the chord ending an armed sequence
(`session_list::quick_switcher::template_list_tests::an_exact_name_is_started_over_an_earlier_match_until_the_arrows_move`,
`the_chord_opens_the_template_list_from_a_terminal`); no match, Escape and a press outside
(`with_no_match_enter_does_nothing_and_escape_or_a_press_outside_closes`); a partial template opening New
(`a_partial_template_opens_new_with_its_choices_applied`); New and hidden quick-launch preparation keeping the chord
(`a_partial_template_opens_new_with_its_choices_applied`, `the_chord_does_nothing_during_a_hidden_preparation`; a menu
over the page and preferences not yet arrived are the switcher's shared gate, pinned by
`session_list::quick_switcher::tests::the_chord_does_nothing_over_a_menu` and
`the_chord_does_nothing_before_the_preferences_arrive`); a click, which also leaves the Hosts panel
(`a_click_starts_the_clicked_template_and_leaves_the_hosts_panel`); a pick resolving the name afresh and keeping its
choice by name across pushes (`a_pick_uses_the_template_as_the_store_holds_it_now`, with a deviation below); the
switcher's `tl:` rows drawing no chips (`the_switchers_tl_rows_draw_no_key_chips`); the held chord once per press
(`actions_tests::the_launch_chords_run_once_per_press_from_a_terminal`); the Keyboard shortcuts overview's row
(`shortcuts::tests::every_chord_the_table_shows_runs_the_action_its_row_names`); the `template-launcher` golden. Every
quick launch from the page leaving the Hosts panel, as the old app's did through the list's New:
`templates::dispatch_tests::a_quick_launch_from_the_page_leaves_the_hosts_panel`

## Template cases of specs in other areas' files

Specs whose rows sit in another area's parity file but which have template or quick-launch cases this area ports. The
rows stay in those files; the cases and the tests replacing them are listed here.

### `tests/quick-switcher.spec.ts`

Its file: sidebar and session view.

Template cases: the `tl:` section ("a template pick waits for launcher inputs before applying the template", "opening a
template reports the launcher's catalog read failure", and the plain query of "the pinned name action opens ordinary New
with remembered defaults", which lists no template rows); the Quick launch row is `tests/quick-launch.spec.ts`'s.

replaced: the `tl:` label, the rows' matching, order, summaries and shortcuts, an unavailable collection
(`templates::rows_tests::a_leading_tl_label_makes_a_template_query`,
`rows_match_a_lowercased_substring_in_order_with_summaries_and_shortcuts`,
`unavailable_templates_give_the_reason_instead_of_rows`), a pick opening New with the template and never launching, the
banner for an incomplete one (`a_switcher_pick_opens_new_with_the_template_and_never_launches`), its refusals in the old
switcher's words without "Not launched." (`a_switcher_picks_refusals_are_the_old_switchers_words`), New opened at once
and the template applied once the catalog answers (`a_pick_applies_its_template_once_the_catalog_is_read`); the section,
its hint and rows as the sidebar draws them in `session_list::quick_switcher::tests` (its row in the sidebar's file
names them); the Name field staying editable after a pick's refusal
(`templates::rows_tests::new_stays_editable_after_a_picks_refusal`)

### `tests/github-checkouts.spec.ts`

Its file: launcher.

Template cases: quick launch into a managed checkout, template base-name numbering, the path race, the template name
pill, and Save as template from a checkout template.

replaced: a hidden quick launch waiting for its checkout preview, one whose preview fails shown
(`launcher::checkout_tests::a_quick_launch_into_a_checkout_waits_for_its_preview_out_of_sight`,
`a_quick_launch_whose_preview_fails_is_shown`), a repository template's taken base name numbered and launched out of
sight, with naming afterwards (`templates::dispatch_tests::a_taken_base_name_is_numbered_and_launched_out_of_sight`),
the moved-on rule kept through the numbering (`a_numbered_quick_launch_the_user_moved_on_from_starts_quietly`), a quick
launch that loses its folder shown without resubmitting (`a_base_name_quick_launch_that_loses_its_folder_is_shown`), the
visible guarded wait not following a taken name
(`launcher::checkout_tests::a_guarded_wait_does_not_follow_a_taken_base_name`), a lost path race in the open launcher
taking a new preview (`a_lost_race_takes_a_new_preview_and_sends_nothing`), a taken base name moving on and a fixed one
staying (`a_taken_default_name_moves_on_and_an_exact_one_stays`,
`templates::apply_tests::a_templates_name_kind_lasts_until_the_name_is_typed`), the "From template · numbered if taken"
pill and "(<base> is taken)" (`templates::checkout_name_tests`), a repository destination
(`apply_tests::a_repository_destination_chooses_that_repository`), Save as template keeping a fixed name
(`templates::save_tests::a_template_name_keeps_its_kind_and_an_unidentified_host_is_omitted`), and the editor left clean
after saving a base name (`templates::editor_tests::saving_a_base_name_leaves_no_unsaved_changes`). Save as template
keeping the unnumbered base after the name moved to `-2`
(`templates::save_tests::save_as_template_keeps_the_unnumbered_base_after_the_name_moved_on`) and working while the
preview is refused (`templates::save_tests::save_as_template_works_while_the_checkout_preview_is_refused`)

### `tests/custom-permissions.spec.ts`

Its file: launcher.

Template cases: custom template words ("custom template words round trip into the launcher and clear on a stacked
permission").

replaced: the words applied from a template and cleared by a later template's permission
(`templates::apply_tests::custom_words_are_cleared_by_a_later_templates_permission`), the editor showing and saving them
(`templates::editor_tests::custom_approvals_carry_their_words`), and words without Custom refused
(`templates::fields_tests::custom_words_must_match_their_choice`; the helm's own refusal is the backend's). A bad quote
refused at Save, and switching templates replacing the raw words, which Discard restores
(`templates::editor_tests::custom_words_that_do_not_parse_refuse_save_and_each_template_shows_its_own`)

### `tests/terminal-create-idempotency.spec.ts`

Its file: launcher.

Template cases: a null-permission template applied through `tl:` ("a null-permission ${harness} template retains consent
and one session after a lost reply").

replaced, with the lost reply's retry with one intent key superseded (the launcher's row: the GUI sends no create keys):
a `permissions: null` template picked under `tl:` resetting New to the harness's default, Guarded for Codex and YOLO for
Goose (`launcher::search_view_tests::a_tl_reset_of_codexs_permission_launches_guarded`,
`a_tl_reset_of_gooses_permission_launches_yolo`); Goose's YOLO question then is the launcher's ordinary one

### `tests/popover-template-movement.spec.ts`

Its file: foundations.

Template cases: the number-key menu when the ⚡ moves or a rename moves its button (six cases).

superseded: SPEC.md "Things changing underneath" (an open menu moves with its thing), as the foundations row records.
The remaining case that row names, a menu whose anchor is gone closing, is
`templates::dialog_tests::the_key_menu_closes_when_its_template_is_gone`; no old case removed the menu's own template

- `templates::dialog_tests::the_key_menu_follows_its_button_through_what_the_helm_pushes` (the six cases, inverted: the
  menu stays open through each ⚡ transfer and rename, moving with its button)

### `tests/tooltip-coverage.spec.ts`

Its file: foundations.

Template cases: the Templates dialog's, its editor's, the leave question's and Save as template's checklist's hover
help, and the Cmd+J rows'.

superseded, as the foundations row records: the component constructors require hover help, so a control without it does
not compile. Its words are tested where they say something
(`templates::shortcuts_tests::hover_help_names_the_behavior_and_the_choice`,
`templates::dialog_tests::the_button_help_names_the_chord_and_the_marked_template`); the Cmd+J rows' and field's in
`session_list::quick_switcher::template_list_tests::the_chord_opens_the_template_list_from_a_terminal`

### `tests/podman-environments.spec.ts`

Its file: launcher.

Template cases: "container template quick launch retains its saved place". The spec's other cases are the launcher's and
Settings and hosts' (the launcher's entry lists them).

replaced: a number key on a container template launching out of sight through the environment route alone, with the
stored recipe and place, and the helm's refusal bringing the launcher back on the stored place, shown in its Recipe and
Container host rows (`templates::dispatch_tests::a_container_template_on_a_number_key_launches_at_its_place`); the
placement applying as the launcher's own Runs on edits, refused when its recipe or place host is gone, and left by a
later template's host (`templates::apply_tests::a_container_template_enters_a_new_container_at_its_home`,
`a_template_for_the_chosen_segment_changes_only_its_recipe_and_host`,
`a_legacy_recipe_template_becomes_a_sandbox_target`,
`a_missing_recipe_or_removed_place_host_is_refused_with_nothing_applied`, `a_template_host_leaves_the_environment`,
`restart_with_refuses_a_placement`, `a_template_folder_is_refused_in_a_new_environment`); Save as template left out for
a recipe as well as on a host an environment owns, as the old footer left it out (SPEC.md, "Sandbox sessions", keeps
Save as template from an environment launch deferred; decided 2026-11-16 by the GPUI migration's coordinator, in place
of capturing the placement), tested by `templates::save_tests::an_environments_host_offers_no_save_as_template` and
`a_launch_into_a_new_environment_offers_no_save_as_template`. The Podman check the old place read started for a host
never checked is the launcher's: a quick launch made out of sight asks for every container host's check and comes back
shown until one answers
(`launcher::environment_launch_tests::a_quick_launch_asks_for_the_checks_and_comes_back_until_one_answers`).

### `tests/launch-progress.spec.ts`

Its file: launcher.

Case: "quick launch shows a named pending pill" and its twin "template shortcut shows a named pending pill" (one case
run for ⚡ and for number key 1).

replaced, with the reveal after the switcher superseded: one create titled with the template-supplied name, no pill at
1.9 seconds, then a status naming the session and its host and counting "2 s", the switcher opening and closing over the
hidden launch with the pill still up (after the pill shows, where the old test ran it before 1.9 seconds), and the
refusal taking the pill down
(`templates::dispatch_tests::the_chords_held_launch_names_its_session_and_host_in_the_pill`,
`a_number_keys_held_launch_names_its_session_and_host_in_the_pill`). The old test then saw the refusal reveal the
launcher as a modal again; under today's SPEC.md ("Keyboard focus", "Who may move focus") the switcher's key presses
count as moving on, so the refusal leaves the "launch not started" notice instead, and the old alert's words come back
through its Open launcher. The `[inert]` check has no counterpart: the pill is a layer that takes neither focus nor the
pointer, and there is no inert page

### `tests/tensorlake-sandbox-sessions.spec.ts`

Its file: launcher.

Template cases: "template placement keeps Host editable after leaving an environment" and "a recipe template quick
launch asks before one recipe request". The spec's other cases are the launcher's.

partly: "template placement keeps Host editable after leaving an environment" is ported whole (a legacy recipe template
opening on New sandbox, Existing host leaving an empty Host to choose, the chosen segment changing nothing, the save
storing the host alone, and a Host added to another template starting empty), with the editor's other placement rules
beside it. The quick launch's question waits for the launcher's New sandbox ("A sandbox template's quick launch is
refused until New sandbox is ported", below).

- `templates::editor_tests::leaving_an_environment_keeps_the_host_editable`
- `templates::editor_tests::entering_an_environment_chooses_its_first_recipe_and_place` (the old authoring mode's
  starting recipe and place, an unusable host and an unbuilt recipe choosable, the target saved without a host)
- `templates::editor_tests::retained_choices_and_an_unusable_only_host_stay_in_reach` (a recipe and host no longer
  listed kept as retained, and New container offered when its only host is unusable)

### `tests/restart-with.spec.ts`

Its file: launcher.

Template cases: "restart with applies an allowed partial agent template", "restart with applies an allowed partial
command template", "restart with applies a template's matching held ${field}" (the launch kind, agent type, host,
destination and name), "restart with refuses a template's held ${field} atomically" (the same fields and the initial
prompt), "restart with refuses held shortcut batches and bolt templates" and "restart with applies matching bolt and
shortcut templates without restarting". The launcher's entry lists the spec's other cases, and also lists the partial
command template and the two shortcut cases as not yet ported there.

partly: the two held-field cases are replaced by the template rules' unit tests, with no port through the open dialog
(decided 2026-11-16 by the GPUI migration's coordinator), as far as those tests reach: the matching case whole, and of
the refused case the agent type, the initial prompt and the host through a container placement. Restart with's refusal
is one check in the template application (`restart_refusal` in `templates/apply.rs`) that names the first held field a
template would change, and these tests run it without a window.

- `templates::apply_tests::restart_with_applies_equal_held_values_as_nothing` ("applies a template's matching held
  ${field}": a template repeating the launch kind, agent type, host, folder, with a trailing slash, and name applies as
  nothing for those fields and its other edits land, and the repeated name brings no name kind)
- `templates::apply_tests::restart_with_refuses_a_change_to_a_held_field` ("refuses a template's held ${field}
  atomically", through a changed agent type and an initial prompt, each refused naming the field with nothing applied)
- `templates::apply_tests::restart_with_refuses_a_placement` (the same rule for a container template, refused as a held
  host)
- `templates::apply_tests::restart_with_refuses_clearing_a_commands_resume` (the same rule for a command restart's
  resume choice, which no old case named)
- `templates::apply_tests::restart_with_resets_permissions_to_todays_default` (the permission reset the partial agent
  template carries, as a rule; the case itself is not yet ported)

Not yet ported:

- of "restart with refuses a template's held ${field} atomically", the launch kind, a plain host (one naming another
  installation, not a placement), the destination and the session name: `restart_refusal` checks each, but no test makes
  a Restart with template change them
- the two partial-template cases and the two shortcut cases: no test applies `tl:`, ⚡, a number key or a sequence to an
  open Restart with, or checks what its main action then sends

### `tests/interface-fonts.spec.ts`

Its file: foundations.

Template cases: the shortcut-digit half of "template summaries split command fonts and shortcut digits keep equal
widths" (the number keys' digits drawn at equal widths). Its summary-fonts half is in the foundations entry
(`templates::summary_tests::template_values_are_host_text`).

partly

- `templates::dialog_tests::the_number_keys_digits_are_drawn_at_equal_widths` (with the real text system, the key
  buttons' chords for keys 1 and 8 are each drawn in the monospace face, so they are equally wide; it measures the
  painted chords, not the buttons' own widths)

Not yet ported: the key buttons' own widths for keys 1 and 8 being equal, which the old case also compared.

### `tests/nonblocking-launch.spec.ts`

Its file: launcher.

Template cases: "quick-launch naming is skipped after a switcher opened during its create" and "quick-launch naming
waits for an asynchronous host result and rechecks input after it closes", which the launcher's entry hands to this
area.

superseded: the second case's premise cannot arise in the GPUI app. Its host refusal opens by itself after the ⚡ press,
and naming waits behind it. Here a dialog opens only through the mark of the user's own action and is refused once they
have pressed anything since (`layers`, "Things the user did not start never open a layer"; decided on 2026-11-17 by the
GPUI migration's coordinator). The ⚡ chord is such a press, so that refusal opens nothing and stays under its host's
row, and no dialog can be up for a naming editor to wait behind. The only layers opened without a user action are the
window-start dialogs, and the chord does nothing while one is up.

- `templates::naming_tests::a_quick_launch_the_user_moved_on_from_is_not_named` (the first case: a key pressed while the
  create is out, standing in for the switcher's chord, sends the success down the moved-on path with no naming, so none
  is left to uncover when the switcher closes)
- `hosts::update::tests::a_refused_plan_after_the_user_moved_on_opens_nothing` (the second case's host refusal, arriving
  after a later press: no dialog opens, and the refusal shows under the host's row)

## Deviations

Differences from the old app at the reference commit, each with its reason, and each also reported to the migration's
orchestrator.

- **One escalation banner per batch.** When one shortcut press or completed sequence applies several templates, the
  permission-escalation banner is judged once: the draft right after the batch's last agent switch against the draft
  from before the whole batch, and its Undo goes back to before the batch. The old app judged each template of a press
  separately, from the draft before that template, so the two can differ when earlier templates of one press changed the
  agent, the permission or the launch kind before the last switch. Chosen because the press is one action for the user,
  applied all or nothing (the migration's templates goal asked for the banner to be judged once per batch). Tested by
  `templates::apply_tests::escalation_is_judged_once_for_the_batch`,
  `a_permission_chosen_before_the_last_switch_still_raises_the_banner`,
  `a_permission_chosen_after_the_last_switch_raises_no_banner` and
  `a_later_edit_after_the_switch_keeps_the_switchs_banner`.
- **Shortcut names in one platform's form.** The Templates dialog's help, hint lines and the Templates button's hover
  name the ⚡ chord, number keys and the sequence prefix as this platform spells them (⌘P on a Mac, Ctrl+Shift+P on
  Linux). The old app, a web page that could not tell which form its client handled, printed both ("⌘P / Ctrl+Shift+P").
  The desktop app knows, and SPEC.md's Keyboard shortcuts rule is "Each shortcut shows only the form this client
  handles". Tested by `templates::dialog_tests::the_button_help_names_the_chord_and_the_marked_template`.
- **The dialog's header stays on screen.** The old Templates dialog scrolled as one piece, its header included. The GPUI
  dialog frame keeps the header (title and Close) pinned and scrolls the body, as every other dialog of the app does; at
  a window too short even for the header the whole card scrolls.
- **A long summary is cut without an ellipsis.** A row's summary stays on one line, as the old one did, but one that is
  longer than the row is cut at the row's edge rather than ending in "…": the summary is several text elements (the
  template's values are host text, each drawn on its own), and an ellipsis can only end one. The launcher's footer
  summary has the same cut.
- **The ⚡ and key buttons show a hover fill.** The old buttons showed only an edge under the pointer; the GPUI button
  component draws the ghost tier's hover fill, the nearest look it has.

- **The number-key menu's labels.** The menu's header (the template's name over "Number shortcut") is the shared menu
  header the session and host menus draw, but its key labels are in the interface face, not the old monospace. Disabled
  controls fade to half opacity, the component layer's, not the old 40%.
- **The ⚡ and key buttons while a write is in flight.** The old list dimmed them to 40% while busy; here they keep
  their look and focus and ignore presses (the button component's busy state), since a dimmed, disabled button gives up
  focus, which would fall to the dialog and lose the user's place. Tested by
  `templates::dialog_tests::a_write_in_flight_leaves_focus_and_the_dialog_alone`.
- **One binding hint for a sequence's unbound keys.** When Enter outside the launcher finds both an unbound number key
  and an unbound ⚡ in a sequence, the old app opened Templates with both hint lines; the GPUI dialog opens with one
  hint, for whichever unbound reference came first in the queue. Tested by
  `templates::sequence_tests::key_refusals_choose_a_hint_and_a_launcher_sentence`.
- **Names come from the pushed list.** The editor checks a new or changed name against the templates the helm pushed,
  not a list it read itself, so the old refusal "The template list is still loading" is gone. A name is refused as
  unprovable only while the helm reports the templates unavailable or before its first snapshot, never for an empty
  list, and the refusal now reads "The template list could not be read, so Farhelm cannot check that no other template
  has this name; save again once it is back" in place of the old "could not be loaded ... retry loading it above", since
  there is no Retry to press. Tested by
  `templates::editor_tests::a_new_name_is_refused_while_the_templates_are_unavailable` and
  `save_refusals_come_in_the_old_order_and_send_nothing`.
- **No list reload after a save.** The old dialog reloaded its list after a save, and a reload that raced the save could
  lack the template just saved. The helm pushes the changes a command causes before that command's reply
  (`ui_channel::run` in `crates/farhelm-helm/src/ui_channel.rs` sends a command's changes and then its reply, pinned by
  `ui_channel_tests::the_changes_a_command_causes_arrive_before_its_reply`), so the list already holds the saved
  template when the editor learns the save is done. Tested by
  `templates::editor_tests::save_writes_the_whole_template_once_and_keeps_focus`.
- **While a save is in flight.** The old editor disabled every control while it was busy and made the dialog itself a
  Tab stop. Here the text fields turn read-only, the buttons (Save, the rows, "+ New template", ✕, "+ Add field") turn
  busy, keeping focus and ignoring presses, and the choices ignore presses without changing their look, so focus never
  falls out of the control the user was on.
- **The editor's small controls.** The model suggestions are neutral buttons with the neutral fill where the old ones
  had none, and the add menu's entries have the template rows' padding (10 by 12 pixels) where the old ones had 8.
  "Replace name" has hover help ("Clear the name so a new one can be typed"), which the old button lacked and every
  control here needs.
- **Where focus returns from a Templates opened by a shortcut.** The old dialog returned focus to the marked-menu toggle
  or the Templates button by name, whoever opened it. A Templates the ⚡ or a number key opens with a hint returns focus
  to what held it before it opened (usually the terminal), under SPEC.md's general rule that closing something returns
  focus to where it was before it opened: a shortcut is a key press that moved no focus, and the GPUI dialog has no
  handle on the sidebar's Templates button. Opened from the Templates button, it still returns focus to that button.
  Tested by `templates::dialog_tests::closing_a_hint_opened_dialog_returns_focus_where_it_was` and
  `closing_returns_focus_to_the_templates_button`.
- **Host names in the Host select.** The select's options are the app's own words, so a host's name is shown as its
  escaped presentation (hidden characters as `<U+…>`) but is not direction-isolated from the words around it, as host
  text elsewhere is; the select has no host-text option labels yet.
- **No build note under Container host.** The old editor drew the launcher's status line under a container template's
  Container host too: the place's reason, or "Preparing…", "Ready on <host>", "Builds on first launch here (a few
  minutes)" or "Checking build status…" for the recipe there. The GPUI editor draws none: the launcher's note is drawn
  by the launcher's own view, and sharing it would mean changing that view's code for the editor. The Container host
  select still names each host's reason after its name, as the old one did, and the launcher draws the note once the
  template is applied there. The editor also starts no Podman check: the old editor's read of the places asked the helm
  to check every host never checked, while the GPUI editor shows each host's last pushed reason ("this host needs a
  Podman check" until the launcher or Settings asks for one).
- **The Runs on switch's two small differences.** A chosen segment that has nowhere to enter (New container on a
  container template while no listed host can hold containers) is drawn enabled and does nothing when pressed, where the
  old one was drawn disabled. A new environment entered with no recipe the provider can run shows "Choose a recipe" in
  its Recipe select, where the old select showed "Unavailable recipe (retained)" for the empty name. Tested by
  `templates::editor_tests::retained_choices_and_an_unusable_only_host_stay_in_reach` (the empty recipe).
- **A rename's bindings are read from the pushed preferences.** The old rename read the preferences again after saving
  the new name and said "shortcut bindings could not be read" when that read failed. Here the store is that read, so the
  message is said while the helm reports its preferences unavailable, or before they have arrived. Tested by
  `templates::editor_tests::unreadable_bindings_stop_the_rename_before_any_move`; the moves and their refusals by
  `a_rename_saves_moves_the_mark_and_key_then_deletes_the_old_name`,
  `a_rename_with_nothing_bound_goes_straight_to_the_deletion`,
  `a_refused_mark_move_stops_the_rename_and_keeps_the_old_template` and
  `a_refused_key_move_stops_the_rename_and_keeps_the_old_template`. The old "a shortcut write was interrupted" words are
  kept for a step that cannot start, which here only a retired store refuses (a reset closes the dialog with it), so no
  test reaches them.
- **Save as template's checklist values.** The old checklist wrapped each checkbox and its "Label: value" in one
  `label`, so a click on the value toggled it too. Here the checkbox is the shared component, labelled with the field's
  name, and the value beside it is host-supplied text drawn through the host-text element (escaped and
  direction-isolated), which the checkbox's own label cannot hold; a click on the value does nothing. No test clicks a
  value.
- **Save as template's name check reads the pushed list.** The old panel read the template list again before its PUT.
  Here the store is that read (the helm pushes every change, a change this window makes before its reply), so a taken
  name is refused against it; as before, a write by someone else between that check and the PUT is not caught. Tested by
  `templates::save_tests::a_spaced_or_taken_name_is_refused_without_a_write`.
- **Where a saved template opens.** The old launcher always opened Templates on the saved template. Here a completion
  follows SPEC.md's "Who may move focus": the launcher closes, and Templates opens on the template only if the user
  pressed nothing while it was saved; otherwise the template just shows in the list. Either way any edit made to the
  launcher's draft while the save was out is discarded with the launcher, as in the old app. Tested by
  `templates::save_tests::enter_saves_the_checked_fields_and_opens_the_new_template` and
  `a_save_the_user_moved_on_from_opens_no_dialog`.
- **How far opening the panel scrolls.** The old panel focused its name field with WebKit's `focus()`, which centers a
  field it scrolls to, so the heading and part of the checklist showed under it. The focus coordinator scrolls a focused
  control just into view, so here the name field sits at the body's bottom edge with the checklist below the fold.
  Showing more needs the coordinator to reveal an area around a control, which it does not offer. Tested by
  `templates::save_tests::opening_the_panel_brings_its_name_field_into_view`.
- **A sandbox template's quick launch is refused until New sandbox is ported.** The old app asked "Create a sandbox for
  this session?" before a sandbox template's launch went out (`tensorlake-sandbox-sessions.spec.ts`, "a recipe template
  quick launch asks before one recipe request"). The GPUI launcher does not ask yet, so a template that places sessions
  on the Tensorlake account (a legacy recipe-only one included) applies, and its launch goes out unconfirmed and the
  helm refuses it, unless "Create Tensorlake sandboxes without asking" is on. The question comes with the launcher's New
  sandbox. Decided 2026-11-16 by the GPUI migration's coordinator: accept the interim refusal, with no stopgap in
  templates.
- **A refused deletion of a renamed template's old name.** As in the old app, the status line then shows only the helm's
  words, not which step of the rename failed; by then the template is saved under the new name with the mark and key
  moved, and the old one is still there. Tested by
  `templates::editor_tests::a_refused_delete_of_the_old_name_shows_the_helms_words`.
- **A save from the leave question that fails.** The old question stayed up through its Save and after a refusal, so the
  user could answer it again. An inline question's confirming answer runs once (the prompt's rule), so here the question
  closes when Save is pressed; a refused save leaves the editor on the draft with the helm's words in the status line,
  and leaving again asks again. Tested by `templates::leave_tests::save_leaves_only_once_the_save_succeeds`.
- **The leave question's answers while a ⚡ or number-key write runs.** The old answers were disabled during any write
  of the dialog's. The prompt has no busy look for its owner's writes, so here Discard and Save stay pressable, and
  pressed then they close the question and do nothing else, keeping the draft, as every other way to open a template
  does nothing while a write is in flight. The bolts and keys are the only writes the question leaves pressable. Tested
  by `templates::leave_tests::answers_while_a_bolt_write_runs_keep_the_draft`.
- **One Delete button.** The old footer kept a Delete and a "Delete permanently" button mounted and hid one, because its
  web view could swap a click listener before painting the new label. A GPUI click handler belongs to the frame that
  drew it, so here one button's handler holds the kind its label showed. Tested by
  `templates::leave_tests::the_label_the_user_saw_decides_the_undo` and
  `a_template_failing_save_rules_is_deleted_permanently`.
- **Deleting a template the list no longer holds.** The old Delete said "Reload the template list before deleting this
  template" when its list, a read that could be stale, lacked the template. The pushed list is never stale that way: a
  template missing from it was deleted elsewhere, so Delete says "This template is no longer stored, so there is nothing
  to delete" and sends nothing. Tested by `templates::leave_tests::deleting_a_template_deleted_elsewhere_sends_nothing`.
- **Undo's refusals.** The old Undo read the list again and said "Cannot check the deleted template's name:" with the
  error when that read failed. Here the pushed list is that read, and while the helm reports it unavailable Undo says
  "The template list could not be read, so Undo cannot check that the deleted template's name is still free; try again
  once it is back". A taken name, where the old Undo borrowed Save's "A template named … already exists; edit that one,
  or choose another name", says "Another template is now named …, so the deleted template cannot be restored under that
  name": Undo has no name to choose. The taken case is tested by
  `templates::leave_tests::undo_refuses_a_taken_name_and_keeps_a_spaced_one`.

- **The sequence's key rules, as decided for the GPUI port.** The sequence takes keys through the actions module's key
  capture, ported from the old armed listener's classifier (its oracle cases are
  `templates::armed_tests::sequence_keys_are_sorted_as_the_old_classifier_did` and
  `other_keys_sort_as_cancelling_and_passing_on`). Two cases had to be decided again. A keypad digit cancels the
  sequence and goes on to its target, as in the old app, so a word never starts with one by accident; only a key that
  carries its native identity can be told apart, so a keypad digit an input method commits (or the test bridge injects)
  reads as the main row's digit. And a second prefix while armed ends the sequence and is consumed, which is what the
  old app's cancel amounted to (its terminal dropped the chord); passed on, it would reach the prefix's binding and arm
  a new, empty sequence.
- **The session launcher's vertical margin on wide windows.** This area fixed the shared dialog frame's vertical margin
  to the old `max(24px, 5dvh)` (`app.css` at the reference commit, line 128). The old launcher's backdrop alone used a
  flat 24 pixels in windows at least 1000 pixels wide (line 6629, "Leave room for populated launch rows"), so the GPUI
  launcher now keeps 5% of the height there, a little less room than the old one on a tall, wide window. Restoring that
  exception is the launcher area's, which was told when the frame changed.
- **The sequence pill fades in without scaling up.** The old pill faded and scaled up from 96% over 200 ms (off under
  reduced motion). The GPUI pill keeps the fade (the component layer's one-shot entrance, also off under reduced motion)
  but not the scale: GPUI cannot scale a box (SPEC_impl.md, "Animation time").
- **The sequence pill's place follows the selected session, not the details bar.** Otherwise the pill sits where the old
  one did. Outside the launcher it is centered on the main pane, 9 pixels lower in the Mac window with a session open;
  with the 30-pixel top row (48 on the Mac) and the 30-pixel details bar, that is the terminal's center. It sits above
  the center only while lines under the top row (a long Restart or Replace, a refusal, a YOLO question) or the stale
  band push the terminal down, as the old one did. In the launcher its bottom is 14 pixels above the main pane's bottom
  edge, or 44 with a session open, which keeps it above the details bar, as the old `bottom: 14px` and `44px` rules did.
  The difference: the old rules keyed on whether the details bar was drawn, and the GPUI pill keys on whether a session
  is selected, so a selected session the store has no row for (no bar) still gets the Mac's 9 pixels and the
  launcher's 30. The tests cover the bottom placement with a session open
  (`templates::armed_tests::the_launcher_pill_stays_above_an_open_sessions_details_bar`) and without one
  (`the_launcher_form_says_apply_and_sits_at_the_bottom`), and the centered pill with no session open
  (`the_prefix_with_no_session_open_centers_the_pill_on_the_main_area`). The centered pill with a session open and the
  Mac's lowering were checked by reading the code and the old stylesheet, not by a test. Its look was compared with the
  old `.launch-sequence-*` rules, since there is no reference screenshot of the pill; the golden is `sequence-pill`.
- **Cancels the GPUI app sees differently.** A lone Alt no longer cancels a sequence (GPUI reports a modifier alone as a
  modifier change, with no key-down; SPEC.md names only Meta, Control and Shift as not cancelling). A dialog that opens
  while a sequence is armed without a click or key (a result the user started finishing) ends the sequence at the next
  key, which goes to the dialog, rather than when it opens, since nothing tells the sequence a layer opened; the pill
  shows until then. Tested by
  `templates::armed_tests::a_dialog_keeps_the_keyboard_from_the_prefix_and_an_armed_sequence`.
- **Input-method text while a sequence is armed.** Text committed by an input method (a CJK source, a dead key's accent)
  arrives without a key-down, so the capture never sees it and a focused terminal still sends it to its program. The old
  listener cancelled on a composing key. Refusing it falls to the terminal's input handler.
- **Where New shows a quick launch's banner.** The needs-more-choices, number-key and sequence banners are drawn in the
  launcher's notices region beside its actions, rather than at the top of the body as the old `.partial-template-banner`
  was (the launcher area asked for that region, so the launcher keeps one place for what it has to say). The banner
  keeps the old look: a warning-colored edge, the raised background, the words in bold. Every banner goes about six
  seconds after its templates applied, or at the first edit. The banner is drawn first in the notices region, as its own
  element, above a press's reason, the helm's words or the YOLO question, which it never hides. Tested by
  `templates::dispatch_tests::an_incomplete_mark_opens_new_with_its_choices_and_banner`,
  `a_refused_mark_opens_new_unchanged_with_the_refusal` and `a_shortcut_refusal_shows_over_a_press_reason`.

- **Template refusals sit above the search box, following the old app.** SPEC.md said a template refusal in the launcher
  appears "beneath search"; it was corrected on 2026-11-15 to match the old app, so every template refusal in New (a
  `tl:` row's, a shortcut's, a sequence's, and a quick launch's "Not launched." one) sits above the box in one plain
  amber line, like the launcher's own refusal of a held field. Tested by
  `launcher::search_view_tests::a_refused_tl_row_changes_nothing_and_says_why_above_the_box`.

- **An unreadable template list is a reason in the results.** With the helm's templates unavailable, `tl:` shows
  "Couldn't load templates:" and the helm's words in the results area, where the old app said "No template matches"
  there and the reason, with a Retry button, beneath the box. There is no Retry, since the templates are pushed and come
  back by themselves, and no "Loading templates…" state, since before the first snapshot the window is inert. Tested by
  `launcher::search_view_tests::unavailable_templates_are_the_reason_in_the_results`.

- **Every quick-launch banner expires.** SPEC.md gives the number-key banner its six-second expiry and has the
  sequence's follow it; it says nothing for the ⚡ chord's, Cmd+J's and the switcher's. The old launcher expired every
  one of them six seconds after it was drawn, and the GPUI launcher does the same, counting from when the templates
  applied (SPEC.md's "after application finishes"). A quick launch the user moved on from, which waits behind the launch
  notice, spends those seconds there, so New opened from the notice later may show no banner. Tested by
  `templates::dispatch_tests::a_partial_number_key_opens_new_with_the_prefix_banner_for_six_seconds`,
  `a_sequence_that_cannot_launch_opens_new` and
  `templates::rows_tests::a_switcher_pick_opens_new_with_the_template_and_never_launches`.

- **Hidden quick-launch preparation does not block the page.** On the page a quick launch first has the launcher read
  its model catalog out of sight (helm-local, usually a frame or two), and then, for a press that launches out of sight,
  the host's launch history while New's folder default is pending and the templates chose neither a folder nor a managed
  checkout; the launcher's slot holds it meanwhile, so New, the template shortcuts and the sequence prefix treat the
  launcher as busy. SPEC.md has hidden preparation "block page input without focusing its hidden fields"; pointer input
  on the page is not blocked, and a click or key during either read counts as moving on, so the completion falls back as
  SPEC.md says (the session appears without being opened, or New is offered by a notice). This is the templates goal's
  default for a hidden preparation; blocking the page would need a layer that draws nothing and takes no focus.

- **A switcher template pick shows no loading state.** SPEC.md has New show "a cancellable loading state until the
  template inputs are ready". The GPUI launcher has no loading state: a pick opens New on its prefill at once, as the
  old switcher did, and applies the template once New's catalog read answers, helm-local and usually a frame or two. In
  that frame or two New takes edits, which the template then applies over, and a press of Launch starts New's plain
  prefill and drops the pick, since a launch drops the batches still waiting for the catalog.

- **A switcher pick shows the needs-more-choices banner.** The templates goal asks for the banner on an incomplete pick,
  so the GPUI app shows it once the template applies. The old app cleared it for the switcher's New (its `open_new` set
  no partial-template banner), so this is an addition over the old app.

- **Edit > Cut can begin a sequence on a Mac.** In a launcher text field with nothing selected, the Mac app's Edit menu
  Cut begins a sequence, as Cmd+X does there; the old app's menu Cut never did. Both are GPUI Kit's one Cut action, and
  the field cannot tell which sent it (`TextInput::on_unselected_prefix`). Tested by
  `components::tests::text_input::by_the_mac_rule_cut_with_nothing_selected_runs_the_handler_and_with_a_selection_cuts`.

- **A refused ⚡ or number-key write says it was not saved.** The old list showed the change before the helm stored it,
  and on a failed write said "The quick-launch mark may not have been saved; showing the mark read from Farhelm" (or the
  keys) once it had read them again, since its write could fail without saying whether the helm stored it. A command on
  the helm channel is answered, and a refused one stored nothing, so here a refusal says "The quick-launch mark was not
  saved:" or "The number key was not saved:" with the helm's words. Tested by
  `templates::dialog_tests::a_refused_write_is_reported_and_nothing_changes`.
- **⚡ is lit once the helm stores it.** The old bolt lit at the click and went dark again if the write failed. Nothing
  in the GPUI list is optimistic: a bolt shows what the store holds, so it lights when the helm pushes the stored mark,
  which on the embedded helm is when its reply arrives. Tested by
  `templates::dialog_tests::the_bolt_marks_and_clears_through_the_preferences`.
- **Unreadable bindings refuse in the pushed state's words.** The old app read the preferences again for each ⚡ or
  number-key write and each quick launch, and refused when that read failed, with the read's message alone in the list
  and "Couldn't prepare quick launch:" or, in the launcher, "Couldn't load templates:" before it. Here the store is that
  read, so the refusal comes while the helm reports its preferences unavailable or before they arrive: the list says
  "The number key was not saved; shortcut bindings could not be read:" (or the quick-launch mark) with the reason, in
  the rename's words; the ⚡ chord, the number keys and the Quick launch row keep the old dispatcher's words. The old
  app refused every press on a failed read, a sequence of words and a Cmd+J pick included; here a press that names its
  templates by name reads no binding and is not refused, so a sequence is refused only when it holds a number key or ⚡.
  And a sequence is judged by whether the bindings were known when its prefix was accepted, since its copy of them is
  what Enter resolves (SPEC.md: "Bindings refresh when the prefix starts"), where the old app judged it by its read at
  Enter: one armed while they were unknown is refused after they come back, and one armed while they were known launches
  its keys even if they become unknown before Enter. The Quick launch row's label follows the old row, which showed its
  own copy of the preferences: "Quick launch: Mark a template" before they arrive, and the last mark the helm pushed
  while they are unavailable, since the store keeps the last preferences it had. Tested by
  `templates::dialog_tests::unreadable_bindings_refuse_a_key_or_mark_write`,
  `bindings_that_never_arrived_refuse_a_key_write`,
  `templates::dispatch_tests::unreadable_bindings_refuse_the_chord_and_number_keys_on_the_page`,
  `bindings_that_never_arrived_refuse_the_chord_on_the_page`, `unreadable_bindings_refuse_a_press_in_the_launcher`,
  `a_sequence_on_unreadable_bindings_refuses_its_keys_but_not_its_words` and
  `templates::rows_tests::the_quick_launch_row_before_the_preferences_arrive_says_mark_a_template_and_refuses` and
  `the_quick_launch_row_keeps_the_last_mark_and_refuses_while_the_bindings_are_unreadable`.
- **A quick launch while the host list is unavailable.** The old app read the host list for each quick launch and, when
  that read failed, opened New with "Couldn't prepare quick launch: the host list failed:" and the error, leaving
  recovery to the user (its "a host-read fallback permits keyboard cancellation before recovery" and "an initial
  host-read failure reveals New and recovery stays manual"). The GPUI launcher reads no host list: it uses the hosts
  collection the helm pushes, which keeps its last hosts when the helm reports it unavailable, and does not look at that
  availability. So a quick launch then goes ahead unseen on the last hosts the helm pushed, and the helm checks the host
  it names at the create against its own live host status (`create_target` and `host_client` in
  `crates/farhelm-helm/src/sessions.rs`: a host it no longer knows is refused as not found, one that is not connected as
  a conflict); a refusal reveals New with its words, as any refused quick launch does. A stale host list can only change
  which host the draft names, and the helm's check stands behind that, so this is kept rather than refusing a launch the
  helm may accept. No test covers it.

- **The template list follows pushed templates.** The old list showed the names its read found when it opened. Here the
  rows are the templates the helm pushed, read on every frame, so a template deleted elsewhere loses its row as soon as
  the push arrives, and an added one appears. The choice is kept by name, and what is drawn selected is latched on each
  frame, so neither moves it to another template: a deleted chosen template stays chosen with no row drawn selected, and
  Enter then opens New with "Not launched. Template … is no longer available", as the old list's Enter did and SPEC.md
  asks, also when no row matches any more (SPEC.md's no-match sentence says so since 2026-11-15). There is no "Loading
  templates…" state and no refresh of the bindings on opening, since both are pushed state the window already holds.
  Tested by `session_list::quick_switcher::template_list_tests::a_pick_uses_the_template_as_the_store_holds_it_now`.

## Superseded

- **The phone-width one-pane layout** (below 620 pixels wide: the list or the editor, with a "‹ Templates" back link).
  The desktop window cannot be made narrower than 720 pixels, so the layout has no place; SPEC.md's sentence was
  reworded when the dialog was ported (decided by the migration's orchestrator on 2026-11-14).
- **Retry beside a failed template list.** The old dialog read the list itself and offered Retry when the read failed.
  The GPUI app's templates are a collection the helm pushes and pushes again on every change, so there is nothing for
  the dialog to ask for: it says "Templates unavailable:" with the helm's words and keeps showing the last templates.
  Tested by `templates::dialog_tests::an_unavailable_list_says_so_and_keeps_the_last_templates`.
- **A deletion whose outcome is unknown.** The old Delete said "The template may have been deleted; reloading the
  template list" when its request went unanswered. A command on the helm channel is answered, or the connection resets,
  which closes the dialog, so there is no such case.
- **A preference write whose reply is lost.** The old list re-read the preferences when a ⚡ or key write went
  unanswered ("a failed number-key save is reported and never saved later", its lost-reply case). A command on the helm
  channel is answered, or the connection resets, which closes the dialog, so there is no such case.
- **The number-key menu's browser timing.** The old cases for its pointerdown toggle ("number popup toggle stays open
  during pointerdown and closes on click"), its delayed close keeping another popup's focus, and its closing when the
  dialog scrolls or resizes guarded the web view's event order and layout. A GPUI popover is a layer whose clicks, focus
  and placement the layer coordinator owns (the foundations parity file records the scroll and resize rule).
- **The old quick-launch dispatcher's fences.** Its cases for a press snapshot handed between sequences, a newer draft,
  and Settings or Rename retiring an older quick-launch read guarded reads the old app made for each press and the
  windows they left open. The GPUI app takes each press in order on one thread, from the pushed store, with no read but
  the catalog's (`templates::dispatch` module docs). What remains is tested by
  `templates::dispatch_tests::presses_during_a_preparation_wait_and_its_result_respects_the_moved_on_rule`.
- **A quick launch's retry after a lost reply.** The old app retried a create whose reply was lost with the same intent
  key, reapplying the template if it had changed. The GUI sends no create keys, and a duplicate around a lost connection
  is accepted (SPEC.md, "Creation"; the launcher parity file's `tests/terminal-create-idempotency.spec.ts` row).
- **Save as template's refusal after focus was lost.** The old panel disabled its name field while the save was out,
  which dropped focus to the page's body, and its "lost" case checked that a refusal put focus back in the name field.
  Here the panel holds still without giving up focus (its name field read-only, its buttons busy), so focus is never
  lost to recover.

## Real-window drives on the test Mac

The shortcuts exist in their Command form only in the Mac app, so they were driven once in a real window on the test
Mac, through the GUI bridge, on 2026-11-15:

- At `1b631967`: ⚡ (Cmd+P), Cmd+1, Cmd+X then 1 and Enter, and Cmd+X then Escape.
- At `3933498b`: naming after Cmd+P and after Cmd+1; Cmd+X in New's search box giving the apply pill; Cmd+X with a
  selection in the folder field cutting it.
- At `062c1db8`: the Cmd+J list, its filter, an unseen launch followed by naming, a partial template opening New with
  its banner, nothing with New open, Escape, Enter with no match, and leaving the Hosts panel.

The bridge sends key events as the app's own, so these drives prove the chords' Command forms and their effects, not the
physical-key matching of a real keyboard's events (`actions`' physical-key table). That stays a person's check, at a
real keyboard on a Mac and on Linux.
