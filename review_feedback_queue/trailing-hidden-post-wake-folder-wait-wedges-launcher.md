# A host reconnect during hidden folder-history read permanently wedges the launcher

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If a waking host's connection refreshes with a new connection incarnation while the hidden launcher is fetching the
host's recent folder history after wake, the history callback discards the reply and never retries or reopens the
launcher — leaving the "Resuming…" pill stuck on screen and permanently blocking new launches, host changes, and
Suspend/Resume on that host until a disconnect or UI reset.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/launch/wake.rs:214-219`.

1. After a host wakes and connects, `wake::follow` (`crates/gui/farhelm-gui/src/launcher/launch/wake.rs:254-271`)
   transitions `wake.wait.phase` to `Phase::Ready` and calls `launcher.continue_after_wake(...)`
   (`crates/gui/farhelm-gui/src/launcher/view.rs:2840-2851`).
2. When `self.draft.folder_default_waiting()` is true (an untouched default folder in New or a quick launch),
   `continue_after_wake` calls `start_or_wait_for_folder` (`view.rs:389-419`), which calls `self.follow_history(cx)`
   (`view/history.rs:95-116`) to send `GET /api/launch-history?host=<id>` for
   `key = HistoryKey { install, incarnation: N, verified }`, sets `self.folder_wait = Some((mark, window))`, and returns
   `true` so the launcher stays hidden in `Waiting::Preparing` with `wake: Some(Wake { phase: Phase::Ready, .. })`.
3. If the host reconnects with a bumped `incarnation: N+1` (or changed `Connected.identity` flipping `verified`) and the
   UI channel pushes the new `Connected` state without delivering an intermediate non-`Connected` state:
   - `wake::follow` (`wake.rs:214-219`) only checks `Install::of(host) == wake.install` and
     `matches!(host.state, HostStateView::Connected { .. })`, so it returns early without failing or refreshing the
     history read.
   - When the in-flight `GET /api/launch-history` reply arrives (`view/history.rs:135-139`),
     `launcher.history_key(cx).as_ref() != Some(&key)` is true, so the callback returns immediately without calling
     `launcher.history_settled_for_quick_launch(cx)` and without re-sending `GET /api/launch-history`.
   - Because the launcher's layer was closed (`view.rs:2833`), `Launcher::render` never runs and `follow_history` is
     never called again.
4. `LaunchSlot` remains stuck in `Waiting::Preparing` with `wake: Some(...)` and `WakePill` permanently displayed,
   keeping `launch::busy(cx)`, `hosts::launch_in_flight(cx)`, and `host_lifecycle_refusal(host, cx)` all true until the
   host disconnects or the UI session resets. (Note: for managed checkouts, `send_preview` calls `settle_waiting_launch`
   unconditionally after `finish`, which detects `preview_key != armed.key` and calls `reveal_prepared`, so managed
   checkouts do not wedge the same way.) Fix direction: Either record the connected `incarnation` when entering
   `Phase::Ready` in `Wake` and fail (or re-drive `continue_after_wake`) if `incarnation` changes, or in
   `view/history.rs:135-139` when `self.folder_wait.is_some()` and the key moved to another connected key, call
   `follow_history` / `history_settled_for_quick_launch` instead of silently returning.

Reviewer metadata (g10-wake-launch-state-and-requests/l6): suggested bucket: high (not highest: no security or data-loss
impact, but permanently wedges the desktop launcher, quick launches, host-registry changes, and Suspend/Resume on that
host until the host disconnects or the UI channel resets); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/view/history.rs:135` (g11-wake-launch-views-and-panes/l1):

In `crates/gui/farhelm-gui/src/launcher/view/history.rs:135-139` (added in commit
`3f6ea3b0cf6f55f82395569cf24be27e8e18d273`), `Launcher::send_history_read`'s reply callback now returns early whenever
`launcher.history_key(cx).as_ref() != Some(&key)` without calling `launcher.history_settled_for_quick_launch(cx)` (line
161). When a hidden launcher is waiting in `self.folder_wait` (`crates/gui/farhelm-gui/src/launcher/view.rs:407-410`),
reached either from `prepare_unseen` (quick launch) or from `continue_after_wake` (`view.rs:2847-2848`) after a host
wakes:

1. On an ordinary hidden quick launch (`prepare_unseen`), if the host disconnects, suspends, or reconnects with a new
   `incarnation` while `GET /api/launch-history` is in flight, `history_key(cx)` changes or becomes `None`. When the
   reply arrives, line 136 returns early without waking `history_settled_for_quick_launch(cx)`. Because the launcher is
   hidden (`self.layer` is `None`), `Render::render` never runs, so `follow_history` is never called again and
   `reveal_prepared` is never called.
