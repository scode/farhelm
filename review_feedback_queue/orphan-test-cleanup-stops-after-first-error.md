# Orphan test cleanup stops after first error

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

One orphan-fixture cleanup failure can skip cleanup of its remaining owned resources.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `e2e/tests/tensorlake-tunnels.spec.ts:308-317 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g074/f5, event 17):

Reviewed commit 8bd92e1336565dfc263fe5f3fa2fef413a82a87a. TLDR: if this browser test's cleanup encounters one failure,
it can leave its test account/recipe behind on the shared stack and obscure the original test failure. This is separate
from f3's pre-try setup failure: it happens after the body has entered finally. At :309-315 config reset, model read and
model write are awaited sequentially; each is fallible filesystem/JSON work. If model read or write fails, recipe
deletion :316 and saved-key clear :317 are never attempted, even though those HTTP operations are independent of the
fake state file. If recipe DELETE throws a transport error, key clear is likewise skipped. HTTP error status alone does
NOT throw here, since this request has no .ok assertion; do not extend g037's HTTP-refusal route to this exact call. A
thrown finally error also replaces an original body assertion error. Confirmed applicable local cleanup contract in
e2e/tests/helpers/cleanup.ts:1-20 explicitly attempts all owned releases, collects failures, preserves an existing body
failure via stderr, and otherwise throws AggregateError after every attempt. This fixture does not use it. Use
cleanupOwnedSteps with independent callbacks and bodyFailed tracking; group dependent model read/filter/write as one
release, leaving recipe deletion and key clear separate. Conditional impact is shared-stack test
contamination/diagnostic loss, not user-data or live-install harm; normal successful cleanup runs all steps. Stack reuse
and no automatic reset are established by the initial/synthesis source reads. Temporal status: introduced at
c340166d754d3edb3de7fd538c6408948d3536c6:test:154-161; assigned 8bd92e1 reformats it and tip history has no subsequent
edit. Suggested bucket other. Confidence confirmed source-level conditional failure path, not dynamically reproduced; no
claim that an actual cleanup exception has occurred. proposed_drop false; possible_cover related supplied g037:g037-l1
describes the same cleanup-contract omission in tensorlake-hosts.spec.ts:318-326, while this is an additional affected
fixture in the assigned tensorlake-tunnels.spec.ts. Parent may consolidate their shared mechanism but must retain this
exact site. Discovery count: one new verified affected-site finding, zero newly investigated-unverified findings so far.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g074/f5`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
