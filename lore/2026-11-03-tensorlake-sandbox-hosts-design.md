# Tensorlake sandbox hosts and per-session sandboxes: the design

NOTE: This is a design record written on 2026-11-03, not documentation of shipped behavior. It captures the maintainer's
idea for running sessions on Tensorlake sandboxes, the decisions made while planning it, and what probing the platform
that day showed. Phase 1 (Tensorlake sandboxes as hosts) was planned in detail and handed to an unattended build; phase 2
(a sandbox per session) was deliberately left at sketch level. The earlier assessments,
[`2026-09-01-tensorlake-sandboxes-as-a-host-kind.md`](2026-09-01-tensorlake-sandboxes-as-a-host-kind.md) and
[`2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`](2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md),
are background; several of their platform facts no longer hold (see below).

## The idea, in the maintainer's words

> - Tensorlake sandboxes provide a very cost effective way of running agents. They charge for used CPU time (idle CPU
>   is free). You can dynamically scale CPU count and memory count on a running sandbox.
> - Named (as opposed to ephemeral sandboxes) persist, including in-memory state. This means we can offer a cheap
>   "suspend" feature for such sessions that will work fine and doesn't even involve restarting the agent (so e.g. not
>   disruptive to agents that have background work active).
> - Tensorlake also supports cloud volumes that can be mounted, so we can support things like shared scratch spaces (not
>   initially in scope).
> - Sandboxes has cheap snapshots. So, for a given github repo, we can initialize a tensor lake snapshot that can be
>   cloned super quickly, and then run a git fetch to refresh it incrementally as a session is spawned.
>
> there is a difficulty in that there is no obvious equivalent of a remote host [...] however I think a workable model
> is:
>
> - you can indeed have a "host" that's a specific tensorlake sandbox. this fits very very well, the only difference is
>   initial setup isn't "give me hostname", it's more like "how do you want the sandbox configured".
> - in addition, we support a "on-the-fly-host-creation" mode for tensorlake sandboxes so you can get the experience of
>   every individual session being a dedicated sandbox, super quickly spawned from a snapshot.

The proposed details, also the maintainer's:

