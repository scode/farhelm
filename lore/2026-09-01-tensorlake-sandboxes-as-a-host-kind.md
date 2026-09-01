# Tensorlake sandboxes as a host kind

NOTE: this is an assessment, not a decision. Nothing here is built. It records what was learned on 2026-09-01 from
probing a real Tensorlake sandbox (`tl` CLI, image `tensorlake/ubuntu-minimal`, docs.tensorlake.ai) and mapping it onto
the code at `5d72703`, so the idea can be picked up later without redoing the experiments. It is the follow-up to
`lore/2026-08-30-fly-sprites-as-a-host-kind.md` and reuses its framing: read that entry first. Line numbers are as of
this day.

The idea is the same as the sprite one: a Farhelm session backed by a per-second-billed microVM that suspends when
idle, so "pause this host" in the UI maps onto "stop paying for it". The conclusion: Tensorlake fits BETTER than
sprites on the two axes that were the real work there — the transport is farhelm's existing ssh path with zero code
changes, and provisioning's hardest sprite gap (no sftp) does not exist — at the cost of one wrinkle sprites did not
have (resume needs an explicit API call; plain ssh refuses a suspended sandbox) and one platform bug that currently
defeats the pause economics (the sshd session leak below).

## What a sandbox is, measured

Ubuntu 24.04, user `tl-user` (uid 1000, passwordless sudo), PID 1 is `tensorlake-init`. No systemd, no `loginctl`, no
`/run/user`. cgroup v2 controllers (`cpuset cpu io memory hugetlb pids`) are present AND enabled in
`cgroup.subtree_control` — delegated, unlike the sprite's. No tmux in the image. CPU/RAM/disk shape and the idle
timeout are fixed at creation and immutable afterwards. Usage Credits plan caps: 4 vCPU / 16 GB / 100 GB / 100
concurrent, plus a "24 hours session duration" line in the billing docs I could not disambiguate (continuous-running
cap? per-connection?) — ask Tensorlake before committing to anything. Pricing per the docs this day: CPU $0.07/core-h,
RAM $0.015/GB-h, disk $0.0002/GB-h, per second while running; a suspended sandbox pays $0.07/GB-month snapshot
storage. Suspend snapshots SURVIVE termination and keep billing until deleted with `tl sbx checkpoint rm`.

The decisive measurements:

- **A real SSH gateway.** `ssh <sandbox-id-or-name>@sandbox.tensorlake.ai`, authenticated by a public key registered
  once with the account (`tl sbx ssh keys add`), is standard OpenSSH against an in-sandbox sshd fronted by the
  platform proxy. Verified: exec channel with byte-perfect binary stdio in both directions (1 MB of `/dev/urandom`
  round-tripped by checksum), working sftp (upload verified by checksum), one stable host key for the gateway.
  `tl sbx describe` prints a ready ssh_config stanza. The docs also claim scp, rsync, and all four forwarding modes.
  This is the piece the sprite CLI only emulated (RC-quality, no sftp); here it is the real thing.
- **Suspend is a live-memory freeze.** Explicit suspend then resume took 1.7 s and kept the same boot id
  (`9bec28ef-…` before and after). A nohup'd 1 Hz shell loop and a platform-managed process both continued exactly
  where they froze: the loop's timestamp log ran to 1788232067 (the suspend), resumed at 1788232087 (right after the
  resume), the gap matching the suspended wall-clock window exactly. There is no sprite-style "cold" tier that kills
  processes; only terminate does. For farhelm this means a resumed host continues its supervisor, tmux server, and
  agents mid-flight under the same boot id — sessions are not "interrupted", they just missed wall-clock time (agents'
  outbound API connections will have timed out during a long suspend, the same caveat the sprite entry noted for
  warm).
