# Parity: Other dialogs

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Feedback, Welcome card, Announcements, Partial deletion, Status (session notifications).

| Old spec                           | Tests | Covers                                                                       | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ---------------------------------- | ----- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/trash-dialog.spec.ts`       | 14    | trash list stays keyboard-scrollable while every list action is disabled     | partly, below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `tests/feedback.spec.ts`           | 12    | feedback uses dark fields and pinned chrome at normal and short heights      | partly: the Help menu cases (its place under the gear, its items and their order, Escape and a second press closing it, focus back on the `?` button, the three links) are replaced by `help_menu::tests` and `gui-e2e-smoke.py`'s `test_45`; the WebKit stacking-context case is superseded by the layer manager, which draws every overlay above the sidebar; the dialog's cases (the Discord link keeping the draft, sending exactly what is shown, thanks and closing, remembering, rewriting and clearing the contact, a failure keeping the text, a close queued behind Send waiting, blank and over-long messages) are replaced by `feedback::tests`, its look by the `feedback-dialog` and `feedback-dialog-reuse` goldens, and opening it from the menu, typing and Cancel by `gui-e2e-smoke.py`'s `test_48`. Missing: the send end to end in the running app, because nothing carries a stand-in endpoint into the app's embedded helm and a Send there would post real feedback, so no bridge run presses Send (the helm's forwarding is covered by its own tests against a stand-in); the short and narrow window geometry checks (the dialog frame's own tests cover a scrolling body with pinned answers); and `modal-layout.spec.ts`'s sent-feedback case, whose text-only body needs the dialog frame's keyboard stop for a scrolling body with nothing focusable, which the frame does not have yet                                                                                                                                                                                                                                       |
| `tests/welcome.spec.ts`            | 7     | welcome dismissal by ${dismissal} is shared and survives reload              | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `tests/announcements.spec.ts`      | 10    | announcement dismissal by ${dismissal} survives reload                       | partly: "Help exposes shown history after release notes and reopening writes nothing", "an empty queue leaves the history item hidden" and the content half of "announcement content, modal isolation and short-window controls" are replaced by `announcements::tests` and `help_menu::tests::announcements_appear_after_a_showing_and_open_the_history`; that case's isolation half is the shared dialog behavior, tested once for every dialog (`focus_tests::tab_stays_inside_the_topmost_dialog`, `components::tests::dialog::a_long_dialog_body_scrolls_and_the_answers_stay_visible` and `a_frame_too_tall_for_the_window_scrolls_as_a_whole`; global chords do nothing while a dialog is open, which each chord's handler decides and tests with its own chord, for example `launcher::entry_tests::the_new_session_action_opens_new_unless_a_dialog_is_open`; that only the key pressed after closing reaches the terminal's program has no test of its own here, since a dialog holds focus while it is open and the terminal receives keys only while focused); "long announcements without links are keyboard-scrollable" waits for the shared dialog scroll stop (Deviations); "announcements wait for sign-in" is superseded (the desktop app never signs in); the automatic batch's cases (dismissal recording, waiting for Welcome and its feedback handoff, the preference decision, a failed shown write) wait for the automatic open, which comes with the Welcome card; no end-to-end leg, since the running app's helm has no announcement to show (its compiled release table is empty) and the bridge's fake fleet cannot queue one |
| `tests/keyboard-shortcuts.spec.ts` | 5     | the ${mac ? "Mac" : "non-Mac"} overview chord owns terminal input and labels | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `tests/sounds.spec.ts`             | 6     | device sounds                                                                | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `tests/notifications.spec.ts`      | 12    | session notifications                                                        | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |

## `tests/trash-dialog.spec.ts` and the trash cases of other specs, partly

The Trash button is `trash::TrashButton`, which the session list keeps and draws in its header, and the dialog it opens
is `trash::dialog`; the rules both use are `trash::listing`. This piece ports the button, its reads and rechecks, and
the dialog with each checkout's own Delete and Retry. The per-host and global confirmed deletes (Delete all and Delete
all on a host), the archive cue and the notices bell's Retry now and Open Trash come with their own pieces.

Replacing tests, in `crates/gui/farhelm-gui/src/trash/dialog/tests.rs` unless named otherwise (`trash/tests.rs` is the
button's, `trash/listing/tests.rs` the rules'):

- "trash list stays keyboard-scrollable while every list action is disabled", partly:
  `opening_measures_each_reachable_host_and_starts_on_footer_close` (every entry's Delete unavailable while the
  opening's measured reads are out, focus on footer Close, Tab to the header's close and then the first entry's Delete).
  The list's own tab stop while nothing in it can take focus is not ported (Deviations).
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

### Announcements

- **No keyboard stop for a long announcement without a link, for now.** The old dialog's body was its own tab stop
  ("Announcement content"), so the keyboard could scroll a long announcement that offers nothing to focus. The shared
  dialog frame has no such stop yet; one is being built for every dialog, and this dialog takes it up then.

### Trash

- **The archive list has no tab stop of its own.** While every control in the list is unavailable (a host still being
  read), the keyboard cannot reach the list to scroll it: the dialog frame has no stop for a scrolling body with nothing
  focusable in it yet, a foundation request.
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
- **The archive list's region is not named.** The old list was a scroll region named "Archived checkouts"; the dialog
  frame's body has no accessible name.
