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
[Sidebar and session view](sidebar-and-session-view.md).

| Old spec                             | Tests | Covers                                                                           | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------ | ----- | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/settings.spec.ts`             | 27    | settings focus stays on its painted fill and descriptions stay with their labels | partly (below)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/credentials-settings.spec.ts` | 3     | credentials keep fixed names, replace write-only values and discard login drafts | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/credential-delivery.spec.ts`  | 2     | saved credentials report delivery failures without retaining the secret draft    | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/ntfy-settings.spec.ts`        | 3     | ntfy settings explicitly save, refuse, preserve and clear a token                | replaced (below)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `tests/tensorlake-settings.spec.ts`  | 6     | Tensorlake settings save a write-only key, test the saved key and clear it       | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/tensorlake-recipes.spec.ts`   | 10    | Settings keeps recipe answers sticky and isolated to Tensorlake in ${palette}    | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/tensorlake-hosts.spec.ts`     | 8     | Add host pins inline recipe answers and focuses Name in ${palette}               | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/tensorlake-lifecycle.spec.ts` | 21    | wake-to-create leaves ${elsewhere} alone (${mode})                               | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/tensorlake-tunnels.spec.ts`   | 2     | tunnel failures make a connected host amber and show each mapping                | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/hosts-panel.spec.ts`          | 2     | Hosts swaps ordinary content and preserves filters, sort, selection and details  | partly: crates/gui/farhelm-gui/src/hosts/tests.rs `the_heading_counts_the_hosts_the_helm_lists` (the region named "Host management"), `details_open_and_close_every_row` (the Details choice lives in the one panel the sidebar keeps, so it survives the swap). The swap, from the sidebar's Hosts control on the host rail's foot: crates/gui/farhelm-gui/src/session_list/host_rail/tests.rs `hosts_swaps_the_list_for_the_panel_and_back_keeping_focus` (the panel in place of the list, Hosts pressed and keeping focus, nothing sent, so the host choice, sort and selection stay, the selection also checked, and Details kept across the swap), `choosing_all_or_a_host_returns_from_the_panel_to_sessions`; scripts/gui-e2e-smoke.py `test_39_the_rail_foot_toggles_hosts_and_opens_settings`; the `hosts-shown-with-a-host-chosen` golden (the panel in place of the list, Hosts pressed, the chosen avatar in the quieter look). Hosts also reports itself expanded while the panel shows, which no test reads back, since GPUI's driver record has no expanded field. Waiting: a quick-switcher pick or New handoff returning to Sessions (the sidebar's); the second case, the header's Clone from Hosts, is the session header's                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/host-updates.spec.ts`         | 5     | host progress cards agree across surfaces and follow connection detail           | not yet ported (its progress card, Update all and Update cases, named above; the attention-list cases are in Sidebar and session view)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `tests/host-checkout-folder.spec.ts` | 1     | host checkout folder saves, inherits, clears and reaches launch preview          | partly (below)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/provisioning.spec.ts`         | 73    | rail ${action} modal stays visible over Sessions and returns to its avatar       | partly (the Hosts panel's rows, a host's menu with Settings, and the host settings dialog; see "The host menu and host settings" below): crates/gui/farhelm-gui/src/hosts/offers/tests.rs `the_local_hosts_update_is_greyed_and_points_at_the_installer` and crates/gui/farhelm-gui/src/hosts/menu/tests.rs `the_local_hosts_greyed_update_says_why_and_sends_nothing` ("the local row's update is greyed out, points at the installer, and sends nothing"), `work_in_flight_withdraws_update_and_rerun_and_holds_settings_and_remove` and menu/tests.rs `settings_waits_for_the_hosts_own_work_in_flight` ("Update planning keeps only its own Settings unavailable"), `a_running_run_holds_the_host_until_it_ends` ("accepted ADD ... while running": no Update or Re-run and Settings unavailable during the run, Update after it), `a_too_new_host_is_not_offered_update`, `a_suspended_host_is_not_offered_update_or_rerun`, `a_failed_run_is_offered_again_as_what_it_was`, `a_failed_update_on_the_local_host_is_offered_again_greyed`, `the_local_host_without_a_supervisor_offers_no_install`, `each_kind_decides_its_own_offers`; crates/gui/farhelm-gui/src/hosts/tests.rs `details_open_and_close_every_row` and `a_failed_run_opens_only_its_own_row` ("global details alone control disclosure", without the update progress), `the_local_host_without_a_supervisor_opens_its_row_to_say_what_to_do` ("local setup answers an absent supervisor", without the probe and its handoff, which are not ported), `a_refusal_shows_under_its_row_and_opens_it`; crates/gui/farhelm-gui/src/hosts/identity/phase/tests.rs (phase words, Too new hover, details); crates/gui/farhelm-gui/src/hosts/settings/tests.rs `the_dialog_closes_when_the_registry_no_longer_lists_its_host` (the settings dialog closing when its host disappears); and Add host's dialog, probe and setup confirmation (below). Waiting: the menu's other commands, setup progress and rerun, Update and Update all, Remove, the host settings' Checkouts section and the rail's entry points |
| `tests/ssh-help.spec.ts`             | 4     | ssh help displays and copies both registered-host commands                       | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/workspace-trust.spec.ts`      | 3     | managed-checkout trust switches persist independently of Claude's legacy route   | partly (below)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `tests/appearance.spec.ts`           | 4     | ${initial.name} before application mount                                         | partly (below)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

