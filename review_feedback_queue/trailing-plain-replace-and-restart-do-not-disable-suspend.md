# Plain Replace and Restart do not disable host Suspend or Resume on their target host

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

While a plain Restart or Replace of a session is running on a sandbox or container host, Suspend stays enabled in Hosts
(and on any other session on that host), so suspending the host can freeze its supervisor mid-request, even though
Restart with and Replace with block Suspend on that host.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/launch.rs:243`.

`host_lifecycle_refusal` (`crates/gui/farhelm-gui/src/launcher/launch.rs:236-255`) calls `targets_host`, which inspects
only `LaunchSlot` (`held.wake` or
`held.waiting == Waiting::Reply && draft.host == Some(host) && draft.target.is_none()`). Launcher-submitted
`Replace with…` and `Restart with…` hold `LaunchSlot` while `POST /api/sessions/{id}/replace` or `/restart` is in
flight, so they disable Suspend/Resume on the target host. Plain `Replace`
(`crates/gui/farhelm-gui/src/session_list/replace.rs`) and plain `Restart`
(`crates/gui/farhelm-gui/src/session_list/restart.rs`) send the exact same routes as `Work::Replace` / `Work::Restart`
on `OpKey::session(key)` without touching `LaunchSlot`. By contrast, `Work::creates_a_session`
(`crates/gui/farhelm-gui/src/operations.rs:283-297`) and `hosts::launch_in_flight`
(`crates/gui/farhelm-gui/src/hosts.rs:1167-1172`) treat plain `Replace` and `Restart` as session launches for the
host-registry exclusion. On the helm, `environment::lifecycle::perform`
(`crates/farhelm-helm/src/environment/lifecycle.rs:28-35,79-114`) only takes the host provisioning lock
(`try_host_provision_lock`), which session replace/restart endpoints do not hold, so `POST /api/hosts/{id}/suspend` from
Hosts (`crates/gui/farhelm-gui/src/hosts/menu.rs:344`, `hosts/lifecycle.rs:158-163`) or another session on the same host
withdraws the connection and suspends the sandbox/container mid-request. Fix direction: either have
`host_lifecycle_refusal(host, cx)` also check whether any session belonging to `host` has `Work::Replace` or
`Work::Restart` in flight in `Operations`, or confirm in SPEC.md that only launcher-submitted launches hold host
Suspend/Resume.

Reviewer metadata (g09-wake-host-suspend-guard/l1): suggested bucket: other (could NOT be in highest: no security,
credential, or unconfirmed user-work loss; freezing mid-Replace/Restart causes the request to fail or leave both
sessions visible until resumed, which SPEC.md Sandbox sessions L1630-1631 accepts for failed/ambiguous Replace);
confidence: confirmed; possible cover: none (SPEC.md Creation L1473-1479 discusses launcher submissions and the old
Dioxus app's `ops.launch_targets` was also set only by `create_form.rs`, though `SPEC_impl.md` and `operations.rs` treat
plain Replace and Restart as launches in `Work::creates_a_session`).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g09-wake-host-suspend-guard/l1`.
