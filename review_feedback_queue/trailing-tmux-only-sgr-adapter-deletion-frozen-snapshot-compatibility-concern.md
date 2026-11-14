# Frozen snapshot compatibility concern remains source-refuted, pending independent check

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the assigned deletion removes the tmux-only sgr_setting string adapter and its re-export, not
pen_changes, paint_history, container, decode, or any shipped wire field. Initial exact source evidence remains
snapshot/v1/paint.rs:694-743 (pen_changes), snapshot/v1/mod.rs:246-269 (encode_history), snapshot/mod.rs:573-587 (v1
restore dispatch), and the assigned 00962/00963 patches. New interaction check: paint_history at v1/paint.rs:400-437
directly paints existing primary grid rows and resets blank-screen cursor/pen/REP; it never calls the retired adapter.
New caller check: core.rs:12489-12493,13000-13029 and host_restart.rs:199-258 carry the host encoder’s blob into the new
host via host_launch.rs:509-518 ChildSpec.initial_snapshot. New test-sensitivity check, independently of production
correctness: snapshot_golden.rs:426-447 chooses both full and limited history_seed encodes; 576-628 compares restoration
with committed state descriptions and current encoding with fixed blob bytes; 684-703 separately requires nonempty
history, partial trimming, exact restored history count, trimmed mark, blank screen, empty title and bracketed paste
off. Thus both encoding bytes and the seed’s observable contracts still have distinguishing assertions. The removed
tests exclusively invoke removed plain-line APIs and do not supply the surviving golden compatibility checks; f4 records
the separate test-responsibility assessment. Contract: SPEC_impl.md:2827-2879 requires every shipped snapshot format
stay readable forever. Suggested bucket: potentially highest if the original compatibility-loss p

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g184/f2`.

### g184/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Original considered wording preserved verbatim: deleting an
exported helper from the frozen v1 snapshot module might change saved terminal restoration or the bytes written by
supported host snapshots. Potential consequence preserved: incompatible restoration could make user-owned retained
terminal history unavailable. This record was promoted from candidate to finding in this synthesis turn so the later
independent check receives that consequence even though the source route is refuted. TLDR: the assigned deletion removes
the tmux-only sgr_setting string adapter and its re-export, not pen_changes, paint_history, container, decode, or any
shipped wire field. Initial exact source evidence remains snapshot/v1/paint.rs:694-743 (pen_changes),
snapshot/v1/mod.rs:246-269 (encode_history), snapshot/mod.rs:573-587 (v1 restore dispatch), and the assigned 00962/00963
patches. New interaction check: paint_history at v1/paint.rs:400-437 directly paints existing primary grid rows and
resets blank-screen cursor/pen/REP; it never calls the retired adapter. New caller check:
core.rs:12489-12493,13000-13029 and host_restart.rs:199-258 carry the host encoder’s blob into the new host via
host_launch.rs:509-518 ChildSpec.initial_snapshot. New test-sensitivity check, independently of production correctness:
snapshot_golden.rs:426-447 chooses both full and limited history_seed encodes; 576-628 compares restoration with
committed state descriptions and current encoding with fixed blob bytes; 684-703 separately requires nonempty history,
partial trimming, exact restored history count, trimmed mark, blank screen, empty title and bracketed paste off. Thus
both encoding bytes and the seed’s observable contracts still have distinguishing assertions. The removed tests
exclusively invoke removed plain-line APIs and do not supply the surviving golden compatibility checks; f4 records the
separate test-responsibility assessment. Contract: SPEC_impl.md:2827-2879 requires every shipped snapshot format stay
readable forever. Suggested bucket: potentially highest if the original compatibility-loss premise held. Confidence:
source-refuted concern, with no execution-based assertion. proposed_drop: no surviving format or caller path depends on
the deleted helper, and surviving tests still pin both sides of that compatibility boundary; pending independent check,
not accepted here. possible_cover: none. Could-be-highest: yes, for the preserved hypothetical retained-history
consequence. No open premise or reproduction remains for this record.

Returned proposal (claims-115, event 10): **refuted**, suggested bucket **other**.

The actual deletion does not change the surviving version-1 encoder, decoder, wire fields or host-seed call chain.
Preserve the promotion from candidate to finding and the hypothetical loss-of-retained-history consequence for
independent checking; this is a proposed source refutation, not an accepted drop.

Original stronger consequence: Deleting an exported helper from the frozen v1 snapshot module might change saved
terminal restoration or bytes written by supported host snapshots; incompatible restoration could make user-owned
retained terminal history unavailable. Line 7 explicitly promoted the earlier candidate so this stronger concern
receives independent checking.

Unresolved premise or limitation: The hypothesized path from deleting this tmux-only adapter to changing a surviving
shipped snapshot byte stream is absent in the reviewed diff. Runtime compatibility was not tested and broad
compatibility perfection is not claimed.

Proposed basis: Narrow source refutation of this deletion causing frozen-format incompatibility. Bucket other reflects
no established adverse route; potential highest consequence remains attached for the independent drop/lowering check.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g184 record](records/review-through-00de0b6c/groups/g184.json). Record ID: `f2`.
