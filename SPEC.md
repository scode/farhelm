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
  shells) that open in the same working directory. Session metadata — title, archived flag, parent reference, stop
  annotations, captured conversation identity — is durable and lives with the session's supervisor, so it survives helm
  loss and re-registration; terminal contents live only as long as the host-side terminal does (see Terminal
  experience).
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
touching the host, the helm states exactly what it is about to do in concrete terms — the files it will place and where,
the systemd units it will create, and that the supervisor will run persistently and start at boot — and proceeds only on
confirmation. The same transparency applies to updates, which use the same mechanism. V1 provisioning targets any Linux
host with a usable systemd user manager, on the two architectures cross-compiled supervisor binaries exist for. The
distribution is not a requirement — nothing provisioning does is distribution-specific — so the plan names whichever one
it found rather than refusing; CI exercises Ubuntu. Everything else — no usable systemd user manager, or an architecture
with no payload — falls back to the manual path (run the binary yourself), which always remains available.

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
exist on the host that runs it. Every release supplies read-only built-in Claude Code, Codex, and Muse profiles, each in
a plain and a permission-skipping ("yolo") variant: `claude`, `claude-yolo`, `codex`, `codex-yolo`, `muse`, and
`muse-yolo`. They appear beside the user's stored, editable definitions and are identified as Built-in; historical
stored starter rows remain editable and deletable. Integrations are not user-authored — a profile optionally names an
agent kind from Farhelm's built-in v1 catalog (Claude Code, Codex), which selects that kind's status heuristics and
conversation-identity capture; profiles without a kind get generic treatment.

Muse support uses `muse` and `muse --yolo` with generic activity status. The yolo variant skips approval prompts and
sandboxing and trusts the workspace for the run. Muse-specific hooks, conversation capture/resume, and waiting-state
recognition are not implemented; no Muse integration kind is implied by the presence of its built-in profiles.

OpenCode is a structured harness, not a built-in profile. Its Zen model is required: Farhelm suggests
`opencode/glm-5.3-flash`, `opencode/grok-4.5`, `opencode/grok-4.6`, and `opencode/glm-5.3`, while the custom-model field
also accepts a bare Zen model name or an `opencode/<model>` value. A bare value is passed as `opencode/<model>`; another
provider prefix is refused. OpenCode has no offered effort choices. Its default permission mode adds no flag, and YOLO
uses OpenCode's `--auto`, which auto-approves only permissions not explicitly denied. OpenCode uses generic activity
status with no hooks, conversation capture/resume, or waiting-state recognition.

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

## Sessions

### Creation

Session creation is one action, not a wizard. Only the working directory is fundamentally required:

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
- Launch composer: New opens a dialog with no selected harness. Structured Codex, Claude, Muse, and OpenCode launches
  carry a harness plus model, effort, and YOLO permission choices where that harness supports them; absent optional
  choices mean the selected harness's defaults and omit their flags. OpenCode is the explicit exception: it requires a
  model and offers no effort choice. The helm owns the released model catalog and validates every structured choice, so
  the browser never turns a model identifier into an argv fragment. A known model identifies its owning harness; a
  custom model needs an explicit harness. Replacing a harness clears only choices that are incompatible with it. An
  invalid combination cannot launch.
- Legacy agent profile or arbitrary command: an explicit secondary creation surface. Existing callers, profiles, and
  their helm-wide last-used profile behavior remain compatible, but New does not silently choose a remembered profile.
  Values from this surface cannot affect a structured request, or its idempotency key. The helm owns the remembered
  profile default: remote supervisor metadata must not override an explicit user choice or indefinitely determine the
  default profile for sessions on other hosts. This does not make the structured composer preselect a harness or
  profile. See the maintainer-confirmed decisions below.
- Recent setups: the helm remembers bounded successful structured combinations and used folders per target-install
  identity. A recent row fills every saved choice and directory but never launches. A retargeted registry row cannot
  expose the replaced install's history. Folder search uses that bounded history, not a recursive filesystem walk;
  explicit browsing asks the selected supervisor for one bounded directory level. Recent-setup rows appear only when the
  current filter actually matches history; the dialog reserves no space for them when it does not.
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

Advanced options (the custom model id) stay hidden by default; the optional session name is not one of them and shares
the top row of the launcher with Launch and Cancel. Project registration (associating metadata with a directory) is
optional convenience and never a prerequisite.

