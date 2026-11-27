# The close-tab shortcut acts on the tab chosen now rather than the tab the user was shown

Reviewed commit: a624f5f80fbab8188716aa337403d96d2da80b04

## TLDR

With Skip session confirmations on, Cmd+W (Mac) or Ctrl+Shift+W (Linux) can kill a shell tab the user was not looking
at. If the shown tab changes in the moment before the key is handled (the shown tab's shell exits or another client
closes it and the view falls back to the tab on its left, or a remembered tab is restored as the tab list arrives), the
shortcut closes the newly chosen tab at once, ending its shell and every process it started, though the screen the user
acted on showed a different tab or the main terminal.

## Details

`TerminalSet::close_shown` (crates/gui/farhelm-gui/src/terminal/set.rs:1383-1406 at the tip) takes its target from
`this.chosen`, the live choice, and the label from the live `tabs` index; only the anchor comes from the last drawn
strip (`close_anchors`), and that map holds every listed tab, so it does not tell which one was shown. It then calls
`ask_to_close`, which with skipping on calls `close_tab` immediately (set.rs:307-320). `chosen` changes without a user
action in three places: `close_tab`'s settle callback (set.rs ~1436-1442, fallback to `previous_surviving_tab`);
`follow_tabs` when a reaped or externally closed shown tab drops from the pushed row (set.rs ~791-797; SPEC.md, "Session
view": "Closing the active shell tab, including when its shell exits or another client closes it, selects the nearest
surviving tab to its left"); and the remembered-tab restore when the first row arrives and the user has not pressed
anything yet (set.rs:839-852). Each notifies; GPUI then draws the dirty window before dispatching a queued key but shows
that frame only at the next frame, so the key acts on a choice the user has not seen. SPEC_impl.md, "GUI: Dioxus" ("The
same rule starts at the control that asks the question or sends the request directly. Its handlers carry the drawn
session key, row, skip preference ... GPUI can draw a dirty frame before dispatching a key, so a handler for a newly
drawn frame may have to refuse the press") and SPEC.md's Skip session confirmations paragraph ("with consent from the
state shown by that render") require the shown target. The × handler carries its drawn id and label (set.rs ~1060-1078).
The Mac Cmd+W's Delete fallback goes through `header_action`, which uses "the header's current render, subject to the
same presentation check as its button" (session_pane/header.rs:772-783). So the tab path is the one that reads live
state. On the Mac the choice between closing a tab and deleting the session is also made from the live `chosen`. The
Delete branch stays safe because the header binds to its render; the close branch does not. Without skipping, a question
titled with the live tab's name appears under its ×, so the user still sees it and can cancel. Fix: record the chosen id
(and its label) at draw time beside `close_anchors`, admitted only once that frame is shown, as the header's
`ShownAction::shown` is. `close_shown` should act on that recorded id and refuse when the live choice disagrees.

Reviewer revision notes: Revision (synthesis turn): re-checked against the restore path and SPEC_impl.md; sharpened the
mechanism (GPUI draws the dirty frame before dispatching the key, so the gap is drawn-but-not-shown, the exact case
SPEC_impl.md names) and the restore route (set.rs:839-852 restores the remembered tab only while the user has not
pressed anything, so the first key after the row arrives is the one exposed). Conclusion unchanged.

### Triage notes

- Recorded bucket: `highest`. Recorded as highest, as the reviewer suggested: ending a shell tab's processes the user
  did not choose is process loss, and the route (the shown-tab choice changing on its own, with a key handled before
  that frame is drawn) is established in source. It moves to high if triage judges a single-frame window not to
  establish the route.
- Suggested bucket (reviewer): highest. Killing a shell tab's processes the user did not choose to end is process loss,
  and the route (a choice change the model makes on its own, followed by a key dispatched before that frame is shown) is
  established in source and named by SPEC_impl.md. It drops to high if the commissioner judges that a single-frame
  window does not establish a route. Requires Skip session confirmations on.
- Confidence: likely. Source establishes that `close_shown` reads the live `chosen` and that `chosen` changes without a
  user action. The premise not executed is a key arriving inside that frame window.
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g09/f2 (finding), group g09 "Open and close terminal tabs with Cmd+T and Cmd+W", findings log line 5.
