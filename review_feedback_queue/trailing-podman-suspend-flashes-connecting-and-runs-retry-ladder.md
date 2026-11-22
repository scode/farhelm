# Podman Suspend still flashes Connecting and post-Suspend release_hold(false) runs an active retry ladder

Reviewed commit: ba8f163fb26383ecab6c9642fbd4299ca1463003

## TLDR

After suspending a container session, its host briefly flashes "Connecting" instead of moving directly from "Suspending"
to "Suspended". In addition, after any successful Suspend of a connected sandbox or container, if the immediate
post-Suspend status check hits a transient failure, Farhelm runs the full one-minute active reconnect ladder instead of
a single quiet check.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/manager.rs:4549`.

Commit ba8f163fb26383ecab6c9642fbd4299ca1463003 attempted to stop the post-Suspend Connecting flash in
`crates/farhelm-helm/src/manager.rs:4549-4557` by suppressing `HostState::Connecting` publication on `attempt == 0` only
when `self.status.borrow().state` matches `HostState::Suspended { suspended_by_farhelm_at: Some(_), .. }`. Two problems
remain:

1. In `crates/farhelm-helm/src/environment/podman.rs:793-825`, `Podman::change` passes `|_| async { Ok(None) }` as the
   `record` callback to `environment::lifecycle::perform` (`crates/farhelm-helm/src/environment/lifecycle.rs:109-111`),
   so `perform` never publishes `HostState::Suspended` under the hold before calling `manager.release_hold(host, false)`
   (`lifecycle.rs:120`). Because the container was `Connected` before Suspend, `HostActor::serve`
   (`manager.rs:4300-4317`) exits on the hold nudge and sets `active = true` (`manager.rs:4317`). When the actor parks
   at `manager.rs:4201-4208` and wakes on `release_hold(host, false)`, line 4206 computes
   `active |= request.fresh_window`, keeping `active = true`. In `connect_phase` (`manager.rs:4549-4557`), the current
   state is still `HostState::TensorlakeChanging { direction: Suspending, .. }`, so `confirmed_suspend` is `false` and
   `connect_phase` immediately publishes `HostState::Connecting { attempt: 0, .. }` while `Podman::status` runs
   `podman inspect` (which is an SSH round-trip on remote container hosts) before settling to `Suspended`.
2. Even for Tensorlake hosts, `release_hold`'s contract (`manager.rs:1865-1869`) promises one quiet probe when
   `fresh_window == false` and the full retry ladder only when `fresh_window == true`. Because `active` stays `true`
   across the hold (`manager.rs:4317` + `4206`), `connect_phase` receives the full `connect_backoff` ladder
   (`manager.rs:4213-4217`) and `active = true`. `confirmed_suspend` only suppresses `Connecting` on `attempt == 0`; any
   transient failure on that check falls into `attempt > 0`, publishing `Connecting` across the full retry ladder. Fix
   direction: In `HostActor::run`, when a hold ends with `fresh_window == false`, reset `active = request.fresh_window`
   (so `release_hold(host, false)` actually runs a single quiet probe with `&[]` ladder and `active = false`, matching
   its doc comment), and have `Podman::change` return a confirmed `HostState::Suspended` (with
   `suspended_by_farhelm_at: None` or an appropriate observation timestamp) from `perform`'s `record` closure when
   `resume == false` so the state transitions cleanly from `Suspending` to `Suspended` without waiting for the follow-up
   inspect.

Reviewer metadata (g07-tensorlake-runtime/l4): suggested bucket: other (could not be in highest: UI status flicker and
unintended retry ladder after suspend; no security, credential, or user data/process loss); confidence: confirmed;
possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g07-tensorlake-runtime/l4`.
