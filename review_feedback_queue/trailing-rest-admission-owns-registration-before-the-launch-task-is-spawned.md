# REST admission owns registration before the launch task is spawned

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: REST admission owns registration before the launch task is spawned

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g045/c8`.

### g045/c8

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 26 (candidate, open):

# Client loss does not interrupt the registration-to-spawn interval

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b.

## TLDR

Considered whether losing the launch response after the new host row is committed but before its provisioning task is
spawned leaves a stranded row or admission hold. The checked REST caller owns that whole interval independently of the
browser, so this cancellation route is refuted. No stranded row or runtime event is claimed.

## Details

start_session_environment registers the row at tensorlake_add.rs:391-398, reconciles it at :403-418, publishes progress
and acquires the provisioning guard/connection hold at :441-461, and spawns the task only at :472-487. The public REST
adapter wraps its call to that entire method in run_owned (sessions/sandbox.rs:61-80), before it begins streaming the
result at :85-105. run_owned actually spawns the supplied future in its own Tokio task (lib.rs:2721-2728); dropping the
handler's waiter cannot cancel that task. Its complete nearby contract (:2710-2720) explicitly covers committed store
work followed by runtime reconciliation. This check addresses the pre-spawn interval, distinct from c6's inherited
post-spawn handle-registration ordering and l4's result provenance. Suggested bucket: other for the hypothetical
stranded bookkeeping; could-be-highest: no billed create occurs before the inner task, and no established security or
lost-work route. Confidence: confirmed source refutation of REST client-cancellation premise, no execution.
proposed_drop: the actual REST caller owns registration/reconciliation/hold/task-spawn through run_owned.
possible_cover: none; supplied neighbour context contains no same pre-spawn cancellation record. Caveats: this is not a
claim that arbitrary internal callers are cancellation-safe or that panics/runtime termination cannot interrupt the
process. No tests, reproductions, builds or live actions performed.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g045 record](records/review-through-00de0b6c/groups/g045.json). Record ID: `c8`.
