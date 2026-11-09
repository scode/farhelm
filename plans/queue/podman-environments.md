# Podman environments: a container on one of your hosts as a session's own machine

Written against main at 231a66f7 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

This plan runs after `sandbox-sessions.md`, the Tensorlake plan that gives a session a sandbox of its own through a
hidden, session-owned host row. Read that plan file (from git history if it has left `plans/queue/`), its report under
`plans/reports/`, and the code it landed before starting. Everything below builds on what it actually landed, not on
what its plan said.

The milestone in one sentence: **a rootless Podman container on any of the user's Linux hosts becomes a disposable
machine for one session, exactly the way a Tensorlake sandbox session works: built from a shared recipe, with Farhelm's
supervisor running inside, the checkout cloned inside, and Suspend, Restart, tunnels, file links and credentials working
as on any host.** Today's experimental containerized sessions (SPEC.md, "Containerized sessions"; the agent runs in a
container while the supervisor stays outside) are retired: existing ones can only be deleted.

The general concept, a provider of disposable environments, must be encoded in code abstractions, with Tensorlake and
Podman as two implementations, so the next provider is a new implementation and the compiler points at every decision it
has to make. The core abstractions below were designed by the planner (Opus) and reviewed by a dedicated Opus design
reviewer before this plan was written.

The maintainer's words while planning (2026-11-08):

> let's try to keep as much as we can consistent for any "container" like solution - podman on a remote host is one such
> case; tensorlake sendboxes are another. we will probably add more later. and some of these REQUIRE the full host stuff
> behind the scenes (supervisors etc), so we either do that for all, or we have to support two mechanisms.

> for podman environments: yes and make sure the "general convept" here is actually properly _in code_, and not just by
> spec/agent spotting patterns. it should be properly encoded in the abstractions where appropriate.

> also have instructions in the plan that tells the execturor to get an opus consult if it needs to change the core
> abstractions significantly. propose the change, get the opus consult review.

This plan covers these TODO.md entries (bucket "Near term"); read each in full in TODO.md:

