# The wake scenarios' unbounded arrival wait bypasses owned cleanup on timeout

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The wake scenarios' unbounded arrival wait bypasses owned cleanup on timeout

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g026/f5`, `g027/l5`, `g069/c3`.

### g026/f5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 26 (finding, open):

Reviewed commit: ba7cc0264028eebb39647deb1d25b055e6886935

## TLDR

If the expected Resume callback never completes, these new browser wake tests time out without reaching cleanup of their
real source session, saved template and shared preference changes. The remaining fixtures can affect later tests on the
shared stack.

## Details

For the 'suspended again' and 'stale suspension after the resume' scenarios, the body awaits answered.wait at :581. Only
the matching Resume handler resolves answered at :557, after publishing state and fulfilling the route. If quick launch
does not issue that Resume, or its callback fails before the release, the raw promise stays pending. route-gate.ts:10-16
creates a native Promise with only an external resolver and has no rejection deadline or cancellation input. The test's
own release and cleanup at :601-618 cannot run while the body remains suspended at :581. At that point the test already
owns a real local session (:536-537), a saved quick-launch template (:538-542), and changed shared preferences (:543).
Its afterEach hook at :153-160 drains fleet routes and removes routes; it neither resolves this local answered gate nor
removes those API fixtures or restores preferences. Closing the page cannot reject the independently created native
Promise.

Pinned e2e/package-lock.json declares @playwright/test and playwright 1.64.0. Independently read official source:
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/timeoutManager.ts lines
83-114 races cb() against timeoutPromise; lines 128-132 reject only the timeoutPromise.
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/workerMain.ts lines
384-397 runs test.fn under that wrapper and proceeds to after hooks when it settles. The runner deadline therefore
bounds reporting; it does not reject this raw gate or resume the suspended body into finally. The configured 60-second
timeout is not cooperative cancellation of arbitrary application promises.

Use a bounded arrival assertion, such as a callback-set flag checked by expect.poll, keeping the response-release gate
separate. Then absent arrival throws inside the entered try and reaches its owned cleanup while the API client is still
alive. Test-authoring contract: bound polling/readiness and preserve owned fixture failure lifetimes. Suggested bucket:
other (maintainer test isolation). Could-be-highest: false. Confidence: confirmed source-level conditional failure path,
not observed execution; no frequency or cascade measured. proposed_drop: false. possible_cover: g069/c3 is the analogous
raw-arrival pattern in podman-environments.spec.ts and the same routeGate helper; this newly assigned test caller may
share a parent remediation group, but is distinct from g026/f4's response-consumer oracle and f6's pre-try ownership
window. The enclosing test stack eventually shuts down; no indefinite runner hang or user-install damage claimed. No
runtime test/reproduction performed. A pinned-tip grep confirms the same await at line 581 remains through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

Reopened synthesis turn 5: supplied g024/l5’s manager-drop issue does not replace or refute this browser-test cleanup
defect. The test uses fleet’s real connected ordinary SSH remote with only browser-visible Tensorlake state rewritten,
and its source session is created on the real local host (:536). It does not construct/drop a per-test manager or
require real stopped Podman actors. Reaching a later correct manager shutdown does not restore the skipped per-test
template/preferences cleanup or prevent cross-test contamination before stack shutdown. The original other bucket,
conditional raw-wait consequence, source confidence, possible-cover note and proposed_drop:false remain unchanged; no
extra manager-lifetime harm is claimed.

Returned proposal (claims-014, event 17): **retain-verified**, suggested bucket **other**.

A missing/failed Resume callback leaves the body suspended on a native promise that the Playwright deadline does not
cancel. Its entered finally therefore cannot release the gate or clean API-owned fixtures. Source establishes the
conditional cleanup failure, not an observed leak.

Original stronger consequence: If Resume never completes its arrival signal, real source session/template/preferences
cleanup is bypassed and later tests on the shared private stack can inherit fixtures. This is maintainer test isolation,
not permanent user-work loss, catastrophic tooling damage or an indefinitely hung runner.

Unresolved premise or limitation: No runtime callback failure or measured contamination. The source conditional is
complete. g069/c3 is a supplied possible same-root caller elsewhere; its record is outside this batch and was not
inspected, so parent must reconcile the shared underlying key before aliasing.

Proposed basis: Retain the verified conditional failure path as other. Reconcile g069/c3 with the underlying key,
preserving this new caller scope rather than silently dropping it.

Existing parent disposition (event 17): verified. Parent inspected claims-014 and read the complete existing
bulk-test-preference-latch-bypasses-finally.md. The same uncancellable route-gate native Promise and Playwright1.64.0
callback-versus-timeout race bypass the body's finally; these lifecycle scenarios add a real source session, template
and preference scope. This extends the existing mechanism instead of creating another item. Checked
ba7cc026:tensorlake-lifecycle.spec.ts:536-543,557,581,601-618 and route-gate.ts:10-16, plus the consultation's
exact-version timeoutManager/workerMain references. Timeout bounds the worker but does not reject the body's arrival
Promise or run backend fixture cleanup. No callback failure or contamination was executed. Current
specifications/Planned/unlanded plans/BUGS/activeoutcomes do not accept the additional caller scope; the existing file
covers the root and receives the new evidence. No highest consequence was proposed.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g026 record](records/review-through-00de0b6c/groups/g026.json). Record ID: `f5`.

### g027/l5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

# Bound the helper-arrival wait so the timeout test reaches gate cleanup

Reviewed commit: 533169cf19571d34af3a8497eaceb8d2d7b7c852

## TLDR

If the wake helper never reaches the expected host read, the new browser test times out without reaching its gate
cleanup. Its afterEach then drains a host route still held by an unreleased gate, adding a teardown failure to the
missing-read failure. This is a test failure-handling defect, not an observed product failure.

## Details

The connection-never-seen mode polls helperRead !== undefined, then awaits helperRead.entered directly at
tensorlake-lifecycle.spec.ts:1916-1917. The resume handler creates helperRead before fulfilling Resume (1856-1857), so
the bounded poll establishes only that the gate exists, not that the helper sent its next GET.
fleet.holdNextRead:115-131 creates an application-owned Promise resolved only by a route arrival or explicit release.
The earlier page Connecting read already entered lag and remains blocked on held.gate.wait (fleet:80-85;
handler:1850-1852). Without helper arrival, the body cannot reach finally:1979-1984, the only owner of
helperRead.release and lag.release. afterEach:151-159 has no gate references and instead calls fleet route stops.
helpers/drained-route.ts:39-43 waits all callback promises; the lagged host callback cannot finish until the body
releases its gate. helpers/cleanup.ts:6-13 catches errors but cannot advance past a never-settling cleanup step. Browser
disposal does not settle these Node-side application Promises.

Runner premise independently checked: package-lock.json pins Playwright 1.64.0. Primary source
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/timeoutManager.ts:83-114
uses Promise.race(cb(), timeoutPromise) at 102-105; its timer rejects the separate timeoutPromise at 128-132, without
cancelling the test body's raw await.
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/workerMain.ts:384-397
awaits that timed wrapper and proceeds to after hooks. Repository helpers/evidence.ts:46-112 installs passive
timeline/context/premise collection, not cancellation for the gate. A runner timeout therefore does not force the body's
finally to run. This does not assert an indefinite overall runner hang: teardown can itself time out or the worker can
end.

Use a bounded readiness assertion for actual arrival, or a Promise with an explicit rejecting deadline/lifetime signal,
so absent arrival throws through this try/finally. Keep blocking the response separate from observing arrival, and
release gates before draining their routes. The supplied test-authoring rules require bounded polling and fixture-owned
cleanup. Production timeout correctness is separate from this test-oracle/cleanup guarantee.

Later-fix check: complete diff from 533169cf through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db for this test,
route-gate.ts and drained-route.ts contains only f560058's selector change at test line 170, removing
.xterm-helper-textarea. The raw wait and cleanup ownership are unchanged. Attribution: this raw body wait is introduced
by the assigned 533169cf patch, not inherited solely from its fixture helper.

Suggested bucket: other (maintainer browser-test diagnostics/teardown). Could-be-highest: no; no real session is created
here and no user-work loss/security consequence is established. Confidence: confirmed by pinned source and runner
source, not execution. proposed_drop: false. possible_cover: parent-supplied g069/c3 concerns the same raw-wait pattern
in podman-environments.spec.ts; retain it as related provenance for parent consolidation, without claiming that another
test covers this new wait. Caveats: requires a missing helper read after the gate is armed; normal arrival and ordinary
assertion failures after arrival release correctly. No shared-stack session contamination, leak beyond the
browser-worker lifetime, or observed occurrence is claimed. No runtime tests, reproductions or live-install actions.
Revision: corrected cleanup line anchors and confirmed no later in-range repair; original narrower failure/teardown
consequence and other bucket unchanged.

Returned proposal (claims-015, event 15): **retain-verified**, suggested bucket **other**.

The raw arrival Promise can remain pending after the runner deadline, preventing the test's own finally from releasing
the gate that afterEach drains. This adds teardown failure; it does not establish an indefinite overall runner hang.

Original stronger consequence: No higher consequence was asserted: timeout/teardown failure within the browser worker;
no real session, shared-stack contamination, leak beyond worker lifetime or observed product failure.

Unresolved premise or limitation: The expected helper GET must fail to arrive after helperRead is armed. Normal arrival
and later ordinary assertion failures release gates correctly. Incidence is unmeasured.

Proposed basis: Record this separate test wait/cleanup boundary with corrected local-fleet references. Keep the
runner-lifetime limitation and source-only confidence.

Existing parent disposition (event 15): duplicate. Parent inspected claims-015 and reconciles this additional
wake-timeout caller with g026/f5 and the complete existing bulk preference-latch item.
At533169cf:tensorlake-lifecycle.spec.ts:1850-1857,1916-1917,1979-1984 the same raw arrival wait can prevent its finally
from releasing the held callback; local fleet:117-131 and afterEach:153-159 then leave drained-route.ts:38-42 waiting.
This is useful extension evidence for the same Promise/runner-timeout ownership defect, preserving the corrected
local-fleet citation and distinct teardown consequence. It establishes no real session leak in this particular caller,
indefinite overall runner hang or observed failure. The two consulted exact-version Playwright sources corroborate the
existing item's mechanism. All original IDs and caller-specific consequences remain retained.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g027 record](records/review-through-00de0b6c/groups/g027.json). Record ID: `l5`.

### g069/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462

## TLDR

When the browser never sends the history request that this test is waiting for, Playwright reports a timeout but the
suspended test body never reaches its cleanup. The ordinary-folder test can leave its real source session and agent in
the shared stack. The Recent-promotion case has the same stuck-body wait, but owns no real source session. These are
browser-test failure-path consequences, not a leak in a user's Farhelm.

## Details

The ordinary-folder fixture creates source at line 501 and enters try at 521. After selecting the remote host at
528-529, it awaits reached.wait() at 530. Only the matching remote launch-history callback at 510-515 resolves reached;
if that read is not issued/reaches no callback, reached stays pending. routeGate.ts:10-16 returns a raw Promise with
only a resolve function: no rejection deadline or cancellation input. Closing a browser context does not resolve that
application-owned Promise. The finally at 539-545, including release.release() and cleanupSession(source.id), cannot run
until the raw await resumes. The environment-Recent publish helper at 430-440 likewise waits on reached.wait() at 437
after feed.notify, with its release only later or in finally at 483-488.

Pinned dependency evidence: e2e/package-lock.json pins @playwright/test and playwright to 1.64.0. Official v1.64.0
runner source,
https://raw.githubusercontent.com/microsoft/playwright/v1.64.0/packages/playwright/src/worker/timeoutManager.ts:83-114,
runs Promise.race([cb(), running.timeoutPromise]); its timer rejects the timeoutPromise at 128-132, not the pending
cb/test-body promise. workerMain.ts at the same tag, lines 384-389, awaits the timed wrapper around test.fn. Therefore
the runner's 60-second deadline bounds its wait/reporting, not cooperative cancellation or JavaScript finally execution
inside the still-suspended test body. This refutes c3's original dismissal based on the configured timeout and finally
release. The concrete failure boundary is an absent history request after the staged trigger, the behavior these tests
need to diagnose without bypassing fixture teardown.

Use a bounded readiness assertion (for example a callback-set arrival flag polled with expect.poll) or another locally
rejecting, page-lifetime-aware wait, so missing arrival throws through this try/finally. Keep the response gate
separately released before drain. Suggested bucket: other, limited to maintainer browser-test cleanup/isolation.
Confidence: confirmed by source; no execution or reproduction. proposed_drop: false; the original proposed drop is
retracted, not accepted. Could-be-highest: false; only the owned test stack is affected and no catastrophic tooling
effect is established. possible_cover: related to l1's owned-fixture cleanup consequence but a distinct gap inside an
already-entered try/finally. Caveats: this requires failure to reach the expected route; the ordinary success path
releases correctly. No indefinite runner hang or leak beyond enclosing stack shutdown is claimed. Sequential route-drain
independence is still being checked separately; this conclusion does not depend on a hung drain or on provider
subprocess settlement after stopping a helm.

Returned proposal (claims-043, event 9): **retain-verified**, suggested bucket **other**.

A test-runner deadline does not cancel a pending application-owned promise. If the expected request never arrives, the
guarded test body remains suspended and cannot enter its finally; fixture teardown supplies no replacement deletion of
its real source session.

Original stronger consequence: The initial event proposed dismissal based on the runner deadline and
release-before-drain ordering. Event 8 reopened it; event 9 retracts that dismissal with pinned runner source. Preserve
the original suspected cleanup obstruction and its reversal. Latest consequence is skipped owned test cleanup until a
later reset/shutdown, not an indefinite runner hang or user-install leak.

Unresolved premise or limitation: The trigger is an absent expected route callback. Runtime frequency and actual
subsequent contamination were not measured. The conclusion does not depend on a separately hypothesized stuck route
drain.

Proposed basis: Retain the conditional source finding. Replace raw request-arrival waits with locally rejecting
readiness assertions so missing arrival throws through the entered cleanup guard; response-gate release before drain
remains independently necessary and correct.

Existing parent disposition (event 9): duplicate. Parent read full claims043, prior g026/f5 and g027/l5 judgments and
complete existing latch item. Same raw resolve-only route-gate Promise plus Playwright1.64 callback/timeout race. New
Podman ordinary-folder501-545 and Recent437/483-488 callers add source/resource evidence. Extend existing item with all
caller scopes; no new standalone defect, no actual timeout/leak or indefinite runner hang. Preserve earlier
deadline-based dismissal and reversal; no highest consequence proposed. Current coverage searches found no acceptance of
the additional caller scope.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g069 record](records/review-through-00de0b6c/groups/g069.json). Record ID: `c3`.