## `tests/settings.spec.ts`, partly

The dialog's shell and its Sessions, Appearance, Hosts, Terminal, Checkouts, Agent trust and Notifications sections are
ported; the cases about the other sections (Recipes, Tensorlake, Containers, Credentials and App) wait for them.
Notifications' own cases are in `tests/ntfy-settings.spec.ts`, below; of this spec's, the navigation case's
Notifications half (the "ntfy alerts" group under its 16-pixel sub-heading, below the 18-pixel section heading) is the
golden `settings-notifications`, and its Sounds half waits for the Sounds sub-heading (Deviations). Appearance's own
cases are in `tests/appearance.spec.ts`, below. Agent trust's are split between `tests/workspace-trust.spec.ts` (below)
and this spec, whose hint-switch cases are mapped in the Agent trust bullet.

NOTE: Terminal shows only the link choice for now. Its **File types to open with Cmd-click** field governs opening
downloaded files and comes with file downloads, later in the migration. Its tunnel consent switch waits for the tunnel
feature's consent question ("Allow tunnels from remote sessions?"), which the terminal area builds with tunnels: turning
the switch on without that warning would grant consent unasked, which SPEC.md rules out ("consent is off until they
accept the warning"). The migration's settings-and-hosts area decided on 2026-11-14 to leave the switch out until the
question exists. The old desktop switch, for when it is ported: turning it on opened the warning and saved consent only
on Allow; turning it off saved at once, and the helm then closed every open tunnel.

NOTE: Nothing in the new app follows the link choice yet: the GPUI terminal reads neither it nor tunnel consent today.
The new app's terminals follow them only once the terminal area's link and tunnel pieces land; until then the helm alone
acts on them (it closes the open tunnels when consent is turned off).

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
  descriptions are the radio group's own, which its tests in `components/tests.rs` cover.
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
- The look: the goldens `settings-sessions`, `settings-appearance`, `settings-hosts`, `settings-terminal`,
  `settings-checkouts`, `settings-agent-trust` and `settings-notifications` (`crates/gui/farhelm-gui/src/goldens.rs`).

Not ported here, and why: the persistence across a second client and reload belongs to the helm's preference route and
the UI channel's pushes, which their own tests cover; "settings gap keeps focus ..." is the layer manager's empty-space
rule (`keeps_focus_on_empty_click` is on for Settings), with no Settings-specific test yet; the text selection half of
it does not apply, since GPUI labels are not selectable; "settings reverses a permanent removal answer without
reloading" needs the host removal question, which comes with the port of Remove host; the narrow list path is not built
(Deviations); "settings ignores a superseded link-choice failure" has no newer choice to supersede, since a second click
while any Settings save is in flight is refused (Deviations).

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
  field, Save's accent fill, "No token saved"): the golden `settings-notifications`.
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

## `tests/provisioning.spec.ts`, Add host's dialog, probe and setup

The Add host dialog, its probe, the setup confirmation and the setup's submission are ported; setup progress, failure
and rerun are not yet.

The app opens the dialog from the host rail's Add host, which calls `hosts::add::open` with itself as the opener; that
button's own tests, and the end-to-end smoke step that opens the dialog and closes it with Escape, are in the sidebar's
parity file (`docs/gpui/parity/sidebar-and-session-view.md`, the `tests/filters.spec.ts` row). The Hosts heading's add
button, the dialog's other entry point, is not drawn yet.

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
- The look: the golden `host-add-dialog` (`crates/gui/farhelm-gui/src/goldens.rs`).

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
accepted reply's body any more (the run shows in the pushed host progress, which part 3 draws); "accepted ADD ...
follows the run", with setup progress.

A reply that arrives after the user moved on (a key or click since Add) still shows in the dialog if it is open, as in
the old app: it opens nothing and moves no focus, which is all SPEC.md's "Keyboard focus" withholds from a late result.
A dialog the user closed shows nothing, and a refusal has no row to show on, since no host exists for it.

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
  radio group's own keyboard, which its tests in `components/tests.rs` cover. The terminal staying dark needs no test of
  its own here: the theme holds one terminal palette for both (`theme::TERMINAL`).
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

The host settings dialog is not built yet; it is to draw the shared folder field (`settings::CheckoutField::folder` with
`settings::FolderScope::Host`) in its Checkouts section. The field's own part of the spec is ported:
`a_host_folder_shows_the_default_as_placeholder_and_use_default_clears_it` in
`crates/gui/farhelm-gui/src/settings/tests.rs` reads the host's route, shows the override, offers Use default, sends
`{"root": null}` under the host's own operation key, reads back, and shows "Default: <folder>" as the empty field's
placeholder; it also counts the field's `Saved` event (the old `on_saved`, the launcher's hook): one for the done save,
none for a refused one. A save, a refusal kept for correction and an empty field clearing the override are the same code
as the default folder's, tested in the `tests/settings.spec.ts` section above. What waits for the host settings dialog:
opening the field from a host's menu, "No default set" in a dialog, and the launch preview reaching the host's folder,
which the launcher's preview owns.

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

