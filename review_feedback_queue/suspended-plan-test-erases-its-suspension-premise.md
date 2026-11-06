# Suspended plan test erases its suspension premise

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930

## TLDR

The suspended-host setup test changes installation paths and erases its own Suspended state before checking the guard.
The resulting failing scheduler ordering remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/provisioning.rs:3888 at 88bf83e267b5066815448885c61ed4148b851930`.

Source assessment (read with the later corrections below) (review record g045/l4, event 13):

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930; survival checked through
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: the regression test intended to prove that confirming setup never probes
a suspended sandbox has a concrete intervening reconfiguration that erases its suspension premise. Its result can depend
on whether the connection actor restores Suspended before the setup task checks it. This is a retained test-reliability
concern, unverified after investigation, not an established shipped user defect. Suggested bucket: other, because the
established scope is maintainer test evidence, without catastrophic tooling impact or any demonstrated user
data/security consequence. Confidence: likely with the actual failing scheduler interleaving unobserved. proposed_drop:
false for this test concern. possible_cover: unknown; no coverage sources searched.

Concrete fixture route: at the reviewed commit, provisioning.rs:3814-3824 plants the sandbox row without install
coordinates. The test at :3888-3938 builds service(&harness, backend, root), plans ADD, waits for Suspended, and
confirms it; :3925-3938 demands a failed run, a resume-first message, exactly one backend probe and zero operations. The
service helper (:1256-1281) uses layout(root), whose :867-875 overrides install lib/state paths to root/lib and
root/state. service.rs:759-764 plans ADD with that layout, not plan_for_row. claim_register_and_start :1048-1052
registers the plan's coordinates before starting revalidation. register :900-913 writes through store convergence, syncs
the registry and calls retry_now. manager.rs:1419-1423 treats the changed coordinates as reconfiguration; :1792-1797
synchronously publishes Connecting, and :1813-1816 nudges the actor. The guard at service.rs:1968-1971 runs later and
reads only current state (:2206-2210), with no acknowledgement requiring the actor to have restored suspension. That
restoration occurs independently at manager.rs:3759-3774, after the status-source check and, on re-entry,
shared-connection release. The fake status source is immediate (:3805-3808), but an immediate future does not impose an
ordering between separately scheduled actor and setup tasks. If preflight wins the race, it reaches FakeBackend::probe
and violates the test's exact probe-count assertion.

Later fixture survival: ca28b604b876770b1ab757783416f83ad1709313 changes sandbox_harness to admit /fixture/farhelm and
/fixture/state. Inclusive-tip provisioning.rs:3832-3851 still supplies those paths, while the unchanged test at
:4166-4216 still creates its distinct temporary service layout. The two installed paths therefore still change at ADD
confirmation; admission removes the new missing-coordinate refusal but does not remove reconfiguration. At the inclusive
tip, service.rs:784, :1087, :936-937 and :2020 retain the plan/register/sync-retry/guard sequence; plan.rs:597-615
honors the helper's root/lib and root/state overrides; manager.rs:1776-1829 still replaces the displayed state with
Connecting before nudging the actor. No test-source barrier orders the actor's next Suspended publication before
preflight.

Investigation limits: source inspection establishes the concrete fixture path, transient state and absence of a
synchronization contract. No runtime failure or frequency is claimed. Runtime tests and dynamic scheduler experiments
are prohibited by this review brief, and readable sources cannot prove which task wins on a particular run. A controlled
barrier that delays the actor's next status check after confirmation reconfiguration would settle the failing
interleaving. A suitable fix should either test the guard before confirmation changes the known-suspended row, or give
this regression test a fixture whose stored coordinates already equal the service's planned coordinates; add explicit
coordination if it intentionally exercises reconfiguration. Do not rely on arbitrary sleeps.

Separate temporal disposition of the original product theory: the foundation commit has the registration-order gap, but
store.rs:6383-6388 explicitly identifies production host creation as future work. The production feature arrives in
ca28b604b876770b1ab757783416f83ad1709313 together with service.rs:615-621 refusing ordinary SSH probing of a Tensorlake
row missing either coordinate, and :1184-1189 doing the same for Update. The host-row rerun instead retains the
Tensorlake plan kind (SPEC_impl.md at inclusive tip, Tensorlake host creation: 'Rerun retains the Tensorlake plan
kind'). Thus the original concrete missing-coordinate user route is fenced before it ships; no initialized-host
production route with changing coordinates was established in this assigned pass. That exact proposed shipped defect
remains withdrawn/proposed_drop true on this basis. This is not a finding that the later production route fence is
defective. The admitted-but-different paths above are an actual test fixture route and are not refuted by that
production fence. All earlier l4 drop events and l1-l3's narrow source refutations remain intact.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g045/l4`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
