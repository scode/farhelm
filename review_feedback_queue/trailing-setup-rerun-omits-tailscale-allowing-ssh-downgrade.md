# Failed-setup Re-run omits Tailscale mode and can downgrade to ordinary SSH

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

Rerunning setup on a Tailscale SSH host does not include its Tailscale setting in the probe request. If the host is
concurrently removed, retargeted, or switched out of Tailscale mode before the helm reads the registry, the helm
silently falls back to ordinary non-Tailscale SSH (bypassing Tailscale peer resolution and host-key pinning) and can
automatically register or install on the host without Tailscale.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/service.rs:959-979`.

In `crates/farhelm-helm/src/provisioning/http.rs:71-76`, `ProbeDestination::Ssh.tailscale` is `Option<bool>`
(`#[serde(default)]`). In `crates/farhelm-helm/src/provisioning/service.rs:959-979`, `ProvisioningService::probe_owned`
uses `requested_mode = tailscale`: (1) the mode-mismatch refusal at `service.rs:963-971`
(`if let (Some(row), Some(requested_mode)) = (&stored, requested_mode) && row.tailscale != requested_mode`) ONLY runs
when `requested_mode` is `Some(...)`; and (2) when `stored` is `None`, line 973-979 falls back to `UserConfig` whenever
`requested_mode != Some(true)`. Meanwhile, the GUI's setup rerun builder
(`crates/gui/farhelm-gui/src/hosts/setup.rs:707-713`, `rerun_command`) sends
`{"target": {"kind": "ssh", "destination": host.destination}, ...}` with NO `tailscale` field (`requested_mode = None`)
and NO `new_entry`, even for a Tailscale SSH host (`host.tailscale == true`). Two concrete failure routes in shipped
code: (a) If the Tailscale host row is removed or retargeted before `probe_owned` reads `self.store.list_hosts()`
(`service.rs:891-896`)—including during the window between a 202 setup-sign-in pending reply and the GUI's reissued
probe—`stored` is `None` and `requested_mode` is `None`, so `probe_owned` silently downgrades `access` to
`SshAccessKind::UserConfig`, dials the destination via ordinary SSH without Tailscale host-key verification, and either
re-registers the discovered supervisor with `tailscale: false` (`service.rs:1357-1364`) or produces a `UserConfig` ADD
plan that `setup::rerun` (`hosts/setup.rs:796,815-817`) immediately auto-submits if "Set up new hosts without asking" is
enabled. (b) If the host's Tailscale mode is toggled off concurrently before `probe_owned` reads `list_hosts()`,
`requested_mode = None` bypasses the refusal at `service.rs:963-971`, so `probe_owned` silently probes and (if
auto-setup is enabled) provisions under `UserConfig` despite the user acting on a rendered Tailscale host (also
violating `SPEC_impl.md`, "GUI: Dioxus", "Answers bind to what was drawn"). Fix: have `setup::rerun_command` always pass
`"tailscale": host.tailscale` explicitly (so `service.rs:963-971` refuses mode changes and never silently downgrades to
`UserConfig` if the row disappeared), and ideally bind registered-host setup reruns on the helm side to an expected
`host_id` / `entry_uuid` so a deleted/retargeted row cannot be recreated as a new entry.

Reviewer metadata (g45-tailscale-provisioning-podman/f7): suggested bucket: highest (security boundary crossing: silent
downgrade from Tailscale-verified host keys/IP resolution to ordinary unpinned SSH using `~/.ssh/known_hosts` and user
SSH config, with automatic provisioning when "Set up new hosts without asking" is enabled); confidence: confirmed;
possible cover: none (corroborates and analyzes helm side of cross-group finding `g46/l1` from
`g46-tailscale-gui-and-repairs`).

Additional source record at `f247754a50574cbd4517c9871625dc171e8df000`, `crates/gui/farhelm-gui/src/hosts/setup.rs:707`
(g46-tailscale-gui-and-repairs/l1):

In `crates/gui/farhelm-gui/src/hosts/setup.rs:707-717`, `rerun_command` builds `POST /api/hosts/probe` for
`SetupRerun::Ssh` with `"target": { "kind": "ssh", "destination": host.destination }` and omits `"tailscale"` (even
though `host: &HostView` carries `host.tailscale`). Meanwhile, `crates/gui/farhelm-gui/src/hosts/menu.rs:433-446` also
reads live `current(host, cx)` at click time instead of freezing the drawn `HostView` (including `tailscale` and
`destination`) on the menu item. On the helm side (`crates/farhelm-helm/src/provisioning/http.rs:71-76`,
`crates/farhelm-helm/src/provisioning/service.rs:881-980`): `probe_owned` acquires `self.plan_slots` before reading
`self.store.list_hosts()`, whereas `hosts::remove_host` (`crates/farhelm-helm/src/hosts.rs:1314-1316`) and
`hosts::set_tailscale` (`crates/farhelm-helm/src/hosts.rs:981-988`) do not wait on `plan_slots`. When `target.tailscale`
is omitted (`None`), two concrete security/correctness failures occur: (1) if the registered Tailscale host is removed
before `probe_owned` reads `list_hosts()`, `stored` is `None` and `requested_mode` is `None`, so `service.rs:973-979`
silently selects `SshAccessKind::UserConfig` (ordinary OpenSSH / `~/.ssh/known_hosts`, bypassing Tailscale
advertised-key verification) and sets `registration.tailscale = false`; if a supervisor answers it is re-registered
without Tailscale, or if absent and "Set up new hosts without asking" is enabled, `setup::rerun` (`setup.rs:815-817`)
submits provisioning automatically over ordinary SSH; (2) if the host's `tailscale` mode is flipped concurrently,
`requested_mode` being `None` bypasses the helm's explicit mode-mismatch refusal at `service.rs:963-971` ("Change the
Tailscale mode for this host in Host settings, then probe it again."). Fix direction: send `"tailscale": host.tailscale`
in `setup::rerun_command` (and freeze the menu's drawn `HostView` fields for Re-run in `hosts/menu.rs`), and consider
having the helm refuse a non-`new_entry` rerun probe or require a host id when rerunning a registered host.

Reviewer metadata (g46-tailscale-gui-and-repairs/l1): suggested bucket: highest (could be in highest: security-impacting
SSH trust-policy downgrade from Tailscale advertised-key verification to ordinary UserConfig SSH / known_hosts);
confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g45-tailscale-provisioning-podman/f7`,
`g46-tailscale-gui-and-repairs/l1`.
