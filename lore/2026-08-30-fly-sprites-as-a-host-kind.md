# Fly.io Sprites as a host kind, and the desktop app against a remote helm

NOTE: this is an assessment, not a decision. Nothing here is built. It records what was learned on 2026-08-30 from
poking at a real sprite and mapping it onto the code as it stood then, so the idea can be picked up later without
redoing the experiments. Line numbers are as of that day.

The idea: a Farhelm session backed by a Fly.io Sprite (`sprites.dev`) instead of a machine you own. Sprites are
per-second-billed microVMs with a persistent filesystem that freeze themselves when idle, so "pause this host" in the UI
would map directly onto "stop paying for it". The conclusion is that it fits well, that the transport is nearly free,
and that the real work is provisioning and a new host lifecycle state. The secondary question, whether the native app
could attach to a remote helm so a laptop can sleep without killing agents, turns out to be mostly already answered by
the existing reconnect design.

## What a sprite is, measured

Probed with the `sprite` CLI (`0.0.1-rc48`) against a test sprite. Ubuntu 26.04, 8 vCPU, about 8 GB, 100 GB overlay,
unprivileged user `sprite` (uid 1001), PID 1 is `tini`. No systemd, no `loginctl`, no `/run/user`, no sshd. cgroup v2
controllers exist but nothing delegates them. tmux 3.6 is preinstalled, which is below the supervisor's 3.7c floor.

The CLI emulates an ssh server over stdio: `ssh -o ProxyCommand="sprite proxy --ssh -s NAME" NAME` works for exec,
stdin piping, and ControlMaster reuse. `sftp` does not ("subsystem request failed"), so payload push has to go through
`cat > file` over ssh or `sprite file`.

Lifecycle: `running` (billed) becomes `warm` (VM memory frozen, free, 100-500 ms wake) and eventually `cold` (processes
gone, 1-2 s wake, timing undocumented). Idle detection is connection-based and aggressive. A tmux loop writing
timestamps every second while holding an open outbound TLS connection froze about one to two seconds after my ssh
session closed; the log resumed on the next connect. Neither running processes nor outbound connections count as
activity, only inbound exec/console/TTY sessions and proxied connections. The tmux server survived the warm freeze
intact. `sprite-env services create` defines runtime-owned processes restarted on crash and on cold boot, the analog of
a user systemd unit plus linger. Checkpoints are cheap overlay snapshots; restore terminates sessions. Org limits were 10
running and 10 warm. Pricing per the docs at the time: CPU $0.07/CPU-h, RAM $0.04375/GB-h, billed per second only while
running; warm and cold pay cold storage only.

## How it maps onto the code

Host kind: a `HostKind::Sprite` beside `Local` and `Ssh` (`crates/farhelm-helm/src/store.rs:158-194`, schema CHECK at
`:1529`), destination being the sprite name plus org. The UI already tolerates unknown kinds via `Unrecognized`
(`farhelm-ui/src/lib.rs:700-720`).

Transport: `SystemTransport::connect` (`transport.rs:96-159`) spawns the same `ssh ... farhelm internal stdio` argv as
for ssh hosts, plus `-o ProxyCommand=sprite proxy --ssh -s <name>`. Nothing above `HostTransport` changes; the
multiplexed protocol rides the byte pipe unchanged. Auth is the sprite CLI's own keyring, the same "user's own access"
posture as ssh keys, so Farhelm stores nothing new. A native websocket client against `api.sprites.dev` could drop the
CLI dependency later.

Provisioning is the real work. `ProvisioningBackend` (`backend.rs:161-231`) needs a second flavor: `inspect` must not
decline on the missing `systemctl --user`; `install_bytes` streams over ssh stdin instead of `sftp_put`
(`backend.rs:659`); the `WriteUnit`/`DaemonReload`/`EnableSupervisor`/`EnableLinger`/`RestartSupervisor` actions
(`plan.rs:74-98`) collapse to one `sprite-env services create farhelm-supervisor ...` with `FARHELM_TMUX` pointing at the
pushed static tmux. The existing musl-static payloads are exactly right. The supervisor itself needs no change:
`scope.rs` already falls back to the process-tree sweep without a user manager, and boot-id tracking classifies a cold
wake as a reboot, so sessions get marked interrupted with resume offered, which is the right outcome.

