# Rewritten terminal contract points to the deleted Terminal type

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Developers following the authoritative implementation specification are directed to a
terminal type and implementation location that no longer exist, making the new terminal ownership and safety behavior
harder to find. This is documentation drift, with no demonstrated runtime effect.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g008/f2`.

### g008/f2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 4 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

TLDR: Developers following the authoritative implementation specification are directed to a terminal type and
implementation location that no longer exist, making the new terminal ownership and safety behavior harder to find. This
is documentation drift, with no demonstrated runtime effect.

Details: SPEC_impl.md:2210-2211 (assigned patch lines 125-126) explicitly rewrites the switch-point map to say
`service/terminals.rs`'s `Terminal` is now a host runtime. The same commit deletes that enum and its dispatch
implementation from crates/farhelm-supervisor/src/service/terminals.rs; the new module imports HostTerminal at line 13
and resolve_terminal returns it at lines 122-128. Whole-file git grep -w Terminal at the reviewed revision found only
the English word in the module header, and no alias or type. The actual runtime is HostTerminal in
crates/farhelm-supervisor/src/hosts/terminal.rs:1226, with status at 2113, evidence at 2539, sweep_root at 2571 and
wire_kind/tab_info at 2641/2648. The contract should name HostTerminal and its actual module, retaining
service/terminals.rs only for identity resolution and attachment bookkeeping. Suggested bucket: other (incorrect
developer-facing architecture pointer). Confidence: confirmed through pinned source and the per-commit deletion, not
execution. Could-be-highest: no; no established security or loss-of-work route from this pointer. proposed_drop: false.
possible_cover: none; no coverage sources searched. Material caveat: behavior described in the rest of the paragraph may
still be correct; the finding concerns the stale abstraction/location newly presented as the replacement architecture.
Revision closes f2's alias-existence check.

Returned proposal (claims-004, event 4): **retain-verified**, suggested bucket **other**.

The rewritten authoritative architecture pointer names a deleted type in the wrong module. This is established
documentation drift; no runtime failure is claimed.

Original stronger consequence: No stronger runtime consequence was alleged: developers are directed to a nonexistent
abstraction and implementation location; ordinary behavior may remain correct.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the narrow in-range documentation defect. Source checks only; no runtime observation.

Existing parent disposition (event 4): verified. Parent explicitly accepted named retain-verified proposal from
claims-004 after inspecting it. The rewritten authoritative architecture pointer names a deleted type in the wrong
module. This is established documentation drift; no runtime failure is claimed. Retain the narrow in-range documentation
defect. Source checks only; no runtime observation. Checked source/results:
["1ed482dde220452e424973faf38a5e1318986333:SPEC_impl.md:2210-2211 names service/terminals.rs's Terminal; the
base-to-reviewed diff explicitly rewrites this pointer while retaining the old name.","At that hash,
service/terminals.rs:1-13 names/imports HostTerminal and :122-128 returns HostTerminal. Whole-file git grep -w Terminal
finds only the English word at line 1, not an alias or definition.","Recording-time SPEC_impl.md:2134 retains the stale
pointer. The claim concerns finding the abstraction, not whether the rest of its behavioral description is correct."]
Current coverage/results: ["Current SPEC.md/SPEC_impl.md searched for terminal switch points, HostTerminal and
service/terminals; the incorrect pointer is the subject, not an acceptance of incorrect architecture guidance.","TODO.md
Planned is empty (:25-26). BUGS.md contains only an unrelated SSH provisioning hang. Active TRIAGE_OUTCOMES.md and full
INDEX.md have no matching item.","Matching plans tmux-end-old-terminals and tmux-drop-from-docs are already landed
(plans/REPORTS.md:29,32), so their broad cleanup criteria are not pending coverage. No unlanded matching plan was
found.","FILTER.md: an incorrect architecture pointer is neither a rare self-correcting display nor a diagnostic, and no
other filter boundary matches."] Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "No stronger runtime consequence was alleged: developers are directed to a nonexistent abstraction and
implementation location; ordinary behavior may remain correct."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g008 record](records/review-through-00de0b6c/groups/g008.json). Record ID: `f2`.
