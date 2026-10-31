# Malformed ADD acceptance test does not observe retries

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The malformed-acceptance regression does not count the retry requests it claims to rule out.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The test's malformed 202 response and later assertions do not observe repeated POST admission. A production no-replay
guard is present, but the oracle cannot show that guard remains effective. No actual duplicate host operation or runtime
failure was observed.

Source locations: `e2e/tests/provisioning.spec.ts:2157-2174`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR: The setup-response regression can stay green if the
client starts replaying an already accepted setup request; its title claims a guarantee its assertions do not observe.
The route at 2161-2166 fulfills every POST /api/hosts/provision identically with malformed 202. Lines 2173-2174 check
only form disappearance and an accepted warning. A client that repeats the POST, obtains that same malformed reply
again, then returns the warning still satisfies both assertions. There is no per-request counter or body history in this
test or its beforeEach/afterEach hooks. Current production is correct on the checked path: hosts.rs:2500-2552 calls
provision_host once and treats Unvalidated as a warning, api.rs:4588-4611 returns that variant after the accepted body
fails decoding, and the send funnel at api.rs:1943-1948,2131-2201 contains no replay loop. No production retry or
user-work-loss bug is alleged. The UPDATE twin at provisioning.spec.ts:3316-3387 counts plans/confirms but exercises the
distinct per-host UPDATE endpoint and callback, so it does not establish the ADD path's no-replay behavior. A useful
repair would record the ADD submission attempts (failing on a second request), observe the named malformed-acceptance
settlement and require exactly one; no elapsed-delay assertion is needed. Provenance: inherited before exclusive base
da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c, where the same test at 2157-2174 already lacks the witness. Assigned
b91fffda992b0dd586b0a90564b20bac89482db2 changes only its Set up selector. Suggested bucket other, confidence confirmed
source-level test gap. proposed_drop false; inherited provenance is explicit for parent scope judgment, not an invented
filter. possible_cover none found: searched SPEC.md, SPEC_impl.md, TODO.md Planned (empty at 23-25), BUGS.md,
plans/queue, queue INDEX and TRIAGE_OUTCOMES for malformed/unvalidated ADD, retries/replay, and retry-witness terms.
SPEC_impl.md:5343-5350 requires single consumption and no UI replay; it does not accept this evidence gap. No matching
accepted plan, queue item, decision or FILTER rule was found. Source-only review: no mutation experiment, test execution
or private runtime evidence was used.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g050/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
