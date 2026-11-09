# Settings lifetime test independently commits the save it should observe

Reviewed commit: 9d2ff7da94f0f16c533b44533c10d3fed7cbd94a

## TLDR

The regression meant to prove that a settings save survives closing its dialog can pass even if the original browser
request is cancelled: the test itself sends an independent write. No current product save-loss bug is established.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The existing failed-test-response item concerns fixture restoration before outstanding writes settle. This finding
concerns a false pass in the save-lifetime assertion, a different consequence. Exact Playwright 1.64.0 source
establishes the independent request and tolerated cancelled fulfillment; no runtime mutation was run.

## Recorded source assessment

Reviewed 9d2ff7da94f0f16c533b44533c10d3fed7cbd94a. TLDR: the lifetime test can pass after the UI save is cancelled,
because interception independently commits its PUT. Parent-supplied official v1.64.0 source settles the remaining
Chromium premise: crNetworkManager.ts:699-712 fulfills via catchDisallowedErrors; :729-740 suppresses cancellation
protocol failures (only invalid-status/unsafe-header protocol failures escape). Therefore ordinary cancellation does not
force this test to fail during fulfillment. WebKit wkInterceptableRequest.ts:125-146 also explicitly tolerates
already-cancelled requests through sendMayFail, consistent with Chromium; that helper's body was not needed to establish
the Chromium false-pass route. Previous supplied client/network.ts:364-367 and client/fetch.ts:195-277 prove fetch uses
a new APIRequestContext request with copied metadata, no original-request cancellation check; server/fetch.ts:590-599
and :700-708 tie disposal to context closure rather than modal teardown. Test settings.spec.ts:1743-1774 closes the
component, releases this independent write and asserts only backend/remounted state. checkout_root.rs:90-106 currently
uses spawn_forever, so this is weakened regression coverage, not a currently shipped save-loss claim. Confidence
confirmed conditional source route on Chromium: if a regression aborts the original pending PUT after interception, the
independent PUT still commits and cancelled-route fulfillment errors are suppressed. No runtime mutation pass or native
scheduling observation claimed. Suggested other; no highest route. proposed_drop none; possible_cover none. Proposed fix
retains browser-request continuation semantics or separately asserts original-request survival; independent teardown
draining can remain a separate test. Original wording, uncertainty and all intervening events remain preserved.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g014/f1`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
