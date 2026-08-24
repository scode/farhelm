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
- **Agent profile**: a named, user-editable definition of how to run an agent. Its fields: the launch invocation
  (command line including arguments, e.g. `claude`, `claude --dangerously-skip-permissions`, `codex`); an optional
  resume invocation, a template that may reference the captured conversation identity (e.g.
  `claude --resume {conversation}`); and optional agent-specific integrations (status heuristics, conversation-identity
  capture — see Status and Durability). Both invocations may reference the session's working directory as `{cwd}`, for
  launchers that take the directory as an argument. The user controls the invocations completely; the integrations are
  the only per-agent machinery Farhelm itself carries.

## Topology

One control plane, two ways to face it:

1. **Native Mac app**: the helm packaged as a Mac app, together with a local supervisor so the Mac itself is a host.
   This is the setup when the Mac is your main machine. The app bundles no tmux: it requires Homebrew's (at or above the
   version floor SPEC_impl.md's terminal-substrate section defines), finds it in the Homebrew prefixes itself because
   GUI apps do not inherit the shell `PATH`, and refuses to start — naming the binary, the version found, and the floor
   — when none acceptable exists; `FARHELM_TMUX` overrides the choice.
2. **Web interface**: the helm — wherever it runs — always serves a browser UI with the same capabilities, so a helm
   running on a Linux host is fully usable with nothing installed on the client machine. The native app's embedded helm
   serves the web UI too.

The two client forms have the same capabilities: terminal, attachments, lifecycle operations, host registration, and
profile management (profile edits are proxied through the helm to the owning supervisor). They differ only in packaging.
These are the only two faces the helm has — a web UI, or the local app embedding it. There is no remote
native-app-to-helm mode: a helm running on a Linux host is reached through its web UI, period.

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
registers the host — no separate host-side setup. Passwordless SSH is the entire prerequisite: with it in place,
provisioning and everyday operation just work out of the box — reaching supervisors needs no port forwards, no opened
firewall ports, and no address configuration beyond the SSH destination. (The web UI's own loopback-plus-forward story
is separate; see Security.) Nothing the supervisor does requires root: install, updates, and operation all happen as the
SSH user (user-level systemd, files in user-owned directories). If some optional step cannot be done without privileges
on a given host, provisioning says so and continues without it rather than escalating. Before touching the host, the
helm states exactly what it is about to do in concrete terms — the files it will place and where, the systemd units it
will create, and that the supervisor will run persistently and start at boot — and proceeds only on confirmation. The
same transparency applies to updates, which use the same mechanism. V1 provisioning targets Ubuntu only, and only
architectures for which cross-compiled supervisor binaries exist; everything else falls back to the manual path (run the
binary yourself), which always remains available.

Provisioning is idempotent and doubles as recovery: re-running it against an already-provisioned host — including from a
brand-new helm whose registry was lost — detects the existing supervisor and re-registers the host with all its sessions
intact. Losing the helm never strands a provisioned host.