Creation guards against accidental double submission (a double-click, a retry after a timeout): one intended create
yields one session or a clear error, never two silently. Deliberately creating several sessions with identical
parameters — same directory, same profile — is a sanctioned workflow, not a duplicate to be suppressed.

A session snapshots its profile at creation — launch and resume invocations and integration selection alike. Editing or
deleting a profile affects future sessions only; existing sessions keep working unchanged.

Failures split cleanly in two: precondition failures (nonexistent directory, unknown profile, unreachable host) fail the
create with a visible error and no session; launch failures of a session that was successfully created surface on the
session itself — **error** when the agent process could not be started at all (exec failure, command not found),
**exited** when it started and then ended, however quickly, with its exit code visible.

### Lifecycle operations

The client supports: create, open, rename, restart, clone, replace, stop, archive, delete.

- **Stop** terminates the agent and its entire process tree — MCP servers, dev servers, and other descendants included.
  Terminal tabs keep running, and the session remains with its terminal still viewable.
- **Restart** relaunches the agent in the same working directory, resuming the session's own conversation where
  supported (see Durability for the exact promise). This is the only relaunch mechanism: the resume offered when opening
  an interrupted session is this same operation, not a separate feature. There is no fresh-restart variant in v1 — for a
  clean conversation, create a new session in the same directory. Restart on a session whose agent is still running
  confirms, stops the agent, then relaunches. Restart reuses the session's terminal when it still exists — whatever
  scrollback the terminal itself retained is still there — and creates a fresh one when it does not (after a reboot, or
  on an archived session). Restart does NOT preserve the previous run's last visible screen: the pane is blank until the
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
  duplicate is allowed, the same as any other create. Clone is offered on archived sessions too: it is the only way to
  get a new, running agent out of one without restarting (and thereby unarchiving) the original.
- **Replace** creates a new session — new id, fresh conversation, same host, working directory, title, and agent (a
  profile while the source's profile is still the one it names, otherwise the source's raw invocation, exactly as clone
  resolves it) — and then DELETES the source; it never archives it. Confirmed directly from the row menu, with one
  inline confirmation and nothing to edit first, since the whole point is the same settings. Contrast restart, which
  keeps the session's own id and its conversation: restart continues a session, replace starts one over under the same
  settings. If the create fails, the source is untouched. If the create succeeds and the removal that follows fails, the
  reply names both sessions; whether the source is still there depends on how the removal failed, and the user checks or
  removes it by hand. Replace is offered wherever clone is offered, archived sessions included — an archived source has
  no agent to kill, only a record to delete.
- **Archive** hides the session from the default list and shuts down everything in it — agent and terminal tabs — with
  confirmation when anything is still running. Archived sessions keep their metadata; their terminal contents are gone
  (see Terminal experience). Restart on an archived session unarchives it and recovers the conversation where the agent
  supports resume.
- **Delete** removes the session and its stored state, in any state, terminating the agent and tabs if running — with
  confirmation that says so when anything is still alive. Deletion may make partial progress before failing, including
  removing attachment files while retaining the session row for retry. There is no rollback guarantee. Report the
  failure visibly and allow a later Delete to finish cleanup; a retained row does not mean previously removed state has
  been restored.

