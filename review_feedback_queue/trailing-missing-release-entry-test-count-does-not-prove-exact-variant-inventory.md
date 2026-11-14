# Missing-entry fixture premise overstates its inventory check

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the release-payload test describes its fixture assertion as proving that exactly one
archive was omitted, but the assertion checks only that archive's absence and the total number of entries. This is a
bounded mismatch in the test's explanation and premise check; it does not establish a product failure.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g073/c3`.

### g073/c3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: cb762a313abbe6da03ae0bf041f07486c4ed4df9. TLDR: the release-payload test describes its fixture
assertion as proving that exactly one archive was omitted, but the assertion checks only that archive's absence and the
total number of entries. This is a bounded mismatch in the test's explanation and premise check; it does not establish a
product failure. Details: in an_asset_absent_from_sha256sums_is_refused, lines 2524-2529 derive the omitted Linux
aarch64 archive name and assert !listed.contains_key(&left_out) && listed.len() == assets::sums_members().len() - 1. The
diagnostic says "the variant leaves out {left_out} and nothing else". Equal cardinality plus one absent member does not
prove the exact remaining inventory: substituting an unrelated name for another expected archive would satisfy that
predicate. Correct production behavior does not make this exact-inventory premise check exact. The actual signed variant
at this revision contains precisely the intended three entries: farhelm-aarch64-apple-darwin.tar.gz,
farhelm-desktop-aarch64-apple-darwin.tar.gz and farhelm-x86_64-unknown-linux-musl.tar.gz. The separate behavioral
assertions at 2537-2545 still require the exact missing-entry refusal and no request for the omitted archive; the
mismatch does not weaken that independent refusal/no-download oracle. The fixture is not alleged to be malformed, and no
current test failure, product/security regression, runtime failure or lost user work is established. A direct equality
comparison between the variant's keys and the normal inventory minus left_out would make the check match its diagnostic;
alternatively narrow the wording to what the predicate establishes. Provenance: discovery pass 1 recorded this same
mismatch as candidate c3, event 7. The premise assertion is added in cb762a31; its parent omission test lacks it. Full
pinned-range path history contains only the two assigned commits, 500548b9253b07a26fd03119679184b69544e31d and
cb762a313abbe6da03ae0bf041f07486c4ed4df9, so this is not attributed merely by comparison with one intermediate revision.
Original proposed omission is preserved: the surplus diagnostic does not weaken the actual absent-entry mechanism, so I
proposed not reporting it as a standalone issue on significance grounds. That proposal was never accepted. Parent now
retains the bounded static test-explanation defect at other priority and reports that focused claims047 checks, exact
cb762a31 source at 2517-2547, the actual fixture and the independent assertions were reviewed; no FILTER boundary or
exact accepted limitation was found. Current proposed_drop: false; the earlier significance-based proposed-drop
rationale remains historical provenance, not a source refutation of this mismatch. Suggested bucket: other/test
explanation and premise specificity. Could-be-highest: no; no security, user-work loss or product failure route is
claimed. Confidence: confirmed by static predicate, fixture and patch analysis, not observed execution. possible_cover:
none in supplied interface records; no prohibited coverage source was searched. No missing source premise remains for
this bounded finding. This recording-only continuation changes kind/disposition under the original c3 ID, preserves the
original consequence and caveats, and adds no stronger allegation or new investigation. No concrete disagreement with
the parent's bounded characterization.

Returned proposal (claims-047, event 7): **retain-verified**, suggested bucket **other**.

The diagnostic overclaim is established by the new assertion, even though the committed fixture is correct and the
production refusal mechanism remains independently asserted. Retain narrowly as a test explanation/oracle-specificity
issue. The reviewer's proposal to omit it as insignificant is a significance judgment, not source refutation or exact
coverage; no FILTER.md boundary fully settles it.

Original stronger consequence: The original record identifies only a test diagnostic/oracle-specificity limitation and
explicitly disclaims a lost absent-entry assertion, failing current test, security consequence or user-work-loss route.
It proposes omitting the surplus exact-set diagnostic as insignificant, pending the caller's judgment.

Unresolved premise or limitation: No premise is missing for the narrow assertion/message mismatch. A stronger claim that
this weakens the missing-entry refusal mechanism, masks a production security bug, or currently fails the test is not
established; unrelated remaining keys are not needed to distinguish the mechanism tested.

Proposed basis: Retain the source-confirmed diagnostic limitation in other, preserving the reviewer's proposed
significance drop and all limits. Either compare the exact key set if exact inventory is intended, or narrow the
diagnostic to the absence/count predicate actually checked. The proper product refusal is not a refutation of this
test-text defect.

Existing parent disposition (event 8): verified. Parent read full claims047, exact cb762a31 release_payloads2517-2547
and complete actual recording-only owner event8. Source predicate absence+cardinality does not prove diagnostic's exact
remaining inventory. Owner now reports bounded finding under sameID, preserving old significance-based omission
proposal, actual correct signed three-entry fixture and independent exact missing-entry/no-download assertions. No
weakened production authentication or that independent refusal oracle, no malformed currentfixture,
runtimefailure/security/workloss. Current coverage search047found no exact
acceptance/Planned/unlandedplan/BUGS/queue/outcome/filter; newly recorded static premise/explanation mismatch other. One
recording-only continuation after already completed source review, no newsource/range/discovery.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g073 record](records/review-through-00de0b6c/groups/g073.json). Record ID: `c3`.