- "Bootstrap a Podman image on a remote host."
- "Podman rigor: review findings for containerized sessions." (each finding's disposition is under "Acceptance
  criteria"; the two approval-card findings move to `environment-agent-operations.md`)
- "Containerized sessions: Restart."
- "Containerized sessions: localhost tunnels and file links."
- "Containerized sessions: plain folders."
- "Credentials for containerized sessions and ordinary hosts." (the containerized half; the ordinary-SSH-host half
  stays)

`environment-agent-operations.md` runs after this plan and covers agent requests from inside environments and CLI
launches into environments. This plan keeps sandbox-sessions' refusal of every agent request from a session-owned host,
now for both providers.

## Decisions already made

Settled with the maintainer on 2026-11-08, in conversation and on a planning page with mockups. Text marked as a planner
rule or planner proposal was added afterwards, after a fresh-context review of the draft, and the maintainer sees it in
the planning PR:

- **Container as host, one mechanism.** Every provider runs Farhelm's supervisor inside the environment and is reached
  as a hidden host. No second mechanism (today's "agent exec'd into a container, supervisor outside" model) survives.
- **The concept is encoded in code.** A provider abstraction with exhaustive per-provider decisions, and a code rule in
  root `AGENTS.md` like the one for agent harnesses (see "Core abstractions").
- **Recipes are shared between providers** until there is evidence to diverge. What recipes already give Tensorlake (API
  endpoints, subscription logins, GitHub token, tunnels) is retained and applies to Podman too.
- **The checkout lives inside the container**, cloned there the way a sandbox session clones it. No bind mount of a host
  checkout. "Folder" destinations work as in sandbox sessions.
- **Hard cut for today's containerized sessions:** minimal complexity, delete only. The launcher switch, creation, tabs,
  exec and account selection go; existing sessions can be deleted and nothing else.
- **Naming:** "container" for Podman, "sandbox" for Tensorlake ("dev-box container on build-box", "dev-box sandbox").
- **Launcher:** see "The UI that was agreed".
- **A recipe not yet built on a host builds at launch**, shown as a launch step ("Preparing dev-box on build-box… (first
  launch here; then at most once a day)"). Tensorlake keeps disabling a recipe without a built snapshot, because its
  builds bill.
- **Which hosts can run containers:** every local or plain SSH host whose Podman passes a background check (rootless,
  cgroup v2, systemd cgroup manager, minimum version). No per-host opt-in.
- **After the container's host reboots**, the container is stopped with its files kept. Farhelm starts it automatically
  once it is connected to that host again, like a host coming back; the sessions then look like sessions on any rebooted
  host (interrupted, Restart offered). This differs from Tensorlake's "never resume on its own" rule, which exists
  because resuming bills. Planner rule, to keep it simple and state it in SPEC.md: any container environment found
  Stopped while its container host is connected is started once (Podman reports a reboot, a manual `podman stop` and an
  out-of-memory kill alike, and telling them apart is not worth the machinery); a failed start shows the reason and
  offers Resume.
- **sandbox-sessions' recorded decisions apply to container sessions too** (planner rule, following from "work as for
  sandbox sessions"), where they are not Tensorlake-specific. Its 2026-11-09 decision: Delete removes the environment
  only with its last session; a non-final Delete (possible after a partially failed Replace) removes only the chosen
  session, says the others and the environment remain, and refuses while the environment is suspended (paused) or
  unreachable; Suspend, Resume and Stop and suspend act on the shared environment with wording covering every session in
  it; creating ordinary additional sessions in an environment stays forbidden. (Its 2026-11-08 decision on SSH key
  registration is Tensorlake-only.)
- **Approval defaults** (planner rule): a container session's hidden host takes the same permission defaults
  sandbox-sessions gave session-owned Tensorlake hosts (the defaults of a new Tensorlake host).
- **Settings:** a new Recipes section holds the shared catalog (per recipe: which providers its base supports, and build
  status per place). Settings → Tensorlake keeps its account settings (API key, SSH key, create-without-asking,
  sandboxes not on this helm). A new Containers section lists each host's Podman status with the reason when unusable,
  and containers not on this helm.
- **Farhelm may pull images:** a recipe's own container image is pulled on the container host when missing, and the
  managed Ubuntu base pulls one pinned image per host.
- **One plan, one stack.**
- **Proxy settings:** the maintainer decided on 2026-11-08 (recorded in TODO.md's Podman rigor entry) that containers
  inherit the host's proxy settings. This plan makes that deterministic (see "Core abstractions").
- **Real verification:** a local container host for real, plus one Tensorlake test sandbox used as an SSH container host
  (budget: **1 sandbox** for this whole plan). Use a throwaway test helm with its own temporary state, never the
  maintainer's live install or its helm (root `AGENTS.md`, "The live install is off-limits"). Follow
  `.agents/tensorlake.md` for the sandbox ledger and sweep habits; its "only on request" rule is satisfied by this
  decision, for this plan only. Terminate the sandbox and delete every snapshot it made as soon as the check is done. If
  the machine has no usable Tensorlake credentials, block with that as the question rather than skipping it. Planner
  proposal for the route (log a DECISION if you use another): create the one sandbox from the stored sandbox-tests
  template in `.agents/tensorlake.md` (it carries rootless Podman), run the throwaway test helm inside it, and register
  the sandbox itself as a plain SSH container host through its ssh-to-localhost setup, the way the existing e2e second
  host does, so the full SSH route is exercised without a fixed ssh alias on this machine. That sandbox is the budget's
  one. Stand-in agents from the e2e suite count for the "conversation resumes" check; no vendor turns are spent, and the
  report says a stand-in was used.
- **Code reviewer:** a single fresh-context gpt-6.1-sol agent at high effort with the general charter below.
- **Opus consult before changing the core abstractions** (see "Changing the core abstractions").
- **No-workhorse mode**, the plans default.
- **Out of scope:** macOS container hosts, resource limits, network policy, Docker, isolation stronger than rootless
  Podman, Podman as a user-added persistent host (no "Add host → Podman"), agent requests from environments (next plan),
  subscription login changes beyond what recipes already do.

## Core abstractions

Planner design, reviewed by a dedicated Opus design reviewer. Names are proposals: map them onto the code
sandbox-sessions actually landed. The shape (what is a trait, what is an exhaustive method, what is one route) is the
agreed design; see "Changing the core abstractions" before departing from it significantly.

### Concepts

- **Provider**: a kind of disposable machine Farhelm creates for a session: `Tensorlake`, `Podman`. More later.
- **Place**: where a provider creates environments: `Place::TensorlakeAccount` or `Place::PodmanHost(HostId)` (a local
  or plain SSH host whose rootless Podman holds the images and containers). The place carries the provider, so a launch
  target is `{ recipe, place }` and cannot name an impossible combination.
- **Recipe**: the shared catalog (today's Tensorlake recipes, generalized): provider-neutral packages, agent auth,
  GitHub token and tunnels; a base whose kinds say which providers they support; Tensorlake-only size.
- **Prepared artifact**: per (recipe, place), a built image plus a prepared image with the supervisor installed and no
  identity. Tensorlake: the existing built and prepared snapshots. Podman: images in the place host's storage. The same
  staleness rule (24 h, a Farhelm update, a rebuilt source).
- **Environment**: one sandbox or container, with a reserved-prefix name chosen before creation.
- **Environment host**: the hidden host row through which the helm reaches the environment's supervisor, owned by one
  session (sandbox-sessions' session-owned host, generalized). User-owned environment hosts stay Tensorlake-only.

### The encoding rule

Every per-provider decision lives in the provider implementations or as an exhaustive method on `ProviderKind`: no
`provider == X`, no `matches!` over providers, no `_` arm over providers in shared code. Add this rule to root
`AGENTS.md`, "Code rules", next to the harness rule (keep the file under its size limit; `scripts/check-agents-md.py`),
and give the new `crates/farhelm-helm/src/environment/mod.rs` a module-doc map like
`crates/farhelm-supervisor/src/agent_kind/mod.rs`'s, saying where each piece of a new provider goes. A third provider is
then one new `EnvironmentProvider` implementation plus a compile error at every exhaustive match.

### `ProviderKind` (in `farhelm-proto`, shared by helm and UI)

Exhaustive methods for the static facts both sides need, for example `bills()`, `idle_timeout()` and
`asks_before_creating()`, plus the serde spelling. Add a method only where something consumes it; the examples are not a
required list, and leaving one out is not a change to the core abstractions. The UI keeps per-provider wording, marks
and help text as exhaustive methods in one UI module, the way the UI's `HostKind` answers per kind. No server-sent
wording keys: strings give no compile error when a provider is added.

### `EnvironmentProvider` (helm, `crates/farhelm-helm/src/environment/`)

The I/O seam. It replaces `SandboxStatusSource` (`crates/farhelm-helm/src/manager.rs`) and keeps a scripted fake for the
paused-clock actor tests. Held in `Providers { tensorlake, podman }` with an exhaustive `get(kind)`; no map.

- `places()`: usable places with refusal reasons, from cached probes.
- `ensure_prepared(recipe, place, progress)`: the two-stage build, idempotent, reporting named steps.
- `create(binding, prepared)`: the host row is recorded before this is called.
- `status(binding) -> Running | Changing | Suspended { since, resume_failed } | Stopped | Missing | Unknown(reason)`.
  `Changing` keeps Tensorlake's transitional states (today `HostState::TensorlakeChanging`), `resume_failed` keeps
  today's resume-failure detail, Tensorlake's `Terminated` maps to `Missing`, and `Unknown` is "not known to be usable".
  The generalization must not regress the Tensorlake host UI.
- `suspend`, `resume`: Tensorlake suspend and resume; Podman `pause`, and `resume` runs `unpause` from Suspended or
  `podman start` from Stopped. No provider-only method; the UI picks its wording from the state.
- `destroy(binding) -> Destroyed`: idempotent, a missing environment counts as destroyed, provider leftovers (Tensorlake
  snapshots) go too, and the result proves the destroy was confirmed against the provider.
- `list_owned(place)`: environments with this helm's marks at a place, for the "not on this helm" lists.

### Storage and `HostKind`

- Never rebuild `hosts` (about 30 tables cascade from it). Reuse the `tensorlake_sandbox` column as the environment-name
  column for every provider, renamed in code only, and add one nullable `environment_provider` column, NULL meaning
  Tensorlake so existing rows need no migration. One decode function maps
  `(kind, environment_name, environment_provider)`; an unknown provider spelling fails the decode, as an unknown kind
  does. Keeping the on-disk name keeps credential delivery's SQL guard (`tensorlake_sandbox IS NOT NULL`) correct for
  Podman rows.
- A Podman row has `kind = 'ssh'` with a synthetic unique destination in a shape `add_ssh_host` refuses (for example
  `podman:<name>`). Nothing dials it.
- A `place_host` column `REFERENCES hosts(id)`, no cascade. Removing or retargeting a host that any environment row
  names as its place (running, suspended, stopped, or an orphaned row a leftovers list shows) is refused, listing those
  environments; the foreign key enforces it too. `HostKind::can_host_environments()` is true only for Local and plain
  SSH: no nesting.
- `HostKind` gets a flat `Podman` variant next to `Tensorlake` (and `"podman"` in the UI's wire enum, which already has
  `#[serde(other)] Unrecognized`), plus an exhaustive `HostKind::environment_provider() -> Option<ProviderKind>`. The
  existing named predicates stay exhaustive matches under the wildcard lint, so the new variant is a compile error at
  each of them.
- `EnvironmentBinding { name, owner, handle: ProviderHandle }`, with the provider derived from the handle;
  `ProviderHandle::{ Tensorlake { ..today's binding fields.. }, Podman { place_host, container_id } }`. Recipe build
  state per (recipe, place) lives in the existing `state_json`, the Podman part keyed by place host id; always recheck a
  Podman artifact with `podman image exists` before trusting the record, since a host can be reinstalled.

### One route for transport and provisioning

Today the transport (`crates/farhelm-helm/src/transport.rs`, `HostTransport`, `SystemTransport::connect`) and
provisioning (`crates/farhelm-helm/src/provisioning/backend.rs`, `ProvisioningTarget`, `SystemBackend::ssh_command`)
reach hosts by separate paths, and credential delivery, setup, the installer and recipe builds go through provisioning.
A container needs both, so both derive from one route:

- `RemoteRoute::{ Ssh { destination, access }, PodmanExec { via: BaseRoute, container, user } }`,
  `BaseRoute::{ Local, Ssh { .. } }`, `HostAccess::{ Local, Remote(RemoteRoute) }`. `ProvisioningTarget::Ssh` becomes
  `Remote(route)`, keeping the serialized spelling of the SSH case (plans are shown in confirmations).
  `SystemBackend::ssh_command` becomes `remote_command(route, script)`, the only route-specific code. It builds argv
  layers,
  `[ssh <via> --] podman exec -i --user U --workdir <home> --env HOME=… --env USER=… --env LOGNAME=… --env XDG_RUNTIME_DIR=/run/user/<uid> <container> sh -c <script>`,
  quoted once per shell that parses it, never by pasting a command into `remote_farhelm`. The uid is recorded in the
  binding at creation or computed inside, as `REMOTE_RUNTIME_PREFIX` does today.
- Two sources of a route, both building through `remote_command`: `host_access(row)` for host rows (resolved from the
  store per attempt, joining the place row, because the place host's destination is user-editable), and a provider
  method giving the route to a place's builder or preparer machine, which has no host row. Together they replace every
  hard-coded `SshAccessKind::Tensorlake` use in credential delivery, the builder shell, the preparer and host creation.
  Tunnels ride the supervisor protocol and need no route.
- The transport's ssh child-process connection becomes a child-process connection (stderr relay, connected marker, exit
  diagnostics) used by ssh and by local or remote `podman exec`; the innermost command is `farhelm internal stdio`. The
  local case needs this process-spawning arm (today local only dials a socket).
- `--stdin-idle-limit-secs` moves onto `HostAccess` and is on for every environment, passed unconditionally rather than
  through the `--help` probe: killing the helm's `podman exec` client does not kill the process it started inside.
- `release_shared_connections` is per route and never closes the place host's ControlMasters. Nested exec rides the
  place host's supervisor master (dedicated masters would fail on hosts that ask for a second factor per login); sshd's
  `MaxSessions` (default 10) then caps the multiplexed environments per place. Write that limit down (SPEC_impl.md), and
  when it is hit, refuse the launch or mark the connection unavailable with a reason naming the container host's SSH
  session limit, never a bare connection error.

### Podman specifics

- **Place probe**, through the place host's route, cached per host with a timestamp and refreshed in the background when
  the launcher opens (never synchronously across every host): `podman info` showing rootless, cgroup v2,
  `cgroupManager: systemd` and the minimum version the spike pins; `systemd-run --user` working; Linux; the
  architecture. The Containers section and the launcher show the reason for an unusable host.
- **Process placement:** lifecycle commands run on the place host through its route. `podman run` and the post-reboot
  `podman start` are wrapped in `systemd-run --user --scope --collect --unit farhelm-env-<name>-<nonce>.scope`, so
  conmon and pasta live outside the helm's cgroup (locally) and the ssh login's (remotely). `container.rs` documents
  why: a helm restart or the end of an ssh session would otherwise kill them. Drop the nonce only if the spike shows an
  old scope never lingers. Resolve podman to an absolute path; strip `FARHELM_*` and the private tmux variables from
  podman's environment.
- **Proxy settings, deterministic:** read the proxy variables from the place host's user manager environment
  (`systemctl --user show-environment`, during the probe), run with `--http-proxy=false`, and write the variables into
  the container's user environment (an `environment.d` file at create) so the in-container user manager, and with it
  every session, sees them. A container's own env reaches only its PID 1.
- **Marks:** labels `farhelm.helm=<tag>` and `farhelm.role=session|build|prepare`, plus the shared reserved name prefix.
  Leftovers are found with `podman ps --filter label=…`, scanning every place with a usable cached probe and every place
  an environment row names. Podman storage is per unix account, not per helm, and several test helms (and the
  maintainer's own) can share an account, so builder and preparer container names and built and prepared image tags all
  carry the helm tag, as session container names carry the reserved prefix.
- **Superseded images:** after a successful rebuild or re-preparation, remove the image it replaced at that place once
  no container uses it; deleting a recipe, or Rebuild at a place, removes that place's images for the recipe. Images the
  user owns (a "your image" base) are never removed.
- **Suspend is `podman pause`** (processes frozen, memory kept): close the helm's exec connection first, because a
  paused container blocks the `podman exec` pipe instead of failing it. The wording never says "stop paying": pause
  frees CPU, memory stays held. **Stopped** (the place host rebooted, or anything else stopped the container) is its own
  state, never shown as Suspended; the helm runs `resume` (`podman start`) once, automatically, when it finds the
  container Stopped while connected to the place host (see decisions).
- **Managed Ubuntu base:** a non-systemd `podman run` of a digest-pinned, fully qualified Ubuntu reference (short-name
  resolution can prompt or fail without a terminal); install systemd, sudo, curl, git and bash; create the login user;
  enable linger; commit; then the shared readiness check in `build_script.rs`. Every `run` passes `--systemd=always` (a
  run flag the image does not carry). The preparer is stopped before `podman commit`, so systemd shuts down cleanly. A
  "your image" base names an image reference and its login user.
- **The preparer** reuses `finish_snapshot_install`'s guard (stopped enabled unit, absent state directory, empty
  machine-id); `SnapshotPreparer::prepare` and the logind/linger preparation take a `RemoteRoute` instead of Tensorlake
  access. Each environment's supervisor learns its identity at hello, as Tensorlake clones do.

### Recipes and launch targets

- One `Recipe` type.
  `RecipeBase::{ ManagedUbuntu, TensorlakeImage, TensorlakeSnapshot, ContainerImage { reference, user } }` with an
  exhaustive `supports(ProviderKind)`; keep the stored serde spellings (`tensorlake_ubuntu`, `image`, `snapshot`) when
  renaming variants. A recipe whose base a provider does not support is offered there disabled, with the reason. Size
  stays Tensorlake-only; Podman ignores it (no resource limits in this milestone).
- `LaunchTarget::Environment { recipe, place }`, carried by templates, Clone, recent setups and quick launch. A template
  naming a removed place host or a deleted recipe is inapplicable, the rule for a host that no longer exists.

### The supervisor

Nothing provider-specific. It runs inside the container as on any host, so hooks, conversation capture, Restart,
attachments, file links, downloads and localhost tunnels work because it is an ordinary supervisor. Environment hosts
count as remote for tunnels (a container's loopback is not the host's).

### Retiring today's containerized sessions

Delete creation, exec, tabs, account selection, the launcher switch and the template field
(`crates/farhelm-supervisor/src/container.rs` and `container/account.rs`, `crates/farhelm-ui/src/container_controls.rs`,
the branches in `service/core.rs`, `launch.rs`, the create form, templates, approvals and the terminal JS). Keep
decoding stored `ContainerConfig` (including the legacy `image_file` key) and the supervisor's removal path
(`podman rm -f`, confirmed absent, plus whatever of the lock and ticket handling is still needed once the creation entry
point is gone; shrink it with a logged DECISION rather than keeping it all by default), so old sessions can only be
deleted; resolve podman lazily, so Delete works on a host without Podman, counting a missing Podman as nothing to
remove. Stored templates, recent setups and history entries that carry a container choice still decode; the choice is
dropped or refused on use, and reading the template list never fails on them. Plain Replace of an old containerized
session is refused.

## Changing the core abstractions

The maintainer requires an Opus consult before any significant change to the abstractions above. Significant means
changing what is a trait method, an exhaustive `ProviderKind` method or a route; adding or removing a concept (provider,
place, recipe, prepared artifact, environment, environment host); moving per-provider decisions out of the provider
implementations; changing the storage discriminator; or giving a provider its own mechanism where the design shares one.
Renaming, splitting a method for testability, or mapping onto what sandbox-sessions actually landed is not.

For a significant change: write the proposal (what changes, why the design above does not work for it, the evidence, and
the smallest alternative you ruled out) to a file in the scratch directory, and run a fresh-context review on **Opus 5.5
at high effort**, launched natively or through `$scode-harness-shellout`, separate from the per-PR code review. Give it
this file's goal, decisions and core abstractions, the relevant code, and the proposal, with this charter:

> Judge whether this proposed change to the core abstractions is necessary and whether it keeps the general concept (a
> provider of disposable environments, with Tensorlake and Podman as implementations) encoded in code so that a third
> provider is a new implementation with compile errors at every decision it must make. Identify whether the existing
> design could handle the case with a smaller change, whether the proposal scatters provider decisions or adds a second
> mechanism, and what the smallest coherent change is. Write findings to the named file; change no product files or VCS
> state.

Follow its verdict or log a DECISION explaining why not; a change it rejects that you still find necessary, and that no
smaller change covers, is a block. If Opus 5.5 cannot be reached, block rather than proceed.

## The UI that was agreed

The sandbox-sessions UI (session view toolbar with Suspend/Resume, the suspended card, Stop and suspend, the
always-confirming Delete, list rows and menus, no rail entry, launch progress steps without a placeholder session) is
the pattern: a container session uses the same controls with its own wording. Reuse its classes and tokens.

**Launcher** (the maintainer picked this from two mockups, after rejecting a group per host in the Host picker because
one may have many hosts):

- A **Runs on** switch above the Host row, with three segments: **Existing host**, **New container**, **New sandbox**.
- **Existing host** shows the Host picker as today, hosts only. The recipe group sandbox-sessions put into the Host
  picker is replaced by the New sandbox segment, so Tensorlake sandboxes and containers are chosen the same way.
- **New container** shows **Recipe** and **Container host** fields. Container host lists hosts whose Podman passes the
  probe, others disabled with the reason, and defaults to the one used last. A note under the fields gives the build
  status: "Ready on build-box", or "Builds on first launch here (a few minutes)".
- **New sandbox** shows **Recipe** only. A recipe without a built snapshot is disabled with the reason, as
  sandbox-sessions does, and its creation confirmation stays.
- Recipes whose base cannot run on the chosen target are disabled with the reason. A segment with nothing usable (no
  Tensorlake API key; no host with usable Podman) is hidden, and with only Existing host left the switch is hidden.
- The footer reads, for example, "Claude · new dev-box container on build-box · owner/repo". Destinations follow sandbox
  sessions (a managed checkout, or Folder fixed to the environment's home).
- Templates, Clone prefill and recent setups restore the segment and its fields.

Mockup, as agreed:

```
Runs on   [ Existing host | *New container* | New sandbox ]
Recipe    [ dev-box ▾ ]
Container host [ build-box ▾ ]
          Ready on build-box.
──────────────────────────────────────────────
Claude · new dev-box container on build-box · owner/repo
```

**Wording.** "Container" for Podman: the closed picker and footer read "new dev-box container on build-box"; list rows
"dev-box container on build-box · owner/repo". No billing sentences for containers. Suspend's help: "Freeze the
container; everything continues on Resume. Its memory stays in use on build-box." Delete's consequence line names the
container and its files ("the checkout, anything not pushed"). Every string here is the agreed meaning, not frozen text.

**Mark.** A container mark distinct from the sandbox cube, in the style of the host marks. Read `docs/harness-marks.md`
first if it governs host marks; propose the geometry and log it as a DECISION.

**Launch progress steps** for a container, by analogy with sandbox sessions: "Preparing dev-box on build-box… (first
launch here; then at most once a day)" only when a build or preparation is needed, "Creating a dev-box container on
build-box…", "Starting Farhelm in the container…", then the ordinary launch step. A failure removes the half-made
container and its host row and shows the reason as a refusal.

**After a reboot** of the container host: while the helm starts the container, the session shows the existing
unavailable card with "Starting the container again after build-box restarted…"; then the ordinary interrupted-session
view.

**Settings.**

- **Recipes** (new section, moved out of Settings → Tensorlake): the catalog and editor. The editor's base choice offers
  managed Ubuntu (both providers), the Tensorlake image and snapshot bases (Tensorlake only), and a container image with
  its login user (Podman only), each saying where it can run. Each recipe row shows build status per place, for example
  "Ready on Tensorlake · Ready on build-box · Not built on this machine", with Rebuild per place. Size is labelled
  "Tensorlake sandboxes only".
- **Tensorlake** keeps the API key, Farhelm's SSH key, the create-without-asking switch and "Sandboxes not on this
  helm".
- **Containers** (new section): each host's Podman status ("Ready", or the reason it is unusable, such as "Podman 1.9 is
  too old", "cgroup v1", "not rootless"), refreshed in the background, and "Containers not on this helm" with Delete, by
  analogy with the Tensorlake list.

**Hosts panel.** Removing or retargeting a host that runs containers is refused, naming the container sessions; the
remedy is deleting them first.

**Templates.** The template editor's Host field offers the same three-way choice as the launcher's Runs on switch,
stored as recipe plus place.

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:**

- SPEC.md: Topology and host registry; the Tensorlake host, recipe and sandbox-session paragraphs; "Containerized
  sessions" (replaced by this plan); Sessions → Creation, Managed checkouts and Lifecycle operations; the Session view;
  the Session list; Settings; Durability and resume; Agent-spawned sessions; Security; Non-goals; "Local authority and
  trust between hosts"; the transport statements ("SSH, and only SSH, for every remote supervisor"; "no relay, no
  supervisor-to-supervisor connection").
- SPEC_impl.md: the Tensorlake sections, "Rootless Podman sessions" (replaced), the transport and provisioning sections.
- Root `AGENTS.md`: "Code rules" (harness rule; host-kind decisions through named predicates on `HostKind` in
  `crates/farhelm-helm/src/store.rs` and `crates/farhelm-ui/src/lib.rs`), "The live install is off-limits", "Sharing the
  machine with other agents", "Agent scratch space", "Tests and validation", "Changelog fragments", "TODO.md and
  FLAKES.md", "Frozen material".
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/tensorlake.md`.
- `website/AGENTS.md`: the docs page `website/src/content/docs/docs/agents/containerized-sessions.md` and the pages that
  mention containerized sessions describe the retired feature; rewrite them for container sessions. Unattended runs
  start no preview server; name the changed pages in the report.
- `releasing/AGENTS.md` before touching `.github/dist-build-setup.yml` (its Podman setup feeds CI).
- `docs/harness-marks.md` before adding a mark.

**Frozen material to leave alone:** the release shots and their tests (`e2e/docs-shots/release-notes.spec.ts` mentions
containers), published release notes and `CHANGELOG.md` sections, `lore/`. If retiring containerized sessions breaks a
frozen release-shot test, block with that as the question rather than editing it.

**Planner proposals** (change them if the code says otherwise, and log a DECISION): every name in "Core abstractions";
the synthetic destination's shape; the probe's cache lifetime; the scope unit's name; the managed base's pinned image
and login user name; the Recipes and Containers sections' exact layout; the mark's geometry; every UI string.

## Acceptance criteria

- **In code:** `ProviderKind`, `EnvironmentProvider` with Tensorlake and Podman implementations, `Providers`,
  `HostAccess` and `RemoteRoute`, and the generalized recipe exist as described, with the encoding rule in root
  `AGENTS.md` and the module map in `environment/mod.rs`. Shared code has no `==`, `matches!` or `_` arm over providers
  outside exhaustive methods (on `ProviderKind`, `RecipeBase::supports`, `ProviderHandle`, `HostKind`, and the one UI
  wording module). Tensorlake hosts and sandbox sessions behave as before apart from the agreed launcher,
  template-editor and Settings changes: their helm and lifecycle tests pass unchanged apart from renames, and browser
  specs for the moved UI are updated to the new controls.
- **Launch:** a container session launches from the launcher, a template, quick launch, Clone and a recent setup, on a
  local host and on an SSH host: the host row is recorded first; a missing base or prepared image builds as a launch
  step; the container is created in its own systemd scope with the session's labels; Farhelm connects through
  `podman exec`; credentials and recipe tunnels are delivered; the managed checkout is cloned inside; the agent starts.
  A failure at any step leaves no container and no row (helm unit tests on a fake Podman at every step). A helm stopped
  in the middle leaves a container and row that "Containers not on this helm" lists.
- **Lifecycle:** Suspend (`pause`) freezes and Resume continues the agent's running command; Stop, Stop and suspend,
  Restart, Restart with, Replace and Replace with work as for sandbox sessions; Delete always confirms and removes the
  container and its row, confirmed against Podman; a container found Stopped while its host is connected (after a
  reboot, or `podman stop` outside Farhelm, which is how tests simulate the reboot) is started once on its own and the
  sessions show as interrupted with Restart offered, and a failed start shows the reason and offers Resume; a helm
  restart reconnects; sandbox-sessions' non-final Delete rule holds for containers (refused while paused or
  unreachable). Superseded images are removed as described. Hitting the container host's SSH session limit gives a
  reason naming it.
- **Ordinary-session features inside:** hooks and conversation capture (Restart resumes the conversation), attachments,
  terminal tabs, file links for any path in the container, downloads, and a localhost tunnel reaching a port inside the
  container.
- **Hosts:** a host whose Podman fails the probe shows the reason in the launcher and in Settings → Containers; a host
  running containers cannot be removed or retargeted. Session-owned container hosts never appear in the hosts panel, the
  rail, host filters, the launcher's host list, the template editor's host list or the agent-facing host listing.
- **Agent requests** from a container session are refused through the same mechanism and notification sandbox-sessions
  uses for sandbox sessions, with wording that covers both ("Agents in sandbox and container sessions can't ask Farhelm
  to act yet."), and agent Delete or Clone aimed at a container session from another host is refused the same way.
- **Retirement:** no path creates the old kind of containerized session; an existing one can be deleted (including on a
  host without Podman, where there is nothing to remove) and nothing else; stored templates, recent setups and history
  carrying the old choice still load.
- **Podman rigor findings**, each settled and named in the report: launch approval cards and their card kind move to
  `environment-agent-operations.md`; detach keys become moot (no `podman exec -it` attach for agents or tabs); the token
  file symlink becomes moot (credentials come from the catalog); proxy inheritance is deterministic and written into
  SPEC.md, SPEC_impl.md and a code comment; a create retry never removes a container that may hold session work (the row
  is recorded first and only a failed launch before the session exists, or Delete, destroys); a checkpoint image is
  refused as a recipe base.
- **Spec first:** SPEC.md and SPEC_impl.md are amended in the stack's first PR (or a leading docs PR) for everything
  listed under binding constraints, including replacing "Containerized sessions" with the container-session model and
  the transport statements.
- **Tests:** helm unit tests on a fake Podman and the scripted provider fake; Rust e2e tests on the local Podman for the
  launch, Suspend/Resume, Stop, Delete and reboot-restart paths (replacing
  `crates/farhelm/tests/e2e/containerized_sessions.rs`); browser specs on the e2e fake for the launcher, Settings →
  Recipes and Containers, the session view and the list, and Delete-only old sessions. Log what the browser specs could
  not cover and how it was covered instead.
- **Real verification** (see decisions): on the local container host and on one Tensorlake test sandbox used as an SSH
  container host, launch a container session with a managed checkout of a public repository, run a command, Suspend and
  Resume (the command survives), Restart (the conversation resumes), open a localhost tunnel to a port inside, Delete
  (nothing left in `podman ps -a` or the images beyond the recipe's built and prepared images). Record the commands and
  results in the report, with the sandbox count.
- **Changelog:** a fragment (`kind: added`) describing container sessions with their caveats (experimental; Linux hosts
  with rootless Podman and cgroup v2; the checkout lives in the container and Delete removes it; Suspend keeps memory in
  use; the first launch on a host builds the image; about ten containers per SSH host, from that host's SSH session
  limit; agents can't ask Farhelm to act yet), and saying that the earlier experimental containerized sessions can now
  only be deleted.
- **TODO.md:** the last PR removes the entries this plan covers (listed under "The goal"); narrows "Credentials for
  containerized sessions and ordinary hosts" to ordinary SSH hosts; keeps the two approval-card findings as one entry of
  their own for `environment-agent-operations.md`, ending with that plan's reference
  (``Plan: `plans/queue/environment-agent-operations.md`.``); and rewords, without adding entries, the two that describe
  the retired model: the Maybe later entry about extending containerized sessions to macOS (drop its pointer to a plan
  file that does not exist) and the last bullet of "Stop containers an agent started through podman before killing
  processes".

## Outline

The code today, from read-only maps made while planning (verify each before relying on it; sandbox-sessions will have
changed some of it):

- **Host kinds:** `HostKind { Local, Ssh, Tensorlake }` in `crates/farhelm-helm/src/store.rs` (`from_columns` maps
  `('ssh', Some(sandbox))` to Tensorlake; exhaustive predicates around lines 718–872); the UI's `HostKind` in
  `crates/farhelm-ui/src/lib.rs`; wire strings in `crates/farhelm-helm/src/hosts.rs`.
- **Transport:** `crates/farhelm-helm/src/transport.rs` (`HostTransport`, `SystemTransport`, `ssh_connection`);
  `crates/farhelm-helm/src/ssh.rs` (`ssh_stdio_args`, `SshAccessKind`, the stdin idle limit); the far end is
  `farhelm internal stdio` (`crates/farhelm/src/main.rs`, `stdio_proxy`).
- **Provisioning:** `crates/farhelm-helm/src/provisioning/` (`backend.rs` `ProvisioningBackend`, `SystemBackend`,
  `run_shell`, `ssh_command`, `ssh_put`, `spawn_probe`; `plan.rs` `ProvisioningTarget`; `credential_delivery.rs`;
  `snapshot_preparer.rs`; `tensorlake_add.rs`; `tensorlake_backend.rs`).
- **Tensorlake:** `crates/farhelm-helm/src/tensorlake/` (`recipes.rs`, `build.rs`, `build_script.rs`, `prepared.rs`,
  `shell.rs` `SandboxShell`, `hosts.rs`, `lifecycle.rs`, `tunnels.rs`, `orphans.rs`); `SandboxStatusSource` in
  `manager.rs`; recipe API types in `crates/farhelm-proto/src/tensorlake/recipes.rs`; UI in
  `crates/farhelm-ui/src/tensorlake/` and `tensorlake_settings.rs`.
- **Agent requests:** `crates/farhelm-helm/src/agent_requests.rs` (`AgentOrigin`, `HelmAgentRequests::handle`).
- **Today's containers:** see "Retiring today's containerized sessions"; the e2e suite
  `crates/farhelm/tests/e2e/containerized_sessions.rs`; CI's Podman setup in `.github/dist-build-setup.yml` and
  `scripts/tensorlake/bootstrap.sh`.

Before the first PR, run the **go/no-go spike** on the minimum Podman version you intend to require (the planning
machine had rootless Podman 4.9.3 on cgroup v2 with the systemd cgroup manager), and record it in the working log:

- `--systemd=always` boots rootless to running or degraded;
- `user@UID.service` starts through linger inside, and the supervisor's transient scopes work under it;
- `podman exec -i` is byte-exact both ways (a large random stream through `cat`, and the full stdout of a short-lived
  command), and stdin EOF propagates;
- pause and unpause with a live exec, and the exec connection afterwards;
- commit of a stopped preparer;
- whether an old `farhelm-env-…` scope can linger (decides the nonce).

If the in-container user manager cannot run, block: no unattended fallback to a different install or containment model.

A likely stack (reshape freely, keeping each PR reviewable, main never broken, and no churn):

1. `docs:` the spec amendments, and the encoding rule in root `AGENTS.md`.
2. `refactor:` `ProviderKind`, the `environment` module and `EnvironmentProvider` with the Tensorlake implementation,
   replacing `SandboxStatusSource`; no behavior change.
3. `refactor:` `RemoteRoute`, `HostAccess`, `ProvisioningTarget::Remote`, `remote_command`, the child-process connection
   and `host_access`; no behavior change.
4. `refactor:` the generalized recipe (`Recipe`, `RecipeBase::supports`, per-place state); no behavior change.
5. `feat:` storage for Podman environment rows (`environment_provider`, `place_host`, `HostKind::Podman`) and the place
   probe.
6. `feat:` the Podman provider: managed base bootstrap, build and preparation per place, create, status, pause, resume
   and start, destroy, leftovers, proxy settings; helm unit tests on a fake Podman.
7. `feat:` launching container sessions: launch flow, launcher, templates, Clone and recent setups, session view, list,
   marks, automatic start after a reboot.
8. `feat:` Settings → Recipes and Containers, and the hosts panel refusal.
9. `feat!:` (or the type that fits) retiring today's containerized sessions, and the docs pages.
10. Browser specs and Rust e2e tests if not folded in above; the real verification; the changelog fragment; the TODO.md
    update.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect: `cargo fmt` and clippy on the touched crates;
the helm crate's host, transport, provisioning, Tensorlake, sessions and agent-request tests, the supervisor's tests,
the UI crate's tests, and the new Rust e2e tests, through `python3 scripts/record-test-run.py`; the touched browser
specs on both engines; `dprint check` on changed Markdown; the changelog check; `python3 scripts/check-agents-md.py`
(this plan changes root `AGENTS.md`); `python -B scripts/check-test-sleeps.py` when tests change; and the real
verification.

## Fallbacks

These departures need the scope reassessment:

- any second mechanism for a provider (a supervisor outside the container, a vendor exec API instead of a supervisor);
- a bind-mounted host checkout;
- a relay through the place host's supervisor instead of `podman exec`;
- Docker support or resource limits;
- a placeholder session row.

These need the Opus consult in "Changing the core abstractions" (and the scope reassessment if they also add a
mechanism): any significant change to the core abstractions.

Block, rather than work around, in any of these cases:

- the spike shows the in-container user manager cannot run on a supported Podman;
- storing Podman rows would need a `hosts` table rebuild;
- verification would need more than the one Tensorlake sandbox the decision allows, or there are no usable Tensorlake
  credentials;
- a frozen release-shot test breaks;
- an Opus consult rejects a change you still find necessary, or Opus 5.5 cannot be reached for one.

Log these as DECISIONs: the spike's results and the minimum Podman version; every name that departs from "Core
abstractions"; the synthetic destination; the probe cache; the managed base's image digest and login user; the scope
name; the mark; the Settings layout details; what the browser specs could not cover; every review finding or consult
verdict you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-podman-environments-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that root
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type,
`$scode-harness-shellout` supplies the launch mechanics.

The review gate and the Opus consult keep the models and efforts this file demands; the scope reassessment runs at your
own model and effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the
resume check, and the cold reads of a blocked question and of the report) run as that file says: native, read-only, at
your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). Podman's own storage
(`podman info`'s graph root) counts as a watched filesystem too, since builds commit images there. It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when a watched filesystem drops
under 10% or under 5 GB free or available memory drops under 10% (whichever comes first), when the number keeps falling,
when the condition recovers, and when the monitor itself fails. In Claude Code a notification is a stdout line of a
monitor started with the Monitor tool; elsewhere, the harness's equivalent. A file update alone is not a notification.

Before relying on the watchdog, verify delivery with a harmless synthetic alert, and verify failure detection by killing
a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a throwaway monitor
without killing it and confirm you are notified within two sample intervals. If any of these notifications is
unavailable or unverified, say so in the log and check the status file at least once a minute. On every path, check
heartbeat freshness before each new build, test run, sub-agent or review launch; a dead monitor, or a heartbeat stale
for two sample intervals, pauses new launches until monitoring is restored.

An alert is an instruction to act: stop launching work, remove jj workspaces, build output, scratch, containers and
images you own that are no longer needed, wait for or stop the job most likely responsible, and resume only when the
watchdog reports headroom. Record the watchdog's handle, watched paths, status path and delivery mechanism in the log,
include its state in every handoff note, and stop it when the plan closes. Do not lengthen the sampling interval to save
turns. Remove every container and image your tests and verification created when you are done with them (root
`AGENTS.md`, "Agent scratch space"), and never touch containers or images you did not create.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/podman-environments/<nn>-<short-name>`.
- Avoid churn; within this run, a PR that needs correcting is restructured rather than corrected on top, including PRs
  an earlier run built.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's
  cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`. PRs stay drafts: never mark one ready and never merge; landing is the monitor's job.

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on **gpt-6.1-sol at high effort**,
launched through `$scode-harness-shellout` (or natively, if your harness serves that model natively). If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions,
core abstractions and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires. Where you disagree with a finding,
decide on the merits and log the DECISION. A finding that would significantly change the core abstractions goes through
the Opus consult before you act on it.

### Scope reassessment

Before implementing a substantial departure from the outline (examples under Fallbacks; not a blacklist), or when the
same component has needed repeated corrective review rounds, run a fresh-context review at your own model and effort.
Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered, including every review finding and consult verdict you decided not to follow.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Wording in this file that turns
out inaccurate (a line number, a file name, a type name) is corrected to the truth. A material scope expansion, a
weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or
a log entry is not authorization. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should
know and possible follow-ups go into the report's "things you should know" and "open questions and possible follow-ups"
sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, every significant change to the core abstractions passed its Opus consult, the real
verification is recorded in the working log (and carried into the report when you deliver), and the last PR updates
TODO.md as described. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12). Never edit `plans/`
yourself.
