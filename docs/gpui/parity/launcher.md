# Parity: Launcher

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Creation, Sandbox sessions, Managed checkouts, Container sessions, Lifecycle operations (Restart with and
Replace with).

The Replace with cases of `tests/replace.spec.ts` and `tests/terminal-restart.spec.ts` are this area's too, although
those specs' rows sit in `sidebar-and-session-view.md` with the rest of their tests.

`tests/host-picker.spec.ts` is split. Its five host picker tests are this area's; its sixth, "quick switcher draws a
session's chosen host mark", is in [Sidebar and session view](sidebar-and-session-view.md).

A case that is only layout or wording (how a row is drawn, the words of a help text) gets no test of its own: its entry
names the screenshot scene that shows it, by its id in `docs/gpui/screenshot-scenes.json`, or says it is layout or
wording, not separately tested, and why (decided by the migration's orchestrator on 2026-11-16). Every other case an
entry lists as not yet ported is still to get a test.

### `tests/launcher-enter.spec.ts`

Tests: 25. Covers: Clone search Enter picks an unmatched name before a second Enter launches.

partly: `launcher::view_tests::a_refused_press_says_why_beside_the_main_action` (Enter in a single-line field),
`launcher::view_tests::enter_on_a_harness_chip_chooses_it_and_presses_launch`,
`launcher::view_tests::custom_opens_its_flags_editor_and_enter_there_only_chooses`,
`launcher::view_tests::enter_on_the_commands_agent_type_and_resume_presses_launch`,
`launcher::launch_tests::a_held_enter_in_a_field_never_launches`; "launcher shows a generic refusal after its target
changes" in `launcher::launch_tests::a_refusal_after_the_host_moved_installation_is_generic`; the search cases ("Clone
search Enter picks an unmatched name before a second Enter launches", "launcher search explains empty results and Enter
reopens before picking") in
`launcher::search_view_tests::{unmatched_text_and_enter_name_the_session, escape_closes_the_results_and_keeps_the_query, a_scoped_miss_says_why_and_enter_changes_nothing, enter_on_an_empty_box_presses_launch_and_spaces_do_not, only_a_fresh_enter_on_an_empty_box_launches}`;
Browse in "launcher action-button Enter keeps reset browse and cancel" in
`launcher::browse_tests::browse_lists_one_level_and_taking_a_folder_chooses_it` (by click; the listing's buttons are
ordinary buttons, whose Enter is the component's); superseded: "launcher container Enter waits for visible session setup
loading", by the templates area's deviation "A switcher template pick shows no loading state"
(`templates-and-quick-launch.md`), and the seven retry-key cases ("launcher choice retries preserve keys only for
unchanged values", "unchanged launcher tab, harness, permissions and default model retain the retry", "launcher model
reselection retains retries while real scalar changes replace them", "ordinary title no-ops preserve retries without
trimming nonblank names", "valid ${editor} spacing preserves raw text and retry words", "effective Guarded reacceptance
preserves a template-default request", "one native same-owned ${kind} replacement retains its retry"), since the GUI
keeps no launch request keys (SPEC.md, "Losing the connection to the helm"); their non-key halves are in the retry-key
bullet below

- "launcher gap keeps focus and container Enter explains an incomplete draft": a click on the empty space of a dialog
  that keeps focus leaves focus on the control in use, in
  `layers_tests::the_page_behind_a_dialog_takes_no_clicks_and_empty_space_keeps_focus`; the launcher is such a dialog
  (SPEC.md, "Opening and closing"), though no launcher-level test clicks its empty space. The container Enter half is
  superseded, since such a dialog's container never takes focus, and the double click that selected no text has no
  counterpart: the launcher's labels are not selectable.
- "launcher text-field Enter explains an empty command beside Launch": the reason in
  `launcher::request_tests::the_command_tab_needs_a_command_and_a_yolo_answer`, through the single-line fields' shared
  Enter, which `launcher::view_tests::a_refused_press_says_why_beside_the_main_action` drives from the folder field.
- "launcher shows the helm's refusal beside Launch":
  `launcher::launch_tests::a_refused_launch_brings_the_launcher_back_with_the_helms_words`, with the alert's place under
  the actions in `components::tests::dialog::notices_below_sit_under_the_answers`.
- "launcher effort dropdown keyboard edits submit the new value exactly once": past five efforts the effort is the
  shared select (`launcher::view_tests::{many_efforts_are_a_select, five_effort_options_are_still_segments}`), whose
  keys and Enter are in
  `components::tests::select::{the_select_keys_open_move_choose_and_close_as_a_native_pop_up, enter_on_a_closed_select_with_a_main_action_runs_it_with_the_value_shown}`.
  Its arrows open the list, where the old native select's changed the value in place. Its composing Enter is superseded
  by the choice controls' shared Enter rule, which says no input method can compose into them
  (`components::enter_choice`'s documentation in `crates/gui/farhelm-gui/src/components/mod.rs`; decided by the
  migration's orchestrator on 2026-11-16).
- "launcher choice-button Enter applies then launches" (beyond the harness chip) and "launcher choice Enter explains an
  incomplete draft": the effort and permission segments in
  `components::tests::radio_group::a_button_group_makes_each_option_a_tab_stop_chosen_with_enter_or_space`, the
  launch-kind tabs in
  `components::tests::tab_strip::the_tab_strip_moves_with_the_arrows_and_enter_chooses_then_launches`.
- "launcher value-control Enter keeps the shown choices" (beyond the agent type and Resume): the host in
  `launcher::view_tests::the_host_picker_browses_with_arrows_and_enter`, the YOLO answer in
  `components::tests::radio_group::space_chooses_a_focused_radio_and_enter_goes_on_to_the_owner`.
- "launcher action-button Enter keeps reset browse and cancel" and "launcher action-button Enter keeps retry and
  Launch": Reset choices in `launcher::view_tests::reset_choices_empties_the_wrapper_field`, Cancel in
  `launcher::view_tests::cancel_close_and_escape_close_the_launcher`, Retry in
  `launcher::view_tests::a_failed_catalog_read_says_so_and_retry_reads_it_again` and Launch in
  `launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`, each by click; Enter on a focused button
  runs it once in `components::tests::button::enter_or_space_on_a_focused_button_runs_it_once` and
  `focus_tests::enter_and_space_activate_a_focused_button_and_a_held_key_does_not`.
- "launcher choice Enter leaves the YOLO question's Enter rules intact":
  `launcher::yolo_tests::{a_yolo_refusal_asks_the_question_with_cancel_focused, cancel_starts_nothing_and_keeps_the_launcher}`,
  and the inline question's own answers in
  `prompt_tests::an_inline_question_answers_once_and_returns_focus_inside_its_dialog`.
- "launcher repeated Enter never launches", on a closed select: the select takes Enter through the choice controls'
  shared rule (`components::tests::select::enter_on_a_closed_select_with_a_main_action_runs_it_with_the_value_shown`),
  whose held Enter doing nothing is that rule's own test,
  `components::tests::enter_choice::an_areas_own_tab_takes_the_enter_rule_through_the_helper`.
- "launcher repeated Enter never launches", on the closed host button, which handles Enter itself:
  `launcher::launch_tests::a_held_enter_on_the_closed_host_button_never_launches`.
- The retry-key cases' non-key halves: taking a search row only edits
  (`launcher::search_view_tests::a_word_and_enter_choose_it_and_empty_the_box`), a model change clears an effort the
  model does not offer (`launcher::draft_tests::model_choices_follow_their_owning_agent`), a blank name is sent as none
  and a nonblank one byte for byte
  (`launcher::request_tests::{an_agent_create_body_carries_the_draft, a_command_create_body_sends_the_command_as_written}`),
  a template's `permissions: null` on Codex launches Guarded
  (`launcher::search_view_tests::a_tl_reset_of_codexs_permission_launches_guarded`), and the wrapper goes out as the
  words it parses to (`launcher::request_tests::an_agent_create_body_carries_the_draft`,
  `farhelm_proto::launch::tests::wrapper_prefix_preserves_raw_spelling_and_join_roundtrips`), with the Custom flags'
  words shown in their editor (`launcher::view_tests::custom_opens_its_flags_editor_and_enter_there_only_chooses`).
- "valid ${editor} spacing preserves raw text and retry words", its custom flags variant: a New create carrying typed
  Custom flags as their words, a trailing space kept in the field while the same create goes out again, and a further
  word changing it, in `launcher::custom_flags_tests::a_trailing_space_keeps_the_words_and_a_new_word_changes_them`.
- "launcher ${attempt} explains an incomplete setup beside Launch", its half before any press: the reason as Launch's
  hover help, and Launch's own help back once the draft is complete, in
  `launcher::view_tests::a_refused_press_says_why_beside_the_main_action`, which also presses. No test reads the refused
  look, which the same refusal drives.
- "launcher Enter names invalid custom permission flags", empty and blank flags: Launch's hover help and Enter in the
  flags field naming the flags, with nothing sent, in
  `launcher::custom_flags_tests::{empty_flags_refuse_launch_and_say_why, blank_flags_refuse_launch_and_say_why}`; the
  unclosed quote's reason is `launcher::request_tests::refusals_name_the_first_thing_to_fix`.
- "launcher effort dropdown preserves an unsupported inherited choice until a native edit": a Clone's carried Max
  offered in its place, refusing the press, and no longer offered once High is picked, which then launches, in
  `launcher::view_tests::a_carried_effort_the_model_does_not_offer_stays_offered`. The old case's efforts were a native
  select; these are segments, whose choosing is the same draft edit.

### `tests/launcher-tabs.spec.ts`

Tests: 6. Covers: New opens on the agent tab, and each tab keeps its own fields and draft.

partly: `launcher::view_tests::the_tabs_switch_fields_and_keep_each_draft`,
`launcher::draft_tests::each_tab_keeps_its_own_draft_across_switches`; the remembered wrapper prefix on an agent switch,
shown escaped when it holds an invisible character, in
`launcher::view_tests::an_agent_switch_shows_an_unsafe_remembered_prefix_escaped`; search moving to the Agent tab in
`launcher::search_view_tests::a_model_taken_on_the_command_tab_switches_to_the_agent_tab`; not yet ported: a folder row
keeping the typed command ("search on the command tab switches to the agent tab only for an agent choice"; a folder row
keeping the tab is in the bullet below), and an unparsable wrapper left on the Agent tab not blocking Save as template
on the Command tab ("a dormant invalid wrapper does not block saving a command template"; its test would sit with Save
as template's in `templates::save_tests`)

- New opening on the Agent tab with focus in the search box:
  `launcher::view_tests::new_opens_on_the_agent_tab_with_focus_in_the_search_box`.
- "search on the command tab switches to the agent tab only for an agent choice", its `name:` half: naming the session
  keeps the Command tab and its typed command, in
  `launcher::search_view_tests::a_name_row_on_the_command_tab_keeps_the_tab_and_its_command`.
- "search on the command tab switches to the agent tab only for an agent choice", a folder row: a saved folder taken
  from search on the Command tab sets the folder and keeps the tab, in
  `launcher::search_view_tests::a_saved_folder_from_search_leaves_a_managed_checkout` (its draft holds no command, so
  the command's survival is not asserted).
- "the arrow keys move between the launch-kind tabs": Right and Home in
  `launcher::view_tests::the_tabs_switch_fields_and_keep_each_draft`, and the strip as one tab stop in
  `components::tests::tab_strip::the_tab_strip_moves_with_the_arrows_and_enter_chooses_then_launches`.
- "agent wrapper input refuses unfinished text and submits raw shell words": the line under the field in
  `launcher::view_tests::a_wrapper_prefix_that_does_not_parse_is_said_at_once`, the refused press ("Correct the wrapper
  prefix before launching", where the old press gave the parse error, "Invalid wrapper prefix: …", which the line under
  the field now gives) in `launcher::request_tests::refusals_name_the_first_thing_to_fix`, the body's wrapper in
  `launcher::request_tests::an_agent_create_body_carries_the_draft` and its raw spelling in
  `farhelm_proto::launch::tests::wrapper_prefix_preserves_raw_spelling_and_join_roundtrips`.

### `tests/launch-button-alignment.spec.ts`

Tests: 1. Covers: launcher summary and actions stay pinned at ${viewport.width}x${viewport.height}.

replaced: the pinned footer is the dialog frame's, tested for every dialog; the launcher's summary beside its actions is
layout, shown in the `app-launcher-scrolled-end` scene and not separately tested (nothing checks that the summary and
the actions do not overlap); all three old window sizes (1280 by 540, 390 by 844 and 360 by 300) are below the minimum
window (Deviations)

- `components::tests::dialog::{a_long_dialog_body_scrolls_and_the_answers_stay_visible, a_frame_too_tall_for_the_window_scrolls_as_a_whole, a_dialogs_answers_wrap_in_a_narrow_window_in_every_footer_shape}`:
  the body scrolls under a footer that stays inside the frame, a frame too tall for its window scrolls as a whole, and
  the answers wrap in a narrow window, the launcher's footer shape (a lead beside the answers) included.
- `launcher::view_tests::the_footer_names_what_launch_would_start`: the Command tab's "Launch Command · …".

### `tests/launch-progress.spec.ts`

Tests: 5. Covers: launcher status waits two seconds, counts up and ends on a real refusal.

replaced: the two-second threshold, the count, the one announcement and the refusal ending it in
`launcher::launch_tests::{a_slow_launch_shows_its_progress_pill_after_two_seconds, a_quick_reply_shows_no_progress_pill, the_progress_pill_counts_in_an_inactive_window}`;
the deadline case's outcome (an unknown outcome after the helm's bound; the pill during such a wait is the first test's)
in
`launcher::launch_tests::{a_timed_out_create_comes_back_without_not_launched, a_timed_out_create_the_user_moved_on_from_has_its_own_notice}`,
without the old retry-key promise, since the GUI sends no keys; superseded: "a deadline after successful headers still
reports an unknown launch outcome", which guarded a browser transport's timeout after a reply's headers, since the GUI
has no per-request deadline (SPEC.md, "Creation"); the quick launch and template shortcut pill case is the templates
area's

### `tests/launch-refusal.spec.ts`

Tests: 7. Covers: a quoted command placeholder is refused by native admission.

partly: `launcher::view_tests::a_refused_press_says_why_beside_the_main_action`,
`launcher::request_tests::the_command_tab_needs_a_command_and_a_yolo_answer`,
`launcher::launch_tests::a_refused_launch_brings_the_launcher_back_with_the_helms_words`; the folder cases ("a missing
folder is refused as an alert and marks the folder until the destination changes", "folder checks warn before Launch and
never prevent an attempt", "a stale check is dropped and checks remain serial while a reply is held", "an inconclusive
folder check shows no warning") in
`launcher::folder_check_tests::{a_missing_folder_is_said_under_the_field, checks_are_serial_and_a_stale_answer_is_discarded, an_inconclusive_answer_says_nothing, a_folder_refusal_marks_the_field}`
and `launcher::view::folder_check::tests::only_the_supervisors_folder_words_name_a_problem` (an answer for an earlier
host or connection is dropped by the same key rule as an earlier path, which only the path case tests; a refusal's mark
clears with the refusal, which any edit drops, tested for the refusal alert by the launch tests); the field's mark is a
warning-colored border without an accessible "invalid" state, which GPUI's text field does not expose (a deviation);
superseded: "failed asynchronous launch preparation is an announced refusal", whose preparation was minting a launch
request key, which the GUI does not do (SPEC.md, "Losing the connection to the helm"); not yet ported: a Save as
template refusal after a refused Launch press taking the "Not launched." alert away ("a save-as-template refusal is not
labelled Not launched"; its test would sit in `templates::save_tests`)

- "a save-as-template refusal is not labelled Not launched", without a press before it:
  `templates::save_tests::a_wrapper_that_does_not_parse_refuses_on_a_plain_line`.

### `tests/cursor-composer.spec.ts`

Tests: 1. Covers: Cursor launches without claiming session tracking.

replaced: `launcher::view_tests::each_agent_type_offers_its_own_permission_vocabulary`,
`launcher::view_tests::a_switch_that_clears_a_choice_says_so` (the note, and its absence on the Command tab), and
`launcher::request_tests::each_agent_types_create_carries_only_what_it_takes` (the create body: the typed model, no
effort, YOLO)

### `tests/goose-pi-composer.spec.ts`

Tests: 2. Covers: Goose and Pi permission transitions update the rendered draft.

replaced: `launcher::view_tests::each_agent_type_offers_its_own_permission_vocabulary`; the transitions of "Goose and Pi
permission transitions update the rendered draft" (Goose's Smart approve, Chat or Approve becoming Pi's YOLO with the
notice, by Pi's button or by Pi's model from the full list, a remembered Smart approve replaced silently, a model id
both offer staying the one picked as Pi's and taking Pi's efforts, and an effort kept across another Pi model) in
`launcher::draft_tests::an_agent_switch_lands_on_a_permission_the_new_agent_offers`; "workspace trust is absent despite
old remembered value ${rememberedTrust}" (SPEC.md, "Creation": "the old remembered trust value is neither read nor
written"): a Clone of a Pi, Codex, Claude or Muse launch that stored trust creating without it in
`launcher::request_tests::a_stored_trust_choice_is_never_sent_again`, and the retired `trust:` labels offering nothing
in `launcher::search_tests::unknown_labels_and_command_words_only_name_the_session`; the GUI has no trust control and
reads no remembered trust

### `tests/grok-composer.spec.ts`

Tests: 1. Covers: the Grok composer clears and hides unsupported model and effort choices.

replaced: `launcher::view_tests::each_agent_type_offers_its_own_permission_vocabulary` (no effort, no model field), and
`launcher::request_tests::each_agent_types_create_carries_only_what_it_takes` (Codex's typed model and High cleared on
the switch to Grok, and Grok's body with no model or effort and Guarded)

### `tests/omp-composer.spec.ts`

Tests: 2. Covers: the OMP composer offers OMP's own vocabulary and the row shows the effective permission.

replaced: `launcher::view_tests::each_agent_type_offers_its_own_permission_vocabulary`; the OMP create body (a catalog
model, Max and Approve) in `launcher::request_tests::each_agent_types_create_carries_only_what_it_takes`; the row's
effective permission ("an OMP session row shows the OMP glyph and its actual permission") is the session list's, where
the OMP mark, the Approve row's shield and its description are in
`session_list::row_marks_tests::{every_agent_type_draws_the_mark_named_for_it, the_shield_follows_the_launch_and_draws_its_mark, an_agent_launch_description_gives_the_mode_its_help_and_the_stored_command}`,
the second also drawing the YOLO shield for an OMP row with no stored permission

### `tests/opencode.spec.ts`

Tests: 2. Covers: OpenCode keeps composer controls while offering default and explicit Zen models.

replaced: "typing a model never switches the selected harness, and bare OpenCode names stay OpenCode's" in the bullets
below; "OpenCode keeps composer controls while offering default and explicit Zen models", Codex's effort dropped on the
switch to OpenCode and a typed model sent as written with YOLO, in
`launcher::request_tests::each_agent_types_create_carries_only_what_it_takes`, its controls in
`launcher::view_tests::each_agent_type_offers_its_own_permission_vocabulary`

- `launcher::model_tests::bare_opencode_names_stay_opencode_models`: under OpenCode a bare name is OpenCode's custom id,
  and OpenCode's qualified id typed under Codex is not applied.
- `launcher::view_tests::enter_on_typed_text_applies_a_custom_id_or_says_who_owns_it`: such an id says which agent
  offers it, and the agent does not switch.

### `tests/github-checkout-composer.spec.ts`

Tests: 18. Covers: managed checkout offers no retired container controls.

partly: the destination switch, the repository field and its suggestions, the preview with its invalidation, the inline
checkout-folder setup, a lost race and the guarded answer's wait for a new preview in
`launcher::checkout_tests::{managed_checkout_hides_the_folder_until_a_repository_is_chosen, suggestions_are_read_after_typing_and_enter_picks_one, a_typed_pair_is_chosen_when_discovery_fails, a_chosen_repository_is_previewed_before_it_launches, a_refused_preview_says_why_and_shows_no_path, what_a_preview_answers_for_asks_again_and_late_answers_never_apply, a_preview_from_another_installation_is_refused, the_destination_switch_keeps_the_folder_and_typed_text_never_chooses, a_checkout_launches_only_with_a_repository_and_a_current_preview, a_checkout_create_carries_its_preview, a_checkout_ignores_an_untouched_copied_title, a_refused_checkout_launch_comes_back_with_focus_in_the_search_box, a_host_without_a_checkout_folder_offers_the_setup_inline, a_lost_race_takes_a_new_preview_and_sends_nothing, the_guarded_answer_waits_for_the_new_preview_then_launches_once, escape_closes_the_suggestions_before_the_launcher}`;
unsafe inherited repository text is not carried at all
(`a_clone_carries_only_a_repository_the_shared_parser_reads_back`; see Deviations); the `gh:` search's discovered
repositories and valid typed pair, its scan note under the search box, Launch held until a pick and a field pick
retiring the query in
`launcher::checkout_tests::{gh_search_offers_discovered_repositories_and_a_valid_typed_pair, a_gh_query_shows_its_scan_note_and_holds_launch_until_a_pick}`;
superseded: "managed checkout offers no retired container controls" (SPEC.md, "Retired containerized sessions": the old
switch is gone, and a Clone carrying a retired container is refused,
`launcher::request_tests::a_retired_container_launch_is_refused`), the keyed retries ("ambiguous checkout retries retain
the exact original body after a later refusal" (926), "a fresh-checkout replace-with retry carries the precondition
shown at that press" (976) and the ambiguous-retry variants of 1383), since the GUI sends no launch request keys (see
Deviations), and the undispatched-preview variants of 1383, a command-line configuration change while New stays open, by
the launch-history deviation; "managed checkout setup notes use ordinary helper styling" (418) is layout, shown in the
`app-launcher-managed-checkout` scene: the inline setup field stands in for the note while it shows, drawn as a
deviation below says

- The highlighted suggestion stays in view while the arrows wrap (161).
  `launcher::checkout_tests::repository_arrows_keep_the_highlight_visible_through_wrapping`.
- Editing the name refreshes the preview without reading suggestions again (212).
  `launcher::checkout_tests::editing_the_checkout_name_reads_no_repository_suggestions`.
- The configuration-change half of 212 is not yet ported. An external configuration change by itself does not make the
  desktop launcher reread history, so the browser's trigger for rechecking the open setup field is absent.
- Focusing Repository closes the composer's search. After a failed scan with no matches, Enter on the "No repository
  matches; enter owner/repo" row leaves the draft unselected, and Enter reopens the dismissed list (462).
  `launcher::checkout_tests::repository_focus_closes_search_and_enter_on_no_match_chooses_nothing`. The lifetime
  explanation appears in `app-launcher-managed-checkout`. The failed scan note's words are checked in
  `launcher::checkout_tests::suggestions_are_read_after_typing_and_enter_picks_one`. Its warning ink has no scene.
- A host switch drops the old preview and checks setup on the new host while the repository stays selected (1297).
  `launcher::checkout_tests::switching_hosts_drops_the_old_preview_and_checks_the_new_hosts_setup`. The fake helm
  answers in command order, so the test checks that the setup field appears independently of the replacement preview.
  The field's own configuration read must wait behind that preview.

- "manual checkout needs selection and explicit resubmit after a proven refusal" (577), beyond the lost race: no Browse
  in a checkout (`launcher::browse_tests::browse_is_not_offered_where_the_folder_cannot_change`), and an invalid `gh:`
  query a warning that Enter cannot take (`launcher::search_tests::empty_results_say_why_and_warn_only_on_a_miss`).
- "guarded checkout answer waits for preview and is cancelled by edit / close" (627):
  `launcher::checkout_tests::{an_edit_cancels_the_guarded_answers_waiting_launch, closing_the_launcher_cancels_the_guarded_answers_waiting_launch, another_preview_cancels_the_guarded_answers_waiting_launch}`.
- "a repository recent selects fresh intent rather than its previous cwd" (1132):
  `launcher::recents_view_tests::a_checkout_setup_chooses_its_repository`.
- "Escape dismisses repository suggestions before closing the composer" (1267), whose old case used the search box's
  results: `launcher::search_view_tests::escape_closes_the_results_and_keeps_the_query`.
- "inline checkout folder setup refreshes discovery and preview and respects host overrides" (212), its refused relative
  folder: `settings::tests::a_refused_folder_keeps_its_draft_and_reason_across_sections` (the launcher reuses Settings'
  checkout folder field).
- "a fresh-checkout replace-with retry carries the precondition shown at that press" (976), its non-key half:
  `launcher::replace_with_tests::replace_with_follows_the_source_and_sends_the_guard_it_shows`.

### `tests/composer-word-search.spec.ts`

Tests: 6. Covers: launcher refusals use warning colors in ${appearance} appearance.

replaced: "composer word search drives a structured launch" and "empty composer search does not launch an incomplete
selection" in
`launcher::search_view_tests::{a_word_and_enter_choose_it_and_empty_the_box, enter_on_an_empty_box_presses_launch_and_spaces_do_not, only_a_fresh_enter_on_an_empty_box_launches}`;
"name and host labels apply one draft choice without launching" in
`launcher::search_view_tests::{unmatched_text_and_enter_name_the_session, host_and_folder_rows_set_the_destination}` and
`launcher::search_tests::name_and_host_labels_offer_only_their_own_rows`; the command tab's search in
`launcher::search_view_tests::a_model_taken_on_the_command_tab_switches_to_the_agent_tab`; which messages are warnings
in `launcher::search_tests::empty_results_say_why_and_warn_only_on_a_miss`; the drawn warning colors are layout: the
"Not launched." alert in both appearances is the `app-launcher-refusal` scene (the scenes run light and dark), and the
search box's warning line is not separately tested, since no scene types a query that warns and the test above fixes
which lines are warnings; the alert role, whole keyboard path and dormant agent draft's create are in the bullets below

- "launcher refusals use warning colors in ${appearance} appearance", its alert role:
  `launcher::view_tests::the_refusal_beside_launch_is_an_alert`.
- "composer word search drives a structured launch", the whole keyboard path in one launcher, including effort's default
  reset, Guarded replacing YOLO and the create carrying the choices:
  `launcher::search_view_tests::the_whole_keyboard_path_builds_a_launch_and_sends_it`.
- "New's command tab keeps the agent draft dormant and preserves the raw invocation", its create carrying no agent
  launch while Claude and YOLO stay on Agent, and the command sent byte for byte:
  `launcher::request_tests::a_command_launch_leaves_the_agent_draft_dormant`.
- "New's command tab keeps the agent draft dormant and preserves the raw invocation", a folder row:
  `launcher::search_view_tests::a_saved_folder_from_search_leaves_a_managed_checkout` (a saved folder taken on the
  Command tab sets the folder and keeps the tab).
- "Tab from the open model list reaches the next launcher control":
  `launcher::view_tests::leaving_the_model_field_closes_its_list` (Tab leaves the field and closes the list; which
  control takes focus is not asserted, and the focus invariant every test step checks keeps it inside the launcher).

### `tests/custom-permissions.spec.ts`

Tests: 9. Covers: custom permission search focuses literal words and switching agents clears them.

replaced: `launcher::view_tests::custom_opens_its_flags_editor_and_enter_there_only_chooses`; "custom permission search
focuses literal words" in `launcher::search_view_tests::custom_from_search_moves_focus_to_its_flags`; an agent switch
clearing the flags in `launcher::draft_tests::custom_flags_stay_with_their_agent_and_their_choice`; the key half of
"queued custom permission edits cannot change an in-flight launch" is superseded, since the GUI keeps no launch request
keys (SPEC.md, "Losing the connection to the helm"); the row's half of "${action} preserves custom words and shows the
stored YOLO provenance" is the session list's

- "queued custom permission edits cannot change an in-flight launch", its non-key half: nothing can edit the launcher
  while its create is out (`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`), and a refusal
  brings the draft back unchanged
  (`launcher::launch_tests::a_refused_launch_brings_the_launcher_back_with_the_helms_words`).
- "custom template words round trip into the launcher and clear on a stacked permission": a Custom template's words
  seeded and cleared by a later template's permission in
  `templates::apply_tests::custom_words_are_cleared_by_a_later_templates_permission`, a later Custom starting empty in
  `launcher::draft_tests::custom_flags_stay_with_their_agent_and_their_choice`, and the template editor's half in
  `templates::editor_tests::{custom_approvals_carry_their_words, custom_words_that_do_not_parse_refuse_save_and_each_template_shows_its_own}`.
- "reset then agent search cannot restore another agent's custom flags": Reset choices ending Claude's flags while the
  remembered YOLO stays, and Codex's Custom then starting empty and launching only its own words, in
  `launcher::custom_flags_tests::reset_then_another_agents_custom_starts_empty`.
- "same-agent search and chips preserve custom permission drafts byte for byte": a model and the agent's own row from
  search, its chip and Enter on the chip keeping unclosed and spaced flags as typed, in
  `launcher::custom_flags_tests::{same_agent_choices_keep_unclosed_flags_as_typed, same_agent_choices_keep_spaced_flags_as_typed}`;
  the closing switch to Codex is the draft test above.
- "custom model acceptance keeps compatible effort with incomplete permission flags":
  `launcher::custom_flags_tests::{a_custom_model_keeps_its_effort_beside_empty_flags, a_custom_model_keeps_its_effort_beside_unclosed_flags}`,
  with High chosen from segments where the old case used the native select (the shared select's keys are its own
  tests').
- "${action} preserves custom words and shows the stored YOLO provenance", for clone and replace-with, the launcher's
  half: the field's escaped spelling, the stored words sent raw in Clone's create and Replace with's replace, and an
  edit's words after a refusal, in
  `launcher::custom_flags_tests::{clone_sends_the_stored_words_then_an_edits, replace_with_sends_the_stored_words_then_an_edits}`.
- "custom permission words reach the real YOLO refusal with their own explanation": the create's Custom words and the
  question's custom-flags reason in
  `launcher::custom_flags_tests::custom_flags_reach_the_yolo_question_with_their_own_reason`; its Cancel is
  `launcher::yolo_tests::cancel_starts_nothing_and_keeps_the_launcher`.

### `tests/guarded-permissions.spec.ts`

Tests: 2. Covers: guarded choices explain each agent and Cursor keeps its own setting.

partly: `launcher::view_tests::each_agent_type_offers_its_own_permission_vocabulary` (Cursor keeps its own setting);
each agent's Guarded explanation, and Cursor's own-setting one, is its segment's hover help, which is wording, not
separately tested: the words are the shared permission words, whose Codex texts
`session_list::row_marks_tests::an_agent_launch_description_gives_the_mode_its_help_and_the_stored_command` asserts; not
yet ported: the "About permission modes" button opening the documentation's permission modes page; "guarded and older
own-setting shields show their actual launch provenance" is the session list's, in
`session_list::row_marks_tests::{the_shield_follows_the_launch_and_draws_its_mark, an_agent_launch_description_gives_the_mode_its_help_and_the_stored_command}`
(Codex's Guarded and own-setting shields, and their descriptions with the stored command)

### `tests/permission-switch.spec.ts`

Tests: 7. Covers: ${query} selects YOLO before choosing a harness.

partly: the search cases in `launcher::search_view_tests::yolo_from_search_before_an_agent_type` and
`launcher::search_tests::bare_yolo_and_guarded_work_before_a_harness_on_the_agent_tab_only`; the banner, its words, Undo
restoring the pre-switch draft after later edits, Dismiss, a later switch taking it away, a direct choice raising none,
and its eight seconds held by hover and focus in
`launcher::escalation_tests::{an_escalating_switch_shows_the_banner_and_undo_restores_the_draft, dismiss_keeps_the_switch, a_later_switch_clears_the_banner, the_banner_shows_eight_seconds_unless_hovered_or_focused, a_direct_permission_choice_raises_no_banner}`;
a template batch in `launcher::search_view_tests::a_tl_row_that_escalates_raises_the_banner`; the rules (passive and
remembered permissions, the first agent: "first harness escalation warns after Guarded chosen by ${source}" through the
button, and "first harness does not escalate passive ${remembered ?? "default"} permission" for a remembered Guarded) in
`launcher::draft_tests::the_first_agent_escalates_only_a_deliberate_choice` and
`templates::apply_tests::escalation_is_judged_once_for_the_batch`; the slide and outline, and both under reduced motion,
in `components::tests::transition`; the passive default variant of "first harness does not escalate passive ${remembered
?? "default"} permission", and the Undo of "first harness escalation warns after Guarded chosen by ${source}" bringing
back the deliberate Guarded with no agent, so that choosing the agent again warns again, in
`launcher::draft_tests::undo_of_the_first_agents_warning_keeps_the_choice_deliberate`; not yet ported: the model
picker's path (258's second run) drawn, the banner shown after a click on another agent's model in the list; its draft
half, a cross-agent model pick raising the banner and Undo restoring the draft exactly, is
`launcher::draft_tests::{a_cross_agent_model_pick_can_raise_the_banner, undo_after_a_cross_agent_model_pick_restores_the_draft_exactly}`;
the drawn band is layout, shown in the `app-launcher-escalation-banner` scene

- "OMP defaults to Guarded and escalation Undo restores the pre-switch draft": OMP's Guarded default in
  `launcher::view_tests::each_agent_type_offers_its_own_permission_vocabulary`, and an explicit Guarded kept on a switch
  to OMP in the shared rule's `farhelm_proto::launcher::tests::switching_a_guarded_agent_to_omp_keeps_guarded`.
- "permission banner lifetime pauses for hover and focus and survives reduced-motion styling", the outline's lifetime
  under reduced motion: `launcher::escalation_tests::under_reduced_motion_the_outline_lasts_as_long_as_the_banner`.

### `tests/remembered-permissions.spec.ts`

Tests: 3. Covers: the remembered structured-launch permissions mode survives an open, a reset, and a later default
launch.

replaced: New preselecting the remembered mode and Reset choices keeping it in
`launcher::entry_tests::new_opens_beside_the_open_session` and
`launcher::draft_tests::reset_choices_returns_to_the_remembered_permission`; what the helm remembers and clears (a
harness's default or forced YOLO, Custom, a reply's claims, command launches) in the helm's
`sessions_tests::{a_successful_structured_launch_remembers_its_permissions_choice, remembered_defaults_follow_the_submitted_launch_not_the_reply}`
and `store::only_an_explicit_selection_shapes_history_and_remembered_defaults`; every link of the old real-launch runs
is tested on its own there, and the GUI reads the remembered mode only from the helm's pushed preferences; the GUI half
of "${label}'s default YOLO does not spill into the next harness" (Pi, OpenCode or Goose showing YOLO, then Codex
showing Guarded) in `launcher::draft_tests::a_default_yolo_does_not_follow_to_the_next_agent`

### `tests/model-defaults.spec.ts`

Tests: 1. Covers: ${label} can launch with its configured model.

replaced: each agent's create with its configured model (no model sent; YOLO for OpenCode, Goose and Pi, Guarded for
OMP) in `launcher::request_tests::each_agent_types_create_carries_only_what_it_takes`; the old case intercepted the
request rather than run a real agent. The Harness default row it starts from is in
`launcher::view_tests::the_model_list_filters_and_enter_picks_the_highlighted_row`

### `tests/named-conversations.spec.ts`

Tests: 3. Covers: initial prompt launcher follows capabilities and binds literal request retries.

partly: "New sends a named conversation outside the reusable launch and preserves refusal edits" is tested; the
retry-key half of "initial prompt launcher follows capabilities and binds literal request retries" is superseded, since
the GUI keeps no launch request keys (SPEC.md, "Losing the connection to the helm"); each agent's label for the
conversation field ("named conversation labels and reporting help follow the selected agent") is wording, not separately
tested, since one table in the launcher's view names it per agent and no behavior depends on the words; not yet ported:
which agents draw the prompt box and the conversation row, and neither drawn on the Command tab, and which agents show
the note that Farhelm will not capture a named start's conversation (Cursor, Goose and OpenCode), since the note follows
whether Farhelm captures that agent's conversation, and so whether Restart can find it later

- "New sends a named conversation outside the reusable launch and preserves refusal edits": the conversation at the
  create's top level, outside the launch, in `launcher::request_tests::an_agent_create_body_carries_the_draft`, and the
  draft back after a refusal in
  `launcher::launch_tests::a_refused_launch_brings_the_launcher_back_with_the_helms_words`.
- "initial prompt launcher follows capabilities and binds literal request retries", its pieces: a prompt the agent
  cannot take stays out of the body (`launcher::request_tests::a_prompt_the_agent_cannot_take_stays_out_of_the_body`),
  and Enter in the prompt adds a line the footer counts
  (`launcher::view_tests::the_footer_reads_an_agent_launchs_choices`).

### `tests/nonblocking-launch.spec.ts`

Tests: 17. Covers: a pending New keeps one immutable draft while sidebar, switcher and terminal work.

partly: `launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`,
`launcher::launch_tests::a_launch_the_user_waited_for_opens_its_session`,
`launcher::launch_tests::a_launch_the_user_moved_on_from_moves_nothing`,
`launcher::launch_tests::a_refusal_the_user_moved_on_from_waits_behind_a_notice`,
`launcher::launch_tests::a_repeated_enter_on_launch_still_opens_the_session`,
`launcher::entry_tests::new_waits_for_a_pending_launch_and_then_brings_back_its_refusal` (New refused while a launch is
pending, and bringing a refused launcher back), `launcher::launch_tests::launch_waits_for_a_host_change`,
`launcher::entry_tests::new_waits_for_a_host_change` (launches and host-registry changes exclude each other),
`launcher::launch_tests::{a_launch_that_does_not_open_reports_started, a_launch_that_opens_its_session_reports_nothing, a_slow_success_hands_the_pill_to_its_started_report}`
(the "Started" report),
`launcher::replace_with_tests::{the_yolo_question_of_a_replace_with_leaves_no_failure_on_the_source, replace_with_opens_its_replacement_only_where_the_source_was_open, a_refused_replace_with_comes_back_and_stays_on_the_source}`
(line 1551's YOLO cancellation, line 1037's refusal outcome for Replace with, and line 1359 in part),
`session_pane::tests::a_first_launch_the_user_moved_on_from_selects_nothing` (line 1194, New; a quick launch is the same
launch work); superseded: the retired variants of 237 and 1037, which revoked the old page's opening identity, since the
operation store, not the view, settles a reply (SPEC_impl.md, "Work that finishes later: the operation stores";
`operations_tests::work_started_from_a_view_settles_after_the_view_is_gone`), and 1782, since an automatic pick uses the
preferences as the helm last pushed them, with no read to go stale (SPEC.md, "Terminal experience"); the quick-launch
naming cases (567, 658) are the templates area's; not yet ported: Clone's and Replace with's half of 345 (the keyboard
staying in the session switched to; New's is ported below, and the three share the submit and its completion)

- 59 and 901, another session opening, its Stop going out beside the create, and a row menu opened meanwhile staying
  open through the create's success: `launcher::pending_tests::the_window_keeps_working_while_a_create_is_out`.
- 345, New's, and 59's typing half (the open session's terminal holding the keyboard while the create is out; what it
  does with the keys is the terminal's):
  `launcher::pending_tests::a_launchs_success_leaves_the_keyboard_in_the_session_switched_to`.
- 454 and 809's YOLO variant, the question waiting behind the notice, its answer resending the same draft and
  destination with consent although the user opened a session on another host meanwhile, and the session then opening:
  `launcher::pending_tests::a_yolo_question_the_user_moved_on_from_waits_and_its_answer_resends_the_draft`.
- 1037's unknown outcome, the host's timeout, for both modes, the row settled while the launcher is out of sight and the
  launcher brought back from its notice:
  `launcher::restart_with_tests::a_timed_out_restart_with_settles_its_row_before_the_launcher_returns` and
  `launcher::replace_with_tests::a_timed_out_replace_with_settles_its_row_before_the_launcher_returns`.
- 1617: `launcher::replace_with_tests::a_locally_refused_replace_with_keeps_the_sources_stop_report`.
- 1693, reframed for the ordered channel, where a Delete sent while a create is out is answered after it (SPEC_impl.md,
  "The UI channel"): the launch the user moved on from selects nothing, and the Delete's pick then follows the ordinary
  rules, the most recently visited session still listed over the launch's newer one:
  `launcher::pending_tests::a_delete_while_a_launch_is_out_picks_by_the_ordinary_rules`.
- 237's Rename variant, "Rename during a pending launch preserves its refusal and draft", reframed: opening Rename is a
  click, so the user has moved on, and a refusal the user moved on from waits behind its notice, which brings the
  launcher back with its draft and refusal, in
  `launcher::launch_tests::a_refusal_the_user_moved_on_from_waits_behind_a_notice` (moved on with a key press); not yet
  ported: Rename's field keeping the keyboard as the refusal arrives.
- 970, "New preserves Hosts and reports its success without retaining its launcher": Hosts still showing and the
  "Started" report after a launch the user moved on from in
  `session_pane::tests::a_first_launch_the_user_moved_on_from_selects_nothing`, the report in
  `launcher::launch_tests::{a_launch_that_does_not_open_reports_started, a_slow_success_hands_the_pill_to_its_started_report}`,
  and the next New starting fresh in
  `launcher::pending_tests::a_launchs_success_leaves_the_keyboard_in_the_session_switched_to`; not yet ported: the
  remembered selection staying the source's, and the report holding no button or link.
- Line 1037's refusal outcome for Restart with:
  `launcher::restart_with_tests::restart_with_settles_as_plain_restart_does`.
- Line 1436's pointer variant (a double click's second click is a repeat, not moving on):
  `focus_tests::what_counts_as_the_user_moving_on`.
- Line 1551's Restart with variant:
  `launcher::restart_with_tests::the_yolo_question_of_a_restart_with_leaves_no_failure_on_the_session` (no failure on
  the session while the question is up).

### `tests/yolo-guard.spec.ts`

Tests: 13. Covers: a YOLO launch asks first and starts only when confirmed${height === 844 ? "" : ` at 390x${height}`}.

replaced: `launcher::yolo_tests::a_yolo_refusal_asks_the_question_with_cancel_focused`,
`launcher::yolo_tests::the_one_off_answer_launches_with_consent`,
`launcher::yolo_tests::the_permanent_answer_stops_asking_then_launches`,
`launcher::yolo_tests::a_failed_stop_asking_starts_nothing_and_keeps_the_question`,
`launcher::yolo_tests::the_guarded_answer_launches_guarded_without_consent`,
`launcher::yolo_tests::cancel_starts_nothing_and_keeps_the_launcher`,
`launcher::yolo_tests::each_yolo_launch_gets_its_own_reason`,
`launcher::yolo_tests::only_an_explicit_yolo_choice_offers_guarded` (no guarded answer for custom flags),
`launcher::yolo_tests::the_launcher_holds_still_while_stop_asking_is_out` (Launch, Close and Cancel during the host
write; a draft edit retiring the question),
`launcher::yolo_tests::a_click_during_stop_asking_keeps_the_new_session_from_opening`; "cancel and an answer in one
burst start nothing" is the prompt's first-answer-wins rule; "after a lost launch reply" is superseded (the GUI sends no
create keys and retries nothing), and so are the five shorter legs (601 to 480 high), whose stacked-action checks need a
window below the minimum (Deviations); all six legs are 390 wide, and what the 844-high one checks is ported at the
sizes the window allows; the plain Replace cases are the sidebar's and the header's, whose YOLO questions are tested,
and the template cases are the templates area's, except two that the launcher's own rule decides (below); 511's focus
going back to Cancel after the failure is superseded: an inline question keeps focus on the answer after a failed
answer, SPEC_impl.md, "Questions: the prompt", pinned by
`prompt_tests::a_failing_answer_keeps_its_question_up_with_the_reason`

- "a YOLO launch asks first and starts only when confirmed" (58), the 844-high leg: the question's heading, what YOLO
  means and that nothing started are wording, shown in the `app-launcher-yolo-confirmation` scene; why this launch is
  YOLO in `launcher::yolo_tests::each_yolo_launch_gets_its_own_reason` and
  `launcher::yolo_tests::a_yolo_refusal_asks_the_question_with_cancel_focused`, and the host named, with the line that
  nothing has started and without the helm's own sentence, in
  `session_list::lifecycle_tests::the_question_names_the_sources_host` (the same builder); Cancel focused, Cancel
  keeping the launcher and the one-off answer's consent in the inline list's tests; an edit retiring the question is the
  launcher's one edit path, which takes the question down for a template that changes the launch (the template bullet
  below) and retires its answer for a name edit
  (`launcher::yolo_tests::the_launcher_holds_still_while_stop_asking_is_out`); a host that allows YOLO without asking
  admitting the same launch is the helm's
  (`sessions_tests::a_yolo_create_on_a_host_that_asks_is_refused_until_confirmed`), and the GUI asks only on the helm's
  refusal marker.
- "a header replace keeps its confirmed nothing-alive answer" and "a sidebar replace keeps its confirmed nothing-alive
  answer", each when confirmed once and when told not to ask again (697, 706, 794, 803), one test each over a shared
  driver (`session_list::lifecycle_tests::nothing_alive_guard_survives`): the source pushed running while its question
  is open, the answer still sending `only_if_nothing_alive`, and the helm's "both sessions still exist" shown where the
  Replace was asked, in
  `session_pane::header_tests::{a_header_replace_keeps_its_confirmed_nothing_alive_answer_when_confirmed_once, a_header_replace_keeps_its_confirmed_nothing_alive_answer_when_told_not_to_ask_again}`
  and
  `session_list::lifecycle_tests::{a_sidebar_replace_keeps_its_confirmed_nothing_alive_answer_when_confirmed_once, a_sidebar_replace_keeps_its_confirmed_nothing_alive_answer_when_told_not_to_ask_again}`.
- "the YOLO question offers no guarded answer for custom permission flags" (360): the custom-flags reason in
  `launcher::yolo_tests::each_yolo_launch_gets_its_own_reason` (its sentence is wording, not separately tested), no
  guarded answer in `launcher::yolo_tests::only_an_explicit_yolo_choice_offers_guarded`, and Cancel in the inline list's
  tests.
- "don't ask again marks the host safe before launching, and a failed mark launches nothing" (511): the write before the
  create in `launcher::yolo_tests::the_permanent_answer_stops_asking_then_launches`, and a refused write starting
  nothing and keeping the question up with its reason in
  `launcher::yolo_tests::a_failed_stop_asking_starts_nothing_and_keeps_the_question`, including a plain Launch then
  sending no consent and asking a clean question without the failure's reason; the reason's warning ink is layout, not
  separately tested, since no scene shows a failed write.
- "the YOLO question can launch guarded and remember that permission" (201), its guarded answer writing nothing to the
  host: `launcher::yolo_tests::the_guarded_answer_launches_guarded_without_consent`.
- "template edits retain an unchanged YOLO question and retire a changed one" and "command templates preserve equal YOLO
  questions and retire changed commands" (877 and 1072): a `tl:` template that leaves the launch as it was keeps the
  question, whose one-off answer then sends the same create with consent, one that changes it retires the question, and
  going back does not bring it back, in
  `launcher::search_view_tests::{an_unchanged_agent_template_keeps_the_yolo_question, an_unchanged_command_template_keeps_the_yolo_question}`.
- The question's Tab order and its scrolling into view, in a stand-in launcher:
  `prompt_tests::{an_inline_question_answers_once_and_returns_focus_inside_its_dialog, an_inline_question_below_the_fold_is_scrolled_into_view_as_it_takes_focus}`;
  its words naming the host in `session_list::lifecycle_tests::the_question_names_the_sources_host` (the same builder);
  Launch after Cancel asking again without consent in
  `templates::dispatch_tests::the_chords_yolo_refusal_reveals_the_ordinary_question_over_its_draft`.
- "cancel and an answer to the launcher's YOLO question in one burst start nothing":
  `prompt_tests::an_inline_question_answers_once_and_returns_focus_inside_its_dialog` (Cancel then Confirm with no draw
  between runs nothing).
- "the YOLO question can launch guarded and remember that permission" (201): the answers' order in
  `prompt_tests::extra_answers_sit_in_order_between_cancel_and_confirm_and_never_start`, and their tiers in the
  `app-launcher-yolo-confirmation` scene; the guarded answer's create in
  `launcher::yolo_tests::the_guarded_answer_launches_guarded_without_consent`; the old held draft drawn with Guarded
  pressed is superseded, since the launcher hides while its create is out
  (`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`); its last leg, the next New opening on
  Guarded, is the helm remembering the permission of a successful launch
  (`sessions_tests::a_successful_structured_launch_remembers_its_permissions_choice`) and New preselecting the
  remembered permission (`launcher::entry_tests::new_opens_beside_the_open_session`, with YOLO remembered).
- The launcher's plain Replace question:
  `launcher::yolo_tests::{the_plain_replace_question_offers_no_guarded_answer_and_retries, the_plain_replace_permanent_answer_writes_then_retries, the_plain_replace_question_leaves_focus_once_the_user_moved_on, the_plain_replace_retry_carries_the_answers_mark}`.
- "a skipped header replace still asks the YOLO question": a skipped confirmation sends the same Replace
  (`session_list::lifecycle_tests::skipped_confirmations_stop_and_replace_on_the_click_with_their_rows_guards`), whose
  YOLO refusal from the header asks with Cancel focused
  (`session_pane::header_tests::a_yolo_refused_replace_from_the_header_asks_below_its_top_row`), and Cancel sends
  nothing (`session_list::lifecycle_tests::cancel_drops_the_question_and_gives_focus_back_to_the_rows_button`).

### `tests/clone.spec.ts`

Tests: 13. Covers: a structured GUI clone pre-fills without launching, then starts its ready successor.

partly: a copied model, Custom flags and command text holding invisible characters spelled out, as the old prefill
showed them, in
`launcher::view_tests::{agent_launch_text_holding_invisible_characters_shows_escaped, copied_command_text_holding_invisible_characters_shows_escaped_and_editable}`
(the first test's read-only prompt half is the old templates spec's "initial prompt launcher template stacking and save
selection", in the templates area's file); the prefill, host fallback and its note, and refusals in
`launcher::entry_tests::{clone_prefills_the_sources_host_folder_title_and_launch, clone_of_a_session_on_a_retargeted_host_row_falls_back_and_says_so, clone_of_a_sandbox_session_with_nothing_to_carry_falls_back_and_says_so, clone_opens_nothing_over_a_dialog_or_for_an_unknown_session, clone_brings_back_a_refused_launch_waiting_behind_its_notice}`;
"each clone restores every field after an intervening draft edit" and "closing a clone without submitting, or submitting
it, both leave the next New Session with fresh defaults" in
`launcher::entry_tests::{each_clone_starts_from_its_source_and_new_starts_fresh_after_closing, a_clone_launch_sends_the_copied_launch}`;
"Clone of a command launch fills in its YOLO answer, agent type and resume command" and "the edited copy leaves the
original untouched" (a draft is a copy; nothing edits its source) in
`launcher::entry_tests::{clone_prefills_the_sources_host_folder_title_and_launch, a_clone_launch_sends_the_copied_launch, clone_falls_back_to_the_open_sessions_host}`
and `launcher::draft_tests`; the catalog cases in
`launcher::view_tests::{a_clone_launches_while_the_catalog_read_is_out, a_clone_launches_after_a_failed_catalog_read, a_carried_effort_the_model_does_not_offer_stays_offered}`;
superseded: the toggle-return, held-focus-work and held-terminal-reveal cases (focus moves only through the focus
coordinator, and a reply such as the catalog's never moves it, so a late catalog keeps focus on the name field too); the
two cases that focus search on opening in
`launcher::search_view_tests::clone_and_replace_with_start_focus_in_the_search_box`; a managed-checkout Clone's fresh
checkout and `-clone` name are in the `github-checkouts.spec.ts` row; a sandbox or container Clone's fresh environment
from the source's recipe and place, folder `~`, found by the source's host id as the old launcher found it, a recipe
gone since kept as unavailable, and a host naming no environment falling back with the old note, in
`launcher::entry_tests::{clone_of_an_environment_session_starts_a_fresh_one_from_its_recipe, a_clone_whose_recipe_is_gone_keeps_it_unavailable, clone_of_a_sandbox_session_with_nothing_to_carry_falls_back_and_says_so}`
and `launcher::environment_tests::a_seeded_environment_has_the_segments_effects_and_chooses_nothing`

- "a structured GUI clone pre-fills without launching, then starts its ready successor": the agent launch seeded as
  deliberate choices in `launcher::draft_tests::clone_seeds_an_agent_launch_verbatim`, one create per press in
  `launcher::entry_tests::a_clone_launch_sends_the_copied_launch`, and its selection in the body in
  `launcher::request_tests::an_agent_create_body_carries_the_draft`; the successor's real command line is the
  supervisor's, in `crates/farhelm/tests/e2e/structured_launches.rs`.
- "a failed catalog read shows a retry and leaves the clone launchable", its Retry:
  `launcher::view_tests::a_failed_catalog_read_says_so_and_retry_reads_it_again`.
- Clone spells out invisible characters in the copied folder and session name. The unchanged fields send the raw bytes
  from the source, and edits show what the user types:
  `launcher::entry_tests::{clone_shows_its_copied_folder_and_name_escaped_and_editable, a_managed_clones_numbered_name_spells_out_its_source_title}`.

### `tests/destination-authority.spec.ts`

Tests: 8. Covers: New inherits a filtered selected destination with replacement=${replace}.

replaced: both cases of "New inherits a filtered selected destination" in
`launcher::entry_tests::{new_opens_beside_an_open_session_the_list_hides, new_does_not_follow_a_retargeted_host_row}`;
"the launcher's folder follows each host's last launch until the user types one" (666) in
`launcher::folder_default_tests::{new_waits_for_the_hosts_last_folder_and_starts_there, a_failed_history_read_settles_on_home, new_beside_an_open_session_waits_for_nothing}`
and
`launcher::draft_tests::{new_waits_for_its_folder_default_once, a_host_change_rearms_the_folder_default_where_the_destination_is_free}`,
its typed-folder half at the draft level; superseded: the queued-activation, key-minting and retry-identity cases (220,
250, 360, 459, 533), since GPUI runs handlers in order against the current draft and the GUI sends no launch request
keys; "applied … retains installation authority through Launch" (165) in
`launcher::folder_default_tests::{a_recent_setups_folder_refuses_launch_on_a_retargeted_host, a_saved_folder_from_search_refuses_launch_on_a_retargeted_host, a_recent_folder_from_the_folder_list_refuses_launch_on_a_retargeted_host, a_reconnect_to_the_same_installation_keeps_the_folder, a_default_is_decided_again_on_a_retargeted_host, an_inherited_folder_is_decided_again_on_a_retargeted_host}`
and
`launcher::draft_tests::{a_history_folder_stays_on_its_installation, a_default_from_a_retargeted_installation_is_decided_again}`

- 666, the rest: the folder re-seeded from the new host's last launch on a host change in
  `launcher::folder_default_tests::a_host_change_waits_for_the_new_hosts_folder`, a typed folder kept across a host
  change in `launcher::draft_tests::a_host_change_reseeds_only_an_untouched_folder` and, in the view,
  `launcher::view_tests::the_host_picker_browses_with_arrows_and_enter`, and a newer managed checkout not counting as
  the last folder in `launcher::recents_tests::the_remembered_folder_is_the_newest_plain_folder_launch`.
- The superseded cases' non-key halves: Home and the history's folders setting the folder (360) in
  `launcher::folder_list_tests::{local_home_moves_to_the_local_host, focusing_the_folder_lists_recent_folders_and_resets}`,
  a recent filling on a click and launching on Enter while search only fills (459) in
  `launcher::recents_view_tests::a_click_fills_and_enter_launches` and
  `launcher::search_view_tests::a_recent_setup_from_search_fills_and_launches_nothing`, and Browse picks only selecting
  (533) in `launcher::browse_tests::browse_lists_one_level_and_taking_a_folder_chooses_it`.

### `tests/podman-environments.spec.ts`

Tests: 10. Covers: active placement segments preserve explicit host recipe and place.

partly (New container). This area's cases, for the launcher's environments (being built by the desktop integration area,
with this area reviewing): "active placement segments preserve explicit host recipe and place" in
`launcher::environment_tests::the_chosen_segment_keeps_its_choices_and_existing_host_gives_the_host_back`; "container
launch keeps recipe and place through refusal" in
`launcher::environment_launch_tests::a_failed_run_brings_the_launcher_back_with_its_recipe_and_place`; the `launcher`
surface of "... refreshes recipe places after a delayed probe without a fleet notice", whose Container host select
follows the pushed Podman checks it asks for on opening and whose note follows the pushed recipe places, in
`launcher::environment_view_tests::{opening_the_launcher_checks_the_container_hosts, opening_the_launcher_checks_no_host_while_the_hosts_are_unavailable, new_container_shows_its_rows_with_unusable_hosts_disabled, the_note_follows_the_recipe_place}`;
New container's default host and its note going with its host in
`launcher::environment_view_tests::{new_container_defaults_to_the_last_container_host, no_note_once_the_container_host_is_gone}`;
the search half of "environment recents hold focused intent until deliberate promotion" and "environment recents stay
independent of a held ordinary folder default" in
`launcher::environment_recents_tests::{an_environment_setup_shows_by_a_chosen_recipe_with_no_host_connected, environment_setups_stay_apart_from_a_hosts_held_folder_default}`,
with a taken setup's placement shown, sent and its destination brought along in
`launcher::environment_recents_tests::{a_taken_environment_setup_shows_and_sends_its_placement, an_environment_setup_brings_its_own_destination}`,
the two histories' merge, the setups' ranking and a search row's placement in
`launcher::recents_tests::{the_environments_launches_merge_by_each_sources_next_launch, environment_launches_pass_the_folder_and_group_by_their_placement}`
and `launcher::search_tests::an_environment_setup_names_its_own_placement`; superseded: the rest of "environment recents
hold focused intent until deliberate promotion" (a re-read on every activity-feed notice, held back from a focused row
until a deliberate choice), since the launcher reads the environments' history once as it opens; not yet ported:
"restored container placement displays the recipe and host it submits", a saved container template taken from the
composer search showing the recipe and container host it then sends in one create (its draft half, entering a new
container at its home, is `templates::apply_tests::a_container_template_enters_a_new_container_at_its_home`). The spec's
other cases are other areas': "container template quick launch retains its saved place" is the templates and quick
launch area's, "a connected container's Delete asks its Farhelm and reports the container that stays" is the session
view's, and "Containers Settings keeps failed cleanup explicit" and the `recipes` surface of the delayed-probe case are
in [Settings and hosts](settings-and-hosts.md). Beyond this spec's cases, SPEC.md's carried placements (Container
sessions follow Sandbox sessions' Clone, and New beside an open environment session preselects its recipe and place,
never in a quick launch): one seed for both providers in
`launcher::environment_tests::a_seeded_environment_has_the_segments_effects_and_chooses_nothing`, driven with a sandbox
in
`launcher::entry_tests::{clone_of_an_environment_session_starts_a_fresh_one_from_its_recipe, new_beside_an_open_environment_session_preselects_its_recipe_and_place}`,
and Replace with keeping the existing environment in
`launcher::entry_tests::replace_with_of_an_environment_session_keeps_its_environment` and
`launcher::environment_tests::restart_with_and_replace_with_keep_the_existing_environment`, its recent setups of new
environments refused in the old words in
`launcher::environment_recents_tests::replace_with_refuses_an_environment_setup`.

### `tests/tensorlake-sandbox-sessions.spec.ts`

Tests: 16. Covers: recipe placement explains prerequisites and requires billing consent.

partly: six of the 16 cases are this area's, and the other ten are the session view's and Hosts'; the session view's are
recorded in [Sidebar and session view](sidebar-and-session-view.md), under its cases of specs listed in other areas'
files. Of "recipe placement explains prerequisites and requires billing consent", New sandbox showing once the account
has an API key, the first recipe with a snapshot chosen on entering it, an unbuilt recipe listed unavailable with "No
snapshot yet" (and the account's prerequisites before it, in the old order), in
`launcher::sandbox_view_tests::{new_sandbox_is_hidden_without_an_api_key, entering_new_sandbox_chooses_the_first_built_recipe_and_names_the_missing_key, sandbox_recipes_are_listed_with_their_mark_size_and_refusal, the_registration_note_shows_and_copies_the_command}`
and
`launcher::environment_tests::{a_new_sandbox_is_refused_for_each_missing_prerequisite_in_the_old_order, the_registration_note_needs_a_readable_key}`;
its billing question ("Create a sandbox for this session?" with the recipe, its size, the billing words and the
"Tensorlake pricing" link, focus on Cancel; Cancel sending nothing and keeping the draft; "Create and launch" sending
one confirmed create on its own route to the Tensorlake account, whose refusal comes back with its draft and whose
consent the next press asks for again; an edit retiring the question; "Create Tensorlake sandboxes without asking"
skipping it with an unconfirmed create, and a press after such a create asking) in
`launcher::sandbox_view_tests::{a_press_asks_before_creating_a_sandbox_with_cancel_focused, cancel_creates_nothing_and_keeps_the_draft, create_and_launch_sends_one_confirmed_create, an_edit_retires_the_question, the_setting_skips_the_question_with_an_unconfirmed_create, a_press_after_a_create_on_the_skip_setting_asks}`
and `launcher::environment_tests::only_a_sandbox_asks_before_creating_unless_its_setting_skips_it`; a quick launch of a
sandbox waiting for the account and then coming back to ask rather than create in
`launcher::sandbox_view_tests::a_hidden_quick_launch_waits_for_the_account_and_comes_back_to_ask`. "A recipe template
quick launch asks before one recipe request" is ported with the templates area's number key in
`templates::dispatch_tests::a_sandbox_template_on_a_number_key_comes_back_to_ask_before_creating`. "A held sandbox
Suspend and a launch on another host proceed side by side": New staying available while a Suspend is out in
`session_list::environment_tests::a_suspend_in_flight_leaves_new_available` and a launch elsewhere not holding a Resume
in `hosts::environment_tests::a_launch_elsewhere_does_not_hold_resume`; not yet ported: its create sent while another
host's Suspend is held. Deferred: resume before create is a near-term TODO item. This covers "an existing-host launch
stays nonmodal while its Resume is held" and "Hosts refuses a host's Resume and Settings while a launch is waking that
host". "Template placement keeps Host editable after leaving an environment" is the templates and quick launch area's.
The registration note shows with no recipe chosen too, as the old launcher's did and SPEC.md says ("whether or not a
recipe is chosen") (`launcher::sandbox_view_tests::a_new_environment_without_a_recipe_is_named_without_one`).

### `tests/github-checkouts.spec.ts`

Tests: 18. Covers: quick launch waits for a managed preview and
${collide ? "reveals its path race" : "starts directly"}${base ? " for a template base name" : ""}.

replaced: Clone's `-clone` default and its numbering, an explicit name staying a conflict, Folder restoring the source's
folder and title, a template base name numbering (1151, and 248: its pill and "(<base> is taken)" note in
`templates::checkout_name_tests`, its quick launches in
`templates::dispatch_tests::{a_taken_base_name_is_numbered_and_launched_out_of_sight, a_numbered_quick_launch_the_user_moved_on_from_starts_quietly, a_base_name_quick_launch_that_loses_its_folder_is_shown}`
and `launcher::checkout_tests::a_guarded_wait_does_not_follow_a_taken_base_name`) and a Clone's lost race re-previewing
a suffix without sending again (1232) in
`launcher::checkout_tests::{a_clone_of_a_managed_checkout_starts_on_its_repository_named_clone, a_taken_default_name_moves_on_and_an_exact_one_stays, a_taken_clone_name_is_previewed_again_with_the_next_suffix, a_lost_race_takes_a_new_preview_and_sends_nothing}`;
Replace with into a checkout unnamed (1390) in `a_checkout_ignores_an_untouched_copied_title`; superseded: the
retained-key cases (768, 1302), since the GUI sends no launch request keys (see Deviations); the templates area's: the
quick-launch and Save as template cases (43, 159, 248, 333); the approvals area's: the agent checkout approval (579);
the other dialogs area's: the three trash cases (1516, 1578, 1687), in [Other dialogs](other-dialogs.md); a folder
search result leaving checkout mode (895) in
`launcher::search_view_tests::a_saved_folder_from_search_leaves_a_managed_checkout`; the real-clone cases (830, 868,
935, 1047) by their launcher halves below, with a real clone in the end-to-end suite and the checkout's references in
the supervisor's unit tests, since a GUI-driven real clone is not run (the end-to-end smoke test's fake fleet has no
repository source); dropped from the GUI port as supervisor behavior (decided by the migration's orchestrator on
2026-11-16): the server halves of 935 and 1047, a stopped borrower keeping its reference to the checkout and a Replace
onto another folder archiving the checkout it released, which the old cases drove through the HTTP API and no server
test drives through Stop or Replace yet

- 830, the launcher's half: a name edit re-previewing in
  `launcher::checkout_tests::what_a_preview_answers_for_asks_again_and_late_answers_never_apply`, the create carrying
  the preview in `launcher::checkout_tests::a_checkout_create_carries_its_preview`, and a recent choosing its repository
  again rather than its old folder in `launcher::recents_view_tests::a_checkout_setup_chooses_its_repository`; the real
  clone, its hook and agent in `crates/farhelm/tests/e2e/github_checkouts.rs`
  (`a_raw_http_fresh_checkout_completes_the_real_clone_hook_and_agent`,
  `a_fresh_checkout_creates_the_missing_root_and_parents_on_first_use`).
- 868, the launcher's half: a Command tab draft launched into a checkout in
  `launcher::checkout_tests::a_chosen_repository_is_previewed_before_it_launches`, the tab kept as the destination
  changes in `launcher::checkout_tests::a_picked_folder_ends_checkout_mode_and_each_choice_counts`, and the command sent
  as written in `launcher::request_tests::a_command_create_body_sends_the_command_as_written`; a command line run in a
  real checkout in `crates/farhelm/tests/e2e/github_checkouts.rs`
  (`a_raw_http_fresh_checkout_completes_the_real_clone_hook_and_agent`).
- 895, its launch: a folder destination sending no checkout in
  `launcher::checkout_tests::{managed_checkout_hides_the_folder_until_a_repository_is_chosen, a_checkout_create_carries_its_preview}`.
- 935, the launcher's half: a Clone switched to Folder borrowing the source's folder and sending no checkout in
  `launcher::checkout_tests::{a_clone_of_a_managed_checkout_starts_on_its_repository_named_clone, a_checkout_create_carries_its_preview}`;
  a deleted reference archiving the checkout only when it is the last, in the supervisor's unit tests
  `service::teardown::tests::{deleting_the_last_reference_archives_and_other_members_hold_the_directory, existing_create_before_last_reference_delete_preserves_the_checkout}`;
  Stop keeping the reference has no test (see the status).
- 1047, the launcher's half: its one launch into a checkout,
  `launcher::checkout_tests::a_chosen_repository_is_previewed_before_it_launches`; every other step drives the server
  directly, and the release on Replace has no test (see the status).

### `tests/terminal-create-idempotency.spec.ts`

Tests: 9. Covers: a null-permission ${harness} template retains consent and one session after a lost reply.

superseded: intent keys and their retries, since the GUI sends no create keys, and a duplicate around a lost connection
is accepted (SPEC.md, "Creation"); the remaining case, "the create form's inputs are disabled while a create is in
flight", in `launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher` (one press, one create; nothing
can be pressed or edited while it is out); dropped: the session view half of "with no CSPRNG, a create still carries a
key while the session view refuses to attach", which removed the page's Web Crypto object, a condition the desktop app
has no counterpart of

- The template null-permission resets the first case launches through (the templates area's rule):
  `launcher::search_view_tests::{a_tl_reset_of_codexs_permission_launches_guarded, a_tl_reset_of_gooses_permission_launches_yolo}`.
- What the GUI shows for a lost reply without a key:
  `launcher::launch_tests::a_timed_out_create_comes_back_without_not_launched`.

### `tests/restart-with.spec.ts`

Tests: 42. Covers: named restart with requires a target when no conversation was captured.

partly: availability and its reasons ("unavailable restart with stays visible and inert with a reason", "is unavailable
for a command launch without a resume command") in
`launcher::restart_with_tests::restart_with_names_why_it_is_not_offered` and
`launcher::draft_tests::restart_with_is_offered_only_for_a_resumable_launch`; the required conversation, edits and their
sending ("named restart with requires a target", in its refusal half; "shows fixed context and submits one edited
resume"; "edits a command launch's command, resume command and YOLO answer", in part) in
`launcher::request_tests::{restart_with_needs_an_edit_and_sends_it, restart_with_refuses_a_missing_conversation_and_a_changed_launch, restart_with_names_its_command_and_host_problems}`,
`launcher::draft_tests::{restart_with_refuses_to_switch_agents, restart_with_shows_the_stored_permission_and_resets_to_it}`
and
`launcher::restart_with_tests::{restart_with_stops_a_working_agent_with_consent, restart_with_an_idle_agent_asks_no_consent}`
(with the changed markers); the escaped stored values ("shows an unsafe stored wrapper prefix without editing its escape
text", "preserves raw command bytes" in both variants) in
`launcher::view_tests::restart_with_shows_an_unsafe_stored_wrapper_prefix_escaped` and
`launcher::restart_with_tests::{restart_with_shows_stored_command_values_escaped, restart_with_sends_stored_command_bytes_for_a_yolo_only_edit, restart_with_sends_the_stored_resume_beside_an_edited_command}`,
the last two also sending a command launch's edited command and YOLO answer; focus on Cancel, "Restart error.", the held
host, the stored effort and model Enter in
`launcher::view_tests::{restart_with_starts_on_cancel_and_says_restart_error, restart_with_shows_its_held_host_as_text, restart_with_keeps_its_stored_effort_offered, restart_with_model_enter_chooses_without_restarting}`;
settling, refusal and the pending row in `launcher::restart_with_tests::restart_with_settles_as_plain_restart_does`;
"hides guarded when it would restore the unchanged launch" in
`launcher::restart_with_tests::restart_with_offers_guarded_only_when_it_changes_the_launch`; held keys in
`launcher::launch_tests::a_held_enter_in_a_field_never_launches`; superseded by the focus coordinator and layer manager:
the backdrop, stray-focus, inert-page and tab-gone focus cases, and the container Enter half of "restart with keeps gap
focus and container Enter uses its enabled action" (the launcher keeps focus on an empty-space click, so its container
never takes it); the shortcut batches and the bolt in Restart with ("restart with refuses held shortcut batches and bolt
templates", "restart with applies matching bolt and shortcut templates without restarting") in
`templates::launcher_modes_tests::{restart_with_refuses_a_held_change_from_a_sequence_or_the_chord_whole, restart_with_applies_matching_shortcuts_without_restarting}`,
whose detail is in the templates file's restart-with entry

- "named restart with requires a target when no conversation was captured", its sending half: a named conversation alone
  restarting the stored launch, sent beside it, in
  `launcher::request_tests::a_named_conversation_alone_restarts_the_stored_launch`.
- "Restart with escapes and isolates peer refusal text":
  `launcher::restart_with_tests::a_restart_refusal_spells_out_hidden_characters`.
- "restart with upgrades an older own-setting launch to guarded":
  `launcher::restart_with_tests::guarded_over_an_older_own_setting_launch_is_an_edit`.
- "a YOLO restart-with requires confirmation inside the dialog", "restart with can answer the YOLO question by
  restarting guarded" and "a YOLO command restart-with asks inside the dialog, worded for an assertion": the answers
  resending the restart in
  `launcher::restart_with_tests::{the_one_time_yolo_answer_resends_the_restart, the_guarded_answer_restarts_guarded, a_command_restarts_yolo_question_speaks_of_the_assertion}`;
  the question inside the launcher, without the helm's own sentence, with Cancel focused, and Cancel sending nothing,
  the same in every mode, in
  `launcher::yolo_tests::{a_yolo_refusal_asks_the_question_with_cancel_focused, cancel_starts_nothing_and_keeps_the_launcher}`,
  and a keyboard answer returning focus inside the launcher in
  `prompt_tests::an_inline_question_answers_once_and_returns_focus_inside_its_dialog` (to Launch, where the old dialog
  returned it to Cancel); the old dialog's look while the guarded restart was out (Guarded pressed and marked) has no
  counterpart, since the launcher hides while its restart is out, as it does while a create is out (Deviations).
- "restart with escapes the old model in its changed marker":
  `launcher::restart_with_tests::the_changed_marker_spells_out_the_old_models_hidden_characters`.
- "restart with compares an older omitted permission by its effective mode":
  `launcher::restart_with_tests::an_omitted_permission_is_compared_by_the_mode_it_ran_with`.
- "restart with edits a command launch's command, resume command and YOLO answer": each marked on its own as it is
  edited, and the three edits sent whole, in `launcher::restart_with_tests::restart_with_marks_an_edited_command`.
- "restart with choice Enter applies effort and permissions without old trust", the stored trust left out of the
  restart: `launcher::restart_with_tests::restart_with_stops_a_working_agent_with_consent` (the choice Enter itself is
  the shared Enter rule's).
- "restart with model Enter refuses an unselected draft and held keys": the unselected draft in
  `launcher::view_tests::restart_with_model_enter_chooses_without_restarting`, and a held Enter in
  `launcher::restart_with_tests::a_held_enter_in_the_model_field_does_nothing`; not yet ported: its composing Enter, an
  input method's Enter while composing in the model field.
- "restart with retries the catalog without reseeding its draft":
  `launcher::restart_with_tests::the_catalog_retry_keeps_an_edit`.
- "restart with keeps gap focus and container Enter uses its enabled action", the gap half:
  `layers_tests::the_page_behind_a_dialog_takes_no_clicks_and_empty_space_keeps_focus`.
- "don't ask again from restart with keeps focus in the dialog and marks before restarting" (the question and its host
  write are the same in every mode):
  `launcher::yolo_tests::{the_permanent_answer_stops_asking_then_launches, a_failed_stop_asking_starts_nothing_and_keeps_the_question, the_launcher_holds_still_while_stop_asking_is_out}`.
- "cancel and an answer to restart with's YOLO question in one burst restart nothing":
  `prompt_tests::{an_inline_question_answers_once_and_returns_focus_inside_its_dialog, a_confirmation_acts_only_on_what_it_showed}`.
- "unavailable restart with stays visible and inert with a reason", the drawn entry:
  `session_pane::header_tests::an_unavailable_choice_stays_in_its_menu_with_its_reason`.
- "restart with keeps Tab from the open model list inside the dialog":
  `launcher::view_tests::leaving_the_model_field_closes_its_list`,
  `layers_tests::a_popover_that_never_takes_focus_leaves_focus_and_tab_with_the_field` and
  `focus_tests::tab_stays_inside_the_topmost_dialog`.
- "restart with pulls stray focus back and still cancels on Escape", Escape's return to the header:
  `session_pane::header_tests::the_with_choices_and_clone_open_the_launcher_and_return_focus_on_cancel`.
- "restart with re-displays and submits custom permission words": the stored Custom words not taken for an edit and sent
  with their spaced word in `launcher::request_tests::restart_with_needs_an_edit_and_sends_it`, unclosed quotes refused
  in `launcher::request_tests::refusals_name_the_first_thing_to_fix`.
- "restart with unchanged repeated and composing choice Enter sends nothing", its unchanged and repeated halves:
  `launcher::request_tests::restart_with_needs_an_edit_and_sends_it` and
  `components::tests::button::enter_or_space_on_a_focused_button_runs_it_once`; its composing half is superseded by the
  choice controls' shared Enter rule, which says no input method can compose into them (`components::enter_choice`'s
  documentation in `crates/gui/farhelm-gui/src/components/mod.rs`; decided by the migration's orchestrator on
  2026-11-16).
- "restart with command Enter validates and keeps the shown YOLO value":
  `launcher::view_tests::a_refused_press_says_why_beside_the_main_action` (Enter in a single-line field),
  `launcher::request_tests::restart_with_names_its_command_and_host_problems` and
  `components::tests::radio_group::space_chooses_a_focused_radio_and_enter_goes_on_to_the_owner`.
- "restart with first ${controlKind} edit hides while held and restores focus after refusal": the effort variant in
  `launcher::restart_with_tests::restart_with_settles_as_plain_restart_does`, the model variant's Enter in
  `launcher::view_tests::restart_with_model_enter_chooses_without_restarting`.
- "restart with applies an allowed partial agent template", "restart with applies a template's matching held ${field}"
  and "restart with refuses a template's held ${field} atomically":
  `templates::apply_tests::{restart_with_applies_a_partial_agent_template_and_sends_its_choices, restart_with_applies_equal_held_values_as_nothing, restart_with_resets_permissions_to_todays_default, restart_with_refuses_a_change_to_a_held_field}`
  and `launcher::draft_tests::each_mode_holds_its_own_fields`.
- "restart with applies an allowed partial command template":
  `templates::apply_tests::restart_with_applies_a_partial_command_template_and_sends_it`.
- "restart with refuses held composer actions and scopes model search":
  `launcher::search_view_tests::restart_with_refuses_a_held_field_and_says_why` and
  `launcher::search_tests::model_rows_follow_the_chosen_agent`.
- "restart with refuses a matching recent and ignores folder memory":
  `launcher::recents_view_tests::replace_with_says_replace_and_restart_with_has_no_strip` and
  `launcher::search_view_tests::history_rows_are_refused_where_they_cannot_apply` (a restart's body has no folder).
- "restart with releases the page while pending and restores modal focus after refusal":
  `launcher::restart_with_tests::restart_with_settles_as_plain_restart_does` (the launcher hides while the restart is
  out and comes back with "Restart error." and the edit on a refusal; focus inside it is the focus invariant every test
  step checks).
- "restart with choice Enter carries the displayed working-agent confirmation":
  `launcher::restart_with_tests::restart_with_stops_a_working_agent_with_consent` ("Stop and restart" and the warning
  shown, and the press sending the consent to stop), pressed by click; Enter on a choice reaches the same press
  (`components::tests::radio_group::a_button_group_makes_each_option_a_tab_stop_chosen_with_enter_or_space`).
- "restart with withdraws working-agent consent when its source exits":
  `launcher::restart_with_tests::restart_with_an_idle_agent_asks_no_consent` (the source goes idle while the launcher is
  open, and the press sends no consent to stop it).

### `tests/host-picker.spec.ts`

Tests: 5. Covers: host picker supports mouse, navigation, typeahead, cancellation, and focus.

replaced: `launcher::view_tests::the_host_list_offers_every_host_with_its_phase_and_a_click_picks_one`,
`the_host_picker_browses_with_arrows_and_enter`, `home_end_up_and_space_in_the_host_list`,
`typeahead_finds_a_host_and_escape_cancels`, `unmatched_typeahead_keeps_the_chosen_host`,
`a_click_opened_host_list_owns_the_keyboard_on_macos`, `the_host_button_is_empty_before_hosts_arrive`,
`cancel_and_the_host_button_are_tab_stops`; superseded: "host picker retains the empty value until hosts arrive", since
the channel opens with a full snapshot of the pushed collections, including hosts (SPEC_impl.md, "The UI channel"). An
empty hosts collection still draws an empty Host button
(`launcher::view_tests::the_host_button_is_empty_before_hosts_arrive`). The picker disabled during a launch's write is
also superseded: the launcher hides while its create is out
(`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`)

- "host picker opened without click focus still owns the keyboard and closes outside", its keyboard half from the search
  box: `launcher::view_tests::a_mac_click_on_the_host_button_from_the_search_box_opens_the_list_and_focuses_the_button`.
- The same case's outside press closes the list without picking the highlighted host.
  `launcher::view_tests::a_press_outside_closes_the_host_list_without_picking`.

The host list keeps its own popover code rather than the shared list the launcher's text fields use
(`launcher/view/field_list.rs`): it opens from a button that takes focus on a click, and moving it over would risk that
focus handling to save a few dozen lines (decided by the migration's orchestrator on 2026-11-16).

## Launcher cases of specs listed in other areas' files

These specs' rows sit in another area's parity file; their launcher cases are this area's (decided by the migration's
orchestrator on 2026-11-14). Each case is listed by its title at reference commit `24bc0580`.

### `tests/claude-trust.spec.ts`

Tests: 4. Covers: Claude trust hint preserves the terminal and opens the opt-in caveats.

not this area's: the Claude trust hint band is drawn by the session view, so its cases belong to the sidebar and session
view area (decided by the migration's orchestrator on 2026-11-14); the Settings half of the spec (the switch its
"Settings and caveats" opens) is ported in [Settings and hosts](settings-and-hosts.md), under
`tests/workspace-trust.spec.ts`

### `tests/sidebar.spec.ts` (sidebar)

Case: the 35 `composer ...` cases (from "composer search reconciles a custom model like a harness button" to "composer
busy guard preserves one structured create through queued edits and cancel"), and the two cases among them not titled
so, "a phone-width recent row shows its folder and hides its host" and "compact recents distinguish guarded defaults and
own-setting"; also three launcher cases of the spec that no parity file listed, "the launch composer autofocuses search
and returns focus to New after Escape" (5527), "opening New focuses search and immediate typing lands there, with or
without a prior menu" (5566) and "model picks discard another harness's default YOLO" (9150).

partly: `launcher::search_view_tests::only_a_fresh_enter_on_an_empty_box_launches` and
`launcher::search_view_tests::a_scoped_miss_says_why_and_enter_changes_nothing` cover "composer no-result Enter keeps a
valid launch draft open without posting", and the model field's cases are in its own tests: "composer model combobox
filters the chosen harness's catalog and applies with Enter" in
`launcher::view_tests::{the_model_list_filters_and_enter_picks_the_highlighted_row, enter_on_typed_text_applies_a_custom_id_or_says_who_owns_it}`
and
`launcher::model_tests::{model_options_filter_and_keep_harness_defaults, model_enter_target_distinguishes_navigation_and_draft_semantics}`,
"composer model combobox refuses a custom id until a harness is chosen" in
`launcher::view_tests::{a_custom_id_with_no_agent_says_to_choose_one, enter_on_typed_text_applies_a_custom_id_or_says_who_owns_it}`,
"composer model combobox can show every harness's models and switching model switches harness" in
`launcher::view_tests::show_all_lists_every_agents_models_and_picking_one_switches_the_agent`, and "composer Escape in
the model combobox reverts the field and keeps the dialog open" in
`launcher::view_tests::{escape_closes_the_model_list_first, the_model_error_lasts_until_a_pick_or_escape}`; "composer
busy guard preserves one structured create through queued edits and cancel" by
`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher` (nothing can edit or press the launcher
while its create is out); the search cases in `launcher::search_tests` (the query's rules) and
`launcher::search_view_tests` (the box); the folder dropdown's cases ("composer folder dropdown preserves keyboard
launch and grouped history": the recents, filtering, the refused Enter that closes the list, Escape then typing, Escape
before the launcher, Tab, and eight recents after Reset to home; and the local-home reset) in
`launcher::folder_list_tests::{focusing_the_folder_lists_recent_folders_and_resets, enter_takes_a_highlighted_row_and_otherwise_launches, typing_filters_reopens_and_drops_the_highlight, a_picked_folder_does_not_filter_the_next_list, escape_and_tab_close_the_list_first, local_home_moves_to_the_local_host, a_late_answer_for_another_host_is_dropped}`;
the recent-setups strip's cases ("composer recent slots appear only with matches", "composer Enter on a focused recent
launches the filled setup", "composer recent rows lead with the harness", "composer recent rows list only explicit
choices", "compact recents distinguish guarded defaults and own-setting", "composer reserves no space for recents when
launch history is empty") in `launcher::recents_tests` and `launcher::recents_view_tests`, except their row geometry
(36-pixel rows sharing a border, a harness chip's neighbour staying in place, the cells lining up from row to row, YOLO
in the danger ink) and the dialog's height with no recents, which are layout, not separately tested (the rows are drawn
in the `app-launcher-agent` scene; the old height check was how that case saw an empty history reserve no space, and the
strip drawing nothing then is tested below); the search's recent-setup, saved-folder and history-model rows in
`launcher::search_tests::{a_typed_path_comes_before_the_saved_folders, a_bare_label_lists_its_whole_kind, a_label_does_not_broaden_into_the_other_history_kind, history_rows_match_the_raw_folder, recent_setups_match_their_place_agent_and_model, recent_setups_are_ranked_and_uncapped, past_models_follow_the_catalogs, history_row_words}`
and
`launcher::search_view_tests::{a_recent_setup_from_search_fills_and_launches_nothing, a_saved_folder_from_search_sets_the_folder, a_past_model_from_search_chooses_its_agent_type, history_rows_are_refused_where_they_cannot_apply, a_history_answer_resets_the_search_highlight}`,
except "composer search recents keep complete 44px two-line rows" (a deviation below); Browse ("composer path actions
keep typing inert and browse the selected remote host", "composer rejects held and queued stale browse activation after
destination changes", and Browse's place under the host in "composer destination block keeps host, browse, folder, and
grouped choices together") in `launcher::browse_tests`, Browse's place in Tab order in
`launcher::view_tests::cancel_and_the_host_button_are_tab_stops`; superseded: "composer holds offered history steady
until destination and search promotion", by the launch-history deviation below (an open launcher's history is not
re-read under it), "composer mounted clone generation replaces the prior draft and notice", since a Clone opens nothing
over an open launcher (`launcher::entry_tests::clone_opens_nothing_over_a_dialog_or_for_an_unknown_session`) and each
opening builds a fresh draft, and every phone-width and narrow leg (390, 540 and 700 pixels wide, "a phone-width recent
row shows its folder and hides its host" among them), which are below the minimum window (Deviations); "composer keeps
launch and cancel inside the initial viewport at default and narrow width" and "composer keeps labeled rows and a
bounded body once a harness and model are chosen" are layout, the first shown in the `app-launcher-agent` and
`app-launcher-scrolled-end` scenes and the second in the `app-launcher-codex-choices` scene, with a harness chosen (the
scene picks no model); not yet ported: a catalog model picked before any agent type choosing its agent ("composer keeps
arbitrary model-first choices reviewable"), a defaults-only recent clearing explicit choices ("composer saved defaults
clear explicit choices without posting"), a restored model holding a hidden character sent as its raw bytes ("composer
escapes a restored custom model while submitting its raw bytes"; the field and the row show it escaped), Local home
taken in a Clone ("composer local-home reset takes over a remote clone destination"), Local home counting as a hand pick
of the host ("composer destination block keeps host, browse, folder, and grouped choices together"), a press on the
folder list's heading keeping the list and focus ("composer folder history tracks edited destinations at narrow width"),
a cut recent folder's full path as its hover help and a press on Launch with the list open launching the typed folder
once ("composer folder dropdown preserves keyboard launch and grouped history"; the Reset rows' words are shown in the
`app-launcher-folder-list` scene), taking a recent replacing a shown notice ("composer reset notices follow every
restored-choice transition"), Enter again on the closed model field after a pick ("composer model combobox filters the
chosen harness's catalog and applies with Enter"), the search's highlighted row kept in view and a query with no results
clearing the highlight ("composer search keeps active results visible and focus contained"; a hovered row's look is
layout, not separately tested, since no scene hovers a search row and the highlight a hover must not mimic is
`launcher::search_view_tests::arrows_move_the_highlight_and_enter_or_a_click_takes_a_row`'s), Tab and Shift+Tab wrapping
to targets kept in view ("composer focus-trap wraps reveal their targets at narrow width"), the Tab order through the
launch groups and the footer ("composer menu-closed Tab order follows the displayed launch groups", "composer footer
names the chosen harness, host, and folder"; the placement segments' stops come with the environments work), and the
launcher opened by a click on New while a row menu is open, or by Enter or Space on New, with focus in search and typing
landing there (5566; a plain opening is tested, in the bullets below)

- "model picks discard another harness's default YOLO" (9150): a Codex model picked from the full list under Pi,
  OpenCode or Goose's default YOLO ending on Codex's Guarded, in
  `launcher::draft_tests::a_default_yolo_does_not_follow_to_the_next_agent`.
- "the launch composer autofocuses search and returns focus to New after Escape" (5527): focus in search on opening in
  `launcher::view_tests::new_opens_on_the_agent_tab_with_focus_in_the_search_box`, the first Escape closing the results
  and the second the launcher in `launcher::search_view_tests::escape_closes_the_results_and_keeps_the_query` (with a
  query that has results, where the old one had none), and focus back on New in
  `session_list::order::tests::new_opens_the_launcher_and_closing_returns_focus_to_the_button`.
- "opening New focuses search and immediate typing lands there, with or without a prior menu" (5566), a plain opening:
  focus in search in `launcher::view_tests::new_opens_on_the_agent_tab_with_focus_in_the_search_box`, and text typed
  with no click first reaching the box in `launcher::search_view_tests::a_word_and_enter_choose_it_and_empty_the_box`.
- "composer search reconciles a custom model like a harness button": the chip and the search's harness row take one
  path, whose reconciliation is
  `launcher::draft_tests::{model_choices_follow_their_owning_agent, a_switch_clears_what_the_new_agent_cannot_use_and_says_so}`,
  the notice drawn in `launcher::view_tests::a_switch_that_clears_a_choice_says_so`.
- "composer keeps a deliberate structured choice through a late catalog": a catalog answer never edits the draft, in
  `launcher::view_tests::{a_carried_effort_the_model_does_not_offer_stays_offered, a_clone_launches_while_the_catalog_read_is_out, a_clone_launches_after_a_failed_catalog_read}`.
- "composer escapes a restored custom model while submitting its raw bytes", the display: the field in
  `launcher::view_tests::agent_launch_text_holding_invisible_characters_shows_escaped` (the draft keeps the raw id), the
  strip's row in `launcher::recents_view_tests::a_row_names_a_custom_model_escaped`.
- "composer isolates strong RTL destination and model summary values": the footer draws them as host text, whose
  isolation is
  `host_text::tests::{a_right_to_left_host_part_reorders_only_within_its_isolate, on_macos_core_text_keeps_a_sentence_flowing_and_its_host_parts_isolated}`;
  no launcher test draws a right-to-left value.
- "composer name field reaches the create request; an empty name sends none":
  `launcher::request_tests::{an_agent_create_body_carries_the_draft, a_command_create_body_sends_the_command_as_written}`,
  the Name row on opening in `launcher::view_tests::new_opens_on_the_agent_tab_with_focus_in_the_search_box`.
- "composer path actions keep typing inert and browse the selected remote host" and "composer rejects held and queued
  stale browse activation after destination changes":
  `launcher::browse_tests::{browse_this_path_from_search_sets_the_folder_and_browses_it, browse_lists_one_level_and_taking_a_folder_chooses_it, a_reconnect_forgets_the_listing, an_edit_of_the_folder_forgets_the_listing, a_host_change_forgets_the_listing}`;
  a queued edit and click in one event turn is superseded, since GPUI runs handlers in order against the current draft.
- "composer recent slots appear only with matches" and "composer reserves no space for recents when launch history is
  empty": `launcher::recents_view_tests::{the_strip_shows_matching_setups_only, an_empty_history_shows_no_strip}`.
- "composer destination block keeps host, browse, folder, and grouped choices together", Browse under the host:
  `launcher::view_tests::the_host_button_is_as_wide_as_its_host_with_browse_under_it`.

### `tests/session-action-completion.spec.ts` (sidebar)

Case: row Replace YOLO arrival respects ${focus} focus.

replaced, by the sidebar, which asks the launcher's YOLO question at the row and header: its YOLO cases in
`session_list/lifecycle_tests.rs` and `session_pane/header_tests.rs`, and smoke step `test_54`

- The moved-on variant: `session_list::lifecycle_tests::a_yolo_question_after_the_user_moved_on_waits_without_focus`,
  and at the launcher's end `launcher::yolo_tests::the_plain_replace_question_leaves_focus_once_the_user_moved_on`; the
  other:
  `session_list::lifecycle_tests::{a_yolo_refused_replace_asks_the_launchers_question_in_its_row, a_yolo_question_that_takes_focus_scrolls_its_row_into_view}`
  and the header's `session_pane::header_tests::a_yolo_refused_replace_from_the_header_asks_below_its_top_row`; over the
  real helm, `scripts/gui-e2e-smoke.py`'s `test_54_a_yolo_replace_asks_in_its_row_and_the_one_off_answer_replaces`.

### `tests/replace.spec.ts` (sidebar)

Case: the 8 Replace with cases (lines 801, 1024, 1081, 1166, 1281, 1384, 1446, 1503 at the pin), and the launcher
variants of two cases no parity file listed: "launchers start fresh after repairing a missing folder (${surface})"
(161), whose new, clone and replace-with variants are the launcher's and whose sidebar and header ones are plain
Replace's, and the Replace with variants of 539, "Replace with selects its replacement after an earlier complete fleet
refresh" and "Replace with reconciles an externally removed source after refusal" (the plain Replace variants are the
sidebar's).

partly: line 1166's prefill and launch in
`launcher::replace_with_tests::{replace_with_prefills_the_source_and_holds_its_host, replace_with_warns_about_a_running_source_and_sends_its_guard, replace_with_opens_its_replacement_only_where_the_source_was_open}`
(its edit through search in `launcher::search_view_tests::a_word_and_enter_choose_it_and_empty_the_box`, the same path
in every mode); 1281 in `launcher::replace_with_tests::cancelling_replace_with_sends_nothing`; 1384 in
`replace_with_warns_about_a_running_source_and_sends_its_guard`, and its recent-setup row's "Replace:" name and "⏎
Replace" hint in `launcher::recents_view_tests::replace_with_says_replace_and_restart_with_has_no_strip`, except Launch
and that row naming the warning as their accessible description (deviations below); 1446 and 1503 in
`launcher::replace_with_tests::replace_with_follows_the_source_and_sends_the_guard_it_shows` (the guard a press sends;
the helm keeps a source more alive than shown); also
`launcher::replace_with_tests::{a_refused_replace_with_comes_back_and_stays_on_the_source, replace_with_waits_while_its_source_is_busy}`;
1024 and 1081 (focus on search) in `launcher::search_view_tests::clone_and_replace_with_start_focus_in_the_search_box`;
superseded: 801 (retry keys; the GUI sends none) and 161's check that the second attempt carries a new key; not yet
ported: 161's second press, once the folder is repaired, sending a new create or Replace and opening its session (the
refusal half is below), and 539's refusal variant, the launcher showing the refusal after the source was removed
elsewhere while the Replace with was out, and the selection then falling back to the session visited before

- 1024 and 1081, the row menu opening Replace with on its row:
  `session_list::menu_tests::the_menus_clone_and_replace_with_open_the_launcher_on_their_row`.
- 161, the refusal half: the refusal brought back with the draft, for New in
  `launcher::launch_tests::a_refused_launch_brings_the_launcher_back_with_the_helms_words` (Clone goes through the same
  submit, as `launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher` says) and for Replace with in
  `launcher::replace_with_tests::a_refused_replace_with_comes_back_and_stays_on_the_source`.
- 539's success variant: a listing that drops the source and lists the replacement arrives before the reply, and the
  replacement still opens, in the first half of
  `launcher::replace_with_tests::replace_with_opens_its_replacement_only_where_the_source_was_open`.
- 1166, the Replace body built from the create's:
  `launcher::request_tests::a_replace_with_body_carries_the_create_and_the_shown_guard`.

### `tests/terminal-restart.spec.ts` (sidebar)

Case: an interrupted legacy session offers Replace with, prefilled with its command.

replaced: `launcher::draft_tests::clone_seeds_command_and_legacy_launches_on_the_command_tab` (the prefill, for Clone);
the card's offer of Replace with in place of Replace in
`session_pane::card_tests::a_legacy_sessions_card_offers_replace_with_in_place_of_replace`

- The card's Replace with opening the launcher in Replace with of its session, on the Command tab with the stored
  command: `session_pane::card_tests::the_cards_replace_with_opens_the_launcher_on_the_sessions_command`.
- The card's words naming Replace with and never promising that "Replace starts it over":
  `session_pane::card::tests::the_cards_words_say_what_ended_and_what_each_choice_does`.

### `tests/restart-replace-progress.spec.ts` (sidebar)

Case: Restart with keeps its draft and avoids a safe-retry promise after a deadline.

replaced: the Restart with request shares the session's pending line
(`launcher::restart_with_tests::restart_with_stops_a_working_agent_with_consent`) and keeps its draft on a refusal
(`restart_with_settles_as_plain_restart_does`); the old return of the dialog after the user switched away is the notice
deviation below

- A timed-out Restart with coming back with the helm's words and no "Restart error." lead:
  `launcher::restart_with_tests::a_timed_out_restart_with_settles_its_row_before_the_launcher_returns`.

### `tests/terminal.spec.ts` (terminal)

Case: its six create-form cases, "the create form prefills the working directory with ~ when no session is selected",
"multi-session flow: create two, open and type in one, stop and delete the other, then delete the first with
confirmation" (its launcher half), "create dialog surfaces a precondition failure, preserves the form, and creates no
session", "create dialog disables the submit control while a create is in flight", "submitting the create form twice
while one create is in flight produces exactly one session" and "a blank title creates a session titled after the
working directory's basename, not an empty string". Only the launcher's cases are listed here; the spec's other cases
are the terminal area's, whose file tracks the spec, and the sidebar and session view's.

replaced: every case's launcher half is named below

- "the create form prefills the working directory with ~ when no session is selected": New without an open session
  starts in `~` in `launcher::folder_default_tests::new_waits_for_the_hosts_last_folder_and_starts_there`, and a history
  with no launch into a plain folder leaves it there in
  `launcher::recents_tests::the_remembered_folder_is_the_newest_plain_folder_launch`.
- "multi-session flow: create two, open and type in one, stop and delete the other, then delete the first with
  confirmation", the launcher's half, each create opening its session:
  `launcher::launch_tests::a_launch_the_user_waited_for_opens_its_session`. Typing in the open session is the
  terminal's, and Stop and Delete with its confirmation are the sidebar and session view's.
- "create dialog surfaces a precondition failure, preserves the form, and creates no session": the refusal shown after
  "Not launched." with the draft kept, and an edit taking it away, in
  `launcher::launch_tests::a_refused_launch_brings_the_launcher_back_with_the_helms_words`; one press sends one create
  (`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`); a second press after fixing the folder
  launches and opens the session in `launcher::launch_tests::a_second_press_after_a_refusal_launches_the_fixed_draft`.
- "create dialog disables the submit control while a create is in flight": the launcher hides while its create is out,
  so nothing in it can be pressed (`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`), and New
  is unavailable meanwhile (`launcher::entry_tests::new_waits_for_a_pending_launch_and_then_brings_back_its_refusal`).
- "create dialog disables the submit control while a create is in flight", opening another session while the create is
  out (with nonblocking-launch's 59): `launcher::pending_tests::the_window_keeps_working_while_a_create_is_out`.
- "submitting the create form twice while one create is in flight produces exactly one session":
  `launcher::launch_tests::{launching_sends_one_create_and_hides_the_launcher, a_repeated_enter_on_launch_still_opens_the_session}`.
- "a blank title creates a session titled after the working directory's basename, not an empty string": a blank name
  sent as no title in `launcher::request_tests::an_agent_create_body_carries_the_draft`; the title then derived from the
  folder is the supervisor's.

### `tests/terminal-multihost.spec.ts` (terminal)

Case: its eight create and Clone cases, each titled after its label: "create-dialog-host-selector", "clone-cross-host",
"clone-cross-host-explicit-edit", "create-dialog-selector-disappearance", "create-dialog-selector-disabled-in-flight",
"create-on-unreachable-refused", "create-intent-key-rebinds-on-host-change" and "create-intent-key-rebinds-on-retarget".
Only the launcher's cases are listed here; the spec's Uninstall, Retry and Adopt host cases are in
[Settings and hosts](settings-and-hosts.md), and its stale-session cases are the sidebar and session view's, as the
terminal area's entry for the spec says.

partly: four cases are named below; superseded: the two intent-key cases, since the GUI sends no create keys (SPEC.md,
"Creation" and "Losing the connection to the helm"), and "clone-cross-host-explicit-edit", whose command was overwritten
when the old launcher bound a Clone's host after its hosts read: a Clone's host is decided once, as it opens, from the
hosts the helm has pushed (SPEC_impl.md, "The UI channel"), falling back with a note when the source's host is not among
them (`launcher::entry_tests::clone_of_a_session_on_a_retargeted_host_row_falls_back_and_says_so`), and nothing binds it
later (Deviations, the first bullet); not yet ported: "create-dialog-selector-disappearance", a chosen host leaving the
registry while New is open (how the launcher answers it is a deviation, in Deviations): the refusal's words are checked
for an unconnected host in `launcher::request_tests::refusals_name_the_first_thing_to_fix`, but no test removes the
chosen host

- "create-dialog-host-selector": the pick in
  `launcher::view_tests::{the_host_list_offers_every_host_with_its_phase_and_a_click_picks_one, the_host_picker_browses_with_arrows_and_enter}`,
  and the create carrying the draft's host in `launcher::request_tests::an_agent_create_body_carries_the_draft`; the
  row's remote mark is the session list's.
- "clone-cross-host": a Clone opening on its source's host with its command, and its create sent there, in
  `launcher::entry_tests::{clone_prefills_the_sources_host_folder_title_and_launch, a_clone_launch_sends_the_copied_launch}`.
- "create-dialog-selector-disabled-in-flight": the launcher, its host picker and folder included, hides while its create
  is out (`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`; Deviations).
- "create-on-unreachable-refused": a host that is not connected refuses Launch before anything is sent, in
  `launcher::request_tests::refusals_name_the_first_thing_to_fix` ("Choose a connected host before launching", where the
  old note said "The selected host is unavailable; choose a connected host before launching") and
  `launcher::view_tests::a_refused_press_says_why_beside_the_main_action`.

### `tests/tensorlake-lifecycle.spec.ts` (settings and hosts)

Case: its 13 cases of a launch that resumes its suspended host first (lines 216 to 630 and 1131 to 1835 at the pin); its
other 8 cases are host and session lifecycle and stay with that file's entry.

deferred: resume before create is a near-term TODO item; all 13 cases depend on it.

## Deviations

- The launcher's draft does not follow later changes to its source session while it is open (a Clone's folder, a Restart
  with's settings), as in the old app, nor a Clone's host: the old launcher withdrew a Clone's carried host, with its
  note, if the host's row was retargeted onto another install while the launcher stayed open, and this one keeps the
  host it opened with, sending the row's install as it stands at Launch; Restart with refuses ("This session's launch
  settings changed; close and reopen restart with") when the source's stored launch changed, and that refusal is shown
  in advance as the hover reason rather than only after a press.
- A command line the shared command-launch check refuses is refused before sending in every launcher mode; the old app
  checked it only in Restart with and left the rest to the helm.
- A launch on a suspended environment or a stopped one that offers Resume names its host and asks you to resume it in
  Hosts, then launch. Resume before create is a near-term TODO item, as SPEC.md's interim Topology note records. Both
  providers' refusals are checked by
  `launcher::request_tests::a_suspended_environment_names_the_resume_needed_before_launch`; quick launch reveals the
  advice after "Not launched." in `templates::dispatch_tests::a_suspended_quick_launch_asks_to_resume_its_named_host`.
- While a new environment's recipe is chosen, the composer search's folder rows and Browse are refused in the old words,
  and so is a `~` path, which the old search took as a choice of the home the folder already was.
- New sandbox reads the Tensorlake account once as the launcher opens; the old launcher read it again on every fleet
  notice. Settings, where it changes, opens from the page the launcher covers, so a key saved there shows at the
  launcher's next opening either way. A failed read is not retried, so it leaves New sandbox hidden until the launcher
  is reopened.
- New sandbox's recipes are listed with the cube mark and their CPU and memory ("ready · 2 CPUs · 4 GB memory"), as
  SPEC.md's "Sandbox sessions" asks; the old select listed names alone, with a refused recipe's reason after its name.
- "Create a sandbox for this session?" has no sandbox mark before its title: the old question drew the cube mark there,
  and the prompt's inline question carries no tile. Its focus starts on Cancel, where the old question started on
  "Create and launch"; that is SPEC.md's rule for a risky answer ("Keyboard focus"), not a deviation.
- Whether "Create Tensorlake sandboxes without asking" skips the question is read from the account as the launcher read
  it when it opened; the old launcher read the setting again at each press (the stale value accepted by the launcher
  area on 2026-11-16). A setting turned off since makes the helm, which checks it again, refuse the unconfirmed create
  with its own words ("confirm creation of a billed sandbox before launching") rather than the question; the next press
  in that launcher asks the question, since a launcher that has sent one create on the setting asks from then on. A
  setting turned on since still asks while the launcher stays open.
- The question shows the recipe's size as the catalog held it at the press, and the helm creates the sandbox at the
  recipe's size when it receives the create; a recipe edited while the question is up is not re-asked about.
- An applied template batch that leaves the launch unchanged keeps an open YOLO question, as the old launcher did, but
  retires an open "Create a sandbox for this session?", which the next press asks again. Whether the old launcher kept
  its sandbox question across such a batch was not established.
- New container reads each host's Podman state from its pushed check and asks for a check of every container host as the
  launcher opens; it does not read `GET /api/environment-places`, so a host never checked says "this host needs a Podman
  check" until its check answers, where the old select showed the places read's words.
- With a new environment chosen, the environment's reasons (recipes loading or unreadable, the recipe's own refusal, the
  container host's) come first, in the host's place, before the destination's and the launch's; the old app checked them
  after the agent tab's own reasons (a missing harness, say), so a draft with both now says the environment's.
- The folder field a check or a refusal finds wrong is marked by a warning-colored border and the line under it; the old
  field also set `aria-invalid`, which the GPUI text field does not expose, so a screen reader finds the line (a
  labelled node beside the field) rather than an invalid field.
- Replace with (and Restart with) refuses Launch with "Wait for this session's other operation to finish" while its
  source has another request in flight; the old app disabled it without a reason while its page-wide operation token was
  held.
- Replace with's Launch does not name the source warning as its accessible description, as the old button did with
  `aria-describedby`; the warning beside it is its own announced status instead. The button's description comes from its
  hover help, and putting the long warning into a tooltip would change what sighted users see.
- A launch refused after the user moved on is offered by a notice with Open launcher, as SPEC.md now says; the old app
  brought the launcher back by itself as soon as no other dialog was open.
- Opening a launched session does not itself close Hosts; the old app's opening did. New returns the sidebar to Sessions
  as it opens, from its button, its shortcut or the quick switcher (SPEC.md, "All or a host avatar returns to Sessions,
  as does an accepted quick-switcher pick or New handoff";
  `session_list::order::tests::the_new_action_leaves_the_hosts_panel_only_when_new_opens`), and opening Hosts while the
  create is out counts as moving on, after which the session is not opened at all
  (`session_pane::tests::a_first_launch_the_user_moved_on_from_selects_nothing`), so the difference shows only for a
  launcher opened some other way while Hosts is shown.
- A create the host does not answer within the helm's 20-second bound comes back with the helm's reply ("the host did
  not answer within 20 seconds; the request is still being carried out, and the session list will show its result") in
  the refusal's place, without "Not launched.", and keeps its draft; moved on from, it has its own notice. The old app
  gave up waiting after its own 60-second deadline and brought the launcher back with "Still waiting on <host>. The
  session may still start; it will appear in the sidebar if it does.", promising that launching again with the same
  settings would not start a second copy, which a GUI that sends no launch request keys cannot promise. Tests:
  `launcher::launch_tests::a_timed_out_create_comes_back_without_not_launched`,
  `launcher::launch_tests::a_create_the_host_never_accepted_is_not_launched`,
  `launcher::launch_tests::a_timed_out_create_the_user_moved_on_from_has_its_own_notice`.
- While one of the window's host-registry changes is in flight, Launch is refused with a reason ("Wait for the host
  change to finish") as its hover text and after "Not launched." on a press; the old app only disabled it. A new host's
  probe counts as such a change, which the old app's token left out, and a save of a registered host's settings (its
  destination and alias included) does not, which the old token covered; both decided by the migration's orchestrator on
  2026-11-14 (`launcher::launch_tests::launch_does_not_wait_for_a_host_settings_save`).
- A managed checkout's create that the helm leaves ambiguous (a timeout, a lost reply) is not retained and retried as
  the same request: the GUI sends no launch request keys, so the old "Retry reconciles the original request at this
  path" has no counterpart, and pressing Launch again asks for a new checkout (decided for every launch: duplicates
  around a lost connection are accepted).
- A managed checkout's preview is asked for one at a time: while one is out, a later change waits for its answer and
  then asks once for the draft as it is then. The old launcher sent a preview for every change (each key of a typed
  name) and dropped the superseded answers; either way only an answer for the current draft is shown.
- The repository field does not report the old combobox states to assistive technology (expanded, the highlighted row as
  its active descendant); the field and the suggestions' rows have their names and roles, as the model field's do. The
  suggestions are drawn in the shared option-row style, like the model list's.
- A Clone of a session in a managed checkout carries the repository only when the shared repository parser reads it back
  unchanged, as every repository the helm and supervisor accept does. For any other identity (one with a capital or an
  invisible character, say) the Clone opens on a managed checkout with no repository chosen, so a single press of Launch
  cannot start it in the source's own checkout folder; choosing Folder brings that folder back, as for any Clone. The
  old launcher carried such text; one with an invisible character was shown escaped and read-only behind a "Clear
  repository" button, which has no counterpart.
- The inline checkout-folder setup is drawn in Settings' look (its label above the field), indented into the launcher's
  control column; the old launcher drew it in its two-column rows with its error in the warning color.
- The footer's prompt count says "1 line" for a one-line prompt; the old footer said "1 lines".
- The footer's lines are cut at the footer's edge without an ellipsis; their full text is each line's accessible name,
  and the target line's hover help.
- The footer's line of choices is not a live region: a screen reader is not told when it changes, as the old
  `aria-live="polite"` line did. The main action's accessible name carries the target.
- The permission help "?" is drawn as a neutral button; the old one was the bare `.btn` at 30 by 36 pixels.
- The model list's rows are drawn in the shared option-row style (the quick switcher's, 9 by 10 pixels of padding); the
  old list's buttons were 32 pixels high with a 1.5-pixel edge.
- The model field does not report the old combobox states to assistive technology (expanded, the highlighted row as its
  active descendant, invalid while an error shows); the field and the list's rows have their names and roles.
- The host list takes its width from its hosts' names, at least 12em and at most 30em; the old list was also at least as
  wide as its button, which differs only when the chosen host's name is the longest and wraps.
- The host button's accessible name is "Host: <host>"; the old combobox was named "Host" and reported the host as its
  value.
- Neither the host button nor the model field reports its list as expanded or its highlighted row as the active
  descendant to assistive technology; the rows themselves report their selected state. The shared button's expanded
  state also draws its pressed look, which the old host button never had, so the host button reports neither.
- The effort control, a select once an agent offers more than five efforts, opens its list on the arrow keys; the old
  native select changed the value in place (`launcher::view_tests::many_efforts_are_a_select`,
  `components::tests::select::the_select_keys_open_move_choose_and_close_as_a_native_pop_up`).
- A press of Launch refused for a wrapper prefix that does not parse says "Correct the wrapper prefix before launching"
  (`launcher::request_tests::refusals_name_the_first_thing_to_fix`); the old press gave the parse error, "Invalid
  wrapper prefix: …", which the line under the field now gives
  (`launcher::view_tests::a_wrapper_prefix_that_does_not_parse_is_said_at_once`).
- The launcher's labels are not selectable text, and a click on one counts as a click on empty space (SPEC.md, "Opening
  and closing"); in the old launcher a double click on the dialog's gaps had to be kept from selecting text ("launcher
  gap keeps focus and container Enter explains an incomplete draft"), which has no counterpart.
- While a launch's create is out, the old launcher stayed on screen with its host picker disabled; this launcher hides
  for the whole time the create is out instead, so neither the picker nor anything else in it can be pressed
  (`launcher::launch_tests::launching_sends_one_create_and_hides_the_launcher`).
- A new environment with no recipe chosen is named "new sandbox" or "new container on <host>" in the footer and in
  Launch's name; the old launcher named it with a double space where the recipe would be (`new  sandbox`)
  (`launcher::sandbox_view_tests::a_new_environment_without_a_recipe_is_named_without_one`).
- The sandbox registration note's command sits in the shared command well, boxed with its Copy beside it, as Settings →
  Tensorlake shows it; the old launcher showed it as inline code with a button beside it
  (`launcher::sandbox_view_tests::the_registration_note_shows_and_copies_the_command`).
- A host removed from the registry while New has it chosen stays chosen, and Launch is refused with "Choose a connected
  host before launching", the words any unconnected host gets. The old launcher said the host was no longer registered,
  in a note under its host field, which then stopped showing the removed host.
- The host button, Browse and Browse's listing entries have the frame's 1-pixel edge; the old ones had 1.5 pixels.
- The composer search's results are drawn as one flat list under group headings; the old area nested each group in a
  `role="group"` node named by its heading. Flat, every row is the list's own child, so the arrows keep the highlighted
  row itself in view rather than its whole group. The box does not report the old combobox states (expanded, the
  highlighted row as its active descendant), as the model field does not.
- The composer search refuses a row that would change what the open launcher holds fixed in every mode that holds
  fields: Restart with's agent type, host, folder and name, as the old one did, and also Replace with's host, which the
  old search did not refuse. That refusal reads "host cannot change here: this dialog keeps host fixed for replace"
  (`launcher::search_view_tests::replace_with_refuses_a_host_row`).
- A refused search row keeps the query in the box and its reason until the next keystroke there; the old box emptied on
  a refusal and kept the reason until the next accepted row. Keeping the query lets the user correct it in place.
- New's folder shows `~` while it waits for the chosen host's history after a host change; the old field kept showing
  the previous host's folder until the answer. Launch waits either way ("Wait for this host's last folder before
  launching").
- The permission-escalation banner's eight seconds keep running while the window is inactive; the old banner's clock
  paused then. SPEC_impl.md, "Animation time: the ticker", has one-shot deadlines, banner expiry among them, fire on
  time whether or not the window is active.
- The composer search's recent-setup rows are drawn in the shared option row's padding with the list's 2-pixel gap; the
  old rows were a fixed 44 pixels tall with 4 pixels between them ("composer search recents keep complete 44px two-line
  rows"). The rows are drawn in the `app-launcher-search-recents` scene.
- The launch history is read when the launcher opens and whenever its host or that host's connection changes; the old
  app also re-read it on every activity-feed notice and on a fallback timer. The desktop app has no activity feed, and
  an open launcher's history changes only by a launch from another window, which shows at the next opening. The checkout
  configuration's revision comes with that read, so a checkout configuration change made from the command line while New
  is open (another checkout folder, say) is not followed either: a preview already shown stays until the next read or a
  press, which the helm refuses as stale and the launcher answers with a new preview and nothing sent
  (`launcher::checkout_tests::a_lost_race_takes_a_new_preview_and_sends_nothing`). The helm's history of launches into
  new environments, which the old app re-read on the same notices, is read once as the launcher opens.
- The folder dropdown leaves out Local home in Replace with, which keeps its host; the old dropdown offered it there too
  (`launcher::folder_list_tests::replace_with_offers_no_local_home`).
- The folder dropdown's headings (Recent, Reset) are labelled lines among the options, not groups named by them, so its
  rows scroll into view one by one; and like the model field, the folder field reports no expanded or active-row state.
- The recent setups strip's cells are fixed tracks in pixels rather than the old `ch` units (the host and choices tracks
  shrink only when the row is narrow, as the old `minmax(0, …)` did), and at narrow widths the host cell is not hidden
  as the old `max-width: 560px` rule hid it. Its rows keep their color under the pointer rather than turning to the
  primary ink, and in Replace with a row does not name Replace with's warning as its accessible description (the old
  `aria-describedby`); the warning beside Launch is its own announced status.
- The window cannot be made smaller than 720 by 560 logical pixels (SPEC.md, "Desktop window chrome"; `MIN_WINDOW_SIZE`
  in `crates/gui/farhelm-gui/src/bootstrap.rs`), so the old cases' legs below that size have no counterpart: every leg
  narrower than 720 pixels (the 360- and 390-pixel phone widths, the YOLO question's among them, and the 540- and
  700-pixel narrow ones) or shorter than 560 (the alignment case's 1280 by 540). Only what such a leg checks at the
  minimum window and above is ported, as `other-dialogs.md` (Trash) and `templates-and-quick-launch.md` (Superseded)
  record for those dialogs' phone layouts.
- In Restart with, a stored command line or resume command holding a hidden character is read-only and spelled out, with
  its Clear, whenever the field holds the stored value, so typing it back exactly as stored makes it read-only again;
  the old field stayed editable after its first edit (`list/create_form.rs` at `24bc0580`: `restart_command_readonly`
  and `restart_resume_readonly` need no edit since opening, `!invocation_edited()` and `!command_resume_edited()`). The
  GPUI rule compares the field with the value the launcher opened with, which in Restart with is the stored one
  (`launcher/view/shown_text.rs`, `command` and `resume`).
  `launcher::restart_with_tests::restart_with_shows_stored_command_values_escaped` checks that the spelling and the
  Clear come back.
