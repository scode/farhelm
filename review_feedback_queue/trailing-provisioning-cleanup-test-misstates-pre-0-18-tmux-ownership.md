# Cleanup test misstates legacy tmux ownership

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Cleanup test misstates legacy tmux ownership

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g046/l2`.

### g046/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 13 (finding, open):

# Cleanup test misstates where older releases installed tmux

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d

## TLDR

The changed test documentation tells maintainers Farhelm never installed tmux beside its binary and always used its private library directory. That contradicts the historical layout the cleanup deliberately preserves.

## Details

The newly written docstring at provisioning.rs:1068-1074 calls ~/.local/bin/tmux 'the user's own tmux, not anything Farhelm installed' and says earlier releases 'always put their tmux' in ~/.local/lib/farhelm. These are historical ownership claims, not facts established by this fixture: the test builds only a plan and never creates a tmux file with known ownership (:1079-1132). SPEC.md:185-187 expressly says: 'A copy that an update before 0.18.0 put beside farhelm in a shared folder such as ~/.local/bin is not removed, since it may be the user's own, as 0.18.0's release notes said.' Backticks in the source surround the paths and program name. The correct constraint is that a shared-folder copy cannot safely be identified as Farhelm's; it may be Farhelm's old payload or the user's own. plan.rs:1161-1167 already captures that distinction and limits the private location guarantee to releases since 0.18.0. Correct current runtime targeting therefore does not refute this separate documentation defect.

Attribution: the selected patch adds both false historical statements. Its parent provisioning.rs:1108-1112 instead acknowledges that deriving the destination from the binary directory installed Farhelm's tmux as ~/.local/bin/tmux. Full-range path history lists later 08399d092d20815ced6619c26df2391944ae2b4a, but endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db still contains the same new claims at :1068-1074. This is introduced inside the pinned range and remains at its end. Amend the comment to say releases since 0.18.0 used the private path and an older shared copy may be user-owned, rather than claiming it definitely is.

Suggested bucket: other (incorrect maintainer documentation about the reason for the removal boundary). Could-be-highest: no established security or user-work-loss route; the incorrect comment still directs preservation of shared files, and no wrong runtime deletion is alleged. Confidence: confirmed by pinned contract, patch, parent and endpoint source; no execution. proposed_drop: false. possible_cover: none; prohibited coverage sources were not searched. Material caveat: this does not repeat c2's source-refuted runtime shared-directory deletion concern or f1's accepted orphan-process residual. The defect is the historical explanation itself. Lead l2 is resolved to a verified finding under the same ID. Final attribution check: the exclusive range base 6e423e30068b62c6cfa276c6d720ee35bb70387a has the parent-style acknowledgment at provisioning.rs:1099-1103 that Farhelm installed the shared copy. The reviewed new historical assertions therefore do not predate the range. Directly read plan.rs:1161-1167 confirms the since-0.18.0 distinction. No additional premise remains open for this documentation finding.

Returned proposal (claims-029, event 13): **retain-verified**, suggested bucket **other**.

The changed test comment makes false historical ownership claims. The specification and nearby plan comment acknowledge that releases before 0.18.0 installed the shared copy. Correct runtime targeting does not refute the documentation defect.

Original stronger consequence: The lead suspected maintainers would learn the wrong legacy ownership/layout; later records verify that same documentation defect. It does not allege loss of the user's executable or broaden c2's refuted route.

Unresolved premise or limitation: None for the documentation inconsistency. No runtime deletion or dangerous instruction is alleged: the comment still says to preserve the shared file.

Proposed basis: Retain the introduced historical explanation error. Correct wording should distinguish releases since 0.18.0 and uncertain ownership of an older shared-folder copy.

Existing parent disposition (event 13): verified. Parent explicitly accepted named retain-verified proposal from claims-029 after inspecting it. The changed test comment makes false historical ownership claims. The specification and nearby plan comment acknowledge that releases before 0.18.0 installed the shared copy. Correct runtime targeting does not refute the documentation defect. Retain the introduced historical explanation error. Correct wording should distinguish releases since 0.18.0 and uncertain ownership of an older shared-folder copy. Checked source/results: ["500548b9253b07a26fd03119679184b69544e31d provisioning.rs:1068-1076 says the adjacent tmux is the user's own, not Farhelm's, and earlier releases always installed privately. The fixture at :1079-1132 only constructs a plan and supplies no historical ownership evidence.","Same revision plan.rs:1161-1167 limits the private guarantee to releases since 0.18.0 and explicitly says earlier releases put theirs beside the binary.","Same revision SPEC.md:185-187 expressly acknowledges a pre-0.18.0 update's shared-folder copy that may be user-owned; current SPEC.md retains that statement.","Exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a provisioning.rs:1099-1106 acknowledges Farhelm installed the shared copy. Endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:1068-1076 retains the new false statements. Source-confirmed introduction and persistence; no runtime claim."] Current coverage/results: ["Both specifications searched/read around shared tmux/private path and release history. They explain the correct preservation boundary; neither accepts the false test explanation.","TODO.md Planned and unlanded plan index empty. tmux-drop-from-docs and tmux-drop-from-install are landed, so their broad comment-sweep language is not unlanded fix coverage.","BUGS.md, queue INDEX and active ledger searched for legacy tmux ownership, pre-0.18 history and shared-binary comments; no exact item or decision. This is separate from runtime c2 and accepted orphan f1.","FILTER.md has no complete match for ordinary incorrect source documentation; no rare trigger or security/data-loss guidance is involved."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The lead suspected maintainers would learn the wrong legacy ownership/layout; later records verify that same documentation defect. It does not allege loss of the user's executable or broaden c2's refuted route."; "None for the documentation inconsistency. No runtime deletion or dangerous instruction is alleged: the comment still says to preserve the shared file.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g046 record](records/review-through-00de0b6c/groups/g046.json). Record ID: `l2`.

