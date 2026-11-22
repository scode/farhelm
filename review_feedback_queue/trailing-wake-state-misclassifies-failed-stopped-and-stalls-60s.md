# Wake-on-launch misclassifies repeated wake failures, stalls 60s on stopped containers, and labels them suspended

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When launching a session on a stopped container or suspended sandbox whose wake fails after the 20-second reply bound
(or when a stopped container fails with the same error as before or stops again while waking), the launcher either hangs
for 60 seconds before claiming the host resumed and failed to connect, or fails immediately saying the host was
"suspended again while it was waking; resume it and launch again" — misdiagnosing the failure and labelling a stopped
container as suspended.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/launch/wake_state.rs:55-117`.

Updated exact line references at 66ad3aaa95ef03a2130e2d0194d58353912ae9da: In
`crates/gui/farhelm-gui/src/launcher/launch/wake_state.rs:55-117` (`Watch::observe`) and `wake_state.rs:209-269`
(`ResumeWait::observe`), combined with `crates/farhelm-helm/src/environment/lifecycle.rs:97-125` and
`crates/farhelm-helm/src/manager.rs:4410-4489`:

1. `Watch` never records that `HostStateView::TensorlakeChanging { direction: LifecycleDirection::Resuming, .. }` was
   observed (`wake_state.rs:61-63` leaves `connection_seen` false).
2. For a stopped Podman container (`HostStateView::Stopped`), `HostStateView::Stopped` has no timestamp, and
   `manager.rs:4454-4457` only updates `stopped.failed` during its one automatic start (`manager.rs:4458-4486`), not on
   explicit `POST /api/hosts/{id}/resume`. When an explicit Resume exceeds the 20-second host reply bound (returning 504
   `CARRIES_ON`) and later fails in `perform()` (`lifecycle.rs:120` calls `release_hold(host, true)`):
   - If the UI channel coalesces the brief `Connecting` state and pushes `Stopped` with the same `start_failed`,
     `Watch::observe` hits `HostStateView::Stopped { .. } if !self.connection_seen => Observation::Waiting`
     (`wake_state.rs:97`). Because `TensorlakeChanging { Resuming }` ended, `ResumeWait::observe` resumes the 60s budget
     and waits the full 60 seconds, then fails claiming "The host resumed but did not connect within 60 seconds" (or
     `CHECKOUT_CONNECTION_TIMEOUT`) even though the container never started.
   - If `Connecting` was delivered (`connection_seen == true`), `start_failed != initial_failure` (`wake_state.rs:94`)
     is false, so `Watch::observe` falls through to `_ => Observation::SuspendedAgain` (`wake_state.rs:98`), and
     `ResumeWait::observe` (`wake_state.rs:231-233`) fails with "Not launched. The host was suspended again while it was
     waking; resume it and launch again".
3. The same "suspended again" message fires whenever a stopped container stops again after connecting during wake,
   violating SPEC.md ("Container sessions", line 5344: "Stopped has its own state and is never labelled Suspended").
   Similarly, `crates/gui/farhelm-gui/src/launcher/launch/wake.rs:150` hardcodes "Not launched. Resuming the host
   failed: " for direct Resume refusals even when `started_stopped` is true.
4. For a Tensorlake sandbox that started in `Suspended { resume_failed: Some(msg), .. }` (from an earlier failed resume)
   or whose post-20s resume fails without changing `resume_failed`, `resume_failed != initial_failure`
   (`wake_state.rs:72`) is false, so returning to `Suspended` after `Connecting` reports `Observation::SuspendedAgain`
   instead of `Observation::Failed` (provider wake failure). Fix direction: Track in `Watch` whether
   `TensorlakeChanging { resuming: true }` was observed (and treat a return to `Suspended`/`Stopped` after `Resuming` as
   a provider wake failure rather than `Waiting` or `SuspendedAgain`), differentiate stopped-container wording from
   suspended-sandbox wording in `ResumeWait::observe` and `wake::reply`, and do not require
   `resume_failed`/`start_failed` text to differ from `self.start` once a new resume attempt has entered `Resuming` or
   `Connecting`.

Reviewer metadata (g10-wake-launch-state-and-requests/l1): suggested bucket: high (not highest: does not affect security
or lose user-owned data/work; degrades launch error handling and wastes up to 60s waiting on a failed container start);
confidence: confirmed; possible cover: none.

Additional source record at `9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75`,
`crates/gui/farhelm-gui/src/launcher/wake_tests.rs:214-278` (g12-wake-folder-launch-tests/f4):

Verified against 9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75 and pinned 66ad3aaa95ef03a2130e2d0194d58353912ae9da.

1. Test gap in g12: crates/gui/farhelm-gui/src/launcher/wake_tests.rs:214-248
   (a_late_resume_failure_keeps_the_provider_diagnosis) starts from fixture host 12 with resume_failed: None and pushes
   connecting(1, None) before the failure. a_stopped_container_starts_before_creating (lines 253-278) only tests the
   happy path (Stopped -> 200 -> Connected) and never tests a failed start or retry after start_failed: Some(...).
2. Shipped defect masked by this gap: In crates/gui/farhelm-gui/src/launcher/launch/wake_state.rs:61-99, Watch::observe
   never records seeing HostStateView::TensorlakeChanging, and only classifies a push as Observation::Failed when
   resume_failed != initial_failure (line 72) or start_failed != initial_failure (line 94). When a container already
   carries start_failed: Some(err) (e.g., after the helm's automatic start attempt at
   crates/farhelm-helm/src/manager.rs:4454-4463) or a sandbox already carries resume_failed: Some(err) and a subsequent
   launch's Resume answers 504 -> TensorlakeChanging(Resuming) -> returns to Stopped/Suspended with the same error
   string: (a) if Connecting was not pushed in between, Stopped hits line 97 (Observation::Waiting) and hangs for the
   full 60-second timeout instead of failing immediately (and Suspended with unchanged timestamps does the same at line
   83); (b) if Connecting was pushed in between, both hit Observation::SuspendedAgain (lines 85, 98), reporting "The
   host was suspended again while it was waking" instead of the provider failure diagnosis. Fix direction: Track whether
   TensorlakeChanging(Resuming) (or any departure from the starting state) was observed in Watch so a return to
   Suspended/Stopped with resume_failed.is_some()/start_failed.is_some() classifies as Observation::Failed even when the
   error text matches the prior failure; add tests in wake_tests.rs covering Podman late start_failed and repeat
   same-error resume_failed/start_failed.

Reviewer metadata (g12-wake-folder-launch-tests/f4): suggested bucket: other (could not be in highest: degraded failure
diagnosis or 60-second wait before failure, no security or data-loss impact); confidence: confirmed; possible cover:
none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/l1`,
`g12-wake-folder-launch-tests/f4`.
