# Binary preflight accepts execute permission that the caller does not have

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Binary preflight can mark an inaccessible executable as available and misreport the resulting launch failure as drift.

## Details

The new drift case uses which_binary to distinguish availability at
crates/farhelm/tests/e2e/real_agent_capture/drift.rs:601–607. The shared helper at
crates/farhelm/tests/e2e/real_agent_capture.rs:1317–1328 checks that the file is regular and that any execute mode bit
is set. This does not check whether the caller has permission to execute it. If no usable candidate exists in the launch
environment, a mode bit for another owner or group can get this case past the binary prerequisite. The attempt then
enters the ordinary launch path at drift.rs:625–650. Its screen classifier at :275–301 has no unavailable case for
execution denial, and launch and terminal errors instead produce Failed (:23–32, :418–443, :683–699).
docs/harness-drift-tests.md:34–44 separates missing prerequisites from failed checks. Use an effective execution-access
check in the PATH lookup and continue to the next entry if the candidate is not executable to the caller. Keep genuine
launch errors reportable after preflight. No change in the pinned range corrected the helper for the drift case.

Confidence: confirmed by inspection of the mode-only predicate and verdict branches, conditional on the caller having no
usable executable in the launch PATH.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l18`.
