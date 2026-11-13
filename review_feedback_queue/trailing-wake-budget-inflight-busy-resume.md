# Wake contract omits the longer in-flight Resume wait

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the new implementation contract describes a one-minute limit for waiting on a busy host, but a launch can remain waiting on a Resume request for up to its separate 150-second deadline even when the final result is a busy refusal. This is inaccurate timeout documentation; no observed hang, reproduction or work loss is claimed. Suggested bucket: other. Could-be-highest: false; the route only prolongs launch preparation and does not grant new execution or destructive authority. Confidence: confirmed source/contract mismatch for the in-flight request boundary; incidence unmeasured. Assigned text says All its waiting, busy refusals included, shares one 60-second budget that only the accepted Resume request does not spend, so a slow resume is never cut off. Product SPEC.md:334-338 similarly says a busy launch keeps asking within the same minute it allows for the connection. api.rs:5718-5720 starts WakeBudget after the initial hosts read. At :5723-5734 the Resume HTTP future is awaited directly, outside budget.within. post_lifecycle at :5677-5694 uses send_within(...150 seconds) at :5681. Only an Ok result is refunded (:5728-5730). Busy refusal time is correctly counted retrospectively, but the next budget check occurs only after this request returns, at :5736. Thus accounting does not bound the pending request to the minute. A request begun near the end of the retry allowance can remain in flight under its own 150-second deadline and later return a marked busy refusal; no accepted Resume is needed for that excess wait. Useful caller checks: send_within :2278-2284 passes the chose

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g001/f3`.

### g001/f3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: ba7cc0264028eebb39647deb1d25b055e6886935. TLDR: the new implementation contract describes a one-minute limit for waiting on a busy host, but a launch can remain waiting on a Resume request for up to its separate 150-second deadline even when the final result is a busy refusal. This is inaccurate timeout documentation; no observed hang, reproduction or work loss is claimed. Suggested bucket: other. Could-be-highest: false; the route only prolongs launch preparation and does not grant new execution or destructive authority. Confidence: confirmed source/contract mismatch for the in-flight request boundary; incidence unmeasured.

Assigned text says All its waiting, busy refusals included, shares one 60-second budget that only the accepted Resume request does not spend, so a slow resume is never cut off. Product SPEC.md:334-338 similarly says a busy launch keeps asking within the same minute it allows for the connection. api.rs:5718-5720 starts WakeBudget after the initial hosts read. At :5723-5734 the Resume HTTP future is awaited directly, outside budget.within. post_lifecycle at :5677-5694 uses send_within(...150 seconds) at :5681. Only an Ok result is refunded (:5728-5730). Busy refusal time is correctly counted retrospectively, but the next budget check occurs only after this request returns, at :5736. Thus accounting does not bound the pending request to the minute. A request begun near the end of the retry allowance can remain in flight under its own 150-second deadline and later return a marked busy refusal; no accepted Resume is needed for that excess wait.

Useful caller checks: send_within :2278-2284 passes the chosen 150 seconds into send_inner; :2488-2517 charges preceding-save wait and HTTP dispatch to that deadline, with no wake-budget override. The real helm makes HostBusy via environment/lifecycle.rs:28-34 nonwaiting try_host_provision_lock, and lib.rs:2783-2789 marks only that typed error. It does not make the client receive or decode the response immediately: queueing/slow helm or network can delay delivery, and SPEC.md Slow hosts expressly contemplates slow network/helm conditions. This is a source-permitted conditional request delay, not an asserted long provider operation in the busy branch. The accepted Resume exception rightly needs a longer request; the documentation should distinguish this unavoidable in-flight classification boundary from the bounded post-response polling/retry allowance, or an intentional implementation change must preserve slow accepted Resume behavior. Ordinary fetch_hosts reads and one-second sleeps are budget.within and do not have this omission.

Tip check: api.rs:5730-5768 retains the direct post_lifecycle await and budget checks in the same order; SPEC_impl.md:6731-6742 retains the same absolute-sounding claim. Scoped later diff contains no WakeBudget/post_lifecycle change, so no in-range repair found. This is distinct from supplied g026/f4, a stale-suspension browser test oracle; no duplicate new ID for that issue. proposed_drop: false. possible_cover: none; no prohibited coverage-source searches. All readily checkable source premises settled; the conditional delay is a timing premise, not missing shipped functionality. No tests, builds or reproductions. f3 lead closed to this documentation-boundary finding; does not challenge the accepted slow-host policy itself.

Returned proposal (claims-001, event 15): **retain-verified**, suggested bucket **other**.

The absolute-sounding one-minute promise does not describe the in-flight classification boundary. Busy requests are charged retrospectively but can remain in flight under the separate 150-second request deadline. This establishes a conditional source/documentation mismatch, not an observed hang.

Original stronger consequence: A launch may wait longer than its specified minute while a Resume ultimately returns busy; no security, destructive authority or work-loss consequence was proposed.

Unresolved premise or limitation: A response delayed beyond the remaining minute is a conditional timing premise, not a missing shipped function. No measured frequency or runtime reproduction. Material high-priority UX impact beyond the documented-boundary mismatch has not been measured.

Proposed basis: Record the inaccurate timeout boundary at other priority. Clarify documentation or deliberately revise the implementation while preserving the accepted slow-Resume exception. No drop proposed.

Existing parent disposition (event 15): verified. Parent primary source verification and fully read focused001 checks/current coverage: the shared minute is enforced only through WakeBudget.within, while the lifecycle POST uses a separate150s deadline and is awaited directly; busy time is charged only after its reply. A busy response delayed past remaining time therefore exceeds the stated all-waiting bound. Busy try-lock is immediate at server admission, but neither network nor helm delivery must be immediate. Retain the conditional bound/documentation mismatch, not an observed hang or material high-impact frequency claim. Current specifications do not clearly accept this pending-busy overrun; matching landed plans preserve slow accepted Resume but do not repair this boundary. No exact current cover or full filter match.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g001 record](records/review-through-00de0b6c/groups/g001.json). Record ID: `f3`.

