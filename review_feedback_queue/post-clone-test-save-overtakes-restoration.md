# A settings test's last save can overtake restoration

Reviewed commit: a54f1697cc32dcc1ab3e5d162a6b059ba3b1b99b

## TLDR

The new post-clone Save can remain in flight after the test restores its private stack's settings, then overwrite that
restoration. Adverse scheduling is unmeasured; no production user-work loss is claimed.

A Hosts-panel test has the same ordering gap for its asynchronous sort save, described below.

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

## Hosts-panel sort save instance

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

Even successful cleanup of the new panel test can be overtaken by its own GUI sort save.
`e2e/tests/hosts-panel.spec.ts:25` calls `chooseSort`, which waits for the local menu/control, rather than persistence
(`helpers/list-header.ts:17–23`). `remember_sort` queues a fire-and-forget preference write
(`crates/farhelm-ui/src/list/view.rs:453–460,2052–2063`; `api.rs:1462–1505`). Sort is outside the configuration-save
request barrier (`api.rs:850–854`). The test's independent API client restores the old sort at line `75`, without first
settling that GUI write (`helpers/fleet.ts:1084–1107`).

The browser's per-field queue does not order its writes against that other client. If the title request remains pending
until after direct restoration and then reaches the helm, the unconditional field update wins again
(`farhelm-helm/src/preferences.rs:249–253`; `store.rs:5158–5169,5219–5225`). Admitted writes also survive client
disconnect (`middleware.rs:51–74`), so page teardown is not an application-level write fence. This does not require the
store to reorder already-enqueued calls. The mechanism remains at tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

Suggested bucket remains **other**, for private test-state contamination. Confidence: **confirmed conditional source
ordering gap**. It needs a different inherited sort and a delayed title write that commits after restoration; actual
request arrival, occurrence and frequency were not measured. No runtime reproduction or product-data loss is claimed.
Settle the exact owned save before restoring, including failure paths, while attempting cleanup steps independently.

Possible cover: the existing failed-response record describes related named settings, trust and template cases, rather
than accepting this new Hosts/sort instance. SPEC.md:1493–1496 and SPEC_impl.md:211–229 require shared sort persistence;
they do not accept fixture contamination. Searches found no exact Planned, unlanded-plan, BUGS, ledger or filter
coverage. Proposed drop: none. This differs from the no-restoration-attempt instance in
`checkout-test-skips-new-spare-setting-restoration.md`: no earlier cleanup error is necessary here.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g016/g016-f3`, found during synthesis.

## Additional review at d6645f77

Additional reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

The host naming test's created-sort save and the template bolt test's final unmark add callers for the same independent
restoration ordering gap. The naming test observes local sort state and drains presentation routes, without retaining
the page's sort PUT before direct API restoration. The successful bolt-test route requires an inherited non-null mark
followed by a final null write committing after restoration; deleting the fixture only clears a mark equal to its name.
A failed body with a pending initial mark is a separate branch and does not require a non-null incoming mark. Actual
adverse scheduling remains unmeasured. Suggested bucket remains other; retain the observed-flake/trivial-fix ledger
decision as possible coverage rather than silently extending it to every new caller.

Source and provenance, run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`:

- `g146/f5`:
  `e2e/tests/host-short-names.spec.ts:24-42,288-294; historical 142efec47287404d1e8f9be461c2cdb78cbd542a:23-40,278-285,339-345`.
- `g148/f1`: `e2e/tests/templates.spec.ts:64-74`.
- `g173/f2`: `e2e/tests/templates.spec.ts:49-51,64-73`.
- `g176/f2`: `e2e/tests/host-short-names.spec.ts:24-42,77-79,259-295`.

Proposed drop: none for the retained remainder. No runtime reproduction, fix or triage decision was performed.