## The host menu and host settings

A Hosts panel row's ⋯ opens the host's actions menu beside it, and its Settings opens the host settings dialog. The
entry points are `hosts::open_menu` (at an anchor, with the control the settings dialog gives focus back to; the host
rail's avatar menu will call it) and `hosts::open_settings` (with the control its closing returns focus to).

Replacing tests, in `crates/gui/farhelm-gui/src/hosts/menu/tests.rs` and
`crates/gui/farhelm-gui/src/hosts/settings/tests.rs`:

- The menu: `a_remote_hosts_toggle_opens_a_menu_offering_settings` (its name, header and first command, and Escape back
  to the toggle), `the_local_hosts_greyed_update_says_why_and_sends_nothing`,
  `settings_waits_for_the_hosts_own_work_in_flight`, `a_second_press_on_the_toggle_closes_its_menu`,
  `escape_on_the_menu_returns_focus_to_its_toggle_under_the_macos_click_rule`. What it offers is `offers::offers`, whose
  tests are listed in the `tests/provisioning.spec.ts` row.
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
- The look: the goldens `host-menu`, `host-settings-dialog` and `host-settings-dialog-end`
  (`crates/gui/farhelm-gui/src/goldens.rs`).

Not ported here: the Checkouts section and `tests/host-checkout-folder.spec.ts`, which come with the next piece, the
dialog drawing the shared checkout-folder field from Settings → Checkouts; `modal-layout.spec.ts`'s host settings cases,
whose margins and scrolling body are the shared dialog frame's (its own tests and goldens), and whose busy-body Tab stop
has no counterpart, since no control here goes unavailable while a save runs.

## Deviations

Where the port does not match the old app, and why. Each stays until the work named in it lands.

### The Settings dialog

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
  and that other open clients pick it up right away, and that System follows each device's own appearance.
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
- **A trust switch's save is refused while another Settings save is in flight**, like a preference switch's (below). The
  operation store admits one request per key, and helm configuration (where trust lives) has no key of its own, so trust
  saves go under the preference row's key. This area accepted that on 2026-11-14, under its goal's instruction not to
  request a key for helm configuration before it is needed: the embedded helm answers these saves in milliseconds, so
  they rarely meet. The old trust switches each had their own request and were unavailable only while their own save
  ran.
- **Terminal has no tunnel consent switch yet.** It waits for the terminal area's consent question (see the NOTE under
  `tests/settings.spec.ts` above).
