# Answering a shared SSH sign-in triggers automatic Update concurrently with manual Uninstall or Update

Reviewed commit: 8026bea2ba3bd3d7280cd37dc504660b23fe0487

## TLDR

When automatic host updates are on (the default) and a remote SSH host needs SSH sign-in, choosing Uninstall or Update
on that host shares the waiting Setup sign-in question. As soon as you answer the prompt, Farhelm immediately starts an
automatic Update in the background while your manual action replays. If you chose Uninstall, Farhelm updates and
restarts the host before showing the Uninstall confirmation dialog and your Uninstall can fail as busy or interrupted;
if you chose Update, your manual Update can fail with a "host already has a provisioning operation in flight" error and
show a warning badge on the host even while the automatic Update runs.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/automatic.rs:322-347`.

- In `crates/farhelm-helm/src/provisioning/setup_sign_in.rs:199-208,270-287`, manual planning (`plan_uninstall`,
  `plan_update_for_ui`, `probe`) and automatic planning (`automatic::scan` -> `plan_update_guarded(host.id, true)`)
  share one `ActiveAttempt` and its `outcome: watch::Sender<Option<SetupSettlement>>` whenever they target the same
  registered `SetupRoute`. Joining via `start_manual` sets `joined.manual |= true` (`setup_sign_in.rs:273,285`), and
  joining via `start_automatic` subscribes to the same `outcome` watch (`setup_sign_in.rs:244-255,271-277,282-287`).
- Meanwhile, `crates/farhelm-helm/src/provisioning/automatic.rs:322-342` (`begin_sign_in`) spawns a `resume` task that
  awaits `sign_in.wait_ready()`. When the user answers the shared SSH question and `SetupSignIn::start`'s task publishes
  `SetupSettlement::Finished(SshSignInOutcome::Ready)` (`setup_sign_in.rs:362-371`), `resume` wakes immediately
  in-process — ahead of the WebSocket UI-channel push and the desktop's HTTP replay
  (`crates/gui/farhelm-gui/src/hosts/setup_sign_in.rs:182-210,240-244`).
- Because manual planning holds no `busy` claim in `ProvisioningMemory`, `still_eligible` (`automatic.rs:286-300`)
  returns `Ok(true)`, `service.plan_update(host.id)` (`automatic.rs:332`) succeeds against the freshly authenticated
  setup master, and `admit` -> `start_automatic_update` (`service.rs:1962-2017`) claims `memory.busy.insert(host)`
  (`service.rs:2005`) and launches an automatic Update run (`service.rs:2592`).
- Consequences:
  1. Manual Uninstall (`POST /api/hosts/{id}/uninstall`): violates SPEC.md ("Topology", Uninstall: "The helm plans the
     removal over SSH and shows it before touching the host... Nothing changes until the user confirms") by immediately
     mutating the host with an automatic Update as soon as the user answers Uninstall's sign-in prompt, while
     Uninstall's replayed `plan_uninstall_owned` (`service.rs:2038-2082`, which does not check `memory.busy`) probes
     concurrently with the running Update and any confirmation hits `409 Conflict: Busy(host)` (`service.rs:2464,2513`).
  2. Manual Update (`POST /api/hosts/{id}/update`): the desktop's replayed `plan_update_for_ui` (`service.rs:1645-1745`,
     which does not check `memory.busy`) runs a redundant SSH probe/inspect and returns a `HostPlanResponse`, after
     which `crates/gui/farhelm-gui/src/hosts/update.rs:217,242-276` immediately posts confirmation; `start_update_owned`
     (`service.rs:1939-1940`) returns `409 Conflict: Busy(host)`, and `hosts/update.rs:270-271` records the refusal
     under the row and calls `super::status::failed(host, Action::Update, cx)`, raising a spurious Update failure and
     host-rail warning badge.
- Turn 2 severity check (why this stays `high` and does not cross into `highest`): `uninstall_plan`
  (`service.rs:2143-2145`) verifies `require_no_running_sessions(client)` on `Connected` hosts before
  `sign_in_for_uninstall`, and refuses `VersionSkew` hosts via `unusable_host` (`service.rs:2232`) before
  `sign_in_for_uninstall`. Automatic updates are already authorized by policy (`auto_update_remote_hosts` enabled), the
  user signed into the exact host's Setup connection, Update preserves sessions/data (`KillMode=process` /
  `AbandonProcessGroup=true`), and `start_uninstall` refuses while `busy` is set and revalidates the full uninstall plan
  (`fresh == pending.plan`) before deleting anything. Thus there is no established route to losing user data,
  credentials, processes, or violating a security boundary.
- How to verify: in `crates/farhelm-helm/src/provisioning/tests/automatic_sign_in.rs`, keep `auto_update_remote_hosts`
  enabled (unlike `opt_out_cancels_only_automatic_checks_and_leaves_the_connection_eligible` which turns it off before
  answering), join the open automatic sign-in with `POST /api/hosts/{id}/uninstall` or `POST /api/hosts/{id}/update`,
  answer the SSH question with `fixture-code`, and observe `automatic.rs`'s `resume` task immediately starting an
  automatic Update run while the manual HTTP replay races into it.
- Fix direction: when a manual caller owns or joins a setup sign-in (`ActiveAttempt::manual == true`), do not fire the
  automatic `resume` continuation on that shared `Ready` settlement (e.g., notify `AutomaticSignIn` that manual work
  superseded the check so `resume` exits without starting an automatic Update, and defer automatic scanning for that
  host while the manual replay/confirmation is active or let the manual operation take precedence).

Reviewer metadata (g39_host_auto_update_2fa_impl/l1): suggested bucket: high (reliably occurs under default settings
whenever a user clicks Uninstall or Update on an outdated 2FA SSH host waiting on automatic sign-in; causes an
unrequested Update mutation before Uninstall confirmation and breaks manual Update with a spurious Busy error and
warning badge. Confirmed not `highest` per the Turn 2 check above).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g39_host_auto_update_2fa_impl/l1`.
