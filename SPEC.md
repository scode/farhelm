# Farhelm product specification

NOTE: This is a product specification. It describes user-observable behavior and deliberately says nothing about
implementation technology (daemon language, terminal substrate, UI framework). Anything labeled a non-goal is out of
scope for v1, not forever. Sections marked "post-v1" describe behavior that must not be precluded by v1 decisions but is
not required to ship.

The problem being solved: I want to run many coding agents (Claude Code, Codex, and other terminal agents) on machines I
control — my Mac and one or more Linux hosts — and supervise all of them from one interface, interacting with each agent
through its real TUI. Existing tools in this space (herdr, superset.sh, various "agent manager" GUIs) each miss some
combination of: durable remote execution, real-terminal fidelity, VCS neutrality, or multi-host aggregation.

## Concepts

- **Host**: a machine that runs sessions. The local Mac is a host; each remote Linux machine is a host. Every host runs
  a supervisor — at most one per user, and Farhelm operates entirely within one user account per host.
- **Supervisor**: the per-host service that manages sessions on that host — launching agents, owning their terminals,
  receiving attachments, handling spawn requests — and nothing else. It has no UI and no knowledge of other hosts.
  Supervisors are the authority on their sessions, and sessions outlive supervisors (see Durability). Supervisors
  themselves outlive clients and the helm wherever the platform layer provides it — Linux in v1; on the v1 Mac the app
  hosts both, so quitting the app takes the supervisor down while its sessions keep running.
- **Helm**: the single control-plane process. The user runs exactly one helm, on whichever machine is convenient — the
  Mac or a Linux host. It holds the host registry, connects directly to each registered supervisor, aggregates their
  sessions, and serves the UI. The helm holds no authoritative session state — supervisors are the authority — but its
  registry and its last-known view of each host's sessions persist across helm restarts, so a helm bounce neither
  affects sessions nor empties the list.
- **Client**: a UI surface attached to the helm — the native Mac app's window or a browser tab on the helm's web UI.
  Killing a client, or the helm itself, never affects a session.
- **Session**: the unit of supervision. A session has a working directory, an agent invocation, a title, and a live
  terminal. A session is agent-centric: it has one main agent terminal, plus optional additional terminal tabs (plain
  shells) that open in the same working directory. Session metadata — title, parent reference, stop annotations,
  captured conversation identity — is durable and lives with the session's supervisor, so it survives helm loss and
  re-registration; terminal contents live only as long as the host-side terminal does (see Terminal experience).
- **Agent profile**: a named definition of how to run an agent. Stored profiles are user-editable; release-owned
  built-ins are read-only. Its fields: the launch invocation (command line including arguments, e.g. `claude`,
  `claude --dangerously-skip-permissions`, `codex`); an optional resume invocation, a template that may reference the
  captured conversation identity (e.g. `claude --resume {conversation}`); and optional agent-specific integrations
  (status heuristics, conversation-identity capture — see Status and Durability). Both invocations may reference the
  session's working directory as `{cwd}`, for launchers that take the directory as an argument. The user controls stored
  invocations completely; the integrations are the only per-agent machinery Farhelm itself carries. Profile edits are
  last-write-wins: two clients editing the same profile at once is not a case Farhelm guards, because it is one user's
  rare action, and no optimistic-concurrency check on profile writes is wanted.

## Topology

One control plane, two ways to face it:

1. **Native Mac app**: `farhelm-desktop`, a bare binary that opens the UI in a native window and starts the helm and a
   local supervisor beside it, so the Mac itself is a host. This is the setup when the Mac is your main machine. It
   bundles no tmux: it requires Homebrew's (at or above the version floor SPEC_impl.md's terminal-substrate section
   defines), finds it in the Homebrew prefixes itself because GUI apps do not inherit the shell `PATH`, and refuses to
   start — naming the binary, the version found, and the floor — when none acceptable exists; `FARHELM_TMUX` overrides
   the choice.
2. **Web interface**: the helm — wherever it runs — always serves a browser UI with the same capabilities, so a helm
   running on a Linux host is fully usable with nothing installed on the client machine. The native app's embedded helm
   serves the web UI too.

The two client forms have the same capabilities: terminal, attachments, lifecycle operations, host registration, and
profile management in the helm-owned catalog. They differ only in packaging. These are the only two faces the helm has —
a web UI, or the local app embedding it. There is no remote native-app-to-helm mode: a helm running on a Linux host is
reached through its web UI, period.

The Mac is not architecturally special: it runs a normal supervisor that any helm — the app's embedded one, or one on a
Linux host — can register and drive. The supervisor is a plain command-line process on every platform:
`farhelm supervisor run` in a terminal and you have a working host — that path always exists, on any platform, and is
how you try Farhelm without a bunch of fuss. System integration is layered on top of that, not baked in. On Linux, v1
ships that layer as user-level systemd units (auto-start at boot, which the durability promises assume) — never system
units, never root. On the Mac, v1 ships no system integration at all: the supervisor runs while the native app runs, or
the user starts it manually. Crude, and deliberately so — a Linux-helm setup can drive Mac agents today, and better Mac
availability is packaging work for later, not an architecture change. In that setup, "run the supervisor" means the
manual binary: the native app has no helmless mode in v1, so launching it alongside a Linux helm would start a second
helm — the unsupported state.

Remote provisioning: the helm can set up a remote supervisor itself, in the style of `herdr --remote`. There is at most
one supervisor per user per host, and adding a host is discovery-first: the helm connects over SSH and looks for that
user's supervisor. If one is already running — say one the user started interactively by hand — the helm uses it as-is;
it never restarts or replaces a running supervisor. If none exists, the user is asked whether to set one up
automatically, and on confirmation one action installs the supervisor binary, sets up the per-user systemd layer, and
registers the host — no separate host-side setup. Passwordless SSH is the prerequisite on the HOST side, plus, on the
helm's own machine, access to the configured release source (GitHub by default) or a staged payload directory. With that
in place, provisioning and everyday operation just work out of the box — reaching supervisors needs no port forwards, no
opened firewall ports, and no address configuration beyond the SSH destination. (The web UI's own loopback-plus-forward
story is separate; see Security.) Downloads go directly to that configured release-asset source — GitHub by default;
neither GitHub nor a configured mirror is a relay or rendezvous service for Farhelm sessions or connections, and the
no-relay guarantee below still holds. Nothing the supervisor does requires root: install, updates, and operation all
happen as the SSH user (user-level systemd, files in user-owned directories). If some optional step cannot be done
without privileges on a given host, provisioning says so and continues without it rather than escalating. Before
touching the host for initial setup, the helm states exactly what it is about to do in concrete terms — the files it
will place and where, the systemd units it will create, and that the supervisor will run persistently and start at boot
— and proceeds only on confirmation. Remote updates use the same one-use plan mechanism behind the same authority, but
the user's Update click is the authorization: no plan is shown for confirmation. While a host is updating, that host's
row alone expands to follow the run's progress; success folds it back unless global details are on, while failure or an
uncertain outcome stays expanded. V1 provisioning targets any Linux host with a usable systemd user manager, on the two
architectures cross-compiled supervisor binaries exist for. The distribution is not a requirement — nothing provisioning
does is distribution-specific — so the plan names whichever one it found rather than refusing; CI exercises Ubuntu.
Everything else — no usable systemd user manager, or an architecture with no payload — falls back to the manual path
(run the binary yourself), which always remains available.

Provisioning is idempotent and doubles as recovery: re-running it against an already-provisioned host — including from a
brand-new helm whose registry was lost — detects the existing supervisor and re-registers the host with all its sessions
intact. Losing the helm never strands a provisioned host.

Connections are direct: the helm connects straight to each registered supervisor, over the user's own SSH access —
passwordless SSH from the helm's machine to the host is the requirement, and the helm handles connectivity itself,
transparently. Supervisors listen on no network port. The helm's own machine is a host without registration: its
supervisor (the desktop app's managed local one, or one running beside a Linux helm) is reached locally, no SSH-to-self
involved. The helm manages it through the same discovery-first flow as remote hosts, minus SSH: a supervisor that
answers is discovered and registered like any other, and the panel never touches the unit file that runs it. What the
panel does NOT do on the helm's own machine is install one. On Linux that job belongs to `farhelm helm setup`, which
writes and owns the helm's and the supervisor's systemd user units; the local row says so instead of offering to
install, so a machine has exactly one thing writing its units. Until a supervisor exists the local host appears with
that instruction, not as a phantom unreachable host. There is no relay, no supervisor-to-supervisor connection, and no
transitive aggregation — the helm sees exactly the hosts in its registry. The machine running the helm must therefore be
able to reach every registered supervisor over the user's network fabric (Tailscale, SSH tunnel); a browser only needs
to reach the helm — which in v1 means loopback on the helm's machine, tunneled when the browser is elsewhere (see
Security).

The host registry belongs to the helm: each entry is a host's SSH destination. A host can carry an optional alias, shown
in place of its destination everywhere except the host details view, which keeps the real destination visible; aliases
are unique among host display names, and the local host can carry one too. Hosts carry stable identifiers independent of
address: a host's identifier is generated by its supervisor at install time, so it survives address changes, while
wiping and reinstalling a supervisor produces a new host identity whose predecessor's sessions are gone with the old
install. Removing a host from the registry merely forgets it — the supervisor and its sessions are untouched and
reappear on re-registration. Registry entries are editable: an SSH destination can be corrected without touching the
host's identity or its sessions. If a destination turns out to present a different identity than recorded (a wiped and
reinstalled host, a recycled address), the helm says so and asks whether to adopt the new host or fix the destination —
it never silently merges. Two destinations reaching the same identity are the same host, shown once. Last-known sessions
of a host that is permanently gone are disposed of by removing the host from the registry.

Exactly one helm runs at a time. Running several concurrently is unsupported in v1. The invariant supervisors enforce is
at most one attachment per session, last attach wins — so a second helm cannot corrupt a session, but it can seize one,
exactly like any other client taking control. "Exactly one" is an operating assumption, not something enforced; that
invariant is the backstop.

The helm is a plain command-line process too, with the same layering as supervisors: on Linux, v1 ships user-level
systemd units for it, so a reboot of the helm's machine brings the web UI back; on the Mac, the helm lives and dies with
the app.

Agent profiles belong to the helm: one catalog applies to every host the helm manages, while the invocation still has to
exist on the host that runs it. Every release supplies read-only built-in Claude Code, Codex, Muse, and Cursor profiles,
each in a plain and a permission-skipping ("yolo") variant: `claude`, `claude-yolo`, `codex`, `codex-yolo`, `muse`,
`muse-yolo`, `cursor`, and `cursor-yolo`. They appear beside the user's stored, editable definitions and are identified
as Built-in; historical stored starter rows remain editable and deletable. Integrations are not user-authored — a
profile optionally names an agent kind from Farhelm's built-in v1 catalog (Claude Code, Codex), which selects that
kind's status heuristics and conversation-identity capture; profiles without a kind get generic treatment.

Cursor is a structured harness with `cursor` and `cursor-yolo` built-in profiles invoking `agent` and `agent --force`.
Its model is optional, with `auto`, `composer-2.5` and literal custom IDs supported. Default permissions add no flag;
YOLO preserves explicit Cursor denies. There is no separate effort selector. Cursor uses generic activity status and has
no conversation tracking, automatic Resume, configuration editing, hooks or instruction injection. The launcher states
that tracking and Resume are unsupported. Restart starts fresh; history and clone preserve launch intent. See
[Cursor](docs/harnesses/cursor.md).

Muse support uses `muse` and `muse --yolo` with generic activity status. The yolo variant skips approval prompts and
sandboxing and trusts the workspace for the run. Muse-specific hooks, conversation capture/resume, and waiting-state
recognition are not implemented; no Muse integration kind is implied by the presence of its built-in profiles.
Structured Muse launches also offer a workspace-trust choice separate from tool permissions: true adds
`--trust-workspace` for that launch; false adds no trust flag. False does not undo trust already implied by `--yolo` or
vendor configuration, so a prompt is possible only when neither has granted trust.