- **No one-pane layout at 620 pixels and below.** The old dialog showed either the section list or the section below
  that width, with "‹ Settings" to go back. The window's minimum width (720 pixels) is above it, so the layout is
  unreachable and is not built.
- **A section switch leaves focus where it was.** The old app moved focus to the shown section's heading, so a keyboard
  user went on from the top of the section. The focus coordinator moves focus to a control only while no layer is open,
  and the heading is not a control, so focus stays on the section button (Linux), or where it was (macOS, where the
  click does not focus the button; a control in the section that was hidden hands focus to the closest one left).
- **A click or arrow press while another preference save is in flight does nothing.** The old app queued every switch's
  newest click per field and sent it after the earlier one ("the latest click wins"). Here the operation store admits
  one request on the preference row at a time, so the second click is refused; a second click on the same switch leaves
  it showing the value in flight, and one on another switch leaves that switch as it was; the user clicks again after
  the reply, which the embedded helm sends within milliseconds. In a radio group (Appearance, Terminal's link choice)
  the arrows move focus and the choice together, so a second arrow press while the first choice's save is in flight
  moves focus to the next option while the choice stays where the first press put it. Accepted by the migration's
  orchestrator on 2026-11-14, since preference saves are helm-local and take milliseconds; revisit only if a user meets
  it.
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
  built so far, Checkouts scrolls even at the dialog's full height (next item), and the list then scrolls with it.
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

- **No Sounds sub-heading.** The old section opened with Sounds and its three switches (session waiting, approval
  request, turn finished) above ntfy alerts. Status sounds belong to the other dialogs area, which adds them to this
  section when it builds them; until then the section holds ntfy alerts alone.
- **Priority is a select.** The old form chose the priority with five segments (Min, Low, Default, High, Urgent); this
  one has a select over the same five, as the migration's orchestrator decided for this port on 2026-11-14. It edits the
  form like the segments did and saves nothing on its own.
- **A refusal stays until the next save starts.** The old form cleared its outcome line on any edit; here the helm's
  words stay under the form, as every Settings failure line does, until the next Save.
- **A Send test after a refused Save shows its outcome below the save's refusal.** The old form had one outcome line,
  which a Send test emptied before showing its own. Here the refused save's words are the draft's and stay until the
  next save, as above, so the test's "Test alert sent" or its refusal shows beneath them; the two lines are about
  different requests, and the test used the settings the helm stores, not the refused ones.
- **Save is refused while another Settings save is in flight**, as Agent trust's switches are (above): ntfy alerts are
  helm configuration with no operation key of their own, so they save under the preference row's key.
- Not visible, recorded for review: the form is one draft over all its fields, saved in one request, rather than one
  `DraftInput` per text field, because `DraftInput` saves only its own field. The migration's settings-and-hosts area
  decided on 2026-11-14 to keep this exception until a second whole-form save makes it worth building into the drafts
  module.

### Add host

- **SSH only.** The old dialog opened on a host kind choice, SSH or Tensorlake sandbox. Containers and Tensorlake wait
  for the next milestone (decided by the migration's orchestrator on 2026-11-14), so the dialog offers SSH alone and
  draws no choice.
- **No "Start its connections yourself first." link.** The old dialog's line asking whether the host wants a second
  factor on every login, with its link to SSH help for the typed destination, waits for SSH help.
- **A duplicate destination is refused by the app as well as the helm**, so the refusal does not wait behind other
  hosts' probes holding the helm's planning slots. The old dialog sent the probe and showed the helm's refusal, which
  the helm also gave before contacting anything. The words are the helm's, so the user reads the same sentence either
  way.
- **An empty destination is sent.** The old field was `required`, so the browser kept Add from sending with nothing
  typed and showed its own "fill out this field" bubble; here the helm refuses it (an unusable destination) and its
  words show in the error line.
- **Add is busy, not disabled, while the probe is out**, so it keeps focus; the old button was disabled, which dropped
  focus. Its "Checking…" label is the old one.
- Interim: **Add is not refused while a launch is in flight.** The old app refused it. Each side is to refuse its own
  action, with a reason, while the other's work is in flight in the operation store (decided by the migration's
  orchestrator on 2026-11-14); this waits until the launcher starts its launches in the operation store, after which Add
  refuses with a reason.

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
  dropped while another preference save is in flight (the operation store's one request per key). Only Settings saves
  preferences, and it cannot be open while the confirmation is, so it would take a save still in flight from just
  before.
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

- **The menu offers Settings alone so far,** with the local host's greyed Update and its reason before it. The rerun,
  Update and Remove arrive with their pieces, at their old places; Retry and Adopt are not offered in this milestone
  (decided by the migration's settings-and-hosts area on 2026-11-14), and the toggle keeps the old hover help, which
  names them.
- **No Checkouts section yet.** It sits between Appearance and Permissions once the shared checkout-folder field exists;
  the footer note already names it ("Checkout folder saves with Save; ...").
- **One save per host at a time, and nothing goes unavailable while one runs.** Every setting saves under the host's one
  operation key, so a click on another setting of the same host while a save is in flight changes nothing (the switch or
  choice goes on showing what it showed); the user clicks again after the reply, which the embedded helm sends within
  milliseconds. The old dialog instead disabled every control while any host write ran and while a field was open, and
  put focus back afterwards; here only the other field's Edit waits while a field is open.
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

### The Hosts panel

- Not drawn yet, each until the piece that gives it something to do: the Hosts heading's Update all and add button (the
  heading keeps its old 32-pixel height meanwhile, and Details sits at the heading's end rather than between the count
  and them), the inline `↑ Update` button, and setup and Update progress (the inline status, the step list, the row
  opening once for a run and its Collapse). The row's actions toggle is the old stylesheet's: always visible, in the
  quiet ink (the old `.host-row-menu` had no `opacity: 0` rule at rest, whatever its comments said), not a deviation.
- The helm's own machine with no supervisor opens its own row, whatever the Details switch says, for as long as no
  supervisor runs there, and shows "No supervisor is running on this machine" with the helm's remedy from its pushed
  state ("The helm reports: ..." and the `farhelm supervisor run` command its dial failure names), so it never reads as
  a phantom unreachable host (SPEC.md, "Topology"). The old app instead probed the machine once per down transition and
  showed the probe's `farhelm helm setup` handoff, turning the global Details switch on to do it. The probe and its
  handoff are not ported, and none is planned (decided by the migration's settings-and-hosts area on 2026-11-14); the
  switch is left to the user (SPEC.md: no run writes it). The old menu's "Set up automatically" is not ported either:
  the panel never installs a supervisor on the helm's own machine (SPEC.md, "Topology"; settled for the port by the
  migration's orchestrator on 2026-11-14).
- A row also opens while the last work on its host was refused, with the helm's words under it, until the next work on
  that host starts; the old app opened it only for provisioning errors, and showed other refusals under the row closed.
  Both kinds of refusal now go through the one operation key a host has.
- Detail lines mix the app's words with host values (builds, identities, destinations, errors), each part its own text
  element so a host value cannot reorder the words around it; a part that does not fit starts on a new line and wraps
  inside itself, where the old page broke a long identity or error mid-run after the words before it. Rows with long
  values are a line or two taller than in the reference screens.
- Identities in the details are escaped like any host text. The old app's identity comparison spelling (spaces and
  variation selectors made visible) for the mismatch and duplicate details waits for that presentation in the host text
  foundation.
- Interim: the remedies of a duplicate entry ("... then press Retry"), a retired entry ("Retry to start a fresh
  connection actor ..."), a failed delete ("Retry to delete it again") and an identity mismatch ("Adopt the identity the
  host reports ...") keep the old words, which name the host menu's Retry and Adopt. M2 offers neither (decided by the
  migration's settings-and-hosts area on 2026-11-14). Adopt and the duplicate's Retry arrive with the identity remedies,
  the failed delete's Retry with Delete sandbox; no piece names the retired entry's Retry yet.
- "Connecting" and the other pending phases are drawn upright and the port asks for no italics: the GUI bundles no
  italic Inter face, and the old app drew them in italics.
- The identity mark's "Local" or "Remote" is its accessible name rather than a visually hidden word after it, with the
  same hover help.
- The old app worded its phase status's accessible name only for connected hosts and hosts offering an inline update;
  the port names every status by its phase words. As in the old app, only Too new carries a description (both versions
  and the remedy).
