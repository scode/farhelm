# Parity: Other dialogs

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Feedback, Welcome card, Announcements, Partial deletion, Status (session notifications).

| Old spec                           | Tests | Covers                                                                       | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ---------------------------------- | ----- | ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/trash-dialog.spec.ts`       | 14    | trash list stays keyboard-scrollable while every list action is disabled     | partly, below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `tests/feedback.spec.ts`           | 12    | feedback uses dark fields and pinned chrome at normal and short heights      | partly: the Help menu cases (its place under the gear, its items and their order, Escape and a second press closing it, focus back on the `?` button, the three links) are replaced by `help_menu::tests` and `gui-e2e-smoke.py`'s `test_45`; the WebKit stacking-context case is superseded by the layer manager, which draws every overlay above the sidebar; the dialog's cases (the Discord link keeping the draft, sending exactly what is shown, thanks and closing, remembering, rewriting and clearing the contact, a failure keeping the text, a close queued behind Send waiting, blank and over-long messages) are replaced by `feedback::tests`, its look by the `feedback-dialog` and `feedback-dialog-reuse` goldens, and opening it from the menu, typing and Cancel by `gui-e2e-smoke.py`'s `test_48`. Missing: the send end to end in the running app, because nothing carries a stand-in endpoint into the app's embedded helm and a Send there would post real feedback, so no bridge run presses Send (the helm's forwarding is covered by its own tests against a stand-in); and the short and narrow window geometry checks (the dialog frame's own tests cover a scrolling body with pinned answers). `modal-layout.spec.ts`'s sent-feedback case is replaced by `feedback::tests::after_a_send_the_thanks_has_a_body_stop_of_its_own` (the text-only body's own stop, "Feedback sent"; the shared stop's keys are tested in `components::tests::dialog`) |
| `tests/welcome.spec.ts`            | 7     | welcome dismissal by ${dismissal} is shared and survives reload              | ported, below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `tests/announcements.spec.ts`      | 10    | announcement dismissal by ${dismissal} survives reload                       | ported in process, below; no end-to-end leg, since the running app's helm has no announcement to show (its compiled release table is empty) and the bridge's fake fleet cannot queue one                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `tests/keyboard-shortcuts.spec.ts` | 5     | the ${mac ? "Mac" : "non-Mac"} overview chord owns terminal input and labels | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `tests/sounds.spec.ts`             | 6     | device sounds                                                                | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `tests/notifications.spec.ts`      | 12    | session notifications                                                        | ported: the bell's presence, loudness, name, help and place before the age, the list beside the bell without opening the session, newest first with New and Resolved and ages, Escape, the bell again, a press elsewhere and the row going each marking what the list showed, nothing sent when nothing unread was shown, a refused mark sent again on the next close, Clear all and the bell's return, and the docs-link rule are replaced by `notices_bell::tests` and the `session-bell-list` golden; the cleanup-helper case tested the old suite itself; the two build-mismatch cases and the older-failure race are superseded by the store (every change is pushed before its reply, and no mark is cached locally); the resize and moved-row dismissals are deviations (below)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |

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
is `trash::dialog`; the rules both use are `trash::listing`. This piece ports the button, its reads and rechecks, and
the dialog with each checkout's own Delete and Retry. The per-host and global confirmed deletes (Delete all and Delete
all on a host), the archive cue and the notices bell's Retry now and Open Trash come with their own pieces.

Replacing tests, in `crates/gui/farhelm-gui/src/trash/dialog/tests.rs` unless named otherwise (`trash/tests.rs` is the
button's, `trash/listing/tests.rs` the rules'):

- "trash list stays keyboard-scrollable while every list action is disabled", partly:
  `opening_measures_each_reachable_host_and_starts_on_footer_close` (every entry's Delete unavailable while the
  opening's measured reads are out, focus on footer Close, Tab to the header's close and then the first entry's Delete)
  and `the_archive_list_has_its_own_stop_while_every_action_is_unavailable` (the list's own stop, "Archived checkouts",
  while nothing in it can take focus; the keys that scroll it are the shared stop's, tested in
  `components::tests::dialog`). Missing: Delete all and each host's Delete in the Tab walk, which come with the
  confirmations piece.
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
- "${global | host} trash cleanup closes immediately with ${normal | reduced} motion": the confirmations piece's. Its
  can, upside down and bobbing, and still under reduced motion, is ported here:
  `the_can_bobs_while_space_is_freed_and_stands_still_under_reduced_motion` in `trash/tests.rs`, and the
  `trash-button-emptying` golden.
- "trash batch issues survive reopening after ${http | transport} failure", "a late trash poll cannot restore a
  confirmed deletion" and "trash header and global confirmation remain reachable at width ${1000 | 360}": the
  confirmations piece's (they confirm a bulk delete). The late poll's race is answered by the decision in
  `trash/listing.rs`'s module docs, not by read generations: commands are answered in order, the newest read per host
  wins, and the host is read again once the delete settles.
- `tests/github-checkouts.spec.ts`, "trash count and cue follow the final reference, including another client": the
  count following another window's change is `reads_follow_each_hosts_connection_and_trash_epoch_only` (a moved trash
  epoch reads that host); the cue and the per-host confirmation are later pieces'. "trash global confirmation freezes
  archives and reports preserved recovery work" is the confirmations piece's. "single trash delete removes one real
  archive and preserves its neighbour", partly: the frozen one-checkout command is
  `an_entrys_delete_runs_at_once_and_keeps_its_row_until_a_listing_confirms_it_gone`; the real archive's removal is not
  driven end to end, because the bridge's fake fleet holds no managed checkout (its sessions run in `/tmp`, and a
  checkout needs a repository source the bridge does not set up). The end-to-end smoke test's step 49 opens the dialog
  over the real helm and supervisor with an empty trash.
- `tests/bulk-session-actions.spec.ts`, "a newly displayed retained mark preserves the open Trash dialog": marking is
  not ported yet (the sidebar's); the dialog is a layer of its own, so nothing that hides the button can close it.
  "archive cues require a visible Trash destination while marks remain" is the archive cue piece's.

The goldens are `trash-button-empty`, `trash-button-full`, `trash-button-emptying` and `checkout-trash-dialog` (the
reference scene's trash).

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
