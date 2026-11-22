# Post-wake disconnect or launch start failure can reopen the launcher with no refusal alert

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If a waking host disconnects or changes installation right as its post-wake checkout preview or launch step completes,
the launcher can reopen (or post a launch-not-started notice) with no error alert explaining why the session did not
start.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/launch/wake.rs:254-298`.

`crates/gui/farhelm-gui/src/launcher/launch.rs:275-282` explicitly anticipates that a stale preview or history callback
can run before `LaunchSlot`'s observer (`wake::follow`) reacts to a host change.

1. In `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:561-589` (`settle_waiting_launch`), if the host disconnected
   or changed installation before `wake::follow` ran, `launcher.preview_key(cx)` returns `None` (`!= Some(&armed.key)`),
   so `start` becomes `false`, and line 588 immediately calls
   `launch::reveal_prepared(entity, armed.window, armed.mark, cx)`. `reveal_prepared` -> `come_back` replaces
   `LaunchSlot`'s `Held` (clearing `wake` and `Waiting::Preparing`) and shows the launcher without setting
   `self.pressed = true` or `Note::WakeRefusal`. When `wake::follow` (or any already-queued `wake::reject` deferred
   closure at `wake.rs:291-297`) runs next, `wake` is gone / `still_preparing` is false, so "Not launched. The host
   became unavailable before the session could launch" is never set.
2. Similarly, in `crates/gui/farhelm-gui/src/launcher/view.rs:1411-1417` (`Launcher::submit`), when
   `(Some(request), Some(host))` succeeds but `launch::start(...)` returns `false` (e.g., `wake_allows_create` returns
   `false` at `launch.rs:552` because `wake.ending == true` or the host just disconnected, or `operations.start` fails),
   `submit` neither sets `self.pressed = true` nor sets a refusal note. The caller (`wake.rs:268`, `view.rs:439`,
   `view/checkout.rs:588`) then calls `reveal_prepared`, clearing `Waiting::Preparing` and reopening the launcher
   silently with no refusal alert. Note: `WakePill` handoff (`launch.rs:585-606`, `layers.rs:2078-2098`,
   `wake.rs:308-314`) and create blocking (`wake_allows_create`) were verified clean. Fix direction: When `wake::follow`
   is invoked or when `launch::start` fails due to `!wake_allows_create` / `operations.start` failure, preserve or set a
   refusal note (or run `wake::follow` before `settle_waiting_launch` / `reveal_prepared`) so the reopened launcher
   always explains why nothing launched.

Reviewer metadata (g10-wake-launch-state-and-requests/l3): suggested bucket: other (not highest: create is still safely
blocked by wake_allows_create, no security or data loss; UX/diagnostic defect when launcher reopens silently without
explaining why launch did not start); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/l3`.
