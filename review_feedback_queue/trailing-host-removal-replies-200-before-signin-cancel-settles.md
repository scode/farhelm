# Host removal replies 200 OK before the cancelled detached sign-in task closes its question and records cancellation

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When you remove an SSH host while a Setup, Update, or Uninstall sign-in question is open, the helm can reply that the
removal succeeded slightly before the background sign-in task has actually closed the question, stopped the SSH child,
and recorded cancellation. On the desktop UI channel, the reply can therefore arrive while the removed host's sign-in
question is still briefly listed and its operation hold is momentarily restored until the next pushed update.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/setup_sign_in.rs:408`.

In `ProvisioningService::delete_registered_host` (`crates/farhelm-helm/src/provisioning/service.rs:2643-2659` at
`1b3fe49af86979aebc2719965682a17f8c6366ac`; lines 2626-2641 at `1ec7724e3cd0e8353c75af6f9b65842795518e9d`), after
committing `remove_host_row`, the helm calls `setup.cancel_host(host)`
(`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:408-413`; lines 221-226 at
`1ec7724e3cd0e8353c75af6f9b65842795518e9d`). `cancel_host` only extracts matching attempts from `self.active` and calls
`let _ = attempt.cancel.send(());`, returning synchronously without awaiting the spawned sign-in task's cancellation
settlement. Inside the spawned task (`setup_sign_in.rs:335-371`), dropping `wait` (which synchronously kills the SSH
process group via `InteractiveChild::drop` and removes the question via `QuestionLease::drop`) and recording
`SshSignInOutcome::Failed { reason: "SSH sign-in was cancelled" }` only happen when that task is next polled by the
Tokio runtime. Consequently, `DELETE /api/hosts/{id}` can finish and trigger the UI channel's pre-reply collection check
(`SPEC_impl.md`, "The UI channel": "When the GUI sees 'done', its store already shows the result") while
`Questions::list()` still contains the removed host's `SshQuestion` and `SetupSignIn::list()` does not yet contain the
`Failed` completion. In `crates/gui/farhelm-gui/src/operations.rs:1040-1053`, receiving the `DELETE` reply before
`Waiting` observes the `Failed` completion also momentarily restores the `setup_sign_in` hold on the deleted host's key
until the later feed bump arrives. Note that the new test `ssh_setup_preflight_returns_pending_then_allows_the_probe`
(`crates/farhelm-helm/src/provisioning.rs:6899-6907`) itself awaits a later `feed.changed()` via `completed_attempt`
after `DELETE` returns `200 OK` before asserting `questions.list().is_empty()`.

How to verify: Inspect `setup_sign_in.rs:335-371` and `408-413` alongside `service.rs:2644-2659` and
`provisioning.rs:6899-6907`. Awaiting `cancel_host` via each cancelled attempt's `outcome` watch
(`outcome.wait_for(Option::is_some)`) is non-blocking because `tokio::select!` with `biased; _ = cancel => ...`
immediately drops `wait`, and `InteractiveChild::drop` only sends `SIGKILL` (`libc::killpg`) and aborts the stderr drain
task without waiting synchronously for child exit.

Fix: Make `SetupSignIn::cancel_host` `async` and await `outcome.wait_for(Option::is_some)` (or a cancellation completion
signal) for each cancelled attempt before `delete_registered_host` returns. Then `Questions::list()`,
`SetupSignIn::list()`, and the SSH process group's `SIGKILL` are all settled before `DELETE /api/hosts/{id}` replies
`200 OK`.

Reviewer metadata (g32_remove_host_signin_lock_helm/f3): suggested bucket: other (Brief transient ordering inconsistency
between the `DELETE` reply and the subsequent feed push; the cancelled task still settles milliseconds later, `biased`
selection prevents late answers from reaching SSH, and no data/security harm occurs.); confidence: confirmed
(established directly from source analysis of `setup_sign_in.rs`, `service.rs`, `ssh_sign_in.rs`, `askpass.rs`,
`operations.rs`, and `provisioning.rs`.); possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g32_remove_host_signin_lock_helm/f3`.
