# Deleted reducer has distinct provenance and temporary-allocation consequences

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Deleted reducer has distinct provenance and temporary-allocation consequences

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g160/c3`.

### g160/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 18 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Original TLDR preserved: Considered whether deleting the
launch-error excerpt reducer and scrollback constants also removes useful diagnostics or shortens current terminals'
retained history. Original could-be-highest retained for actual terminal-history loss; no such loss is established from
this helper removal, and c1 separately preserves deliberate old-terminal loss. Established distinct consequences are
other: a misleading durable launch diagnostic and additional bounded temporary allocation, as already reported by
neighbour g100/c1 and g100/l1. Confidence: confirmed conditional source analysis, no execution. Proposed_drop: prior
blanket source refutation withdrawn for these distinct effects; no parent drop accepted. The narrower final-output cap,
final-exit ordering and surviving test-sensitivity checks in earlier events remain valid, but do not establish screen
provenance or bounded intermediate copying. Source checks: parent tmux.rs:191-299 reduces incrementally with bounded
line/whitespace suffixes and at most five lines; replacement launch_artifacts.rs:97-121 collects references to every
nonblank screen row and appends/escapes five whole rows before trimming to 500 bytes. This changes intermediate
allocation/work bounds even though the final result meets the same cap. The full-screen input extraction predates the
patch and is not this new cost; no OOM, measured latency or hard RSS-contract breach is claimed. For provenance,
snapshot/v1/mod.rs:242-268 encodes old primary history above a blank screen. vendor grid/resize.rs:25-28 and 46-71 grows
the visible grid by pulling available history rows; term/mod.rs:756-758 uses that path for the primary grid. The
published replacement host can receive an owned viewer resize (handlers.rs:2060-2082); if its shell subsequently exits
without consuming the spec and leaves an imported row visible, status.rs:642-652 calls the screen excerpt helper, whose
last-five-nonblank selection can quote that old row. Processed exit evidence establishes final output ordering, not
provenance of inherited cells. The comment at launch_artifacts.rs:44-56 claiming the blank new screen never quotes a
previous run is therefore too strong. Full-range attribution checked: exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a already has the same host-screen quoting path and blank-seed rationale at
launch_artifacts.rs:52-94; base-to-reviewed diffs of grid/resize.rs and term/mod.rs are empty. Thus do not attribute new
runtime contamination to this helper move. The bounded reducer-to-whole-row allocation change does occur in the assigned
removal/replacement commit. Contract: SPEC.md:2890-2893 requires omitting excerpts that cannot safely be attributed to
the launch. Possible_cover: known g100/c1 and g100/l1; parent owns consolidation rather than counting new discoveries
here. Material caveats: a height increase between replacement publication and failed-shell classification, retained old
history and an unoverwritten imported row are needed for the diagnostic route. No user-work loss or security route is
established by either distinct helper effect. The original potential retained-history-loss consequence and prior
proposed-drop bases remain preserved in the append-only history; their existence is not being used to dismiss these
different diagnostic/resource defects. Readily accessible source premises are settled; this ID is no longer an open
lead.

Returned proposal (claims-101, event 18): **retain-verified**, suggested bucket **other**.

The replacement adds screen-row indexing and whole-row escaped copies before truncation, where the old reducer retained
bounded suffixes. That in-range resource-bound change is source-established, with no demonstrated latency, OOM or hard
memory-budget breach. The separate inherited-history diagnostic route is conditionally established but already present
at the exclusive base. Earlier blanket refutation was withdrawn for both distinct effects; final-output and final-exit
ordering checks still stand. Parent should consolidate with its named excerpt records without counting new discoveries
here.

Original stronger consequence: Potential loss of optional launch diagnostics or retained terminal history after deleting
shared helpers. No modern-host retained-history loss was established. The historical highest possibility remains
preserved for independent checking, alongside deliberate old-terminal loss in g160/c1. Later effects are a misleading
durable launch diagnostic and additional bounded temporary allocation; neither establishes security or user-work loss.

Unresolved premise or limitation: Material user-visible delay, OOM or a hard memory-contract breach is not established.
The diagnostic route requires retained history, a height increase after replacement publication and before failed-shell
classification, an unconsumed spec and a still-visible inherited row. It is conditional source analysis, not a
reproduction.

Proposed basis: Retain the in-range intermediate-allocation change as other; preserve the conditional pre-range
provenance effect and the earlier highest-history-loss refutation for separate scope/drop judgment. No exact alias among
this batch or existing queue items was established.

Existing parent disposition (event 18): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g160 record](records/review-through-00de0b6c/groups/g160.json). Record ID: `c3`.
