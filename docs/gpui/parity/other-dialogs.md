# Parity: Other dialogs

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Feedback, Welcome card, Announcements, Partial deletion, Status (session notifications and the rail's notices
bell), and Desktop window chrome's Keyboard shortcuts paragraph.

| Old spec                           | Tests | Covers                                                                       | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ---------------------------------- | ----- | ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/trash-dialog.spec.ts`       | 14    | trash list stays keyboard-scrollable while every list action is disabled     | partly, below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `tests/feedback.spec.ts`           | 12    | feedback uses dark fields and pinned chrome at normal and short heights      | partly: the Help menu cases (its place under the gear, its items and their order, Escape and a second press closing it, focus back on the `?` button, the three links) are replaced by `help_menu::tests` and `gui-e2e-smoke.py`'s `test_45`; the WebKit stacking-context case is superseded by the layer manager, which draws every overlay above the sidebar; the dialog's cases (the Discord link keeping the draft, sending exactly what is shown, thanks and closing, remembering, rewriting and clearing the contact, a failure keeping the text, a close queued behind Send waiting, blank and over-long messages) are replaced by `feedback::tests`, its look by the `feedback-dialog` and `feedback-dialog-reuse` goldens, and opening it from the menu, typing and Cancel by `gui-e2e-smoke.py`'s `test_48`. Missing: the send end to end in the running app, because nothing carries a stand-in endpoint into the app's embedded helm and a Send there would post real feedback, so no bridge run presses Send (the helm's forwarding is covered by its own tests against a stand-in); and the short and narrow window geometry checks (the dialog frame's own tests cover a scrolling body with pinned answers). `modal-layout.spec.ts`'s sent-feedback case is replaced by `feedback::tests::after_a_send_the_thanks_has_a_body_stop_of_its_own` (the text-only body's own stop, "Feedback sent"; the shared stop's keys are tested in `components::tests::dialog`) |
| `tests/welcome.spec.ts`            | 7     | welcome dismissal by ${dismissal} is shared and survives reload              | ported, below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `tests/announcements.spec.ts`      | 10    | announcement dismissal by ${dismissal} survives reload                       | ported in process, below; no end-to-end leg, since the running app's helm has no announcement to show (its compiled release table is empty) and the bridge's fake fleet cannot queue one                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `tests/keyboard-shortcuts.spec.ts` | 5     | the ${mac ? "Mac" : "non-Mac"} overview chord owns terminal input and labels | ported, below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `tests/sounds.spec.ts`             | 6     | device sounds                                                                | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `tests/notifications.spec.ts`      | 12    | session notifications                                                        | ported: the bell's presence, loudness, name, help and place before the age, the list beside the bell without opening the session, newest first with New and Resolved and ages, Escape, the bell again, a press elsewhere and the row going each marking what the list showed, nothing sent when nothing unread was shown, a refused mark sent again on the next close, Clear all and the bell's return, and the docs-link rule are replaced by `notices_bell::tests` and the `session-bell-list` golden; the cleanup-helper case tested the old suite itself; the two build-mismatch cases and the older-failure race are superseded by the store (every change is pushed before its reply, and no mark is cached locally); the resize and moved-row dismissals are deviations (below)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `tests/rail-notices.spec.ts`       | 4     | rail notices quiet, alerting and resolved in ${palette}                      | ported in process, below; the sidebar parity file lists it too                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

## `tests/welcome.spec.ts` and `tests/announcements.spec.ts`

The Welcome card and the owner of the window-start dialogs are `welcome` (`welcome::open` from the Help menu,
`welcome::StartDialogs` at a window start or reset, which the main view keeps); the Announcements dialog, its history
and the automatic batch are `announcements` (`open_history`, `open_pending`). Replacing tests, in
`crates/gui/farhelm-gui/src/welcome/tests.rs` and `announcements/tests.rs`:

- "welcome dismissal by ${dismissal} is shared and survives reload":
  `every_close_of_the_first_card_marks_it_seen_once_and_returns_home` (each of Got it, the × and Escape sends one sparse
  `PUT /api/preferences` of `welcome_seen` and leaves focus home; a reset decides again). The write being shared by
  every client and surviving a reload is the helm's preference store, tested in the helm
  (`welcome_seen_sparse_patch_and_type_contract`); the GPUI app keeps no copy of it.
- "welcome stays closed when preferences are ${refused | timed out}":
  `a_seen_helm_or_a_failed_preference_read_opens_no_card_and_each_reset_decides_again`. The preferences arrive in the UI
  channel's first snapshot, which never waits on a host, so a failed or timed-out read is a snapshot without the
  preferences entry.
- "welcome waits for initial sign-in before deciding" and "announcements wait for sign-in": superseded (the desktop app
  never signs in).
- "a refused seen write still dismisses the welcome": `a_failed_seen_write_still_closes_the_card`.
- "welcome links, modal isolation and short-window dismissal":
  `read_get_started_opens_the_guide_and_leaves_the_card_open` and
  `an_unseen_helm_opens_the_card_at_window_start_with_focus_on_got_it`; the links' look is the `welcome-dialog` golden;
  isolation and the short window are the shared dialog behavior named under the announcements' content case below.
- "send feedback now records seen and hands focus to feedback":
  `send_feedback_now_marks_seen_and_opens_feedback_without_sending`.
- "help reopens welcome without rewriting seen": `reopening_from_help_writes_nothing_and_returns_focus_to_the_button`,
  `help_menu::tests::welcome_reopens_the_card_without_writing_and_returns_focus_to_the_button`, and `gui-e2e-smoke.py`'s
  `test_52`.
- "announcement dismissal by ${dismissal} survives reload": `each_dismissal_records_exactly_the_shown_entries_once` and
  `the_pending_batch_opens_at_window_start_newest_first` (in `announcements/tests.rs`).
- "announcements wait for welcome${feedback ? ' and its feedback handoff' : ''}":
  `the_batch_waits_for_the_card_and_opens_when_it_closes` and `the_batch_waits_for_the_feedback_handoff_too`.
- "a late queue result waits behind a reopened welcome": superseded. The queue is pushed state read from the store when
  the window start's check runs, so no read finishes late; the check runs at the start, or when the first-open card and
  its handoff close, and a card reopened from Help is no part of it.
- "announcements wait for the preference decision and survive a failed preference read":
  `the_pending_batch_opens_at_window_start_newest_first` (its reset without preferences).
- "a failed shown write dismisses and leaves history hidden": `a_failed_record_still_closes_the_batch`; the history item
  follows the pushed history, which a failed write does not change
  (`help_menu::tests::announcements_appear_after_a_showing_and_open_the_history`).
- "Help exposes shown history after release notes and reopening writes nothing", "an empty queue leaves the history item
  hidden" and the content half of "announcement content, modal isolation and short-window controls": the history tests
  in `announcements/tests.rs`, `an_empty_queue_or_a_later_push_opens_nothing` and
  `help_menu::tests::announcements_appear_after_a_showing_and_open_the_history`. That case's isolation half is the
  shared dialog behavior, tested once for every dialog (`focus_tests::tab_stays_inside_the_topmost_dialog`,
  `components::tests::dialog::a_long_dialog_body_scrolls_and_the_answers_stay_visible` and
  `a_frame_too_tall_for_the_window_scrolls_as_a_whole`; global chords do nothing while a dialog is open, which each
  chord's handler decides and tests with its own chord, for example
  `launcher::entry_tests::the_new_session_action_opens_new_unless_a_dialog_is_open`; that only the key pressed after
  closing reaches the terminal's program has no test of its own here, since a dialog holds focus while it is open and
  the terminal receives keys only while focused).
- "long announcements without links are keyboard-scrollable":
  `a_linkless_history_has_a_body_stop_named_announcement_content` and `a_linked_history_has_no_body_stop` (the shared
  stop's keys are tested in `components::tests::dialog`).

Also tested, from the decisions on the window-start mark (SPEC_impl.md, "Overlays: the layer manager"):
`a_press_before_the_open_refuses_the_card_and_the_batch_until_a_reset`. A reset while the card or its feedback handoff
is open writes nothing and decides again:
`a_reset_while_the_card_or_its_handoff_is_open_writes_nothing_and_decides_again`. Bridge runs keep the start dialogs
closed unless `gui-agent.py start --start-dialogs` asks for them:
`an_agent_run_with_the_start_dialogs_off_opens_and_writes_nothing_at_start` in process, and `gui-e2e-smoke.py`'s
`test_53` end to end (no card in the smoke's own default start; the card at a second run's `--start-dialogs` start).

The golden is `welcome-dialog` (the reference scene's card).

## `tests/trash-dialog.spec.ts` and the trash cases of other specs, partly

The Trash button is `trash::TrashButton`, which the session list keeps and draws in its header, and the dialog it opens
is `trash::dialog`; the rules both use are `trash::listing`. Ported: the button, its reads and rechecks, and the dialog
with each checkout's own Delete and Retry, and the per-host and global confirmed deletes (each host's "Delete all on"
and the footer's Delete all), the archive cue's wiggle (`trash::cue`), and the rail notices panel's Retry now and Open
Trash, which call into the button (`TrashButton::retry`, `TrashButton::open`). The archive cue's flight comes with its
own piece.

Replacing tests, in `crates/gui/farhelm-gui/src/trash/dialog/tests.rs` unless named otherwise (`trash/tests.rs` is the
button's, `trash/listing/tests.rs` the rules'):

- "trash list stays keyboard-scrollable while every list action is disabled":
  `opening_measures_each_reachable_host_and_starts_on_footer_close` (every entry's Delete unavailable while the
  opening's measured reads are out, focus on footer Close, Tab to the footer's Delete all, then round to the header's
  close, the first host's "Delete all on" and its first entry's Delete) and
  `the_archive_list_has_its_own_stop_while_every_action_is_unavailable` (the list's own stop, "Archived checkouts",
  while nothing in it can take focus; the keys that scroll it are the shared stop's, tested in
  `components::tests::dialog`).
- "stuck trash retries in place until its host confirms the files are gone":
  `a_failed_removal_offers_retry_through_the_same_delete`,
  `an_entrys_delete_runs_at_once_and_keeps_its_row_until_a_listing_confirms_it_gone`, and in `trash/tests.rs`
  `only_a_host_freeing_space_is_rechecked_until_it_reports_completion` (a failed read keeps the last removal report);
  `an_accepted_delete_keeps_a_capable_hosts_row_removing_until_a_later_listing` in `trash/listing/tests.rs`.
- "one trash entry deletes immediately and preserves neighbours after ${success | failure}":
  `an_entrys_delete_runs_at_once_and_keeps_its_row_until_a_listing_confirms_it_gone`,
  `closing_the_dialog_leaves_a_running_delete_to_finish`, `a_refused_delete_stays_visible_on_the_next_opening`, and
  `each_host_card_shows_its_archives_and_an_unreachable_host_offers_nothing` (the unreachable host offers no Delete).
- "deleting another trash entry preserves an earlier refusal" and "an unrelated trash success retains an uncertain
  outcome until its own retry": `a_refused_delete_stays_visible_on_the_next_opening`, and
  `a_delete_answer_settles_only_its_own_checkouts_notices` in `trash/listing/tests.rs`.
- "trash notices survive outages and disappear when their host is removed" and "a late failed trash reply cannot restore
  notices for a removed host": `notices_go_only_when_a_readable_registry_drops_their_host`.
- "single trash delete requires settled listings and a healthy host refresh":
  `opening_measures_each_reachable_host_and_starts_on_footer_close` and
  `deletes_wait_for_a_readable_registry_and_say_why`; the guards themselves in
  `an_entry_delete_selects_one_listed_checkout_only_under_the_hosts_guards` (`trash/listing/tests.rs`).
- "trash entry deletes pause after rows move under the pointer": `rows_moving_pause_every_entrys_delete`.
- "trash polls only the host freeing space and stops on its completion":
  `only_a_host_freeing_space_is_rechecked_until_it_reports_completion` and
  `reads_follow_each_hosts_connection_and_trash_epoch_only` in `trash/tests.rs`.
- "${global | host} trash cleanup closes immediately with ${normal | reduced} motion":
  `delete_all_sends_one_frozen_delete_per_reachable_host` and
  `a_hosts_delete_all_asks_inside_its_card_and_deletes_only_there`; the can, upside down and bobbing, and still under
  reduced motion, is `the_can_bobs_while_space_is_freed_and_stands_still_under_reduced_motion` in `trash/tests.rs`, and
  the `trash-button-emptying` golden. Delete all waiting on every reachable host and skipping an unreachable one is
  `delete_all_waits_for_every_reachable_host`, and
  `delete_all_batches_every_reachable_host_and_waits_on_any_unsettled_one` in `trash/listing/tests.rs`; Cancel and
  Escape returning focus to the button that asked, the band going with the question, are
  `cancelling_returns_focus_to_the_asking_button_and_takes_the_band_away` and
  `cancelling_returns_focus_to_delete_all_while_it_is_unavailable`; an open question holding the entries' Deletes and
  Close, the header's close and Escape dismissing it before the dialog, with the settle pause restarting as it closes,
  are `an_open_question_holds_the_dialog_and_close_or_escape_dismisses_it_first`; a confirmation refused at the press
  (its host reconnected under the question) sending nothing and leaving the dialog open is
  `a_confirmation_the_guards_refuse_sends_nothing_and_keeps_the_dialog`.
- "trash batch issues survive reopening after ${http | transport} failure", partly: a bulk delete's lost answer is one
  notice naming every ID it sent until later reports settle them,
  `a_bulk_delete_answer_is_one_notice_that_later_reports_shrink` in `trash/listing/tests.rs`; it is kept and shown on
  the next opening as an entry delete's is (`a_refused_delete_stays_visible_on_the_next_opening`), through the same
  path.
- "a late trash poll cannot restore a confirmed deletion": answered by the decision in `trash/listing.rs`'s module docs,
  not by read generations: commands are answered in order, the newest read per host wins, and the host is read again
  once the delete settles.
- "trash header and global confirmation remain reachable at width ${1000 | 360}", partly: the global question is drawn
  in the frame's pinned region above the footer, which scrolls on its own, and the focus coordinator scrolls its
  starting answer into view (`delete_all_sends_one_frozen_delete_per_reachable_host` checks the answer takes focus; the
  `checkout-trash-confirm-all` golden shows it). Scrolling the region by keyboard waits for a foundation change (the
  first entry under "### Trash" below). The 360-pixel width is below the app's minimum window.
- `tests/github-checkouts.spec.ts`, "trash count and cue follow the final reference, including another client": the
  count following another window's change is `reads_follow_each_hosts_connection_and_trash_epoch_only` (a moved trash
  epoch reads that host); the per-host confirmation is `a_hosts_delete_all_asks_inside_its_card_and_deletes_only_there`;
  the cue playing only for this client's confirmed archive is `trash/cue/tests.rs`'s
  `a_confirmed_archive_from_this_window_wiggles_the_can` and `nothing_but_a_confirmed_archive_wiggles` (a bare Delete, a
  refusal and another client's removal play nothing), and reduced motion and a hidden button
  `reduced_motion_or_a_hidden_button_plays_nothing`; the flight waits for a foundation change (under "### Archive cue"
  below). "trash global confirmation freezes archives and reports preserved recovery work": the freezing is
  `delete_all_sends_one_frozen_delete_per_reachable_host`, and a host's report of what it kept is shown as any delete's
  is (`a_refused_delete_stays_visible_on_the_next_opening`). "single trash delete removes one real archive and preserves
  its neighbour", partly: the frozen one-checkout command is
  `an_entrys_delete_runs_at_once_and_keeps_its_row_until_a_listing_confirms_it_gone`; the real archive's removal is not
  driven end to end, because the bridge's fake fleet holds no managed checkout (its sessions run in `/tmp`, and a
  checkout needs a repository source the bridge does not set up). The end-to-end smoke test's step 49 opens the dialog
  over the real helm and supervisor with an empty trash.
- `tests/bulk-session-actions.spec.ts`, "a newly displayed retained mark preserves the open Trash dialog": marking is
  not ported yet (the sidebar's); the dialog is a layer of its own, so nothing that hides the button can close it.
  "archive cues require a visible Trash destination while marks remain", partly: a Trash button not drawn plays no
  wiggle (`reduced_motion_or_a_hidden_button_plays_nothing`, with the Hosts panel in the list's place); the marks that
  hide it in the old app wait for marking, and the flight it counted for a foundation change.

The goldens are `trash-button-empty`, `trash-button-full`, `trash-button-emptying`, `checkout-trash-dialog` (the
reference scene's trash), and `checkout-trash-confirm-all` and `checkout-trash-confirm-host` (its questions; the
reference screens show none).

## `tests/keyboard-shortcuts.spec.ts`, ported

The overview is `shortcuts`: its table (`shortcuts::sections`), its dialog, and its two ways in, the chord (handled on
the main view's ground) and the `?` menu's Keyboard shortcuts item. Replacing tests, in
`crates/gui/farhelm-gui/src/shortcuts/tests.rs` unless named otherwise:

- "the ${mac | non-Mac} overview chord owns terminal input and labels": the labels for both platforms are
  `each_platform_shows_only_its_own_forms_with_new_on_the_mac_alone` (the Linux app has no New, as the migration's
  orchestrator decided on 2026-11-13), and that each chord shown is the one the app handles is
  `every_chord_the_table_shows_runs_the_action_its_row_names`; the chord from a focused terminal reaching no program,
  focus starting on Close and Escape returning to the terminal are
  `the_chord_from_a_terminal_opens_the_overview_and_escape_returns_home` and `gui-e2e-smoke.py`'s `test_65`; the chord
  doing nothing while another layer holds the keyboard is
  `the_chord_does_nothing_over_a_menu_or_over_the_overview_itself` (the old case used New's launcher; the refusal is the
  same check for every dialog and menu).
- "the Help item closes with Close, Escape or outside and restores its toggle":
  `the_menu_item_opens_the_overview_and_every_close_returns_to_the_help_button`, which also walks Tab from Close to the
  body's own stop, "Keyboard shortcuts", and back; the item's place in the menu is
  `help_menu::tests::the_items_keep_specs_order_with_the_update_check_and_the_linux_notices_last`.
- "the ${mac | non-Mac} overview rejects near chords, repeat and composition": near chords, an extra or a missing
  modifier, and the physical Slash on a layout that prints something else there are the physical-key table's own tests
  (`actions_tests::keypad_digits_and_extra_modifiers_do_not_match`, `the_position_decides_whatever_the_layout_prints`,
  `keys_at_those_positions_without_the_chord_reach_the_program`); a held repeat is
  `actions_tests::a_held_number_key_acts_once_and_a_held_text_size_chord_repeats`, which holds this chord and finds it
  runs once. Composition is a deviation (below).
- "the chord is unavailable before sign-in or initial preferences":
  `the_chord_does_nothing_before_the_preferences_arrive`; the sign-in half is superseded (the desktop app never signs
  in).
- "each opening refreshes template bindings and a failed read keeps cached names":
  `the_template_rows_name_the_marked_template_and_each_assigned_key_in_order` and
  `the_template_rows_follow_the_pushed_preferences_while_open_and_at_each_opening`; the failed read is a deviation
  (below).

NOTE: Two of the shortcuts the overview advertises do nothing yet: the template list (Cmd+J, Ctrl+Shift+J) and the
template number keys (the 0–9 row and each assigned key's row). The keymap reserves their chords, so they never reach a
terminal's program, but no handler acts on them: the template list has no action, and `TemplateNumberKey` has no handler
outside an armed sequence, which reads bare digits itself. They start working when the templates area wires them. Quick
launch (Cmd+P, Ctrl+Shift+P) works: it has its action and the main view's handler, and the drift test checks its chord
as bound to that action. The drift test checks that each chord is bound or reserved as its row says, not that anything
handles it.

The golden is `keyboard-shortcuts-dialog` (Linux forms; the old overview was never captured, so SPEC.md and the old code
are its reference).

## `tests/rail-notices.spec.ts`

The bell is `rail_notices::NoticesButton`, which the session list keeps and draws at the top of the rail's foot; its
panel draws the pushed `Notices` collection. Replacing tests, in `crates/gui/farhelm-gui/src/rail_notices/tests.rs`
unless named otherwise:

- "rail notices quiet, alerting and resolved in ${palette}":
  `the_bell_reports_its_unread_count_in_its_name_and_beside_the_glyph`,
  `the_bell_counts_unread_unresolved_notices_and_the_panel_is_newest_first`,
  `the_panel_lists_notices_newest_first_beside_the_bell` (the notice's host, checkout, size, reason, failure time and
  the resolved entry without Retry now), `clear_resolved_removes_only_the_resolved_notices_shown`,
  `clear_resolved_is_unavailable_with_nothing_resolved` and `an_empty_panel_says_there_are_no_notifications`; the
  computed colors in both palettes are the `notices-alerting` and `notices-empty` goldens, and the closed loud bell is
  the `notices-bell-loud` golden. The bell's place above Hosts and its Tab stop are
  `session_list::host_rail::tests::the_foot_holds_hosts_above_the_settings_gear_with_their_names_and_help` and
  `tab_reaches_the_foot_after_the_rail_and_the_lists_controls`.
- "rail close marks only shown notices and preserves independent focus":
  `every_close_path_marks_only_the_unread_notices_shown` (Escape, the bell, Close, a press elsewhere, Tab out of the
  panel), `a_press_on_hosts_closes_the_panel_marks_it_and_shows_host_management` (a press on another control closes and
  marks, and still takes effect), `the_close_marks_only_what_a_frame_drew` and
  `closing_with_nothing_unread_shown_sends_nothing`. Its fenced GET is superseded: the notices are pushed, and nothing
  reads them.
- "Trash reads follow host epochs and coalesce independently": the Trash button's, in `trash/tests.rs`
  (`reads_follow_each_hosts_connection_and_trash_epoch_only`).
- "rail Retry and Open Trash share the permanent guarded cleanup owner": `retry_now_follows_the_trashs_guards` and
  `open_trash_opens_the_dialog_and_closing_it_returns_to_the_bell` (with host management shown); the guard itself is
  `trash::listing::tests::retry_needs_a_capable_listing_showing_the_checkout_stuck`. Marked sessions are not ported yet
  (the sidebar's), so the case's marking half waits for them; the Trash dialog is a layer of its own, which nothing that
  hides the header's button closes.

## Deviations

Where the port does not match the old app, and why. Each stays until the work named in it lands.

### The Help menu

- **The Help menu's placement.** The old menu hung from the `?` button's bottom edge, kept at least 8 pixels from the
  window's edges, with a pointer toward the button. The layer manager places every side menu from its anchor's top and
  keeps it flush inside the window (its beside placement records that shared clamp), so at the rail's foot the menu sits
  on the window's bottom edge, and no pointer is drawn (the session row's menu has the same gap). The toggle's
  `aria-haspopup` is not ported either, as for every menu button so far.
- **Menu keys.** The old Help menu also opened from its closed button with the arrow keys and moved to its first and
  last items with Home and End. The layer manager's menu context binds only Up and Down for every menu, so neither is
  ported here; it is a gap of the shared menu, not of this one.
- **A resize no longer closes the Help menu.** The old menu closed when the window's size changed, because its measured
  position went stale. The layer manager anchors the menu to the button, so it follows it instead. A press outside the
  menu still closes it; the layer manager's own tests cover that for every popover.
- **The menu can be taller than the smallest window.** With Keyboard shortcuts and Welcome in it, the Help menu fills a
  window at the 720 by 560 minimum exactly on Linux, and an installed Linux app that offers both Announcements and Check
  for updates as well would draw it about 20 pixels taller than the window. Menus have no maximum height and do not
  scroll, so the overflow is cut off at the window's edge. That is a follow-up for the shared menu, not for this one.

### Send feedback

- **The Send feedback dialog.** Its message box has no resize corner: it grows with its text to the growing box's cap
  and then scrolls (decided by the maintainer on 2026-11-13). While a send is out the fields are read-only rather than
  disabled and Send draws busy, so focus stays where it was; after a failed send it stays on Send rather than moving to
  the message as the old dialog's did, which only moved it because disabling the fields had dropped it. A disabled Send
  takes the shared button's retired look (the `--bg-2` fill and `--border-dim` edge) rather than the old dialog's own
  (`--bg-1` and `--border`), with the same `--fg-2` text. The old app announced a successful send once more in a status
  region outside the dialog after it closed; here the thank-you line is itself a status, announced while it shows. Its
  failure wording for a helm it could not reach at all is not ported: a lost connection resets the window, which closes
  the dialog. Re-use's hover help no longer says "for the desktop app and web UI", since there is no web UI. The
  attached box's term column is 104 pixels wide where the old grid's `max-content` came to about 99: GPUI draws Inter a
  few pixels wider, and at 100 the "Operating system" term wrapped onto a second line.

### Welcome

- **Where the `?` menu is.** The old card said Send feedback is "in the **?** menu at the top of the sidebar", which was
  already stale at the reference commit: the `?` button sits at the host rail's foot. The card says "at the bottom left
  of the window" instead (decided by the migration's orchestrator, 2026-11-14).
- **The links sit on a line of their own.** The old card ran Read Get started and Send feedback now on at the end of
  their sentences. GPUI has no inline flow for a control inside a paragraph, so each sits on a line of its own under its
  sentence, at the start, which makes each pointer one line taller than the old one.
- **Focus after the feedback handoff from the card that opened by itself.** The old feedback dialog always returned
  focus to the Help toggle, also when the first-open card had handed off to it. Here it returns focus where the card
  would have, home, since that card has no originating control (SPEC.md); after a card reopened from Help it returns to
  the `?` button, as before.

### Keyboard shortcuts

- **Composition opens it.** SPEC.md says composition does not open the overview, and the old listener checked the
  event's `isComposing`. Here the keystroke interceptor takes the chord by the Slash key's position before the element
  that owns the input handler sees it, so the refusal falls to the chord's handler, and nothing in the app can tell that
  handler whether the focused input is composing (GPUI's key events carry no composing flag, and no foundation asks the
  focused input for marked text). A chord typed mid-composition opens the overview. It stays until an input-method query
  exists for handlers to ask.
- **No read when it opens.** The old overview read the helm's preferences once per opening and kept the names it had
  cached when that read failed. Here the ⚡ and number-key rows come from the preferences the helm pushes, which the
  store holds current (every change to them is pushed, an agent's or the command line's included), so each opening
  already shows the current bindings, and there is no read to fail.
- **A menu keeps the chord too.** The old listener refused only while a modal dialog was open, so it opened over an open
  menu. Here, as for every chord in this app, a menu holding the keyboard counts as well (`layers::focus_layer_open`).
- **Group headings have no letter spacing.** The old headings were spaced `0.06em` apart; GPUI's text style cannot
  express letter spacing, so the capitals sit at their normal spacing.

### Trash

- **The footer does not stack at phone widths.** The old dialog stacked Close and Delete all as full-width rows on a
  narrow page; the app's 720 by 560 minimum window never reaches that breakpoint (the migration's orchestrator,
  2026-11-13).
- **The button is never disabled.** The old button was unavailable while the page's launch-and-registry lock was held.
  The GPUI app has no such lock (each launch has a key of its own), so the button is always available, and an entry's
  Delete waits on its host's own key instead, which a host's removal holds.
- **No read when the dialog closes.** The old page read every host again, without sizes, when the dialog closed. SPEC.md
  does not ask for it: a host is read when its trash epoch moves, when it reconnects, when the dialog opens, and, while
  it reports removal, at each three-second recheck.
- **A delete that times out on its host reads nothing at once.** The old page read every host again once its deletes
  settled. The helm now stops waiting on a host after 20 seconds and answers a delete with a timeout while the cleanup
  goes on, holding the host's trash gate; a read sent then would wait the same 20 seconds, and since the helm answers
  the window's commands one at a time, every later command would wait behind it. So after a timed-out delete the host is
  read once its trash epoch moves, which the helm does when the late cleanup lands (decided by the other-dialogs area's
  coordinator, 2026-11-15). The old app had no such bound on the host.
- **A failed read ends the rechecks.** The old page kept a host that last reported removal on its three-second poll
  after a failed read, and its reader also retried the failure after half a second. Here a failed read keeps the host's
  last removal report (the can stays over) but schedules no further recheck; the host is read again when its trash epoch
  moves, when it reconnects, or when the dialog opens. A read fails only when the host or its connection is in trouble,
  and one that timed out held the window's commands for the helm's 20-second host-wait bound, which the old app did not
  have; the window cannot tell a timeout from a quick failure (decided by the other-dialogs area's coordinator,
  2026-11-15).
- **Waiting for a foundation change: the global question's region is not keyboard-scrollable.** SPEC.md puts the global
  question in "its own bounded, keyboard-scrollable region directly above the footer". It is drawn in the frame's
  bounded notices region above the footer's actions, which scrolls on its own beyond about a quarter of the window, but
  while an answer holds focus Page Up and Page Down do not scroll it, and Tab reaches only the answers, so a warning
  longer than the region cannot be read by keyboard. The region needs a keyboard stop of its own, or Page Up and Page
  Down scrolling, while it holds controls; the shared frame has neither. Requested of the foundations by the
  other-dialogs area's coordinator (2026-11-15).
- **The question's band is the old band with the prompt's plain text.** The band is the old strip, red-tinted under a
  red rule with 10 by 12 pixels of padding, drawn only while the question is open. The words inside are the prompt's
  plain look, 13-pixel text at a line height of 19.5, where the old band had the dialog's 12-pixel text at 1.6: the
  prompt sets its own text size. The plain look also gives the question the status role and names it by its first line,
  the count and hosts, where the old band was a group named "Confirm permanent checkout deletion"; neither changes
  without changing the prompt. The global band sits on the footer's background under the footer's top rule, inside the
  footer's own inset, rather than in a region of its own reaching the dialog's edges.
- **Dismissing a host's question returns focus to its "Delete all on".** Close, the header's close and Escape with a
  question open dismiss the question, as in the old dialog, and return focus to the button that asked it. The old dialog
  returned focus to Delete all for the global question; for a host's it moved none, so focus left inside the band (its
  Cancel, or Escape inside it) fell to the footer's Close, and stayed in place only for Close or Escape outside the
  band. Cancel here does the same as dismissing, since the prompt returns focus to the button that asked.
- **Each of the question's paragraphs is two lines.** The old band's paragraphs ran on after the host names ("… on this
  machine, build-box? Work in them …"); here each ends where the host names do, and the rest is a line of its own, in
  the same words. A host value is drawn as a block of its own, so the rest started a new line anyway, with the "?" alone
  at its start.
- **A confirmation the guards refuse takes the question away.** A Delete permanently pressed while another trash delete
  of this window's runs, while the registry cannot be read, or after a host reconnected or became busy with its own work
  is taken: the question goes, nothing is sent, the dialog stays open, and nothing says why. The old band's answer was
  unavailable while a delete ran, and a refused band answer kept the band up, also without a message. For a reconnected
  host the outcome matches the old app's, which closed the band and read the trash again: the reconnect reads the host
  again here, and the dialog shows its trash afresh.
- **Hosts progress independently in outcome, not in time.** Each host's delete is its own request on its own key, and
  one host's answer never waits on another's to be applied. But the helm answers the window's commands one at a time, so
  a host that does not answer delays the others' answers by up to the helm's 20-second host bound.

### Archive cue

- **Waiting for a foundation change: the row's outline does not fly into the can.** SPEC.md's cue has "the row's dashed
  outline" fly into the trash before it wiggles. That outline is drawn above the sidebar, over whatever is there, and
  only the layer manager may draw such an overlay; it has none for this (its one free-floating overlay is the
  quick-launch pill). Requested of the foundations by the other-dialogs area's coordinator. Until then the can wiggles
  at once on the archived reply, as the old cue did when it had no row to fly from.
- **Only the can turns.** The old wiggle turned the whole button, its edge and count with it. GPUI turns an SVG, not a
  box of elements, so here the can turns inside a still button.
- **The wiggle is drawn at the ticker's ten steps a second.** The old animation ran at the display's rate. Here each
  tick draws the turn the old eased keyframes give at that moment, so its sharpest swings fall between ticks: the first
  tick shows about -14 degrees, and the 12-degree swing back is not seen. The ticker's ten-a-second cap is the app's
  rule for every animation.
- **A wiggle ends when the window goes inactive.** The ticker sends no ticks to an inactive window, so a wiggle under
  way ends there, upright, rather than being held tilted; one asked for while the window is inactive does not start. The
  old 900 ms animation ran on regardless, and would have been over by the time the user came back.
- **It plays over the bob.** A can already upside down and bobbing (a delete of this window's running, a host freeing
  space) wiggles too, the turn added to its upside-down one, as the old wiggle turned the button around the bobbing can.

### Session bell

- **The notification list's placement and dismissals.** The list opens beside its bell through the layer manager's one
  beside placement, so it shares the Help menu's clamp and has no pointer notch, and the bell's toggle has no
  `aria-haspopup`. The old list closed when the window was resized or when a row above it moved (a local Delete), since
  its measured position went stale; this one follows its bell instead, and closes only when the bell is no longer drawn.
  Either way its close marks what it showed read.
- **Clear all waits for its answer.** The old list closed at once on Clear all, moved focus to the row's open button,
  and showed a refused clear on the row's error line. The row's line belongs to the operation store, which a keyless
  send does not reach, so here the list stays open, its button busy, until the helm answers: on success the bell goes
  and the list with it, and a refusal shows under the button, worded as the old line was ("Clear notifications: …").
  After a successful clear, focus goes back where it was before the list opened, or home, not to the row's open control
  as in the old app. That is a follow-up: the list needs the row's focus handle, which only the session list's wiring
  has.
- **New follows the store's read mark.** The old list kept the read mark as it stood when it opened, so an entry stayed
  New while the list was open even if another window moved the mark. This list draws New from the store's mark every
  frame (only what its close needs is kept), so an entry stops being New if the mark moves while the list is open: the
  read mark is kept by the helm and shared by every client (SPEC.md, "Status"), so another client closing its own list
  moves it, and so can a late answer to this window's own previous close.
- **The session bell sits centred on the age's line.** The old bell sat on the age's text baseline; here it is centred
  on the same line, which can put it about a pixel lower, so the row's height stays the age's.
- **An entry's text wraps by parts when it holds a docs link.** Each run of host text and the link are elements of their
  own (host text stays direction-isolated), so the link starts a new line when it does not fit after the words before
  it, where the old text flowed inline. Text without a link wraps as before.
- **An overflowing list scrolls only with the pointer.** Ten long entries can be taller than a short window; the list
  then scrolls its entries, but the keyboard reaches only its links and Clear all, so an entry without a link cannot be
  scrolled to by keyboard, where the old list's focused panel scrolled with the arrow keys.

### Rail notices

- **The panel opens from the bell's top, not up from its bottom.** The old panel's bottom edge sat 4 pixels below the
  bell's, so a long list grew upward. The layer manager's one beside placement starts a popover 4 pixels above its
  anchor's top and keeps it inside the window, so at the rail's foot the panel sits on the window's bottom edge, as the
  Help menu does, and no pointer notch is drawn. An upward placement would be a change to the layer manager.
- **The open bell wears the open look.** The old toggle had no look of its own while its panel was open. The shared
  button draws every open toggle with the accent fill and the inverse ink (the Help button's look), so the bell does
  too, its glyph and count in the inverse ink, red or not, while the panel is open.
- **The close marks only the unread notices it showed.** The old reader sent every shown id on each close; here the ids
  of notices that were already read or resolved are left out, and a close that showed nothing unread sends nothing, as
  the session bell does. The helm changes nothing for those ids either way.
- **A Retry refusal shows on the notice.** The old panel showed a Retry's error line at the panel's top. Here the
  Trash's own record of that checkout's last refusal shows under the notice's actions, the same words the Trash dialog
  shows, whichever of the two retried it.
- **No "Loading notifications…" wait in practice.** The notices arrive with the first snapshot; the line shows only
  while the collection has never been read. The old panel's "Could not refresh notifications" shows when the helm pushes
  the collection as unavailable.