- **Resume is NOT transparent over ssh.** The gateway refuses a suspended sandbox outright — "sandbox … is currently
  suspended — resume it (`tl sbx resume …`) or create a new one" — where the sprite woke transparently inside the
  ProxyCommand. Only `tl sbx ssh` (WebSocket PTY) and `tl sbx exec` auto-resume. So a farhelm resume needs one `tl`
  CLI or REST call before reconnecting.
- **Managed processes.** `tl sbx exec --detach --name X --restart always --health-http …`, with `tl sbx ps`, `logs`,
  `restart`, `kill` — restart policies with backoff, the analog of `sprite-env services create`, i.e. the service
  manager a supervisor needs on a host with no systemd.
- **Checkpoints clone.** `tl sbx checkpoint NAME` then `tl sbx create -s SNAP_ID` brings up a new sandbox with the
  bootstrapped filesystem in seconds. Sprites had checkpoints too, but restore terminated sessions; here cloning is a
  usable "provision once, stamp out hosts" primitive.

## The sshd session leak, with the evidence

Idle-suspend is documented as inbound-proxy-traffic-based: a sandbox stays active while handling "an open SSH session,
a connected WebSocket PTY, a request to an exposed user port, or any SDK/CLI call". The mechanism works — but native
ssh currently defeats it, because a closed client session's proxied connection is never closed on the sandbox side, so
its sshd processes survive forever and the platform keeps counting the connection as an open SSH session. The first
observations of this were confounded (every "still running" wait was also under 600 s — the documented default
timeout — from the last CLI call, so a silent clamp to the default would have explained them too); what follows is the
controlled version that settles it, all timestamps Unix epoch.

- **The process leak, deterministic.** Every clean native-ssh session — BatchMode, one command, stdin from
  `/dev/null`, output received, client exit 0 — leaves exactly two sshd processes behind (`sshd: tl-user [priv]` and
  `sshd: tl-user@notty`), never reaped. Observed as monotone `ps` counts across successive sessions on two sandboxes:
  23 → 25 → 27 on the first probe, 3 → 5 → 7 on a fresh one.
- **The controlled twins.** Two sandboxes created the same second with `--timeout 120` (both `describe` outputs echo
  `Timeout: 120s`). Twin A: never touched after creation. Twin B: three clean single-command ssh sessions ending 2 s
  after creation (T0=1788234694), then nothing. One `tl sbx ls` at T0+362: A `suspended`, B `running`. A second at
  T0+1062: A still `suspended`, B still `running` — 1060 s past B's last activity, roughly 9x its configured timeout
  and well past the 600 s default. So the configured timeout is honored on an untouched sandbox (no clamp), and ssh
  residue alone is what pins its twin.
- **The kernel's view, from inside.** Inspecting B ~18 minutes after its sessions closed: each leaked pair (process
  ages 1076-1077 s, matching the T0+2 sessions exactly) still holds an ESTABLISHED TCP connection from the platform
  proxy (peer `10.12.3.40`) to in-sandbox port 22, Recv-Q/Send-Q both 0, TCP keepalive timer ~102 minutes remaining —
  the stock 2-hour keepalive, which a suspend would freeze anyway. The sshd listener is spawned by `indexify-daemon`
  (the platform agent, PID 536), which also holds the proxy legs.

Put together: the client's disconnect reaches the gateway, the gateway never closes its sandbox-side leg, sshd waits
indefinitely on a dead-but-ESTABLISHED socket, and the accounting — consistent with its documented "open SSH session"
rule — counts those connections forever. The process leak and the never-suspends behavior are one bug seen from two
sides. A standalone three-sandbox repro script (control, observer-only, observer+ssh, with an in-sandbox sampler
logging sshd processes and `ss -tnop` socket state every 5 s through the disconnected window) reproduced all of it in
one run for the upstream report — and settled a leftover question in the process: the observer-only leg suspended
~119 s after its last exec with the sampler still ticking, so in-sandbox process activity does NOT count as activity;
only the proxied traffic does, exactly as documented. Until Tensorlake fixes the leak, a paused host would need its
sshds reaped as part of the pause verb, which is ugly but works.

