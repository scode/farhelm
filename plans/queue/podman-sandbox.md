# Feature: containerized sessions with rootless podman (experimental, Linux only)

Written against main at ad8b2762 on 2026-10-28. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan adds to them.

NOTE: Describe API balancers and proxies only in generic terms ("a local API proxy or load balancer on the host") in
code, tests, docs, commit messages and the report. No private tool, host, user or product names.

## The goal

The source is the TODO.md entry in the "Maybe later" bucket, verbatim:

> Containerize agents on remote hosts or locally on the desktop, with Docker, cgroup-based isolation, or something
> similar. The cgroup scopes Farhelm already launches into track and reap an agent's processes; they do not isolate it.

This plan builds the first, deliberately minimal step of it. The maintainer's request, verbatim except for one private
tool name replaced by `[...]`:

> Build a minimal version of podman support for farhelm. By minimal I mean:
>
> - We do not do anything yet to assist in downloading or creating a container image. The contract will just be "ubuntu
>   version X with FOO packages installed" or something.
> - We focus on the isolation of _capabilities_ of agents, not resources use. Just max mem etc. It's about creating a
>   _sandbox_, not constrainints its size or cpu usage.
> - The user is responsible telling us the path to the container image on the target host (for now).
> - We support, for now, setting up the port forward required for a codex-lb/[...]/cli-proxy-api type setup, but for now
>   the user is responsible for providing a container image in which the user also has a pre-configured codex/claude/etc
>   set up right to expect a API proxy on a certain port (farhelm asks the user, so the user can choose whatever port
>   they use anyway and use the same config, we just need to know which port to setup to forward to the host's
>   balancer).
> - For how we assume the balancer is on the local host (next to the supervisor, not the helm).
> - When launching a session there is a "Containerized" option which currently only supports podman, and it requires
>   picking the path to the image and the port number.
> - It is advertised in changelog and docs as an experimental feature subject to rapid change, but the docs will contain
>   the basic info on what is required of the image etc.
> - No support for subscription auth right now.
>
> linux support only

Added by the maintainer while planning:

> we need the injection of a GH token. have farhelm prompt for the path to a file on the host that contains the gh_token

> container does NOT get to read/write outside of the container or anything we intentionally inject/bind mount/make
> available

On tabs: "this is vital and must be in SPEC - it should "feel" like the container is the host. otherwise UX is confusing
and it's easy to make mistakes."

### What a containerized session is

A session launched with the new **Containerized** option has one rootless podman container on the session's host, for
the session's whole life: created at its launch, removed when the session is deleted. Its agent and its terminal tabs
both run inside that container, so the container feels like the session's host. The container gets exactly these things
from the host and nothing else:

- the session's managed checkout, mounted read-write at the same absolute path it has on the host, as the working
  directory;
- one TCP port: connections to `127.0.0.1:<port>` inside the container reach `127.0.0.1:<port>` on the host, where the
  user runs an API proxy or load balancer next to the supervisor;
- optionally, a GitHub token read from a file on the host, as the `GH_TOKEN` environment variable of the agent and of
  each tab.

Outbound network access to the internet stays open (git push, package installs). No Farhelm program, socket, state file,
credential or hook enters the container.

### Facts verified on a Linux host with rootless podman 4.9.3 while planning

These shaped the design; re-verify them on the executing host before relying on them, and log what you find.

- `--network pasta:-T,<port>` makes exactly that host loopback port reachable from the container's `127.0.0.1:<port>`;
  another host loopback port is unreachable, also through the container's default gateway address. The default network
  reaches no host loopback port at all.
- `podman run oci-archive:<absolute path> ...` runs straight from an image file saved with
  `podman save --format oci-archive`; podman copies it into local storage first and reuses layers it already has.
- `--userns=keep-id:uid=1000,gid=1000` runs processes as the Ubuntu 24.04 image's `ubuntu` user (uid 1000,
  `HOME=/home/ubuntu`), and files they write into a bind-mounted host folder come out owned by the host user.
- With that mapping, a file created by container root shows up on the host owned by a subordinate uid (165536 there):
  the host user can delete it from a folder they own but cannot edit it, or empty a folder root created, without
  `podman unshare`.
