# Replace with on a suspended checkout host can launch a fresh clone without showing the checkout warning

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

In Replace with on a session in a managed checkout, if the host suspends while the launcher is open and you press Down
then Enter on a retained managed-checkout recent setup, the launcher immediately wakes the host, starts a fresh clone
without the source checkout's changes, and deletes the source session (moving the old checkout to trash if it was the
last session) — even though the only screen you saw promised that the replacement keeps working in the existing checkout
with its changes and never showed the fresh-clone / trash warning.

## Details

**highest.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/request.rs:395-397`.

Revised from Turn-1 candidate `l4` after cross-group check with `g11` (`l2`):

1. When Replace with opens on a session in a managed checkout, it starts in Folder mode and renders
   `replacement_folder_hint` (`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:964-1002`): "This session's managed
   checkout of <repo>. The replacement keeps working in it, with its changes."
2. During render (`crates/gui/farhelm-gui/src/launcher/view.rs:2148-2150`),
   `self.source_frame.capture(source, (), window)` records `SourceConsent { stop_working, delete }` (`view.rs:565-568`).
   `SourceConsent` does NOT record whether `replacement_checkout_warning` (`view/checkout.rs:1007-1030`) was rendered.
3. If the host suspends while Replace with is open, `view/history.rs:206-247` (`retains_checkout_setups` /
   `command_pane_history`) retains that host's managed-checkout setups in the empty command pane.
4. Pressing Down and Enter on a retained managed-checkout row calls `Launcher::activate_start_row`
   (`crates/gui/farhelm-gui/src/launcher/view/search/start.rs:139-146`), which calls
   `self.take_recent(&entry, RecentSurface::CommandPane, window, cx)` (`view/recents.rs:101-135` ->
   `self.draft.choose_repository(repo.clone())` at line 121, switching to Managed checkout) and immediately calls
   `self.press(window, cx)` at line 145 in the same event handler, before any new frame draws
   `replacement_checkout_warning`.
5. Before commit `3f6ea3b0`, `LaunchDraft::checkout_refusal` (`crates/gui/farhelm-gui/src/launcher/request.rs:398-401`)
   returned `Some(WAIT_FOR_PREVIEW)` on a connected host because `target.preview` was `Preview::Waiting`, preventing
   same-tick submission and forcing a render showing `replacement_checkout_warning` and the preview. Commit `3f6ea3b0`
   added `if self.can_wake(host) { return None; }` at `request.rs:395-397`, removing that barrier for resumable hosts.
6. Consequently, `self.refusal(cx)` is `None`, `press_at` (`view.rs:1297-1310`) accepts the previous Folder-mode frame's
   `SourceConsent`, and `submit` -> `begin_host_wake` -> `wake::start` immediately hides the launcher, wakes the host,
   obtains a fresh preview, creates a fresh clone without the source checkout's changes, and deletes the source session
   (moving the old checkout to trash if it was the last session, violating SPEC.md "Managed checkouts" lines 1744-1746
   and SPEC_impl.md "Questions: the prompt"). Note: The other properties checked in Turn-1 `l4` (`SourceConsent`
   threading `DeleteGuard`/`stop_working` across wake, `connected_host` guards in
   `create_body`/`replace_body`/`restart_body`, `Install::of` checks, and `wake_owns_source`/`targets_host`/`waking`
   exclusion locks) remain verified clean. Fix direction: Include whether the replacement leaves the source's checkout
   (requiring `replacement_checkout_warning`) in `SourceConsent` / `source_frame` (or clear `source_frame` in
   `changed()` / require a drawn frame after switching from Folder to Managed checkout in Replace with before `press`
   can submit), so same-tick `take_recent` + `press` cannot bypass `replacement_checkout_warning`.

Reviewer metadata (g10-wake-launch-state-and-requests/l4): suggested bucket: highest (destructive action —
replacing/deleting the source session and moving its managed checkout to trash while starting from a fresh clone without
its changes — proceeds beyond what the displayed warning promised, which explicitly stated the replacement keeps working
in the checkout with its changes; per review_feedback_queue/AGENTS.md Priority buckets, harm beyond what a destructive
warning stated stays highest); confidence: confirmed; possible cover: none (cross-group overlap with g11 finding l2).

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/view/search/start.rs:138` (g11-wake-launch-views-and-panes/l2):