Connections are direct: the helm connects straight to each registered supervisor, over the user's own SSH access —
passwordless SSH from the helm's machine to the host is the requirement, and the helm handles connectivity itself,
transparently. Supervisors listen on no network port. The helm's own machine is a host without registration: its
supervisor (the app's bundled one, or one running beside a Linux helm) is reached locally, no SSH-to-self involved. The
helm manages it through the same discovery-first flow as remote hosts, minus SSH: if none is running, the user is
offered the same automatic setup (user-level systemd on Linux); until one exists, the local host appears with that
offer, not as a phantom unreachable host. There is no relay, no supervisor-to-supervisor connection, and no transitive
aggregation — the helm sees exactly the hosts in its registry. The machine running the helm must therefore be able to
reach every registered supervisor over the user's network fabric (Tailscale, SSH tunnel); a browser only needs to reach
the helm — which in v1 means loopback on the helm's machine, tunneled when the browser is elsewhere (see Security).

The host registry belongs to the helm: each entry is a host's SSH destination. Hosts carry stable identifiers
independent of address: a host's identifier is generated by its supervisor at install time, so it survives address
changes, while wiping and reinstalling a supervisor produces a new host identity whose predecessor's sessions are gone
with the old install. Removing a host from the registry merely forgets it — the supervisor and its sessions are
untouched and reappear on re-registration. Registry entries are editable: an SSH destination can be corrected without
touching the host's identity or its sessions. If a destination turns out to present a different identity than recorded
(a wiped and reinstalled host, a recycled address), the helm says so and asks whether to adopt the new host or fix the
destination — it never silently merges. Two destinations reaching the same identity are the same host, shown once.
Last-known sessions of a host that is permanently gone are disposed of by removing the host from the registry.

Exactly one helm runs at a time. Running several concurrently is unsupported in v1. The invariant supervisors enforce is
at most one attachment per session, last attach wins — so a second helm cannot corrupt a session, but it can seize one,
exactly like any other client taking control. "Exactly one" is an operating assumption, not something enforced; that
invariant is the backstop.

The helm is a plain command-line process too, with the same layering as supervisors: on Linux, v1 ships user-level
systemd units for it, so a reboot of the helm's machine brings the web UI back; on the Mac, the helm lives and dies with
the app.

Agent profiles belong to each supervisor: the invocation has to exist on the host that runs it. Creating a session
offers the target host's profiles, and `--agent` on the spawn CLI resolves against the session's host. Syncing profiles
across hosts is post-v1 convenience, not a v1 requirement. A fresh supervisor is not empty: it ships with editable
starter profiles for Claude Code and Codex, each in a plain and a permission-skipping ("yolo") variant: `claude`,
`claude-yolo`, `codex`, and `codex-yolo`. Integrations are not user-authored — a profile optionally names an agent kind
from Farhelm's built-in v1 catalog (Claude Code, Codex), which selects that kind's status heuristics and
conversation-identity capture; profiles without a kind get generic treatment.

Standard operation must never require falling back to SSH or a separate command line, with three v1 carve-outs:
transport, web-token bootstrap, and starting the v1 Mac supervisor by hand when a Linux helm drives Mac agents. Reaching
a remote helm's web UI takes a user-managed SSH port forward, and obtaining or rotating that UI's token happens on the
helm's machine (see Security) — accepted v1 friction, deliberately outside "standard operation". On provisionable hosts,
install and updates are the helm's job (over the user's SSH access); the complete list of what may legitimately require
manual host-side work is that same pair, on hosts provisioning does not cover. Everything else — session operations,
profile management, directory browsing — must work from the client. SSH otherwise remains an escape hatch, never a
requirement.

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
- Agent profile: defaults to the last-used profile on the target host; if that profile no longer exists, the client asks
  instead of guessing.
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

Advanced options stay hidden by default. Project registration (associating metadata with a directory) is optional
convenience and never a prerequisite.

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

The client supports: create, open, rename, restart, stop, archive, delete.

- **Stop** terminates the agent and its entire process tree — MCP servers, dev servers, and other descendants included.
  Terminal tabs keep running, and the session remains with its terminal still viewable.
- **Restart** relaunches the agent in the same working directory, resuming the session's own conversation where
  supported (see Durability for the exact promise). This is the only relaunch mechanism: the resume offered when opening
  an interrupted session is this same operation, not a separate feature. There is no fresh-restart variant in v1 — for a
  clean conversation, create a new session in the same directory. Restart on a session whose agent is still running
  confirms, stops the agent, then relaunches. Restart reuses the session's terminal when it still exists — the previous
  run's output stays in scrollback — and creates a fresh one when it does not (after a reboot, or on an archived
  session). Restart touches the agent terminal only; terminal tabs are unaffected.
- **Archive** hides the session from the default list and shuts down everything in it — agent and terminal tabs — with
  confirmation when anything is still running. Archived sessions keep their metadata; their terminal contents are gone
  (see Terminal experience). Restart on an archived session unarchives it and recovers the conversation where the agent
  supports resume.
- **Delete** removes the session and its stored state, in any state, terminating the agent and tabs if running — with
  confirmation that says so when anything is still alive.

Process-tree ownership is session-wide. Restart reaps any leftover descendants of the prior run before relaunching —
never alongside them. Stop, archive, and delete reap everything the agent started. An agent exiting on its own does not
trigger a hunt for daemonized survivors; the session's next restart or its teardown does. Operations that need the
working directory — restart, opening a terminal tab — fail with a clear error naming the directory if it has vanished
since creation; the session itself remains, and archive and delete still work.

### Session view

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

One flat list across all registered hosts, with filtering and search by host, directory, agent profile, status, and
title. The list can be ordered by most recent activity, by creation time, or by title, chosen from a control that is
reachable without opening the filter controls; the order someone picks is remembered per client across browser reloads
and desktop relaunches, and most recent activity is what a client shows until someone picks otherwise. A client that
asks the helm for no particular order gets creation time. The filter controls open on demand rather than standing
permanently above the list; while an applied filter's controls are closed, the list says visibly that a filter is in
force, so a narrowed list can never masquerade as a small fleet. No mandatory hierarchy. Agent-spawned sessions (see
below) carry a parent reference usable as a filter, but parentage does not nest the list and implies nothing about VCS
state.