OpenCode is a structured harness, not a built-in profile. Its Zen model is required: Farhelm suggests
`opencode/glm-5.3-flash`, `opencode/grok-4.5`, `opencode/grok-4.6`, `opencode/glm-5.3`, `opencode/gpt-6-luna`,
`opencode/gpt-5.6-terra`, `opencode/gpt-6-sol`, and `opencode/gpt-6-astra`. The custom-model field also accepts a bare
Zen model name or an `opencode/<model>` value. A bare value is passed as `opencode/<model>`; another provider prefix is
refused. OpenCode has no offered effort choices. Its default permission mode adds no flag, and YOLO uses OpenCode's
`--auto`, which auto-approves only permissions not explicitly denied. OpenCode uses generic activity status with no
hooks, conversation capture/resume, or waiting-state recognition.

Goose, Pi, and OMP are structured OpenRouter harnesses, not built-in profiles. All three require an explicit model and
suggest `z-ai/glm-5.3-flash`, `x-ai/grok-4.5`, `x-ai/grok-4.6`, `z-ai/glm-5.3`, `openai/gpt-6-luna`,
`openai/gpt-5.6-terra`, `openai/gpt-6-sol`, and `openai/gpt-6-astra`; a literal custom OpenRouter id remains available
after selecting a harness. Goose requests `off`, `low`, `medium`, `high`, or `max` thinking and offers `approve`,
`smart approve`, `chat`, and `yolo` modes. Pi requests `off`, `minimal`, `low`, `medium`, `high`, `xhigh`, or `max`
thinking and has only the visibly labelled YOLO mode; this describes the absence of Pi's built-in tool gate, not its
project-resource `--approve` flag. Pi stores that mode as `yolo`; an older snapshot that omitted the formerly optional
permission field reads and displays as YOLO too. OMP requests `off`, `minimal`, `low`, `medium`, `high`, `xhigh`, or
`max` thinking (OMP's `auto` level is not offered) and offers `default` (which adds no flag), `approve`
(`--approval-mode always-ask`), and `yolo` (`--approval-mode yolo`); its `write` mode is not offered, and
`smart approve`/`chat` are refused. An omitted OMP permission stays omitted in both argv and stored selection — unlike
Pi, no default is rewritten onto it, and the session row displays exactly that absence. OMP gets no waiting-state
recognition: an OMP approval prompt shows the generic running/idle status, a settled scope decision rather than a
detection gap. Provider capabilities may clamp or reject a requested effort.

Standard operation must never require falling back to SSH or a separate command line, with four v1 carve-outs:
transport, web-token bootstrap, bringing up the helm's own machine, and starting the v1 Mac supervisor by hand when a
Linux helm drives Mac agents. Reaching a remote helm's web UI takes a user-managed SSH port forward, and obtaining or
rotating that UI's token happens on the helm's machine (see Security) — accepted v1 friction, deliberately outside
"standard operation". Bringing up a Linux helm machine is `farhelm helm setup`, run once there: that machine's systemd
units are written by one owner rather than by whichever surface got there first, which is why the hosts panel refers to
it instead of installing a supervisor locally (see Topology). On provisionable hosts, install and updates are the helm's
job (over the user's SSH access); what may legitimately require manual host-side work is that same transport and token
pair, on hosts provisioning does not cover. Everything else — session operations, profile management, directory browsing
— must work from the client. SSH otherwise remains an escape hatch, never a requirement.

## Install and uninstall

[docs/install_uninstall.md](docs/install_uninstall.md) must document the current installation, update and uninstall
process in user-facing terms and stay accurate as that behavior changes. Assume readers understand filesystems, macOS
and Linux; minimize implementation detail. Explain the commands, installed and retained files, service behavior,
operator prerequisites, ownership and deletion safeguards, their limits, and how to handle refusals or partial removal.
Identify each installation ownership record by its exact path and explain what it contains before relying on the term
"ownership checks." Distinguish operator shutdown prerequisites from checks actually enforced by the command. Changes to
those user-visible behaviors must update the document in the same change. Installation instructions must link to it so
users can find both the removal procedure and its safety guarantees.

### Installation and updates

The standalone installer installs Farhelm for the current user without root. It places `farhelm` in `~/.local/bin` by
default; `FARHELM_INSTALL_DIR` selects another directory. On supported macOS releases it also installs `farhelm-desktop`
beside the CLI and assembles `~/Applications/Farhelm.app`, including copies of both executables. The existing app-bundle
opt-out remains supported. Installation does not create or start services: Linux service setup is a separate, explicit
`farhelm helm setup` operation, and macOS runs through the desktop app or manually started processes as described in
[Topology](#topology).

Re-running the installer updates the installation. Its default is the latest stable release; `FARHELM_VERSION` selects a
specific release, including a prerelease. Updating the software preserves user data. Automatic updates are outside this
initial uninstall scope, as are package-manager installations and changes to the packaging layout.

An installation must have an equally discoverable removal path. The installation instructions document
`farhelm uninstall` alongside installation, and a successful installer run prints that command. Users of releases
without uninstall support may need to upgrade once before using it. Supporting those older binaries directly, or
providing a separately downloaded uninstaller, is not required initially.

### Uninstall scope and interaction

`farhelm uninstall` removes the selected local standalone installation for the current user. It supports the installer's
custom installation directory as well as its default. It must identify the installation being removed rather than assume
that whichever files happen to be in the default directory are the intended targets. Ambiguous ownership or an
unsupported installation must produce an actionable refusal before changes begin.

The command shows the files and services it intends to remove and the data it will retain, then asks for confirmation.
`--yes` skips that confirmation, not ownership checks. Without an interactive confirmation channel, the command requires
`--yes` rather than proceeding implicitly. `--dry-run` reports the proposed actions and any blockers without changing
files, stopping processes, or disabling services.

Removal covers the installed CLI and desktop executable where present, the recognized installer-created macOS app
bundle, and Linux user services owned by `farhelm helm setup` for this installation. Recognize services through the
existing setup marker and the executable recorded in their unit files, and reuse setup's service-removal behavior. Those
services are stopped and disabled before their executables are removed. Operator-authored services and service drop-ins
are not deleted; retained integration files are reported. The user must stop services outside Farhelm's removal
authority. Basic uninstall does not analyze effective service overrides or inspect their processes.

Persistent user data is retained, including session history, attachments, credentials, the host registry, preferences,
logs, and cached payloads. Completion names the retained data locations; retaining data must not be presented as erasing
all traces of Farhelm. There is no `--purge` in this first version. Project directories, agent harness installations and
their own data, independently installed dependencies, and unrelated files are untouched. Uninstall does not remove
shared parent directories or change their permissions.

The command affects only the selected local installation. It does not contact registered hosts, uninstall remotely
provisioned supervisors, or remove a separate provisioning-owned local installation. Removing a helm does not stop
sessions on its remote hosts.

### Operator prerequisites and failure behavior

Before uninstalling, the user must stop local sessions and their additional terminals, quit the desktop app, and stop
manually started Farhelm processes. Sessions may survive a supervisor exit, so quitting the app or supervisor alone is
not enough. A one-time stop/restart when upgrading to the first uninstall-capable release is acceptable.

Basic uninstall operates on files. It does not walk processes, discover runtime sockets, reconstruct session ownership,
or introduce a runtime registry. It neither forcibly terminates agents nor claims to prove that all processes have
stopped. The command explains the stopping prerequisite before removal, including with `--yes`.

Refusals identify the actual filesystem check, the path and observed result or concrete error. A file's existence must
never be described as proof that a process is running. Diagnostics appear in ordinary output without requiring verbose
mode and contain enough evidence to investigate a suspected false positive.

Ownership checks precede removal. A foreign file or bundle must not be deleted merely because its name matches an
expected artifact, and symlinks must not redirect removal into unrelated files or directories. Missing artifacts are
acceptable when the remaining installation can still be identified safely.

Removal can fail partway through. Failures return a nonzero status, distinguish completed actions from remaining work,
and explain how to retry. Completed removal steps need not be rolled back, but retry must tolerate them and continue
cleanup safely. The CLI remains available until the other required removal steps succeed so an ordinary partial failure
does not remove the user's retry command. Successful removal does not promise that the now-removed command remains
invocable.

Initial support assumes installation, updates, setup, desktop startup, and session creation do not run concurrently with
uninstall. The user must keep those operations stopped until uninstall finishes. Coordination that closes races between
checks and removal is deferred in TODO.md; this version does not claim safety under concurrent lifecycle operations.

### Acceptance coverage

Automated tests must exercise the actual installer and installed uninstall command through fresh installation and update
followed by removal. They must verify custom paths, dry-run and confirmation behavior, retained data and unrelated
files, foreign artifacts and symlinks, the stopping prerequisite, and partial-failure retries. Linux coverage must
establish the service ownership and removal behavior. Concurrent lifecycle operations are outside this initial
acceptance scope.

Filesystem-refusal tests must verify the diagnostic's evidence against the fixture that caused the refusal. An assertion
that output merely says "unsafe" or names an artifact does not establish this contract.

macOS file behavior must be tested on a native macOS runner; a simulated macOS layout on Linux alone is insufficient.
Bundle construction and removal can be tested headlessly without rendering the UI. These focused checks must pass before
shipping uninstall support.

## Sessions

### Creation

Session creation is one action, not a wizard. Choose an existing directory or explicitly request a fresh GitHub
checkout; the agent choice is independent of that destination:

- Working directory: an existing directory on the target host, named by an absolute path (a relative path would resolve
  against the supervisor process rather than the client, and would shift meaning across supervisor restarts). `~` and
  `~/path` are also accepted and resolve against the home of the user running the supervisor on the target host —
  concretely, the supervisor's own `HOME` environment, resolved once at supervisor start; a supervisor with no usable
  `HOME` refuses `~` with an error naming that, rather than guessing at an account database. The expansion happens once,
  at creation, and the session stores the expanded absolute path, so the anti-drift property above is preserved; `~user`
  forms are refused. The create form prefills the field with `~`, so the common home-directory create needs no typing;
  because expansion is host-side, the same default is correct for every target host. A directory picker/completer
  against the target host's filesystem is provided.
- Title: optional; auto-generated when omitted. Renameable later. A title is a single-line label, so a title you supply
  is refused if it contains control characters (escape sequences, newlines, tabs); an auto-generated one has any such
  character replaced with U+FFFD rather than being refused, since the directory it comes from is legitimate and you did
  not choose the label. A supplied title is bounded in size too, and by the same rule for both verbs: the text you send
  on creation — working directory, invocation, title, and any invocation override — must fit in 64 KiB between them, and
  a rename's title alone is held to that same bound. Renaming has no conflict detection: two renames of one session both
  succeed, and the later write is the title that sticks.
- Launch composer: New opens a dialog with no selected harness. Structured Codex, Claude, Muse, Cursor, Goose, Pi,
  OpenCode, and OMP launches carry a harness plus model, effort, permission, and workspace-trust choices where that
  harness supports them; visible permission vocabulary is `default`, `approve`, `smart approve`, `chat`, and `yolo`.
  Absent optional choices mean the selected harness's defaults and omit their flags, except an omitted Pi permission
  means its mandatory YOLO mode. OpenCode, Goose, Pi, and OMP require a model; OpenCode and Cursor offer no effort
  choice. The helm owns the released model catalog and validates every structured choice, so the browser never turns a
  model identifier into an argv fragment. A known model identifies its owning harness; a custom model needs an explicit
  harness. A shared known model retains a selected owning harness, while an unselected ambiguous id asks for one.
  Replacing a harness clears only choices that are incompatible with it. An invalid combination cannot launch. New
  normally preselects no harness or model. The permissions mode remembers the last successful structured launch,
  helm-wide across every client; an explicit workspace-trust choice on Muse or Pi is remembered separately after a
  successful user launch. `trust:true` and `trust:false` are single search actions on those harnesses. Muse true uses
  `--trust-workspace`; Pi true and false use `--approve` and `--no-approve` respectively. The choice grants or declines
  whether Muse bypasses its workspace prompt or Pi approves project-local content for one launch; it never writes vendor
  trust state. Codex and Claude can still ask for directory trust; Farhelm does not silently answer their prompts.
  Codex's per-run project override needs the final resolved working directory, which a fresh checkout does not have when
  the helm compiles launch argv. Other harnesses have no supported interactive workspace-trust switch. "reset choices"
  returns both segments to their remembered values rather than to harness defaults, and a recent-setup row's own saved
  choice overrides it when used. The launch-composer search matches harnesses, `other / command`, models scoped by the
  chosen harness, effort words offered by that harness and model, supported trust actions, host and name actions,
  folders, and recent setups. `name:foo` applies the entire value as the session name. `host:foo` filters host choices,
  and `host:local` selects the helm-local host even if it has an alias. The default local host label in the GUI is
  `local (this machine)`. Accepting a result applies it and clears the box while keeping focus there. Enter on an empty
  box launches only a complete, valid selection through the ordinary Launch path; Enter on a non-empty query with no
  result never launches, and Escape closes the result list without clearing the query, so Enter after Escape does
  nothing until the box is emptied.
- Legacy agent profile or arbitrary command: `other / command` is a harness-picker choice in the same composer. It
  replaces only the model, effort, permissions, and workspace-trust controls with the profile picker and raw invocation
  field. Existing callers, profiles, and their helm-wide last-used profile behavior remain compatible, but New does not
  silently choose a remembered profile. Values from this mode cannot affect a structured request or its idempotency key.
  The helm owns the remembered profile default: remote supervisor metadata must not override an explicit user choice or
  indefinitely determine the default profile for sessions on other hosts. Choosing this mode does not make the
  structured composer preselect a harness or profile. Search in this mode ignores the retained structured draft:
  harnesses and known models remain available globally, while effort actions are absent until a structured harness is
  active. Accepting a harness, model, or recent setup activates the structured launch it names; accepting a folder keeps
  the current mode. The structured model/effort/permissions/trust summary is absent while a profile or command is
  active. See the maintainer-confirmed decisions below.
- Recent setups: the helm remembers bounded successful structured combinations and used folders per target-install
  identity. A recent row fills every saved choice and destination; clicking it never launches, and pressing Enter on a
  focused row launches the filled setup through the ordinary Launch path. A retargeted registry row cannot expose the
  replaced install's history. Folder search uses that bounded history, not a recursive filesystem walk; explicit
  browsing asks the selected supervisor for one bounded directory level. Recent-setup rows appear only when the current
  filter actually matches history; the dialog reserves no space for them when it does not.
- Host: defaults to the host of the currently open session, else the helm's own host. "The host of the currently open
  session" means the install the user was looking at, not merely its registry row id: a row retargeted or adopted onto a
  different install after the session was selected falls back to the helm's own host rather than silently aiming the
  create at the successor. The comparison is by the registry's recorded install identity, so it protects exactly the
  installs that have one: a host that has never identified itself to the registry is compared by row id alone, and its
  replacement by another identity-less install goes unnoticed — an accepted residual, since absent identities carry no
  continuity evidence in either direction.

Creation launches the agent; you type your first prompt into its terminal. Automatic initial-prompt delivery is post-v1.
The expected shape when it comes: for agents that accept an initial prompt on the command line (Claude Code and Codex
both do), substitute the prompt into the profile's invocation — no readiness detection needed. Injecting a prompt into
the terminal of an already-running agent requires reliably detecting that it is ready for input, which is the same hard
problem as status detection; that route is only for agents without the argv affordance. Either way it is an additive
change (an optional field on create/spawn), which is why v1 can skip it safely.

A custom model id is typed into the model field and applied with Enter; it needs a chosen harness. The optional session
name sits in the destination block under host and folder, visible without any disclosure. Launch and Cancel are the
first controls in the launcher, above everything else, and Launch's label names the chosen harness, host, and folder.
Project registration (associating metadata with a directory) is optional convenience and never a prerequisite.

Creation guards against accidental double submission (a double-click, a retry after a timeout): one intended create
yields one session or a clear error, never two silently. Deliberately creating several sessions with identical
parameters — same directory, same profile — is a sanctioned workflow, not a duplicate to be suppressed.

A session snapshots its profile at creation — launch and resume invocations and integration selection alike. Editing or
deleting a profile affects future sessions only; existing sessions keep working unchanged.

Failures split cleanly in two: precondition failures (nonexistent directory, unknown profile, unreachable host) fail the
create with a visible error and no session; launch failures of a session that was successfully created surface on the
session itself — **error** when the agent process could not be started at all (exec failure, command not found),
**exited** when it started and then ended, however quickly, with its exit code visible.

### Fresh GitHub checkouts

Selecting `gh:owner/repo` in the composer explicitly requests a new checkout on the selected host. It works with
structured harness choices, legacy profiles, and raw commands. Selecting a folder or editing the ordinary directory
returns to an existing-directory launch without changing the agent choice. Ordinary Clone and Replace start from the
source session's actual directory; a new checkout requires an explicit repository selection or a saved repository setup.

The helm owns a working-copy root and optional post-clone command, globally with per-host overrides. There is no default
root and no configuration GUI. The root must already exist on the target host; `~` expands there, using the supervisor's
captured home. Clearing an override restores inheritance; an empty hook override disables the inherited hook.
Configuration changes affect new attempts, not an already accepted attempt or its retries.

Before Launch becomes available, the composer shows the exact host and path. Preview creates no directory and reserves
nothing. An unnamed checkout uses the lowest available positive `repo-N` and that basename as its session title. An
explicit title keeps its printable display spelling while the path uses `repo-` followed by its lowercase ASCII slug;
runs outside letters and digits become hyphens. A title already starting with `repo-` does not repeat that prefix. An
empty slug or a component over 200 bytes is refused. Existing files, directories and symlinks all occupy a name. If
another create wins the displayed path, Launch reports the conflict and obtains a new preview; it never submits
automatically or silently chooses another directory. An explicit name remains a conflict rather than gaining a suffix.

Repository input is a GitHub owner/repository pair, not a URL, branch selector or shell fragment. The owner has 1–39
ASCII letters, digits or hyphens, starts and ends alphanumeric, and has no consecutive hyphens. The repository has 1–100
ASCII letters, digits, underscores, dots or hyphens, excluding `.` and `..`. Identity is lowercase. Clone uses the
constructed HTTPS GitHub URL, with the target user's ordinary credentials. Clone, configured hook, and agent run in that
order inside the session terminal, so progress, authentication prompts and failures are visible. A failed stage prevents
later stages and preserves partial content. A checkout is not permission to run a hook unless the maintainer configured
that hook.

Once allocation has occurred, failures retain the session and its checkout association for inspection and Delete. A
completed preparation permits ordinary restart without repeating clone or hook. Interrupted, missing, corrupt or
ambiguous preparation refuses automatic repetition. Retrying an accepted create with its original key reconciles the
original allocation and configuration, including after a helm restart or configuration edit. A transport-ambiguous reply
keeps that original request and key. Authenticated reconciliation can return the accepted result or a durable refusal
that prevents this key from allocating later. That refusal resolves even an earlier lost reply: the composer refreshes
the preview and requires another explicit submission. An ordinary conflict without that proof retains the original
request and key.

In fresh-checkout mode, the composer's folder field shows the current preview's effective path read-only, or a pending
or error state while no path is available. An ambiguous retry keeps displaying the original request's path. Browse and
recent folders remain available; `use existing folder` deliberately leaves checkout mode and restores an editable path.
Typing into the checkout path cannot turn a fresh checkout into an existing-folder launch.

An interruption after mkdir but before durable identity capture leaves ownership unestablished. Recovery retains a
visible error session and refuses to adopt or prepare the unknown directory. Explicit Delete may retire that unresolved
session and plan, with a diagnostic naming the preserved path; the directory remains untouched for manual inspection.

Ownership follows use, not the lifetime of the session that first requested the checkout. Ordinary sessions in a managed
directory or its canonical subdirectories also retain references, including references to managed ancestors. Stopped,
exited, and errored sessions still count. Delete releases its reference; only the final reference causes the recorded
checkout to move into `farhelm-archived-working-copies` under its original root, using its original basename plus a
timestamp and collision handling. This is a no-overwrite move, never recursive deletion or a cross-device copy fallback.
Unresolved move failures retain recoverable metadata. A foreign object replacing the recorded path must remain
untouched.

Repository suggestions combine successful repository launches for the same host installation with immediate Git clones
under the configured root. Discovery does not confer ownership, contact GitHub, fetch, recurse, run repository code, or
return credential-bearing origin URLs. Incomplete discovery is visible and does not prevent selecting a valid manually
entered pair. A saved repository setup remembers repository intent and agent choices: using it obtains a new preview and
creates a new checkout, rather than reopening its prior directory. Fresh creates do not populate ordinary folder history
with ephemeral paths; an explicit existing-directory launch can still do so.

Unlabelled search retains ordinary matching. Leading `name:`, `host:`, `harness:`, `model:`, `effort:`, `trust:`,
`folder:`, `recent:` and `gh:` labels offer or filter their respective single actions, case-insensitively; unknown
labels and colons inside model IDs retain ordinary meaning. Accepting a name or host action clears search without
launching. Invalid `gh:` input cannot launch a hidden existing directory. Host, installation, destination, title, agent
choice and observed configuration changes invalidate an undispatched preview. Late responses cannot restore its
authority or steal focus. An already ambiguous submission remains bound to its original request.

### Lifecycle operations

The client supports: create, open, rename, restart, clone, replace, replace with, stop, delete.

A list rename opens in a modal editor owned by the list rather than by the row-actions popup. Listing updates may
reorder, filter, or temporarily fail without moving its textarea, so its draft, selection, composition, and source
session identity stay intact. A successful rename closes that editor; a refusal leaves its draft and error visible for
correction. A complete, authoritative listing that no longer contains the source disables submission without discarding
the draft, because a filtered, truncated, failed, or stale listing is not proof that the session disappeared.

- **Stop** terminates the agent and its entire process tree — MCP servers, dev servers, and other descendants included.
  Terminal tabs keep running, and the session remains with its terminal still viewable.
- **Restart** relaunches the agent in the same working directory, resuming the session's own conversation where
  supported (see Durability for the exact promise). This is the only relaunch mechanism: the resume offered when opening
  an interrupted session is this same operation, not a separate feature. There is no fresh-restart variant in v1 — for a
  clean conversation, create a new session in the same directory. Restart on a session whose agent is still running
  confirms, stops the agent, then relaunches. Restart reuses the session's terminal when it still exists — whatever
  scrollback the terminal itself retained is still there — and creates a fresh one when the terminal no longer exists,
  such as after a reboot. Restart does NOT preserve the previous run's last visible screen: the pane is blank until the
  new agent draws, and a full-screen program's final frame (which was never in scrollback to begin with) is gone. Losing
  it is accepted. Farhelm must never capture a terminal's screen and paint it back into a relaunched terminal ahead of
  the new process — a frame with no process behind it looks live, accepts typing, and is overwritten when the real
  program draws, which is worse than blank. Restart touches the agent terminal only; terminal tabs are unaffected.
- **Clone** opens an ordinary, editable create form pre-filled from an existing session's host, working directory,
  title, and agent — the fresh-conversation counterpart to restart's resumed one. The source session is untouched:
  cloning starts a brand-new, independent create through the same form and the same confirmation described under
  Creation and identity above, so every field can be edited before submitting and the request can be cancelled like any
  other create. A structured source carries its stored declarative launch selection into the composer verbatim,
  including omitted default fields; it is never rediscovered by parsing the compiled invocation. A legacy agent carries
  over as a profile only while the source's profile is still the one it names (the same identity a session's own profile
  snapshot already tracks); otherwise the form falls back to the source's raw invocation, exactly as "the client asks
  instead of guessing" already requires for a vanished remembered default. Cloning does not deduplicate titles — a
  duplicate is allowed, the same as any other create.
- **Replace** creates a new session — new id, fresh conversation, same host, working directory, title, and agent (a
  profile while the source's profile is still the one it names, otherwise the source's raw invocation, exactly as clone
  resolves it) — and then DELETES the source. Confirmed directly from the row menu, with one inline confirmation and
  nothing to edit first, since the whole point is the same settings. Contrast restart, which keeps the session's own id
  and its conversation: restart continues a session, replace starts one over under the same settings. If the create
  fails, the source is untouched. If the create succeeds and the removal that follows fails, the reply names both
  sessions; whether the source is still there depends on how the removal failed, and the user checks or removes it by
  hand. Replace is offered wherever clone is offered.
- **Replace with** opens the same editable create form clone opens, pre-filled the same way clone pre-fills it, so every
  field can be edited before launching — the key use is starting an equivalent session on a different harness or effort.
  Launching creates the new session and then deletes the source, with exactly Replace's create-then-delete contract and
  failure reporting (the same asymmetry: an untouched source on a failed create, both ids named on a failed removal).
  Unlike clone, it keeps the source's own host — clone is the way to start a session on a different host. Offered
  wherever clone and replace are offered. Clone, replace with, and New are one launcher — same layout, same controls,
  same search, same validation — differing only in what is pre-filled when they open and in what launching does (create;
  create then delete the source).
- **Delete** removes the session and its stored state, in any state, terminating the agent and tabs if running — with
  confirmation that says so when anything is still alive. Deletion may make partial progress before failing, including
  removing attachment files while retaining the session row for retry. There is no rollback guarantee. Report the
  failure visibly and allow a later Delete to finish cleanup; a retained row does not mean previously removed state has
  been restored.

Process-tree ownership is session-wide. Restart reaps any leftover descendants of the prior run before relaunching —
never alongside them. Stop and delete reap everything the agent started. An agent exiting on its own does not trigger a
hunt for daemonized survivors; the session's next restart or its teardown does. Operations that need the working
directory — restart, opening a terminal tab — fail with a clear error naming the directory if it has vanished since
creation; the session itself remains, and delete still works.

This cleanup covers ordinary agent descendants, including accidentally daemonized processes, rather than hostile
same-account processes deliberately escaping cleanup. Detached services started by shell initialization before the agent
launches are outside the cleanup guarantee: they may serve the user's login environment beyond this session.

### Session view

For opening a terminal tab, the session's working directory is a path, not a tracked inode or a preserved symlink
destination. Use that path with normal filesystem resolution. If a symlink along it now points elsewhere, opening the
tab there is explicitly acceptable. Do not add directory-identity tracking or symlink-change refusal for this operation.
A missing or unusable directory still produces a clear error. This does not change the separate agent-restart identity
check.

Opening a session shows the agent's real TUI, live. The session view supports additional terminal tabs: plain shells
spawned in the session's working directory, for poking at the workspace next to the agent. Tabs survive client
disconnects and supervisor restarts exactly like the agent terminal, but they are not durable metadata: after a host
reboot, tabs are gone and the user re-adds them; nothing recreates them automatically. A tab can be closed individually,
which kills that shell and its processes — that is the whole per-tab operation set in v1. A tab whose process exits on
its own is reaped automatically and silently: the tab disappears as if closed, its dead pane's scrollback is discarded,
and no notice or exit code is shown. This is deliberately NOT the agent terminal's contract — an exited agent stays
viewable with its scrollback — because a tab's shell exiting is the user being done with the tab. A shell that dies
before the tab's open completes still refuses the open loudly, with the shell's last words as the error. A tab someone
has hand-split into several panes (through the session's own tmux access) counts as exited only when EVERY pane in it
has — one exited half must not condemn a shell still running beside it.

When a session's terminal contents no longer exist on a reachable host after a reboot, opening it shows the session's
metadata and says why there is no terminal, rather than an empty pane.

### Session list

One flat list across all registered hosts, with an always-visible host selector. It starts at `ALL`; `This machine`
means the registered local host, and configured remote hosts follow in registry order, including unavailable hosts whose
last-known sessions remain useful. Choosing a host sends a server-side query immediately and is not persisted. The list
can be ordered independently by most recent activity, by creation time, or by title; the order someone picks is
remembered by the helm as one preference shared by every client, together with the last-selected session and compact-row
choice, and most recent activity is what a client shows until someone picks otherwise. No client keeps its own copy:
every client reads the helm's preference once after authenticating and writes it on change, so a browser tab and the
desktop app open in the same order and on the same session. Per-client persistence — browser storage, a desktop state
file, anything that lets two clients remember different answers — is not wanted. A client that asks the helm for no
particular order gets creation time. No mandatory hierarchy. Sessions may carry an optional parent reference usable by
the API, but parentage does not nest the list and implies nothing about VCS state. Parent tracking is not comprehensive:
`farhelm spawn --parent` can record it, while `farhelm agent create` and `clone` need not record the asking session.

The option labelled most recent activity sorts connected running and waiting sessions first, then every other session —
idle, unclassified, ended, and anything on an unreachable host. Inside each group the order is the most recent observed
START of a work burst: a session promotes when it moves from known idle or waiting into running with changed output
(both idle-to-running and waiting-to-running count), and later output inside that burst leaves the position alone.
Moving between the groups is what idling and completion do — a finished session drops below still-running work without
its burst key moving, and a session returning to running rejoins the first group on that same key. Creation time is the
stable fallback where an older supervisor has no work-start observation. The last-activity time shown in the row and the
seen/unseen comparison stay independent: they describe output recency and keep advancing with output, and neither moves
a row. The grouping and the key are authoritative helm and supervisor data, so every client agrees without keeping a
private rank.

The list always carries a count of every session. The host selector is a narrowing query, so its count says how many
matched alongside how big the whole fleet is.

The list is served and rendered WHOLE. The fleet this product is for is tens of sessions across a few hosts, not
thousands, and the design assumes that scale outright: every supervisor answers a listing with its entire list in one
reply, the helm holds every host's list in memory and sorts and filters the whole fleet there, and a client receives one
array and renders all of it. Each of those replies is cut at a fixed cap of a few hundred rows, and a list the cap cut
says so in the same place the count lives — "could not read to the end" means exactly that the cap was hit, never that
something went wrong reading — rather than presenting a partial list as the whole one. No pagination, cursors, streaming
or incremental listing, or per-order server-side indexing is wanted at any layer, and it is fine for the helm and every
client to hold and sort the entire fleet in memory; a fleet that outgrows the cap is outside what this product is built
for, and the notice is the whole of the answer to it.

A row's first line shows its status (drawn as described under Status), locality mark, title, agent label, and last
activity time; status and locality marks occupy aligned columns. Compact ended rows replace the live dot with a distinct
ended-state icon, preserving complete status details in accessible text and a tooltip. Noncompact rows show the complete
ended status on a separate full-width line that wraps instead of truncating. Times share a right-aligned column before
the row menu. A session whose host cannot yet be placed either way marks neither, rather than guessing. Its second line
shows the helm-supplied host name (an alias when set), then `:`, then its working directory; a legacy row with no host
name shows only the directory, rather than inventing a local identity or a dangling separator. The second line is hidden
when the helm-wide compact preference is on, which defaults off and is shared at the next preference seed across
clients. The working directory and launch command remain abbreviated only where shown, with their full, untouched values
always available on the row (a tooltip on the web and desktop clients); an abbreviation is never the only place a value
is recorded. A row's own actions menu, beyond the lifecycle operations above, also offers a mark read / mark unread
toggle — reachable there or by clicking the dot itself — that sets the session's seen state directly (see Status).
Hovering a live status dot, agent mark, or permission mark explains that mark. A clickable dot also names its mark read
or mark unread action. The hover text uses the same status and permission meaning the row exposes to assistive
technology. Exactly how a row lays out its lines and pixels is an implementation choice, covered in SPEC_impl.md rather
than here.

Per-host connection state is always visible in the host list, which names each host and pins its current phase beside
it. The host count, its unpersisted details checkbox, and the secondary add action share one header row. Host actions
open on demand from the row menu, and details reveals the version, identity, session count, remedies, diagnostics, and
provisioning progress under every row. Profiles use secondary buttons, while the host selector stays a native control;
session creation remains the blue primary action. Sessions on an unreachable host stay in the list from the helm's
last-known knowledge (which survives helm restarts), clearly marked stale, rather than vanishing. Lifecycle operations
against an unreachable host are refused with a clear error; nothing queues for later delivery in v1. Opening such a
session shows its metadata — title, directory, last-known status — behind a clear host-unreachable notice; there is no
terminal to show and no pretense of one. Changes made from any client — creates, renames, stops, deletes, status
transitions — appear in all other connected clients automatically; the agent-spawn behavior below is one instance of
this general rule, not a special case.

### Status

Each session shows one of: **running** (agent actively working), **waiting** (a detected pending question or approval
directed at the user), **idle** (agent alive and at rest, no pending ask), **exited** (process ended), **interrupted**
(the host rebooted while the session was last known live — an explicit lost-track state; see Durability), **error** (the
agent process could not be started at all). Exited sessions show their exit code when known; an exit that happened while
the supervisor was down shows the code only when the surviving terminal genuinely retains it, and an explicit unknown
otherwise — never a guess (see Durability). A user-initiated stop yields exited with an annotation — "stopped" is not a
distinct status. Host unreachability is per-host connection state, not a session status.

An interrupted session stays interrupted until the user acts: opening it and declining resume leaves it interrupted;
restart or delete are the ways out.

How a status is DRAWN depends on how much it has to say. The three live states are a color-coded dot beside the
session's title — running pulses, waiting and idle do not — with the status word itself always present as text for
screen readers and anything else that reads rather than looks, never replaced by the color. Outside compact mode, ended
states keep their complete wording visible, including exit code, stop annotation, and launch failure details. Compact
mode uses distinct stopped, exited, interrupted, and error icons; those details remain in accessible text and tooltips
without expanding the row. Ended icons do not have the live dot's mark-read action. The pulse is a claim about the
present, so it stands down wherever the status is a last-known report rather than a live one: a session on an
unreachable host shows a still dot whatever its status says.

The dot's colour is one of four, not three: running pulses green; waiting is red, since it is the one live status that
is a request directed at a human and belongs with the other attention colours rather than beside "nothing is wrong";
idle is grey when its last output has been seen; idle is blue when it has not. Seen state is one stored fact per session
— an activity stamp recorded the last time some client looked at it, or nothing at all if no client ever has — kept by
the helm and shared by every client the same way the list order and the last-selected session are (see Session list): no
client keeps its own copy, and opening the same session from a second client sees the same colour the first one does. A
session reads UNSEEN whenever its most recent activity is newer than that recorded stamp, or nothing is recorded at all;
otherwise it reads SEEN.

The stamp is set automatically in two moments — opening a session records its current activity as seen, and that
session's activity advancing while it stays open re-records the new value — with which client did the opening, and
whether its window has focus or its tab is visible, both ignored: a session sitting in a background tab is still
recorded as seen the same way a foregrounded one is. It can also be set or cleared by hand: a "mark unread"/"mark read"
toggle, reachable from the session's row, sets or clears the stamp directly. A manual mark unread on the session
currently open STICKS — the automatic rule does not immediately re-record it as seen — until the user leaves and returns
to it, or new output arrives. A failed automatic mark is logged but silent, the same best-effort exception the
list-order/last-selection preference gets (see Errors and diagnostics); a failed manual toggle surfaces to the row like
any other operation.

Beside the status, a session shows how long ago it was last active as a short relative age (`2m`, `3h`), with the full
timestamp available on the row. The age describes output recency, not list position: the recently-active order groups by
reported status first and compares burst starts inside each group, so a row can sit above another whose age is newer.
The age is a difference between two machines' clocks and is only as good as they are, so it is never the only place the
underlying time is recorded. It is also independent of the status beside it: a session nothing has classified yet shows
no status and still shows its age.

Two cases have no age to show, and both show nothing rather than a guess. A helm predating the last-activity field sends
no stamp, and the session's creation time stands in as the displayed age. A session with neither stamp gets no age at
all, never one counted from 1970.

Running/waiting/idle discrimination for raw TUIs is inherently heuristic, and the waiting/idle boundary especially so.
The bar: best-effort observation-based heuristics (output activity, terminal state), optionally sharpened per agent
profile with agent-specific heuristics. A profile may canonically remove only an audited, tightly located redraw region
before output comparison, while retaining the raw bounded screen for approval detection; unfamiliar screens remain raw.
It may also recognize a current vendor work indicator, but a waiting prompt wins and neither heuristic creates lifecycle
state. Wrong status must be cosmetic only — status detection must never gate or delay interaction with the terminal.
Integrations that require configuring the agent itself (e.g. Claude Code hooks) may be supported later but are not part
of v1 and must never be required. OMP is a stated exception in the other direction: its integration is conversation
identity only, and Farhelm performs no OMP waiting recognition at all. An OMP approval prompt shows the generic
running/idle classification, never waiting — a settled scope decision, not a heuristic waiting to be sharpened.

Notifications (desktop or otherwise) are explicitly out of v1. The status column is the whole story.

## Terminal experience

The real TUI is the primary and, in v1, the only interaction surface. Typing goes straight to the agent's terminal;
whatever the agent renders is what you see. There is no composer, no message abstraction, no send button in v1.

- Full fidelity: colors, cursor movement, alternate screens, resize. If it works over plain SSH it must work here. The
  default palette, foreground, and background are Ghostty's defaults, and launched agents see `COLORTERM=truecolor`.
- Shift+Enter (the exact chord — no other modifier held, not mid-IME-composition) is sent as ESC CR in a SINGLE write,
  in every terminal tab alike, agent and shell. Single-write delivery is part of the promise, not an implementation
  detail: a lone ESC arriving in its own read is indistinguishable from the Escape key to line editors that disambiguate
  by read boundary or a short timeout (Codex's input stack; zsh with a small KEYTIMEOUT), which turns the chord into
  Escape-plus-submit. Delivered whole, the sequence is the newline binding Claude Code and Codex honor in place of
  submit (verified against both, 2026-08-19). Other programs receive the same bytes and interpret them per their own
  line editing — stock emacs-mode zsh inserts a newline, bash's default quietly ignores the pair — the same outcomes
  those shells give under a reference terminal that encodes the chord identically (Ghostty).
- Scrollback is whatever the host-side terminal naturally retains, and it survives client disconnects: detach, reconnect
  a day later, and the buffer is still there. There is no separate history store — when a host reboots, terminal
  contents are gone, and recovering the conversation is the agent's job (resume). A stopped or exited session's terminal
  stays viewable while its host is up, since the terminal outlives the process. Viewable means what the terminal itself
  holds: a full-screen program's last frame is not retained after it exits, and no snapshot of it is taken or stored.
- Opening a session attaches to it — and opening a CLIENT counts as opening a session: with a non-empty fleet, a freshly
  loaded client selects and attaches the session the user most recently selected from any client — the helm remembers
  one selection for all of them (see Session list) — falling back to the newest-created one (chosen from the rows the
  listing carries, so when the list was cut at its cap under an order other than creation time the pick can be the
  newest the reply reached rather than the fleet's true newest — the accepted edge of the whole-list cap), so launching
  the app is itself the deliberate act the attach semantics below key off. Opening a second client therefore attaches to
  whatever was most recently selected anywhere and takes the terminal over exactly as clicking the same session there
  would. The attached client owns input and terminal dimensions: the PTY resizes to that client, and the last size
  sticks when nothing is attached. Reconnecting replays the terminal so the session looks as it would have had the
  client stayed attached, modulo redraws caused by dimension changes. The floor: the host-side terminal retains, and
  replay covers, at least the current screen plus 10,000 lines of scrollback. The sidebar visibly marks the selected
  session's row whenever that session is listed, so which session the main pane is interacting with is readable at a
  glance rather than only from the titlebar. A filter that excludes the selected session leaves no row to mark — the
  titlebar remains the identifier in that state, and the main pane deliberately stays put (filtering the list is not
  deselecting).
- One attached client per session, enforced by the supervisor: attaching from a second client visibly detaches the
  first, which keeps a non-live snapshot and an explicit take-control action. No shared-input mirroring in v1.
- A viewer that is slow is served slowly, for as long as it takes. Honoring that can briefly slow the agent's OUTPUT — a
  paused viewer may leave the agent's writes blocking for as long as the flow-control window allows — but never
  indefinitely and never silently: the delay is bounded by that window and, in the limit, by the stall timeout below,
  after which the viewer is detached and the agent runs unimpeded. No viewer can stop a session's work outright, and no
  viewer can slow one it is not attached to.
- A viewer that stops consuming output entirely for a sustained interval (a wedged tab, a machine asleep past its
  connection's lifetime) is detached with a visible stall reason rather than honored forever — the same surface as a
  takeover detach, with reattach behaving exactly as any reconnect does. Flow control never drops terminal output:
  whatever bound it degrades to is the same replay floor above, never a silent gap.
- A terminal that loses its CONNECTION recovers by itself, without the session ever being closed and reopened by hand.
  That covers the connection dropping visibly and the connection dying silently — a sleeping laptop or a timed-out
  network path leaves a terminal that looks connected and carries nothing, which is checked for rather than left for the
  user to discover by typing. Recovery follows the same two regimes as a host connection: bounded retries, then periodic
  re-probing, so a terminal whose network comes back overnight is simply there again. Which phase it is in is visible in
  the terminal itself, along with a way to retry immediately, and a recovered terminal reattaches exactly as any client
  does — landing where the session is now, not scrolling its history past again.
- A client displaced by a takeover keeps its snapshot and its take-control action rather than reattaching: it was
  displaced on purpose, and a client that came back on its own would fight the one that displaced it. A viewer detached
  for stalling keeps its reason: the wedge is why it was detached, and returning into the same wedge repeats it. Both
  come back the way any client attaches — because someone asks.
- If Delete fails after disconnecting a viewer, automatic reconnection to a surviving terminal and remaining detached
  until the user reconnects are both explicitly acceptable. Prefer whichever is simpler to implement; neither outcome is
  a defect or a reason to add recovery machinery. Keep the cleanup failure visible. Recovery must not restart an agent
  or take control from another viewer, and retaining the session record does not guarantee that its terminal or
  scrollback survived cleanup.
- A terminal recovering on its own never TAKES the session. Recovery is unattended by definition — the client was not
  there to be told anything while its connection was gone — so if someone else has attached meanwhile, the automatic
  attach is refused and that client lands where it actually stands: displaced, with the same take-control action any
  other displaced client has. Taking a session over stays a thing someone does on purpose, whether by opening it or by
  asking for it back.
- Selecting text copies it to the system clipboard: a plain drag when the pane has no mouse reporting active, or
  Shift-drag (Option-drag on macOS) to force a local selection when it does — the same modifier xterm itself uses to win
  a selection back from an app that has grabbed the mouse. A terminal program's own OSC 52 WRITE is honored the same
  way, and is the only path that reaches the clipboard for a selection an app under mouse reporting makes for itself; an
  OSC 52 READ is never answered with clipboard contents. A user-initiated paste intentionally sends the pasted content
  to the selected terminal; it does not authorize a program to query the clipboard. Every completed selection re-copies,
  even one identical to what is already on the clipboard. Clipboard operations are explicitly best-effort and silent on
  failure — permission policy, secure-context requirements, and an engine's own clipboard behavior are outside this
  system's control — a deliberate, named exception to the Errors and diagnostics section's surface-every-error rule
  below, not a lapse in it.

## Attachments

Attachments are intended for ordinary session inputs such as screenshots and documents, not giant bulk transfers such as
50 GB uploads. This describes expected use, not a required numeric size limit. Quitting the desktop app interrupts
unfinished transfers rather than keeping the app open for them.

Pasting or dropping content into any of a session's terminals — the agent's or a tab's — is classified by flavor, in
precedence order: file references first, then image data, then plain text. A file reference means an actual file object
on the clipboard or in a drag; pasted text that merely looks like a path is still text. Files and images are intercepted
— the client transfers the file to the session's host and inserts the resulting host-side path into the terminal input
at the cursor, so the agent picks it up with no manual copying. Plain text passes through as ordinary terminal input.
Dropped directories are rejected with a visible error in v1. Interception is unconditional for files and images — remote
and local sessions alike, regardless of any native paste handling the agent would have had in a plain local terminal.

Files land in a per-session attachments directory under the supervisor's own data area, never in the working directory —
dropping untracked files into a workspace would be exactly the kind of implicit mutation this system promises not to
make. Attachment files are removed as part of deleting their session. An explicitly requested Delete may remove them
before a later step fails and leaves the session row for retry; this partial deletion is acceptable, with a visible
failure and no rollback guarantee. Stop does not gain permission to remove attachment files from this rule.

Cancellation or disconnection during final publication, including desktop Quit, may leave a complete attachment even
though the client receives no acknowledged path. Stopping the wait does not roll back publication. Such a file is an
ordinary attachment: it is retained until session deletion, not removed on startup or Stop. Retrying may create an
additional copy under a different name. Before publication starts, ordinary staging cleanup still applies; partial files
must never become published attachments. A failure response must distinguish a definitely unpublished upload from one
whose publication outcome is unknown.

Attachment bytes ride the existing edges — client to helm, helm to supervisor. There is no direct client-to-supervisor
path; a browser never needs to reach any machine but the helm's.

Transfer must not block the terminal: you can keep typing while it runs, and the path is inserted at whatever cursor
position is current when the transfer completes. For a typical screenshot this is imperceptible.

Upload failures must be visible; an attachment must never disappear silently.

## Desktop window chrome

The macOS desktop window integrates its native title bar with the app header: native traffic-light controls sit in the
sidebar's top row beside Profiles and the version readout, with no separate visible app-title strip. The session header
and terminal tabs continue the app's surface to the top edge. Empty header space provides window dragging, and
double-clicking it zooms the window (repeating the gesture restores the prior frame); a single click without movement
never zooms. Controls and terminal text retain their own interactions. Browser and Linux window layouts retain their
existing appearance and gain no drag or zoom behavior. In narrow macOS windows, the app-level row stays fixed above both
scrolling panes so horizontal scrolling cannot move application controls underneath native window buttons. Startup,
authentication errors, and build-mismatch notices also keep their content clear of native controls.

## Durability and resume

The runtime guarantees below do not establish support for every historical data schema or a downgrade path between
arbitrary versions. Upgrade compatibility is decided with the maintainer per feature, as specified in the
maintainer-confirmed decisions below; agents must surface potential data/state loss and missing downgrade paths before
proceeding with such changes.

Sessions depend on exactly one thing staying up: their host. Every other component is disposable:

- Clients and the helm can close, crash, or restart freely — closing the app, network loss, Mac sleep, a helm upgrade —
  and every session keeps running, local and remote.
- The supervisor itself restarting — crash, upgrade, manual restart — must not interrupt its sessions. Terminals and
  agent processes outlive the supervisor process; only a host reboot takes sessions down. This is what makes
  user-controlled updates routine instead of scary.

Local sessions have full behavioral parity with remote ones; what differs in v1 is availability, not behavior. The Mac's
supervisor runs while the app runs (or when started manually — see Topology), and per the rule above, its sessions keep
running while the supervisor is down and reattach when it returns. Sessions persist across app restarts.

The environment contract: a session process behaves as if the user had SSHed into the host and typed the command in
their interactive shell — PATH, rc-file variables, locale included — even though the supervisor starts at boot. That
SSH-and-type test is the contract when shell sourcing subtleties (login vs. non-login, `.profile` vs. `.bashrc`) would
otherwise leave room for argument. A bare `claude` in a profile must work exactly as it does from the user's own shell;
"command not found because a daemon launched it" is a bug, not a caveat. The environment is evaluated at each launch:
edit your rc files and the next launch or restart sees the change; already-running sessions do not.

When a host reboots, its supervisor starts automatically on hosts with the system-integration layer; on the v1 Mac it
returns when the app or binary is next started, and interruption is classified at that point — whenever the supervisor
comes back. After a boot, sessions last known running show as **interrupted** — explicitly a lost-track state, not a
claim about what happened in between: the agent may have exited on its own moments before the reboot, the supervisor
cannot know, and interrupted says exactly that. Sessions already known exited (including user-stopped ones) keep their
status; an exit during supervisor downtime with no reboot involved shows as exited — with the true exit code when the
surviving terminal still holds it, unknown code otherwise (reporting a code the terminal genuinely retains is not
guessing; inventing one where nothing retains it would be). Interrupted sessions' terminal contents are gone — there is
no history store (see Terminal experience) — but the conversation itself is recoverable. Opening an interrupted session
offers restart-with-resume. Nothing respawns unattended — an agent (especially one launched with permissive flags) only
restarts when the user opens the session and confirms. The system must not presume the original OS process survived the
reboot.

The resume promise is per-session: for agents with conversation-identity integration, the supervisor captures which
agent conversation belongs to each session, and restart resumes exactly that conversation (e.g.
`claude --resume <conversation-id>`) — even when several sessions share a working directory. Claude Code, Codex, Goose,
and Pi integrations at this level are required in v1. Identity is reported by the agent itself when its kind supports a
launch reporter, and scanned from the outside — the agent's terminal, its own on-disk session records — otherwise; a
report wins over a scan, because it is the agent's own answer rather than a correlation over what the agent happened to
leave on disk. What capture never does is write to the agent's own configuration or record directories. A hook passed on
the command line for one launch is allowed because it writes nothing the vendor owns — no configuration file, no
conversation record, no trust state — and cannot outlive the launch that carried it. It is not invisible in the
absolute: the report it delivers lands in farhelm's own database, and every run leaves a line in farhelm's own hook log.
Vendor-owned state is the boundary the no-agent-configuration rule from Status is protecting, and that rule's own
example — hooks written into the agent's configuration — still stands. Claude retains scanning as its fallback when no
report has been accepted. Codex, Goose, Pi, and OMP are report-only integrations: Farhelm never selects their
conversation by scanning vendor state. A reporting credential alone does not establish which Codex conversation is in
the foreground. Goose persists a credential-free named MCP reporter with the conversation and reuses it on resume; Pi
loads a private static extension from Farhelm's state directory on every launch. A Pi report without a session file
withdraws the old resume target. Before a Pi resume, Farhelm reads the bounded first record of that exact file without
following symlinks and requires its session ID to match. A failed check changes the durable offer to fresh and rejects
the stale Resume request so the user can refresh; it never silently launches fresh under that request.

Codex reports must come from the foreground native Codex process under the session's owned pane, not a nested Codex
process that inherited its credential. Farhelm also verifies the exact reported transcript's root-session metadata;
process ancestry alone cannot distinguish threads sharing a process. The durable locator keeps runtime session identity
separate from persistent thread identity, and resume uses the latter. Custom Codex homes work through the exact reported
path; Farhelm does not search another home or choose a newer file. A legitimate reported `/clear` switches the current
identity even when its transcript is not yet persisted: the old conversation stops being offered, and only the new
conversation's exact file may make it resumable. An unrelated rejected report leaves the foreground identity untouched.
Compaction preserves the conversation, and a verified new conversation replaces it. Unverifiable historical bare IDs
remain stored but are not offered as exact resume targets. Missing or changed transcript evidence refuses Resume rather
than silently launching fresh or selecting a different historical conversation.

Every conversation-identity report carries a closed vendor discriminator naming the adapter that produced it — the
injected hook command, the Goose helper, or a shipped asset — and a report addressed to a session of another kind is
refused before any vendor state is consulted. The discriminator routes; it does not prove. Codex admission requires
foreground and record proofs, and its exact resume additionally requires versioned proof that the binding was admitted
under those proofs, with the historical exception described in SPEC_impl.md. OMP, Goose, Claude, and Pi retain their
existing admission and resume rules; the discriminator alone adds no foreground protection. Old senders that predate the
discriminator fail closed rather than reporting untagged. A refused report changes nothing: no stored identity, no
offer, no ambiguity verdict, no pending state. Resume is never silently turned into fresh, and historical captures are
never rewritten to look proven.

OMP (the `omp` program, the `@oh-my-pi/pi-coding-agent` CLI) is another report-only integration beside Pi. A launch
whose program is `omp` gets Farhelm's private extension when the invocation is an interactive-shaped launch; utility
subcommands, print/mode/export/alias/help/version/license/list-models occurrences, the reserved-word rejecting forms,
internal worker selectors, `--trusted-extension` launches (which OMP refuses to combine with an injected `-e`), and a
genuine end-of-options `--` are left without the extension — runnable exactly as written. A genuine `--` additionally
refuses the CREATE when the derived OMP resume template would be appended behind it (the appended `--resume` would land
in prompt position); a `--` consumed as an option value or an explicit resume template creates normally. An OMP report
carries the same durable locator shape under an `omp:` prefix instead of Pi's `pi:`, and the two are never
interchangeable: an OMP locator is never accepted for a Pi session or the reverse, and neither passes as a plain
conversation id for the other kinds. Before an OMP resume, Farhelm reads a bounded prefix of the reported file without
following symlinks, skips at most one leading shape-checked title slot (a fixed-width 256-byte record OMP rewrites in
place), and requires the next record to be the session header at `version: 3` carrying the reported id — anything else
refuses. The refusal fails closed the same way Pi's does: the durable offer becomes fresh and the stale Resume request
is rejected, never silently launched fresh. A session header with no title slot cannot be told apart from a Pi-shaped
file by its bytes, so OMP's vendor isolation lives at the locator and report boundary, not in file bytes. Two OMP
limitations are stated rather than smoothed over. OMP can move an active conversation's file without any event Farhelm
subscribes to, so an immediate exit after such a move can leave a stale locator until the next subscribed event —
pre-resume verification is what keeps that offer from resuming an absent file. And a conversation stored somewhere other
than a session file, or compressed into a `.jsonl.gz` archive, has nothing Farhelm can verify, so its resume offer
withdraws — fail closed, not a silent fresh start. One OMP difference works in the user's favor: OMP 18.2.4 persists a
new conversation eagerly, so after `/new` the fresh conversation can be resumable at once instead of waiting for a first
assistant message.

When an integrated session has no explicit resume invocation, its resume invocation is derived from the original launch
argv retained for that session: Claude appends `--resume <conversation-id>`, and Codex appends
`resume <conversation-id>`, Goose uses `session --resume --session-id <conversation-id>`, Pi uses
`--session <verified-absolute-file>`, and OMP uses `--resume <verified-absolute-file>`. For Pi and OMP, `{conversation}`
in a resume template means that verified file path, not Farhelm's internal durable locator; for Codex it means the
verified persistent thread ID, not the runtime session ID or encoded locator. The original argv is reused as-is,
including permission and configuration arguments, and is preserved as argv elements rather than rejoined shell text —
except that OMP's own session selectors are stripped from the retained argv first, so an old resume or fork target
cannot survive between the user and the verified one. This immediate rule assumes every original argument is reusable
and that the launch has no initial prompt or launch-only option; separating those concerns into common, launch, and
resume arguments is deferred.

Anything farhelm attaches to an agent launch must be invisible from inside the session when it works AND when it fails:
no output on the agent's terminal, no non-zero exit, no error the agent's own UI can show. A hook that cannot do its job
gives up silently within a bounded time and leaves its diagnostics in farhelm's own state directory, never in the user's
session. The bound is on the part the agent waits for — reading the vendor's payload and reporting the result — because
that is the whole of what can hold the agent up; writing the diagnostic happens afterwards, is best-effort, and is not
itself bounded. One accepted exception to the invisibility rule is a line the vendor itself prints because of a flag we
pass (Codex's hook-trust warning), which must be documented.

The other exception is farhelm's own, and it is deliberate rather than tolerated: on a launch that gets the hook, the
hook prints exactly one line for the AGENT to read — that `$farhelm <request>` in the user's message means "use the
`farhelm agent` CLI", and that `farhelm agent instructions` explains the rest. Nothing reaches the user's terminal and
nothing is written to disk. It is on by default, because an agent that has never heard of the CLI will not go looking
for it, and `FARHELM_AGENT_INSTRUCTIONS=off` in the supervisor's environment removes it while leaving identity capture
exactly as it was. A launch with no hook has no pointer either, for the plain reason that there is nothing to print it.
The instructions themselves are printed only when that command is run, so a session where the user never mentions
farhelm pays one line and nothing more.

For agents without integration, restart falls back to the profile's resume invocation verbatim apart from placeholder
substitution (which may land in the agent's own picker or most-recent-conversation behavior), or a fresh launch when the
profile defines none. If a supported agent's conversation identity was never captured for a session, restart says so and
offers that same fallback or a fresh launch — it must never silently resume the wrong conversation. A resume invocation
referencing `{conversation}` is never run with the placeholder unfilled: no captured identity means restart offers a
fresh launch and says why, not a garbled command line. `{cwd}` is always filled where it stands as a whole argument, on
every launch and restart; there is no launch without a working directory.

## VCS neutrality

Ordinary session operation is version-control-agnostic. Explicit fresh-checkout creation is the bounded exception:

- Sessions launch in any existing directory: detached HEAD, no `.git`, colocated or pure `jj` workspaces, nested
  repositories, and plain directories all work identically.
- No requirement of branches, one-branch-per-session, Git worktrees, default-branch workflows, or any PR topology.
  Stacked changes work because the control plane stays out of the way, not because it models them.
- The system never performs VCS mutations implicitly. Repository state is owned by the agent, repository instructions
  (`AGENTS.md` and kin), and user-chosen tools (`jj`, Graphite, plain Git, whatever).
- An explicit GitHub checkout request runs the clone and configured post-clone command described above. It creates no
  branch/worktree workflow, and later session operations do not infer one. Last-reference Delete moves the owned
  directory intact; it does not inspect, reset or clean its repository state.
- The working directory and the running agent are authoritative; the UI never presents a cached branch model as truth.
  VCS-specific UI, if any exists, is informational and degrades to hidden when not applicable.

## Agent-spawned sessions

A running agent must be able to create sessions itself, via a stable CLI or local API available inside its session,
e.g.:

```
farhelm spawn --cwd /home/user/ws/auth-followup \
  --title auth-followup --agent claude \
  --parent "$FARHELM_SESSION_ID"
```

The spawn CLI talks to the session's own supervisor, authenticated by a per-session credential present in the session's
environment — any process inside the session may spawn, and that is the point. `farhelm spawn` targets the session's own
host, and only that host: it is answered by the supervisor on the other end of its socket, which knows nothing about any
other machine. That is a property of this command rather than a limit on what an agent can create —
`farhelm agent create` and `farhelm agent clone` below go through the helm and reach any host in the fleet. The two
coexist on purpose: spawn is the scripting primitive that works with no helm attached, and the agent verbs are the
fleet-aware ones. The helm learns of new sessions automatically; they appear in all clients without manual registration
or refresh.

The CLI's contract, since agents will script against it: on success it prints the child session id to stdout and exits
zero, and success means the session exists — a child whose agent then fails to launch still exists, in error or exited
status. Precondition failures exit nonzero with a message on stderr. `--cwd` and exactly one agent selector are
required. `--inherit-agent` explicitly reuses the asking session's stored invocation, agent kind, resume template, and
profile snapshot, and works with no helm attached. `--agent <name>` resolves the exact name through the attached helm's
catalog; `--profile-id <id>` selects the exact catalog row without treating the id as a name. Both catalog selectors are
refused with a remedy when no helm is attached. The title is generated when omitted. An optional idempotency key makes
retries safe: re-running spawn with the same key after a timeout or ambiguous outcome returns the existing child rather
than creating another. Keys are scoped to the host and live as long as the child session does. Guaranteed
Farhelm-injected environment: the session id (`$FARHELM_SESSION_ID`) and the per-session credential; other
Farhelm-specific variables are illustrative, not contract. (The user's login-shell environment is separately guaranteed;
see Durability.)

A session can also ASK, not only create. `farhelm agent <verb>`, run inside a session with the same injected credential
spawn uses, reaches the helm rather than the session's own supervisor: the supervisor forwards the question to the helm
currently attached to that session and relays the answer back, because a session has no way to reach the helm's machine
directly. The verbs are answered with the HELM's view — every host and profile it knows, and every session it knows,
whichever machine they are on — with the asking session and its host marked. That is deliberately wider than spawn's
own-host-only rule above, which stands unchanged: creating is a local act, asking is not. Every verb goes this way,
including questions about the session's own host, so there is one answer to what an agent sees. The failure this defines
is "no helm is attached to this session", reported as such, with opening the session in a client as the remedy — never a
silent fallback to what the supervisor alone could have answered. The verbs may also ACT — rename, stop, restart — on
any session named by id, including the asking session when the caller deliberately supplies its id, with the helm
applying its ordinary rules to the operation exactly as it would for a client request. Rename also requires the title
the caller observed; the owning supervisor compares and changes it atomically, so a stale agent cannot overwrite a
concurrent rename. Restart requires an explicit `--session` target, one mode selected from that session's discovery
offer (`resume`, `fallback-template`, or `fresh`), and an explicit `--stop-if-running` consent when the target is live.
The owning supervisor revalidates both the offer and liveness at handling time: a stale mode is refused rather than
changed into another mode, and Fresh never discards an available resumable conversation. An explicit self restart warns
before dispatch that it can interrupt the invoking CLI, lose its acknowledgement, and leave resumed task continuation
unconfirmed; it never prints an unobserved completion as success. There is no `farhelm agent replace`: an agent
replacing its own session would be killing itself mid-request, which is a design question this version leaves open
rather than answers by accident.

The verbs also CREATE, and this is where reaching the helm buys something no supervisor-local design could offer.
`farhelm agent create` makes a session on an explicitly named host, and `farhelm agent clone` copies an explicitly named
source session onto an explicitly named host. Host selectors use the display NAME the hosts listing reports; stable host
IDs are also exposed so duplicate names remain visibly distinct, but acting commands do not silently reinterpret a name
as an ID. Both print the new session's id on stdout and nothing else, matching spawn's contract, with the human-readable
confirmation on stderr. The preconditions are the helm's ordinary ones: a directory that does not exist on the target is
that supervisor's own refusal, reported verbatim rather than paraphrased on the way back, and an unreachable target is
refused with its state named. The new session appears in every client the way any other create does.

Agent-requested cross-host create and clone are temporary exceptions to the host-to-host security boundary below. They
currently allow arbitrary execution on the target host; this exposure is explicitly accepted pending the guardrails
tracked in TODO.md's Maybe later bucket. Their existence does not authorize additional cross-host execution
capabilities. Cross-host stop, rename, and restart are separately permitted bounded operations. Restart uses only the
selected session's stored launch configuration on its owning host; it accepts no replacement command.

`create --profile` resolves an exact NAME in the helm's catalog; duplicate names are refused. `create --profile-id`
selects an exact ID without falling back to a matching name. A clone follows its explicitly selected source's
snapshotted profile id on any host while the helm still holds it. No match is a refusal naming the profile. There is
deliberately no fallback to the source's raw invocation: a command line written for one machine may name a binary that
is absent, a different build, or one that takes different flags on another. A session created from a raw invocation has
no profile to follow and clones as that invocation. Create requires exactly one profile name, profile ID, or raw
invocation; it never chooses the remembered default for an agent.

Profile names and IDs are ordinary fleet metadata exposed by `farhelm agent profiles`. Discovery also has `--json` forms
with a versioned envelope, exact IDs, the caller's host identity, and completeness fields. It never exposes raw profile
command lines, credentials, resume templates, or provider configuration. Duplicate names remain separate rows; an acting
command refuses an ambiguous name rather than choosing one.

`farhelm agent instructions` (also spelled `farhelm agent help`) prints the agent-facing account of all of the above:
the verbs, the `*` marker, that a session's own credential is what authorizes the question, and what to do about "no
helm is attached". It is the one verb that reaches nothing — no supervisor, no helm, no credential — because it is what
an agent runs first, and a manual that fails on an unattached session is a manual nobody reads at the moment they need
it. The verb list it prints is derived from the CLI itself, so it cannot describe a set of verbs that does not exist.

`$farhelm help` in a conversation asks the agent for a brief introduction, the available user-level actions, and a few
natural-language examples, not a raw CLI help dump. The agent derives the actions from that generated inventory and does
not query the fleet or mutate anything merely to explain them. This conversational convention does not change ordinary
shell `--help`.

The instructions must identify session titles, working directories, and agent labels in fleet listings as externally
supplied data, not instructions to follow. The helm relaying those values does not make their authors trusted. This is a
short interpretation rule for the reading agent, not a guarantee that Farhelm prevents model prompt injection.

As with interactive creation, spawning launches the agent without an initial prompt in v1; the spawning agent (or the
user) interacts with the child through its terminal.

The parent reference is optional organizational metadata only and implies no VCS relationship. It is acceptable for
agent-created sessions to have no parent reference; the parent filter need not identify everything an agent created. The
control plane creates no worktree, workspace, or branch as part of spawning — if the agent wants a `jj workspace` first,
the agent creates it.

## Errors and diagnostics

- Every failed operation surfaces a concrete, actionable error in the client. A dialog must never close as though an
  operation succeeded when it failed. Two best-effort exceptions log a failure but stay silent rather than surfacing it:
  the helm-side preference (list order, last selection, and compact layout), because losing next-launch convenience must
  not turn a choice that already took effect into a failed current operation, and a helm that lost the preference falls
  back to the defaults; and the automatic "mark seen" a session's own opening or activity advance triggers (see Status),
  because a lost automatic mark costs nothing worse than a dot that is one open-and-close cycle behind, corrected by the
  next successful write. The manual "mark unread"/"mark read" toggle is not covered by either exception — a failed
  toggle surfaces like any other operation.
- Connection state per host is always visible in the host list; reconnection uses bounded retries followed by periodic
  low-frequency re-probing, so a host that comes back overnight resurfaces by itself. Actions stay in each row's menu,
  while the global details disclosure shows the evidence and remedies behind the phase.
- Logs are available for: the helm, each supervisor, session creation, process/PTY lifecycle, attachment transfer,
  reconnection, and resume attempts.
- Long-lived input/output/paste paths have health checks, so "typing goes nowhere" is detected and reported rather than
  left for the user to infer.
- Mixed versions across helm and supervisors are a normal steady state, since updates are user-controlled. Incompatible
  versions refuse to connect with a clear, actionable error; there is no silent degradation.

## Security

The [maintainer-confirmed decisions](#maintainer-confirmed-decisions) below define local account authority, directional
trust between hosts, and the exact temporary exceptions for agent-requested session creation and cloning. Apply those
boundaries when interpreting the transport and credential rules here.

Steady-state operation has exactly two network edges — the browser to the helm (token-authenticated) and the helm to
each supervisor (SSH) — plus one deliberately local one.

- **Client to helm**: the helm serves its web UI over plain HTTP bound to loopback only, with a required token. The helm
  refuses to bind non-loopback addresses in v1; TLS serving is post-v1. Reaching the UI from another machine means an
  SSH port forward the user sets up themselves — there is no built-in tunneling or Tailscale integration in v1. The
  browser therefore always talks to localhost, which is conveniently a secure context — the precondition the browser
  clipboard APIs require to be reachable at all. Eligibility is not the same as success: engine policy and per-request
  permission still apply on top of it, and a clipboard operation that the engine refuses fails silently by the Terminal
  experience section's own clipboard contract above, not with an error. The token still matters on loopback: it keeps
  other local processes and users out. The helm generates it on first run; the user views or rotates it on the helm's
  machine (`farhelm helm token show|rotate`), and the browser asks for it once per device and keeps a session
  thereafter. Rotating the token invalidates every device credential for new requests; already-admitted requests may
  finish. Existing terminal and event-feed connections may remain usable or close on rotation, whichever keeps the
  implementation simpler; reconnecting requires a current credential. Rotation does not stop running agent sessions. The
  native app embeds its helm; that edge is local. The token keeps other users OUT of the helm; it does not let the
  browser tell the helm apart from another local user's process that binds the same port while the helm is down. That
  gap is accepted in v1: the browser UI is recommended only on a machine with no other, untrusted local users, and the
  native app is the preferred client wherever it is available. `docs/security.md` records the reasoning.
- **Helm to supervisor**: SSH, and only SSH, for every remote supervisor. Passwordless access from the helm's machine,
  as the user, is the requirement; authentication is the user's SSH keys, and supervisors listen on no network port of
  their own. Registering a host means giving the helm its SSH destination — there is no supervisor token to manage. The
  helm's own machine's supervisor is reached locally, no SSH involved.
- **Session to supervisor (spawn)**: the spawn CLI reaches its own supervisor over local IPC only, never the network.
  Its per-session credential identifies the asking session and dies with that session. It does not restrict permitted
  operations to that session: fleet operations follow the Agent-spawned sessions contract and the confirmed trust
  boundaries below. This is an interface credential, not containment against processes with the same Unix account
  authority.

Further requirements:

- SSH is Farhelm's one transport integration: given passwordless SSH access, the helm rides it automatically for
  provisioning, updates, and all supervisor communication. How the SSH connection itself is possible (a tailnet, a LAN,
  whatever) is the user's business. No public relay, no third-party rendezvous service.
- Provisioning rides the user's existing SSH access — their keys, agent, and config. Farhelm stores no SSH credentials
  of its own.
- Agent credentials (e.g. Claude subscription auth) live on the host running the agent, in the agent's own standard
  configuration. The system must not extract, proxy, or repurpose agent OAuth credentials. Claude Code authenticates
  directly with a consumer subscription, unmodified.
- Updates are user-controlled: optional or version-pinnable, never silently forced.

## Non-goals for v1

- Notifications of any kind.
- Automatic initial-prompt delivery (at creation or spawn).
- Terminal history persistence beyond what the live terminal retains. A durable history store is a possible future
  add-on; v1 leans on agent conversation resume instead.
- A prompt composer or message-level abstraction over the terminal.
- Multi-writer session sharing, or multiple concurrent helms.
- TLS on the web edge — the helm binds loopback only, and tunnels provide transport security.
- Tailscale integration, and built-in tunneling for the web edge — that stays a local port plus user-managed SSH
  forwarding. (Automatic SSH transport for the helm-to-supervisor edge is in scope; see Topology.)
- IDE functionality, diff viewers, code review UI.
- Built-in Git branch management, stacked-PR tooling, or a `jj` graph editor.
- Cloud-hosted execution or multi-user collaboration.
- Reimplementing or wrapping agent internals (ACP or structured agent protocols may come later but must never displace
  raw terminal mode).

## Acceptance test

The first usable version is complete when all of the following pass:

1. From the helm on the Mac (native app) — with helm-side access to the configured release source (GitHub by default) or
   a staged payload directory — given nothing but passwordless SSH to a fresh Ubuntu host, provision it in one action:
   supervisor installed and started without root, host registered, sessions operable with no further network setup. Also
   open the same helm's web UI from a browser (token-authenticated).
2. Create and launch an official Claude Code session in one action, in an existing `jj` workspace where Git reports
   detached HEAD.
3. Create a local (Mac) session the same way; both appear in one list.
4. Paste a Mac screenshot into the remote session's terminal; the path appears at the cursor and Claude reads the file.
5. Quit and relaunch the app: both sessions are still running, terminal state intact, exactly as left. Then reboot the
   Mac: the remote session is untouched; the local session shows interrupted, and opening it offers resume that restores
   the conversation.
6. Attach to the remote session from the web UI; the native app visibly detaches.
7. Ask Claude to create a new `jj workspace` and spawn a child session via the provided CLI; the child appears in the
   client without refresh.
8. Restart the Linux supervisor while its session runs: the terminal is uninterrupted and no state is lost.
9. Reboot the Linux host: its sessions show as interrupted; opening one offers resume, and the session's own
   conversation — not just the most recent one in that directory — is restored.
10. Create two Claude Code sessions in the same directory; restart both; each resumes its own conversation. Repeat with
    two Codex sessions.
11. Add a terminal tab to a session and use a shell in the agent's working directory.
12. Trigger an invalid operation (e.g. create a session in a nonexistent directory) and get a visible, actionable error,
    not a silent failure.

## Maintainer-confirmed decisions

These requirements were explicitly confirmed with the maintainer on 2026-09-07. They record intended behavior and
accepted tradeoffs, not verification that the current implementation satisfies them. Other requirements in this spec
remain authoritative; this section distinguishes direct confirmation from indirectly inferred intent.

An agent must raise a conflict with these decisions to the maintainer before proceeding with a conflicting change,
unless the user's instruction clearly and intentionally overrides the decision. A general request to implement a feature
is not such an override. When a decision changes, reconcile its detailed contracts as well as this section.

Where a maintainer-confirmed decision accepts alternative outcomes and says to prefer the simpler implementation, every
named outcome is explicitly acceptable, not an unresolved specification gap. Reviewers must not flag an accepted outcome
alone as a defect or require additional machinery solely to select another accepted outcome. Preserve the decision's
firm boundaries; this allowance does not extend to unrelated behavior or override other requirements.

### Healthy local filesystems

Confirmed 2026-09-19: assume each host's local filesystem is healthy. Local filesystem I/O errors or hangs may cause
failures or halt progress on that host. Do not add complexity to recover from or bound those problems solely to keep the
affected host making progress; those outcomes are accepted, not defects requiring additional recovery machinery. This
applies to each supervisor host as well as the machine running the helm.

The boundary is host isolation: a remote host's broken filesystem must not freeze or break the helm or prevent it from
serving other hosts. Requests involving the affected host may fail or remain pending, but the helm must otherwise
continue to function. This allowance concerns filesystem errors and hangs, not ordinary cancellation or disconnection
while the filesystem is healthy.

### Desktop Quit

Quit must close the desktop app promptly, without waiting for in-flight uploads or other requests to finish.
Interrupting that work is intentional product behavior, not merely an acceptable simplification; do not add a
graceful-completion window that delays Quit. Ordinary cleanup of interrupted work still applies, with the accepted
final-publication race described under Attachments; cleanup does not require rolling back a completed attachment. Agent
sessions outlive the app under the existing durability contract. Giant bulk uploads, such as 50 GB files, are not an
expected attachment use case; this does not impose a new numeric upload limit. Credential rotation is a separate
operation and retains its admission-only contract.

### Provisioning download sanity limit

Release assets downloaded onto the helm for provisioning must have a simple per-download size limit, set far above any
practical expected release size. This is only a sanity check against pathological responses, not a quota system or a
general resource-isolation feature. Refuse an oversized download with a simple failure message; minimize implementation
complexity rather than adding elaborate recovery or UX. This requirement concerns the helm's release-payload downloads,
not `install.sh` or user attachment uploads. The precise threshold is an implementation choice with ample headroom.

### Partial deletion

An explicitly requested Delete may partially remove a session's state before a later step fails. This includes removing
its attachment files while its database row remains listed for retry. The failure must be visible, and another Delete
must be able to continue cleanup. Rollback or preservation of already-removed files is not required; reviewers must not
flag partial deletion alone as a defect or require transactional recovery machinery for that accepted outcome. This
allowance applies to Delete, not to removing attachment files during Stop.

### Terminal-tab working directory

A terminal tab opens using the session's associated working-directory path. This is deliberately a simple path contract:
normal filesystem resolution applies, including any changed symlink targets. Farhelm need not preserve or compare the
original directory's inode or resolved destination when opening a tab. Following the path to a different directory is an
accepted outcome, not a correctness or security finding requiring additional identity machinery. Missing or unusable
paths still fail clearly. This decision concerns terminal tabs; the existing agent-restart identity check is separate.

### Optional parent metadata

Parent tracking is explicitly optional for now. `farhelm spawn --parent` can record a relationship, but
`farhelm agent create` and `clone` are not required to attribute the new session to the asking session. The resulting
incomplete parent filter is an accepted limitation, not a missing correctness guarantee. Future work should consider
either removing agent parent/child relationships or making them useful; neither direction is selected or required now.

### Local authority and trust between hosts

The local security boundary is the Unix account on a particular host. Farhelm does not isolate an agent from other
processes or state accessible to that account. Session credentials identify and admit interface requests; they do not
provide same-account containment. Running agents without permission checks in disposable remote environments is an
intended use. Future container or sandbox support would require a new, explicit isolation contract.

A supervisor trusts its attached helm to administer it, launch processes, and forward user input. That trust is
directional: the helm and GUI must treat remote supervisor messages and agent-controlled output as untrusted. A remote
host must not gain unauthorized execution or access to secrets on the helm's machine or another host through Farhelm.
Existing redaction promises remain requirements even where the sender already has local account authority.

A program in an attached remote terminal may write the viewer machine's system clipboard through OSC 52, without a
separate local selection or copy gesture. This is an explicitly allowed, bounded effect across the remote-host boundary,
including when a malicious program replaces the clipboard. Programs must not read that clipboard through Farhelm. A
user-initiated paste is intentional delivery of the pasted content to the selected terminal, not permission for
program-initiated clipboard reads. Clipboard writes remain best-effort as specified in Terminal experience; an opt-out
control is not a current requirement.

Agents may intentionally stop, rename, and restart sessions on other hosts through the helm. Those named, bounded
effects are authorized even when invoked by a malicious agent. Existing agent-requested session creation and cloning
across hosts are the only temporary execution exceptions: they permit arbitrary execution on the target today, and that
exposure is accepted pending the guardrails in [TODO.md's Maybe later bucket](TODO.md#maybe-later). Existing
agent/supervisor-originated creation retries share that acceptance; permanent retention of their retry records is not
required. This does not waive correctness of user-initiated GUI requests or select a pruning implementation.

Do not add other arbitrary cross-host execution capabilities by analogy with those exceptions. Future agent-driven
orchestration, such as setting up several sessions on another host, is wanted with an explicitly authorized launch
policy; trusted profiles are a possible design, not a security property established for the current catalog.

### Remote input, session defaults, and availability

Agents may discover the helm catalog's profile names and IDs. Listing those names and IDs in lookup suggestions is
explicitly allowed, not a confidentiality defect. This permission does not extend to raw command lines or embedded
credentials and does not require a new discovery interface. It also does not make current profiles trusted execution
guardrails; the separate host-authority rules still apply.

Agent instructions must identify fleet session metadata as data, never instructions to follow; see
[Agent-spawned sessions](#agent-spawned-sessions) for the CLI contract. Merely echoing an agent's own input into its own
session terminal does not establish a security defect: the agent already controls that output. This does not excuse
unsafe rendering of remote input by the helm or GUI, secret disclosure, or violations of existing formatting contracts.

The helm owns the remembered default for profile-backed session creation. The structured composer still opens without a
selected harness; this authority rule does not require it to preselect a profile. A remote supervisor's reported
timestamps, profile references, or other session metadata must not override an explicit user choice or indefinitely
determine that default for other hosts. The temporary agent-requested create/clone exception does not authorize this
influence over user-driven session creation.

Failures or malicious behavior from a remote host must not disrupt unrelated hosts or ordinary helm/GUI controls, apart
from the explicitly permitted operations above. Supervisors need sensible recovery from ordinary failures; they need not
defend their availability against hostile processes with the same local account authority. Choose proportionate remedies
rather than assuming a quota or scheduling architecture is required.

### Ownership during cleanup and provisioning

Farhelm supports documented interactions with its private tmux server, including creating windows from inside a session.
Arbitrary reconfiguration is the local operator's responsibility; Farhelm need not reconstruct its intended
configuration afterward. Missing objects must be handled sensibly, and an operation must not accidentally affect the
wrong object. The helm and GUI must still handle the resulting remote failures safely.

Session teardown covers ordinary agent descendants, including background servers. Detached services started by shell
initialization before the agent launches are outside that guarantee; see [Lifecycle operations](#lifecycle-operations).

After a failed Delete disconnects a viewer, either automatically reconnecting to a surviving terminal or remaining
detached until the user reconnects is explicitly acceptable. Choose the simpler implementation. Reviewers must not treat
either outcome alone as a bug or require additional recovery machinery to choose between them. The cleanup failure must
remain visible; recovery must not restart an agent or take control from another viewer. A retained session record does
not promise that cleanup preserved the terminal or its scrollback.

Provisioning may enforce permissions on directories dedicated to Farhelm. It must preserve permissions on existing
shared directories merely used to hold its executable or service files. If those permissions prevent installation,
report the obstacle rather than silently changing them. Trust in the helm does not authorize incidental changes to
unrelated host configuration.

### Upgrade compatibility and client scale

Viewing and rotating the browser sign-in token through `farhelm helm token show|rotate` on the helm's machine is
sufficient for the current product. An app-UI token-management surface is not a current requirement.

Browser sign-in token rotation prevents old credentials from admitting new requests to the helm. It does not require
cancelling requests already admitted, including attachment uploads, or rolling back work already performed. Already-open
terminal and event-feed connections may continue to work, including terminal input, or may close as a consequence of
rotation. Both outcomes are explicitly acceptable; prefer the simpler implementation. Neither outcome alone is a defect
or a reason to add cancellation or continuity machinery. Any new request or connection, including a reconnect, must
authenticate with a current credential. Rotation does not stop the agent processes running in Farhelm sessions.

Supporting a range of historical data schemas and hardening every upgrade/downgrade path are not current design goals.
During feature design, agents must alert the maintainer to potential loss of data or state and absence of a downgrade
path before proceeding. Compatibility is decided per feature; this is not blanket permission for destructive migrations
or ordinary runtime data loss. Revisit broader compatibility as the project matures, and record future breaking
transitions when there is an expectation of users beyond the maintainer.

The helm is optimized for a handful of browser/desktop clients, not a large device fleet. Retaining the 64 newest client
credentials is acceptable even when an older credential is actively used. Beyond a few tens of enrollments,
reauthentication friction is acceptable; activity-based eviction is not required. Enrollments are credentials, not
physical devices, and eviction does not delete Farhelm sessions or terminate their agent processes.
