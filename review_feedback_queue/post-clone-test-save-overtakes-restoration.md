# A settings test's last save can overtake restoration

Reviewed commit: a54f1697cc32dcc1ab3e5d162a6b059ba3b1b99b

## TLDR

The new post-clone Save can remain in flight after the test restores its private stack's settings, then overwrite that
restoration. Adverse scheduling is unmeasured; no production user-work loss is claimed.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `e2e/tests/settings.spec.ts:702-710`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Reviewed commit: a54f1697cc32dcc1ab3e5d162a6b059ba3b1b99b; mechanism preserved verbatim at inclusive tip
463cac1528b5c5e259404975380d321a3fff0412. TLDR: A failed settings test can leave its post-clone command installed after
cleanup, contaminating later tests on the shared stack. The added fill/click/status assertion is at
e2e/tests/settings.spec.ts:702-704. On an assertion failure while the save is unsettled, finally:706-710 restores
through the independent Playwright request fixture without retaining or awaiting that save's response. The intercept at
670-677 gates GET only; PUT is immediately continued. Removing the route cannot settle a PUT already forwarded; no
explicit page disposal precedes restoration. checkout_post_clone.rs:114-121 uses spawn_forever and checks mounted only
after awaiting the save. api.rs:643-655,1657-1668,1849-1877 has a client-local save fence, not cross-client ordering. At
the pinned tip the protected PUT is under own_state_change (farhelm-helm/src/lib.rs:787-797,827-830), the handler writes
configuration (checkout_config.rs:1026-1043), and its setter enters an independently scheduled blocking transaction
(1265-1272,1293-1305). Thus page disposal cannot retract an accepted write, and an older write delayed before its
transaction can commit after the independent restoration. e2e/playwright.config.ts:58-73,98-107 reuses the helm across
specs; evidence fixture:e2e/tests/helpers/evidence.ts:31-65 adds observation, not configuration repair. Adjacent
settings.spec.ts:647-656 drains every submitted PUT before restoring. Suggested fix: retain and settle every page PUT
before restoration, including failure paths. Suggested bucket: other; only private fixture state is at risk. Confidence:
confirmed conditional source mechanism; the needed assertion failure with a still-unsettled write and its adverse
scheduling have not been reproduced or frequency-measured. Normal passing execution is unaffected. possible_cover: no
exact acceptance found. TRIAGE_OUTCOMES.md:14855-14879 and17867-17885 concern other named tests; the ordinary-user
close/reopen ordering discard at13371-13384 has a different trigger and scope;
held-preference-reply-blocks-fixture-restoration.md concerns a cleanup gate preventing restoration rather than a late
write overwriting it. FILTER rare-glitch acceptance is not established for durable private-stack contamination.
proposed_drop: none. No runtime execution performed.

Provenance: `g270/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
