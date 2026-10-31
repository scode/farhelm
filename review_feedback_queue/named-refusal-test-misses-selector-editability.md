# Named-refusal test misses selector editability

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The named-create refusal regression checks a retained value without proving that users can still edit and retry it.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The oracle does not exercise a second selector edit after refusal. A companion retry test supplies partial coverage but
does not witness the specific promised editability boundary. No production stuck selector is established.

Source locations: `e2e/tests/named-conversations.spec.ts:141-166`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d. Original discovery owner/ID a062/d2. Suggested bucket
other; confidence confirmed bounded test-oracle omission, not a confirmed GUI defect or executed false pass. The test's
docstring explicitly promises the target remains editable at 141-143. It fills the selector before submitting at 159;
after the mocked refusal, 162-166 observes the error, request metadata, absence of the selector in reusable launch
metadata and the retained value. It never observes editability or performs another edit. Retaining the value alone does
not establish that the field accepts input. The actual selector's disabled flag and input handler depend on busy
(crates/farhelm-ui/src/launch_controls.rs:581-585), supplied from the form at create_form.rs:7259 and ops.busy at 3296.
The companion test at named-conversations.spec.ts:58-77 does edit after earlier refusals and asserts submit readiness
after its named request; that is useful partial coverage, but it does not edit or assert editability of the selector
after the named refusal. The submit button has its own aria-disabled readiness presentation at create_form.rs:7680-7693,
so its assertion is not itself a direct selector-editability witness. No current busy-reset failure or different
execution route was sought or alleged. Required searches of SPEC.md, SPEC_impl.md, TODO Planned, unlanded plans, BUGS,
queue index/files and TRIAGE_OUTCOMES.md found no exact accepted oracle omission or covering item. SPEC.md:1041-1051
concerns refusal presentation rather than this test's claimed editable-field witness. proposed_drop none; possible_cover
companion retry/readiness test is partial only. A direct post-refusal editable assertion or successful edit would
establish the missing observation; none was run or added under this source-only assignment.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a062/d2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