- Podman passes the caller's environment into a container only through explicit `--env` flags; `--env NAME` takes the
  value from podman's own environment, so a value never needs to be on a command line. The user's own `containers.conf`
  can still add variables and mounts to every container (that host's adds proxy variables).
- A variable passed with `--env` is written into the container's (or exec session's) config under the user's podman
  storage, and `podman inspect` shows it, for as long as that record exists.
- The host user can signal processes running as container root (the user owns the user namespace) and can read the
  container's cgroup, but cannot read those processes' environment.
- `podman run` and `podman exec` leave their helper process (conmon), and `podman run` its network helper (`pasta`), in
  the caller's cgroup and with the caller's environment, including any `FARHELM_*` markers, while the container's
  processes go into a separate `libpod-<id>.scope`. Killing a running container's conmon leaves it in a state only
  `podman rm -f` clears: `podman stop` then leaves it "stopping", and `podman start` refuses it.
- Killing a `podman exec` client alone leaves the process inside running, and `podman exec` does not forward SIGTERM.
  Killing the exec session's conmon ends the process inside by hangup, but only for an exec with a terminal (`-t`);
  without one, the process survived.
- After an exec session's scope is SIGKILLed, the next `podman stop` waits about five seconds and exits non-zero even
  though the container did stop; `podman container inspect` shows the real state. `podman start` on a running container
  is a silent no-op. With `systemd-run --scope --collect`, a unit name can be reused once the earlier unit is gone.
- The shared rootless podman pause process inherits the environment of whichever process first ran podman after boot,
  Farhelm markers included.
- A variable passed with `--env` to a `podman exec` stays in podman's storage only while that exec session lives.

### Acceptance criteria

**Launcher (web UI and desktop app share it).**

- The Agent tab gets a "Containerized" switch, labelled experimental, placed directly after the wrapper prefix in the
  launch controls. Switching it on reveals three inputs: **Image file** (required: an absolute path on the session's
  host to an OCI archive), **API proxy port** (required: 1–65535), and **GitHub token file** (optional: an absolute path
  on the session's host; blank means no token). Each has a one-line help text. Launch is blocked, with the reason beside
  the actions as for other invalid fields, until the required inputs are valid.
- The switch is offered for managed-checkout launches only. With "Where: folder" it is unavailable, with a short reason.
  Command-tab launches never offer it.
- The footer summary shows a `container` marker when the switch is on, the way it shows `yolo`.
- Nothing is remembered between launches: the switch starts off and the inputs empty. Values come back through Recent
  setups, Clone, Replace with, and templates, which carry them because they live in the launch.
- Templates carry the settings: the template fields, the template editor, and "Save as template". `farhelm spawn` and
  other CLI flags for them are out of scope (a separate Near term TODO entry).

**The container (supervisor).**

- One container per session, named from the session id the way scope unit names are derived (never stored), created
  rootless from `oci-archive:<image file>` at the session's launch. Its main process is an idle keeper under podman's
  own init (`--init`), so the agent and tabs are `podman exec` sessions inside it. It runs as the image's uid-1000 user
  through `--userns=keep-id:uid=1000,gid=1000`, mounts only the checkout (at its host path, read-write), uses the
  network `pasta:-T,<port>`, and keeps podman's default privileges (privilege changes such as `sudo` inside are
  allowed). Podman's init is podman's own binary, not Farhelm's or the user's data, and is allowed.
- The container's own helpers (conmon and `pasta`) must not live in the agent's scope or a tab's scope, and must not
  carry the session's markers, so neither an agent stop, an agent exiting, a tab closing nor any of Farhelm's sweeps
  takes the container with them. So every podman call that creates or starts the container runs with
  `FARHELM_SESSION_ID`, `FARHELM_AGENT_ID`, `FARHELM_TAB_ID`, the session token and the socket variable removed from its
  environment, in a scope of its own made with the existing scope wrapper (`ScopeManager::launch_prefix`). Name that
  unit so neither existing unit-name pattern matches it (for example `farhelm-<session>-container`), and have Delete
  check it by name. On a host with no usable systemd user manager, start the container unscoped, still without the
  markers. The agent's and tabs' `podman exec` clients keep their markers, which is how the existing stop and tab-close
  sweeps still find their exec helpers. A runtime test asserts that the container's conmon carries no session marker. A
  supervisor restart needs nothing extra: the supervisor's service leaves its children running.
