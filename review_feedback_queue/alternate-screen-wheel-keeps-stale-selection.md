# Alternate-screen wheel input bypasses selection dismissal

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

After selecting text in a full-screen terminal program with mouse reporting off, wheel navigation leaves that local
selection highlighted.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static route
and parity difference; runtime frequency/program-specific rendering not measured. No security, loss of user-owned work,
availability failure or harmful-input consequence asserted.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1736-1740.

T grid-terminal.js:1742-1746 sends alternate-screen mouse-off wheel messages through sendMessage, bypassing
sendUserInput1408-1413 which clears selection. T grid-input.js:591-599 explicitly routes alternate-screen wheel events
to the program as arrow-key input. R grid-terminal.js header85-88 and sendUserInput1396-1407 establish clearing
selection for program input. Static official xterm6.0.0 CoreBrowserTerminal.ts lookup shows alternate-buffer wheel path
calls coreService.triggerDataEvent(sequence,true) at839, the same user-input route used by keys; exact surrounding
source and selection-service subscriber are being checked next.

After selecting text in a full-screen terminal program with mouse reporting off, wheel navigation leaves that local
selection highlighted. If the program redraws in response to the arrow input, the selection now covers changed cells and
a later Copy can return their new contents rather than dismissing the selection as the old terminal does. Details/source
basis: R grid-terminal.js:85-88 says input sent to the program clears local selection. The introduced sendUserInput
at1396-1407 implements that rule for keys/text/paste and mouse reports, but the alternate-screen mouse-off wheel
branch1736-1740 calls sendMessage directly. At T the corresponding functions are1398-1413 and1742-1746. sendMessage only
sends; it never calls clearSelection. T grid-input.js:591-599 sends wheel input to the program whenever alternate-screen
mode is set. T crates/farhelm-supervisor/src/grid_input.rs:666-687 confirms that a wheel step with mouse reporting off
and alternate screen on is always encoded as an up/down arrow (regardless of mode1007), so this is real program input,
not a no-op local scroll. Ordinary local selection is enabled in this mouse-off state; a program response that only
redraws rows does not meet noteDamage897-903's resize/history-reset/alternate-switch clearing conditions. T
grid-terminal.js:1493-1496 Copy reads the still-present selection through selectionText1014-1018 over the current model.
Exact parity check: official xterm.js6.0.0 tag resolves to commit f447274f430fd22513f6adbf9862d19524471c04. At that
immutable revision, src/browser/CoreBrowserTerminal.ts:806-841 converts alternate-buffer mouse-off wheel steps to arrows
using coreService.triggerDataEvent(sequence,true). src/common/services/CoreService.ts:61-81 emits onUserInput for that
true argument; src/browser/services/SelectionService.ts:135-143 subscribes to it and clears a nonempty selection. These
three source files were retrieved statically, read in bounded excerpts, and compared byte-for-byte with their
exact-commit retrievals. Thus preserving the local selection is a source-confirmed behavior difference from the
unmodified xterm6 terminal, not an inferred native-input timing issue.

Possible coverage and caveats: none exact found. T SPEC_impl.md:1297-1305 promises that the grid reproduces the old
terminal behavior with documented file-header departures; the R header85-88 promises clearing on program input, so this
is not such an accepted departure. T SPEC.md:2285-2303 describes local selection/copy and its clipboard-platform
best-effort exception, which does not accept retaining a stale local selection for this input path. T TODO.md
Planned23-25 is empty. Searches of SPEC.md, SPEC_impl.md, TODO.md, unlanded plans/queue, BUGS.md, queue INDEX and
TRIAGE_OUTCOMES.md found no alternate-screen-wheel selection exception or planned repair. Existing queue
classic-mouse-snapshot-copy-blocks-later-links.md(1-34) concerns copied selection blocking later web links on a detached
legacy mouse-reporting xterm snapshot; secondary-press-suppresses-primary-selection-copy.md(1-31) concerns
overlapping-button copy ownership; neither has this full-screen mouse-off wheel trigger and retained live-selection
consequence. TRIAGE_OUTCOMES.md:7333-7353 fixes a new plain click recopying a legacy mouse selection; it does not cover
wheel input retaining this grid selection. The rare-glitch/subsecond filters are not an established match: this is a
deterministic ordinary wheel path with no narrow timing, crash, unsupported input or peer failure.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g160/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f (R).
Persistent at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (T), grid-terminal.js:1742-1746.. This was a
source-only review; runtime occurrence and frequency were not measured.
