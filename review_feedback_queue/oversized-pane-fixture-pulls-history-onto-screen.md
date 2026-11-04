# Oversized-pane migration fixture moves its short scrollback onto the visible screen

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The oversized-terminal regression claims to preserve existing scrollback but moves its entire short scrollback onto the
visible screen before Restart, so it misses the positive-scrollback case it says it exercises.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static fixture
and pinned dependency behavior; no runtime measurement needed for this deterministic transition.

Source locations: fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm/tests/e2e/host_migrations.rs:762-772.

host_migrations.rs:84-105 prints sixty numbered lines and a final mark; old_session creates at 80x24 (:237-238) and
asserts history_size>0 only before resizing (:254-258). The test grows the pane to 10000x400 (:762-767). Its post-resize
premise (:768-772) uses pane_text, which runs capture-pane -p -S - with no end bound (:171-175), so it includes the
visible screen as well as scrollback. Official pinned tmux 3.7c source in the parent's read-only tmux-3.7c tree
establishes the actual transition: cmd-resize-window.c:109-112 selects manual size and immediately recalculates;
resize.c:400-407,26-53 and layout.c:615-664,358-407 propagate it to window_pane_resize; window.c:1194-1215 calls
screen_resize. screen.c:338-369 resizes height before reflow; :450-470 pulls up to the 376 extra rows from hscrolled,
decrementing hsize, while grid.c:432-444 increments both counters for ordinary scrolled output. All sixty short fixture
lines fit on the 400-row screen; widening does not wrap them, and grid_reflow (:1434-1512) leaves history zero.
cmd-capture-pane.c:192-223 confirms -S - includes all rows through the visible screen by default, so the premise passes
on those visible lines. The test still checks the oversized default-size fallback and retention of the old window; it
does not establish retention of nonempty scrollback at that boundary.

Possible coverage and caveats: none found. Searched SPEC.md, SPEC_impl.md, TODO Planned, unlanded plans/queue, BUGS.md,
queue INDEX/text and TRIAGE_OUTCOMES.md for the exact fixture, oversized-pane, dimensions, resize/history and
history_size cases. SPEC_impl.md:2195-2213 and SPEC.md:1389-1394 distinguish scrollback from the visible screen and
require the oversized fallback; they do not accept this test-premise gap. Planned is empty; TODO Near-term
hidden-terminal work at :181-213 is different. No matching plan, BUGS entry, queue item or ledger decision found.
TRIAGE_OUTCOMES.md:11976-11986's new-production-hook discard does not match: this fix needs only test fixture output and
its existing helper. No FILTER.md filter accepts this ordinary missing fixture premise.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g191/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is fba73468a9cb09c217a39da2be7f2c7cea064cb8;
host_migrations.rs is unchanged through pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (git diff is empty).. This
was a source-only review; runtime occurrence and frequency were not measured.