- Creating the container (which copies a possibly large image into podman's storage) shows its progress and any failure
  in the session's terminal, the way the managed checkout's clone does (for example, the shim runs the scoped
  `podman run -d` in the pane before the agent's exec). A start that finds the container already running is a success.
- The agent runs as `podman exec -it` in the checkout. Its environment inside the container is the image's own plus
  terminal type and colour variables, plus `GH_TOKEN` when a token file is given. Farhelm's own variables reach only the
  host-side podman client. The container keeps running after the agent exits on its own, until stop or Delete.
- The token file is read at every agent start and every tab open, with surrounding whitespace trimmed, and passed with
  `--env GH_TOKEN` in its name-only form. The token never appears on any process's command line or in anything Farhelm
  writes (database, logs, launch spec file). The supervisor's pre-launch check confirms the file is readable and
  non-empty and keeps nothing; the read that delivers the token happens later, at exec time, and its failure is a launch
  failure shown in the terminal.
- No Farhelm integration runs inside the container: no hook arguments or reporter environment are added to the agent's
  command line, no `BROWSER` fallback, and the Claude workspace-trust edit to the host's Claude config is not made. Such
  a launch behaves like one whose conversation hooks the user turned off (`FARHELM_AGENT_HOOKS`, SPEC.md around line
  2342), which today is a supervisor-wide setting and needs a per-launch override: the screen still drives status, none
  of the missing-conversation-hook notifications fire, and there is no conversation capture. Plain Restart is therefore
  unavailable with its reason shown, as SPEC already says for a session without a captured conversation. Restart with is
  also unavailable with its reason shown: that is a new rule, an explicit exception to SPEC's Restart with paragraph
  (around SPEC.md line 1306, which otherwise offers it with a required conversation field), and the new SPEC section
  says so. Replace and Replace with start over in a new session with a new container.
- The supervisor refuses a containerized launch with a plain message, before starting anything, when the host is not
  Linux, no `podman` is found by absolute path, the image path is not an existing regular file, the token file cannot be
  read or is empty, or the launch is not into a managed checkout. Anything else podman rejects shows podman's own error
  in the session's terminal through the existing launch-failure path. No usability probe or host capability report is
  added.

**Isolation (each proven by a runtime test).** From inside a containerized session's container:

- a file on the host outside the checkout (one beside the checkout, and one in the host user's home) cannot be read or
  written, and the supervisor's socket and state folder do not exist at their host paths;
- the proxy port reaches a listener on the host's loopback;
- another host loopback port is unreachable, directly and through the container's default gateway address;
- no Farhelm credential or socket variable is present in the agent's or a tab's environment.

**Terminal tabs.** A tab in a containerized session is a login shell inside the session's container, in the checkout,
through `podman exec -it`, run directly by the tab helper (`farhelm internal tab-shell`) rather than after the host's
login shell, so the host's shell startup files never run first. The tab helper reads the token file itself; the token
never passes through tmux. Opening a tab after stop starts the container again, under the session's existing lifecycle
claim that tab opening already takes; a tab opened before the container exists is refused with a plain message. Tabs
survive the agent exiting on its own. Closing a tab stops the tab's own scope as today, which ends the tab's shell
inside the container by hangup; a process the tab started that detached from its terminal keeps running until the
container stops (stop, Delete). The SPEC states that limit. After a host reboot the session's terminal is gone, and
since Restart is unavailable, a containerized session can then only be replaced or deleted; the SPEC says so.

**Stop, Delete and Replace.**

- Stop (the session menu's "stop", and `farhelm agent stop`) stops the whole container: the agent, its tabs, and every
  process in it. It runs the existing agent stop first (the agent's scope, which ends its exec by hangup, since
  `podman exec` forwards no SIGTERM), then `podman stop` on the container. Stop on a session whose agent already exited
  still stops the container. The container and its files are kept; a later tab starts it again. This differs from an
  ordinary session, where stop leaves the tabs running, and the SPEC says so.
- Delete removes the container through podman after Delete's existing read-only preflights and upload cancellation and
  immediately before the process-tree reap, so a Delete that a preflight refuses never loses the container. Removal is
  idempotent (`podman rm -f --ignore`, then confirm the container no longer exists), so a retried Delete converges. The
  container's own scope is checked by name. Replace deletes the old session, and its container with it. Two containers
  forwarding the same proxy port run side by side, so Replace needs no ordering care.
- Confirmation comes from podman's reported state, not from a podman command's exit status, the way the scope kill
  treats `systemctl kill`'s: stop is confirmed when the container reads as exited or stopped, Delete when it no longer
  exists. A stop or removal that cannot be confirmed fails the operation, with podman's state shown, the way an
  unconfirmed scope does today (SPEC.md, Lifecycle operations; SPEC_impl.md around line 2941). Stopping only a scope is
  never enough on its own: that leaves the container running.

**File links in terminal output.** File links resolve paths on the session's host, so in a containerized session a path
outside the checkout names a host file, not the container's. In a containerized session's agent terminal and tabs, file
links are offered only for paths inside the checkout, which is the same path on both sides; any other path is not
offered as a link at all, so clicking can never download a host file the container only named. Tests cover both sides of
that boundary, including a path that leaves the checkout through `..` or a symlink.

**Spec, docs, changelog, TODO.**

- SPEC.md gets a section on containerized sessions. It states the contract above in product terms: what the container is
  given and what it is not; one container per session; the "feels like the host" principle with tabs inside; stop
  closing the tabs too; Restart unavailable; file links limited to the checkout; the uid-1000 image user; and the
  limitations: the LAN, a tailnet and cloud metadata addresses stay reachable; a localhost tunnel, including one offered
  for a URL the agent prints, reaches that port on the host, not in the container; no Farhelm integration inside;
  detached tab processes live until the container stops; root-created files in the checkout come out owned by a
  subordinate uid (with the `podman unshare` remedy), which also makes emptying the Trash fail for an archived checkout
  holding a folder container root created, until the user runs `podman unshare rm -rf` on it; the token is visible to
  the user's own podman tooling while an exec session that received it lives; the user's `containers.conf` applies;
  Delete removes the container but not the image podman copied in, which stays in the user's podman storage until they
  prune it; the container keeps running after the agent exits until stop or Delete; after a host reboot the session can
  only be replaced or deleted; managed checkouts only. The trust section ("Local authority and trust between hosts",
  which says sandbox support would need "a new, explicit isolation contract") points at it. SPEC_impl.md records the
  mechanism and its reasons.
- A docs page under `website/src/content/docs/docs/agents/`, beside "Agent wrappers", with an Experimental badge (the
  stub badge mechanism with `variant: caution` is the nearest precedent; no "experimental" precedent exists yet). It
  says the feature is experimental and subject to rapid change, and what the image must provide: a Linux OCI archive
  saved with `podman save --format oci-archive`, tested with an Ubuntu 24.04 base; a uid-1000 user whose home holds the
  harness's configuration, set up to use the API proxy at `127.0.0.1:<port>` (no subscription logins); the harness
  binaries on `PATH`; `bash` and whatever the keeper process needs (for example `sleep`); `git` and `gh`, with
  `gh auth setup-git` or equivalent so git uses `GH_TOKEN`; optionally `sudo`. It lists the limitations above. It may
  include a minimal example `Containerfile`; Farhelm itself builds and downloads nothing.
- A changelog fragment, `kind: added`, saying the feature is experimental and may change quickly.
- The last code PR narrows the source TODO.md entry to what remains (the macOS desktop, help with images, stronger
  isolation; the other follow-ups have their own Near term entries), keeping its plan reference.

## Decisions already made

Settled with the maintainer on 2026-10-28, on the planning page:

- **No Farhelm integration inside the container** (hooks-off behaviour). Full integration is a Near term TODO entry.
- **Image file:** a path to an OCI archive, run directly from the file.
- **User:** the image's uid-1000 user through keep-id; the harness config lives in that user's home. Matching the
  in-container uid, user name and paths to the host account is a Near term TODO entry.
- **Token:** an optional file, passed as `GH_TOKEN`, read at each start, never stored by Farhelm and never on a command
  line.
- **One container per session**, created at launch and removed on Delete. The maintainer first chose one per agent
  launch, then asked whether that really was simpler; told the difference is modest, chose one per session, which also
  makes Restart possible later.
- **Tabs open inside the container**, and SPEC must say a containerized session should feel like the container is its
  host.
- **Stop stops the whole container, tabs included.** The maintainer was shown that ordinary sessions keep their tabs on
  stop, and that the alternative (stop only the agent, tabs survive) would let a detached root process inside the
  container outlive stop, and chose this. Whether ordinary sessions should also close tabs on stop is a Near term TODO
  entry.
- **Restart is unavailable, with its reason; Replace starts over.** For plain Restart that is what SPEC already says for
  a session without a captured conversation. Restart with is made unavailable too, as a stated exception to SPEC's
  Restart with paragraph, since relaunching the agent alone would contradict stop's whole-container behaviour (the
  planning review found this; it follows the maintainer's choice of "Restart unavailable"). Restart for containerized
  sessions is a Near term TODO entry.
- **Where it's offered:** Agent launches only, shown on every host, with the supervisor refusing plainly where it cannot
  work; no new host capability report. Managed checkouts only (chosen over allowing plain folders, where git breaks for
  worktrees and repository subfolders); plain folders are a Near term TODO entry.
- **Templates are included** ("templates are needed or it'll get super annoying to use even for early testing");
  `farhelm spawn` and CLI flags are not, and are a Near term TODO entry.
- **Network:** the LAN, a tailnet and cloud metadata stay reachable, documented. Localhost tunnels and file links
  outside the checkout reaching the container are a Near term TODO entry.
- **File links only inside the checkout.** Agreed after planning showed that a link to a path outside the checkout would
  download the host's file rather than the container's: such paths are not offered as links in containerized sessions.
- **Privileges:** podman's defaults, so `sudo` inside works; the root-owned-files caveat is documented with its
  `podman unshare` remedy.
- **Tests:** no network pulls in tests, and no silent skipping. When podman, or the offline base image
  `docker.io/library/ubuntu:24.04` in local podman storage, is missing, the podman tests fail unless the run explicitly
  opts out, and the opt-out is documented in the validation inventory. The maintainer: "explicit skip is required if
  running w/o podman tests is intended."
- **Accepted from the planning review** (the maintainer saw these and did not object): nothing remembered between
  launches; closing a tab ends its shell but not processes it detached; Farhelm's checks before launch limited to cheap,
  certain ones; no extra gate on agent-originated requests (containerizing only removes authority, and an agent inside a
  container cannot ask for anything); podman's own init allowed; the token's presence in podman's own storage
  documented.
- **TODO:** the last code PR narrows the source entry to what remains.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md ("Local authority and trust between hosts", Lifecycle operations, Restart's
rule for sessions without a captured conversation around lines 2348 and 2509, the launcher and template sections,
agent-spawned sessions) and SPEC_impl.md (process-tree ownership and cgroup scopes around line 2924, "What running
sessions hold across versions", the protocol version rules); root `AGENTS.md`, "Code rules" (read the harness map in the
module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs` first; per-harness behavior goes where it says, never
`kind == X` or matches over harnesses in shared code; this feature is not per-harness and should need none);
`website/AGENTS.md` and `website/EDITORIAL_RULES.md` for the docs page; root `AGENTS.md`, "Sharing the machine with
other agents" (no fixed container names, ports or paths in test harnesses: derive them from the run's own random
directory), "The live install is off-limits", "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`; the
protocol version rules in the doc comment around `crates/farhelm-proto/src/lib.rs` line 180 (a new launch or template
field earns the next protocol version, 64 to 65 at the time of writing, with the pinned-version test renamed).

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The settings are one optional field on `LaunchSelection` (`crates/farhelm-proto/src/launch.rs`), serialized only when
  present so existing launch fingerprints stay byte-stable, as `wrapper_prefix` does, and the matching template field on
  `TemplateFields` (`crates/farhelm-proto/src/launcher.rs`). Both land in PR 01 under one protocol bump. The field names
  the engine (podman only) so a later engine is an additive value.
- The agent's `podman exec` goes into the launch shim's final command builder (`agent_command_with_browser` in
  `crates/farhelm-supervisor/src/launch.rs`, around line 1165), fed by a new `LaunchSpec` field; the clone, post-clone
  hook and systemd scope stay on the host as today. The shim reads the token file itself at exec time, so the token
  never passes through the launch spec. Where the container gets created and started (shim, a separate scope, or the
  supervisor) is yours, within the container criteria above.
- Tabs follow `open_tab_window` (`crates/farhelm-supervisor/src/service/core.rs`, around line 13574) with a
  `podman exec` login shell in place of the host login shell; how the tab's window records that it belongs to a
  containerized session follows the existing per-window option precedent.
- The hooks-off behaviour reuses the existing `FARHELM_AGENT_HOOKS` path (`HookPolicy`, the `launch_hooked` column, the
  notification suppression) with a per-launch override, rather than a parallel mechanism.
- The podman executable is found by absolute path, the way the scope manager finds `systemd-run`, so a login shell's
  `PATH` cannot substitute it.
- Put the changelog fragment in the first PR users can reach (the launcher PR), and amend it as later PRs land.

## Outline

1. `plan/podman-sandbox/01-container-wire`: the launch and template fields and their validation, the protocol bump, the
   helm passing them through, and the supervisor's refusals (so a containerized launch is refused cleanly until the next
   PR), with tests.
2. `plan/podman-sandbox/02-container-session`: the container's creation and lifetime, the agent's `podman exec`,
   hooks-off behaviour, stop, Delete and Replace, the isolation and teardown runtime tests, the test opt-out, and the
   SPEC and SPEC_impl sections for them.
3. `plan/podman-sandbox/03-container-tabs`: tabs inside the container, starting a stopped container, tab close, file
   links limited to the checkout in containerized sessions, their tests, and the SPEC text for both.
4. `plan/podman-sandbox/04-launcher-switch`: the launcher switch, inputs, validation, footer marker, folder-launch
   unavailability, the template editor and Save as template, with UI tests, and the changelog fragment.
5. `plan/podman-sandbox/05-docs`: the docs page, the changelog fragment's final wording, and the TODO.md narrowing.

Reshape the split if the code argues for it (for example, PR 01's refusal becomes PR 02's first check rather than a
separate step), as long as no PR adds code a later one deletes, and log the DECISION.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
the crates you touch, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares,
the proto, helm and supervisor unit tests you touch, the new podman runtime tests and the existing launch, restart, tab
and teardown end-to-end tests, all through `python3 scripts/record-test-run.py`, the UI tests for the launcher and
templates, and `python3 releasing/check-changelog.py format`. A runtime test that returned early is not evidence: read
the retained output. Update the inventory in `.agents/validation.md` and the lists in `.agents/run-all-tests.md` for the
new tests and their opt-out, and run `python3 scripts/check-agents-md.py` for that instruction change.

## Fallbacks

Examples of departures that need the scope reassessment: mounting anything into the container beyond the checkout
(including the Farhelm program, socket or state); passing more host environment than listed; any Farhelm integration
inside the container; tracking or killing individual processes inside the container beyond what podman and the existing
scopes do; a new host capability report on the wire; an image registry, `podman load` step or image cache managed by
Farhelm; network policy beyond `pasta:-T,<port>`; remembering values between launches; supporting plain folders or
Command-tab launches; CLI flags.

Decisions to log: the fields' shape and names, where the container is created and where its podman helper lives, how the
token reaches the agent and tabs without a command line, how stop and Delete confirm the container's state, the test
opt-out's mechanism, what the real podman showed when re-verifying the facts above, every test changed and why, and
every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-podman-sandbox-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory, `/tmp` and podman's local
storage, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux). It writes each sample to a
private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output, scratch, and test containers and images you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/podman-sandbox/<nn>-<short-name>`, as the
  outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- Never touch the live install (root `AGENTS.md`, "The live install is off-limits"), and never touch containers, images
  or podman storage you did not create: other agents on the machine use podman too. Name test containers from the run's
  own random directory and remove them when the test ends.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name, a verified fact that no longer holds on the
executing host) is corrected to the truth and logged.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. In particular, an isolation criterion above that cannot
be met is never quietly weakened. If no fallback covers it, record the concrete tradeoff and continue with independent
work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what cannot proceed without the
user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR narrows this plan's TODO.md entry as described. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
