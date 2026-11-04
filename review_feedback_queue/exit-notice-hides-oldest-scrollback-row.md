# The exit notice can hide the oldest scrollback row permanently

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Once an agent's program exits, scrolling to the beginning of its retained output can still leave the oldest row clipped
out of view.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by
source/layout arithmetic under the stated ordinary settled-fit premise; no runtime geometry measurement. No security,
permanent data deletion, user-owned-work loss or availability consequence alleged.

Source locations:
7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:493-527,808-839,848-881.

the new virtual viewport always paints model.rows lines from a top index clamped to
zero(viewShape493-498,topIndex525-527,scrollToIndex539-545,paint848-881). The existing exit-line layout808-839 reserves
its room by lifting screen with negative margin and applying clipPath inset(lift). On a fitted pane whose remaining
vertical space is under one cell, that clips the first painted row. layoutExit runs even when scrolled back and topIndex
is zero, so no earlier index can bring that first retained row into the visible region. A full-cell exit strip is normal
for a fitted pane, whose rows fill the available height.

Once an agent's program exits, scrolling to the beginning of its retained output can still leave the oldest row clipped
out of view. There is no earlier scroll position that exposes that row. The row remains stored and can still be included
in a full-buffer copy; this is a viewing defect, not lost history. Details/source basis: the introduced virtual viewport
uses model.rows as its visible row count (viewShape493-498,layoutRows578-593), paints those rows starting at the virtual
top (paint848-881), and clamps topIndex/scrollToIndex to zero(525-527,539-545). The preexisting exit layout at808-839
lifts the entire screen by whole cells and applies clipPath inset(lift) to make a cell of room for the exit notice. It
runs unconditionally at the end of paint, including while viewing old history. In an ordinary fitted pane below
the1000-row cap with at least two rows, proposeSize220-224 chooses floor(availableHeight/cell.height); fit439-459
subtracts padding but no exit line. Thus less than one cell remains below those rows, so layoutExit826-833 clips at
least the first painted row. With historyTrimmed=false, the first virtual row at topIndex0 is the oldest retained
history row, not a synthetic trim notice. Because topIndex cannot become negative, it never moves lower into the visible
area. The screen container and rows clip overflow(T app.css:8179-8206), so native scrolling of that DOM does not recover
it. The exit-line lifting itself predates this patch; the defect is its interaction with the newly introduced virtual
history viewport, which still counts the clipped cell as visible. A suitable repair can reserve that cell in viewport
accounting or avoid fixing the exit notice beneath a scrolled history view, while preserving access to every retained
row.

Possible coverage and caveats: R layoutExit's comment809-814 deliberately lifts the latest program screen to make room
for the notice, but does not explicitly accept making the oldest history row permanently unreachable once virtual
scrollback exists. T SPEC.md:1590-1595 says an exited agent stays viewable with scrollback;2122-2138 defines viewable as
what the terminal holds and preserves that history after exit. T SPEC_impl.md:2466-2470 places program exit over its
final screen; it does not waive history accessibility. T TODO.md Planned23-25 is empty. Searches across SPEC.md,
SPEC_impl.md, TODO.md, unlanded plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md for exit notice/line, oldest
output, clipped history and viewability found no exact matching acceptance, plan, known bug, queue item or ledger
decision. The rare-glitch filter does not fit a repeatable oldest-row omission persisting throughout the exited state;
the dialog-scroll filter concerns reachable dialog content, not a terminal row with no available earlier scroll index.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g160/f5`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f (R);
persistent at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (T), same relevant line ranges.. This was a
source-only review; runtime occurrence and frequency were not measured.
