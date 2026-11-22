# Folder-wake tests omit 60s timeout expiry, host removal/retarget, and Replace with submission

Reviewed commit: 9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75

## TLDR

The automated tests for waking a suspended or stopped host before launching into an existing folder do not verify that
the 60-second retry or reconnection timeouts actually fire, that removing or retargeting the host mid-wake cancels the
launch, or that Replace with completes with the confirmation captured when you pressed Replace.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/wake_tests.rs:1-328`.

Verified against 9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75 and pinned 66ad3aaa95ef03a2130e2d0194d58353912ae9da (revised
in Turn 2 to include the weak assertion at line 327). In crates/gui/farhelm-gui/src/launcher/wake_tests.rs (and across
crates/gui/farhelm-gui/src/):

1. a_504_before_the_provider_phase_keeps_the_launch_preparing (lines 123-148) advances 75 seconds while Resuming is
   active and then connects, proving the budget did not expire during Resuming, but no test in wake_tests.rs ever
   exercises either 60-second timeout firing: neither the folder connection timeout (wake_state.rs:253) nor the
   busy-retry timeout (wake_state.rs:249). If budget expiry were disabled for folder launches or busy retries, every
   test in wake_tests.rs would still pass.
2. Neither host removal (wake.rs:210) nor installation retarget (wake.rs:211-213) during wake is tested anywhere in
   crates/gui/farhelm-gui/src/.
3. a_replacement_holds_its_source_through_a_wake_refusal (lines 300-328) only tests a 409 Resume refusal, and at line
   327 only asserts `shown_refusal(cx).is_some()` without asserting the exact refusal words, `launch-wake` dismissal, or
   that no `/api/sessions/a/replace` was sent after the refusal; no test verifies Replace with completing after wake and
   sending POST /api/sessions/{id}/replace carrying the DeleteGuard frozen at press time. (Restart with wake omission is
   recorded under l1; visible New FolderDefault::Pending under f3; Podman/repeat provider failure under f4.) Fix
   direction: Add deterministic GPUI window tests in wake_tests.rs covering 60-second folder connection timeout
   (including after Resuming transitions to Connecting), 60-second HostBusy retry timeout, host removal and installation
   retarget during wake, Replace with post-wake submission asserting the frozen DeleteGuard, and tighten line 327's
   refusal/cleanup assertions.

Reviewer metadata (g12-wake-folder-launch-tests/l2): suggested bucket: other (could not be in highest: test assertion
gaps in maintainer test suite without an established security or data-loss defect in shipped code); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g12-wake-folder-launch-tests/l2`.
