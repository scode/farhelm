# Parity: Settings and hosts

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Topology, Supported host setup, Local authority and trust between hosts.

`tests/host-updates.spec.ts` is split. Five of its tests are this area's: the host progress card ("host progress cards
agree across surfaces and follow connection detail", "host progress previews only diagnostics that cannot fit beside its
current step" with its no-steps variant, "a one-step host card keeps a fitting multiline diagnosis whole"), Update all
and the single Update ("Update all includes required and optional rows but no other attention", "one listed update uses
its own Update action"). The first two hover the card from an attention row and from the Hosts panel; the one-step test
only from an attention row. These five drive the card, Update all and Update through attention rows, so they also wait
for the attention list. The other ten, the attention list itself and its Always update, are in
[Sidebar and session view](sidebar-and-session-view.md). What Always update does (the requests, the preference save, its
failure and Try again) is this area's (`crates/gui/farhelm-gui/src/hosts/status.rs`), tested where the row below names
it; the sidebar's row keeps the cases that draw it.

### `tests/settings.spec.ts`

Tests: 27. Covers: settings focus stays on its painted fill and descriptions stay with their labels.

partly (below)

### `tests/credentials-settings.spec.ts`

Tests: 3. Covers: credentials keep fixed names, replace write-only values and discard login drafts.

partly (below)

### `tests/credential-delivery.spec.ts`

Tests: 2. Covers: saved credentials report delivery failures without retaining the secret draft.

partly (below)

### `tests/ntfy-settings.spec.ts`

Tests: 3. Covers: ntfy settings explicitly save, refuse, preserve and clear a token.

replaced (below)

### `tests/tensorlake-settings.spec.ts`

Tests: 6. Covers: Tensorlake settings save a write-only key, test the saved key and clear it.

partly (below)

### `tests/tensorlake-recipes.spec.ts`

Tests: 10. Covers: Settings keeps recipe answers sticky and isolated to Tensorlake in ${palette}.

partly (below)

### `tests/tensorlake-hosts.spec.ts`

Tests: 8. Covers: Add host pins inline recipe answers and focuses Name in ${palette}.

not yet ported

### `tests/tensorlake-lifecycle.spec.ts`

Tests: 21. Covers: wake-to-create leaves ${elsewhere} alone (${mode}).

not yet ported

### `tests/tensorlake-tunnels.spec.ts`

Tests: 2. Covers: tunnel failures make a connected host amber and show each mapping.

partly (below)

### `tests/hosts-panel.spec.ts`

Tests: 2. Covers: Hosts swaps ordinary content and preserves filters, sort, selection and details.

partly: crates/gui/farhelm-gui/src/hosts/tests.rs `the_heading_counts_the_hosts_the_helm_lists` (the region named "Host
management"), `details_open_and_close_every_row` (the Details choice lives in the one panel the sidebar keeps, so it
survives the swap). The swap, from the sidebar's Hosts control on the host rail's foot:
crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs `hosts_swaps_the_list_for_the_panel_and_back_keeping_focus`
(the panel in place of the list, Hosts pressed and keeping focus, nothing sent, so the host choice, sort and selection
stay, the selection also checked, and Details kept across the swap),
`choosing_all_or_a_host_returns_from_the_panel_to_sessions`; scripts/gui-e2e-smoke.py
`test_39_the_rail_foot_toggles_hosts_and_opens_settings`; the `hosts-shown-with-a-host-chosen` golden (the panel in
place of the list, Hosts pressed, the chosen avatar in the quieter look). Hosts also reports itself expanded while the
panel shows, which no test reads back, since GPUI's driver record has no expanded field. Waiting: a quick-switcher pick
or New handoff returning to Sessions (the sidebar's); the second case, the header's Clone from Hosts, is the session
header's

### `tests/host-updates.spec.ts`

Tests: 5. Covers: host progress cards agree across surfaces and follow connection detail.

partly: the Hosts heading's Update all, which the old cases reached through the attention list, is
crates/gui/farhelm-gui/src/hosts/update/tests.rs `update_all_updates_every_available_remote_host_and_skips_the_rest` and
`update_all_lets_each_host_keep_its_own_result` (see "Update" below). The per-host state the attention list reads is
crates/gui/farhelm-gui/src/hosts/status/tests.rs: which hosts get an Update row, required or optional ("Update all
includes required and optional rows but no other attention"),
`the_badge_needs_an_available_update_and_an_older_build_or_protocol`, and the working host left out of them,
`live_work_is_the_running_run_or_else_the_requests_out`; a row's Update and the list's Update all call
`hosts::update::start` and `start_all`, tested above. Always update's request, save, failed save surviving Hosts, Try
again and newer choice, which the sidebar's Always update cases draw, are
`always_update_requests_the_hosts_then_saves_and_a_failed_save_waits_for_try_again` and
`a_newer_choice_in_settings_or_elsewhere_retires_the_failed_save`; the setup or Update warning the rail and the list
show is `the_hosts_control_acknowledges_the_warning_and_failures_while_it_shows_warn_nothing`,
`a_refused_update_warns_and_a_reset_forgets_it`, `a_withdrawn_run_warns_and_the_next_run_does_not`,
`a_refused_setup_rerun_and_a_refused_submission_warn`, `an_unreadable_plan_and_a_refused_confirmation_warn` and the rule
tests beside them. The progress card itself (`crates/gui/farhelm-gui/src/hosts/progress/card.rs`, entry point
`hosts::progress_card`) is crates/gui/farhelm-gui/src/hosts/progress/card/tests.rs: its content for the first case,
`the_card_shows_the_host_its_count_time_and_every_step`, its live connection detail,
`the_connection_detail_follows_the_hosts_phase`, and the elapsed time it shares with the Hosts row,
`the_card_counts_from_the_runs_first_sight_and_by_itself`; the previews,
`a_short_window_previews_only_diagnostics_that_cannot_fit` and `a_run_without_steps_yet_previews_its_diagnostics_too`,
with the share and the exact fit (the one-step case) in
`the_shorter_text_stays_whole_and_the_longer_ones_share_the_rest` and
`a_fitting_text_stays_whole_and_a_preview_keeps_whole_lines`; the goldens `host-progress-card` and
`host-progress-card-preview`. The Hosts-row halves are ported: hovering a row's running status shows the card beside the
sidebar and leaving takes it away, `hovering_the_running_status_shows_the_card_beside_the_sidebar` in the same file.
Still waiting: the attention-row halves of all five cases (the card on a working row, and Update all and Update drawn
through attention rows), which wait for the attention list

### `tests/host-checkout-folder.spec.ts`

Tests: 1. Covers: host checkout folder saves, inherits, clears and reaches launch preview.

partly (below)

### `tests/provisioning.spec.ts`

Tests: 73. Covers: rail ${action} modal stays visible over Sessions and returns to its avatar.

partly (the Hosts panel's rows, a host's menu with Settings and Remove, the host settings dialog and Remove host; see
"The host menu and host settings" and "Remove host" below): crates/gui/farhelm-gui/src/hosts/offers/tests.rs
`the_local_hosts_update_is_greyed_and_points_at_the_installer` and crates/gui/farhelm-gui/src/hosts/menu/tests.rs
`the_local_hosts_greyed_update_says_why_and_sends_nothing` ("the local row's update is greyed out, points at the
installer, and sends nothing"), `work_in_flight_withdraws_update_and_rerun_and_holds_settings_uninstall_and_remove` and
menu/tests.rs `settings_waits_for_the_hosts_own_work_in_flight` ("Update planning keeps only its own Settings
unavailable"), `a_running_run_holds_the_host_until_it_ends` ("accepted ADD ... while running": no Update or Re-run and
Settings unavailable during the run, Update after it), `a_too_new_host_is_not_offered_update`,
`a_suspended_host_is_not_offered_update_or_rerun`, `a_failed_run_is_offered_again_as_what_it_was`,
`a_failed_update_on_the_local_host_is_offered_again_greyed`, `the_local_host_without_a_supervisor_offers_no_install`,
`each_kind_decides_its_own_offers`; crates/gui/farhelm-gui/src/hosts/tests.rs `details_open_and_close_every_row` and
`a_failed_run_opens_only_its_own_row` ("global details alone control disclosure", without the update progress),
`the_local_host_without_a_supervisor_opens_its_row_to_say_what_to_do` ("local setup answers an absent supervisor",
without the probe and its handoff, which are not ported), `a_refusal_shows_under_its_row_and_opens_it`;
crates/gui/farhelm-gui/src/hosts/identity/phase/tests.rs (phase words, Too new hover, details);
crates/gui/farhelm-gui/src/hosts/settings/tests.rs `the_dialog_closes_when_the_registry_no_longer_lists_its_host` (the
settings dialog closing when its host disappears); and Add host's dialog, probe and setup confirmation (below). setup
progress, failure and rerun (below, "Setup progress, failure and rerun"). Update, its progress, refusals, rerun and
Update all (below, "Update"). Uninstall, its refusals, run and removal notice (below, "Uninstall"). The rail's entry
points: crates/gui/farhelm-gui/src/session_list/host_rail/avatar_menu_tests.rs
`a_secondary_click_on_an_avatar_opens_its_hosts_menu_beside_it` (the avatar opens its host's menu as the opener every
dialog from it returns to). Waiting: the menu's other commands

### `tests/ssh-help.spec.ts`

Tests: 4. Covers: ssh help displays and copies both registered-host commands.

ported (below)

### `tests/workspace-trust.spec.ts`

Tests: 3. Covers: managed-checkout trust switches persist independently of Claude's legacy route.

partly (below)

### `tests/appearance.spec.ts`

Tests: 4. Covers: ${initial.name} before application mount.

partly (below)

### `tests/podman-environments.spec.ts`

Tests: 10. Covers: Containers Settings keeps failed cleanup explicit.

partly (below)

## `tests/settings.spec.ts`, partly

The dialog's shell and its Sessions, Appearance, Hosts, Containers, Credentials, Terminal, Checkouts, Agent trust,
Notifications and App sections are ported; the cases about the other sections (Recipes) wait for them. Tensorlake is
listed in its place, between Recipes and Containers, with part of its content (its own cases are in
`tests/tensorlake-settings.spec.ts`, below). Containers' own cases are in `tests/podman-environments.spec.ts`, and
Credentials' in `tests/credentials-settings.spec.ts` and `tests/credential-delivery.spec.ts`, below. App, listed only in
the installed desktop app (where its updater runs), had no browser spec, since the web interface never showed it.
Notifications' own cases are in `tests/ntfy-settings.spec.ts`, below; of this spec's, the navigation case's
Notifications half (the "ntfy alerts" group under its 16-pixel sub-heading, below the 18-pixel section heading) is the
golden `settings-notifications`, which also shows its Sounds half (the sub-heading and its three switches, which the
other-dialogs area added with the status sounds; their tests are listed under `tests/sounds.spec.ts` in
`docs/gpui/parity/other-dialogs.md`). Appearance's own cases are in `tests/appearance.spec.ts`, below. Agent trust's are
split between `tests/workspace-trust.spec.ts` (below) and this spec, whose hint-switch cases are mapped in the Agent
trust bullet.

NOTE: Terminal shows the link choice and the tunnel consent switch. Its **File types to open with Cmd-click** field
governs opening downloaded files, so it waits for the terminal area's downloads, which are not on `main` yet; until they
land it is not drawn. The switch is the old Mac app's, and like it is drawn only on the Mac, where the terminal offers
tunnels: turning it on asks the terminal's tunnel warning ("Allow tunnels from remote sessions?") and consent is saved
only on Allow; turning it off saves at once, and the helm then closes every open tunnel. The golden `settings-terminal`
is decided for the Mac on every machine, so it shows the switch.

The app opens Settings from the gear on the host rail's foot, which calls `settings::open` with itself as the opener and
opens it at Sessions; the gear's own tests, and the end-to-end smoke step that clicks it, are in the sidebar's parity
file (`docs/gpui/parity/sidebar-and-session-view.md`, the `tests/filters.spec.ts` row). The one entry point still to
come is the trust hint band, the session view's, which calls the Agent trust entry point
(`settings::open_at_agent_trust`); until it is built, only tests open Settings at Agent trust that way.

Replacing tests, in `crates/gui/farhelm-gui/src/settings/tests.rs` unless named otherwise:

- Opening and focus ("settings keeps both close controls pinned around its grouped scrolling body", the opening half of
  "settings navigation keeps mounted panes and follows the narrow list path"):
  `the_gear_opens_settings_at_sessions_with_focus_on_its_first_switch`,
  `escape_and_both_close_controls_return_focus_to_the_gear`,
  `the_frame_keeps_one_size_across_sections_capped_by_the_window`, and for the scrolling body under the pinned footer,
  `the_tallest_sections_last_line_clears_the_footer_scrolled_to_the_end`.
- Navigation ("settings navigation keeps mounted panes ...", its wide half):
  `the_dialog_lists_only_the_sections_this_build_has`, `switching_sections_keeps_a_pending_save_and_a_failure_line`. The
  section buttons are `Button`s in the navigation tier, whose activation and pressed state `Button`'s own tests cover.
- Saves and refusals ("settings reports and reverts a refused ${choice.field} save" for automatic updates, frame colors
  and naming; "quick-launch naming defaults on and its settings switch persists independently"; "host browser
  permissions persist independently through the settings switches"; "automatic host updates default off and persist ..."
  for the default and the write): `each_switch_saves_its_own_preference_field_under_the_preferences_key`,
  `a_refused_save_restores_the_stored_value_and_shows_the_error_beside_the_switch`,
  `name_sessions_on_quick_launch_reads_as_on_when_the_helm_stored_no_choice`,
  `a_save_in_flight_goes_on_after_the_dialog_closes`.
- Following the helm (the live half of "... persist across clients", now that changes arrive live, and the host
  switches' help explaining the current behavior): `an_open_dialog_follows_a_change_made_elsewhere_with_its_help`.
- Terminal ("terminal link segments keep native keys and descriptions", and "settings reports and reverts a refused
  ${choice.field} save" for `terminal_link_click`): `the_terminal_section_offers_the_link_choices_with_their_risks`
  (which also checks the group's name and role), `a_link_choice_saves_only_its_preference_field_and_follows_the_helm`,
  `a_refused_link_choice_shows_the_stored_choice_and_a_failure_line`,
  `an_unset_or_unknown_link_choice_reads_as_intercept`. The arrows, Space and the description lines as accessible
  descriptions are the radio group's own, which its tests in `components/tests/radio_group.rs` cover.
- Terminal's tunnel switch, which no browser spec covered (it was the Mac app's only):
  `the_tunnel_switch_is_drawn_only_on_the_mac` (none on Linux),
  `the_terminal_section_offers_the_link_choices_with_their_risks` (the switch, its name, role, place and help),
  `the_tunnel_switch_turns_on_only_through_the_warning` (Cancel saves nothing and returns focus to what held it, the
  switch under the Linux click rule; Allow saves only `tunnel_consent: true`, with no optimistic copy),
  `a_refused_allow_leaves_the_switch_off_with_the_helms_words`,
  `turning_tunnels_off_saves_at_once_and_a_refusal_restores_on`,
  `a_tunnel_press_during_another_save_asks_nothing_and_says_why`,
  `turning_tunnels_off_during_another_save_sends_nothing_and_says_why`, and
  `an_allow_whose_save_meets_another_says_so_under_the_switch`. The warning itself, its words and its save are the
  terminal area's (`docs/gpui/parity/terminal.md`). Pending: focus after a mouse press and Cancel on a Mac, where the
  click leaves the switch unfocused and focus goes back to what held it before; the tests decide focus by Linux's rule.
- Agent trust's hint switches ("settings reports and reverts a refused ${choice.field} save" for
  `claude_trust_hint_dismissed` and `codex_trust_hint_dismissed`): they save and refuse as every preference switch does,
  in `a_refused_save_restores_the_stored_value_and_shows_the_error_beside_the_switch`, and
  `the_hint_switches_are_the_shared_dismissals_inverted` pins their field and inversion. Superseded, since a hint switch
  reads the pushed preference row and not the trust read (Deviations, "The hint switches can be used before the trust
  read answers"): "settings keeps the pending ${hint.name} hint choice clickable through a held read", "settings repairs
  a ${hint.name} trust read crossing a successful hint save", and "settings reverts a pending ${hint.name} hint save
  after reopening before refusal", with and without an unreadable corrective read.
- Checkouts' default folder ("settings saves and clears the all-hosts checkout folder", "an untouched checkout folder
  save writes nothing and edited paths are trimmed", "a pending checkout folder save survives closing settings"):
  `checkouts_reads_its_routes_once_and_stays_locked_until_they_answer` (the untouched Save),
  `the_folder_saves_an_edited_draft_trimmed_and_reads_it_back` (trimmed, read back, "Checkout folder saved", an empty
  field clearing it), `a_refused_folder_keeps_its_draft_and_reason_across_sections` (the refusal through a section
  switch), `a_folder_save_in_flight_goes_on_after_the_dialog_closes`, and the failed read with its Retry, which both old
  fields had: `a_failed_folder_read_shows_its_reason_and_retry_reads_again`. The field is the shared
  `settings::CheckoutField`; its host scope is pinned in the `tests/host-checkout-folder.spec.ts` section below.
- Checkouts' post-clone command ("settings saves and clears the all-hosts post-clone command", "post-clone command stays
  locked until its initial read completes"):
  `the_post_clone_command_keeps_its_spacing_and_only_all_whitespace_clears_it` (verbatim with its spacing and line
  break, Enter typing the break, read back, all-whitespace clearing it) and
  `checkouts_reads_its_routes_once_and_stays_locked_until_they_answer` (read-only and no Save until the read answers, a
  stored explicit disable shown empty, an untouched Save writing nothing). Superseded: "post-clone commands with
  carriage returns refuse edits but can be cleared", since the field keeps a carriage return (Deviations);
  `a_stored_carriage_return_survives_an_edit` pins that premise. "a pending post-clone save orders reopened settings
  before the next save" is the UI channel's ordering: a reopened Settings' read is admitted only after the pending
  save's reply, so the field it fills is read-only until then and shows the saved command; no Settings code orders it.
- Checkouts' drafts across sections ("checkout drafts survive switching sections and are discarded on close"):
  `an_unsaved_draft_survives_a_section_switch_and_goes_with_the_dialog` (an unsaved command kept through a section
  switch with nothing sent, and the stored one shown after closing and opening again), and
  `a_refused_folder_keeps_its_draft_and_reason_across_sections` for a refused folder.
- Checkouts' spare switch ("prepared checkout settings preserve saved choices across reads, refusals and close"):
  `the_spare_switch_saves_through_its_route_and_a_refusal_restores_it` (a failed read leaving it unavailable with the
  reason, the documentation link, a save through its route, a refusal restoring the stored choice). The switch shares
  Agent trust's state, read and failure line (`settings/configured.rs`), whose tests cover a reply reaching only its own
  opening.
- App, which no browser spec covered (the old desktop app's only): `app_is_listed_last_while_the_desktop_updater_runs`
  (listed after Notifications only with an updater; `the_dialog_lists_only_the_sections_this_build_has` has none) and
  `the_app_switch_writes_the_apps_own_setting_and_shows_only_what_was_saved` (the note and the switch's two helps, a
  write through the updater with nothing sent to the helm, a refused write showing the setting in force with the
  updater's reason, kept across a section switch and gone after the next saved press), and
  `the_app_switch_writes_while_a_preference_save_is_unanswered` (a helm save in flight never holds App's write back).
  The header's words: `the_header_says_this_device_only_where_app_is_listed`.
- The look: the goldens `settings-sessions`, `settings-appearance`, `settings-hosts`, `settings-terminal`,
  `settings-checkouts`, `settings-agent-trust`, `settings-notifications` and `settings-app`
  (`crates/gui/farhelm-gui/src/goldens/settings.rs`). The reference set has no `settings-app` picture: its capture drove
  the web interface, which has no App section.

Not ported here, and why: the persistence across a second client and reload belongs to the helm's preference route and
the UI channel's pushes, which their own tests cover; "settings gap keeps focus ..." is the layer manager's empty-space
rule (`keeps_focus_on_empty_click` is on for Settings), with no Settings-specific test yet; the text selection half of
it does not apply, since GPUI labels are not selectable; "settings reverses a permanent removal answer without
reloading" is Remove host's, below, for its removal half (the switch's own save is this section's preference switch
tests); the narrow list path is not built (Deviations); "settings ignores a superseded link-choice failure" has no newer
choice to supersede, since a second click while any Settings save is in flight is refused (Deviations).

## `tests/ntfy-settings.spec.ts`, replaced

Settings → Notifications' ntfy alerts form is ported, Send test included. Replacing tests, in
`crates/gui/farhelm-gui/src/settings/tests.rs` unless named otherwise:

- "ntfy settings explicitly save, refuse, preserve and clear a token": the read and the lock until it answers,
  `notifications_reads_its_settings_once_and_stays_locked_until_they_answer`; the save of the whole form, its outcome
  line and the emptied token field, `a_save_sends_the_whole_form_and_holds_no_token_after`; a refused topic kept for
  correction with the helm's words, `a_refused_save_keeps_the_form_with_the_helms_words_until_closed`; the hint that
  another server clears a saved token, and Clear saved token,
  `another_server_says_a_saved_token_will_be_cleared_before_saving`, which also reads the token field's three
  placeholders; nothing edited while a save is in flight, `nothing_edits_the_form_while_its_save_is_in_flight`; the save
  body's token rule and the same-server comparison,
  `a_blank_token_keeps_the_saved_one_and_a_typed_or_cleared_one_does_not` and
  `the_server_changes_only_with_its_scheme_host_or_port` (`settings/ntfy.rs`); the masked token field,
  `a_masked_input_holds_what_was_typed_and_paints_none_of_it` (`components/tests.rs`). The look (the monospace server
  field, the priority segments, Save's accent fill, "No token saved"): the golden `settings-notifications`.
- "ntfy send test reports outcome while leaving unsaved fields alone":
  `send_test_sends_nothing_of_the_form_and_reports_how_it_went` (the test sends no body, its refusal and success show in
  place and survive a section switch with the unsaved fields, closing discards both, and nothing else is sent while it
  is out); that a refused save's outcome and draft survive too is in
  `a_refused_save_keeps_the_form_with_the_helms_words_until_closed`.
- "ntfy settings load failure is visible and retry restores editing":
  `a_failed_read_shows_its_reason_and_retry_reads_again`.

Not ported here, and why: the 390-pixel width and the stacked actions, since the window's minimum width is above it;
persistence across a reload and the token never being returned, which are the helm route's own and covered by its tests;
the tint of an unavailable Save after a failed read, which no golden shows.

## `tests/credentials-settings.spec.ts` and `tests/credential-delivery.spec.ts`, partly

Settings → Credentials is ported: the three lists, the in-place editor with its write-only secret, the agent-login kinds
with Codex's `auth.json` pasted or chosen through the platform's file dialog, the "Not encrypted on disk" warnings,
delivery failures after a done replacement, and the Tensorlake API key form with Save, Test and Clear. Replacing tests,
in `crates/gui/farhelm-gui/src/settings/credential_tests.rs` unless named otherwise:

- "credentials keep fixed names, replace write-only values and discard login drafts": the creation and the list read
  again, `a_new_credential_saves_its_name_kind_and_secret_and_the_list_reads_again`; the fixed name, the empty secret of
  a saved credential and the replacement of its value only,
  `a_replacement_sends_only_the_secret_and_shows_the_hosts_it_did_not_reach`; one warning per view, naming the
  Tensorlake key only on the overview, `the_overview_and_the_editor_each_show_one_storage_warning`; the Codex caveat and
  the file that fills the same draft as paste and saves nothing,
  `a_chosen_file_fills_the_codex_login_and_saves_nothing`; a closed editor's draft gone and nothing created,
  `back_cancel_and_closing_discard_the_draft_without_asking`; Delete,
  `delete_is_unavailable_while_a_recipe_uses_the_credential`; the catalog's read, its lists and their times,
  `credentials_reads_its_catalog_whenever_shown_and_lists_each_entry_in_its_kinds_list`; the editor's own checks and a
  refusal kept for correction, `the_editor_checks_the_name_and_the_secret_before_sending` and
  `a_refused_save_keeps_the_draft_with_the_helms_words`; a failed read with Retry,
  `a_failed_catalog_read_shows_its_reason_and_retry_reads_again`; a Delete answered after Back leaving a newer editor
  and its text, `a_delete_answered_after_back_leaves_a_newer_editor_open`; switching agents emptying the secret,
  `switching_agents_empties_the_secret`; a cancelled file dialog freeing Choose file…,
  `a_cancelled_file_dialog_frees_choose_file`. The look (the warning's danger edge and fill, the key form, the three
  empty lists): the golden `settings-credentials`. The draft kept across a section switch is the dialog's rule, tested
  for every section in `crates/gui/farhelm-gui/src/settings/tests.rs`. Not ported: the 390-pixel width (the window's
  minimum is above it).
- "the two Tensorlake key forms share acknowledged metadata and separate drafts": its Credentials half, Save, Test and
  Clear with the write-only placeholders and the request bodies,
  `the_tensorlake_key_saves_tests_and_clears_with_write_only_metadata`, the controls locked until the account read
  answers, `the_tensorlake_key_waits_for_the_account_read`, and `each_control_sends_only_its_own_field` and
  `the_test_message_counts_sandboxes` (`settings/tensorlake_key.rs`). Its Tensorlake half, the second form over the same
  account with its own label, warning and typed key, is
  `a_save_in_either_key_form_shows_in_both_and_each_keeps_its_own_typed_key`
  (`crates/gui/farhelm-gui/src/settings/tensorlake_tests.rs`).
- "recipe references refresh deletion guards and preserve unavailable picker choices": partly. An open editor's refresh
  by a fresh read is `an_open_editor_follows_a_fresh_read_and_keeps_its_secret`; the recipe editor's pickers offering
  only their kind, `each_picker_offers_its_kind_and_save_sends_the_names_chosen`, and keeping a missing choice visible
  as unavailable until None is chosen, `a_missing_choice_stays_visible_until_another_is_chosen`. Not ported: the recipe
  editor's pickers following a credential added or deleted while it stays open (see "Recipes" under Deviations).
- "saved credentials report delivery failures without retaining the secret draft":
  `a_replacement_sends_only_the_secret_and_shows_the_hosts_it_did_not_reach`.
- "connected credential warnings remain readable in ${palette}": a Tensorlake host's row and rail mark; waits for the
  Tensorlake hosts in the Hosts panel and the rail's host status.

## `tests/tensorlake-settings.spec.ts` and `tests/tensorlake-tunnels.spec.ts`, partly

Settings → Tensorlake draws the EXPERIMENTAL warning, the intro, its own copy of the Tensorlake API key form (labelled
"API key", with its own "Not encrypted on disk" warning) and "Creating sandboxes" with the create-without-asking switch
and the Usage Credits note, and between them the SSH key group: Create SSH key until the key exists, then the
registration command with Copy and "Command copied", or the helm's reason when its key cannot be read, and **Sandboxes
not on this helm** with Refresh, Checking…, "None found · checked", its rows with Delete, Retry cleanup and Cleaning
up…, and the read's and Delete's failures. Replacing tests, in `crates/gui/farhelm-gui/src/settings/tensorlake_tests.rs`
or, for the list, `crates/gui/farhelm-gui/src/settings/tensorlake_sandbox_tests.rs`:

- "Tensorlake settings save a write-only key, test the saved key and clear it": partly. Save, Test and Clear with the
  write-only placeholders, the request bodies and Test using the saved key are the shared form's, tested in Credentials
  (`the_tensorlake_key_saves_tests_and_clears_with_write_only_metadata` in
  `crates/gui/farhelm-gui/src/settings/credential_tests.rs`); the Tensorlake copy's label, warning and help, and a save
  in one form showing in the other, are `a_save_in_either_key_form_shows_in_both_and_each_keeps_its_own_typed_key`. The
  SSH key group: the key created only on request, then the registration command, copied byte for byte with "Command
  copied" for 1.5 seconds, `create_ssh_key_shows_the_registration_command_and_copy_copies_it`; a key the helm cannot
  read, `an_unavailable_key_shows_the_helms_reason_and_offers_nothing`. Its look, which the reference screens do not
  reach, is in the screenshot batch's app scenes `app-settings-tensorlake-ssh-key`, `-command` and `-copied`. That the
  command names the helm's tag and carries the public key's text is the helm's, covered by its tests. The discovery
  cases: a saved key reading the list, `a_saved_key_reads_the_list_which_says_when_it_found_none`; a rejected key's
  failure is a failed read, `a_failed_read_keeps_the_rows_and_a_refresh_recovers`; Clear hiding the list,
  `a_clear_hides_the_list_and_its_rows`. Not ported: a successful Test reading a failed list again (Deviations,
  "Tensorlake"); the 390-pixel width (the window's minimum is above it); the key never appearing in a reply is the helm
  route's own, covered by its tests.
- "sandbox refresh shows pending, empty, failure and recovery feedback":
  `a_saved_key_reads_the_list_which_says_when_it_found_none` (Checking… with Refresh unavailable, then None found) and
  `a_failed_read_keeps_the_rows_and_a_refresh_recovers`. Its pictures in both palettes have no app scene: the list shows
  only with a saved key, which would make the batch's app call the real Tensorlake API, and there is no stand-in for it
  (the orchestrator's question of 2026-11-15; until it is answered, drives stop at the section with no key).
- "sandbox discovery ignores replies superseded by key replacement or clear": the UI channel answers in order, so a read
  sent before a key change is answered before it, and only the newest read's answer is shown:
  `a_clear_hides_the_list_and_its_rows`, `a_saved_key_reads_the_list_which_says_when_it_found_none` (a replaced key
  reads again) and `only_a_tensorlake_host_coming_or_going_reads_the_list_again` (an older read's answer dropped).
- `tensorlake-tunnels.spec.ts`, "Settings lists unknown Farhelm sandboxes and confirms snapshot cleanup": the rows'
  state, "from another helm", creation time, a failed cleanup's line with Retry cleanup and a running cleanup's Cleaning
  up…, `rows_show_their_state_and_cleanup`; Delete's question, Cancel, Delete sandbox's request, the helm's answer at
  its bound and the reads after every answer, `delete_asks_sends_the_sandbox_and_reads_the_list_again`; the list read
  again when a Tensorlake host comes or goes and on nothing else in the fleet,
  `only_a_tensorlake_host_coming_or_going_reads_the_list_again`. Which sandboxes the helm lists and what a cleanup
  terminates and deletes are the helm's, covered by its tests. The spec's other test, the tunnel health of a host row,
  comes with the Tensorlake hosts in the Hosts panel.
- "Tensorlake's create-without-asking switch shows only the stored choice":
  `the_create_without_asking_switch_shows_only_the_stored_choice`, which also checks that the save names only its own
  field, and, for a done save whose answer does not decode,
  `a_done_save_whose_answer_does_not_decode_reads_the_account_again`.
- "Tensorlake settings open with the experimental warning" and "Tensorlake settings load failure locks the section until
  a retry succeeds": `the_warning_heads_the_section_and_a_failed_read_locks_it_until_a_retry`. The look (the warning's
  fill and edge, the intro, the key form with no key saved): the golden `settings-tensorlake`. Its section list puts
  Tensorlake before Containers, as SPEC.md orders them; the reference screens, taken from the old app, list Containers
  first.

## `tests/provisioning.spec.ts`, Add host's dialog, probe and setup

The Add host dialog, its probe, the setup confirmation and the setup's submission are ported; setup progress, failure
and rerun are in the next section.

The app opens the dialog from the host rail's Add host, which calls `hosts::add::open` with itself as the opener; that
button's own tests, and the end-to-end smoke step that opens the dialog and closes it with Escape, are in the sidebar's
parity file (`docs/gpui/parity/sidebar-and-session-view.md`, the `tests/filters.spec.ts` row). The Hosts heading's add
button (`+`), the dialog's other entry point, opens it the same way, with itself as the opener:
`crates/gui/farhelm-gui/src/hosts/tests.rs` `the_headings_add_button_opens_add_host_and_gets_focus_back`.

Replacing tests, in `crates/gui/farhelm-gui/src/hosts/add/tests.rs`:

- "the add dialog adds a host under its alias and refuses the same destination again": its probe half,
  `add_sends_the_probe_with_its_new_entry_under_the_dialogs_key`; its refusal,
  `a_destination_already_added_is_refused_naming_it_without_a_probe`.
- "the add dialog refuses a taken or malformed alias before contacting the host":
  `a_blank_alias_is_sent_as_none_and_a_malformed_one_is_refused_before_contact` (the malformed half); the taken alias is
  the helm's refusal, shown as any 409 in
  `manual_setup_refusals_and_the_timeout_show_in_the_dialog_with_the_destination_kept`.
- "the add dialog's probe sends no install paths, and a doubled submit produces one real probe":
  `add_sends_the_probe_with_its_new_entry_under_the_dialogs_key`.
- "discovery registers an answering supervisor through the real handler": the dialog's half,
  `a_discovered_host_closes_the_dialog`; the registration is the helm's, covered by its own tests.
- "manual discovery preserves the concrete peer-safe reason and never provisions", "probe failure is concrete,
  peer-safe, and cannot become a setup offer" and "an unreadable successful probe refreshes a registration without
  inventing an offer": `manual_setup_refusals_and_the_timeout_show_in_the_dialog_with_the_destination_kept` (no refresh:
  the hosts list is pushed).
- "the add dialog's cancel stays live while discovery is pending":
  `escape_and_cancel_close_the_dialog_while_a_probe_is_out`, and for a reply after the user moved on,
  `an_answer_after_the_user_moved_on_opens_nothing_and_moves_no_focus`.
- The look: the golden `host-add-dialog` (`crates/gui/farhelm-gui/src/goldens/hosts.rs`).

Replacing tests for the setup, in `crates/gui/farhelm-gui/src/hosts/setup/tests.rs`:

- "host-setup-layout ... keeps answers and exact plan text" and "real discovery offers one peer-safe concrete plan and
  mutates nothing before confirmation": `a_plan_opens_the_setup_confirmation_in_place_of_add_host` (the plan's text, the
  answers' order, focus on Set up, nothing sent), with the golden `host-add-setup-plan` for the layout. The narrow
  window's stacked answers are not built: the window's minimum width is above the old 560-pixel breakpoint. Focus starts
  on Set up because SPEC.md, "Opening and closing", lists host setup among the confirmations that start on their
  confirming button ("This covers session Stop, Delete, Replace, Restart and tab close, host setup, removal and
  uninstall, and permanent trash deletion"); its rule that questions arriving later start on their harmless answer does
  not apply, since this one opens only while the user has not moved on since Add.
- Without asking, a refusal reopens Add host when the user stayed:
  `without_asking_a_refusal_reopens_add_host_when_the_user_stayed`.
- Copy plan: `copy_plan_copies_the_helms_plan_text`.
- "a permanent setup answer is reversible through settings after the next add": `the_permanent_answer_also_stops_asking`
  and `with_the_preference_on_a_plan_is_submitted_without_asking`; turning the switch back off is Settings' own test.
- "cancel and a setup answer in one burst add nothing and change no preference": `cancel_sends_nothing`; that only the
  first answer acts is the prompt's rule, covered by its own tests.
- The submission: `set_up_submits_the_plan_under_the_probes_key_and_closes`.
- "a refused ADD attempt reopens the form and refreshes committed state" and "the add dialog's alias survives the setup
  question": `a_refused_setup_reopens_add_host_with_the_destination_alias_and_reason` (no refresh: the hosts list is
  pushed); after the user moved on, `a_setup_refused_after_the_user_moved_on_shows_in_the_hosts_panel` and, without
  asking, `without_asking_a_refusal_after_the_user_moved_on_opens_nothing`; a refused permanent answer,
  `a_refused_permanent_answer_shows_in_the_hosts_panel`, `a_refused_permanent_answer_is_gone_after_a_reset` and
  `a_refused_permanent_answer_is_gone_once_the_preference_is_on`.
- A plan after the user moved on: `a_plan_after_the_user_moved_on_opens_no_confirmation`.

Not ported, and why: "a malformed accepted ADD closes the form and warns without retrying", since nothing reads the
accepted reply's body any more (the run shows in the pushed host progress); "accepted ADD ... follows the run" is in the
next section.

A reply that arrives after the user moved on (a key or click since Add) still shows in the dialog if it is open, as in
the old app: it opens nothing and moves no focus, which is all SPEC.md's "Keyboard focus" withholds from a late result.
A dialog the user closed shows nothing, and a refusal has no row to show on, since no host exists for it.

## `tests/ssh-help.spec.ts`

SSH help is `hosts::ssh_help`. Its entry points are `hosts::ssh_help::open`, for the ? menu's "SSH needs 2FA?", which
the area that owns the ? menu draws later, passing the ? button as the control closing returns focus to (the old app
returned it to its Help toggle; the menu item goes with its popover), and which the tests stand in for with a button of
their own; and Add host's "Start its connections yourself first." link, which opens it for the destination as typed.

Replacing tests, in `crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs` unless named otherwise:

- "ssh help displays and copies both registered-host commands":
  `the_menu_entry_shows_each_hosts_two_commands_and_copies_their_exact_bytes` (the read and its body, focus on Close,
  the host's name, destination and commands as host text, each Copy's name and the raw bytes it copies, Escape back to
  the opener under macOS's click rule, nothing else sent). The helm's half, that a registered host gets both commands
  and an unregistered destination is never added, is the helm's own test
  (`ssh_help_lists_and_validates_without_mutation` in `crates/farhelm-helm/src/ssh_help.rs`).
- "ssh help names the empty state and remains dismissible during a failed read":
  `with_no_ssh_hosts_it_points_at_add_host_and_a_refusal_shows_in_its_place`.
- "add host opens ssh help for its draft without losing the form":
  `the_ssh_help_link_shows_the_drafts_commands_and_closing_returns_to_the_draft` in
  `crates/gui/farhelm-gui/src/hosts/add/tests.rs` (the empty draft's line, Escape and Close back to the link with the
  draft intact, the typed destination's section, no probe sent). Its checks of the link's color, underline and focus
  ring are the `host-add-dialog` golden's and the shared button's; that the page behind takes no focus and Tab stays in
  Add host afterwards are the layer manager's invariants, checked after every test step.
- "ssh help returns focus when a setup offer replaced its opener":
  `a_probe_answered_while_ssh_help_is_open_opens_nothing_over_it` in `crates/gui/farhelm-gui/src/hosts/add/tests.rs`. A
  setup offer never replaces Add host once the user has moved on since Add, and the click on the link is moving on: with
  SSH help open over a probe, the plan opens no confirmation, SSH help keeps focus, Add host's line says the user moved
  on, and closing SSH help returns focus to the link, which is still there. A probe that registers the host closes Add
  host, and SSH help over it closes with it, as the old help went with its form; the test checks that too, with focus
  back on the rail's Add host.
- No old spec covered the reasons the helm cannot compose a connection's command:
  `an_unavailable_connection_is_named_once_at_the_top_and_offers_no_copy`.

The reply's types are the helm's own, in `farhelm-proto` (`farhelm_proto::ssh_help`), with a wire-shape test there
(`the_reply_keeps_its_wire_shape`).

## `tests/provisioning.spec.ts`, setup progress, failure and rerun

A host row shows its setup run from the pushed host progress (`crates/gui/farhelm-gui/src/hosts/progress.rs`), and its
menu offers Re-run once a setup failed (`hosts::setup::rerun`). Replacing tests, in
`crates/gui/farhelm-gui/src/hosts/tests.rs`, `hosts/progress/tests.rs` and `hosts/setup/tests.rs`:

- "confirmed setup expands its row once and can be folded": `a_running_setup_opens_its_row_once_and_collapse_folds_it`
  (the row opening with Details off, the steps, the inline status, Collapse, the fold surviving the run's progress, and
  success leaving the row folded with its phase words back), `a_running_setup_opens_its_row_until_folded` and
  `the_panel_keeps_a_runs_first_sight_and_fold_until_the_run_changes` (a new run starts open), and end to end, step 43
  of `scripts/gui-e2e-smoke.py`. Its layout checks (the verb and clock inside the sidebar, before the toggle) are the
  goldens below; the status sits on its own line under the name, as the old one did.
- "accepted ADD registers before execution and releases the page lock while running", its row half: the running row's
  status and steps above; Settings unavailable and no Update or Re-run while it runs are `offers`'
  `a_running_run_holds_the_host_until_it_ends`. Its rail half (the avatar's progress mark) is the sidebar's.
- The failed run's step and reason (the drive of part 2 found the helm's "disk full" shown nowhere), as the old specs'
  `.provisioning-run-message` checks read them: `a_failed_setup_opens_its_row_with_the_step_and_the_reason`, also when
  the user folded the run first; end to end, step 43.
- The inline status's count, current step and clock:
  `the_inline_status_counts_steps_names_the_current_one_and_times_the_run`,
  `the_clock_reads_minutes_and_seconds_and_grows_hours` and
  `the_elapsed_time_counts_from_when_the_panel_first_saw_the_run` (the clock moving with nothing pushed, in an inactive
  window under reduced motion, redrawn only by the ticker's one-second clock, which the panel lets go of once the run
  ends); a run pushed while Hosts is closed keeps its clock, and a reset starts the clock and the fold over:
  `a_run_keeps_its_clock_while_hosts_is_closed_and_a_reset_starts_it_over`; the pulse stops when Sessions replaces
  Hosts, through Hosts, a host choice or a reset: `hiding_hosts_stops_the_setup_pulse` and
  `a_host_choice_and_a_reset_stop_the_setup_pulse_too`; the one-second clock stops too, through the same hook:
  `an_updates_clock_counts_until_the_panel_is_hidden`.
- "failed ADD rerun probes the registered destination and discovery resolves the run":
  `re_run_probes_the_registered_destination_and_sets_up_under_the_hosts_key` (the probe for the registered destination
  with its recorded install paths, the question, the submission under the host's key, a refused submission under the
  row); discovery resolving the run is the helm's, and the row follows the pushed state.
- "cancel and a permanent answer on a host row's setup question change no preference": the rerun asks the same question
  as Add host's setup (its title and focus in the rerun test above); the first answer winning a burst is the prompt's
  own rule and test, and Cancel's sending nothing is `cancel_sends_nothing`.
- A rerun's probe answered "manual" or refused: `a_rerun_that_leads_nowhere_says_why_under_the_row`; its plan after the
  user moved on: `a_reruns_plan_after_the_user_moved_on_opens_nothing_and_says_so`; with "Set up new hosts without
  asking" on: `with_the_preference_on_a_rerun_sets_up_without_asking`.
- The look: the goldens `host-setup-running-hosts-panel` and `host-setup-failed-hosts-panel`
  (`crates/gui/farhelm-gui/src/goldens/hosts.rs`), against the references of the same names.

Not ported, and why:

- "an unknown future progress-step status renders verbatim": the GUI decodes the helm's progress into a closed type
  (`farhelm_proto::provisioning::StepStatus`), so a status this build does not know breaks the channel, as an unknown
  host phase does, instead of reaching a row.
- "expanded setup failure closes the menu below it" and "setup disclosure transitions invalidate fixed-surface
  geometry": the layer manager anchors a menu to its element, which a growing row above moves along with it, so there is
  no measured position to retire.
- "a failed local ADD keeps its rerun action in the local setup state" and the local row's automatic setup: the helm
  never sets up its own machine (its local probe answers that the Farhelm app starts that machine's supervisor when it
  opens, SPEC.md "Topology": "What the panel does NOT do on the helm's own machine is install one"), so the local host
  has no setup run to show or rerun.
- The reference scenes `host-setup-running` and `host-setup-failed` show the attention list and the rail's avatar, which
  are the sidebar's (host status on the rail is its M3), not this area's.

## `tests/provisioning.spec.ts`, Remove host

A host menu's Remove asks the removal confirmation and sends `DELETE /api/hosts/{id}`, or with "Remove hosts without
asking" on, sends it at once (`crates/gui/farhelm-gui/src/hosts/remove.rs`). Only an SSH host's menu offers it: a
Tensorlake or container host's removal deletes its machine, which is Delete sandbox, a later piece.

Replacing tests, in `crates/gui/farhelm-gui/src/hosts/remove/tests.rs` unless named otherwise:

- "rail removal returns to its avatar on cancel and Hosts on success", its row half:
  `cancel_sends_nothing_and_returns_focus_to_the_rows_toggle` (under macOS's click rule) and
  `remove_sends_the_delete_under_the_hosts_key` (the question stays up until the helm forgets the host; focus after the
  removal recovers, see Deviations). The avatar half follows from that and
  `crates/gui/farhelm-gui/src/session_list/host_rail/avatar_menu_tests.rs`
  `a_secondary_click_on_an_avatar_opens_its_hosts_menu_beside_it`, which pins the avatar as the menu's opener.
- "host retirement releases before removal and preserves approval focus: Remove success", the removal's half:
  `remove_asks_naming_the_host_its_destination_and_what_removal_keeps` (focus on Remove) and
  `remove_sends_the_delete_under_the_hosts_key` (Remove sends the removal); the question closing when its host goes,
  `the_question_closes_when_its_host_is_gone`. Its approval-card half is not this area's: the question keeps focus on
  its busy answer while the removal runs, and the result moves no focus outside it.
- `settings.spec.ts`, "settings reverses a permanent removal answer without reloading", its removal half:
  `the_permanent_answer_also_stops_asking` and `with_the_preference_on_remove_goes_without_asking`.
- A refused removal, which the old dialog showed inside itself:
  `a_refused_removal_keeps_the_question_up_with_the_reason` ("Farhelm refused this action:" and the helm's words in
  amber under the body, focus moved to Cancel as the old `focus_remove_refusal` did, nothing under the row, and a second
  Remove that succeeds); Cancel after it, `cancel_after_a_refusal_returns_focus_to_the_rows_toggle`;
  `a_permanent_answer_whose_removal_the_helm_refuses_keeps_the_question_up` (the preference stays on, and pressing the
  answer again saves nothing more); with the preference on,
  `without_asking_a_refused_removal_shows_under_the_hosts_row`. The helm's success closing the question without a host
  list, `the_helms_success_closes_the_question_without_a_host_list`. A refused permanent answer,
  `a_refused_permanent_answer_shows_on_the_panels_line` and, refused by the operation store while another preference
  save was in flight, `a_permanent_answer_refused_by_another_preference_save_says_so`.
- A Remove refused before anything is sent: `a_session_launch_in_flight_refuses_remove_with_a_reason` (a launch, a
  Restart and a Replace in flight, from the menu), and inside the question that stays up,
  `a_launch_that_started_while_the_question_was_open_refuses_its_answer` and `a_busy_host_refuses_remove_with_a_reason`;
  a question asked while the host is busy, `asked_while_the_host_is_busy_only_cancel_acts`.
- The menu, in `crates/gui/farhelm-gui/src/hosts/menu/tests.rs`: `a_remote_hosts_toggle_opens_a_menu_starting_on_retry`,
  `settings_waits_for_the_hosts_own_work_in_flight` (Remove does nothing while the host is busy),
  `the_local_hosts_greyed_update_says_why_and_sends_nothing` (no Remove on the helm's own machine) and
  `a_sandbox_hosts_menu_offers_no_remove`; which kinds remove by forgetting, `offers/tests.rs`
  `each_kind_decides_its_own_offers`.
- End to end, over the scripted backend's fake hosts: `scripts/gui-e2e-smoke.py`'s step 44 (Cancel, Remove, the
  permanent answer, and a removal without asking).
- The look: the goldens `host-remove-confirmation` and `host-menu` (`crates/gui/farhelm-gui/src/goldens/hosts.rs`).

Not ported here: "a retarget and a removal before the claim each submit nothing" and "removal before plan completion
submits nothing", which test an Update's plan against a removal and come with Update; the refusal dialog's "Remove
instead" answer ("uninstall and Update planning refusals open framed dialogs"), which comes with that dialog.

## `tests/provisioning.spec.ts`, Update

A remote host's Update goes out from its menu, from its row's inline `↑ Update`, from the menu's Re-run of a failed one,
and for every host whose Update is available from the Hosts heading's Update all, all as the same plan request and its
confirmation (`crates/gui/farhelm-gui/src/hosts/update.rs`); a refused plan opens the host refusal dialog
(`hosts/refusal.rs`). The attention list's Update rows will call the same `hosts::update::start` and `start_all`.
Replacing tests, in `crates/gui/farhelm-gui/src/hosts/update/tests.rs` unless named otherwise:

- "single Update shows inline progress without opening its row" and "an update shows pending status from acceptance":
  `the_menus_update_plans_then_confirms_under_the_hosts_key` (the plan request without a body, the confirmation naming
  its plan, "Updating…" while either is out, the count and current step once the run is pushed, the row folded, focus
  back on the toggle), and end to end, step 47 of `scripts/gui-e2e-smoke.py`. The inline status's words and counting:
  `hosts/progress/tests.rs` `the_inline_status_counts_steps_names_the_current_one_and_times_the_run` and
  `a_running_setup_opens_its_row_until_folded` (a running Update never opens its row). Its clock is the setup's, the
  ticker's one-second clock that the panel holds for every row while a run shows
  (`the_elapsed_time_counts_from_when_the_panel_first_saw_the_run`, and for an Update's run
  `an_updates_clock_counts_until_the_panel_is_hidden`).
- "global details alone control disclosure while update progress stays inline": the same test (Details shows the run's
  steps, and nothing else opens the row while it runs); failure opening the row: the same test and step 47.
- "UPDATE planning refusal shows the concrete reason in a dialog" and the Update half of "uninstall and Update planning
  refusals open framed dialogs": `a_refused_plan_opens_the_refusal_dialog_naming_the_host` (the title, the red summary,
  the destination, the helm's words, Close focused and Close as its Enter action, no confirmation sent); after the user
  moved on: `a_refused_plan_after_the_user_moved_on_opens_nothing` (the dialog opens through the press's mark, and a
  refusal it could not show stays under the row). A refusal the dialog showed is not kept under the row, as in the old
  spec's ".provisioning-error count 0". The newer build, newer schema and upgrade floor refusals are the helm's words,
  which the dialog shows as they are; the helm's own tests pin them.
- "queued Update refusals retain modal focus and isolation": `two_refusals_from_update_all_show_one_after_the_other`.
- The refusal dialog's Remove (the Remove half of "uninstall and Update planning refusals open framed dialogs"):
  `a_refusal_marked_remove_instead_offers_remove_host` (Close alone without the helm's marker; with it, Remove opens the
  removal confirmation), and for an uninstall's refusal, "Uninstall" below.
- "a Busy-refused UPDATE consumes its plan and leaves unrelated controls usable":
  `a_refused_confirmation_shows_under_the_row_only`; the store's one request per key keeps other hosts' controls usable.
- "failed UPDATE rerun submits automatically through the host update route":
  `re_run_of_a_failed_update_sends_the_update_again` (also the local host's greyed Re-run), and step 47.
- "update all dispatches remote hosts and keeps their results independent":
  `update_all_lets_each_host_keep_its_own_result`; "update all skips a host already busy with setup":
  `update_all_updates_every_available_remote_host_and_skips_the_rest` (a setup running, a save in flight, a newer host
  and the local host skipped).
- The inline `↑ Update` (no old spec drove it): `the_inline_update_shows_on_an_older_remote_host_and_starts_the_update`
  and `the_hover_names_both_builds_the_urgency_and_the_action`.
- A launch in flight (no old spec covered it): `a_launch_in_flight_refuses_the_update_before_sending`.
- The look: the goldens `hosts-panel-updates`, `host-update-running-hosts-panel` and `host-update-refusal`, which have
  no reference scene (none shows an outdated host or an Update; the refusal dialog's old look is
  `host-uninstall-refusal`'s), and `hosts-panel` and its siblings with Update all in the heading.

Superseded, with no replacing test: the cases about the old page's own bookkeeping around an Update, which the UI
channel makes unnecessary because the helm pushes every run before it replies and the store holds the one request per
host: "a retarget and a foreign run each invalidate an unsubmitted update", "a wrong-host UPDATE acceptance warns on its
source row and claims nothing", "a malformed accepted UPDATE warns, survives old success, and never replays", "a lost
submission reply survives a failed retry plan ...", "a progress read failure recovers on the next feed-driven real
read", "a progress read failure during an update recovers on retry demand alone", "a plan held under a held OpLock
submits exactly once on release", "a retarget and a removal before the claim each submit nothing", "removal before plan
completion submits nothing", "a stale Completed around submission settles nothing and replays nothing", the identity and
changed-host cases ("a first identity learned while ...", "changed-host advice while ...", "a changed host preserves
earlier update uncertainty while ..."), "incarnation churn keeps a followed run while a target change detaches it", "a
progress read racing submission commits nothing ...", "a held acceptance across a target change retires the old run
...", "a progress read across a held acceptance and target change cannot adopt the old run", and "a competing run during
retry planning preserves the sticky update warning". The helm still refuses a plan for a host that changed (its one-use
plan names the host it was made for). Not ported here: "held rail Update refusal keeps its avatar ..." (the rail's entry
points are the sidebar's). "An open Update card follows the new current step in a short viewport" is ported: the card
keeps its current step whole however short the window (crates/gui/farhelm-gui/src/hosts/progress/card/tests.rs
`a_short_window_previews_only_diagnostics_that_cannot_fit`), and the running status opens it from the Hosts row
(`hovering_the_running_status_shows_the_card_beside_the_sidebar`).

## `tests/provisioning.spec.ts` and `tests/terminal-multihost.spec.ts`, Uninstall

A remote host's Uninstall goes out from its menu as a plan request, opens the confirmation with the helm's plan, and
sends the plan's id when confirmed; the run shows in the host's row, and once the helm forgets the host the Hosts panel
shows the removal notice (`crates/gui/farhelm-gui/src/hosts/uninstall.rs`). The old uninstall cases were split between
`provisioning.spec.ts` (refusals) and `terminal-multihost.spec.ts` (the uninstall of the fleet's real remote). Replacing
tests, in `crates/gui/farhelm-gui/src/hosts/uninstall/tests.rs` unless named otherwise:

- "uninstall-remote-host: the plan names the kept data, confirming removes the host" and "uninstall-progress-inline: a
  running uninstall shows its progress in the row": `the_menus_uninstall_plans_asks_runs_and_leaves_a_notice` (the plan
  request without a body under the host's key, "Uninstalling…" while it is out with the row folded, "Uninstall Farhelm ·
  <name>" with the kept data note and focus on Uninstall, the confirmation naming its plan, the running row opened on
  its steps with the count and current step inline, the notice's words and full sentence once the host is gone, Copy
  putting the kept path on the clipboard, and × hiding it), `cancel_sends_nothing_and_owes_no_notice` (Cancel sends
  nothing and returns focus to the row's toggle), and end to end, step 61 of `scripts/gui-e2e-smoke.py` over the
  scripted backend. Folding the running row: `hosts/progress/tests.rs` `a_running_setup_opens_its_row_until_folded`.
- "uninstall and Update planning refusals open framed dialogs" (its uninstall half), and the refusals SPEC.md lists
  (sessions or tabs on the host, not connected, a `farhelm helm setup` unit, a foreign supervisor path, a symlinked lib
  directory): `a_refused_plan_opens_the_refusal_dialog_with_remove_only_when_marked`. The helm words each refusal and
  marks the ones that offer Remove; its own tests pin which (`crates/farhelm-helm/src/provisioning/uninstall_refusal.rs`
  and the uninstall tests in `crates/farhelm-helm/src/provisioning.rs`). End to end, step 61 opens the helm's
  not-connected refusal of a fake host.
- "confirmed uninstall refusal reopens its dialog": `a_refused_confirmation_opens_the_refusal_and_withdraws_the_notice`.
- "confirmed uninstall refusal survives a vanished host row":
  `a_refusal_after_the_host_vanished_still_opens_and_takes_the_notice_back` (the dialog names the host from the plan,
  and the notice its leaving the list showed goes with the refusal).
- "uninstall recheck refusal stays on the failed step": a refusal of the run's own check is a failed step of the pushed
  run, which opens the row like any failed run (`hosts/tests.rs` `a_failed_run_opens_only_its_own_row`);
  `a_failed_run_withdraws_the_notice_and_uninstall_continues_it` and `a_host_gone_after_a_failed_run_shows_no_notice`
  (SPEC.md: "choosing uninstall again continues from there"; the menu keeps Uninstall after a failure,
  `hosts/offers/tests.rs` `a_failed_run_is_offered_again_as_what_it_was`).
- A result after the user moved on (no old spec covered it):
  `results_after_the_user_moved_on_open_nothing_and_show_under_the_row`.
- A launch in flight (no old spec covered it): `a_launch_in_flight_refuses_uninstall_before_sending`.
- Which hosts offer Uninstall: `hosts/offers/tests.rs` `each_kind_decides_its_own_offers` and the menu's tests in
  `hosts/menu/tests.rs`.
- The look: the goldens `host-uninstall-refusal` (reference `host-uninstall-refusal`), `host-uninstall-confirmation`,
  `host-uninstall-running-hosts-panel` and `hosts-panel-uninstall-notice`, which have no reference scene, and
  `host-menu` with Uninstall in it.

Superseded, with no replacing test: "Settings waits while an uninstall confirmation is being planned" (the old modal
queue; here a host's own Settings waits while its plan request is out, as for every request on its key, and no other
host's does).

## `tests/terminal-multihost.spec.ts` and `tests/provisioning.spec.ts`, Retry and Adopt

The host menu's Retry tries a host's connection again now, and its Adopt accepts the identity a mismatched host reports
(`crates/gui/farhelm-gui/src/hosts/remedies.rs`); fixing the destination instead is the host settings dialog's
destination field. The duplicate entry's message is the phase detail the Hosts panel already drew, and its "then press
Retry" is this Retry. Replacing tests, in `crates/gui/farhelm-gui/src/hosts/remedies/tests.rs` unless named otherwise:

- "identity-mismatch-surfaced: both identities and an adopt for the reported one": both identities in the details,
  `hosts/identity/phase/tests.rs`; the Adopt naming the reported one,
  `adopt_sends_the_identity_it_showed_and_a_refusal_opens_the_dialog`.
- "identity-unverified-offers-no-adopt: the remedies are named instead": `hosts/offers/tests.rs`
  `retry_is_offered_in_every_phase_but_deletion_and_adopt_only_for_a_mismatch` and the unverified remedy line in
  `hosts/identity/phase/tests.rs`.
- "adopt-requires-current-identity: the displayed identity is sent, and a 409 is shown":
  `adopt_sends_the_identity_it_showed_and_a_refusal_opens_the_dialog` and
  `adopt_sends_the_identity_the_menu_showed_not_one_read_at_the_press`.
- Retry, which no old case drove alone (the old menu's first item): `retry_sends_one_attempt_under_the_hosts_key`, and a
  refusal after the user moved on, `a_refusal_after_the_user_moved_on_shows_under_the_row`. It is offered on every kind
  of host, the local one and one this build does not know included, as the old menu offered it on every row ("the local
  row menu contains Retry"), and in every phase but an environment's deletion (`hosts/offers/tests.rs`,
  `each_kind_decides_its_own_offers` and `retry_is_offered_in_every_phase_but_deletion_and_adopt_only_for_a_mismatch`).
- Fixing the destination instead (no old case): `fixing_the_destination_instead_is_the_host_settings_dialog`, and the
  destination field's own tests in `hosts/settings/tests.rs`.
- A launch in flight (no old case; the old page lock held both):
  `a_launch_holds_both_back_and_each_line_lasts_until_its_own_action_starts`.
- The look: the golden `host-menu`, with Retry first as in reference `host-menu`. No reference shows Adopt.

No end-to-end step: a fake host never connects, so the scripted backend cannot stage an identity mismatch or a
duplicate, and Retry on a fake host only restarts an attempt that fails again, which the in-process tests already pin.

## `tests/appearance.spec.ts`, partly

The old spec tested a choice kept in the web view's own storage. Appearance is a helm preference in this app (the
maintainer's decision of 2026-11-13), so its cases map onto the preference row's save and push, and onto the main view,
which hands the stored choice to the theme. Replacing tests, in `crates/gui/farhelm-gui/src/settings/tests.rs`:

- The choice, its save and the palette following it ("all appearance choices update live while the mounted terminal
  stays dark"): `an_appearance_choice_saves_only_its_preference_field`,
  `a_refused_appearance_save_shows_the_stored_choice_and_a_failure_line`,
  `an_unset_or_unknown_appearance_reads_as_system`, `the_stored_appearance_paints_the_window_from_the_first_snapshot_on`
  (the stored choice applied at the first snapshot, on every push and after a reset) and
  `system_follows_the_operating_system_and_an_explicit_choice_does_not`. The arrows and Space on the radios are the
  radio group's own keyboard, which its tests in `components/tests/radio_group.rs` cover. The terminal staying dark
  needs no test of its own here: the theme holds one terminal palette for both (`theme::TERMINAL`).
- Another client's choice ("another same-origin page updates appearance and clearing restores System", which is now
  another client of the same helm, and clearing is a row with no appearance):
  `an_open_appearance_section_follows_a_choice_made_elsewhere`, and the window's palette in
  `the_stored_appearance_paints_the_window_from_the_first_snapshot_on`.
- Surviving a remount and a reload ("appearance choices survive Settings remounts and reloads"): the section draws the
  stored row each time it opens (the refused-save test opens it over a stored Dark), and a reload is a new UI session's
  snapshot, which the main view applies like any push (the reset and reconnect in
  `the_stored_appearance_paints_the_window_from_the_first_snapshot_on`); the helm's preference route and the UI
  channel's tests cover the persistence itself.
- The agent build's `--appearance` start hook, which has no old counterpart: it keeps its palette for the run
  (`the_agent_builds_start_hook_wins_over_the_stored_appearance`).
- The order of the sections: `the_dialog_lists_only_the_sections_this_build_has`.
- The look: the golden `settings-appearance`.

Superseded: the four "... before application mount" cases. They held the browser's renderer module back to prove that a
script in the page's head colored the canvas before the app existed. This app has no page, head or renderer module to
hold; what the window draws before the first snapshot is a deviation, below.

## `tests/host-checkout-folder.spec.ts`, partly

The host settings dialog draws the shared folder field (`settings::CheckoutField::folder` with
`settings::FolderScope::Host`) in its Checkouts section. Replacing tests, in
`crates/gui/farhelm-gui/src/hosts/settings/tests.rs`, each opening the dialog from a host's menu:

- Opening the field: `the_checkouts_section_sits_between_appearance_and_permissions_on_every_host` (one read of
  `/api/hosts/<id>/checkout-root` as the dialog opens, the section between Appearance and Permissions, the empty field
  with "Default: <folder>" as its placeholder and no Use default, the old note's "new checkouts" wording, and the local
  host's dialog with "No default set").
- Saving, refusing, reading back and clearing: `the_host_folder_saves_under_the_hosts_key_and_use_default_clears_it`
  (`~another-user/work` refused with the helm's words, wrapped inside the section, and the draft kept, a folder saved
  under the host's key and read back with "Checkout folder saved" and Use default, and Use default sending
  `{"root": null}`, then the empty field with the default as placeholder and Use default gone).
- The old field's lock while other host writes ran, replaced by the host's operation key:
  `saves_on_the_host_wait_for_each_other_without_locking_the_folder` (Deviations, "One save per host at a time").

The field's own tests in `crates/gui/farhelm-gui/src/settings/tests.rs` cover the rest:
`a_host_folder_shows_the_default_as_placeholder_and_use_default_clears_it` counts the field's `Saved` event (the old
`on_saved`, the launcher's hook), one for the done save and none for a refused one, and an empty field clearing the
override is the same code as the default folder's, tested in the `tests/settings.spec.ts` section above. The goldens
`host-settings-dialog` and `host-settings-dialog-end` show the section.

Not ported here, and why: the launch preview reaching the host's folder (the spec's launcher half, and its later default
change reaching the preview) belongs to the launcher's preview. The old case checked that nothing in the field overflows
at 1280 and 390 pixels wide; here the shared frame is never wider than the window and the field takes the section's
width, and the one part that did overflow, a long refusal, is pinned by the save test above at the dialog's own width.
The fix is in the shared field, so a long refusal or read failure in Settings → Checkouts, which was cut off the same
way, wraps too. No test drives a narrow window.

## `tests/workspace-trust.spec.ts`, partly

Its Settings case ("managed-checkout trust switches persist independently of Claude's legacy route") is ported, with the
Settings half of `tests/claude-trust.spec.ts` (whose row is in `launcher.md`: the hint band is the session view's): the
switch the band's "Settings and caveats" opens, and the hint switch that reverses its Don't show again. Replacing tests,
in `crates/gui/farhelm-gui/src/settings/tests.rs`:

- The read when the section opens, and no toggle over a choice not known:
  `agent_trust_reads_each_agent_once_when_shown_and_waits_for_the_answer`,
  `a_failed_trust_read_leaves_the_switch_unavailable_with_its_reason`,
  `a_trust_read_reaches_only_the_opening_it_was_sent_for`.
- Saves through each agent's own route, and a refusal with the helm's reason:
  `a_trust_switch_saves_through_its_route_and_a_refusal_restores_it`. Persistence across a reload is the helm's routes'
  own, which their tests cover; the old case's write through Claude's legacy route is the helm's too.
- The hint switches as the shared dismissals: `the_hint_switches_are_the_shared_dismissals_inverted`, and
  `name_sessions_on_quick_launch_reads_as_on_when_the_helm_stored_no_choice` for unset, `false` and `true`.
- The entry point a trust hint calls (`settings::open_at_agent_trust`):
  `the_trust_hint_entry_point_focuses_its_agents_switch_once_known`,
  `a_trust_read_that_answers_after_the_user_moved_on_moves_no_focus` and
  `a_click_that_leaves_focus_on_the_dialog_still_stops_the_trust_handover`.
- The look: the golden `settings-agent-trust`.

Not ported here: the two hint-band cases ("... trust hint stays scoped to its managed agent terminal and own dismissal",
"Pi and command launches do not borrow a workspace trust hint") and the rest of `claude-trust.spec.ts` belong to the
band, which the session view draws.

## `tests/podman-environments.spec.ts`, partly

Settings → Containers is ported (`crates/gui/farhelm-gui/src/settings/containers.rs`). Of the spec's ten tests, two are
Settings': "Containers Settings keeps failed cleanup explicit", and the `recipes` surface of "refreshes recipe places
after a delayed probe without a fleet notice", which is Settings → Recipes' and waits for it. The other eight, and the
`launcher` surface of the delayed-probe test, are the launcher's (new container placement, recents, quick launch) or the
session view's (a connected container's Delete), and are listed in those areas' parity files as they are ported.

Replacing tests, in `crates/gui/farhelm-gui/src/settings/tests.rs`:

- "Containers Settings keeps failed cleanup explicit":
  `a_leftover_delete_asks_sends_its_place_and_id_and_shows_a_refusal` (the region lists the leftover, Delete asks
  "Delete container?" with its consequence and sends nothing, Cancel sends nothing, Delete container sends the place and
  canonical id, the helm's refusal shows and the row stays) and
  `a_leftover_with_a_host_row_is_deleted_through_host_removal`; the spec's Container hosts region holding "Build
  machine" is `containers_reads_the_hosts_and_leftovers_once_then_checks_each_container_host`, which also checks that
  each container host is checked again on showing, as the old section did on mount.
- With no old case: `a_pushed_result_changes_the_row_with_no_request_sent`, `before_a_result_the_places_read_shows`,
  `a_refused_check_keeps_its_reason_until_the_next_round` and `a_fleet_change_the_places_reflect_reads_them_once`
  (following the pushed hosts collection, which stands in for the old section's polling and its re-read on fleet
  notices), `a_refresh_drops_the_older_rounds_check_answers` and
  `an_older_rounds_reads_answering_after_a_refresh_are_dropped` (refresh rounds), and
  `failed_container_reads_show_their_reasons_and_never_none_found` (the old reader's "a failed discovery must stay an
  error").
- The look: the golden `settings-containers` (`crates/gui/farhelm-gui/src/goldens/settings.rs`).

The helm answers the listing and a Delete within its host reply bound (decided by the migration's orchestrator on
2026-11-15, under its rule that every host-waiting route the GUI sends is bounded); its tests are in
`crates/farhelm-helm/src/environment/podman/tests.rs` (the listing and the leftover Delete against a stalled container
host, and the listing asking its hosts concurrently) and
`crates/farhelm-helm/src/provisioning/session_launch_failure_tests.rs` (removing a container host).

## The host menu and host settings

A Hosts panel row's ⋯ opens the host's actions menu beside it, and its Settings opens the host settings dialog. The
entry points are `hosts::open_menu` (at an anchor, with the control the settings dialog gives focus back to; the host
rail's avatar menu calls it too, with the avatar as that control) and `hosts::open_settings` (with the control its
closing returns focus to).

Replacing tests, in `crates/gui/farhelm-gui/src/hosts/menu/tests.rs` and
`crates/gui/farhelm-gui/src/hosts/settings/tests.rs`:

- The menu: `a_remote_hosts_toggle_opens_a_menu_starting_on_retry` (its name, header and first command, and Escape back
  to the toggle), `the_local_hosts_greyed_update_says_why_and_sends_nothing`,
  `settings_waits_for_the_hosts_own_work_in_flight`, `a_second_press_on_the_toggle_closes_its_menu`,
  `escape_on_the_menu_returns_focus_to_its_toggle_under_the_macos_click_rule`, and in the installed desktop app
  `in_the_desktop_app_the_local_hosts_update_asks_the_updater_to_check` (the local row's Update asks the app's updater
  for a check and sends the helm nothing). What it offers is `offers::menu_offers`, whose tests are listed in the
  `tests/provisioning.spec.ts` row, with `in_the_desktop_app_the_local_hosts_update_checks_for_a_release` in
  `hosts/offers/tests.rs`.
- Opening and closing ("rail ${action} modal ... returns to its avatar", its row half):
  `focus_starts_on_the_first_edit`, `escape_and_both_close_controls_return_focus_to_the_toggle`,
  `the_dialog_closes_when_the_registry_no_longer_lists_its_host`,
  `a_click_on_edit_focuses_its_field_under_the_macos_click_rule`.
- Saving (no old spec covered alias, appearance or permissions): `each_permission_saves_its_own_route_and_field`,
  `a_refused_permission_shows_the_stored_value_and_the_helms_words`,
  `the_destination_saves_as_typed_and_a_refusal_stays_in_place` (a refusal, and the helm's answer that the change waits
  for a setup or Update on the host, kept in place), `the_alias_saves_and_clears_and_escape_closes_only_its_field`,
  `escape_while_a_save_is_in_flight_keeps_the_field_open`, `an_appearance_choice_saves_icon_and_color_together`,
  `the_picker_is_reached_and_used_by_keyboard`.
- The Checkouts section: the tests listed under `tests/host-checkout-folder.spec.ts` above.
- The look: the goldens `host-menu`, `host-settings-dialog` and `host-settings-dialog-end`
  (`crates/gui/farhelm-gui/src/goldens/hosts.rs`).

Not ported here: `modal-layout.spec.ts`'s host settings cases, whose margins and scrolling body are the shared dialog
frame's (its own tests and goldens), and whose busy-body Tab stop has no counterpart, since no control here goes
unavailable while a save runs.

## `tests/tensorlake-recipes.spec.ts`, partly

Settings → Recipes lists the shared catalog with each recipe's summary, its per-place container lines with a Rebuild per
place, its built-snapshot status and its prepared-snapshot line, and builds snapshots. New recipe and each row's Edit
open the recipe editor in place of the list, with Name, Size, the Software cards and each base's fields, Ubuntu packages
and Tunnels, and Save; the editor's GitHub and Agents blocks with their credential pickers; Delete and the question
before leaving with unsaved changes come in later changes. Replacing tests, in
`crates/gui/farhelm-gui/src/settings/recipe_tests.rs`, `crates/gui/farhelm-gui/src/settings/recipe_place_tests.rs`,
`crates/gui/farhelm-gui/src/settings/recipe_editor_tests.rs` and, for the editor's draft without a window,
`crates/gui/farhelm-gui/src/settings/recipes/draft_tests.rs`:

- "a recipe's snapshot builds, rebuilds, recovers from a failed step and goes with the recipe": partly. The row's status
  in every build state, with the one action each offers (Build snapshot, Rebuild snapshot, Retry, or none while a build
  runs or for a base Tensorlake cannot run), `snapshot_status_follows_the_build_state`,
  `base_changes_keep_the_snapshot_and_build_actions` and
  `container_bases_explain_their_provider_and_offer_no_tensorlake_build`; the summary's words,
  `summaries_follow_the_mockup`; Build accepted under the recipe's key and followed through the pushed catalog to its
  end with nothing read, `build_is_accepted_under_the_recipes_key_and_the_push_follows_it_to_its_end`; no build without
  a saved API key, `without_an_api_key_build_is_unavailable_and_says_why`; a refused build's words under its row,
  `a_refused_build_shows_the_helms_words_under_its_row_until_the_next_build`, dropped once a push shows the recipe
  building, `a_push_showing_the_build_running_drops_an_older_refusal_for_good`; the rows from the pushed catalog, with
  only the account read when the section shows, `recipes_draws_the_pushed_catalog_and_reads_only_the_account_when_shown`
  and `a_pushed_change_redraws_the_rows_with_no_read`. The look: the goldens `settings-recipes` (empty, as the
  reference) and `settings-recipes-rows` (built with changed packages and rebuilding, failed, and a container base).
  What a build does in Tensorlake (the builder's source, its steps, the snapshots it keeps and deletes) is the helm's,
  tested in `crates/farhelm-helm/src/tensorlake/build_tests.rs`. Delete is the editor's (the managed recipe row below).
- "a failed recipe refresh says so and Retry loading recovers":
  `an_unreadable_catalog_keeps_the_rows_and_retry_loading_asks_again`, with the failure and the recovery pushed by the
  helm (see "Recipes" under Deviations for what Retry loading sends); a failed read with no rows,
  `an_empty_catalog_says_so_and_an_unreadable_one_shows_only_its_reason`.
- "recipe preparation reports lazy progress, failure and readiness": partly. The line in every preparation state, with
  no action of its own, `prepared_status_reports_all_phases_and_lazy_recovery` and
  `prepared_status_keeps_readiness_last_checked_and_ages_display_only`; its age moving on the clock with nothing read,
  `a_ready_snapshots_age_moves_on_the_clock_which_goes_when_hidden`, and its pulse,
  `a_running_preparation_pulses_until_the_section_is_hidden`. What preparation does in Tensorlake (the preparer, its
  cleanup and the snapshot it records) is the helm's, tested in `crates/farhelm-helm/src/tensorlake/build_tests.rs`.
- "recipe preparation explains ${state} without a Prepare now action": `prepared_status_explains_each_helm_stale_reason`
  and, for the interrupted state, `prepared_status_reports_all_phases_and_lazy_recovery`; the stale look is the golden
  `settings-recipes-places`.
- The per-place lines and their Rebuild, which no case of this spec drove (`tests/podman-environments.spec.ts`'s
  "recipes refreshes recipe places after a delayed probe without a fleet notice" is the launcher area's row, and its
  Recipes half is ported here): `the_pushed_places_draw_a_line_per_container_place_with_its_hosts_name`,
  `places_not_pushed_yet_read_as_reading`, `unreadable_places_show_their_reason_in_the_row`,
  `a_places_rebuild_is_accepted_under_the_recipes_key_and_the_push_shows_it_building`,
  `a_refused_rebuild_shows_under_the_row_until_a_push_shows_it_building`,
  `a_refusal_stays_while_another_place_of_its_recipe_already_builds`, `a_place_without_a_line_building_keeps_a_refusal`,
  and the check asked on opening, its outcome brought by the helm's push of the places,
  `container_hosts_are_checked_once_and_the_push_brings_the_checks_outcome`; the look, the golden
  `settings-recipes-places`.
- "Settings keeps recipe answers sticky and isolated to Tensorlake in ${palette}": partly. New recipe opens the editor
  in place with focus on Back, Cancel and Save pinned with the dialog's footer above the Close row, and Back returning
  to the list, `new_recipe_opens_the_editor_in_place_and_back_returns_to_the_list`; switching sections hides the answers
  and keeps the editor, `switching_sections_hides_the_answers_and_keeps_the_open_editor`; the invalid name's and the
  helm's refusals beside the pinned answers, `a_bad_name_or_size_is_refused_locally_until_the_next_edit` and
  `a_refused_save_keeps_the_draft_and_leaves_no_refusal_under_the_row`; the editor opening at its top, Back included,
  from a scrolled list, `the_editor_opens_at_its_top_from_a_scrolled_list`; the form locked while a save is out, the
  credential choices included, `while_a_save_is_out_the_form_takes_no_edit`. The look, in both palettes: the golden
  `settings-recipe-editor` (see "Recipes" under Deviations for the answers' band). The leave and delete questions in the
  answers' place, `every_way_out_asks_first_and_keep_editing_stays` and
  `delete_asks_then_deletes_under_the_recipes_key`. No golden draws them: the reference screens have none.
- "a managed recipe is created, edited and deleted without exposing its token": partly. Creation under the recipe's key
  with its row from the pushed catalog, `a_new_recipe_is_created_under_its_key_and_its_pushed_row_shows`; Edit with the
  name fixed, `a_saved_recipes_name_is_fixed`; Edit then Save sending the recipe's endpoint, login and GitHub token
  names back unchanged, `edit_then_save_sends_the_recipes_credential_choices_back_unchanged` and, without a window,
  `an_unchanged_reopened_recipe_saves_back_unchanged` and
  `named_endpoint_selection_survives_origin_changes_and_can_be_removed`; choosing the GitHub token and the Claude Code
  endpoint in the form, `each_picker_offers_its_kind_and_save_sends_the_names_chosen`; Delete with its confirmation in
  the old words, sent under the recipe's key and closing the editor when done,
  `delete_asks_then_deletes_under_the_recipes_key`, a refusal the helm gives at once kept in the editor,
  `a_refused_delete_stays_in_the_editor`, and a cleanup still running at the helm's bound (see "Recipes" under
  Deviations), `a_delete_that_carries_on_closes_the_editor_and_the_row_says_so`.
- "the recipe editor refuses bad names, asks before leaving, and saves a custom base": partly. The local name refusal,
  `a_bad_name_or_size_is_refused_locally_until_the_next_edit`; the helm's refusal kept with the draft,
  `a_refused_save_keeps_the_draft_and_leaves_no_refusal_under_the_row`; the cards, each base's fields, the requirements
  note and build help, and every base's text and the packages kept across cards,
  `each_base_card_shows_its_fields_and_keeps_every_bases_text`, `drafts_become_requests_or_local_refusals`,
  `all_bases_reopen_with_their_packages`, `reopening_a_container_base_preserves_its_reference_and_user` and
  `a_new_container_recipe_saves_edited_reference_and_user`; tunnel rows, `tunnel_rows_are_added_and_removed_by_row`; a
  save refused for the recipe's build in flight, `a_save_while_the_recipes_build_is_out_says_why_and_sends_nothing`; the
  question before leaving, from Back, Cancel, Escape and both close controls,
  `every_way_out_asks_first_and_keep_editing_stays`, with no question for an unchanged editor,
  `an_unchanged_editor_leaves_without_asking`.
- "${agent} modes retain drafts and select only named ${agent} logins": the draft's rules,
  `claude_modes_keep_drafts_and_submit_only_the_selected_credentials` and
  `codex_modes_keep_drafts_and_submit_exclusive_selections`; the mode choice keeping each mode's fields, with a local
  refusal for Subscription without a login, `switching_an_agents_mode_keeps_each_modes_draft`; each login picker
  offering only its agent's logins, `switching_an_agents_mode_keeps_each_modes_draft` (Claude) and
  `each_picker_offers_its_kind_and_save_sends_the_names_chosen` (Codex); the catalog read as the editor opens, with
  Retry loading credentials, `the_catalog_is_read_as_the_editor_opens_and_retry_reads_it_again`. That a referenced login
  cannot be deleted is the helm's, and the Credentials editor shows it
  (`delete_is_unavailable_while_a_recipe_uses_the_credential`).
- "closing Settings with an unsaved recipe asks first and finishes the close": the question on Escape and both close
  controls, `every_way_out_asks_first_and_keep_editing_stays`; Discard finishing the return or the close unsaved,
  `discard_finishes_the_leave_without_saving`; Save finishing it once the save is done, and a refused save keeping the
  editor, `the_questions_save_finishes_the_leave_once_the_save_is_done`; a close from another section showing Recipes
  with the question first, `a_close_from_another_section_shows_recipes_first`; a close while the save is out waiting for
  it and finishing even unanswered, `a_close_while_the_save_is_out_waits_for_it`, and one while a delete is out closing
  at once, `a_close_while_the_delete_is_out_closes_at_once` (see "Recipes" under Deviations for both); a newer question
  winning over an earlier answered Save, `keep_editing_after_an_earlier_save_answer_stays_in_settings`; and a refused
  save taking an open question away, `a_refused_save_takes_an_open_question_away`.
- "a recipe opening preserves a newer field focus before header close": partly. The editor's opening moves focus to Back
  only if the user has not moved on since the press that opened it (`Recipes::open_editor`, through the focus
  coordinator's mark), and the header close's question takes focus to its Save,
  `every_way_out_asks_first_and_keep_editing_stays`. Not ported: a test that focuses a field between the opening and its
  first frame.

## Deviations

Where the port does not match the old app, and why. Each stays until the work named in it lands.

### The Settings dialog

- **The header's summary follows whether App is listed.** The old dialog always read "For every host on this helm, and
  for this device"; it kept appearance and the status sounds per device. Here those are helm preferences, so only App's
  switch is this device's, and without App (Linux, dev and agent builds, where no updater runs) the summary reads "For
  every host on this helm" (decided by the migration's orchestrator on 2026-11-15). The goldens without App show the
  shorter words.
- **Appearance starts at System on the switch to this app.** The old app kept the choice in each device's web view
  storage, and nothing reads it: the maintainer decided on 2026-11-13 that the moved settings reset to their defaults,
  as a breaking change named where it ships.
- **The connecting screen is in the operating system's palette.** The theme starts every launch at System, and the
  stored choice arrives with the channel's first snapshot, so with Dark or Light stored against an operating system in
  the other appearance, a window shown before the snapshot ("Connecting to the helm…") is in the system's palette and
  switches when the snapshot arrives. Measured in-process, not in the running app:
  `the_stored_appearance_paints_the_window_from_the_first_snapshot_on` opens the main window before the first snapshot
  on a light system and reads its palette as light, then dark once the snapshot storing Dark arrives. How long the
  connecting screen stays up with the embedded helm, and so whether a user sees the switch, has not been measured. The
  old app colored its page from device storage before anything drew. Accepted by the migration's orchestrator on
  2026-11-14, with no device-local cache of the choice.
- **The Appearance help is reworded.** The old line said "for this device"; this one says the choice applies to the helm
  and that a change from the command line or an agent shows right away, and that System follows each device's own
  appearance. The other Settings help that the old app ended with "when they reload" says the same (the orchestrator's
  drift scan of 2026-11-15: one desktop app talks to its helm, so "other open clients" misled).
- **A trust hint's way into Settings focuses the agent's switch, later, and reveals only the switch.** The old entry
  point focused the agent's whole block (`#checkout-<agent>-trust`, which took focus with `tabindex=-1`) at once, and
  both old specs assert that. Here the dialog opens with focus on itself and moves focus to the agent's switch once the
  section's read of that agent has answered, since an unavailable switch cannot hold focus; it moves nothing if the user
  has moved on by then. The move scrolls the switch into view. That is closer to SPEC.md's "open agent trust at the
  relevant switch", but SPEC.md also says the hint scrolls "its block into view", and what is revealed is the switch,
  not the whole block: for a switch that started below the fold (Muse's and Pi's at the dialog's full height), the help
  under it, and Muse's hint switch, can stay below the fold. The switch-only reveal is accepted as it stands: revealing
  the whole block would take a change in the focus coordinator, and none is requested.
- **The hint switches can be used before the trust read answers, and when it fails.** The old ones were unavailable
  until the read came back. Here whether a hint was dismissed comes from the pushed preference row, not from the trust
  read, so a hint switch shows the stored dismissal from the start and saves like any preference switch.
- **A trust switch's save is refused while another Settings save is in flight**, with a line beside the switch saying
  so, like a preference switch's (below). The operation store admits one request per key, and helm configuration (where
  trust lives) has no key of its own, so trust saves go under the preference row's key. This area accepted that on
  2026-11-14, under its goal's instruction not to request a key for helm configuration before it is needed: the embedded
  helm answers these saves in milliseconds, so they rarely meet. The old trust switches each had their own request and
  were unavailable only while their own save ran.
- **The tunnel switch looks and behaves like the other switches where the old one did not.** Its help sits 4 pixels
  under it, as every switch's does, where the old desktop switch and its help were separate children of the section, 14
  pixels apart. It has hover help (the help's first sentence), which every control here has; the old one had none. It
  shows the pushed preference from the start, where the old one was unavailable until its own read of the preferences
  answered. And a press while another Settings save is in flight asks nothing and says why (below), where the old switch
  was unavailable while any tunnel action ran. Like the old one, it is drawn only on the Mac: the old desktop app was
  the Mac app, and on Linux the terminal offers no tunnel while the switch, its help and its warning speak of this Mac.
  SPEC.md's "In the desktop app, the Terminal section also has tunnel consent" names no platform, while its tunnel
  paragraph offers tunnels "In the Mac app".
- **Turning the tunnel switch off shows off while the save is in flight.** The old switch held its last acknowledged
  value in both directions while a request ran. Here turning it off is a draft's save like any switch's, which shows the
  value being saved (the area's no-optimistic-copy rule allows a pending value only as the draft); turning it on still
  shows the stored value until the helm pushes consent.
- **When Allow's save meets another preference save, the busy line shows under the tunnel switch.** The press checks the
  preference row's key before asking, so this happens only when a save starts while the warning is up. The words are
  Settings' own busy line ("Not saved: another operation was in progress. Try again."), which the terminal area's tunnel
  entry point gives as its refusal text.
- **A tunnel switch refusal that arrives after Settings has closed is shown nowhere.** The old app's tunnel controller
  put such a failure in its global tunnel notice when no control was left to show it. Here, as for every Settings save,
  the reply settles nothing on screen once the dialog is gone; with no optimistic copy the switch's stored value is
  still right when Settings opens again.
- **No one-pane layout at 620 pixels and below.** The old dialog showed either the section list or the section below
  that width, with "‹ Settings" to go back. The window's minimum width (720 pixels) is above it, so the layout is
  unreachable and is not built.
- **A section switch leaves focus where it was.** The old app moved focus to the shown section's heading, so a keyboard
  user went on from the top of the section. The focus coordinator moves focus to a control only while no layer is open,
  and the heading is not a control, so focus stays on the section button (Linux), or where it was (macOS, where the
  click does not focus the button; a control in the section that was hidden hands focus to the closest one left).
- **A click or arrow press while another preference save is in flight is refused, and says so.** The old app queued
  every switch's newest click per field and sent it after the earlier one ("the latest click wins"). Here the operation
  store admits one request on the preference row at a time, so the second click sends nothing: a second click on the
  same switch leaves it showing the value in flight, and one on another switch leaves that switch as it was, and either
  way "Not saved: another operation was in progress. Try again." shows under the control pressed, until a save from the
  same view starts or another press is refused; the user clicks again after the reply, which the embedded helm sends
  within milliseconds. The words are in the past tense because the line outlives the save that blocked the press. In a
  radio group (Appearance, Terminal's link choice) the arrows move focus and the choice together, so a second arrow
  press while the first choice's save is in flight moves focus to the next option while the choice stays where the first
  press put it, with the line under the group. The refusal was accepted by the migration's orchestrator on 2026-11-14,
  since preference saves are helm-local and take milliseconds, to be revisited only if a user meets it; the orchestrator
  asked the same day that it not be silent. Covered by `a_press_during_another_settings_save_says_why_it_sent_nothing`
  and `the_fields_the_spare_switch_and_ntfy_say_why_a_press_sent_nothing`.
- **Checkouts' fields are refused while another Settings save is in flight.** The default checkout folder and the
  post-clone command each had a request of their own in the old app, so no other Settings save ever held their Save
  back. Here they are helm configuration saved under the preference row's key, like the switches (above), so their Save
  sends nothing while any other Settings save is in flight, and the same line shows under the field until its next save
  starts.
- **Every switch reports its refused save.** The old app showed no failure for the two host switches, nor for turning
  Skip session confirmations on, which SPEC.md's best-effort list in "Errors and diagnostics" kept silent because the
  old choice had already taken effect in an optimistic copy. With no optimistic copy a failed save never took effect, so
  the migration's orchestrator decided on 2026-11-14 that these report it too, with new failure lines worded like the
  old ones, and SPEC.md says so in the same place.
- **Turning Skip session confirmations off restores confirmations only once the helm has saved it.** The old app
  restored them in the window at once, whatever the helm answered (`session-confirmations.spec.ts`, "restoring
  confirmations reports save failure and recovers"). Here every session confirmation reads the helm's stored choice, so
  a refused save leaves them skipped, the switch back on and its failure line shown. The maintainer chose this on
  2026-11-14 (follow the helm), and SPEC.md's "Errors and diagnostics" says so.
- **The section list does not stay put while a long section scrolls** (the old `position: sticky`). Of the sections
  built so far, Checkouts (next item) and Notifications (Sounds above the ntfy form, as in the old app) scroll even at
  the dialog's full height, and the list then scrolls with it.
- **Checkouts does not fit the full-size dialog.** The old section fit the 780-pixel dialog with about 25 pixels to
  spare; here it is about 30 pixels taller than the dialog's body, so the body scrolls. Unscrolled, its last control,
  the spare switch's documentation link, sits at the body's visible edge with its descenders and underline below it
  (`settings-checkouts` golden); scrolled to the end, the link lies wholly inside the body, above the footer
  (`the_tallest_sections_last_line_clears_the_footer_scrolled_to_the_end`), so nothing sits under the footer for good.
  The two causes are recorded below: the taller header (about 10 pixels) and the link on a line of its own. Putting the
  link back at the end of the help's last sentence is not cheap: the host-text sentence pieces (`HostSentence`) join
  text parts only, and GPUI has no inline flow for a control inside a paragraph.
- **A refused checkout field keeps its reason until the next save starts.** The old folder and post-clone fields cleared
  the line on the next keystroke. Here the reason is the draft's (`DraftInput`), which reports edits to no one else, so
  the line stays while the user corrects the field, as the Rename editor's does.
- **The checkout folder field shows the trimmed folder only once the helm has saved it.** Save sends the folder trimmed,
  as before; the old field also trimmed its own text as Save was pressed, so a refused folder showed trimmed. Here the
  text stays as typed until the read after a done save shows the stored folder.
- **The post-clone command saves over a stored carriage return.** The old field refused to save a nonblank command when
  the stored one held a carriage return, because a browser's text area turned it into a line feed before the draft saw
  it, and a save would have rewritten the command silently. GPUI Kit's multi-line field keeps a carriage return as it is
  (gpui-base 0.9.1 strips line breaks only in single-line fields), so the guard has nothing to guard and is not ported.
- **The spare switch's documentation link sits on its own line** under its help, where the old one ended the help's last
  sentence: GPUI lays text out in blocks and has no inline flow for a control inside a paragraph. The extra line is one
  of the two causes of Checkouts' overflow (above).
- **The header is about 10 pixels taller** than the old one: the shared dialog frame lays out its title and summary on
  GPUI's default line height, which is taller than the browser's `normal`; Rename's header has the same height.

### Notifications

- **A refusal stays until the next save starts.** The old form cleared its outcome line on any edit; here the helm's
  words stay under the form, as every Settings failure line does, until the next Save.
- **A Send test after a refused Save shows its outcome below the save's refusal.** The old form had one outcome line,
  which a Send test emptied before showing its own. Here the refused save's words are the draft's and stay until the
  next save, as above, so the test's "Test alert sent" or its refusal shows beneath them; the two lines are about
  different requests, and the test used the settings the helm stores, not the refused ones.
- **Save is refused while another Settings save is in flight**, with the same line under the form, as Agent trust's
  switches are (above): ntfy alerts are helm configuration with no operation key of their own, so they save under the
  preference row's key.
- Not visible, recorded for review: the form is one draft over all its fields, saved in one request, rather than one
  `DraftInput` per text field, because `DraftInput` saves only its own field. The migration's settings-and-hosts area
  decided on 2026-11-14 to keep this exception until a second whole-form save makes it worth building into the drafts
  module.

### Recipes

- **Retry loading asks the helm to read again by sending the account read.** The old button read the catalog again. Here
  the catalog is pushed, and the GUI has no read of it left; the helm re-runs every pushed read after each command it
  answers, so Retry loading sends the section's one remaining read (`GET /api/tensorlake`), and the helm's next push
  shows whether the catalog could be read. The visible behavior is the old one; what is sent differs.
- **A refused build's words outlive Settings.** The old row kept them in the dialog and lost them when Settings closed.
  Here the build is work on the recipe's own operation key, which keeps a refusal until the next build of that recipe
  starts or the pushed state shows the recipe start building, its snapshot or one of its container places (the old row's
  rule, settled on the start because every push re-checks it), so reopening Settings still shows it.
- **The running dot sits on the build's own line.** The old row drew it before the snapshot status's last line, which a
  pending cleanup or an unsupported-base warning can follow; the dot then marked a line that is not running. Here it
  marks the "Building snapshot" line wherever it falls.
- **The no-key note names where the key is saved.** Without a key, Build snapshot is unavailable beside a note. The old
  note read "Save an API key above to build snapshots.", which was already inaccurate in the old app, whose key forms
  were in Tensorlake and Credentials, both listed below Recipes. The key form is in Credentials here, so the note reads
  "Save an API key in Settings → Credentials to build snapshots."
- **The editor's answers are a band of the dialog's footer.** The old Cancel and Save stuck to the bottom of the section
  panel's column (`.recipe-editor-sticky-actions`, `position: sticky`), as wide as that column, with a rule above them
  and the dialog's own footer rule under them. GPUI has no sticky positioning, so Settings pins them in its frame's
  footer region above the Close row (the region the launcher's pinned question uses), as wide as the dialog, ruled off
  from the Close row by a rule of its own; above them the footer's rule stands in for the old band's. Switching sections
  hides them with the editor, as before. No reference scene pictures the editor, so the comparison is with the old
  stylesheet; the golden `settings-recipe-editor` pins the band.
- **The editor opens with focus on Back, not its title.** The old editor focused its title, a heading made focusable;
  headings are not controls here. Back sits right above the title, so moving focus there brings the editor's top into
  view as the old focus did.
- **Size and port fields are text fields.** The old ones were number inputs with the browser's step arrows; here they
  are plain text fields, and Save refuses anything but a whole number in the old words ("CPUs must be a whole number",
  "Tunnel ports must be from 1 to 65535").
- **The Name field takes any length.** The old Name input stopped taking characters at 32 (`maxlength`); the text field
  here has no length cap, so a longer name can be typed, and Save refuses it before sending anything, with the helm's
  shared rule's words ("A recipe name can be at most 32 characters").
- **Every Software card dims while a save is out.** The old cards were disabled buttons during a save, which changed
  only their cursor; here an unavailable card is drawn at half opacity, as an unavailable segment or chip of the shared
  choice control is (its plain radio look dims only the circle), so for the length of each save all four cards dim.
- **The recipe editor's pickers show the credential catalog as it was when the editor opened.** The old editor re-read
  the catalog on every change the helm announced. The catalog is not pushed, so this editor reads it once per opening
  (this app's rule for what is not pushed; the migration's orchestrator kept that rule here on 2026-11-15 rather than
  re-reading when Recipes shows again). The common case is the editor's own help sending the user to Settings →
  Credentials: a credential added there while the editor stays open is not offered until the editor's next opening,
  which means Back and Edit again. A credential deleted meanwhile shows as unavailable only at the next opening too; a
  Save that names it is refused by the helm with its own words.
- **The credential pickers show names in the interface font.** The old pickers were monospace (`font-mono`); the shared
  select has no monospace look, so the names read as the rest of the form's choices do.
- **The leave question goes when it is answered.** The old question stayed up while its Save ran and after a refusal,
  with the reason above it, so the user could answer again. The prompt's inline confirming answer runs once, so here
  Save takes the question away; a refused save shows the helm's words above the ordinary answers, and leaving again asks
  again.
- **A delete whose cleanup outlasts the helm's bound closes the editor and leaves the helm's words on the row.** The old
  editor waited for the delete however long its cleanup took. The helm now answers within 20 seconds, with a 504 saying
  the deletion carries on, the recipe leaving the list once it is gone, and, if it is still listed later, to delete it
  again to see why (a late failure is only in the helm's log). The editor has nothing left to edit, so it closes as for
  a done delete, and the helm's words stay on the recipe's operation key, where the row shows them under its build
  button until the push takes the recipe away. This area decided it on 2026-11-15, as the nearest to the old app the
  bound allows: the user ends at the list either way, and the row says why the recipe is still there. The editor tells
  the two apart by the status alone, and the helm answers any cleanup failure of its timeout kind with a 504 too, so
  such a failure inside the bound also closes the editor; the row then shows the helm's own words for that failure while
  the recipe stays listed.
- **While a save is out, the leave question offers Keep editing and Save, and no Discard.** The old question, asked by a
  close during a save, drew all three answers and disabled every one until the save ended. The prompt cannot disable one
  answer, and a Discard could not keep its word, since the draft on screen is the one being saved. Keep editing and the
  question's Save stay usable: Save waits for the save already out. As in the old editor, that save's done answer
  finishes the close even while the question is unanswered. A refused save takes the open question away, so the ordinary
  answers return under the helm's words; the old question came back with all three answers.
- **A close while a delete is out closes at once.** The old editor asked the leave question then, with every answer
  disabled, and dropped the close once the delete succeeded. Here Settings closes without asking: the recipe is going,
  and the delete goes on in the operation store, its refusal, if any, shown under the recipe's row.
- **A refused save that lands after Settings closed shows under the recipe's row.** Settings closing drops the editor,
  so a refusal that arrives after it has nowhere else to show; it stays on the recipe's operation key, which the row's
  build shares, and the row shows it in the helm's words until the recipe's next build or save starts or a push shows
  the recipe start building. The old editor dropped such a refusal unseen. A refusal the editor showed itself leaves the
  key, so it never shows twice.
- **A place's name comes from the pushed hosts.** The old rows named each place from `GET /api/environment-places`, and
  re-read the places whenever that list, read again on every feed notice and polled while a check ran, changed. Here the
  name is the host's own in the hosts collection the helm pushes, through the rule every host name follows, so a place
  reads as its host does everywhere else in the app: the local host as "Local (this machine)", where the old row said
  "this machine". A finished Podman check reaches the place's words with the helm's next push of the places, which every
  check's end brings; as before, the section asks for a check of every container host once when it is first shown in an
  opening. A place whose host is not registered still reads "Unavailable place".

### Containers

- **The leftovers are not re-read on fleet notices.** The old section re-read **Containers not on this helm** on every
  fleet notice, as it did the hosts. The hosts still follow the fleet here (each host's check from the pushed hosts
  collection, and the places read again when a container host is added, removed, renamed or retargeted), but the
  leftovers are read once per opening, on Refresh and after a done Delete, under this app's rule that what the helm does
  not push is read once per opening. The visible case: after a Delete the helm answered at its 20-second bound, the
  container may be gone a moment later, but its row stays until the user presses Refresh, as the reply's own words say
  ("the list shows whether the container is gone when it is read again"); the old app cleared the row by itself.
- **The hosts show before their checks are asked for.** The old section showed nothing new until it had asked for every
  host's check and read the hosts again; here the first read shows at once, then the checks go out, and each check's
  result replaces the read's state for its host as the push brings it. The read answers without waiting on a host.
- **The Delete question starts on Cancel.** The old inline question left focus on the Delete that opened it. Here it is
  the prompt's inline question, which takes focus, and starts on Cancel since deleting a container cannot be undone
  (SPEC.md, "Opening and closing").
- **A Delete or listing that waits on a host gives up after 20 seconds.** The old routes waited for every Podman
  command; the helm now answers within its host reply bound, so a container host that stops answering shows "did not
  answer within 20 seconds" where the list or the refusal would be, and a Delete carries on and shows in the list's next
  read.

### Credentials

- **A recipe change made while the section stays shown arrives with the next read.** The old section read the catalog
  again whenever the helm's change feed moved. The helm does not push the credential catalog over the UI channel (whose
  "Used by recipes" lines a recipe change moves), so the section reads the catalog each time it is shown, after each of
  its own saves and deletes, and when an editor closes; each read also refreshes an open editor's "Used by recipes" line
  and Delete's availability without touching its unsaved secret. A change an agent or the command line makes while the
  section stays shown waits for the next of those reads, and a Delete it made stale is refused by the helm with its own
  words.
- **Back during a save loses that save's delivery failures.** The save goes on, and the catalog read Back starts lists
  its result, but the hosts a replacement did not reach are reported to the editor that sent it, which Back closed, so
  the overview does not show them. The old editor dropped the reply the same way, after its own Back.
- **Focus moves to a field, not the editor's title.** The old editor put focus on its title (a heading made focusable)
  and Back returned it to the section's heading; headings are not controls here. Opening puts focus on Name for a new
  credential and on the secret for a saved one, and closing puts it on the list's Add button.
- **The editor's answers scroll with it.** The old Delete, Cancel and Save stuck to the bottom of the dialog's body
  (`position: sticky`); GPUI has no sticky positioning, so they follow the editor's content, below the warning.
- **Saves of one credential refuse each other**, with "Not saved: another operation was in progress" under the editor:
  every save and delete of a credential is work on that credential's own operation key, which this section added. The
  Tensorlake API key saves under the preference row's key, as all other helm configuration in Settings does.

### Tensorlake

- **A press on the switch during its save is refused with a line, not ignored by a disabled switch.** The old switch was
  disabled while its save ran. Here it stays available, so focus stays on it, and a press while the save is out (or
  while any other Settings save holds the preference row's key) sends nothing and says so under the switch, as every
  other Settings switch does. It still shows only the stored choice throughout, as before, so nothing on screen changes
  between the press and the helm's answer, which the embedded helm gives in milliseconds.
- **A refused Create SSH key reads the account again, when it is answered and once more the next time Tensorlake is
  shown.** The old section read nothing after a failed create, and its request waited for `ssh-keygen` however long that
  took. Here the helm answers within its 20-second bound and lets a generation still running carry on, saying the key
  shows once it exists, while the account is read once per opening and not pushed; these two reads are what make the key
  show without reopening Settings (`a_refused_create_reads_the_account_now_and_on_the_next_showing`). The first read
  comes right after the 20 seconds, when a generation that was still running has usually not finished, so a user who
  stays on Tensorlake after a 504 sees the key only after showing another section and Tensorlake again, or opening
  Settings again; it does not appear by itself. A refusal's line goes once a read shows the key.
- **Sandboxes not on this helm reads only when its answer can change.** The old list read on every fleet notice while
  healthy, and again after a successful Test while failed. Here it reads when the section first knows a key is saved, on
  Refresh, after a key's Save or Clear in this opening, after every answer to a Delete, and when the pushed hosts list
  gains or loses a Tensorlake host (decided by the migration's coordinator on 2026-11-15), since each read lists the
  whole Tensorlake account. A Test that succeeds after a failed read no longer reads the list; Refresh does.
- **A row's creation time is local, to the second.** "None found · checked" is local hours and minutes, as the old list
  showed it and SPEC.md asks. A row's creation time is the viewer's local time, `YYYY-MM-DD HH:MM:SS` with no zone
  named, where the old list wrote UTC to the minute: the maintainer decided on 2026-11-15 that every time the app shows
  is local, and seconds are the app's one stamp form.
- **Its Delete question starts on Cancel**, as Containers' does, since deleting a sandbox cannot be undone; the old
  inline question left focus where it was. A Delete the helm refuses, or answers at its bound, reads the list again too,
  so the row shows how the cleanup stands rather than its earlier Delete.
- **A done Create SSH key moves focus to Copy.** The old button was replaced by the command and focus was left to the
  browser. Here the focus coordinator's recovery would put it on the next control, the switch that creates sandboxes
  without asking, so it moves to Copy, the control that takes the button's place
  (`create_ssh_key_shows_the_registration_command_and_copy_copies_it`).

### Add host

- **SSH only.** The old dialog opened on a host kind choice, SSH or Tensorlake sandbox. Containers and Tensorlake wait
  for the next milestone (decided by the migration's orchestrator on 2026-11-14), so the dialog offers SSH alone and
  draws no choice.
- **A duplicate destination is refused by the app as well as the helm**, so the refusal does not wait behind other
  hosts' probes holding the helm's planning slots. The old dialog sent the probe and showed the helm's refusal, which
  the helm also gave before contacting anything. The words are the helm's, so the user reads the same sentence either
  way.
- **An empty destination is sent.** The old field was `required`, so the browser kept Add from sending with nothing
  typed and showed its own "fill out this field" bubble; here the helm refuses it (an unusable destination) and its
  words show in the error line.
- **Add is busy, not disabled, while the probe is out**, so it keeps focus; the old button was disabled, which dropped
  focus. Its "Checking…" label is the old one.
- **Add refuses during a launch with a reason, rather than being disabled.** The old app disabled Add host's entry
  points and its submit while its one page-wide token for launches and host registry changes was held. Here each side
  refuses its own action while the other's work is in flight in the operation store (decided by the migration's
  orchestrator on 2026-11-14): Add, pressed while a launcher New or Clone or a session's Restart or Replace is in
  flight, sends nothing and says "Add host: not started, a session launch is in progress; try again when it finishes" in
  its error line, the fields kept (`add_refuses_while_a_session_launch_restart_or_replace_is_in_flight`); other work,
  such as a Stop or a preference save, does not hold it back
  (`add_probes_while_work_that_creates_no_session_is_in_flight`). The setup question's Set up and a row's Re-run refuse
  too: a new host's Add host reopens saying so while the user has not moved on, and otherwise the Hosts panel's line
  names the host ("Set up build-box: not started, …", "Re-run build-box: not started, …"), as Remove's does
  (`hosts/setup/tests.rs` `set_up_during_a_launch_sends_nothing_and_reopens_add_host_saying_why`,
  `without_asking_a_setup_refused_during_a_launch_reopens_add_host`,
  `set_up_of_a_rerun_during_a_launch_says_why_on_the_panel`,
  `re_run_during_a_launch_sends_nothing_and_says_why_on_the_panel`). "Set up, don't ask again" refused that way leaves
  the preference off (`the_permanent_answer_during_a_launch_leaves_the_preference_off`).

### SSH help

- **Add host stays drawn under SSH help.** The old Add host form hid itself while SSH help was open over it (its veil
  stayed). Here it stays on screen under SSH help's veil, dimmed twice: the layer manager has no hidden layer, and
  closing SSH help returns focus to the link only while the link is drawn.
- **No ? menu entry yet.** "SSH needs 2FA?" in the ? menu waits for the area that owns that menu, which calls
  `hosts::ssh_help::open`.
- **No golden.** The reference screens have no SSH help scene.
- **A refusal shows the helm's words alone**, in the app's shared refusal wording; the old line started with the method,
  URL and status. An unusable typed destination shows the helm's 400 body as it sent it, a JSON object with its `error`
  field, which the old line also showed raw after its prefix.

### The setup confirmation

- **A plan that arrives after the user moved on opens no confirmation.** The old app held it and opened it once no other
  dialog was open. Here nothing opens (SPEC.md, "Keyboard focus"); Add host, if still open, says that Farhelm is not
  running there and that Add shows what setting it up changes, and Add probes again.
- **A setup refused after the user moved on shows at the top of the Hosts panel** ("Add host: " and the helm's words)
  instead of reopening Add host; no row exists for a host the helm did not add. The old app always reopened the form,
  once no other dialog was open. The line stays until Add host opens again or the next setup starts, as the old panel's
  add error went when the form opened or the next add was sent. "Moved on" is judged from the action that sent the plan:
  the answer pressed, or Add itself when the plan went out without asking.
- **A refused "Set up, don't ask again" is reported.** Its preference save outlives the question that sent it, so its
  refusal shows on the same panel line ("Set up new hosts without asking: " and the helm's words), with the same
  lifetime, and goes once the preference is on (however it got there) or after a reset; the setup itself goes on either
  way. The old app showed nothing (its preference writer was fire-and-forget); the area's rule that a failed save is
  reported asks for the line. The old writer also queued a write behind a busy preference save, where this one is
  refused while another preference save is in flight (the operation store's one request per key) and the panel line says
  so instead, in the words Settings uses for such a press ("Not saved: another operation was in progress. Try again.").
  Settings, the templates dialog's shortcut writes and a tunnel consent save under the same key; Settings cannot be open
  while the confirmation is, so it takes a save still in flight from just before, or one of the others
  (`a_permanent_answer_refused_for_another_preference_save_says_so_in_the_hosts_panel`).
- **Add is not held back while a setup request is out.** The old app held its page lock until the provision request
  replied, so Add was unavailable meanwhile; here a new Add can start at once. The helm answers that request quickly
  (202 once the run has started), and two adds of different destinations do not conflict.
- **Two screen-reader details are not ported**: the old plan marked its step numbers hidden from assistive technology
  (the list's order said them), and the permanent answer was described by the footer note. The numbers here carry no
  role and no hiding of their own (what assistive technology reads of them was not checked), and the answer's
  description is its hover help: the prompt has no way to point an answer at its note.
- **No header close control.** The old confirmation had the dialog's "×" ("Cancel host setup"); a question of the prompt
  has none. Cancel and Escape do what it did.
- **Copy plan is the compact button** (2 by 8 pixels of padding at 12 pixels), where the old one was 2 by 6 at 11.
- **The plan is not a named keyboard scroll region of its own** ("Host setup plan"): the dialog frame's body scrolls,
  and Tab reaches Copy plan inside it.
- **The question is named by its title**, "Set up host · <destination>", where the old one kept the Add host dialog's
  name.

### The host menu and host settings

- **Focus starts on the first command that can act**: Retry, as in the old menu, whenever the host is idle; while it is
  busy, the first of the others that can act, never a greyed command, since a greyed command takes no highlight and
  starting there showed no mark of where focus was. The arrow keys still reach every greyed command, as the old ones
  did, and it still draws no highlight there.
- **Adopt names its identity on the line under its label**, "Accept identity <id>", in the identity spelling, where the
  old item's label was "Adopt <id>" and its line "Accept the supervisor's identity": a menu item's label is the app's
  own words, and a host value is drawn through the host-text element, which the line takes.
- **Adopt's identity is the one the host reported when the menu opened.** The old item followed the rendered state, and
  the old page drew no new frame between the user's look and the click. GPUI dispatches a key press (Enter) after a
  fresh frame, so an item that followed the host would send an identity the user may not have seen; the open menu keeps
  the one it opened with, and a host that reports another meanwhile is refused by the helm, as the old contract wanted.
- **Retry and Adopt wait for a session launch, each with its own line.** The old app held both behind its one page-wide
  token. Here each refuses during a launch with the Hosts panel's line ("Retry <name>: not started, …", "Adopt <name>:
  not started, …"): Adopt rewrites the registry's entry, as every host registry change does, and Retry drops a connected
  host's connection, which can cut short a launch's create still waiting on that host (decided by the migration's
  settings-and-hosts area on 2026-11-15). The line is the latest one refused and stays until that same remedy starts on
  any host, the other is refused in its place, Add host opens or the UI session resets, where the old page showed no
  line at all. A launch is not held back while a Retry request is in flight, where the old page's token held it: the
  helm answers Retry at once, and the connection drops only after that answer.
- **A host that reports no build reads "Farhelm not connected"** in the header. The old header showed the host's
  recorded install path (`remote_farhelm`) in the build's place.
- **The menu offers Retry, Adopt, Re-run, Update, Settings, Uninstall and Remove so far,** with the local host's Update
  greyed with its reason, or, in the installed desktop app, live as the app's check for a newer release ("Check for a
  newer Farhelm and install it"), whose progress and outcome show beside the version readout, as in the old app. Re-run
  is drawn for a failed Update, and for a failed setup on an SSH host only: a Tensorlake sandbox's setup rerun probes
  through the Tensorlake route and comes with those pieces, and a container's setup belongs to its environment's launch.
  Remove: see "Remove host" below; Uninstall: see "Uninstall" below; Retry and Adopt: see "Retry and Adopt" above. The
  toggle keeps the old hover help. The live Update's hover help is new ("Update: install this helm's Farhelm version on
  this host", and the desktop app's local check's "Update: check for a newer Farhelm release and install it"): the old
  menu items had none, and every menu item here has one.
- **One save per host at a time, and nothing goes unavailable while one runs.** Every setting saves under the host's one
  operation key, so a click on another setting of the same host while a save is in flight sends nothing (the switch or
  choice goes on showing what it showed) and "Not saved: another operation was in progress. Try again." shows where that
  control's refusal would; the user clicks again after the reply, which the embedded helm sends within milliseconds. The
  old dialog instead disabled every control while any host write ran and while a field was open, and put focus back
  afterwards; here only the other field's Edit waits while a field is open. The checkout folder follows the same rule
  (decided by the migration's settings-and-hosts area on 2026-11-14): the old field was disabled with the rest of the
  dialog, but its save is work on the same key, so the key already keeps it and the other saves from crossing, and the
  field keeps its own refusal line, so no other control's outcome can replace it. The field stays editable meanwhile,
  and a Save or Use default the key refuses sends nothing, keeps reading "Save" or "Use default", and shows the same
  line under the field. Covered by `a_press_during_another_save_on_the_host_says_why_it_sent_nothing` and
  `saves_on_the_host_wait_for_each_other_without_locking_the_folder` (`hosts/settings/tests.rs`).
- **Only the destination waits for a session launch.** The old dialog's saves all went through the one page-wide token
  that launches held, so every one of them waited while a session launched. Here only the SSH destination's Save refuses
  while a launcher New or Clone or a session's Restart or Replace is in flight, saying "Destination: not started, a
  session launch is in progress; try again when it finishes" under the field with the draft kept; the alias, the look,
  the checkout folder and the permissions save as usual (decided by the migration's orchestrator on 2026-11-14). Covered
  by `the_destination_waits_for_a_launch_and_the_alias_does_not` (`hosts/settings/tests.rs`).
- **The alias is not checked before it is sent.** The old dialog refused control characters and more than 64 characters
  itself; the helm refuses both, in its own words, which show under the row as any refusal does.
- **A refused save also shows under the host's row** in the Hosts panel, which opens, as every refused work on a host
  does there (see the Hosts panel's deviations).
- **The Connection rows' buttons are the shared compact buttons** (`Button::compact`: 2 by 8 pixels of padding), the
  closest look the component layer has to the old rule, which drew every button in a settings row with 2 by 6 pixels, no
  fill and no edge (an edge only under the pointer). So Edit fills under the pointer instead of showing an edge, and the
  open field's Save and Cancel keep the primary and neutral fills the old rule took away. The rows stand a few pixels
  further apart than in the reference.
- **The header is about 10 pixels taller** than the reference's, as Settings' and Rename's are: the frame lays out the
  title on GPUI's default line height.

### Remove host

- **A refusal inside the question reads "Farhelm refused this action:" and the helm's words, in amber, under the body.**
  The old dialog stayed open, every answer disabled, through its request, as the question does now, and on a refusal
  turned its summary into a red "Farhelm refused this action" and drew the helm's words below the explanation in the
  body's ink at 16 pixels. A question's summary is frozen as it opened, so those words lead the reason line, which the
  prompt draws in its amber (`--warn-bright`) at the dialog frame's 12 pixels. Focus moves to Cancel after the refusal,
  as the old dialog's did, and the row shows nothing of it.
- **A Remove refused before anything is sent, answered on the question, says why inside it.** The question stays up with
  the line the Hosts panel would show ("Remove <name>: ..."), focus on Cancel, as for the helm's refusal. The old app
  ignored such a press (its page lock); see the panel-line item below for the menu's Remove.
- Interim: **with "Remove hosts without asking" on, a refusal shows under the row.** The old app opened its short host
  refusal dialog ("Remove host · <name>", Close); Update's refusals brought that dialog (`hosts::refusal`), and Remove
  does not open it yet.
- **The header's phase has no colored dot.** The old header drew the host's phase with its dot in the phase's color,
  then the build. A question of the prompt has a one-line text summary, so the phase words and build are drawn in the
  summary's quiet ink ("Connected · Farhelm 0.0.0-unreleased"), frozen as the question opened. Drawing the dot needs a
  header option on the prompt, which is not requested yet.
- **No header close control.** The old dialog had "×" ("Cancel host removal"); a question of the prompt has none. Cancel
  and Escape do what it did.
- **After a removal, focus recovers as from any control that goes** (SPEC.md, "Where focus rests": home, the selected
  session's terminal), where the old app moved it to the rail's Hosts control. The question closes with its host, and
  the row's menu toggle it would return focus to goes with the row.
- **A Remove from the menu refused before anything is sent says why at the top of the Hosts panel**, naming the host:
  while the host's key has other work in flight ("Not started: another operation was in progress. Try again.", the
  operation store's shared reason after Remove's own lead-in), and while a session launch is in flight. The old app
  ignored such a press without a word (its page lock). A launch is the launcher's New or Clone, or a session's Restart
  or Replace, in flight.
- **"Remove, don't ask again" stores the preference only once its removal is sent.** The old app stored it first, so a
  removal its page lock then held back still turned the preference on; here a removal refused before anything is sent (a
  session launch in flight, the host's key busy) leaves the preference as it was. A removal the helm refuses after it
  was sent leaves the preference on, as the old app did; the question stays up offering both answers again.
- **A refused "Remove, don't ask again" is reported** on the same panel line ("Remove hosts without asking: " and the
  helm's words, or "Not saved: another operation was in progress. Try again." when another preference save was in
  flight); the old app showed nothing.
- **The footer note shows only beside the permanent answer.** With Remove unavailable the note naming "don't ask again"
  is left out with that answer; the old dialog always drew it, beside its disabled answers.
- **The header is about 10 pixels taller** than the reference's, as the host settings dialog's is: the frame lays out
  the title on GPUI's default line height.
- **The body is not a named scroll region of its own** ("Host removal details"): the dialog frame's body scrolls, and it
  holds no control to reach.
- **Tensorlake and container hosts offer no Remove.** Their removal deletes the machine, which the old menu offered as
  Delete sandbox (or Delete container) with a question of its own; that comes with the containers and Tensorlake work.
- **With Remove unavailable, "Remove, don't ask again" is left out** rather than drawn unavailable beside it, as the old
  dialog drew both: the prompt has no way to disable an extra answer. Remove is unavailable only when the host turned
  busy between the menu and the question.

### Update

- **No refusal queue.** The old app held refusals in a map and showed one dialog at a time, the next after the last
  closed. Here each refusal that arrives before the user moves on opens its own dialog over the last, and closing the
  top one returns focus to the one below (the goal's "No queue for results that arrive later"); one that arrives after
  the user moved on opens nothing and shows under its row.
- **The refusal dialog's Remove follows the helm's marker alone.** As in the old dialog, Remove shows after Close for a
  refusal the helm marks "remove instead" and starts Remove host's flow; the helm sets that marker only on an uninstall
  plan's refusals, so an Update's refusal shows Close alone (an uninstall's may show Remove). The upgrade floor's manual
  remedy names removing the host from Farhelm in the helm's words, and the menu's Remove does it.
- **No separate uncertain outcome.** The old app warned under the row when it could not tell whether a submitted Update
  started or how it ended (a lost or malformed reply, a host retargeted or a progress read failing mid-run). Here the
  confirming request is answered (accepted, with the run already pushed, or refused) or lost to a reset, whose generic
  notice is the report, so only a failed run or a refusal opens the row.
- **Refused before sending, in new words.** While a session launch is in flight the Update is not sent and the Hosts
  panel's line says so once per press, naming the host ("Update <name>: not started, a session launch is in progress;
  try again when it finishes", or "Re-run <name>: …" for a failed Update's Re-run, as a setup's Re-run reads), or
  "Update all" for the heading's press (the migration's rule of 2026-11-14; the old app refused through its page lock,
  with its generic busy words). A press that meets other work on the host's key sends nothing and says nothing: no
  control offers Update on a busy host, so only a press racing a frame gets there.
- The inline `↑ Update` shows on every remote host kind whose Update is available, SSH, Tensorlake and container, as the
  old app drew it (its `remote_update_available` admitted all three). SPEC.md ("Session list") named an SSH host until
  the migration's orchestrator decided on 2026-11-14 to keep the old behaviour and reword SPEC.md to a remote host, so
  this is not a deviation.
- **The inline button's urgency is not described separately.** The old button pointed assistive technology at a hidden
  copy of its hover; here the hover help is the button's own and its name is "Update <name>".

### Uninstall

- **No header close control.** The old confirmation had the dialog's "×" ("Cancel Farhelm uninstall"); a question of the
  prompt has none. Cancel and Escape do what it did.
- **The plan is not a named keyboard scroll region of its own** ("Host uninstall plan"): the dialog frame's body
  scrolls, and Tab reaches Copy plan inside it, as in the setup confirmation.
- **The kept data note's wash is `--warn` at 12%**, the palette's warning chip fill, where the old note's was 10%.
- **Uninstall stays in the menu, unavailable, while the host is busy.** The old menu left it out while a run or a plan
  was in progress and drew it disabled while the page's lock was held; here it is drawn unavailable whenever the host
  has a run going on or a request out, as Remove and Settings are.
- **A plan that arrives after the user moved on opens nothing** and says under the row "Farhelm planned this uninstall
  after you moved on. Choose Uninstall again to see what it removes." (new words; SPEC.md, "Keyboard focus"). The old
  app held the confirmation until no other dialog was open (the goal's "No queue for results that arrive later").
- **The removal notice's summary is not a tab stop.** The old summary was focusable so a keyboard user could open its
  hover text; here the full sentence, path included, is the notice's accessible name, and Copy and × are reachable by
  Tab.
- **The notice's Copy and × are the shared compact button and a 20-pixel icon square**, where the old ones were text
  buttons with 1 by 3 pixels of padding, so the line is a few pixels taller than the old one.
- **The notice shows at the top of the Hosts panel only.** As in the old app, the window that confirmed shows it once
  the host has left the list; while Hosts is hidden it waits there.
- **A confirmation's refusal that arrives after another client removed the host takes back the notice** the host's
  leaving had shown, since Farhelm was not removed from it. The old app left that notice up.

### The Hosts panel

- **The running status's hover is the progress card, made only while it shows.** As SPEC.md asks, hovering a row's
  running status opens the card beside the sidebar (the old `HostProgressCard`). The card is made when the tooltip shows
  and dropped when it hides, since it holds the ticker's clock and its running step's pulse; the old page kept a hidden
  copy per row. Its words, "Progress of the run on <host>", are the status's accessible description and what the tooltip
  reports; the status itself is still named for its count and current step ("Setting up: 1 of 9 steps done, now Install
  the farhelm binary"). While only a run's requests are out ("Setting up…"), the hover is those words, as the old label
  had no card then.
- **Known gap: a tall card can cover the Linux window's title bar.** Beside placement keeps help inside the window's
  viewport, but on Linux the window draws its own 20-pixel shadow margin and title bar inside that viewport, so a card
  taller than the window's usable height (a setup's nine steps with explanations and a connection detail) is placed over
  them, and with a compositor it can hang outside the visible frame. The card is the first help tall enough to meet
  this. Keeping placement inside the window's client area is a change to the layer manager, requested from its owner;
  this waits for it.
- **The card's connection detail sets host values as the row's details do.** Builds and identities are monospace and an
  error is in the interface face, where the old card set every host value monospace.
- **A shrunk card shows the steps around its current one in proportion.** In a short window the card keeps its current
  step whole and clips the steps before and after it in proportion to how much of each there is, where the old card
  centered the current step in its list.
- **The elapsed time counts from when this window first saw the run running**, as the old page counted from when it
  first saw it: the helm's progress has no timestamps. A reset starts it over, as a reload did. The goldens read "0:00",
  since their clock does not move; the reference caught "0:06".
- **A step's explanation is not drawn under it in the row's step list.** The old panel drew only the Tensorlake snapshot
  preparation's, which comes with the Tensorlake pieces. The progress card draws every step's, as the old card did.
- **A rerun's refusals show under the row.** A refused probe, a "manual" answer, a reply that cannot be read and a
  refused submission show under the host's row, like any refused work on the host, where the old app opened its
  host-keyed refusal dialog for a refused or manual probe; that dialog comes with Update. A plan that arrives after the
  user moved on opens nothing and says so under the row: "Farhelm is not running on this host. Choose Re-run again to
  see what setting it up changes." (new words, after Add host's for the same case).
- **Collapse is the shared compact button** (2 by 8 pixels of padding at 12 pixels), where the old one was 1 by 6 at 11
  pixels.
- The inline status is upright like the pending phase words (the reference's is italic, inheriting the connecting
  phase's style). The row's actions toggle is the old stylesheet's: always visible, in the quiet ink (the old
  `.host-row-menu` had no `opacity: 0` rule at rest, whatever its comments said), not a deviation.
- The helm's own machine with no supervisor opens its own row, whatever the Details switch says, for as long as no
  supervisor runs there, and shows the helm's dial failure as its evidence and "Quit Farhelm and open it again; it
  starts this machine's supervisor when it opens" as its remedy, so it never reads as a phantom unreachable host
  (SPEC.md, "Topology"); the unavailable-session card shows the same remedy, with the dial failure as its diagnostics.
  The old app showed the helm's dial failure as the remedy whenever there was one, and its own words only when there was
  none. Its evidence line was always the fixed "No supervisor is running on this machine"; that sentence now shows only
  when the helm gives no detail, and otherwise the evidence line is the helm's dial failure. The remedy is now always
  the app's sentence alone because the helm's detail is dial advice for a hand-started helm (start
  `farhelm supervisor run` with the right state directory), which is wrong for the desktop app's own machine: the app
  starts that supervisor itself whenever it opens and none answers (decided by the migration's orchestrator on
  2026-11-16). The old app also probed the machine once per down transition and showed the probe's `farhelm helm setup`
  handoff, turning the global Details switch on to do it. The probe and its handoff are not ported, and none is planned
  (decided by the migration's settings-and-hosts area on 2026-11-14); the switch is left to the user (SPEC.md: no run
  writes it). The old menu's "Set up automatically" is not ported either: the panel never installs a supervisor on the
  helm's own machine (SPEC.md, "Topology"; settled for the port by the migration's orchestrator on 2026-11-14).
- A row also opens while the last work on its host was refused, with the helm's words under it, until the next work on
  that host starts; the old app opened it only for provisioning errors, and showed other refusals under the row closed.
  Both kinds of refusal now go through the one operation key a host has.
- Install ids in the details (a connected host's identity, and the mismatch, unverified and duplicate lines) use the
  host text foundation's identity spelling, as the old app's `display_identity` did: spaces, variation selectors and
  non-ASCII look-alikes are written out as `<U+XXXX>`, so two ids the user compares never look alike
  (`install_ids_are_spelled_so_two_ids_never_look_alike` in `hosts/identity/phase/tests.rs`).
- The remedies of a duplicate entry ("... then press Retry"), a retired entry ("Retry to start a fresh connection actor
  ...") and an identity mismatch name the host menu's Retry and Adopt, which the menu offers. The mismatch's remedy says
  adopting "resets all five of this host's permissions to asking", where the old one said only that adopting resets the
  host to asking before YOLO launches (SPEC.md, "Topology": adopting "resets all five permissions to asking"). It still
  says only "fix the destination", without naming where: the host settings dialog edits an SSH host's destination, and a
  sandbox's or a container's is not the user's to change. Interim: a failed delete's "Retry to delete it again" names
  the environment's own Retry, which comes with Delete sandbox; the menu offers no Retry while an environment is being
  deleted.
- "Connecting" and the other pending phases are drawn upright and the port asks for no italics: the GUI bundles no
  italic Inter face, and the old app drew them in italics.
- The identity mark's "Local" or "Remote" is its accessible name rather than a visually hidden word after it, with the
  same hover help.
- The old app worded its phase status's accessible name only for connected hosts and hosts offering an inline update;
  the port names every status by its phase words. As in the old app, only Too new carries a description (both versions
  and the remedy).
