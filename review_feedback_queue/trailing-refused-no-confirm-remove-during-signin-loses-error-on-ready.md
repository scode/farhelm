# Refused no-confirmation Remove during SSH sign-in loses its error when Ready replays immediately

Reviewed commit: 1ec7724e3cd0e8353c75af6f9b65842795518e9d

## TLDR

With "Remove hosts without asking" enabled, if you click Remove on an SSH host while a Setup, Update, or Uninstall
sign-in finishes and the helm refuses or times out the Remove, the refusal message disappears immediately before you can
see it because the desktop replays the pending setup/update/uninstall right away and clears the host's error slot.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/hosts/setup_sign_in.rs:205`.

In crates/gui/farhelm-gui/src/hosts/remove.rs:145-152, when "Remove hosts without asking" is enabled
(`!asks_first(cx)`), `remove::start` calls `submit(&removal, None, cx)` with no confirmation dialog, relying solely on
`Operations::failures` under `OpKey::host` to show any helm refusal under the host row (`remove.rs:26-27`,
`remove.rs:342-344`, `remove.rs:375-377`). When `Work::RemoveHost` runs over a detached setup sign-in hold
(`crates/gui/farhelm-gui/src/operations.rs:973-1011`) and `SshSignInOutcome::Ready` arrives while
`DELETE /api/hosts/{id}` is on the wire, `Waiting::changed` sets `self.ready = true` and waits for `RemoveHost` to
settle (`crates/gui/farhelm-gui/src/hosts/setup_sign_in.rs:184-187, 205-210`). When `DELETE` settles with a refusal
(e.g., 409 host busy or 504 reply timeout from `crates/farhelm-helm/src/hosts.rs:1290-1303`), `Operations::settle`
(`operations.rs:1046-1071`) restores the `setup_sign_in` hold, calls `cx.notify()`, and inserts the `Work::RemoveHost`
refusal into `self.failures`. Immediately during the same GPUI effect flush, `Waiting`'s `_operation_watch`
(`setup_sign_in.rs:83-85`) runs `Waiting::changed`, which sees
`self.ready && pending_sends == 0 && in_flight != Some(Work::RemoveHost)`, calls `self.finish(Ok(()), cx)`, releases the
hold, and defers `setup_sign_in::plan` (`setup_sign_in.rs:241-243`). That deferred `operations.start(&key, replay, ...)`
calls `self.admit(key)` (`operations.rs:980, 1027`), which executes `self.failures.remove(key)` before any frame renders
the Remove refusal. How to verify: in `crates/gui/farhelm-gui/src/hosts/setup/tests.rs`, run a variant of
`remove_starts_during_host_setup_sign_in_and_defers_ready_replay` with `skip_host_remove_confirmation: true` where
`Ready` arrives during `DELETE`, `DELETE` replies 409, and the replayed probe replies 200/202 — the Remove refusal is
never shown on the host row. Fix: when `RemoveHost` without a confirmation dialog is refused (or in general when
`remove::submit` receives a refusal while `setup_sign_in` is about to replay), surface the Remove refusal on a surface
that starting the replay does not overwrite (such as the host refusal dialog, as noted in the TODO at
`remove.rs:148-150`, or a panel line), or avoid silently clearing a `RemoveHost` failure when an automatic sign-in
replay starts.

Reviewer metadata (g33_remove_host_signin_lock_gui/f1): suggested bucket: other (narrow timing overlap requiring "Remove
hosts without asking" plus Ready arriving during a refused/timed-out Remove; no security or data-loss impact, only a
lost error display for the failed Remove).; confidence: confirmed; possible cover: none (SPEC_impl.md, "Helm ↔
supervisor transport: system ssh + stdio protocol":2784-2786 specifies that a Ready arriving during Remove replays after
the DELETE reply, and remove.rs:148-150 notes a TODO to port the old short host refusal dialog for no-confirmation
Remove, which would avoid sharing `Operations::failures` with the replay).

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g33_remove_host_signin_lock_gui/f1`.