2. On a post-wake hidden launch (`continue_after_wake`), `wake::follow`
   (`crates/gui/farhelm-gui/src/launcher/launch/wake.rs:214-219`) returns early whenever
   `wake.wait.phase == Phase::Ready` and `matches!(host.state, HostStateView::Connected { .. })` with matching
   `Install::of(host)`, without checking `incarnation` or `verified`. If a coalesced host update advances `incarnation`
   (or changes `verified`) while staying `Connected` during the in-flight `GET /api/launch-history`, `wake::follow` does
   not fail the wake and `send_history_read` returns early at line 138 without calling
   `history_settled_for_quick_launch(cx)`. `LaunchSlot` stays in `Waiting::Preparing` with `wake: Some(...)` and
   `WakePill` visible indefinitely, blocking `entry::new_available`, `entry::quick_launch_available`, and
   `launch::host_lifecycle_refusal` for that host. Note: Also checked whether `continue_after_wake`
   (`view.rs:2847-2848`) could drop `SourceConsent` for Replace with / Restart with when `folder_default_waiting()` is
   true; refuted by `crates/gui/farhelm-gui/src/launcher/draft.rs:555-576, 1020-1022, 1094-1109` (`ReplaceWith` and
   `RestartWith` start `FolderDefault::Settled` with `folder_install: None` and hold `Field::Host`, so
   `folder_default_waiting()` is unreachable for them). Fix direction: In `send_history_read`, when
   `launcher.folder_wait.is_some()` and the reply is discarded as stale (or when `history_key(cx)` no longer matches
   `key`), still invoke `launcher.history_settled_for_quick_launch(cx)` (which re-runs `start_or_wait_for_folder` ->
   `follow_history` to either retry on the new incarnation or fail/reveal the launcher via `reveal_prepared`).

Reviewer metadata (g11-wake-launch-views-and-panes/l1): suggested bucket: high (could not be in highest: does not lose
user data, credentials, or processes, but permanently wedges New, Clone, Replace with, Restart with, quick-launch
shortcuts, and host Suspend/Resume until UI session reset); confidence: confirmed; possible cover: none.

Additional source record at `9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75`,
`crates/gui/farhelm-gui/src/templates/dispatch_tests.rs:844-866` (g12-wake-folder-launch-tests/f1):

Verified against 9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75 and pinned 66ad3aaa95ef03a2130e2d0194d58353912ae9da.

1. Source trace: When a quick launch omits a folder destination (folder_default == FolderDefault::Pending) on a
   suspended host, wake::follow (crates/gui/farhelm-gui/src/launcher/launch/wake.rs:254-270) enters Phase::Ready on
   Connected and calls Launcher::continue_after_wake -> start_or_wait_for_folder
   (crates/gui/farhelm-gui/src/launcher/view.rs:401-410), which calls follow_history(cx) (issuing GET
   /api/launch-history?host=<id> under HistoryKey { install, incarnation, verified }) and parks with self.folder_wait =
   Some((mark, window)) while held.waiting remains Waiting::Preparing and wake.alarm is None.
2. If the host receives a coalesced Connected update with a new incarnation (or a changed Connected { identity } that
   flips HistoryKey::verified) while GET /api/launch-history is in flight: wake::follow (wake.rs:214-219) sees
   wake.wait.phase == Phase::Ready and matches!(host.state, HostStateView::Connected { .. }) and returns early without
   failing or re-driving follow_history. Then when GET /api/launch-history replies, send_history_read
   (crates/gui/farhelm-gui/src/launcher/view/history.rs:135-139) checks
   `if launcher.history.asked.as_ref() != Some(&key) || launcher.history_key(cx).as_ref() != Some(&key) { return; }` and
   returns early BEFORE calling launcher.history_settled_for_quick_launch(cx) (line 161). Because the launcher is
   hidden, render() never runs to call follow_history again, leaving Waiting::Preparing, WakePill, launch::busy(cx),
   hosts::launch_in_flight(cx), and host_lifecycle_refusal(host, cx) wedged indefinitely. (Note: the same early return
   in history.rs:135-139 can also wedge an ordinary non-wake hidden quick launch if the host disconnects or changes
   incarnation during GET /api/launch-history; contrast checkout.rs:754 which calls Launcher::settle_waiting_launch
   unconditionally.)
3. Test gap in g12: crates/gui/farhelm-gui/src/templates/dispatch_tests.rs:844-866
   (a_suspended_quick_launch_returns_if_the_host_drops_out_during_history) only pushes HostStateView::Unreachable during
   the history read, never a Connected push with an incremented incarnation (or changed verified state), so it missed
   this permanent wedge. Fix direction: In history.rs:135-139, still invoke
   launcher.history_settled_for_quick_launch(cx) (or re-drive start_or_wait_for_folder / fail the wake in
   wake.rs:214-219 when the connected incarnation/key changes) so a stale history reply cannot strand folder_wait; add a
   test in templates/dispatch_tests.rs pushing a Connected update with a new incarnation while GET /api/launch-history
   is in flight.

Reviewer metadata (g12-wake-folder-launch-tests/f1): suggested bucket: high (could not be in highest: no security,
credential, or data-loss impact, but permanently wedges the desktop window's launcher, template shortcuts, host registry
changes, and that host's Suspend/Resume until the host disconnects or the app reconnects to the helm); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/l6`,
`g11-wake-launch-views-and-panes/l1`, `g12-wake-folder-launch-tests/f1`.
