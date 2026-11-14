# Draining mocked Deletes can outlive the client needed to clean independent sessions

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: if the real cleanup inside a mocked Delete remains pending through the test deadline, teardown
waits on that route before cleaning the kept session or restoring preferences. Playwright then disposes the API client;
later cleanup cannot use it, so a failed test can leave its independent session and preferences in the shared stack.
This is a maintainer test-isolation defect, not a live-install or user-data-loss claim.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g029/f3`, `g069/l2`.

### g029/f3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa. TLDR: if the real cleanup inside a mocked Delete remains
pending through the test deadline, teardown waits on that route before cleaning the kept session or restoring
preferences. Playwright then disposes the API client; later cleanup cannot use it, so a failed test can leave its
independent session and preferences in the shared stack. This is a maintainer test-isolation defect, not a live-install
or user-data-loss claim. Details: the connected test creates kept and gone at 759–762, changes
skip_session_confirmations at 767, and installs environmentDeletes at 764–766. The helper records sent at 720 then
awaits cleanupSession(request,id) at 721 before fulfilling the mock response at 723–725. The test can therefore pass its
sent assertion at 785 while real stop/delete remains pending, then fail its notice assertion at 786 and enter finally.
Its cleanup array puts every route stop before either owned session or preference restoration (790–794).
drained-route.ts:39–43 waits for all in-flight rewrite promises; cleanup.ts:6–13 awaits each step sequentially. A
rejected step is caught, but an unsettled rewrite prevents later independent cleanup from starting. The elsewhere test
at 835–838 and bulk test at 965–968 have the same route-before-independent-session dependency.

Pinned runner premises independently checked: e2e/package-lock.json pins Playwright 1.64.0; playwright.config.ts:119–137
keeps a shared stack with serial tests and a 60-second test timeout, and :160–169 requires tests to restore state. No
actionTimeout is set in its use block. Official v1.64.0 packages/playwright/src/index.ts:360–373 sets
_defaultContextTimeout=actionTimeout||0; client/fetch.ts:65–82 gives a new APIRequestContext that default;
client/timeoutSettings.ts:72–81 preserves explicit zero; client/fetch.ts:251–266 sends that timeout for each request.
Thus this runner's cleanup API request has no separate finite deadline. timeoutManager.ts:83–114 races the test-body
promise against its timer rather than cancelling the pending JavaScript finally. workerMain.ts:384–418 moves on to
test-scoped fixture teardown after that wrapper ends; index.ts:176–189 disposes the request fixture.
server/fetch.ts:148–157 emits Dispose, and :548–552 rejects/destroys outstanding requests. That disposal can settle the
blocked mock callback and let its route drain finish, but the later cleanupSession and patchPreferences now use a
disposed request context. Client fetch's closed channel refuses them; neither independent cleanup began while its client
was alive.

Primary source URLs (all v1.64.0, read through browser source retrieval, no execution):
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/index.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/timeoutManager.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/workerMain.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/client/fetch.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/client/timeoutSettings.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/server/fetch.ts . Separate
stop-admission/drain from independent resource teardown, and bound or abort the in-flight real API work while a cleanup
client remains alive; a separately owned teardown fixture/client can also preserve this lifetime. Suggested bucket:
other. Could-be-highest: no established security, user-work-loss or catastrophic tooling consequence. Confidence:
confirmed conditional source-level failure path, not observed execution. Exact condition: the mock route's real
stop/delete remains unanswered until the test timeout and client disposal. Source establishes deterministic cleanup
ordering under that condition; frequency is unmeasured. proposed_drop: false. possible_cover: parent-supplied g069/l2 is
the same pattern in e2e/tests/podman-environments.spec.ts, not this test file; parent owns deduplication. Caveats:
ordinary promptly settled requests clean up correctly; enclosing stack shutdown still bounds retained fixtures. No
indefinite runner hang, lost user work, runtime tests or reproduction asserted. Additional discovery checks: official
server/fetch.ts:323–325 rejects a new request on an already disposed context, and GlobalAPIRequestContext.dispose at
724–728 calls _disposeImpl; thus subsequent cleanup cannot recover through the same request context even if no disposal
reason string was set. Repo fleet.ts:165–175 and fake_agent.rs:1675–1697 establish that createSession uses the basic
fake agent waiting on PTY stdin until quit, not a browser-owned process; page disposal alone is not session cleanup.
fleet.ts:1098–1131 confirms preference restoration is another real API write using the same client. A pinned grep of the
spec, evidence fixture and config found no request override, actionTimeout or per-test timeout adjustment. All readily
checkable client-lifetime and fixture-ownership premises are now settled. Source-only; original condition and bounded
shared-stack consequence remain unchanged.

Returned proposal (claims-016, event 8): **retain-verified**, suggested bucket **other**.

If the real API cleanup awaited by the mock Delete stays pending through the test deadline, sequential route draining
delays independent session and preference cleanup until their request fixture is disposed. A rejected step is caught,
but a still-pending step is not bounded. This is a verified conditional test-isolation path; no timeout, leak or
frequency was observed.

Original stronger consequence: The lead and both findings consistently allege only retained maintainer-owned test
sessions/preferences and shared-stack contamination. They explicitly exclude an indefinite runner hang, live-install
effects, user-work loss and catastrophic tooling harm. No higher consequence was lowered.

Unresolved premise or limitation: No missing premise for the conditional lifetime/order mechanism. The real stop/delete
must remain unanswered through the test deadline and fixture disposal; this was not reproduced or frequency-measured.
Promptly settled requests clean up normally, and outer stack reset/shutdown bounds the residue.

Proposed basis: Retain this distinct client-lifetime boundary. It is a useful extension to the cleanup-prerequisite
family, but not exact coverage by its rejecting-step item: cleanupOwnedSteps alone cannot fix an unsettled route.
Reconcile the underlying key with the parent's g069/l2 without adopting an uninspected external record as an alias.
Bound/abort route-owned API work while an independent cleanup client remains alive, or give teardown an independently
owned client.

Existing parent disposition (event 8): verified. Parent explicitly accepted named retain-verified proposal from
claims-016 after inspecting it. If the real API cleanup awaited by the mock Delete stays pending through the test
deadline, sequential route draining delays independent session and preference cleanup until their request fixture is
disposed. A rejected step is caught, but a still-pending step is not bounded. This is a verified conditional
test-isolation path; no timeout, leak or frequency was observed. Retain this distinct client-lifetime boundary. It is a
useful extension to the cleanup-prerequisite family, but not exact coverage by its rejecting-step item:
cleanupOwnedSteps alone cannot fix an unsettled route. Reconcile the underlying key with the parent's g069/l2 without
adopting an uninspected external record as an alias. Bound/abort route-owned API work while an independent cleanup
client remains alive, or give teardown an independently owned client. Checked source/results:
["2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa:e2e/tests/tensorlake-sandbox-sessions.spec.ts:719-725 records sent then
awaits real cleanup before fulfillment; :759-767 creates independent kept/gone sessions and changes a shared preference.
:785-786 can reach the sent witness while the notice is still waiting. finally:790-795 drains all routes before session
cleanup and preference restoration. :835-839 and :965-969 have the same ordering.","Same
revision:e2e/tests/helpers/drained-route.ts:39-43 waits for every entered rewrite before unroute;
helpers/cleanup.ts:6-13 awaits cleanup steps sequentially. allSettled catches rejection but cannot settle an unanswered
rewrite. fleet.ts:576-589 and :1108-1131 use the same request fixture for session deletion and restoration.","Same
revision:e2e/package-lock.json pins Playwright 1.64.0. playwright.config.ts:119-148 keeps a shared stack and 60-second
test deadline, with no actionTimeout. evidence.ts extends observation fixtures without overriding the test request
fixture; the test file has no timeout/request override.","Official Playwright v1.64.0 source was retrieved, without
execution: packages/playwright/src/index.ts request fixture creates/disposes its API context and _setupContextOptions
sets _defaultContextTimeout=actionTimeout||0; packages/playwright-core/src/client/fetch.ts:newContext installs that
default and fetch forwards TimeoutSettings.timeout. timeoutSettings.ts:_timeout preserves explicit zero. The repo's real
cleanup calls therefore have no shorter independent deadline.","Official v1.64.0
packages/playwright/src/worker/timeoutManager.ts:withRunnable uses Promise.race; worker/workerMain.ts runs test-scoped
fixture teardown after the test wrapper finishes. index.ts's request fixture disposes its context then.
server/fetch.ts:_disposeImpl marks it disposed and emits Dispose; outstanding requests reject/destroy on that event, and
_sendRequest rejects later requests on the disposed context. Disposal may release the blocked route, but independent
cleanup using that same client is then too late.","Primary URLs checked:
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/index.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/timeoutManager.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/workerMain.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/client/fetch.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/client/timeoutSettings.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/server/fetch.ts . Prefer
symbol references because rendered web-source and raw-source line numbering differed.","The new test/helper use is
introduced by 2ef10815 and git diff to 00de0b6 for the affected spec is empty. This does not assert an indefinite runner
hang, persistence past outer stack reclamation, or live-install/user-work loss. The record's supplied g069/l2 is an
external same-pattern reconciliation hint, not an independently checked alias in this batch."] Current coverage/results:
["SPEC_impl.md Testing describes real shared test stacks and fake agents; neither it nor SPEC.md accepts failure to
restore independent shared fixture state after an unanswered cleanup request. No exact accepted limitation
found.","TODO.md Planned is empty. Plans/queue searches for cleanupOwnedSteps, environmentDeletes, drainedRoute, route
timeout, disposal and client lifetime found no unlanded repair of this dependency. Environment-agent-operations, which
added these Deletes, is landed per plans/REPORTS.md:28.","BUGS.md's SSH-transfer helper hang is unrelated. INDEX.md and
matching files were checked: route-retirement-failure-skips-owned-session-cleanup.md addresses rejecting prerequisites
before cleanup and explicitly disclaims recovery with a closed request context; the new call sites already use its
suggested independent-step helper, yet an unsettled step still loses client lifetime.
held-preference-reply-blocks-fixture-restoration.md concerns a separate never-settled post-fulfillment gate in
popover-template-movement, not real API cleanup and disposal. replace-cleanup-snapshots-owned-ids-before-drain.md
concerns enumerating IDs too early. manual-unroute-leaves-listing-callback-running.md concerns missing drain. None
exactly covers this trigger/cause/scope.","TRIAGE_OUTCOMES.md:1184-1196 chooses observed-flake/trivial-fix treatment for
the held-preference gate in four named tests. The same consequence family is insufficient to extend that decision to
this request-client lifetime defect. No archive read. post-clone-test-save-overtakes-restoration concerns a late write
overtaking restoration, also distinct.","FILTER.md rare-glitch rules do not cover shared sessions/preferences left
behind for later tests; that state needs explicit cleanup/reset and is not merely a transient display or safely retried
operation. Other filters' triggers do not match."] Exact cited cover: [] Original stronger consequence and missing
premise remain preserved: "The lead and both findings consistently allege only retained maintainer-owned test
sessions/preferences and shared-stack contamination. They explicitly exclude an indefinite runner hang, live-install
effects, user-work loss and catastrophic tooling harm. No higher consequence was lowered."; "No missing premise for the
conditional lifetime/order mechanism. The real stop/delete must remain unanswered through the test deadline and fixture
disposal; this was not reproduced or frequency-measured. Promptly settled requests clean up normally, and outer stack
reset/shutdown bounds the residue.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g029 record](records/review-through-00de0b6c/groups/g029.json). Record ID: `f3`.

### g069/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa

## TLDR

If the mocked Delete's real cleanup request is still pending when the test times out, teardown of the browser route runs
ahead of cleanup of the other owned session. The runner disposes the test's API client while that drain is pending; the
later session cleanup then has a disposed client and cannot restore the shared stack. A failed Delete test can
consequently leave its independent kept session/agent behind. This is browser-test isolation damage only.

## Details

This test creates kept and gone at lines 643-646. Its page-route callback at 661-665 calls
cleanupSession(request,gone.id) before fulfilling the mocked browser Delete. The request fixture's API calls bypass page
routing, so a still-pending stop/delete is real owned API work. In finally, lines 688-690 put both route stops before
cleanup of either owned session. helpers/drained-route.ts:39-43 awaits all in-flight rewrite promises;
helpers/cleanup.ts:6-13 awaits each cleanup step sequentially. Catching a rejected step cannot advance past a
still-pending step. Thus the active Delete rewrite is an ordering dependency for teardown of the independent kept
session.

Pinned Playwright 1.64.0 sources establish the timeout/disposal path. packages/playwright/src/index.ts:360-373 sets
playwright._defaultContextTimeout=actionTimeout||0; this suite config sets no actionTimeout.
packages/playwright-core/src/client/fetch.ts:65-82 copies that value into a new APIRequestContext and lines 251-266 use
its timeout settings; timeoutSettings.ts:72-81 preserves explicit default 0. These requests therefore have no own finite
deadline in this runner configuration; do not assume the standalone library's 30-second default.
packages/playwright/src/worker/timeoutManager.ts:102-105 races the test body rather than cancelling it.
workerMain.ts:384-418 advances to fixture teardown after that wrapper times out. index.ts:176-189 disposes the request
fixture at teardown, and client/fetch.ts:178-181 refuses further fetches once it has a close reason (or the disposed
channel rejects them). When disposal interrupts the rewrite's pending request, its work settles and the route drain may
return, but subsequent cleanupSession calls now use that disposed request context. The independent session had no chance
to begin cleanup before this lifetime ended. Official source URLs are
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/index.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/client/fetch.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/client/timeoutSettings.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/workerMain.ts ;
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/timeoutManager.ts .

Separate stopping admission/draining of rewrites from independent owned-resource teardown, and bound/abort the in-flight
API operation while the cleanup client is still alive. A fixture-owned teardown phase with its own deadline/client can
also preserve this ordering. Merely catching errors or eventually closing the browser does not grant the later requests
a usable client. Suggested bucket: other (maintainer browser fixture cleanup/isolation). Confidence: confirmed
source-level failure path, conditional on the Delete rewrite's real API request remaining pending across the test
deadline; no occurrence or cascade was observed. proposed_drop: false. Could-be-highest: false; owned test resources
only, with no catastrophic tooling consequence established. possible_cover: l1/c3 share the final isolation consequence
but cover pre-try and raw-arrival waits, respectively. Caveats: normal promptly settling Delete cleanup reaches all
owned steps. No indefinite runner hang or leak beyond stack shutdown is claimed. This is not g070/l1's real provider
subprocess-after-helm-stop boundary; it is the browser test's API client and independent sessions. The originally
suspected delayed route-drain consequence is retained and now has a concrete runner/client-lifetime route.

Additional disposal premise checked: official v1.64.0 packages/playwright-core/src/server/fetch.ts:148-157 sets
_disposed and emits Dispose; lines 548-552 reject outstanding HTTP work and destroy its request on that event; lines
323-325 reject new sends on an already-disposed context. GlobalAPIRequestContext.dispose at 724-728 invokes
_disposeImpl. Source:
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright-core/src/server/fetch.ts . This
directly establishes both why the pending rewrite can settle at fixture disposal and why later independent cleanup
cannot reuse its client. No additional missing cancellation premise remains for the conditional source-level route. The
enclosing stack lifetime still bounds retained fixtures; no runtime observation was made.

Concrete assertion-to-cleanup ordering: the callback pushes sent at line 663 before awaiting real cleanup at 664. The
test can pass its sent assertion at 683 while the mocked response is still unavailable; its remaining-count notice
assertion at 684 then waits/fails without the callback completing. The body enters finally with that rewrite in flight,
which is exactly the work the second route stop drains before either owned-session cleanup. This path needs no forced
worker kill or generic claim about arbitrary JavaScript scheduling: an unanswered real stop/delete in the explicitly
invoked callback suffices. No change to l1 is implied; its exact pre-try session/template gaps remain separate.

Returned proposal (claims-044, event 13): **retain-verified**, suggested bucket **other**.

Verified conditional source ordering: an unanswered real cleanup request in the Delete rewrite blocks route retirement,
which precedes every independent session cleanup. The test deadline can expire and dispose the shared API context before
those later requests start. Only private test-stack contamination is established; no occurrence, indefinite runner hang,
or leak beyond wider stack shutdown is claimed.

Original stronger consequence: Original lead: a slow route drain can consume independent cleanup. Later finding:
disposal can make later owned-session requests unusable, leaving the kept session/agent until wider stack teardown. No
broader user-resource or indefinite-hang claim was made or discarded.

Unresolved premise or limitation: No missing source premise for the conditional ordering. A real Delete cleanup request
remaining pending through the test deadline, occurrence, and frequency remain unobserved trigger facts. Promptly
settling requests reach all owned cleanup steps.

Proposed basis: Record the conditional browser-test cleanup failure separately from exception-only route retirement and
provider subprocess settlement.

Existing parent disposition (event 13): duplicate. Parent read full claims044 and prior source/runner coverage for
g029/f3. Exact same introduced2ef10815 browser Delete rewrite pattern in a second spec: real cleanup awaited by
callback; finally drains callback before independent owned cleanup; pending call can consume request-fixture lifetime
and disposal makes later cleanup unusable. Merge new podman-environments643-690 caller into retained g029/f3's
underlying client-lifetime defect, preserving both session/preference scopes and all Playwright1.64 source
checks/corrected citations. Rejecting-step cleanup helper does not fix a pending predecessor, so distinct from existing
route-retirement item. Conditional test contamination until broader reset, no actual occurrence or user-work loss.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g069 record](records/review-through-00de0b6c/groups/g069.json). Record ID: `l2`.
