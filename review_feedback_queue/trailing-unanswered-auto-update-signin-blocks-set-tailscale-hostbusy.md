# Unanswered automatic-update SSH sign-in holds ssh_route_lock for up to 120s, blocking Tailscale host mode changes as HostBusy

Reviewed commit: 8026bea2ba3bd3d7280cd37dc504660b23fe0487

## TLDR

While an unanswered automatic-update SSH sign-in question is waiting in the attention row (up to two minutes per
prompt), trying to turn **Tailscale host** on or off in Host settings fails with "This host is busy; try again after its
current operation finishes" even though no setup or update is running and you never started an operation on that host.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/setup_sign_in.rs:318`.

- In `crates/farhelm-helm/src/provisioning/setup_sign_in.rs:318-334`, `SetupSignIn::start`'s spawned task acquires
  `service.manager.ssh_route_lock(*host).read_owned().await` and holds that read guard for the full duration of
  `setup.wait(&route.destination, route.access, &attempt).await` — up to `QUESTION_LIFETIME` (120 seconds,
  `crates/farhelm-helm/src/ssh_sign_in.rs:23`) per prompt.
- Before commit `8026bea2`, a registered host's `SetupSignIn` check only ran in response to an explicit user Setup,
  Update, or Uninstall action. Commit `8026bea2` causes `AutomaticUpdates::scan` (`automatic.rs:250-262`) — enabled by
  default — to start background `SetupSignIn` checks for every outdated SSH host whose setup master is not yet open.
- In `crates/farhelm-helm/src/hosts.rs:993-997`, `set_tailscale` (`POST /api/hosts/{id}/tailscale`) calls
  `state.manager.ssh_route_lock(host).try_write_owned().map_err(|_| crate::environment::lifecycle::HostBusy)?`, which
  immediately returns 409 Conflict (`"This host is busy; try again after its current operation finishes"`,
  `crates/farhelm-helm/src/environment/lifecycle.rs:22`) whenever any read guard is held.
- Unlike `delete_registered_host` (`service.rs:2644-2646`, which calls `setup.cancel_host(host)`) and `put_preferences`
  (`preferences.rs:285-287,320-322`, which calls `cancel_automatic_sign_ins()`), `set_tailscale` does not cancel
  auto-only sign-in checks for `host` before taking `ssh_route_lock`, so an unrequested background automatic-update
  prompt blocks SSH mode changes for up to two minutes.
- How to verify: start an outdated SSH host with automatic updates enabled so `AutomaticUpdates` opens a Setup sign-in
  question (`active` holds `manual: false`), then invoke `POST /api/hosts/{id}/tailscale` toggling `tailscale`; observe
  409 Conflict with `HostBusy`.
- Fix direction: before `set_tailscale` tries to acquire `ssh_route_lock(host)` exclusively (after taking
  `_provisioning` and validating the edit), cancel auto-only `SetupSignIn` checks for `host` (similar to
  `cancel_automatic_checks`, scoped to `host` with `attempt.release()` so the retry opportunity is preserved and any
  shared manual check still refuses), and wait briefly for the cancelled check's read guard to drop (or retry
  `try_write_owned` after cancellation settles).

Reviewer metadata (g39_host_auto_update_2fa_impl/l2): suggested bucket: other (narrow UX overlap: only hits users
toggling Tailscale host on an outdated 2FA SSH host during the 2-minute unanswered automatic-update prompt window, with
no data loss or security consequence, and recovers once the prompt expires or is cancelled).; confidence: confirmed;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g39_host_auto_update_2fa_impl/l2`.