- Adding a host offers "ssh" or "tensorlake sandbox".
- A Tensorlake host is described by a sandbox recipe: the sandbox size, Ubuntu packages to install, and outbound TCP
  tunnels (for inference load balancers reachable from the helm's side, for example). Uploading Codex or Claude
  credentials is out of scope for now.
- A recipe can instead say "reuse snapshot": name something that already exists in Tensorlake, and Farhelm provisions
  nothing but its own supervisor.
- Recipes are named and kept. They live in a Tensorlake section of Settings, and naming one in the add-host dialog saves
  it there.
- When Farhelm manages a recipe's sandbox image, Settings has a button to refresh the template: re-apply the recipe to a
  fresh sandbox and snapshot it again.
- Phase 2: a session launch can target a recipe instead of a host. The session then gets its own sandbox, created on
  demand from the recipe and mapped 1:1 to it.

## Decisions made while planning phase 1

- **Transport is Tensorlake's native ssh gateway**, which makes a sandbox an ssh host to Farhelm's existing transport.
  On this date the gateway was broken for sandboxes in the CAS image format (see below). The agreed fallback: build on
  `tensorlake/legacy/ubuntu-systemd` while CAS ssh is broken and on `tensorlake/ubuntu-systemd` once it works, with the
  base held in one place. Live resize is out of phase 1. The maintainer took the bug to Tensorlake directly, and
  Tensorlake fixed it the same day (see below).
- **Farhelm talks to Tensorlake through its REST API with an API key** the user pastes into Settings, stored the way the
  ntfy token is (write-only, never returned to the UI). The `tl` CLI was rejected: it has no machine-readable output for
  sandboxes and keeps its token in the OS keychain.
- **Farhelm generates its own ssh key for Tensorlake.** An API key cannot register ssh keys (only a user login can), so
  Settings shows the one-time `tl sbx ssh keys add` command. This amends the spec's "Farhelm stores no SSH credentials of
  its own". Using the user's own ssh setup was the alternative; the gateway refuses outright when the client offers an
  unregistered key first, so that path needs an `IdentitiesOnly` config block per user.
- **Templates and the refresh button are in phase 1**, because phase 2 needs exactly that path and it is cheap.
- **Suspend and resume are in phase 1.** A sandbox that idle-suspends on its own shows as Suspended with a Resume
  button; Farhelm never resumes one by itself. Sandboxes are created with the plan's maximum idle timeout, which is 24
  hours on Usage Credits and unlimited on Pro. The 24-hour limit is accepted and documented for users.
- **Removing a Tensorlake host terminates the sandbox and deletes its snapshots**, behind a confirm that says so. There
  is no "forget only".
- **A reused image must boot systemd.** Farhelm still runs its logind/linger preparation on it and otherwise refuses with
  an explanation.

## What the platform looked like on 2026-11-03

Measured with `tl` 0.5.160 against real sandboxes. Treat these as a snapshot of a young platform that changes without
notice.

- **Billing**: active CPU is billed only while busy; RAM and disk are billed on allocation while running (about $0.12
  an hour for 8 GB, whether or not anything runs); a suspended sandbox pays only snapshot storage. Suspend is the cost
  lever, not idle CPU.
- **Suspend and resume** each return in about 0.65 s. Processes, PIDs and the boot id survive; the wall clock is right
  afterwards and uptime excludes the suspended time. The 2026-09-01 entry's claims hold here.
- **Idle timeout** counts only traffic through Tensorlake's proxy. An ssh connection with an open channel counts. A
  connection with no channel does not, keepalives included (an `ssh -N` connection did not stop an idle suspend). Work
  inside the sandbox never counts.
- **SSH and suspension**: a new ssh connection to a suspended sandbox is refused ("currently suspended — resume it"),
  and no ssh traffic wakes it, although the API's `auto_resume` defaults to true. Existing connections never notice a
  suspension, because the gateway answers keepalives itself; a new channel on such a connection hangs until the client's
  keepalive kills the whole connection.
- **A silent connection leaks the sandbox awake.** The gateway dropped a connection after 300 to 400 seconds with no
  traffic at all, and the session inside the sandbox survived with its TCP connection still ESTABLISHED. That leaked
  channel kept the sandbox running, and billing, until the leftover process was killed by hand. A 30 second keepalive
  avoided it. The sshd leak in the 2026-09-01 entry (two processes left per closed session) did not reproduce.
- **CAS ssh was broken.** For every sandbox from a CAS image (the default format, and the only one that supports live
  resize), the gateway accepted the key and closed the connection about three seconds after the client opened a
  session. The same steps worked on legacy images. The in-sandbox sshd was up and reachable through the WebSocket TCP
  tunnel, so the fault was the gateway's path into CAS sandboxes. Details and a repro went to Tensorlake by way of
  <https://snippets.scode.org/s/tensorlake-cas-ssh-gateway-breakage/>, and Tensorlake fixed it the same day: a fresh
  `tensorlake/ubuntu-systemd` sandbox then took ssh logins and round-tripped binary stdio exactly. Phase 1 therefore
  builds on CAS images, with the legacy image kept only as a fallback should it regress.
- **No labels.** The API has no labels or metadata on sandboxes, so the only way to mark a sandbox as Farhelm's is its
  name.
- **The gateway itself**: one host key for every `<id>.sandbox.tensorlake.ai` name; a sandbox name works wherever its
  id does; about 0.9 s for a fresh connection plus a command and about 160 ms for a multiplexed one; 16 to 26 MB/s on one
  channel; binary stdio exact; `ssh -R` remote forwards work. Its sshd runs with `UsePAM=no`, so a login gets no
  `XDG_RUNTIME_DIR` and `systemctl --user` fails unless the command sets it.
- **systemd**: `tensorlake/ubuntu-systemd` boots systemd as PID 1 but ships logind masked. Unmasking and starting logind
  and enabling linger for `tl-user` gives a running user manager with `cpu memory pids` delegated, and transient user
  scopes work. This survives into clones, where lingering user units start on the cold boot. `tensorlake/ubuntu-minimal`
  has Tensorlake's own init instead (the browser-test harness in `.agents/tensorlake.md` gets systemd there by installing
  `systemd-sysv` and cloning).
- **Snapshots and images**: a filesystem checkpoint takes about 1.5 s and is complete about 4 s later; a clone is
  running about 2 s after create and a lingering user unit about 5 s after that. Clones share `/etc/machine-id` unless it
  is emptied before the checkpoint. Registering a snapshot as a named image is instant, and an image is only a pointer to
  its snapshot. Deleting a snapshot or an image did not break sandboxes created from it (tested on CAS only). Suspend
  snapshots and the hidden snapshots of `tl sbx copy` outlive their sandbox and bill until deleted.
- **Live resize** (CAS only) takes one to two seconds and keeps processes running. CPUs go up and down; memory never
  shrinks below what the sandbox booted with, so a sandbox that should flex has to boot small.
- **Memory checkpoints and live copies** restore running processes with the same boot id: the basis for forking a warm
  sandbox in phase 2, and a trap for Farhelm, since a forked supervisor keeps its identity.

## Phase 1 as planned

A Tensorlake host is an ssh host whose machine Farhelm also manages. The hosts table keeps `kind = 'ssh'` for it, with
the vendor detail in new nullable columns: rebuilding `hosts` to widen its kind CHECK would cascade-delete the rows of
about ten tables, and the code has always avoided that. The Rust side still gets a distinct kind so exhaustive matches
point at every decision that differs. The destination is `<name>@<name>.sandbox.tensorlake.ai`, with the sandbox name
chosen before the sandbox exists (a per-helm prefix plus a random part), so the host row itself is the record of what
Farhelm created. Adding a Tensorlake host writes the row first and then runs the slow, billed steps (template build,
sandbox create) as steps of the ordinary setup run, so progress has a place to show.

