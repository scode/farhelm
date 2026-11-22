# The coordinator does not enforce the remaining action budget at retry launch

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The release verifier can start a retry whose action allowance extends past the shared checkpoint deadline.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d releasing/mac-vm-test/BRICK-TEST.md:382–396;
releasing/mac-vm-test/OPERATIONS.md:120–130,154–158; releasing/mac-vm-test/brick_test.py:793–815,866–875,897–923;
releasing/mac-vm-test/guest_workflow.py:557–612; releasing/mac-vm-test/guest_tester.py:761–806

The verifier can let retry actions run past the original checkpoint deadline even if the preceding backoff and turns
were charged correctly. The documented shared budget includes initial execution, backoff and retries; the operational
contract gives each guest child group its own execution deadline. Pass-level guest readiness, credential injection, and
setup are done before the checkpoint clock, and the 30-second host transport cushion explicitly allows cleanup and
diagnostics. Neither is the finding. The coordinator starts a host deadline using the effective floor and uses elapsed
wall time after an earlier attempt, so earlier staging, cleanup/export, and backoff already reduce later eligibility.
After computing the remaining seconds it saves the claim, stages fresh guest files, and launches a guest executor that
creates its working files and spawns its child before starting a new countdown for that same relative remainder. No
deadline recheck or remaining-time refresh occurs at that boundary. This applies even though staging itself is
separately bounded by the default Tart command limit (control.py:68–72,129–139). The docs do not itemize whether every
per-attempt setup or export operation must be charged; the narrower mismatch is granting a stale remainder to guest
actions under the coordinator's original checkpoint deadline, not evidence export finishing later. The fixture at
test_native_coordinator.py:287–315 checks floor, elapsed first attempt and backoff but replaces the live dispatch chain
with a callback. Preserve pre-checkpoint setup and post-action cleanup separately, and revalidate the action budget at
actual retry dispatch. The reviewer suggested other, confirmed clock mismatch; elapsed time and a real overrun are
unverified. , Possible coverage: null.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `test-and-release-tools/trl-06`.