The list always carries a count, and it counts the list you are looking at: archived sessions are outside the default
view, so they are outside its count, and the archive-inclusion switch widens the rows and the count together. That
switch is which list you are looking at rather than a filter you applied, so it does not make the list call itself
filtered. A filter someone typed or chose does, and the count then says how many matched alongside how big the view is.
A list the client could not read to the end says that in the same place, rather than presenting a partial list as the
whole one.

A row shows a session's title, its status (drawn as described under Status), how long ago it was last active, and its
working directory. The host is named on its own line only for a session that is not on the helm's own machine — a fleet
that is mostly local gains nothing from a host word repeated on every row, and the line returns the moment a session is
remote. The working directory and the launch command are shown abbreviated, with their full, untouched values always
available on the row (a tooltip on the web and desktop clients); an abbreviation is never the only place a value is
recorded. Exactly how a row lays out its lines and pixels is an implementation choice, covered in SPEC_impl.md rather
than here.

Per-host connection state is always visible, as a compact per-host indicator naming each host with its current phase;
the full hosts panel — retry, retargeting, removal, profiles — opens on demand rather than occupying the session list
permanently. Sessions on an unreachable host stay in the list from the helm's last-known knowledge (which survives helm
restarts), clearly marked stale, rather than vanishing. Lifecycle operations against an unreachable host are refused
with a clear error; nothing queues for later delivery in v1. Opening such a session shows its metadata — title,
directory, last-known status — behind a clear host-unreachable notice; there is no terminal to show and no pretense of
one. Changes made from any client — creates, renames, stops, deletes, status transitions — appear in all other connected
clients automatically; the agent-spawn behavior below is one instance of this general rule, not a special case.

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

- Full fidelity: colors, cursor movement, alternate screens, resize. If it works over plain SSH it must work here.
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
  session's terminal stays viewable while its host is up, since the terminal outlives the process.
- Opening a session attaches to it — and opening a CLIENT counts as opening a session: with a non-empty fleet, a freshly
  loaded client selects and attaches the session the user most recently had selected there — including after a desktop
  relaunch — falling back to the newest-created non-archived one, so launching the app is itself the deliberate act the
  attach semantics below key off. Opening a second client therefore takes the terminal over exactly as clicking the same
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
- Reconnection is for lost connections only, and the two detaches that are NOT lost connections deliberately stay put. A
  client displaced by a takeover keeps its snapshot and its take-control action rather than reattaching: it was
  displaced on purpose, and a client that came back on its own would fight the one that displaced it. A viewer detached
  for stalling keeps its reason: the wedge is why it was detached, and returning into the same wedge repeats it. Both
  come back the way any client attaches — because someone asks.
- A terminal recovering on its own never TAKES the session. Recovery is unattended by definition — the client was not
  there to be told anything while its connection was gone — so if someone else has attached meanwhile, the automatic
  attach is refused and that client lands where it actually stands: displaced, with the same take-control action any
  other displaced client has. Taking a session over stays a thing someone does on purpose, whether by opening it or by
  asking for it back.
- Selecting text copies it to the system clipboard: a plain drag when the pane has no mouse reporting active, or
  Shift-drag (Option-drag on macOS) to force a local selection when it does — the same modifier xterm itself uses to win
  a selection back from an app that has grabbed the mouse. A terminal program's own OSC 52 WRITE is honored the same
  way, and is the only path that reaches the clipboard for a selection an app under mouse reporting makes for itself; an
  OSC 52 READ is never answered — no program running in a terminal is handed the system clipboard's contents, under any
  circumstance. Every completed selection re-copies, even one identical to what is already on the clipboard. Clipboard
  operations are explicitly best-effort and silent on failure — permission policy, secure-context requirements, and an
  engine's own clipboard behavior are outside this system's control — a deliberate, named exception to the Errors and
  diagnostics section's surface-every-error rule below, not a lapse in it.

## Attachments

Pasting or dropping content into any of a session's terminals — the agent's or a tab's — is classified by flavor, in
precedence order: file references first, then image data, then plain text. A file reference means an actual file object
on the clipboard or in a drag; pasted text that merely looks like a path is still text. Files and images are intercepted
— the client transfers the file to the session's host and inserts the resulting host-side path into the terminal input
at the cursor, so the agent picks it up with no manual copying. Plain text passes through as ordinary terminal input.
Dropped directories are rejected with a visible error in v1. Interception is unconditional for files and images — remote
and local sessions alike, regardless of any native paste handling the agent would have had in a plain local terminal.