Before commit `3f6ea3b0cf6f55f82395569cf24be27e8e18d273`, on a connected host, pressing Enter on a managed-checkout
recent setup in Replace with (`crates/gui/farhelm-gui/src/launcher/view/search/start.rs:138-147`) called `take_recent`
(which switched `self.draft.checkout` from `None` to `Some(repo)` at
`crates/gui/farhelm-gui/src/launcher/view/recents.rs:120-121`) and then immediately called `self.press(window, cx)`.
Because the host was connected, `checkout_refusal` (`crates/gui/farhelm-gui/src/launcher/request.rs:398-401`) returned
`WAIT_FOR_PREVIEW`, refusing immediate launch and keeping the dialog open so the user saw `replacement_checkout_warning`
(`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1007-1030`: "Starts from a fresh clone without the changes in
this session's checkout of <repo>. If this replaces its last session, the old checkout moves to the trash...") before
pressing Replace again. With commit `3f6ea3b0`, when an environment host suspends while Replace with is open:

1. `command_pane_history` (`crates/gui/farhelm-gui/src/launcher/view/history.rs:234-247`) retains the verified
   installation's managed-checkout recent setups in the empty command pane.
2. Replace with is initially in Folder mode on the source's checkout directory, displaying `replacement_folder_hint`
   (`checkout.rs:986-989`: "This session's managed checkout of <repo>. The replacement keeps working in it, with its
   changes.") and NOT displaying `replacement_checkout_warning`.
3. Pressing Down and Enter on a retained managed-checkout recent setup calls `take_start_row` (`start.rs:139-146`),
   which calls `take_recent(&entry, RecentSurface::CommandPane, ...)` (switching to Managed checkout) and immediately
   invokes `self.press(window, cx)` in the same event handler before any redraw.
4. `press_at` (`view.rs:1297-1310`) reads `self.source_frame.current().and_then(|frame| frame.shown().copied())`, which
   returns the `SourceConsent` captured from the previous frame (when the dialog showed Folder mode keeping the
   checkout's changes). `checkout_refusal` (`request.rs:395-397`) returns `None` because `self.can_wake(host)` is
   `true`.
5. `begin_host_wake` (`view.rs:2808-2836`) immediately hides the launcher, wakes the host, obtains a checkout preview
   out of sight (`submit_or_wait_for_preview`), creates a fresh clone, and deletes the source session (moving its
   checkout to trash if last session, where deleting the sandbox/container also deletes its trash). This violates
   `SPEC.md:1744-1746` ("If the source has a managed checkout, choosing Managed checkout in Replace with warns under
   Where that the replacement starts from a fresh clone without the source checkout's changes"), `SPEC.md:1173-1174`
   ("launches that retire a session (Replace with) limited to the action and consent already shown"), and
   `SPEC_impl.md:993-997` (render-binding). Note: The other parts of lead `l2` were checked separately: `confirm_yolo`
   across wake is recorded as `f3`, and `permits_launch_preparation` / `allows_launch_with` in `environment.rs`,
   `menu.rs`, `header.rs`, and `entry.rs` were verified and recorded as `c3`. Fix direction: In `take_start_row`
   (`start.rs:144-146`), do not immediately call `self.press` when `take_recent` changed Replace with's destination away
   from the source's managed checkout (or switched into Managed checkout when `replacement_checkout_warning` was not yet
   drawn); let the form redraw with the fresh-clone warning so the user presses Enter/Replace again to confirm it.

Reviewer metadata (g11-wake-launch-views-and-panes/l2): suggested bucket: highest (or high if commissioner judges moving
the checkout to trash rather than permanently deleting it stays outside highest; recorded as highest because Replace
with is a destructive confirmation that deletes the source session and moves its checkout to trash under a contradictory
on-screen promise that "The replacement keeps working in it, with its changes" — see `review_feedback_queue/AGENTS.md`,
"Priority buckets": "what it says is the limit of what the user agreed to, so harm beyond it keeps highest");
confidence: confirmed; possible cover: none.

Distinct from `replace-can-submit-before-checkout-warning.md` (which covers a Wayland deferred-presentation race on a
connected host): here commit `3f6ea3b0`'s `can_wake` bypass in `request.rs:395-397` lets the very first Enter press
submit synchronously without ever drawing `replacement_checkout_warning`.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/l4`,
`g11-wake-launch-views-and-panes/l2`.
