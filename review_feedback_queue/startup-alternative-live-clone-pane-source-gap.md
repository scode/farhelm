# Alternative live-clone pane protection remains unresolved

Reviewed commit: faadf20b37130fe659b9f3498e0cff20d69ad48c

## TLDR

Startup inspects one eligible pane's liveness. No ordinary supported producer was found for a pane-less row with a
selected dead pane and another eligible live clone; the possible destructive route remains an incomplete source lead.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-supervisor/src/service/core.rs:6923`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Synthesis revision, same local ID; reviewed faadf20b37130fe659b9f3498e0cff20d69ad48c, coverage pinned at
463cac1528b5c5e259404975380d321a3fff0412. TLDR remains hypothetical: startup might remove a checkout still being
prepared if a pane-less row resolves to a different dead pane while another eligible pane runs its clone.
Correction/clarification: this is an incomplete source investigation, not a source-established conditional bug and not a
new independently actionable finding. Missing SOURCE premise remains the shipped supported producer of that exact
row/pane arrangement. a045's actual synthesis and discovery progress explicitly establishes no such producer.
Deterministic selection refutes only multiple-candidates-return-None, not alternative-pane liveness. Pinned
TRIAGE_OUTCOMES.md:18535-18554 is the earlier forged duplicated-codec-row record: its trigger is deliberately forged
duplicate facts, fixed by strict-listing uncertainty, not an ordinary alternative live pane. Pinned
TRAILING_REVIEW.md:170-173 preserves the historical forged-pane uncertainty and explicitly says no accidental producer,
security consequence or existing-user-work loss was established. The file is absent from pinned INDEX; no existing queue
item is removed or declared duplicate by this event. SPEC.md:3501-3512 excludes direct private-server interference but
does not establish a supported producer; SPEC_impl.md:2489-2496 requires live/uncertain claim retention. Suggested
bucket other on established evidence, potentially highest only if a supported producer and user-work-loss route are
supplied. Confidence: missing source premise, not likely confirmed mechanism or unknown runtime frequency.
possible_cover: prior forged-pane/duplicate-codec record is neighboring context, not matching trigger coverage for this
hypothetical ordinary arrangement. proposed_drop:false; retain explicit incomplete gap without promoting it to a new
verified bug. Other narrow exclusions remain unchanged; no nominal discovery pass will be claimed over this gap.

Provenance: `a075/a045-live-clone-other-pane`. These are reviewer-local identities, not additional findings. Proposed
drop: none for the retained remainder.
