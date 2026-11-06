# Identity adoption overwrites held lifecycle display

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a

## TLDR

Concurrent installation adoption can overwrite the displayed Suspend or Resume progress while that operation still holds
the connection.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/manager.rs:2663-2691,2731-2758 at 1a57aa8e265cbb59cedc228433b189f59f7ad59a`.

Source assessment (read with the later corrections below) (review record g044/l11, event 27):

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a. NEW pass-2 verified source finding, not a duplicate endpoint
variant. TLDR: while a sandbox is being suspended, finishing a concurrent identity adoption can replace Suspending with
Connecting even though the connection remains parked; the screen can offer lifecycle controls again while the original
operation still owns the host. Mechanism: manager.adopt captures the mismatched state/status sender at 2663-2674 and
awaits store.adopt_identity at 2688-2691. It holds no host provisioning guard across this gap. store.rs:7731-7799
performs the identity/cache transaction but knows no connection hold; it can commit before the adoption continuation is
scheduled. Another authenticated client can start Suspend while the first adoption completion is pending:
lifecycle.rs:50-55 takes the provisioning guard, :80-91 publishes TensorlakeChanging through hold_connection and
waits/starts vendor work. Adoption's later status.send_modify at manager.rs:2731-2748 tests only Retired and
unconditionally publishes Connecting, preserving status.hold. Its retry_now at 2758 wakes a parked actor but does not
release the hold, so the actor cannot restore lifecycle intent while the vendor operation continues. Public
hosts.rs:1431-1440 adds no serialization to adopt. Middleware owns independent requests rather than serializing them
(middleware.rs:66-74; lib.rs run_owned:2609-2615), so a direct authenticated concurrent REST sequence establishes the
producer without requiring a stale GUI event. Multiple clients can also issue these separate actions: UI
hosts.rs:4415-4432 offers Suspend on an identity mismatch, and :4940-4947 derives transition disabling from the
displayed TensorlakeChanging phase, which adoption erases. Server provisioning locking still refuses any competing
lifecycle operation; no actual competing vendor request or premature dial is claimed. SPEC.md Topology:286-291 requires
explicit Suspend to show Suspending until confirmation and not offer competing lifecycle action; SPEC_impl.md Tensorlake
hosts:6018-6034 describes held transient intent. Temporal: original assigned 88bf83e267b5066815448885c61ed4148b851930
introduced the hold without making existing adoption publication respect it (then manager.rs:2649-2677,2717-2744);
explicit Suspend caller was added in 8a7f66ede90651d47d1facc1e877ce81f842af1a, and clear in-progress presentation in
reviewed 1a57aa8e... makes this route user-visible. git diff of manager/middleware/lifecycle from reviewed hash through
pinned 8a8c9579e04299410652f0e4681975a7198dfc7b is empty. Suggested bucket other: diagnostic/control-availability error,
confidence confirmed by source-level admitted interleaving, no runtime frequency/duration claim. proposed_drop false;
possible_cover unknown; parent handles coverage. Distinct root from g015:l3's active flag after successful Suspend,
g035:l4's completed retirement masking, and g034:l2's ambiguous Resume omission: this is a live actor's separate
manager-owned late adoption publication bypassing hold presentation. Suggested fix: preserve held display when
completing adoption (or serialize adoption with holds before observing its mismatch), while still announcing the durable
identity/cache change and invalidating claims as needed. Source-derived regression: gate adoption immediately after its
store commit and before status publication, start/acknowledge a held Suspend, release adoption, and assert
TensorlakeChanging remains until lifecycle completion; do not use timing sleeps. Caveats: requires overlapping approved
adoption and Suspend on the same mismatched Tensorlake host, e.g. authenticated concurrent requests; single-page busy
guards do not serialize other clients. A hold still prevents dialing/routing, and approved adoption's cache purge is
intended; no security/data/process-loss route established. No unresolved source premise remains.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g044/l11`; supporting records
`g049/iface_g044_l11`, `g034/c11`. The retained private log preserves earlier wording and its corrections. This record
does not claim runtime reproduction.
