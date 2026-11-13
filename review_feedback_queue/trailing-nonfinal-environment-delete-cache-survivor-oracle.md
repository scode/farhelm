# Non-final Delete test accepts clearing every cached session

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The new test for deleting one session while keeping its environment can pass if the helm also forgets every other session there. It does not establish its stated promise to forget only the selected session. This is a test-oracle defect; no current production cache-loss behavior is alleged.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g021/f5`.

### g021/f5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998

TLDR: The new test for deleting one session while keeping its environment can pass if the helm also forgets every other session there. It does not establish its stated promise to forget only the selected session. This is a test-oracle defect; no current production cache-loss behavior is alleged.

Details: a_delete_with_other_sessions_left_keeps_the_environment documents `forget only that session` at agent_requests.rs:17758-17760 and constructs helper plus starter at :17771-17775. Its supervisor fake returns Deleted(1) independently of the helm cache. After Delete, :17791-17799 checks the successful reply, reported remaining count and sole request; :17801-17803 checks only the host registry row. The only post-action cache assertion, :17805-17816, checks !cached.contains(helper), which an empty Vec satisfies. Clearing the host's entire session-cache slice while retaining its host row and correct supervisor count passes all assertions. registered() (:17746-17752) reads list_hosts only; neither it nor the fake count establishes starter remains. cached_sessions at store.rs:9531-9572 performs a plain host-scoped query and returns Ok(empty Vec) for an empty cache; it has no hidden nonempty/completeness check. The test ends without another session-list observation. This distinguishes concrete competing local cleanup mechanisms: selected-session forgetting versus whole-host cache forgetting.

Contract: the test's own declared behavior at :17759 is `forget only that session`; SPEC_impl.md Environment Delete and the seal (:6636-6637) says `Deleted` runs ordinary local forgetting and reports the supervisor's count. Sandbox sessions promises the other sessions remain on non-final deletion. The test-authoring checklist requires `Assert an observable that distinguishes the competing mechanisms.` Assert the expected remaining cache contents (starter, helper absent), preferably after establishing the initial two-session cache premise. Current production store::forget_session correctly includes both host_id and session_id in its DELETE (store.rs:9484-9490); that does not repair this independent oracle weakness.

Attribution/range checks: full-range git log -G for the test name identifies 222bf6253bcd168db04bcbb1df20180c88306998 as its introduction. The only later in-range agent_requests.rs commit is f560058da19e02716e6f943883a9508b447aff4e, whose full patch removes unrelated fixture protocol fields only. The cache assertion and stated contract remain unchanged at 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db; the complete base-to-tip path diff confirms their addition. Suggested bucket: other (maintainer test validity). Could-be-highest: false; no shipped security or work-loss route alleged. Confidence: confirmed source/data-flow analysis. proposed_drop: false. possible_cover: none searched; supplied interface records concern separate production or other-test defects. Caveats: other tests may catch an equivalent regression; parent owns coverage decisions. All readily accessible premises settled. No runtime test, reproduction or build performed.

Returned proposal (claims-011, event 14): **retain-verified**, suggested bucket **other**.

The new test declares selected-session forgetting but only checks that the deleted session is absent. Clearing the whole host's session cache while retaining the host row and supervisor reply satisfies every post-action assertion. This is a bounded test-oracle defect, not evidence of current production cache loss.

Original stronger consequence: The test can pass if deleting helper also forgets starter and every other cached session on the host. The history consistently limits this to a false-passing test; it never establishes production loss of user work.

Unresolved premise or limitation: No missing premise for the stated local oracle weakness. Whether another test catches a whole-host cache-clearing mutation was not investigated and is not claimed. No shipped cache-loss behavior is alleged.

Proposed basis: Retain as other: source establishes an assertion that cannot distinguish selected-session forgetting from clearing the host's session-cache slice. There is no exact current cover and no matching filter.

Existing parent disposition (event 14): verified. Parent primary source check and complete claims-011 source/current-cover checks: new non-final Delete test17755-17817 promises only chosen-session forgetting but only checks helper absent and host registered, never starter retained. Whole host session-cache clearing passes its assertions; correct production host+session SQL is not a test oracle. Introduced222bf625 and no in-range repair, exact existing cover or matching filter. No current product cache-loss allegation.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g021 record](records/review-through-00de0b6c/groups/g021.json). Record ID: `f5`.

