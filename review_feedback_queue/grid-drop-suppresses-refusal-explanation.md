# Grid drops suppress the event that explains the refusal

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Dropping content onto a grid terminal cancels the drop without displaying the promised instruction to copy and paste
instead.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by application
source plus primary browser specification; no runtime reproduction.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/terminal.js:2717-2720.

terminal.js:2717-2720 sets dataTransfer.dropEffect to none in the grid dragover handler, while :2682-2692 emits the
refusal only from onDrop. The same immutable module explicitly documents at :2940-2942 that none cancels a drag without
a drop event; its page guard consequently uses copy for terminal panes (:2943-2946) so their explanation can run. That
guard cannot repair the grid case because the island already prevents dragover and the page handler returns on
defaultPrevented. e2e/grid-client/grid-attach.spec.ts:559-569 tests only a directly dispatched synthetic drop through
dispatchPayload (:515-518), bypassing native admission and thus missing the defect.

Dropping content onto a grid terminal cancels the drop without displaying the promised instruction to copy and paste
instead. Source: terminal.js:2717-2720 cancels dragover and assigns dropEffect none; the explanation is emitted only by
onDrop at :2682-2692. The document guard returns on defaultPrevented (:2943-2946), so it cannot repair the island's
decision. The same module explains the native-event distinction at :2939-2942. Primary documentation now independently
confirms it: HTML Standard, Drag-and-drop processing model,
https://html.spec.whatwg.org/multipage/dnd.html#drag-and-drop-processing-model, retrieved 2026-11-03, tool text lines
1166-1192: a cancelled dragover selects the current operation from effectAllowed/dropEffect; none takes the
failure/dragleave branch on release, while drop is fired only in the other branch. No browser scheduling or runtime
premise remains for the ordinary native-drop contract. The added regression
e2e/grid-client/grid-attach.spec.ts:504-518,559-569 directly dispatches a synthetic drop and bypasses dragover/native
admission.

Possible coverage and caveats: none exact. Pinned-tip SPEC.md Attachments :2392-2406 requires refusal with the
copy/paste message; SPEC_impl.md The grid client :1710-1735 describes shared interception and explicit drop refusal.
TODO.md Planned is empty (:23-25), while Near term :34-37 postpones accepting drops rather than explaining refusal.
TRIAGE_OUTCOMES.md drop-on-hidden-terminal-navigates-away.md :5731-5745 fixes hidden/unmounted pane handling, a
different trigger; it supports delivery of a refusal rather than accepting this omission. Searches across SPEC.md,
SPEC_impl.md, Planned, unlanded plans/queue, BUGS.md, full queue INDEX and TRIAGE_OUTCOMES.md found no exact acceptance,
scheduled fix or existing queue item. FILTER.md Rare, self-correcting glitches and imprecise diagnostics requires a rare
trigger; an ordinary drop does not establish one. Fix direction: preserve native delivery for the refused drop, or
display its guidance before cancellation.

Additional reviewed locations: g165/f3:
7509285e647ea86ccd2cee7905675dc87dad9b6f:e2e/grid-client/grid-attach.spec.ts:504-520,559-569.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g163/f1`, `g165/f3`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
7509285e647ea86ccd2cee7905675dc87dad9b6f; persists unchanged at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb
(git diff for terminal.js is empty).. This was a source-only review; runtime occurrence and frequency were not measured.
