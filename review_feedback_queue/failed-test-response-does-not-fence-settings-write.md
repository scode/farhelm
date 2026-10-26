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
