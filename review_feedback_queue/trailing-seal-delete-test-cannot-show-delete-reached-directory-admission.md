# Seal Delete test cannot show Delete reached directory admission before the concurrent create settled

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

A regression could let Delete decide to destroy an environment during a concurrent launch without reliably failing the
existing regression test.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:12147`.

handlers.rs:12138-12165 holds working_copy_operations, dispatches Delete, waits only for the earlier per-session
agent-request fence, plants the other session, releases directory admission and checks remaining=1. Awaiting plant and
checking the earlier fence force no poll at the directory gate. If Delete stops at any earlier asynchronous operation, a
missing directory lock can still produce the expected remaining count once planting finishes. SPEC_impl.md:6222-6241
requires holding directory admission for the seal decision so a concurrent create settles before the decision or is
refused after it. Assert directly that Delete reaches and depends on the held directory gate while the other session is
absent. Bucket: other (test oracle). Incorrectly sealing a live environment would risk user-owned work if shown in
shipped behavior, but this is only a coverage gap. Confidence: source-confirmed observation gap. proposed_drop: none.
Needing a later witness does not refute the gap. Possible coverage: lifecycle_e2e.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f6`.
