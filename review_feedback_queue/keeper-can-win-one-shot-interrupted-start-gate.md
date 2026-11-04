# The keeper can win the interrupted-start fixture's one-shot gate

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Two startup-reclaim regression tests lack the ordering needed to hold their intended first launch; a keeper launch can
take the gate instead and fail setup before reclamation is tested.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
missing ordering and competing-gate mechanism; occurrence/frequency of an actual failure is unobserved. This is not a
measured flake, and no product data-loss consequence is claimed.

Source locations: crates/farhelm/tests/e2e/host_reclaim.rs:1042-1043,1125-1126.

host_reclaim.rs:1042-1043 and 1125-1126 start a keeper through the same interrupted_seams immediately after the
interrupted create returns. gated_host_program chooses its held invocation with /bin/mkdir <fixture>/held after /bin/cat
consumes stdin (host_creates.rs:1109-1129); later invocations skip the wait. The interrupted path writes the spec into a
pipe, drops stdin, and forgets the child without waiting for that mkdir (hosts/spawn.rs:486-503, called at
service/core/host_launch.rs:558-569). Thus the keeper's child can win mkdir and wait on the absent gate while the
intended interrupted child proceeds. keeper() then fails or times out before the intended restart assertions. Assert a
child-published held marker after the interrupted create and before starting the keeper, with bounded waiting and
fixture diagnostics.

Two startup-reclaim regression tests lack the ordering needed to hold their intended first launch; a keeper launch can
take the gate instead and fail setup before reclamation is tested. Exact basis: R/T host_reclaim.rs:1042-1043 and
1125-1126 launch the keeper immediately after the interrupted create returns. R host_creates.rs:1109-1129 / T:1181-1202
selects the one gated invocation with mkdir only after cat consumes the spec and EOF. R host_launch.rs:558-569 /
T:560-572 returns Interrupted after deliver_and_abandon_for_test; R/T hosts/spawn.rs:486-503 only write_all's stdin,
closes it and forgets the child. Neither acknowledges the child's later mkdir. The first child may be descheduled before
mkdir while the keeper reaches it first; the keeper then waits on the absent gate and the intended interrupted
invocation bypasses the hold. The later premise assertions at R/T host_reclaim.rs:1057-1065,1140-1143 occur after the
dependent keeper operation. Fix: before launching another invocation, await a child-published held marker from the first
invocation with a bounded shared wait and useful premise diagnostics.

Possible coverage and caveats: none matching trigger/consequence/scope found across required coverage sources. T
SPEC_impl.md:2393-2441 specifies the product's reclaim, not acceptance of unsynchronized fixture setup; T TODO.md:23-25
has empty Planned. Full queue INDEX was read in the initial pass, and targeted gate/helper/test-name searches across all
coverage sources were repeated during synthesis with no matching item. T FILTER.md:24-42 could be interpreted broadly as
covering retryable test failures, but it does not clearly accept a test's missing readiness premise; retain per :19-20.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g189/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 81234061e87b7599c157b7b855a468c9c4b2414e (R).
Persistence rechecked directly at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a source-only
review; runtime occurrence and frequency were not measured.
