# Delete-only retirement leaves the separator explanation claiming every row has Rename

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the row-menu explanation still tells future maintainers that Rename is unconditional and
Delete always has a preceding command. This change introduces Delete-only retired rows, making that explanation false
precisely where the conditional separator is now necessary.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g035/l3`.

### g035/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 17 (finding, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR: the row-menu explanation still tells future maintainers
that Rename is unconditional and Delete always has a preceding command. This change introduces Delete-only retired rows,
making that explanation false precisely where the conditional separator is now necessary. Details: the retained comment
immediately above delete_follows_a_separator says 'Rename is unconditional today, so this is always true in practice'
and describes the first-item check as defensive future-proofing. The assigned change switches the actual menu_order call
to session_menu_order_for_session at row.rs:1396, whose retired branch packs only Delete (5118-5128); it also wraps all
non-Delete rendered commands in !retired (2603-3041). Consequently Delete has position zero on these supported rows, and
delete_follows_a_separator is false. Rendering at 3042-3048 correctly omits its separator. The new pure test also
asserts Delete at position zero and no second item. Production layout is correct; the distinct defect is the stale
rationale teaching that this first-item case never occurs today. Replace the unconditional-Rename claim with the actual
ordinary-row/retired-row distinction so future readers understand why the conditional is required. Attribution:
excluded-base row.rs:1385-1388 already has the old comment, when the menu still offered Rename unconditionally. The
reviewed patch's complete diff introduces the retirement branch and renderer gate but leaves that comment unchanged.
Therefore this is new in-range documentation drift caused by changed behavior, not an unrelated pre-range prose defect.
Pinned tip row.rs:1463-1466 retains the same inaccurate claim. Governing contract: SPEC.md Retired containerized
sessions requires Delete only, and root AGENTS.md requires nearby explanations to track changed invariants. Suggested
bucket: other, internal maintenance documentation accuracy only. Could-be-highest: no security, process-loss or
data-loss route is alleged or inferred. Confidence: confirmed source/comment contradiction, not execution.
proposed_drop: false. possible_cover: none in the complete refreshed neighbours or own prior records; l1/l2 concern
different focus/navigation behavior, not this rationale. No tests, reproductions, builds, checkout edits or live-install
actions. The originally suspected misleading first-item explanation is retained; no runtime separator defect is claimed.
Numbered source follow-up pins the inaccurate sentence to row.rs:1415-1417 at the reviewed commit, directly above the
correct position>0 predicate at 1418-1420. Its original broader location 1396 identifies the changed caller that made
the sentence stale. No change to consequence, bucket or confidence.

Returned proposal (claims-021, event 17): **retain-verified**, suggested bucket **other**.

The changed behavior makes an existing source explanation false. The correct separator implementation does not refute
the distinct maintenance-documentation defect, which is in scope.

Original stronger consequence: Misleading separator rationale tells maintainers the first-item case does not happen. No
runtime layout, data-loss or security consequence was alleged.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: New other-priority source-documentation item. Keep separate from retired cancellation focus and
stopped-waiting keyboard order.

Existing parent disposition (event 17): verified. Parent explicitly accepted named retain-verified proposal from
claims-021 after inspecting it. The changed behavior makes an existing source explanation false. The correct separator
implementation does not refute the distinct maintenance-documentation defect, which is in scope. New other-priority
source-documentation item. Keep separate from retired cancellation focus and stopped-waiting keyboard order. Checked
source/results: ["04d85f0c98a9b635709d00b056719a0baa7fd3fc:crates/farhelm-ui/src/list/row.rs:1414-1417 says Rename is
unconditional and Delete always has a preceding item. :1396 selects session_menu_order_for_session, whose :5116-5121
retired branch contains only Delete, so Delete position is zero and :1418-1420 is materially needed
today.","Exclusive-base-to-reviewed diff introduces the retired-only order/renderer gate while retaining that
explanation. This is in-range drift, not an old independently false statement. Tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:row.rs:1463-1469 retains it. No runtime separator defect is claimed."] Current
coverage/results: ["SPEC.md:4892-4900 and SPEC_impl.md:8312 onward require retired Delete-only behavior; neither accepts
contradictory source rationale. Root AGENTS documentation requirements reinforce updating the explanation.","Planned and
unlanded-plan index are empty. Stored podman retirement section:284-296 specifies retirement but is already landed and
does not repair the comment. BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md searches found no
unconditional-Rename/separator item.","FILTER.md has no source-comment-drift filter; no race or large-content trigger
applies."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Misleading
separator rationale tells maintainers the first-item case does not happen. No runtime layout, data-loss or security
consequence was alleged."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g035 record](records/review-through-00de0b6c/groups/g035.json). Record ID: `l3`.
