# Primary-selection writes cannot suppress drag-copy guidance (predates range)

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: a program that writes only its primary selection can still receive guidance that it copied
nothing. Source confirms the mismatch, but both behavior and the primary-write promise already exist at the review's
exclusive base. Details at reviewed revision: effects.rs:180-181 preserves primary effects; hosts/terminal.rs:3310-3321
broadcasts only ClipboardTarget::Clipboard; service/handlers.rs:1907-1915 supplies that subscription to the viewer;
grid_stream.rs:844-849 independently retains only clipboard writes and :786-788 emits that target. Thus
terminal.js:4326-4330 cannot increment gridOsc52Writes for primary writes and :4426-4431 permits the false notice.
Current contract SPEC.md:3074-3075: 'Any completed program clipboard write, including a primary-selection write,
suppresses it during the grace window.' Actual full-range checks: at exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a, SPEC.md:3055-3057 already says 'any completed program clipboard write,
including a primary-selection write, suppresses it during the grace window'; SPEC_impl.md:1833-1835 already describes
the grid counting callbacks for either target. Base hosts/terminal.rs:3210-3221 has the same clipboard-only broadcast,
base grid_stream.rs:845-850 the same target filter, and base terminal.js:6012-6016/6113-6124 the same delivered-write
count and grace decision. Therefore neither the promise nor this grid-path omission is newly introduced anywhere in the
pinned range. This is stronger attribution evidence than the earlier immediate-parent comparison. Suggested bucket:
other for misleading guidance; could

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g186/f1`.

### g186/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Linked identity: distinct primary-guidance cause split from
prior c2 events at caller request; c2 retains the independent highest/security clipboard-confidentiality candidate and
source refutation. TLDR: a program that writes only its primary selection can still receive guidance that it copied
nothing. Source confirms the mismatch, but both behavior and the primary-write promise already exist at the review's
exclusive base. Details at reviewed revision: effects.rs:180-181 preserves primary effects; hosts/terminal.rs:3310-3321
broadcasts only ClipboardTarget::Clipboard; service/handlers.rs:1907-1915 supplies that subscription to the viewer;
grid_stream.rs:844-849 independently retains only clipboard writes and :786-788 emits that target. Thus
terminal.js:4326-4330 cannot increment gridOsc52Writes for primary writes and :4426-4431 permits the false notice.
Current contract SPEC.md:3074-3075: 'Any completed program clipboard write, including a primary-selection write,
suppresses it during the grace window.' Actual full-range checks: at exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a, SPEC.md:3055-3057 already says 'any completed program clipboard write,
including a primary-selection write, suppresses it during the grace window'; SPEC_impl.md:1833-1835 already describes
the grid counting callbacks for either target. Base hosts/terminal.rs:3210-3221 has the same clipboard-only broadcast,
base grid_stream.rs:845-850 the same target filter, and base terminal.js:6012-6016/6113-6124 the same delivered-write
count and grace decision. Therefore neither the promise nor this grid-path omission is newly introduced anywhere in the
pinned range. This is stronger attribution evidence than the earlier immediate-parent comparison. Suggested bucket:
other for misleading guidance; could-be-highest: no established security or loss-of-work route for this distinct defect.
Confidence: confirmed source analysis for the contract mismatch and base-range provenance; no execution. proposed_drop:
proposed scope exclusion only, because the independently found grid-path behavior and its contradictory promise both
predate the exclusive base; not a claim that behavior meets the contract, and not an accepted parent drop or lowering.
possible_cover: caller-supplied exact g188/c3 event 6 describes the same mismatch for consolidation; no prohibited
coverage sources searched. Caveats: removal's old xterm path counted primary OSC 52 writes directly, but
terminal-host/grid sessions already had this omission before the range; do not label this a newly added filter or a
newly introduced documentation promise. Fix direction if taken separately: deliver a primary-write notice for the count
without writing its text to the viewer system clipboard. All prior c2 primary-guidance consequences/evidence are
preserved by this linked record.

Returned proposal (claims-117, event 12): **out-of-range**, suggested bucket **other**.

The misleading guidance mechanism is real by source, but both the omission and its contradictory promise already exist
at the exclusive base. This is a proposed range exclusion, not a claim that the behavior meets its contract.

Original stronger consequence: The distinct f1 consequence remains other: false program-handles-selection guidance after
a completed primary write. Earlier c2 histories found this while investigating a highest clipboard-confidentiality
candidate, but the latter remains separately preserved under c2 and must not disappear through this split or scope
exclusion.

Unresolved premise or limitation: No missing premise for base provenance or the conditional source mechanism. Actual
occurrence/frequency was not measured. Whether to record a pre-existing issue beyond this trailing-review range is the
parent's scope decision.

Proposed basis: Exclude this exact pre-existing grid-path mismatch from findings introduced in the pinned range,
preserving its contract conflict and c2 history. Conservative independent-check flag retains the earlier
security-related identity split; no independent drop approval is supplied here.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g186 record](records/review-through-00de0b6c/groups/g186.json). Record ID: `f1`.