Files land in a per-session attachments directory under the supervisor's own data area, never in the working directory —
dropping untracked files into a workspace would be exactly the kind of implicit mutation this system promises not to
make. Attachment files are removed when their session is deleted.

Attachment bytes ride the existing edges — client to helm, helm to supervisor. There is no direct client-to-supervisor
path; a browser never needs to reach any machine but the helm's.

Transfer must not block the terminal: you can keep typing while it runs, and the path is inserted at whatever cursor
position is current when the transfer completes. For a typical screenshot this is imperceptible.

Upload failures must be visible; an attachment must never disappear silently.

## Durability and resume

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

Anything farhelm attaches to an agent launch must be invisible from inside the session when it works AND when it fails:
no output on the agent's terminal, no non-zero exit, no error the agent's own UI can show. A hook that cannot do its job
gives up silently within a bounded time and leaves its diagnostics in farhelm's own state directory, never in the user's
session. The bound is on the part the agent waits for — reading the vendor's payload and reporting the result — because
that is the whole of what can hold the agent up; writing the diagnostic happens afterwards, is best-effort, and is not
itself bounded. The one accepted exception to the invisibility rule is a line the vendor itself prints because of a flag
we pass (Codex's hook-trust warning), which must be documented.

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
environment — any process inside the session may spawn, and that is the point. Spawning targets the session's own host
only in v1. The helm learns of new sessions automatically; they appear in all clients without manual registration or
refresh.

The CLI's contract, since agents will script against it: on success it prints the child session id to stdout and exits
zero, and success means the session exists — a child whose agent then fails to launch still exists, in error or exited
status. Precondition failures exit nonzero with a message on stderr. Only `--cwd` is required; everything else defaults
exactly as interactive creation does (last-used profile on the host, generated title). An optional idempotency key makes
retries safe: re-running spawn with the same key after a timeout or ambiguous outcome returns the existing child rather
than creating another. Keys are scoped to the host and live as long as the child session does. Guaranteed
Farhelm-injected environment: the session id (`$FARHELM_SESSION_ID`) and the per-session credential; other
Farhelm-specific variables are illustrative, not contract. (The user's login-shell environment is separately guaranteed;
see Durability.)

As with interactive creation, spawning launches the agent without an initial prompt in v1; the spawning agent (or the
user) interacts with the child through its terminal.

The parent reference is organizational metadata only and implies no VCS relationship. The control plane creates no
worktree, workspace, or branch as part of spawning — if the agent wants a `jj workspace` first, the agent creates it.

## Errors and diagnostics

- Every failed operation surfaces a concrete, actionable error in the client. A dialog must never close as though an
  operation succeeded when it failed. Remembering desktop selection and sort across relaunches is the one exception:
  persistence is best-effort, and failures are logged but silent because losing next-launch convenience must not turn a
  choice that already took effect into a failed current operation.
- Connection state per host is always visible — at minimum as the session list's compact per-host indicator, with the
  full hosts panel a toggle away; reconnection uses bounded retries followed by periodic low-frequency re-probing, so a
  host that comes back overnight resurfaces by itself. The UI shows which phase it is in either way.
- Logs are available for: the helm, each supervisor, session creation, process/PTY lifecycle, attachment transfer,
  reconnection, and resume attempts.
- Long-lived input/output/paste paths have health checks, so "typing goes nowhere" is detected and reported rather than
  left for the user to infer.
- Mixed versions across helm and supervisors are a normal steady state, since updates are user-controlled. Incompatible
  versions refuse to connect with a clear, actionable error; there is no silent degradation.

## Security

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
  machine (the app UI, or `farhelm helm token show|rotate`), and the browser asks for it once per device and keeps a
  session thereafter. Rotating the token invalidates every device session — that is what rotation is for. The native app
  embeds its helm; that edge is local.
- **Helm to supervisor**: SSH, and only SSH, for every remote supervisor. Passwordless access from the helm's machine,
  as the user, is the requirement; authentication is the user's SSH keys, and supervisors listen on no network port of
  their own. Registering a host means giving the helm its SSH destination — there is no supervisor token to manage. The
  helm's own machine's supervisor is reached locally, no SSH involved.
- **Session to supervisor (spawn)**: the spawn CLI reaches its own supervisor over local IPC only, never the network.
  Its per-session credential is scoped to that one session and dies with it.

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
- Profile syncing across hosts.
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

1. From the helm on the Mac (native app), given nothing but passwordless SSH to a fresh Ubuntu host, provision it in one
   action: supervisor installed and started without root, host registered, sessions operable with no further network
   setup. Also open the same helm's web UI from a browser (token-authenticated).
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
