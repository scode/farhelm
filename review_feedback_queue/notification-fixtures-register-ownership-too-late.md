# Notification fixtures register ownership after another creation

Reviewed commit: 10e01eaf9f2c75e83d475076db118423b4a5f2f5

## TLDR

If creating a second notification-test session fails, the first session remains outside cleanup in the owned test stack.
Actual occurrence and effects on later tests were not reproduced.

## Details

`e2e/tests/notifications.spec.ts:458–460` creates `belled`, creates `quiet`, then registers both IDs. `515–517` repeats
that ordering for another pair. If the second call rejects after the first returned successfully, `created.push` is
never reached. The changed hook (`143–151`) invokes cleanup that releases only registered IDs (`90–96`). The creation
helper returns the actual API response without separate ownership registration (`helpers/fleet.ts:391–418`). Register
each acquired ID before awaiting another creation. No ambiguous create response is needed for this counterexample.

Suggested bucket: **other**, for fixture leakage/contamination until stack shutdown. Confidence: **confirmed conditional
source mechanism; runtime incidence unmeasured**. No security or non-test user-work loss is alleged. Proposed drop:
pre-existing body ordering outside the changed hook; parent revision `399–401,456–458` has the same gap. Preserve this
attribution caveat; pre-existence is not acceptance.

Possible cover: final `TRIAGE_OUTCOMES.md:17182–17197` requires independent cleanup, not delayed ownership registration.
The analogous fix at `12515–12525` concerns sidebar tests. Notification assertions at `10057–10073` and another listing
record at `14681–14703` have different scopes. No exact SPEC, Planned, BUGS, queue or filter acceptance was found.
Provenance: `test-cleanup-tests-1/f2`, discovery pass 1, trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `test-cleanup-tests-1/f2`.
