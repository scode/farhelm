# Claude's planned abort reset removes an established report pin

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

## TLDR

The planned reset can admit a delayed child report after a failed Restart stops the correcting parent. Later
specifications explicitly document pin removal after abort, but acceptance of this whole consequence is ambiguous. This
reviews the plan, not the later shipped implementation.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `plans/queue/claude-wrapper-capture.md:74-79,118-123,146-154`. Locations and revisions in the
assessment below identify the reviewed source; later main is outside this review's pinned range.

Synthesis of reviewed commit 463cac1528b5c5e259404975380d321a3fff0412, not a review of later implementation. Original
producer prose: a055 'Aborted relaunch leaves Claude unpinned'; g131 'Pin survives relaunch or conflicts with retained
capture', only its distinct unpin-loss interpretation. Original narrow exclusion remains DISAGREE; literal pin retention
remains refuted. Later recording-coverage verdict AMBIGUOUS: 11ce27e971f0892fe6d3e1840a1ca8a2b9bb42d4 SPEC.md:2313-2332
expressly describes Claude selecting reports, first-report pins, ancestor correction, and abort not restoring the pin
(2325-2327); SPEC_impl.md:2918-2936 and 3055-3080 expressly apply the shared pin reset to Claude (3071-3074). This
clearly covers the intentional reset mechanism. It does not expressly identify the combined trigger of a report left
after the decision-time drain, the old parent subsequently stopped, a definitive failed spawn restoring the old pane,
and the delayed child report replacing an already-valid Resume target without a surviving correcting parent. The
adjacent accepted gap is framed as a lost parent's report / upgrade and subsequent ancestor reporting, not removal of an
established pin after the tree is stopped. It may have been intended to accept all first-report consequences after
abort, but that intent is not unambiguous from these sections, so no full-consequence exclusion is approved.
Reviewed-source route remains core.rs:11137,11348-11397 (drain then stop); hook.rs:337-350 (recorded evidence survives);
store.rs:4145-4155,4238-4245 (clear pin and restore pane without it); core.rs:11633-11635,11723-11730,11802-11812
(definitive abort and republication); core.rs:15764-15778 / procs.rs:478-513 (dead-pane recorded-chain anchor);
report_files.rs:319-332 (current generation used). TLDR: the planned reset can let a delayed nested report replace
Resume after the parent was stopped. Suggested bucket OTHER, as parent provisionally retained: a conditional plan
concern with no shipped-Claude-regression claim. Confidence confirmed source composition under the plan's specified
future first-report admission; actual occurrence unmeasured. Missing source premise for any claim about the later
shipped implementation: its admission/control-flow was not reviewed and is outside this assigned range. possible_cover
the named later specification paragraphs, partial for reset and ambiguous for full harm; pinned plan child-first
acceptance 118-122 and Codex-only ledger 16086-16097 do not explicitly settle this distinct trigger. proposed_drop no.
No runtime observations or later-code safety claims inferred; existing events preserved.

Provenance: `a077/f1`, `g131/f3`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