Recipes are named JSON records in the style of launch templates. A managed recipe builds a template: a builder sandbox
with a fixed per-recipe name, the apt packages, an emptied machine-id, a filesystem checkpoint. The supervisor is not
baked in; each host gets it through normal setup, so templates carry no supervisor identity. The logind and linger
preparation runs as a setup step on every Tensorlake host, so a reused image gets it too. Removal is idempotent: terminate
the sandbox (a missing one counts as done), delete the snapshots whose source is that sandbox, and forget the row last. A
Tensorlake host checks the sandbox's status through the API before dialing and shows Suspended instead of dialing when
the sandbox is suspended, which covers the user's Suspend, the idle timeout, and a suspend from the `tl` CLI alike.
Suspend closes Farhelm's own ssh connections before calling the API, because a request on a connection to a suspended
sandbox hangs instead of failing. Outbound tunnels are an `ssh -N -R` per connected host with a 30 second keepalive,
which needs an explicit amendment to the spec's rule that remote hosts cannot reach the helm machine through ssh
forwarding.

The planning review removed a separate ownership ledger and a reconcile pass at helm start. The host row (created before
the sandbox) and a fixed builder name per recipe cover what they would have covered. It also replaced a persisted
"suspended" flag with the status check above.

The UI was settled the same day from rough mockups. Add host keeps one dialog with an "SSH" / "Tensorlake sandbox"
switch; a Tensorlake host needs a name (the sandbox's real name is generated) and a recipe, and a new recipe is written
in the full editor inside the dialog. Creating a sandbox always asks first, because it bills; the general "set up new
hosts without asking" preference does not cover it, and a separate Tensorlake toggle does. The confirmation links to
Tensorlake's pricing instead of estimating cost, since hard-coded rates go stale. Settings gets one Tensorlake section
for the API key (worded to say plainly that it is stored unencrypted), Farhelm's ssh key, recipes, and the toggle. In
the UI the built artifact is a "snapshot" ("Rebuild snapshot"), because "template" already means launch templates.
Suspended hosts get an inline Resume button, launching on one resumes it first, and a session on one shows a banner
with Resume. Suspend asks first only when an agent on the host is working. Deleting a sandbox always asks, whatever the
"remove hosts without asking" preference says. Agents cannot request Tensorlake hosts in phase 1; that became a TODO.

A last round of questions settled the rest. Managed recipes install Claude Code and Codex with their official
installers (not Homebrew or npm) when the snapshot is built; for a reused image that is the user's job. Agents
authenticate through an API endpoint: an inference load balancer on the helm's side, reached through a recipe tunnel,
with Claude Code's base URL and token in its settings and a Codex model provider with a base URL and bearer token.
Farhelm writes those per host during setup. Subscription logins and credential upload, and injecting `GH_TOKEN` the way
containerized sessions get it, were deferred to TODO entries. New Tensorlake hosts start YOLO sessions without asking,
with unrestricted outbound internet. There is no automatic suspend in phase 1, and no spend limit beyond Tensorlake's
own. Settings lists Farhelm-created sandboxes that this helm has no host or snapshot build for, including ones another
helm created, told apart from the user's other sandboxes by a reserved name prefix that also identifies the creating
helm, because the API offers nothing better.

Open at the time of planning: whether a helm machine that sleeps (a laptop running the desktop app) leaves a leaked,
billing session in the sandbox via the silent-drop behavior above. If it does, the remote end of Farhelm's connection
has to give up after a stretch of silence, so that the session ends and the sandbox can idle out.

## Phase 2, as far as it was thought through

A session launch names a recipe; Farhelm creates a sandbox for that session from the recipe's template, refreshes a
repository checkout with an incremental `git fetch` from a per-repository snapshot, runs the session there, and destroys
the sandbox with the session. Things phase 2 needs that phase 1 does not:

- Fast spawn, which wants CAS (streamed snapshots start in about two seconds regardless of size), and so depends on
  the gateway fix of 2026-11-03 holding.
- Per-repository snapshots on top of a recipe's template, and a refresh story for them.
- Session-owned host rows that the hosts panel hides, with the session's lifecycle driving the sandbox's.
- Either the supervisor baked into templates with an identity reset per clone, or a per-spawn install fast enough not to
  matter.
- Answers to the trust questions in the 2026-09-29 entry: an unsupervised agent in a sandbox should not gain more reach
  into the user's other hosts than it would have anywhere else.
- Possibly live resize (boot small, grow on demand) and shared scratch volumes.
