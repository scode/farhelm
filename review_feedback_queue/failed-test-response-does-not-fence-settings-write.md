# A failed browser response does not fence a test settings write

Reviewed commit: 5c0656a04a6ce2ed645bbc9d59f1b0c4f11f9978

## TLDR

An individual transport failure can let a settings test restore old values before its earlier owned save commits,
contaminating its private stack. The repair follows its prescribed response-wait strategy; acceptance of the residual
limitation is ambiguous.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `e2e/tests/settings.spec.ts:123-138; TRIAGE_OUTCOMES.md:17881-17884`. Locations and revisions in the
assessment below identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 5c0656a04a6ce2ed645bbc9d59f1b0c4f11f9978. TLDR: after an individual browser transport failure, a
settings save can still overwrite restoration and leave the private browser-test stack configured incorrectly. Source
mechanism: settings.spec.ts:110-112 dispatches the final true save and polls persistence, while 123-138 awaits each
browser response, accepts null/failure and independently restores previous. Exact official Playwright/core 1.64.0 source
matches the lockfile hashes: coreBundle.js:37726-37754 maps a generic Chromium Network.loadingFailed event to
_setFailureText; 14240-14245 resolves the request response promise to null; 56391-56392 and 61118-61119 return null
through dispatcher/client; 24619-24631 reports failure without disposing the page. playwright package
lib/matchers/expect.js:13178,13385-13414 gives this unoverridden poll a five-second deadline.
workerProcessEntry.js:1652-1677 awaits the test function (including its finally) before test fixture teardown; repo
playwright.config.ts:76 gives the test sixty seconds. Thus an early individual network failure is an available
request-failure path while cleanup can still run; no page close, test termination, special route.abort control or
missing callback is required. Conditional harmful sequence: previous=false (as the original report required);
normal-mode true PUT has delivered a valid body and reached an owned handler, but its database task has not
acquired/applied the update; that browser request alone fails before response headers; direct API reads continue to see
false and the poll fails within five seconds; finally sees null, sends restoration false, then the earlier true write
commits. Router lib.rs:803-804,828-830 owns that PUT; middleware.rs:51-74 preserves admitted operations after
disconnect; checkout_config.rs:68-76,1183-1193 applies an unconditional independently scheduled update. A later request
is not fenced by completion of the failed browser transport. Suggested bucket: other; only private fixture configuration
is established. Confidence: confirmed conditional source mechanism; external per-request failure after valid-body
admission, adverse server scheduling, actual occurrence and frequency remain unobserved. These are explicit trigger
conditions, not a remaining missing Playwright source premise. No reproduction, test or browser execution occurred.
Caveat/correction: the UI sixty-second timeout alone does not establish the path because it overlaps the test deadline;
the generic early Network.loadingFailed path does. The original untracked-final-request omission is fixed. This broader
failed-response limitation predates the patch. Exact ledger reconciliation: TRIAGE_OUTCOMES.md:14870-14874 explicitly
prescribed awaiting responses while tolerating failure, and 17878-17884 imports that repair shape. The implementation
follows that prescription; an incorrect execution/disposition is NOT established. Neither section explicitly accepts
residual stack contamination, so retain f1 as an additional limitation of the prescribed cleanup strategy for
parent/test-source reconciliation. Possible_cover: those exact ledger sections explain authorized scope; the neighboring
checkout-test-skips-new-spare-setting-restoration finding concerns no restoration attempt and is distinct.
Proposed_drop: withdraw the earlier attribution that deletion itself proves an incomplete execution; do not drop the
independent conditional source mechanism. f1 remains open under the same ID. No user-work/security/highest route is
alleged.

Provenance: `g202/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.

## Additional forwarded-save and retry instances

Additional reviewed commits: `9a82acb9b536ed471027eb4d455f6063b9cb7074`, `1e68fa9d1f7cbca7842900e8872092a3cb58704e`, and
`b260f8a14ebaad9a3cd333582dac49a26b596237`.

The first two add cases where cleanup drains route callbacks without awaiting the forwarded save's HTTP response:
`e2e/tests/settings.spec.ts:86,119–126,440,473–483` at the first commit and
`e2e/tests/claude-trust.spec.ts:535,596–597,632–647` at the second. Official Playwright 1.64.0 source, checked against
the reviewed lockfile, shows `route.continue()` ends at browser-protocol acknowledgement rather than server response
(`coreBundle.js:14385–14438,37838–37885,56420–56480,61290–61355`). Draining the callback is therefore not a mutation
completion boundary.

The third instance removes its refusal route before the real retry (`e2e/tests/templates.spec.ts:433–435`), so no retry
handler remains for cleanup at `445–450` to join. The verified runner's default poll deadline is five seconds
(`package/lib/matchers/expect.js:13178,13328–13349,13385–13414`); its worker awaits the test function, including
`finally`, before fixture teardown (`workerProcessEntry.js:1652–1677`). This supplies an assertion-failure path with
cleanup time inside the repository's sixty-second test budget.

These conditional server schedules are source-established: admit the valid old PUT but delay its blocking worker before
the database mutex; let other workers serve unchanged values until the poll fails; commit cleanup restoration and, where
applicable, template deletion; then let the delayed write apply. The mutex provides exclusion, not request admission
order (`crates/farhelm-supervisor/src/db.rs:128–142`). Admitted writes survive disconnect, and preference updates apply
their field unconditionally (`crates/farhelm-helm/src/preferences.rs:238–255`, `store.rs:5241,5296` at the third
revision; `store.rs:5244,5278–5279,5296` at the second). The late quick-launch write can name a now-deleted test
template.

Suggested bucket remains **other**: private test-stack contamination only. Confidence: confirmed conditional source
mechanisms; sufficiently adverse scheduling, actual occurrence and frequency remain unmeasured. No tests, browser
launches or experiments ran. Trust-hint dismissal is separate from granting workspace trust. These instances preserve
the original ledger-authorized response-wait repair while adding sites that never await the real retry response.
Possible cover remains the existing item and its explicit scope, rather than a second queue item or accepted
contamination. Provenance: `g067/f1`, `g069/f1`, `g068/f2`; earlier blanket cleanup refutations and overconfident
missing matcher claims were corrected under those same identities.