Lifecycle is the design decision. Today's connection actor (`manager.rs`: 3 s refresh, 45 s re-probe forever) would
hold a sprite `running` around the clock and wake a parked one every 45 s. So `HostState` (`manager.rs:433`) needs a
`Paused` variant in which the actor holds no connection and does not re-probe, kept unroutable by `is_connected`
(`:637`). Pause is then nothing more than dropping the ssh connection; the sprite goes warm on its own about a second
later. Resume is reconnecting; the wake is transparent inside the ProxyCommand. The flip side is that the helm's
connection is the only thing keeping the sprite awake, so an agent mid-turn freezes the moment the helm disconnects, and
its API streams drop. Pause has to be an explicit verb, or a policy with a real activity signal (last terminal output),
never "no browser attached". A paused host renders from the stale session cache the way a down host does.

Creation from the UI is `sprite create --skip-console` from the helm's machine, then the ordinary provision flow.
Destroy needs a much scarier confirm than the current "forget only" remove.

Spec conflicts to surface rather than override: SPEC.md lists cloud-hosted execution as a v1 non-goal (`:589`) and names
ssh as the one transport integration (`:565`); the provisioning section requires a usable systemd user manager (`:90`).
Keeping ssh as the byte transport makes the deviation "a second host kind and a second service manager", but the text
still has to change.

Risks: the CLI is a release candidate and its ssh emulation is a stand-in for sshd, so pin its behavior in a test the way
tmux is pinned; warm-to-cold timing is undocumented and outside our control, which is the one durability caveat the UI
would have to state plainly.

Rough build order: the host kind and ProxyCommand transport first (end-to-end sessions on a hand-provisioned sprite),
then the provisioning flavor, then the `Paused` state with explicit pause/resume and a status chip from the REST API,
then create/destroy and any auto-pause policy.

## The desktop app against a remote helm

The tension: with a sprite, the helm must be the always-on party, so the helm has to live on a 24/7 box, and SPEC.md
(`:59-60`, `:69-71`) says the native app has no helmless mode and there is no remote native-app-to-helm mode. A browser
tab against a Linux helm already gives resumability across laptop sleep: `reconnect.rs` is built for exactly that case,
and the supervisor's tmux replays scrollback on reattach. So this is a packaging question, not an architecture one.

What would have to change, and the rough size: gate the embedded helm thread, its exit-on-death monitor, the tethered
local supervisor, and `await_local_supervisor` (`desktop.rs:455-624`, `:1208-1300`, the last of which would otherwise
hang forever) behind a "remote helm" config, about a day. Auth is the real coupling: the webview reads its bootstrap
token straight out of the local `helm.db` via `show_token` (`auth.rs:70`), and `store_device_secret` on non-wasm is a
stub (`auth.rs:341-344`); backing it with `desktop-client.json` and using the browser's `TokenPrompt` is one to two
days, less if TODO.md's "move desktop preferences into the helm" item lands first. The `Host` header check in
`middleware.rs:92-118` requires a same-port forward, and whether the app owns the `ssh -N -L` tunnel is a spec decision
(SPEC.md `:547` says no built-in tunneling); owning it is small and makes sleep/wake clean. Auto-reconnect and heartbeat
are gated on build-stamp match (`reconnect.rs:209`), so a remote helm one release apart silently loses the feature this
mode exists for; remote mode needs a loud skew banner. In remote mode the Mac is not a host unless the Linux helm
registers it over ssh with a hand-started supervisor, which SPEC already supports. Three to five days all in, auth being
the only non-mechanical part.

The cheaper alternative, which came up in the same discussion: an installed web app (Chrome "Install app", Safari "Add
to Dock") against the same-port localhost tunnel gives the icon, the alt-tab entry, and the standalone window for the
price of a `manifest.json`, with the reconnect behavior already in place. That prompted `docs/browser-limitations.md`,
which explains why the tunnel has to be loopback: secure contexts gate the programmatic clipboard (copy-on-select, OSC
52) and app install, and loopback over plain http is the only way to get them without TLS. If the day-to-day setup
becomes "24/7 Linux helm, laptop is a client", the thing worth keeping from the native app is arguably just an optional
tunnel-owning launcher, not a WebKit-embedding binary.