## How it maps onto the code

- Host kind: a `HostKind::Tensorlake` beside `Local` and `Ssh` (`crates/farhelm-helm/src/store.rs:129`, schema CHECK
  `kind IN ('local','ssh')` at `:1071`), destination being the sandbox name.
- Transport: nothing changes at all. `SystemTransport::connect` (`transport.rs:113`) already spawns the user's own
  `ssh` with `ssh_stdio_args` (`ssh.rs:121`); the destination is `<name>@sandbox.tensorlake.ai` or an ssh_config
  alias, auth is the registered key. No ProxyCommand, no CLI in the byte path. A hand-provisioned sandbox could
  plausibly run as a plain `ssh` host row today. One nit: `ssh_base_args` sets `ControlPersist=60`, which holds the
  gateway session — activity — for 60 s after last use; the pause path either waits that out or tears the master
  down.
- Provisioning: the same "second flavor" the sprite entry called the real work, but smaller. `inspect`
  (`provisioning/backend.rs:164`) must not decline on the missing `systemctl --user`; `sftp_put` (`backend.rs:659`)
  works as-is (verified above), where sprites forced an ssh-stdin rewrite; the
  `WriteUnit`/`DaemonReload`/`EnableSupervisor`/`EnableLinger`/`RestartSupervisor` actions (`provisioning/plan.rs:84-96`)
  collapse to one `tl sbx exec --detach --name farhelm-supervisor --restart always …` with `FARHELM_TMUX` pointing at
  the pushed static tmux. The musl-static payloads are exactly right. `scope.rs`'s no-user-manager fallback applies
  unchanged — with a note that the delegated cgroup controllers might later allow real scopes without systemd, which
  the sprite could not offer.
- Lifecycle: the same `Paused` variant in `HostState` (`manager.rs:389`) — today's connection actor (3 s refresh,
  45 s re-probe) would hold the sandbox running around the clock. Pause = drop the connection and let the idle
  timeout fire, or `tl sbx suspend` for immediacy (plus, today, the sshd reap above). Resume = `tl sbx resume` (CLI
  or REST) BEFORE reconnecting, the one place the helm needs Tensorlake tooling; sprites needed none for wake. Auth
  posture is the same "user's own access" as ssh keys: the `tl` PAT and the registered pubkey are the user's, Farhelm
  stores nothing new.
- Creation from the UI: `tl sbx create` with shape and timeout fixed forever at that moment, so pause policy is
  partly a creation-time decision; checkpoint-clone makes "another host like that one" cheap. Destroy needs the scary
  confirm plus a snapshot sweep, because suspend snapshots outlive termination and bill monthly.

## Spec conflicts to surface rather than override

The same three the sprite entry named: SPEC.md lists cloud-hosted execution as a v1 non-goal (`:680`), frames ssh as
the one transport integration, and requires a usable systemd user manager for provisioning (`:92-94`). Since the byte
transport here really is plain ssh, the deviation reduces to "a second host kind and a second service manager" — a
smaller textual change than the sprite's, but the text still has to change.

## Risks

The sshd leak (above) is the current blocker for the pause economics. The 24 h session-duration cap is unquantified
for named sandboxes. The timeout being immutable makes pause policy partly irreversible per host. And the platform is
young: the docs lag the CLI (tunnel, port, fs are undocumented), and behavior already drifted once between an August
probe and this one — pin what matters in tests the way tmux is pinned.

## Rough build order

The sprite ladder minus a rung: (1) a hand-provisioned sandbox as a plain ssh host row, which likely works now with
zero code; (2) `HostKind::Tensorlake` and the provisioning flavor (sftp path as-is, managed-process actions); (3) the
`Paused` state with explicit suspend/resume through `tl` or REST, including the sshd reap while the leak stands;
(4) create, clone, and destroy with snapshot hygiene.
