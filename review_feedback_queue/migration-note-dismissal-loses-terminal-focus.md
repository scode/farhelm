# Dismissing the migration note drops keyboard focus outside the terminal

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

After closing the old-terminal note from the keyboard, ordinary typing no longer reaches the already-revealed terminal
until the user refocuses it.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by source and
standard semantics, not by runtime measurement. This revision supersedes the earlier unverified-after-investigation
label; actual engine behavior was not executed, but no unavailable source premise remains for the ordinary keyboard
route.

Source locations: 4ed000fb454cebd9c22a88f358a93e15566b8c9e:crates/farhelm-ui/src/migration.rs:263-266,290-294,312-316.

dismiss only sets the dismissal signal; the next render returns an empty element, removing the activated dismiss button,
with no focus return in this component.

After closing the old-terminal note from the keyboard, ordinary typing no longer reaches the already-revealed terminal
until the user refocuses it. Trigger: reach the note's dismiss button by keyboard with a terminal already revealed, then
activate it. The click only updates dismissal state and the next render removes the focused button. Neither it nor the
remaining terminal schedules a handoff. Source basis at pinned tip: terminal.js:3240-3246 focuses only on a
selected-element change; dismissal does not change selection or the terminal_specs memo (session_view.rs:1604-1645).
Grid keyboard/input handlers belong to its input element, not document/body (grid-terminal.js:1437-1462). Band-induced
resize only refits (terminal.js:6075-6108); grid fit itself measures and reports size without focusing
(grid-terminal.js:432-460). Dependency verification: pinned Cargo.lock:1273-1276,1583-1586,1668-1671 selects Dioxus
0.7.10. Local source for that exact version dioxus-web/src/mutations.rs:145-150,249-254 forwards replacement/removal;
dioxus-interpreter-js/src/unified_bindings.rs:56-66 calls root.replaceWith/node.remove with listener cleanup and no
focus restoration. Primary HTML Standard infrastructure section DOM trees, removing steps
(https://html.spec.whatwg.org/multipage/infrastructure.html#html-element-removing-steps; consulted 2026-11-03, displayed
lines 134-143) synchronously changes a removed focused area's document focus to its viewport; interaction Processing
model lines 1250-1256 routes keys to the focused area's anchor. These static sources resolve the original missing
focus-destination premise.

Possible coverage and caveats: no exact coverage found. At pinned tip SPEC.md:2221-2238 and SPEC_impl.md:2483-2502
define migration dismissal without accepting focus loss; TODO.md:23-25 Planned is empty.
Notification-bell-removal-skips-focus-return and terminal-focus-origin-overwrites-header-focus queue items have
different controls and triggers. TRIAGE_OUTCOMES.md:16853-16865 covers notification-list dismissal only. No
unlanded-plan or BUGS item removes this cause. Neither rare-self-correcting nor subsecond-input FILTER applies to the
ordinary keyboard dismissal route. Proposed fix: a post-unmount handoff to the selected revealed terminal, preserving
newer focus owners; browser focus/input checks can validate the repair.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g103/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 4ed000fb454cebd9c22a88f358a93e15566b8c9e. Tip
persistence verified at ca9b0a19856dc136f63ad2d14f1076027edce1eb: migration.rs:293-296,320-324,346-350.. This was a
source-only review; runtime occurrence and frequency were not measured.
