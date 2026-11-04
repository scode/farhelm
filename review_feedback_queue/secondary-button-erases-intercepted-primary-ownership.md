# A second mouse button erases an intercepted primary link press's ownership

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Holding a web-link click and pressing a second mouse button loses the primary click's interception, so its subsequent
drag/release reaches the terminal program and its original link no longer opens.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by source and
standard mouse-event semantics. W3C Pointer Events 4 dated 2026-10-08, section 3.1.1.1
(https://www.w3.org/TR/2026/WD-pointerevents4-20261008/#chorded-button-interactions) states that the Mouse Event model
produces mousedown and mouseup per button; this code uses those events, not non-overlapping pointerdown/pointerup. No
runtime measurement or native default-menu behavior is claimed or needed for the shown handler route. Fix: independent
primary ownership through its matching release while preserving secondary reports.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1657-1718.

In the reviewed commit, a primary web-link press under tracking/default interception stores withheld=true at 1684-1698.
Press a second button before releasing the primary: link is null for a non-primary button, withheld is false, and
startGesture(state) at 1718 stops/replaces the existing gesture (1657-1661). Subsequent moves prioritize the held
primary button (grid-input.js:489-492 at a4f30ed8574d5e50f01771b90b1020b959f6acd4) but read withheld=false (1575-1576),
and its release is sent (1588). The original link is also lost. Preserve primary ownership independently through its
matching release.

The one gesture stores withheld at 1688-1698. Every reporting press calls startGesture(state) at 1718; startGesture
stops and replaces an existing gesture at 1657-1661. A secondary press is never withheld (link requires ev.button === 0
at 1684), so it replaces the held primary's withheld=true with false. Subsequent moves and primary release use only that
replacement's withheld at 1575-1576 and 1588. SPEC_impl.md:1657-1666 explicitly requires independent primary ownership
through release, including another button pressed during interception.

Holding a web-link click and pressing a second mouse button loses the primary click's interception, so its subsequent
drag/release reaches the terminal program and its original link no longer opens. Exact trigger: mouse-reporting program,
default intercept setting, hovered web link, primary press followed by another button press in the terminal before
primary release. Source: primary link press gets withheld=true, but non-primary link is null (tip:1690-1704), and every
reporting press replaces the singleton gesture (1663-1667,1724). Moves/release then use withheld=false (1581-1594);
grid-input.js:489-493 prioritizes the held primary bit. Caller terminal.js:5342-5348 supplies only interceptLinkPress,
and terminal-links.js:195-197 accepts only primary web presses; no second independent guard exists for the grid.
Supervisor grid_input.rs:672-729 encodes the received primary motion/release without requiring that it previously
encoded a down, so missing original down does not suppress the leak (choose ordinary Drag/Motion reporting with SGR).

Possible coverage and caveats: no exact coverage. Tip SPEC.md:2304-2331 requires withheld drag/release;
SPEC_impl.md:1645-1669 explicitly covers overlapping buttons. SPEC.md:3966-3969 accepts some pre-redraw actions but
preserves named guards and does not authorize losing press ownership through a held overlap. Planned bucket is empty
(TODO.md:23-25); targeted searches of plans/queue, BUGS.md, INDEX and TRIAGE_OUTCOMES found no interception match.
Existing queue secondary-press-suppresses-primary-selection-copy.md:1-31 and ledger TRIAGE_OUTCOMES.md:19387-19396
concern terminal.js clipboard copying, not forwarded grid input or lost link activation. Filters lack this exact
trigger/consequence.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g161/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f. Tip
persistence rechecked at ca9b0a19856dc136f63ad2d14f1076027edce1eb: grid-terminal.js:1581-1594,1663-1724.. This was a
source-only review; runtime occurrence and frequency were not measured.
