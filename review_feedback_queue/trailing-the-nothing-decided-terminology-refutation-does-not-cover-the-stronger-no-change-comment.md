# The nothing-decided terminology refutation does not cover the stronger no-change comment

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The nothing-decided terminology refutation does not cover the stronger no-change comment

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g038/l3`.

### g038/l3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 20 (candidate, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa

## TLDR
Preserve the earlier candidate for no-definitive-decision terminology, but explicitly limit it to that wording. A separate moved statement claiming no host change or fleet event is now reported as l4; this candidate does not refute it.

## Details
Original suspected consequence remains future maintainers mistaking nothing-decided shorthand for a no-deletion guarantee and misunderstanding why refresh is required. Exact proposed-drop basis retained: SPEC_impl.md Environment Delete and the seal says "Not connected, not sent, sent without an answer, or past the deadline: the helm answers with a marker that nothing was decided". SPEC.md Sandbox sessions likewise includes an unanswered request in its nothing-decided wording. sessions/sandbox.rs:120-122 explicitly clarifies that "an unanswered request may still have been acted on, which the next listing shows". The terminology therefore names lack of a definitive answer rather than a state-preservation contract. API source separately classifies a lost supervisor answer marked by the helm as Unasked, and a lost page-to-helm reply as Refused (api/sandbox_sessions.rs:143-177,224-236); the helper lost-reply wording does not prove those boundaries are conflated.

Synthesis limit/correction: my prior reference to current code refreshing and the pure helper controlling only a question does not establish all nearby documentation accurate. It is not a production-correctness dismissal. The newly supplied stronger comment at view.rs:3060-3064 says the refusal changed nothing on the host and no fleet event will correct a stale row. That categorical claim cannot inherit this proposed terminology drop. Direct same-revision unknown-outcome and supervisor hint checks substantiate the separate l4 documentation record. Keeping this candidate preserves its exact earlier rationale and consequence without merging away the stronger concern.

Suggested bucket: other for the original possible terminology explanation defect. Confidence: confirmed source/contract interpretation of the scoped wording only, not executed. proposed_drop: exact specification adoption of no-definitive-decision terminology plus its explicit uncertainty clarification quoted above, subject to caller disposition; no parent drop accepted. possible_cover: focused claims-023/parent late input led to l4; no prohibited coverage search. Could-be-highest: no established security or work-loss route from this wording, unchanged from original history. Caveats: Unasked never guarantees no state change; do not extend this candidate to the moved no-change/no-event rationale or to a hypothetical newly proved reader-harm route. No runtime tests, builds, reproduction or live action. No unresolved source premise for this scoped candidate.

Returned proposal (claims-023, event 17): **refuted**, suggested bucket **other**.

The named nothing-decided wording adopts the specifications' terminology for no definitive answer. Neither that terminology nor the checked helper establishes a no-mutation guarantee. The helm's own nearby contract explicitly warns that an unanswered request may already have acted.

Original stronger consequence: The original lead suspected that future readers would understand Unasked as guaranteed no deletion and misunderstand why refresh is required. It alleged an internal explanation defect, not observed data loss; the latest candidate withdrew that interpretation in favor of no definitive decision.

Unresolved premise or limitation: No claim that nothing was mutated follows from the named wording under the explicit governing uncertainty contract; no caller was shown authorizing extra destruction based on that interpretation.

Proposed basis: Refute only the asserted no-mutation guarantee in the named comments, based on exact shared specification wording and the explicit uncertainty contract. Route through independent drop checking conservatively because the misunderstood contract concerns destructive user work.

Existing parent disposition (event 17): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: source-contract-refuted-nothing-decided-as-no-mutation-guarantee. Full023 sharedSPEC noanswer/nothingdecidedterminologymeansnodefinitiveanswer, helmnearbyexplicitunansweredmayhaveacted andlaterlistingseal; helperoffersseparatequestionnotautomaticWhole. Distinctsupervisor-tohelmUnaskedvslosthelm-to-pageRefusedboundaries. Proposednarrowrefutationofno-mutationreadinginoriginalnamedshorthandonly, notallnearbycomments. Newstronger'Are fusalchangednothing'commentwasalreadyreturnedoriginal038andretainedasdistinctl4 (parentnonflaggedrecord); freshgateandqueuepreserveit, noterasedbyl3drop. Noactualuserlossallegedbutconservativeflagstays.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g038 record](records/review-through-00de0b6c/groups/g038.json). Record ID: `l3`.

