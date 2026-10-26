# Restart source reseeding retains a conversation selector

Reviewed commit: b37d328ce973442d007ce518b69a26d8a14ebec7

## TLDR

Restart with retains an entered conversation when its source is reseeded. A shipped source-switch action after editing,
and a harmful resulting restart, remain unestablished; the older Clone/Replace discard does not explicitly settle this
new surface.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/create_form/draft.rs:574-577,698-706,2670-2676`. Locations and revisions in
the assessment below identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit b37d328ce973442d007ce518b69a26d8a14ebec7. Synthesis reconciles g250/g250-synthesis-1.result.md: AGREE
with its narrow ordinary-route refutation, but no whole-finding exclusion. The sole identified RestartWith producer
remains the modal-inert header (session_view.rs:2201-2219); native Cmd+N explicitly returns for a mounted ARIA modal
(lib.rs:1541-1556). Cancel, successful restart and New toggle-close remove the form
(list/view.rs:4134-4160,4194-4220,4247-4262), so an ordinary next opening gets a new conversation state. Reseeding the
same mounted form still preserves conversation and restart_edit sends nonempty selectors. Missing SOURCE premises remain
(1) a shipped unusual queued-event route that switches distinct edited restart sources without unmounting and (2) a
harmful wrong-conversation action despite the selector remaining visible/editable. These are not merely unmeasured
runtime frequency. Established bucket OTHER; potentially highest only once harmful route is shown. possible_cover: older
Clone/Replace reseeding discard at pinned TRIAGE_OUTCOMES.md:18380-18390 is not acceptance for the later Restart
surface. proposed_drop: none for whole candidate; ordinary-route allegation only is source-refuted. Confidence
unresolved lead with explicit missing source premises. No concrete further source question identified to settle unusual
delivery, so affected investigation stops incomplete; no nominal empty discovery search or experiment performed.

Provenance: `a037/g250-f4`, `a073/a036-d1`. These are reviewer-local identities, not additional findings. Proposed drop:
none for the retained remainder.
