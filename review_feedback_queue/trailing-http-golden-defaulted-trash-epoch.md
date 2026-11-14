# The new Trash epoch golden fixture cannot detect a defaulted decoder regression

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The fast HTTP contract tests do not protect the newly introduced value that tells idle
windows when Trash changed; a browser decoder regression can pass them and leave idle Trash stale. This is a
test-quality finding, not a claim that the current decoder is broken. Revision: promote c1 from candidate; retract its
earlier proposed drop based on the currently matching name/type, because that does not refute a test's failure to
distinguish a future decoder regression. Original consequence and source citations are retained here and in the original
event. Exact sensitivity: the changed producer fixture assigns trash_epoch: 0 to every host
(http_contract_tests.rs:119-140; corresponding host-list.json additions); UI lib.rs:824-827 declares a defaulted u64. A
decoder serde rename while retaining the Rust member would decode the old JSON field as zero without affecting the rest
of Host, and the complete consumer golden test (api/http_contract_tests.rs:163-321) never asserts trash_epoch. Producer
equality still passes because the helm serializer did not change. Merely adding a zero-valued assertion would still not
distinguish this regression: use a nonzero fixture value and assert that exact value in the browser mirror test.
Contract: SPEC_impl.md:495-501 says shared golden files keep HTTP mirrors from drifting. The test module's explicit
rationale at http_contract_tests.rs:10-18 says fields added/renamed should fail this pair, and specifically says 'an
Option left at None or a defaulted field would let a rename of that field slip through both sides.' Attribution: this
commit adds th

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g017/c1`.

### g017/c1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 6 (finding, open):

Reviewed commit: 06d4b68f3a9e913f47cd426fedee5946a4eed77b. TLDR: The fast HTTP contract tests do not protect the newly
introduced value that tells idle windows when Trash changed; a browser decoder regression can pass them and leave idle
Trash stale. This is a test-quality finding, not a claim that the current decoder is broken. Revision: promote c1 from
candidate; retract its earlier proposed drop based on the currently matching name/type, because that does not refute a
test's failure to distinguish a future decoder regression. Original consequence and source citations are retained here
and in the original event. Exact sensitivity: the changed producer fixture assigns trash_epoch: 0 to every host
(http_contract_tests.rs:119-140; corresponding host-list.json additions); UI lib.rs:824-827 declares a defaulted u64. A
decoder serde rename while retaining the Rust member would decode the old JSON field as zero without affecting the rest
of Host, and the complete consumer golden test (api/http_contract_tests.rs:163-321) never asserts trash_epoch. Producer
equality still passes because the helm serializer did not change. Merely adding a zero-valued assertion would still not
distinguish this regression: use a nonzero fixture value and assert that exact value in the browser mirror test.
Contract: SPEC_impl.md:495-501 says shared golden files keep HTTP mirrors from drifting. The test module's explicit
rationale at http_contract_tests.rs:10-18 says fields added/renamed should fail this pair, and specifically says 'an
Option left at None or a defaulted field would let a rename of that field slip through both sides.' Attribution: this
commit adds the field to both the HostView fixture and the golden JSON, so populating and asserting the new field is
directly this patch's contract responsibility. Other coverage checked: sessions_tests.rs:1407-1416 checks the typed
HostView counter after deletion; it does not serialize the host reply or decode it as the UI Host. All Rust UI
occurrences of trash_epoch are its declaration, typed struct literals, or HostTrashReader's consumption at
list/trash.rs:690-697; none supplies a nonzero JSON decoder assertion. Browser coverage does exist:
e2e/tests/rail-notices.spec.ts:37-46 injects nonzero trash_epoch values, and :281-312 distinguishes idle per-host
refresh; that coverage would catch a decoder rename if run. It does not fulfill the golden pair's independent fast
Rust-test role: the module explains browser tests are where drift formerly surfaced and CI does not run them. Suggested
bucket: other/test quality. Confidence: confirmed through source and assertion analysis; no tests or reproductions run.
proposed_drop: false. possible_cover: existing browser sensitivity noted above, not a claim that no test anywhere covers
the behavior. Could-be-highest: no; this record establishes a test gap and potential stale Trash display, not security
or work loss. Caveats: g022/f2's missing fleet wake after a particular delete ordering is a distinct product defect
supplied by the caller; this record neither duplicates it nor claims to independently validate that ordering. Correct
current field names/types remain a fact, but do not remove the sensitivity gap.

Returned proposal (claims-008, event 6): **retain-verified**, suggested bucket **other**.

The fast producer/consumer golden pair cannot distinguish a defaulted Trash epoch decoder from a working decoder. This
is an established test-sensitivity gap, not an established current product failure. Browser coverage does exercise
nonzero epochs and must remain acknowledged.

Original stronger consequence: A decoder regression could silently default the Trash epoch and leave idle Trash stale
while the fast contract pair passes. Initial line 1 proposed setting the candidate aside because current field
names/types match; line 6 explicitly withdrew that proposed drop and retained the test-quality finding. Existing browser
sensitivity and absence of reproduction remain caveats.

Unresolved premise or limitation: None for the test gap. A current stale-Trash product defect from a decoder regression
is not established; that product consequence remains hypothetical. The separate fleet-wake ordering defect mentioned in
history was not checked and is not an alias.

Proposed basis: Retain as other/test quality. Pin a nonzero epoch in the producer golden and assert that exact value in
the consumer test. Preserve the withdrawn proposed drop as history; do not report the hypothetical product decoder
failure as present.

Existing parent disposition (event 6): verified. Parent source-checked this primary claim and read complete focused008
checks/current coverage. The new golden rows all use epoch0 and consumer never asserts trash_epoch; defaulted u64 means
a field rename can evade the fast pair. Current shared names/types do not refute a missing oracle. Browser rail-notices
coverage does exercise nonzero epochs and must remain a caveat: no claim of absent coverage everywhere or current
decoder/product failure. In-range06d4b68f addition, no later exact repair, no Planned/unlanded
plan/BUGS/queue/active-ledger cover or full filter match. Distinct from the separate environment projection's
unpopulated producer schema.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g017 record](records/review-through-00de0b6c/groups/g017.json). Record ID: `c1`.
