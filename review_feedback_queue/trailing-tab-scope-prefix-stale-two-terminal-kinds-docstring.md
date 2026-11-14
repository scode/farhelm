# Tab scope-helper documentation retains a removed two-kind responsibility

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: changed internal documentation says the tab scope helper serves two kinds of terminal tab,
although only terminal-host tabs can be opened. This is an intermediate false responsibility map for maintainers, not an
established runtime defect.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g113/l4`.

### g113/l4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: changed internal documentation says the tab scope
helper serves two kinds of terminal tab, although only terminal-host tabs can be opened. This is an intermediate false
responsibility map for maintainers, not an established runtime defect. Details: core.rs:13807–13813 retains 'Shared by
both kinds of tab' while its assigned hunk replaces the old two-kind marker explanation with host-tag storage. Parent
core.rs:14605–14612 explicitly identifies the two kinds as tmux and host, refuting a possible scoped/unscoped
interpretation. Pinned git grep finds only the helper definition and caller core/host_tabs.rs:405; that caller at
:396–420 always opens a terminal host. SPEC_impl.md, Every new terminal in a host and Tabs in hosts, :2236–2253 and
:2311 onward specifies the sole path. The exclusive range base 6e423e30068b62c6cfa276c6d720ee35bb70387a still has the
tmux caller at core.rs:15954; in-range 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513 deletes open_tmux_tab and that caller.
Thus this description becomes stale within the pinned range; it does not predate the entire review. The assigned
driver-removal commit partly updates the same docstring without removing the stale responsibility. Later correction
verified exactly: git log -S identifies 08399d092d20815ced6619c26df2391944ae2b4a, whose core.rs diff at old :13760
removes 'Shared by both kinds of tab' and accurately describes the sole host path. The scoped diff to pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db includes that correction. No endpoint defect is alleged. Suggested bucket:
other/documentation correctness. Confidence: confirmed source/caller/range-history analysis, no runtime observation or
reproduction. Could-be-highest: no security or user-work-loss route established or suspected from this wording.
proposed_drop: false for this intermediate documentation error; later correction is historical disposition evidence, not
a refutation that it was false at the reviewed revision. possible_cover: no duplicate in the supplied neighbour records;
their stale-comment findings concern different symbols/contracts. Caveat: narrow responsibility-description error only;
no claim that actual scope selection, provenance, cleanup or runtime behavior is wrong. Revision resolves the last
history detail by naming the exact later correction; original consequence, bucket and proposed-drop basis unchanged.
Lead l4 is closed as this finding with no unresolved source premise.

Returned proposal (claims-073, event 11): **needs-parent**, suggested bucket **other**.

The intermediate documentation defect is established, but it was corrected before the pinned tip. The allowed result
dispositions have no fixed-in-range category. Preserve the historical finding and let the parent record that explicit
disposition; do not call it source-refuted or out of range.

Original stronger consequence: Maintainers receive a false responsibility map. No runtime scope, provenance or cleanup
defect and no security/user-work consequence was claimed.

Unresolved premise or limitation: Parent must choose its bookkeeping disposition for a genuine intermediate defect fixed
inside the reviewed range; no source premise remains unresolved.

Proposed basis: Verified historical documentation defect, repaired in-range by 08399d092d20815ced6619c26df2391944ae2b4a;
no current queue addition appears necessary.

Existing parent disposition (event 11): rejected. Parent read full claims073.647f0227 removes the remaining tmux tab
caller and1ed482 retains an explicit two-kind docstring whose prior wording names tmux/host. Actual intermediate
responsibility-map defect, corrected at exact clause by08399d09 and retained corrected at00de. Preserve historical
defect/repair without residual item. No stronger scope or runtime consequence established.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g113 record](records/review-through-00de0b6c/groups/g113.json). Record ID: `l4`.
