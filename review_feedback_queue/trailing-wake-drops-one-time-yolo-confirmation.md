# One-time YOLO confirmation is dropped when the target host wakes before submit

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If a sandbox or container host becomes suspended or stopped while the launcher is showing the inline YOLO confirmation,
clicking "Start YOLO session anyway" wakes the host but discards the YOLO approval, so the launch is immediately refused
again and bounces back to the same YOLO confirmation.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view.rs:1385`.

Bucket note: `g10/l5` suggested `high` while `g11/f3` and `g12/f2` suggested `other`; placed in `other` because it fails
safe by re-asking once on a narrow re-suspend timing window.

In `crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:241-245`, answering the launcher YOLO question calls
`launcher.submit(true, false, source, mark, window, cx)` (`confirm_yolo = true`). In
`crates/gui/farhelm-gui/src/launcher/view.rs:1371-1387`, `submit` clears `self.yolo = None` and immediately delegates to
`self.begin_host_wake(source, mark, window, cx)` when `self.draft.can_wake(host)` is true, without passing
`confirm_yolo`. `Wake` (`crates/gui/farhelm-gui/src/launcher/launch/wake.rs:26-40`) does not store `confirm_yolo`, and
`Launcher::continue_after_wake` (`view.rs:2840-2851`) calls `start_or_wait_for_folder` or `submit_or_wait_for_preview`,
both of which eventually invoke `self.submit(false, false, ...)` (`view/checkout.rs:524,581`) with
`confirm_yolo = false`. When the host is suspended or stopped while the inline YOLO confirmation is open (for example
after idle timeout or suspend from another client), clicking "Start YOLO session anyway" passes `self.refusal(cx)`
(because `can_wake` makes the host launchable), wakes the host, and sends the create/replace/restart with
`confirm_yolo: false`. The helm refuses it with `HelmMarker::YoloConfirmation` again, reopening the same confirmation.
Fix direction: Carry `confirm_yolo: bool` through `begin_host_wake`, `wake::start` / `Wake`, `continue_after_wake`, and
`submit_or_wait_for_preview` / `Armed` (only for the host installation that was confirmed, since `Wake` already aborts
if `Install::of(host)` changes).

Reviewer metadata (g10-wake-launch-state-and-requests/l5): suggested bucket: high (not highest: fails safe by re-asking
for YOLO confirmation rather than bypassing it; degrades UX by discarding an explicit user confirmation and bouncing
back to the same prompt); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/view.rs:1385` (g11-wake-launch-views-and-panes/f3):

In `crates/gui/farhelm-gui/src/launcher/view.rs:1371-1387`, `Launcher::submit` receives `confirm_yolo: bool` (passed as
`true` from `yolo_question.rs:243` when the user confirms "Start YOLO session anyway"). Before building the request,
line 1385 calls `self.begin_host_wake(source, mark, window, cx)`, which does not pass or store `confirm_yolo`. When the
host finishes resuming and connects, `Launcher::continue_after_wake` (`view.rs:2840-2851`) calls
`submit_or_wait_for_preview` (`checkout.rs:500-526`), which calls `self.submit(false, false, source, mark, window, cx)`
(`checkout.rs:524`, `checkout.rs:581`) with `confirm_yolo = false`. The helm therefore refuses the launch with
`HelmMarker::YoloConfirmation` again after the wake. Fix direction: Either carry `confirm_yolo` through `Wake` and
`Armed` (only for the same frozen installation/draft) or note that re-asking is intended; carrying it on `Wake` matches
`SPEC_impl.md:1002-1003` ("An answer is accepted after that warning is shown and carries its permission through a wait
for a host setting or checkout preview").

Reviewer metadata (g11-wake-launch-views-and-panes/f3): suggested bucket: other (could not be in highest: fails safe by
re-prompting YOLO confirmation rather than bypassing it); confidence: confirmed; possible cover: none.

Additional source record at `9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75`,
`crates/gui/farhelm-gui/src/launcher/view.rs:1371-1387` (g12-wake-folder-launch-tests/f2):

Verified against 9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75 and pinned 66ad3aaa95ef03a2130e2d0194d58353912ae9da (interface
between g10/g11 production wake code and g12 wake_tests.rs).

1. Clicking "Start YOLO session anyway" invokes launcher.submit(true, false, source, mark, window, cx)
   (crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:241-244).
2. Launcher::submit (crates/gui/farhelm-gui/src/launcher/view.rs:1371-1387) calls self.begin_host_wake(source, mark,
   window, cx) without forwarding confirm_yolo. When the host wakes and connects, Wake calls
   launcher.continue_after_wake(consent, mark, window, cx) -> submit_or_wait_for_preview -> self.submit(false, false,
   source, mark, window, cx) (crates/gui/farhelm-gui/src/launcher/view/checkout.rs:524), sending confirm_yolo: false and
   triggering YoloConfirmation again.
3. No test in wake_tests.rs exercises a YOLO-confirmed submit waking a re-suspended host. Fix direction: Carry
   confirm_yolo (or an explicit one-time override flag) in Wake / continue_after_wake / Armed, or verify whether SPEC.md
   wants a re-suspension to require re-confirming YOLO; add a regression test in wake_tests.rs.

Reviewer metadata (g12-wake-folder-launch-tests/f2): suggested bucket: other (could not be in highest: fails safe by
re-asking the YOLO confirmation rather than bypassing consent); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/l5`,
`g11-wake-launch-views-and-panes/f3`, `g12-wake-folder-launch-tests/f2`.