Process-tree ownership is session-wide. Restart reaps any leftover descendants of the prior run before relaunching —
never alongside them. Stop, archive, and delete reap everything the agent started. An agent exiting on its own does not
trigger a hunt for daemonized survivors; the session's next restart or its teardown does. Operations that need the
working directory — restart, opening a terminal tab — fail with a clear error naming the directory if it has vanished
since creation; the session itself remains, and archive and delete still work.

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
reboot or an archive, tabs are gone and the user re-adds them; nothing recreates them automatically. A tab can be closed
individually, which kills that shell and its processes — that is the whole per-tab operation set in v1. A tab whose
process exits on its own is reaped automatically and silently: the tab disappears as if closed, its dead pane's
scrollback is discarded, and no notice or exit code is shown. This is deliberately NOT the agent terminal's contract —
an exited agent stays viewable with its scrollback — because a tab's shell exiting is the user being done with the tab.
A shell that dies before the tab's open completes still refuses the open loudly, with the shell's last words as the
error. A tab someone has hand-split into several panes (through the session's own tmux access) counts as exited only
when EVERY pane in it has — one exited half must not condemn a shell still running beside it.

When a session's terminal contents no longer exist on a reachable host — exited across a reboot, or archived — opening
it shows the session's metadata and says why there is no terminal, rather than an empty pane.

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

The list always carries a count, and it counts the list you are looking at: archived sessions are outside the default
view, so they are outside its count. The host selector is a narrowing query, so its count says how many matched
alongside how big the default non-archived view is.

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
activity time; live status dots and locality marks occupy aligned columns, while ended status words stay beside the
title with the dot slot reserved. Times share a right-aligned column before the row menu. A session whose host cannot
yet be placed either way marks neither, rather than guessing. Its second line shows the helm-supplied host name (an
alias when set), then `:`, then its working directory; a legacy row with no host name shows only the directory, rather
than inventing a local identity or a dangling separator. The second line is hidden when the helm-wide compact preference
is on, which defaults off and is shared at the next preference seed across clients. The working directory and launch
command remain abbreviated only where shown, with their full, untouched values always available on the row (a tooltip on
the web and desktop clients); an abbreviation is never the only place a value is recorded. A row's own actions menu,
beyond the lifecycle operations above, also offers a mark read / mark unread toggle — reachable there or by clicking the
dot itself — that sets the session's seen state directly (see Status). Exactly how a row lays out its lines and pixels
is an implementation choice, covered in SPEC_impl.md rather than here.

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
restart, archive, or delete are the ways out.

How a status is DRAWN depends on how much it has to say. The three live states are a color-coded dot beside the
session's title — running pulses, waiting and idle do not — with the status word itself always present as text for
screen readers and anything else that reads rather than looks, never replaced by the color. Ended states keep their word
visible, because an exit code, the stop annotation, and the reason an agent never started are facts no dot carries. The
pulse is a claim about the present, so it stands down wherever the status is a last-known report rather than a live one:
a session on an unreachable host shows a still dot whatever its status says.

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
timestamp available on the row; that is what makes the list's recently-active order legible instead of implicit. The age
is a difference between two machines' clocks and is only as good as they are, so it is never the only place the
underlying time is recorded. It is also independent of the status beside it: a session nothing has classified yet shows
no status and still shows its age.

Two cases have no age to show, and both show nothing rather than a guess. A helm predating the last-activity field sends
no stamp, and the session's creation time stands in — the same fallback that orders the list, so the column and the
order agree. A session with neither stamp gets no age at all, never one counted from 1970.

Running/waiting/idle discrimination for raw TUIs is inherently heuristic, and the waiting/idle boundary especially so.
The bar: best-effort observation-based heuristics (output activity, terminal state), optionally sharpened per agent
profile with agent-specific heuristics. Wrong status must be cosmetic only — status detection must never gate or delay
interaction with the terminal. Integrations that require configuring the agent itself (e.g. Claude Code hooks) may be
supported later but are not part of v1 and must never be required.

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
  a day later, and the buffer is still there. There is no separate history store — when a host reboots or a session is
  archived, terminal contents are gone, and recovering the conversation is the agent's job (resume). A stopped or exited
  session's terminal stays viewable while its host is up, since the terminal outlives the process. Viewable means what
  the terminal itself holds: a full-screen program's last frame is not retained after it exits, and no snapshot of it is
  taken or stored.
- Opening a session attaches to it — and opening a CLIENT counts as opening a session: with a non-empty fleet, a freshly
  loaded client selects and attaches the session the user most recently selected from any client — the helm remembers
  one selection for all of them (see Session list) — falling back to the newest-created non-archived one (chosen from
  the rows the listing carries, so when the list was cut at its cap under an order other than creation time the pick can
  be the newest the reply reached rather than the fleet's true newest — the accepted edge of the whole-list cap), so
  launching the app is itself the deliberate act the attach semantics below key off. Opening a second client therefore
  attaches to whatever was most recently selected anywhere and takes the terminal over exactly as clicking the same
  session there would. The attached client owns input and terminal dimensions: the PTY resizes to that client, and the
  last size sticks when nothing is attached. Reconnecting replays the terminal so the session looks as it would have had
  the client stayed attached, modulo redraws caused by dimension changes. The floor: the host-side terminal retains, and
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
- If Delete or Archive fails after disconnecting a viewer, automatic reconnection to a surviving terminal and remaining
  detached until the user reconnects are both explicitly acceptable. Prefer whichever is simpler to implement; neither
  outcome is a defect or a reason to add recovery machinery. Keep the cleanup failure visible. Recovery must not restart
  an agent or take control from another viewer, and retaining the session record does not guarantee that its terminal or
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
failure and no rollback guarantee. Archive and Stop do not gain permission to remove attachment files from this rule.

Attachment bytes ride the existing edges — client to helm, helm to supervisor. There is no direct client-to-supervisor
path; a browser never needs to reach any machine but the helm's.

Transfer must not block the terminal: you can keep typing while it runs, and the path is inserted at whatever cursor
position is current when the transfer completes. For a typical screenshot this is imperceptible.

Upload failures must be visible; an attachment must never disappear silently.

## Desktop window chrome

The macOS desktop window integrates its native title bar with the app header: native traffic-light controls sit in the
sidebar's top row beside Profiles and the version readout, with no separate visible app-title strip. The session header
and terminal tabs continue the app's surface to the top edge. Empty header space provides window dragging; controls and
terminal text retain their own interactions. This treatment changes only header appearance and spacing. Browser and
Linux window layouts retain their existing appearance. In narrow macOS windows, the app-level row stays fixed above both
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
`claude --resume <conversation-id>`) — even when several sessions share a working directory. Claude Code and Codex
integrations at this level are both required in v1. Identity is reported by the agent itself when its kind supports a
per-launch hook, and scanned from the outside — the agent's terminal, its own on-disk session records — otherwise; a
report wins over a scan, because it is the agent's own answer rather than a correlation over what the agent happened to
leave on disk. What capture never does is write to the agent's own configuration or record directories. A hook passed on
the command line for one launch is allowed because it writes nothing the vendor owns — no configuration file, no
conversation record, no trust state — and cannot outlive the launch that carried it. It is not invisible in the
absolute: the report it delivers lands in farhelm's own database, and every run leaves a line in farhelm's own hook log.
Vendor-owned state is the boundary the no-agent-configuration rule from Status is protecting, and that rule's own
example — hooks written into the agent's configuration — still stands. Scanning stays the fallback whenever no report
has been accepted, which covers more than unhooked launches: a hook that is skipped, fails, times out, or is refused
leaves the scan in charge exactly as before. The hook is therefore never required. Both Claude Code and Codex offer such
a hook and write discoverable session records, which is why requiring this in v1 is safe.

When an integrated session has no explicit resume invocation, its resume invocation is derived from the original launch
argv retained for that session: Claude appends `--resume <conversation-id>`, and Codex appends
`resume <conversation-id>`. The original argv is reused as-is, including permission and configuration arguments, and is
preserved as argv elements rather than rejoined shell text. This immediate rule assumes every original argument is
reusable and that the launch has no initial prompt or launch-only option; separating those concerns into common, launch,
and resume arguments is deferred.

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

The control plane is version-control-agnostic and mutates nothing:

- Sessions launch in any existing directory: detached HEAD, no `.git`, colocated or pure `jj` workspaces, nested
  repositories, and plain directories all work identically.
- No requirement of branches, one-branch-per-session, Git worktrees, default-branch workflows, or any PR topology.
  Stacked changes work because the control plane stays out of the way, not because it models them.
- The system never performs VCS mutations implicitly. Repository state is owned by the agent, repository instructions
  (`AGENTS.md` and kin), and user-chosen tools (`jj`, Graphite, plain Git, whatever).
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
status. Precondition failures exit nonzero with a message on stderr. Only `--cwd` is required; with no selector, the
child reuses the asking session's own stored invocation, agent kind, resume template, and profile snapshot. That path
works with no helm attached. `--agent <name>` resolves the exact name through the attached helm's catalog and is refused
with a remedy when no helm is attached. The title is generated when omitted. An optional idempotency key makes retries
safe: re-running spawn with the same key after a timeout or ambiguous outcome returns the existing child rather than
creating another. Keys are scoped to the host and live as long as the child session does. Guaranteed Farhelm-injected
environment: the session id (`$FARHELM_SESSION_ID`) and the per-session credential; other Farhelm-specific variables are
illustrative, not contract. (The user's login-shell environment is separately guaranteed; see Durability.)

A session can also ASK, not only create. `farhelm agent <verb>`, run inside a session with the same injected credential
spawn uses, reaches the helm rather than the session's own supervisor: the supervisor forwards the question to the helm
currently attached to that session and relays the answer back, because a session has no way to reach the helm's machine
directly. The verbs are answered with the HELM's view — every host it knows, every session it knows, whichever machine
they are on — with the asking session and its host marked. That is deliberately wider than spawn's own-host-only rule
above, which stands unchanged: creating is a local act, asking is not. Every verb goes this way, including questions
about the session's own host, so there is one answer to what an agent sees. The failure this defines is "no helm is
attached to this session", reported as such, with opening the session in a client as the remedy — never a silent
fallback to what the supervisor alone could have answered. The verbs may also ACT — rename, stop, archive — on the
asking session or on any session named by id, with the helm applying its ordinary rules to the operation exactly as it
would for a client request. There is no `farhelm agent replace`: an agent replacing its own session would be killing
itself mid-request, which is a design question this version leaves open rather than answers by accident.

The verbs also CREATE, and this is where reaching the helm buys something no supervisor-local design could offer.
`farhelm agent create` makes a session on any host, and `farhelm agent clone` copies the asking session onto any host —
in both cases naming the target by the display NAME the hosts listing reports, since that is the only handle an agent
has ever been shown; an aliased host is named by its alias only. Both print the new session's id on stdout and nothing
else, matching spawn's contract, with the human-readable confirmation on stderr. Omitting the host means the asking
session's own, which is a legitimate ask rather than a degenerate case. The preconditions are the helm's ordinary ones:
a directory that does not exist on the target is that supervisor's own refusal, reported verbatim rather than
paraphrased on the way back, and an unreachable target is refused with its state named. The new session appears in every
client the way any other create does.

Agent-requested cross-host create and clone are temporary exceptions to the host-to-host security boundary below. They
currently allow arbitrary execution on the target host; this exposure is explicitly accepted pending the guardrails
tracked in TODO.md's Maybe later bucket. Their existence does not authorize additional cross-host execution
capabilities. Cross-host stop, archive, and rename are separately permitted bounded operations.

The agent is resolved by NAME in the helm's catalog. `create --profile` resolves that name once into a launch bundle,
and a clone follows its source's snapshotted profile id on any host while the helm still holds it. No match is a refusal
naming the profile. There is deliberately no fallback to the source's raw invocation: a command line written for one
machine may name a binary that is absent, a different build, or one that takes different flags on another. A session
created from a raw invocation has no profile to follow and clones as that invocation. A create naming neither a profile
nor an invocation falls back to the helm-wide remembered default.

Profile names and IDs are ordinary fleet metadata that agents may discover, including through suggestions in a no-match
refusal. This does not make raw profile command lines or embedded credentials public, and it does not require a
dedicated profile-listing command.

`farhelm agent instructions` (also spelled `farhelm agent help`) prints the agent-facing account of all of the above:
the verbs, the `*` marker, that a session's own credential is what authorizes the question, and what to do about "no
helm is attached". It is the one verb that reaches nothing — no supervisor, no helm, no credential — because it is what
an agent runs first, and a manual that fails on an unattached session is a manual nobody reads at the moment they need
it. The verb list it prints is derived from the CLI itself, so it cannot describe a set of verbs that does not exist.

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

### Desktop Quit

Quit must close the desktop app promptly, without waiting for in-flight uploads or other requests to finish.
Interrupting that work is intentional product behavior, not merely an acceptable simplification; do not add a
graceful-completion window that delays Quit. Ordinary cleanup of interrupted work still applies. Agent sessions outlive
the app under the existing durability contract. Giant bulk uploads, such as 50 GB files, are not an expected attachment
use case; this does not impose a new numeric upload limit. Credential rotation is a separate operation and retains its
admission-only contract.

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
allowance applies to Delete, not to removing attachment files during Archive or Stop.

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

Agents may intentionally stop, archive, and rename sessions on other hosts through the helm. Those named, bounded
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

After a failed Delete or Archive disconnects a viewer, either automatically reconnecting to a surviving terminal or
remaining detached until the user reconnects is explicitly acceptable. Choose the simpler implementation. Reviewers must
not treat either outcome alone as a bug or require additional recovery machinery to choose between them. The cleanup
failure must remain visible; recovery must not restart an agent or take control from another viewer. A retained session
record does not promise that cleanup preserved the terminal or its scrollback.

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
