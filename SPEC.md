# Farhelm product specification

NOTE: This is a product specification. It describes user-observable behavior and deliberately says nothing about
implementation technology (daemon language, terminal substrate, UI framework). Anything labeled a non-goal is out of
scope for v1, not forever. Sections marked "post-v1" describe behavior that must not be precluded by v1 decisions but is
not required to ship.

The problem being solved: I want to run many coding agents (Claude Code, Codex, and other terminal agents) on machines I
control — my Mac and one or more Linux hosts — and supervise all of them from one interface, interacting with each agent
through its real TUI. Existing tools in this space (herdr, superset.sh, various "agent manager" GUIs) each miss some
combination of: durable remote execution, real-terminal fidelity, VCS neutrality, or multi-host aggregation.

## Documentation scope

The README introduction, the docs Overview and their artwork are a short hook that may describe the main, first-class
path. Qualifications for other agents and hosts belong in the guides; a claim that holds on that main path is not a
defect in the introduction. The maintainer's principle is: "The overview is meant to be short and a hook. We cannot be
lawyers and make it 100 pages of caveats. don't change anything, just fix spec."

Published release notes and their screenshots are the historical record and stay as published, except for an absolutely
catastrophically major mistake with huge user impact. The maintainer's principle is: "We don't change unless it's an
absolutely catastrophically major mistake with huge user impact."

## Concepts

- **Host**: a machine that runs sessions. The local Mac is a host; each remote Linux machine or Mac is a host. Every
  host runs a supervisor — at most one per user, and Farhelm operates entirely within one user account per host.
- **Supervisor**: the per-host service that manages sessions on that host — launching agents, owning their terminals,
  receiving attachments, handling spawn requests — and nothing else. It has no UI and no knowledge of other hosts.
  Supervisors are the authority on their sessions, and sessions outlive supervisors (see Durability). Supervisors
  themselves outlive clients and the helm: on Linux under systemd, on remote Macs under launchd (see Topology), and on
  the local Mac because the app starts its supervisor detached, so quitting the app leaves the supervisor running (the
  helm lives in the app and stops with it). Nothing restarts that local Mac supervisor if it crashes while the app is
  closed, and nothing starts it at login; opening the app does.
- **Helm**: the single control-plane process. The user runs exactly one helm, normally the one the desktop app embeds,
  on the Mac or a Linux desktop; a standalone helm (see Topology) serves no UI. It holds the host registry, connects
  directly to each registered supervisor, aggregates their sessions, and serves the UI through its API. The helm holds
  no authoritative session state — supervisors are the authority — but its registry and its last-known view of each
  host's sessions persist across helm restarts, so a helm bounce neither affects sessions nor empties the list.
- **Client**: a UI surface attached to the helm — the desktop app's window. A helm accepts one UI at a time (see
  [One GUI at a time](#one-gui-at-a-time)). Killing a client, or the helm itself, never affects a session.
- **Session**: the unit of supervision. A session has a working directory, a launch, a title, and a live terminal. A
  session is agent-centric: it has one main agent terminal, plus optional additional terminal tabs (plain shells) that
  open in the same working directory. Session metadata — title, parent reference, stop annotations, captured
  conversation identity — is durable and lives with the session's supervisor, so it survives helm loss and
  re-registration. Farhelm avoids storing terminal output except where it directly helps the user, such as a bounded
  excerpt in a startup error (see Terminal experience).
- **Environment** is a disposable machine Farhelm creates for a session from a shared recipe. A Tensorlake environment
  is a sandbox, and a Podman environment is a rootless container on a local or ordinary SSH Linux host. Every
  environment runs its own ordinary supervisor and has a session-owned host entry. A **place** identifies where it is
  created, either the Tensorlake account or a registered container host. Its checkout and files live inside it.
- **Launch**: what a session runs, as the user chose it in the launcher. Every launch has one of two launch kinds. An
  **agent launch** names an agent type Farhelm knows (Claude Code, Codex, and the other harnesses under Topology) plus
  that agent type's own choices, such as model and permissions. Custom permission flags replace Farhelm's permission
  flags and always count as YOLO. An optional wrapper prefix places user-written Bash text before the agent command
  Farhelm composes, without changing its agent type or other choices. Farhelm composes the command, and the command it
  uses to resume a conversation, from those choices when the session is created (and again on Restart with), and stores
  them; Replace and agent clone copy the stored commands rather than composing them anew, while Clone and Replace with
  compose from the choices. A **command launch** is a command line the user writes, with the user's own assertion of
  whether it bypasses safety checks (YOLO). A command launch may also declare that it runs a given agent type, which
  buys that agent type's status reading and conversation reporting, and, with a resume command the user writes, Resume
  (see Creation and Durability). The agent type is the field the two kinds share: required for an agent launch, optional
  for a command launch. This spec also calls an agent type a harness, after the vendor program behind it.
- **Launch template**: a named, partial set of launcher edits. Applying a template is exactly the same as making its
  edits by hand in the launcher, and nothing more: a template sets only the fields it contains, the result can be edited
  further before launching, and several templates can be applied one after another. A session records the launch that
  resulted, never which templates produced it, so editing or deleting a template cannot affect any session. Template
  edits are last-write-wins: two clients editing the same template at once is not a case Farhelm guards, because it is
  one user's rare action, and no optimistic-concurrency check on template writes is wanted. The same holds when a GUI
  save lands after an agent's or the command line's edit (One GUI at a time). An agent's own template write is the one
  exception: it is refused if the template changed while its card waited (Agent-spawned sessions).

## Topology

One control plane, one face: the **desktop app**, `farhelm-desktop`, a native app for macOS and Linux (Wayland). It
opens the UI in its own window and runs the helm. On the Mac it also starts a local supervisor beside it, so the Mac
itself is a host; the NOTE below covers Linux. The helm is embedded in the app and serves only that app's window, on a
loopback port chosen for that launch. The window still talks to the helm only through the helm's API (its UI channel,
terminal streams, and plain HTTP for file transfers), never through the helm's internals, even though both run in one
process. Keeping that split is what leaves a remote helm possible later.

There is no web interface: Farhelm serves no browser UI. A remote helm, meaning a desktop app attached to a helm on
another machine, is not offered now. Decided by the maintainer on 2026-11-12, when the GUI moved to a native app on
macOS and Linux: "a large part of the point is to drop the entire browser mess", while "we DO still want to keep the
'helm' concept ... we may still support running a remote helm". A helm started without the desktop app, by
`farhelm helm run` or through the systemd units `farhelm helm setup` writes on a Linux machine, still manages its hosts
and serves agents and the command line, but no UI attaches to it. Whether that standalone helm stays is an open
question.

NOTE: The Linux desktop app is new. How it runs and keeps its local supervisor is not specified yet; the Mac desktop
app's rules below apply to the Mac.

The desktop app has the same capabilities on both platforms: terminal, attachments, lifecycle operations, host
registration, and launch-template management in the helm-owned catalog.

The Mac is not architecturally special: it runs a normal supervisor that any helm — the app's embedded one, or one on a
Linux host — can register and drive, and the helm that registers it then owns it (see the ownership rule below). The
supervisor is a plain command-line process on every platform: `farhelm supervisor run` in a terminal and you have a
working host — that path always exists, on any platform, and is how you try Farhelm without a bunch of fuss. System
integration is layered on top of that, not baked in. On Linux, v1 ships that layer as user-level systemd units
(auto-start at boot, which the durability promises assume) — never system units, never root. The Mac desktop app starts
its local supervisor and leaves it running when it quits; nothing starts that local supervisor at login or restarts it
after a crash before the app is opened again. A remote Apple-silicon Mac set up over SSH instead gets a per-user launchd
job, which runs the supervisor right away and restarts it after a crash. After a reboot, reconnecting over SSH does not
load the job; choosing Update in the host menu starts it. Automatic loading at screen login is unverified. The desktop
app has no helmless mode in v1: launching it alongside a standalone helm would start a second helm — the unsupported
state.

Remote provisioning: the helm can set up a remote supervisor itself, in the style of `herdr --remote`. There is at most
one supervisor per user per host, and adding a host is discovery-first: the helm connects over SSH and looks for that
user's supervisor. If one is already running — say one the user started interactively by hand — the helm uses it as-is;
it never restarts or replaces a running supervisor. If none exists, the user is asked whether to set one up
automatically, and on confirmation one action installs the supervisor binary, sets up its per-user background service,
and registers the host — no separate host-side setup. The setup question names the concrete files and systemd unit or
launchd job it will write. On Linux it says the supervisor runs persistently and starts at boot (or at login when
lingering is refused); on a remote Mac it names the job's label, says it runs now and restarts after a crash, and states
the measured reboot behavior. The user may choose to skip this setup question for later additions from the same helm;
that choice is shared by every client and does not change discovery of an already-running supervisor. Passwordless SSH
is the prerequisite on the HOST side, plus, on the helm's own machine, access to the configured release source
(get.farhelm.io by default) or a staged payload directory (a developer-facing, best-effort option; see
[Supported host setup](#supported-host-setup)). With that in place, provisioning and everyday operation just work out of
the box — reaching supervisors needs no port forwards, no opened firewall ports, and no address configuration beyond the
SSH destination. Downloads go directly to that configured release-asset source — get.farhelm.io by default, whose
archive links redirect to the project's GitHub releases; neither it, GitHub, nor a configured mirror is a relay or
rendezvous service for Farhelm sessions or connections, and the no-relay guarantee below still holds. Nothing the
supervisor does requires root: install, updates, and operation all happen as the SSH user (user-level systemd on Linux
or launchd on a remote Mac, files in user-owned directories). If some optional step cannot be done without privileges on
a given host, provisioning says so and continues without it rather than escalating. Before touching the host for initial
setup, the helm states exactly what it is about to do in concrete terms — the files it will place and where, the
background service it will create, and its persistence behavior — and proceeds only on confirmation. Remote updates use
the same one-use plan mechanism behind the same authority, but the user's Update click or explicit automatic-update
opt-in is the authorization: no plan is shown for confirmation. Update never downgrades a host: it installs the helm's
own build, so a host whose supervisor reports a newer build is refused with both versions named, and the user updates
the helm instead. It also refuses a host whose supervisor reports a database schema newer than the helm's build can
open, which happens after a development build ran there; that host needs a build at least that new. A supervisor below
this release's upgrade floor is also refused, with a manual recovery remedy: stop its Farhelm supervisor service, then
remove its dedicated Farhelm installation directory. The remedy names the executable the host reported; its parent may
be a shared or custom binary directory, in which case only Farhelm-owned files are removed and the shared directory
stays. Then remove the host from Farhelm and set it up again. Its data directory can stay, without a promise that the
new release can use every feature's old data, or the user can delete it too if they accept losing it. While an update is
running, its host row stays folded and shows `Updating…` until a progress snapshot is available, then shows the current
step, completed-step count, and client-measured elapsed time inline in the status spot. Hovering that status opens the
same card as a working host in the attention list: the host's mark and name, operation and completed-step count with
elapsed time, every step with its status and explanation, the current step highlighted, and any degraded or failed
step's reason. Connection detail follows the steps while the host is not connected, including the attempt and last
connection error, and the run's own message follows when present. In a short window the step list shrinks, keeping the
current step in view as it changes. If connection detail or a run message cannot fit beside a readable current step,
only the oversized text becomes a preview of whole lines from its end and the card says `Full details in Hosts`.
Messages that fit stay whole. The card remains non-interactive; opening Hosts through the host identity gives access to
the whole diagnosis. This is the narrow exception to full detail on hover. The card uses the shared hover delay and
beside-the-sidebar placement, is absent on touch, and adds no keyboard stop to the status label. The sidebar is too
narrow to show a long step name, let alone the rest of the run. Reduced-motion settings replace the animated indicator
with a static one. Remote binary upload appears as a separate step before installation, so the user can tell when
network transfer is still underway. The row and its hover use the helm's readable names for the same steps in the plan,
with completed work checked green, the running step pulsing amber (static under reduced motion), pending work grey,
unchanged work grey, degraded work as an amber warning, and failure red with the helm's reason. Success returns the
status spot to its normal label and leaves the row folded unless global details are on. Failure or an uncertain outcome
still expands the row so its step list and diagnostic remain visible in Hosts. When setup or Update fails or has an
uncertain outcome while Hosts is closed, the host's rail avatar shows a warning mark until Hosts is opened or a later
run of that same action succeeds. Starting a retry does not acknowledge the warning, and an unrelated successful action
does not clear it. Setup recovery by discovery can resolve the same failed setup run. Hosts never opens by itself. The
Hosts header's `Update all` action makes the same authorized Update request for each remote host whose individual Update
action is available at the click. A host already busy with setup or another run is skipped rather than queued for a
later update; the local host is excluded. Each remote host keeps its own validation, progress, and result, so one
failure does not hide or delay the others. Having an available Update action does not itself mean the host needs an
update: the rail's update badge and the attention list's Update rows require a compatible older build or a lower
protocol, as well as an available remote Update action.

**Update remote hosts automatically** in Settings is off by default for new and existing installations. While on, the
helm starts that same authorized Update for each host with an update by the badge's test, including required protocol
updates, even with no interface open. Enabling it also considers hosts that already need an update. The local host, busy
hosts and newer builds are excluded; busy hosts are reconsidered when their work finishes. Progress and failure are the
ordinary Update progress and failure in Hosts. A failed automatic attempt is not retried until that host's connection
incarnation changes after failure or the helm restarts; switching the preference off and on does not clear this memory.
A planning refusal starts no run: the helm logs the reason and remembers that failed attempt under the same no-retry
rule. The badge remains, and a manual Update from the host menu can show the refusal. There is no separate automatic
failure surface. Turning the switch off prevents new automatic starts, including planning still in flight; it does not
cancel an accepted run.

V1 provisioning targets Linux hosts with a usable systemd user manager on x86_64 or aarch64, and Apple-silicon Macs over
SSH. An Intel Mac has no payload and cannot be set up this way. On Linux the distribution is not a requirement — nothing
provisioning does is distribution-specific — so the plan names whichever one it found rather than refusing; CI exercises
Ubuntu. Hosts whose basic command-line tools come from BusyBox rather than GNU coreutils or its usual replacements are
not a supported target: Farhelm accommodates them only where that takes a trivial change, and spends no further
complexity on them. On Linux, no usable systemd user manager or an architecture with no payload falls back to the manual
path (run the binary yourself), on the best-effort basis described in [Supported host setup](#supported-host-setup).

Setting up or updating a remote Linux host neither needs nor installs tmux. Earlier releases installed their own tmux
build as `~/.local/lib/farhelm/tmux`, and its directory comes first on the `PATH` sessions start with, so a `tmux` typed
in a session there ran that copy. Every Linux setup and Update plan therefore removes that file as a step of its own,
after the supervisor is started or restarted, and a `tmux` typed in a session afterwards no longer runs that copy. A
copy that an update before 0.18.0 put beside `farhelm` in a shared folder such as `~/.local/bin` is not removed, since
it may be the user's own, as 0.18.0's release notes said. A host without the file finds the step already done. A removal
that fails ends the step with a warning that says the file may have been left in place, and the setup or Update still
completes, since the new supervisor already runs.

A remote Mac's setup installs `farhelm` in `~/.local/lib/farhelm`, keeps its state in `~/.local/state/farhelm`, and
writes `~/Library/LaunchAgents/io.farhelm.supervisor.plist` for the SSH user's launchd domain. There is no macOS version
floor: inspection refuses a Mac missing a command the path needs, naming that command. Setup, Update (manual, Update all
and automatic) and uninstall are unsupported when the Farhelm desktop app is installed in `~/Applications/Farhelm.app`
or `/Applications/Farhelm.app`, and the refusal says so without suggesting starting a supervisor by hand. Discovery
still adopts a supervisor already running there, including the app's, without restarting or replacing it. The desktop
app's own local supervisor is unchanged.

Tensorlake's prepared snapshots include the login-manager and lingering preparation: Farhelm uses passwordless sudo to
start the login manager and enable lingering for the sandbox user before installing its supervisor in the snapshot.
Ordinary setup recovery on an existing sandbox still performs that preparation before inspection. Every recipe's
snapshot build checks first that its base boots systemd as its init system. Setup refuses, with an explanation, a
sandbox where systemd is not running, such as one created from an image a recipe reused before recipes always built
snapshots.

Provisioning is idempotent and doubles as recovery: re-running it against an already-provisioned host — including from a
brand-new helm whose registry was lost — detects the existing supervisor and re-registers the host with all its sessions
intact. Losing the helm never strands a provisioned host.

Removing Farhelm from a remote host is a host action too, the uninstall item in that host's menu, with no command-line
counterpart. The helm plans the removal over SSH and shows it before touching the host: the supervisor's user service
and its unit file or launchd plist go, any terminal hosts are ended, and Farhelm's private lib directory with the binary
is removed, with any tmux binary an earlier release left there. Uninstall has no tmux step of its own: stopping the
supervisor's service ends whatever still runs under it, an earlier release's private tmux server included, but one that
outlived a supervisor stopped before uninstall (on a host never updated to a release that ends it at startup, say) keeps
running, which the maintainer accepted on 2026-11-11. Each file or directory is named by its path on the host (the
terminal hosts are named without their socket paths); terminal hosts that cannot be confirmed ended keep the lib
directory in place and the host listed, for a retry; the host's Farhelm data directory stays, named by its path, with a
note that deleting it by hand removes the data too. Nothing changes until the user confirms. Setup and uninstall
planning show their action in the folded row while the helm prepares a dialog; neither action changes the global details
switch. Confirming closes the dialog, and the running row expands once to show the helm's steps on the same rail as the
plan. The user may fold that run, and it stays folded until another run begins; the inline status still says setting up
or uninstalling and shows the step count, current step and elapsed time. Success folds the row, while failure opens it
for the step and its reason. It refuses, naming them, while the host has any session that has not ended (an unknown
status counts) or any open terminal tab, and checks again at confirmation. A session or tab started on the host while
removal runs may be ended with it: the confirmation-time check is sufficient, though an implementation may check more
strictly. It also refuses a host it cannot check because it is not connected, a host whose supervisor unit
`farhelm helm setup` manages there (that host's own `farhelm uninstall` removes it), a supervisor running from anywhere
other than Farhelm's lib directory, and a lib directory that is itself a symbolic link (removing it would remove only
the link), naming where the link points; a link further up the path, such as a linked home directory, is not refused.
Removing the host from the list, which never touches the host, stays available whatever its layout, and the refusal
points to it as the way to stop using the host without uninstalling; when the host list already shows incompatible
versions, an unverified or changed identity, or two entries reaching the same machine, uninstall's refusal also explains
the remedy and offers Remove. Success removes the host from the list, cached sessions included, as removing it does, and
the window that confirmed shows a one-line removal notice. Its hover text names the kept data directory, a copy control
copies that path, and × dismisses the notice. A run that fails partway keeps the host listed with its steps showing what
is left, and choosing uninstall again continues from there. Such a retry may go ahead without a connection only once the
unit file or plist is gone, and only while the systemd unit is not running or the launchd job is not loaded, since a
running or startable supervisor may have sessions the retry cannot check. A loaded-but-idle launchd job still counts as
able to start, so deleting its plist alone does not permit an offline retry. Linger on Linux is left as it is. The
helm's own machine has no uninstall in the panel: Farhelm there is removed with `farhelm uninstall` (see
[Uninstall scope and interaction](#uninstall-scope-and-interaction)).

The helm connects to each registered supervisor through local IPC, the user's own SSH access, or a Podman exec pipe into
a container. A remote container uses SSH to its container host and Podman exec there. Passwordless SSH from the helm's
machine remains the prerequisite for remote hosts, and the helm handles connectivity transparently. Supervisors listen
on no network port except IPv4 loopback ports claimed by a helm for an environment host's configured recipe tunnels.
Those listeners belong to the claiming connection and close when it ends. The helm limits these claims to environment
hosts. The supervisor's transport-independent protocol does not know the provider or host kind. The helm's own machine
is a host without registration: its supervisor (the desktop app's managed local one, or one running beside a Linux helm)
is reached locally, no SSH-to-self involved. The helm manages it through the same discovery-first flow as remote hosts,
minus SSH: a supervisor that answers is discovered and registered like any other, and the panel never touches the unit
file that runs it. What the panel does NOT do on the helm's own machine is install one. On Linux that job belongs to
`farhelm helm setup`, which writes and owns the helm's and the supervisor's systemd user units; the local row says so
instead of offering to install, so a machine has exactly one thing writing its units. Until a supervisor exists the
local host appears with that instruction, not as a phantom unreachable host. There is no supervisor-to-supervisor
connection or transitive aggregation. A container host's supervisor does not relay the container's protocol. The helm
sees exactly the hosts in its registry, including the session-owned hosts described under
[Sandbox sessions](#sandbox-sessions) and [Container sessions](#container-sessions). Those entries participate in
ordinary host management and session aggregation, with their permanent rail icons hidden. The machine running the helm
must therefore be able to reach every registered supervisor over the user's network fabric (Tailscale, SSH tunnel).

The host registry belongs to the helm. An ordinary remote entry carries its SSH destination, and an environment entry
carries its provider binding and place. A host can carry an optional alias, shown in place of its destination everywhere
except the host details view, which keeps the real destination visible; aliases are unique among host display names, and
the local host can carry one too.

Whenever a surface uses a host's **short name**, it derives it from that host alone: an explicit alias exactly as
stored, otherwise `Local` for the local host, otherwise the first label of its SSH hostname. For that last case it
removes `ssh://`, a user prefix through the last `@`, brackets, and a port only in an SSH URI or after a bracketed
address. IP addresses stay whole, including IPv6 scope suffixes. A dot-less name stays as it is; a leading dot keeps the
whole remainder instead of producing an empty label. Short names may coincide. They are display only, never a
host-resolution key, and deriving one never stores an alias or changes the helm's display name. If the derived form is
empty, the UI shows the full display name instead, including for a newer host kind with no destination.

The add host dialog takes an optional alias beside the destination, and the host is added with it. Adding a destination
that is already a host entry is refused, naming that entry, before anything contacts the destination: it is almost
certainly a mistake, and an existing entry is set up again from its own row instead. What counts is the destination
exactly as typed, plus the identity check below that refuses a second entry reaching a machine another entry holds.
Hosts carry stable identifiers independent of address: a host's identifier is generated by its supervisor at install
time, so it survives address changes, while wiping and reinstalling a supervisor produces a new host identity whose
predecessor's sessions are gone with the old install. Removing a host from the registry merely forgets it — the
supervisor and its sessions are untouched and reappear on re-registration. Environment removal instead destroys its
resource, as described below and under Container sessions. Registry entries are editable: an SSH destination can be
corrected without touching the host's identity or its sessions. Every host, the local one included, has a settings
dialog in the GUI that holds its destination (SSH hosts), its alias, a chosen icon and color, its checkout folder,
whether it starts YOLO sessions without asking, whether its sessions manage sessions on this same host without asking,
whether it runs `farhelm` commands from its sessions without asking, and separate permissions to open public and
local-network links without asking (see Agent-spawned sessions). The header and footer, including the close controls and
the note that the checkout folder needs an explicit Save while other changes save as you make them, stay visible while
only the settings between them scroll. Its **Checkouts** section sits between Appearance and Permissions, for every host
kind. An empty folder uses the default from Settings → Checkouts, shown as a placeholder; **Use default** clears a saved
override. Saving checks the path's shape without contacting the host. Changes apply to new checkouts; existing ones stay
where they are. A remote host offers sixteen icons, with the cloud as the default, and the default foreground or one of
seven identity colors: lavender, orchid, teal, steel, sand, copper and red. The local host starts with the laptop in
red, but offers the same choices. The choice is kept by the helm and shared with every client. It tints the icon, and
the host's terminal frames when **Color terminal frames by host** is on, never a status or the host name. Red identifies
a host and says nothing about its state. New Tensorlake and container hosts start with YOLO launches allowed without
asking. Other hosts ask first, and every host asks before carrying out an acting `farhelm` command from one of its
sessions. **Manage sessions on this host without asking** and both link permissions start off on every host, Tensorlake
sandboxes included, until the user explicitly turns them on, including hosts that existed before the settings did.
Adopting a new identity for a host, described next, resets all five permissions to asking: each is a judgment about the
install the user knew, and a new identity, whether a reinstall or a different machine, is one they have not judged. If a
destination turns out to present a different identity than recorded (a wiped and reinstalled host, a recycled address),
the helm says so and asks whether to adopt the new host or fix the destination — it never silently merges. An entry that
reaches a machine another entry already holds connects nothing: it says which entry holds the machine, by name, and asks
the user to remove that entry or change this one's destination and then press Retry. Farhelm never connects two entries
to one machine and never resolves this on its own. Last-known sessions of a host that is permanently gone are disposed
of by removing the host from the registry.

A host that is a Tensorlake sandbox Farhelm created is the exception to two of those rules, because Farhelm owns the
machine and not only the entry: removing it is not merely forgetting it, and its destination cannot be corrected. A
Tensorlake host can instead belong to a sandbox session; it follows these rules except where
[Sandbox sessions](#sandbox-sessions) gives sessions their own controls and consequences, including the failed-Replace
case where several sessions remain in one sandbox. Removing a sandbox Farhelm has recorded creating terminates it,
deletes every snapshot taken from it (suspend snapshots outlive their sandbox and keep billing), and forgets the entry
only after both succeed. A removal Tensorlake refuses, or one made without a saved API key, leaves the host listed with
the reason, and removing it again finishes the job. A row with neither a saved sandbox ID nor an outstanding create
request is removed without touching a same-name sandbox. This also applies when a legacy installation or power loss lost
both records: any leftover is handled explicitly. The hosts panel calls this **Delete sandbox** instead of Remove,
offers no Uninstall for these hosts, and always asks first with a plain dialog naming the sandbox and saying its files
and sessions will be gone; `Remove hosts without asking` never skips it. An agent's `farhelm agent host remove` of such
a host is refused: only the user deletes a sandbox. Before each connection attempt such a host asks Tensorlake whether
its sandbox is running, and never while it is connected. A sandbox that is suspended, by Tensorlake's idle timeout or
anything else, shows the host as **Suspended**, a grey state, and Farhelm does not dial it: a new ssh connection to a
suspended sandbox is refused, and a new request on an old one hangs. Its details say since when, and whether Tensorlake
suspended it on its own. Farhelm checks again once per re-probe interval and connects by itself when the sandbox runs
again; it never resumes a sandbox on its own. A sandbox Tensorlake no longer has makes the host unreachable with the
advice to delete it. When Tensorlake cannot answer at all (no key, an outage), Farhelm dials anyway rather than cutting
off a sandbox that may be running.

The hosts panel offers **Suspend** for Tensorlake hosts unless they are suspended, suspending or resuming, including
when the supervisor is unreachable. **Resume** appears in both the menu and the suspended row. Suspend asks first when
an agent on that host is working, its status is unknown or stale, or activity cannot be checked completely. Immediate
Suspend requires a connected host after the activity read and a complete listing with conclusive non-working statuses.
An offline or unknown host asks even when its cached listing is empty. Suspension preserves running processes and
sessions. A session on a suspended host says **&lt;host&gt; is suspended; this session continues when it resumes** and
offers **Resume host**. Launching on a suspended host resumes it first and waits for a fresh connection. While another
operation on that host (a Suspend or Resume from elsewhere, setup, an update) holds it, the launch keeps asking within
the same minute it allows for the connection, and a Resume that finishes elsewhere meanwhile is enough. If the host is
suspended again while the launch waits, the launch fails at once and says so. Any other refusal fails it at once. For a
fresh GitHub checkout, the folder is allocated once the host is ready and the session starts without a second press.
Only a fresh preview on the new connection can authorize that create. Farhelm does not suspend automatically.

An explicit Suspend shows **Suspending…**, with a spinner and how long ago Tensorlake was asked, until Tensorlake
confirms suspension. Only then does the host show **Suspended**, with Farhelm's own completion time. A failed or
timed-out request shows its error and reconnects rather than claiming the sandbox is suspended. Resume similarly shows
**Resuming…** during the Tensorlake call, then **Connecting…** while a fresh connection is established. Neither
transition offers a competing lifecycle action or host setup/update. Repeating Resume on an already-running, connected
sandbox preserves its live sessions' connections.

The Suspend confirmation is titled **Suspend &lt;host&gt;?**, with its explanation below the heading and **Cancel** /
**Suspend** in the footer. Its close control, Escape and Cancel dismiss without suspending. The sandbox name, recipe and
resources use the normal subdued details styling in Hosts.

The session launcher shows its own **Resuming** preparation through both the Tensorlake call and reconnection, including
quick launch. Both existing-directory and fresh-checkout launches say the session starts when the host is ready. A
failed preparation says the session was not launched. A host another surface is suspending or resuming remains
unavailable to this launcher.

Environment supervisor connections use a two-minute input-silence limit. The helm sends a lightweight heartbeat every 30
seconds independently of session refreshes or storage work. If the helm vanishes while the gateway keeps its connection
open, the remote helper exits so that its leaked SSH session cannot keep the sandbox running indefinitely. Ordinary SSH
hosts and connections from older helms have no such deadline.

**Add host** offers **SSH** and **Tensorlake sandbox**. Podman containers are created only for sessions through the
launcher. A sandbox needs a Name and a saved recipe, and **New recipe…** opens the full recipe editor inside the dialog
and saves it under its name. Without an API key, the form points to Settings → Tensorlake. The inline editor opens at
**New recipe**, focuses Name and keeps Cancel / Save in the dialog's pinned footer; it has no Recipes back button. Help
under Name distinguishes the displayed host name from the generated `farhelm-host-…` sandbox name. Recipe uses the same
control styling as Name and shows the selected recipe's size, base, package count and tunnel count beneath it. Continue
opens the standard setup confirmation, titled **Create Tensorlake sandbox · &lt;Name&gt;**, with the size beneath the
title, numbered setup steps and Copy plan in the body. One callout says the sandbox bills while running, links to
Tensorlake pricing and explains the 24-hour idle-suspend limit on Usage Credits. **Back** returns to Name and Recipe
without creating anything; the next Continue obtains a fresh plan. Only **Create Tensorlake sandboxes without asking**
skips this confirmation, including for setup reruns; the ordinary host-setup preference does not, and sandbox
confirmation never changes that ordinary preference. Cancelling retires the displayed creation offer immediately. The
host appears as soon as its setup run is accepted, before a snapshot is built or a sandbox is created, and shows
progress in the usual setup view. Its details show the sandbox name, recipe, CPU and memory. New Tensorlake hosts start
with **Start YOLO sessions without asking** on; the other four host permissions remain off.

Setup builds the recipe's snapshot if missing (or joins a build already running), ensures its prepared snapshot, and
creates the named sandbox from it at the recipe's size with the plan's maximum idle timeout and automatic resume off.
The supervisor starts on that sandbox's boot; setup connects to it and delivers the host's credentials without
installing Farhelm again. **Prepare snapshot** appears only when preparation is needed, with the explanation **Only the
first time, and then at most once every 24 hours or after Farhelm updates.** The explanation stays visible while
waiting, including in folded progress. Every base uses this path for new hosts. Rerunning setup against an existing host
keeps ordinary user-manager preparation, inspection and installation, and Update keeps its ordinary installation path.
Rerunning an interrupted initial create first recovers that create's saved installation facts, rather than using the
recipe's latest prepared snapshot. Hosts created before recipes always built snapshots still skip snapshot building on
setup recovery. Claude Code's authentication is merged into its settings' environment; Codex gets a Farhelm model
provider or subscription login. Unrelated settings are preserved and malformed files are refused. None clears the
managed authentication keys without creating an otherwise absent settings file. Credential delivery also runs before
each connection becomes usable, and when a named token is replaced. Updates do not independently reapply these settings.
Token-bearing settings files are private from the start of upload. A sandbox that already contains Farhelm supervisor
state in a user-supplied base is refused during preparation, where the recipe row shows the failure, or on first
ordinary setup: it would copy another host's identity. Prepared snapshots carry no supervisor state, so each clone
creates its own identity on boot. Farhelm does not connect to a supervisor whose copied state it refused, including
after the helm restarts. Failed setup keeps the host and its sandbox listed; rerunning continues against the same
sandbox. When a create's outcome is unknown and the sandbox is not yet visible, setup and Delete sandbox keep the host
listed until that request settles. A later retry finds and cleans up a sandbox that appeared after the failed reply;
absence settles only after 15 minutes from the create request.

Confirmed 2026-10-10: a host entry is one logical host from the user's point of view. Correcting its destination, or
adopting a new identity for it, changes the machine or installation behind the entry, not which host it is to the user;
a user who wants a different host replaces the entry instead. Approvals, settings changes and confirmed operations
therefore follow the entry. One still in flight when the destination changes or a new identity is adopted (an approved
stop, restart, rename, create or clone, an "Always allow" answer, a change to the setting for running `farhelm` commands
without asking, a confirmed uninstall) may be carried out against the entry's new machine, and Farhelm adds no check of
the installation identity to prevent that. Agent host edits and removals are narrower: their cards explicitly show the
destination, so a destination changed during that wait is refused, as specified under Agent-spawned sessions. A
destination change that does not adopt a new identity keeps the host's settings, including on a host that was never
contacted. The identity rules above still hold: adopting a new identity resets all five permissions to asking, and the
New dialog's default host and recent setups stay tied to the install the user saw.

Exactly one helm owns each supervisor, and every session on it. The helm that registers a host owns that host's
supervisor and all of its sessions from then on, and a helm that restarts, updates or reconnects is still the same helm.
Sessions and supervisors do not move between helms. A second helm reaching a supervisor another helm owns is outside the
design, not a case the rest of the system tolerates, and no behavior is specified or implemented on the assumption that
it might happen. Policy lives in the owning helm: approvals of agent actions, each host's permission settings, launch
templates and the helm's other catalogs are that helm's, and the host's supervisor answers to that helm alone. Running
several helms at once is unsupported in v1. Taking a host over from another helm, if it is ever supported, will be an
explicit operation the user asks for, never the side effect of a second helm connecting. Adding a host again from a
brand-new helm whose registry was lost (provisioning doubles as that recovery; see Remote provisioning above) is the
replacement Farhelm supports today: the new helm registers the host and owns it from then on.

NOTE: ownership is stated, not enforced. Nothing yet stops a second helm from registering a host another helm owns, or
from driving its sessions once it has. Enforcing it needs each host to store the identity of its owning helm, refuse
connections from any other helm, and make taking a host over, the lost-registry case above included, an explicit user
operation. The supervisor's one-attachment-per-session rule (Terminal experience) is about the clients of one helm and
is unchanged: a second client attaching a session visibly detaches the first, which is a takeover between clients, not
between helms.

The helm is a plain command-line process too, with the same layering as supervisors: on Linux, v1 ships user-level
systemd units for it, so a reboot of the helm's machine brings the helm back; in the desktop app, the helm lives and
dies with the app.

Launch templates belong to the helm: one catalog applies to every host the helm manages, while a command a template
carries still has to exist on the host that runs it. Farhelm ships no built-in templates; the agent types themselves are
what a release supplies. Integrations are not user-authored: an agent type selects Farhelm's own status reading and
conversation-identity reporting for that harness, and a command launch with no declared agent type gets generic
treatment.

Confirmed 2026-10-10, extended 2026-10-11: a template's session name, launch command and resume command cannot contain
control characters or invisible formatting characters. Saving such a value is refused with a clear message, from the
GUI, the CLI or an agent alike, because the template editor's single-line fields can neither show nor keep them. A
template saved before this rule still launches, but saving any change to it is refused until those fields are fixed.

Recipes belong to the helm in one shared catalog under Settings → **Recipes**. They describe the environments Farhelm
creates for sessions and the Tensorlake sandboxes it adds as ordinary hosts. A recipe has a base, optional Ubuntu
packages, Claude Code and Codex authentication choices, an optional named GitHub token, and outbound tunnels. Its size
(CPUs, memory in GB and disk in GB, whole numbers) applies only to Tensorlake. Managed Ubuntu supports both providers,
using `tensorlake/ubuntu-systemd` for Tensorlake and a digest-pinned, fully qualified Ubuntu image for Podman. A
Tensorlake image name or snapshot ID supports Tensorlake only. A container image reference and login user support Podman
only. Unsupported recipes remain visible but disabled with a reason. Farhelm builds on every supported base, adding
packages, Claude Code and Codex before saving the software snapshot or image used for hosts or sessions. Each endpoint
is a base URL plus an optional named API endpoint credential from Settings → Credentials. A tunnel makes an environment
loopback port reach a host and port as seen from the helm's machine. A recipe's name is lowercase letters, digits and
hyphens, starts with a letter, does not end with a hyphen, is at most 32 characters, and never changes: hosts refer to
their recipe by name, and renaming means creating a new recipe. Creating a recipe under a name that exists is refused
rather than overwriting it, and deleting a recipe a host uses is refused. Endpoint tokens belong to the credential
catalog, and recipes store only their names. Create and update refuse missing or wrong-kind credentials; deleting a
credential a recipe uses is refused. The endpoint picker offers None and endpoint-token names, preserves the selected
name across URL changes, and points to Settings → Credentials for entering or replacing values. An unavailable draft
choice stays visible until the user chooses another or None; it cannot be saved as a missing reference. Upgrading drops
old recipe-embedded tokens rather than importing them; the user re-enters them in Credentials. Edits to size, base and
packages affect only hosts created afterwards (base and packages after the next snapshot build); tunnels and credential
selections are read when a host connects or is set up. Replacing a named endpoint token, agent login or GitHub token
pushes its new value to hosts already using it, including a connection still being acquired. Pending recipe edits do not
change that connection's selections. Other hosts receive the replacement before their next connection becomes usable.
Each host serializes these deliveries and remembers successful settings independently, so an unchanged reconnect does no
remote file work and a later file's failure does not undo an earlier success. Delivery failure leaves the connection
usable, makes the host's status and navigation mark amber, and shows a sanitized reason in its details. The next
connection or credential replacement retries; there is no periodic retry. Saving a replacement commits its value first,
waits at most the usual SSH command timeout for delivery (including lock waits), and reports
`Saved; not delivered to <host>: <reason>` for failures. Ending the connected phase cancels its pending pushes before
further transport work. The list shows each recipe's name, supported providers, base, package count, tunnel count and
build status per place, with Rebuild per place. Size is labelled **Tensorlake sandboxes only**. The editor's Software
block offers managed Ubuntu, Tensorlake image and snapshot bases, and a container image with its login user, each
stating its supported providers, plus Ubuntu packages for every base. The custom-base note explains that the base must
run systemd as its init system, allow passwordless sudo, be Ubuntu or Debian with apt, and include bash, curl and git.
The build checks these requirements first and stops with the missing requirement. Help explains that Farhelm then
installs Claude Code and Codex with their official installers and saves a snapshot or image at that place. The editor
replaces the list in place. Leaving it with unsaved changes, whether by its own Back or Cancel or by closing Settings
with Escape or a close control, asks whether to save, discard or keep editing, and saving or discarding then completes
the close. Delete asks for confirmation. A list refresh that fails keeps the rows shown, says they may be out of date,
and offers **Retry loading**.

Each agent's recipe choice is **None | API endpoint | Subscription**. API endpoint requires a base URL and offers named
endpoint tokens; Subscription requires a named login for that agent from Credentials. Switching modes retains each draft
but saves only the selected choice. Claude Subscription writes `CLAUDE_CODE_OAUTH_TOKEN` in the host's usual
`~/.claude/settings.json` environment and removes `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN`. Claude API endpoint
removes that subscription token; None removes all three keys. Other non-credential settings remain intact, and Farhelm
writes no first-run or onboarding metadata. Claude may show its first-run setup once on a new host, which the user
completes. Codex None and Subscription remove `model_provider` only when it selects `farhelm`, and remove
`experimental_bearer_token` from that provider table, preserving the table and unrelated configuration.

For each agent, credentials belong either to Farhelm (API endpoint or Subscription) or to the base image and host
(None). When Farhelm supplies credentials, credentials for that agent already in the image or host configuration give
undefined results.

Codex Subscription atomically writes the selected login to the host's usual `~/.codex/auth.json` with mode 0600 only
when its name or revision differs from the last successful login delivery. Codex owns and refreshes that file
afterwards; Farhelm never reads it. The remembered login survives switching to Endpoint or None, so returning to the
same saved login leaves refreshed bytes alone. Switching away leaves `auth.json` on the host; signing out in Codex
removes it.

The recipe's **GitHub** block selects None or a GitHub-token name from Settings → Credentials. Farhelm atomically
replaces one private token file in that host's supervisor state directory. The matching supervisor reads it afresh at
every host-terminal agent spawn, terminal-tab spawn and Restart relaunch, injecting `GH_TOKEN` before the login shell so
managed checkout clones see it too. It is never persisted in a session launch or passed on a command line. None removes
only this owned file; ordinary host environment and manually configured Git authentication remain untouched. Existing
processes keep their environment. Older supervisors ignore the file and need setup or Update.

Farhelm-built snapshots include Ubuntu's `gh` package. First GitHub-token delivery configures gh's own global Git
credential helper for github.com without supplying a token to that command. Hosts created from reused images before
recipes always built snapshots, and hosts using older snapshots, need `gh` installed for private HTTPS clones. If it is
missing, the token file is still installed, the connection remains usable, and host details warn that private clones
need gh. The next connection or credential replacement retries it.

In Settings the editor opens at its top, including Back, with field labels smaller than its section captions. Its
actions and leave/delete questions and refusal messages stay visible at the bottom of the scrolling panel, above the
unchanged Close footer. Switching Settings sections hides those actions with the editor. In Add host, the same answers
use the dialog's pinned footer. Keyboard navigation reaches the answers after the fields. As with other framed dialogs,
a very short window scrolls the whole card when its header and footer cannot fit.

Settings also lists **Sandboxes not on this helm**: sandbox names beginning with `farhelm-host-`, `farhelm-sess-`,
`farhelm-build-` or `farhelm-prepare-` that no host or recipe build or preparation on this helm owns. A session-owned
host left after an interrupted launch or refused cleanup remains visible in Hosts. Unregistered sandbox-session
leftovers are discoverable here by their reserved prefix. Each shows its creation time when available, its state, and
**from another helm** when its name carries another helm's tag. **Delete** asks plainly before terminating it and
deleting its snapshots. Unfinished cleanup remains in the list with its error and **Retry cleanup**, even after closing
and reopening Settings; running cleanup shows **Cleaning up…**. This operation state lasts until cleanup succeeds or the
helm restarts. With a saved API key the section stays visible even when empty, with a small **Refresh** beside its
heading. A read shows **Checking…**, then an empty success shows **None found · checked HH:MM** in the viewer's local
time. A failed read shows its reason and keeps earlier rows rather than claiming an empty result. Farhelm never cleans
it automatically. The browser-test namespace `farhelm-sbx-` and sandboxes without Farhelm's reserved prefixes never
appear.

For Tensorlake, every recipe supplies a built snapshot and, on first use, a prepared snapshot for new hosts or sessions.
The UI calls these artifacts snapshots, never templates, a word reserved for launch templates. Saving a recipe never
builds one, because a build bills: the list offers **Build snapshot** while a recipe has none, **Rebuild snapshot** once
it has one, and **Retry** after a build failed or was cut short by the helm stopping, all three needing a saved API key
and Farhelm's registered SSH key. One build per recipe runs at a time; a second click joins it, and different recipes
build at once. A build creates a builder sandbox named `farhelm-build-<helm tag>-<recipe>` from the recipe's base at the
recipe's size, which bills while it runs; refuses to go on unless systemd is the sandbox's init, passwordless sudo
works, apt is available, and bash, curl and git exist, naming the first missing requirement; installs the recipe's
packages with apt, then Claude Code and Codex with their official installers, each downloaded completely over HTTPS
before it runs, and links both onto every default `PATH`; empties `/etc/machine-id` so each host gets its own; takes a
filesystem snapshot and waits until Tensorlake reports it completed; records it; then terminates the builder and deletes
the builder's other snapshots. While a build runs, the recipe's row names its step ("step 5 of 9"); afterwards it says
when the snapshot was built and the agents' versions, or that the build failed, at which step and why, with **Retry**.
The row warns when the recipe's base or packages changed since the build, and when its disk is now smaller than the
snapshot's, which a host cannot start from. A rebuild replaces the snapshot only once the new one is recorded and then
deletes the old one; hosts already started from it keep running. A failed build leaves the previous snapshot in use and
deletes its builder and the builder's snapshots. Every builder and snapshot is recorded before anything else happens, so
whatever a failed deletion or a helm stopping mid-build leaves is deleted by the next build or by deleting the recipe,
and the row says when something still awaits deletion; nothing cleans up at helm start. Deleting a recipe cancels its
running build and deletes its builder and snapshots first; if Tensorlake refuses any of that, or a build sandbox Farhelm
asked for in the last 15 minutes may still appear, the recipe stays, with the reason, so a retry finishes the job.
Changing a base keeps the built snapshot until a successful rebuild replaces it. A recipe cannot select a snapshot
Farhelm built for a recipe as its base, its own included, because Farhelm deletes those when their recipe moves on; that
save is refused.

Preparation takes a second filesystem snapshot on top of that built snapshot, with Farhelm's supervisor installed and
its user unit enabled to start at boot. A short-lived `farhelm-prepare-<helm tag>-<recipe>` sandbox uses the confirmed
recipe's CPUs and memory and the built snapshot's own disk size. It bills while preparing and has a 15-minute idle
timeout if the helm stops. Preparation stops the supervisor without disabling its unit, removes its whole state
directory, verifies that no supervisor database or other state remains, and empties `/etc/machine-id` before taking the
snapshot. Each host started from it therefore creates its own supervisor identity; copied supervisor state is never
reset or accepted.

One prepared snapshot is current per recipe. It becomes stale only when it is more than 24 hours old, the helm's Farhelm
version differs, or the recipe's built snapshot changes. The first new host or session that needs a fresh one prepares
it; concurrent requests join that preparation. Saving a recipe does not prepare anything, and there is no Prepare button
or background renewal. Rebuild still rebuilds the recipe's software snapshot. Preparation records the new snapshot
before retiring and deleting its predecessor, terminates its preparer and deletes its other snapshots. Failed or
interrupted cleanup keeps ownership recorded for a later preparation or recipe deletion; deleting a recipe cancels and
cleans both its build and preparation. A failed preparation retains the previous snapshot's record but never reuses a
stale one.

Farhelm delivers none of its own agent logins, endpoint credentials, GitHub tokens, gh Git configuration or tunnels to
the preparer. A custom base's existing authentication is preserved and may be inherited by hosts created from it for
agents whose recipe choice is None; its author is responsible for that content. Credentials and tunnels selected in
Farhelm are applied separately to each host.

The recipe row shows preparation below its built-snapshot status: **Ready for new hosts and sessions** with age, Farhelm
version and the 24-hour policy in the positive tone; a pulsing current phase and step count while preparing; the reason
when stale; and the failed phase or an interrupted attempt with the next-new-host-or-session retry explanation. An idle
row qualifies readiness as last checked, since age display can advance without a new helm read. The helm always checks
freshness before reuse.

A sandbox matching a recipe's build name is recovered only from an outstanding create request; a recorded sandbox ID
permits cleaning up that sandbox alone. A build start without either record does not establish ownership. An unrelated
sandbox at that name and its snapshots are left alone, and a build fails if the name stays occupied. The sandbox is
hidden from **Sandboxes not on this helm** while the recipe exists; it must finish or be removed in Tensorlake before a
retry can build. Missing records after an older installation or power loss require the same explicit cleanup.

Upgrading converts recipes that installed packages to the Tensorlake Ubuntu base, keeping their packages and built
snapshot. Recipes that reused an image or snapshot become that base with no packages and no built snapshot. Their rows
show **No snapshot yet** with **Build snapshot**, unless an earlier build failed or was cut short, which keeps its
failure and offers **Retry**, the same build; adding a host builds first. Hosts already created from those recipes keep
running, reconnecting, suspending and deleting as before, and rerunning their setup does not build a snapshot.

Cursor is an agent type launched as `cursor-agent`, and YOLO adds `--force`; Farhelm launches Cursor by that name, never
as the generic `agent`, which other tools also use. Its model is optional, with `auto`, `composer-2.5` and literal
custom IDs supported. The non-YOLO choice is labelled "Agent's own setting": Farhelm found no flag that wins over
Cursor's configured Run Everything mode, so it cannot classify that launch and treats it as YOLO for the host guard.
YOLO adds `--force` and preserves explicit Cursor denies. There is no separate effort selector. Cursor uses generic
activity status and has no conversation tracking, automatic Resume, configuration editing, hooks or instruction
injection. The launcher states that tracking and Resume are unsupported, so a Cursor session cannot be restarted;
Replace starts it over, and clone preserves launch intent. See [Cursor](website/src/content/docs/docs/agents/cursor.md).

Grok is an agent type for the official `grok` CLI. A guarded launch is `grok --no-leader --permission-mode auto`,
selecting Grok's auto-review classifier; YOLO instead adds `--always-approve`. Farhelm exposes neither a Grok model
picker nor an effort picker because those command-line contracts have not been verified. Every generated fresh and
resume command retains `--no-leader`: the shared leader is outside the tracked process's ownership boundary, while a
private leader still permits Grok's native subagents. Grok uses generic activity status. The launch layer retains the
selected permission flags before `--resume <conversation-id>`, but it does not offer Resume until the capture
integration has verified an exact conversation. See [Grok](website/src/content/docs/docs/agents/grok.md).

Muse support uses `muse --permission-profile :auto-review` for guarded launches and `muse --yolo` for YOLO. Its screen
reader recognizes Working and Thinking above the composer, the composer at rest, and the workspace-trust question. Other
in-conversation questions and approval dialogs have not been verified; unfamiliar screens use generic activity status.
Guarded selects Muse's AI reviewer and workspace sandbox; the yolo variant skips approval prompts and sandboxing and
trusts the workspace for the run. Muse 1.4.14 or later receives per-launch `SessionStart` and `Stop` hooks, without a
plugin installation or edits to Muse's own configuration. The conversation becomes known at the first prompt, and
Restart resumes its reported UUID with `muse resume <conversation-id>`, keeping the launch's permission flags. A
supported Muse that cannot report still launches; without a captured conversation Restart explains why it is
unavailable. Muse's managed-checkout trust setting adds `--trust-workspace` when Farhelm composes an agent launch into a
managed checkout. With it off, Farhelm adds no trust flag; YOLO or vendor configuration may still grant trust. Muse
follows the agent-version support rule under "Supported user environments".

OpenCode is an agent type. Its model is optional and uses OpenCode's configured default when omitted. For an explicit
choice, Farhelm suggests `opencode/glm-5.3-flash`, `opencode/grok-4.5`, `opencode/grok-4.6`, `opencode/glm-5.3`,
`opencode/gpt-6-luna`, `opencode/gpt-5.6-terra`, `opencode/gpt-6.1-sol`, and `opencode/gpt-6-astra`. The custom-model
field also accepts a bare Zen model name or an `opencode/<model>` value. A bare value is passed as `opencode/<model>`,
and either spelling of a suggested model is that OpenCode model even where another harness offers the same bare name;
another provider prefix is refused. OpenCode has no offered effort choices. Its only offered permission is YOLO,
including when omitted, and compiles to `--auto`, which auto-approves permissions not explicitly denied. OpenCode uses
generic activity status with no hooks, conversation capture/resume, or waiting-state recognition.

Goose, Pi, and OMP are agent types. Each uses its configured model when none is chosen. For an explicit OpenRouter
choice, Farhelm suggests `z-ai/glm-5.3-flash`, `x-ai/grok-4.5`, `x-ai/grok-4.6`, `z-ai/glm-5.3`, `openai/gpt-6-luna`,
`openai/gpt-5.6-terra`, `openai/gpt-6.1-sol`, and `openai/gpt-6-astra`; a literal custom OpenRouter id remains available
after selecting a harness. Goose requests `off`, `low`, `medium`, `high`, or `max` thinking and offers `yolo`
(preselected), `approve`, `smart approve`, and `chat` modes. An omitted Goose permission means YOLO and explicitly sets
`GOOSE_MODE=auto`. Pi requests `off`, `minimal`, `low`, `medium`, `high`, `xhigh`, or `max` thinking and has only the
visibly labelled YOLO mode; this describes the absence of Pi's built-in tool gate, not its project-resource `--approve`
flag. Pi stores that mode as `yolo`; an older snapshot that omitted the formerly optional permission field reads and
displays as YOLO too. OMP requests `off`, `minimal`, `low`, `medium`, `high`, `xhigh`, or `max` thinking (OMP's `auto`
level is not offered) and offers `yolo` (`--approval-mode yolo`), `guarded` (preselected, `--approval-mode write`,
workspace edits approved automatically and other actions asking), and `approve` (`--approval-mode always-ask`);
`smart approve`/`chat` are refused. OMP gets no waiting-state recognition: an OMP approval prompt shows the generic
running/idle status, a settled scope decision rather than a detection gap. Provider capabilities may clamp or reject a
requested effort.

Standard operation must never require falling back to SSH or a separate command line, with three v1 carve-outs:
transport, bringing up the helm's own machine, and recovery from a release below the upgrade floor. Bringing up a
standalone Linux helm machine is `farhelm helm setup`, run once there: that machine's systemd units are written by one
owner rather than by whichever surface got there first, so the helm never installs a supervisor on its own machine (see
Topology). On provisionable hosts, install and updates are the helm's job (over the user's SSH access); what may
legitimately require manual host-side work is that same transport, on hosts provisioning does not cover. Everything else
— session operations, template management, directory browsing — must work from the client. SSH otherwise remains an
escape hatch, never a requirement.

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

The standalone installer temporarily supports only a Mac installing the desktop app, on Apple silicon (including
Rosetta). This limits only the installer: Linux remains supported for running a helm and session hosts, which the helm
provisions over SSH itself. Other operating systems are refused before any download or filesystem change.

The installer runs as the current user without root. On the Mac, `~/Applications/Farhelm.app` is the whole installation
and, for now, the only user-facing way to launch Farhelm; `~/.local/bin/farhelm` is only a link into it, for using
`farhelm` from a terminal. There is no custom install directory. Releases that cannot be updated while running, and
releases without the app resources, are refused before anything installed changes; staging may create the executable
directory, but installs nothing. Installation does not create or start services. The desktop app manages its own helm
and local supervisor, as described in [Topology](#topology).

Re-running the installer updates an installation at or above the release's upgrade floor (see
[Upgrade compatibility and client scale](#upgrade-compatibility-and-client-scale)). Its default is the latest stable
release; `FARHELM_VERSION` selects a specific release, including a prerelease. An older installation is refused before
the update starts: stop local sessions and their terminals, quit Farhelm, run `farhelm uninstall`, then run the
installer. Uninstall keeps the data on disk, but whether the new release can use it is decided per feature. A release
too old to have uninstall needs its app and installed command files removed by hand, as the installation documentation
describes. A fresh install is never below the floor. Updating the software preserves user data. Updating while Farhelm
runs is supported on the Mac: the running Farhelm keeps working on the version it started with, its sessions keep
running, new sessions start, and agents' `farhelm` commands and conversation tracking keep working; quitting and
reopening Farhelm finishes the update, after which sessions started before it use the new version too. An update of an
installation already in this layout, stopped at any point, leaves an app that launches either the old version or the new
one. Package-manager installations are outside this initial scope.

The installed Mac app also keeps itself up to date, the way Chrome does. An app running from
`~/Applications/Farhelm.app` in this layout, and not a build from main, checks for the latest stable release shortly
after it starts and again whenever a day of wall-clock time has passed since its last check, so a Mac that sleeps
overnight still checks about daily. When that release is newer than the installed version, the app verifies the
release's signed checksums and its installer against the keys compiled into it, then runs that installer, pinned to that
release and handed the verified checksums, in the background; nothing on screen changes while it works. While an
installed update waits for a restart the automatic check still refreshes release information, but installs nothing; only
an on-demand check (below) replaces an unwithdrawn waiting update with a newer one. Failed automatic work is logged and
tried again at the next check, without changing that schedule. An install failure shows **Check failed** beside the
version immediately, with its reason in the hover; release downloads failing after a successful `/latest` probe count as
install failures. Optional advisory failures are only logged. A failed probe shows that notice only when automatic
probes have been failing for at least three days, measured from the first failure in the current streak. A successful
probe, automatic or on demand, ends that streak. The streak start survives restarts; a missing value means no streak, a
future value or backwards clock resets it to now, and turning automatic updates back on clears it. A gap with no checks
(sleep, quit, or automatic updates off) starts no clock on its own. A release that fails verification is never
installed, and since the app then cannot update itself, its version readout carries a warning saying that this Farhelm
can no longer verify its updates and must be reinstalled with the installer. The warning stays while the app runs, until
a later check verifies and installs or finds nothing newer, or a newer Farhelm is installed some other way; after a
relaunch it returns with the first check that fails the same way. The same lasting warning is shown when the installer
refuses because the existing installation is below its upgrade floor, with instructions to uninstall first and then run
the installer; apps released before this support show an ordinary install failure with details in their log. It follows
the stable channel only; prereleases are not offered.

Every check first reads the installation as the installer left it, under the same lock installs and uninstall take. When
an install or uninstall was interrupted, the updater installs nothing, automatically or on demand, and the version
readout carries the reinstall warning's mark with a hover saying that Farhelm does not update itself until the install
is finished, and the installer command that finishes it (or, for an interrupted uninstall, that running uninstall again
finishes it and the installer keeps Farhelm). Such a check, like one that finds another install or uninstall running or
cannot read the installation, still reads the release advisories, so a withdrawal is shown and a withdrawn waiting
update loses its restart offer. An installation record that is there but cannot be read is shown as a failed check. The
warning clears at the next check that finds the installation complete. Another install or uninstall running at that
moment, or one that changes the installation between the check and the update's own install, is not a failure: an
automatic check tries again about a minute later without showing anything, and a check the user started says why it did
nothing.

When the app starts and its own version of Farhelm is already gone (an update or uninstall removed it between the app
opening and holding it), it opens itself again, once, if the same installation is still at its place on another version
whose program is there; otherwise it refuses to start with one sentence: that Farhelm was uninstalled, that an install
was interrupted and the installer finishes it, or to open Farhelm again.

Every update check also reads signed release advisories. When the running release was withdrawn, the version readout
carries a warning whose hover includes the maintainer's reason. The ordinary update path can still install a newer,
unwithdrawn release; automatic and on-demand checks never install a withdrawn target. A withdrawn update already
installed and waiting for restart loses its red update-ready marker and **Restart to update** offer. Its warning says
manual recovery is required before restarting: an ordinary quit and relaunch may still open that withdrawn version. The
updater installs nothing over a withdrawn waiting update, even on demand; it cannot establish that replacing it would
preserve local state. When a safe update waits while the running version was withdrawn, the readout keeps its red update
marker and also explains that withdrawal. Withdrawal and reinstall warnings can coexist, with both reasons in the hover.

Advisories are kept only for the current run. A later verified file can lift a withdrawal; a missing file, a failed
download or invalid signed content leaves the last verified information unchanged, and never causes a reinstall warning
or prevents an otherwise allowed update. After relaunch, withdrawals are relearned at the next check. With automatic
updates off, there is no background advisory fetch: the warning can be learned only by an on-demand check. Advisories do
not affect the standalone installer, the helm or remote hosts. Replay protection and directives other than release
withdrawal are outside this behavior.

Whenever the installed version is newer than the running one and is not known to be withdrawn, however it got there (the
app's own background install, or the installer run by hand in a terminal, which the app notices within about a minute),
the version readout in the sidebar's top bar shows that an update is ready: a red up-arrow before the version, and the
whole readout red instead of grey. Its hover says which version is installed and that a restart finishes the update.
Clicking it opens a small menu with **Restart to update**, which quits Farhelm and opens it again on the new version
(sessions keep running, as they do across any quit), and **What's new**, which opens the release notes on the docs
website (`https://farhelm.io/docs/release-notes/`) in the system browser. Without an update waiting, selecting the
version checks for updates. A dim refresh glyph after it hints at that action; hover and keyboard focus brighten both
and turn the glyph a quarter. The reinstall warning keeps its mark and the same check action. Where no updater runs (a
build from main, say), the readout stays plain text without the refresh glyph.

The update can be checked for on demand by selecting the version, by choosing the local machine's host row's Update (see
[Session list](#session-list)), or by choosing **Check for updates** in the `?` menu (see [Feedback](#feedback)). Each
checks now and installs a newer unwithdrawn release if there is one, even while an earlier unwithdrawn update already
waits for a restart. A check started this way, or an automatic check a user's request joins, spins the refresh glyph
with **Checking…** beside the version readout, then **Installing…** while a newer release installs. When an update waits
and the readout has no refresh glyph, a separate spinner carries the same progress. **Up to date** stays there for about
five seconds; **Check failed** carries a warning look and stays until the next check starts or the user clicks it, with
the reason kept in the hover. An automatically shown failure, and a user's failed check that took its place, instead
stays until a check succeeds, a newer version is installed by any route, or the user dismisses it; later unsuccessful
work does not silently clear it. A user's check takes precedence with its progress and outcome. After dismissal, a later
automatic failure shows again only if it qualifies under the same rules. Selecting the version retries. An installed
update takes over with the red update-ready readout. An up-to-date answer while an update already waits shows nothing
beside that red readout. A failure to restart into an update stays in the hover only. The inline progress and outcome
are announced to screen readers, and either spinner stops moving under reduced motion while the progress text remains.
Hover wording explains the readout's check or restart action. Automatic work that no user joins shows no progress; its
qualifying failures show as described above. The desktop settings dialog has a switch for installing updates
automatically, on by default; turning it off stops the startup and daily checks and the background installs, while
on-demand checks keep working and the red readout still appears for a version installed by hand. With automatic updates
off, the user updates through an on-demand check or by running the installer, as before. None of this applies to the web
UI, to a build from main, or to remote hosts. Remote hosts have their own Update actions and the separate,
off-by-default **Update remote hosts automatically** setting described under Topology.

The installer, run by hand, intentionally trusts get.farhelm.io over TLS: it learns the latest stable release from
get.farhelm.io, downloads that release's `SHA256SUMS` from it and the release archives through it, over HTTPS only (no
plain-HTTP redirects), and checks archives against those checksums, but it does not itself verify the release signature
the helm checks when provisioning other hosts, because a fresh machine has no pinned key or verifier to check it with. A
release get.farhelm.io does not publish is refused, with no fallback to another source; releases from before
get.farhelm.io existed are not published there. The helm's release mirror setting (`FARHELM_RELEASE_BASE_URL`) has no
effect on the installer; a mirror that is safe for the helm, which verifies signatures, would not be safe for a download
that does not.

Installation follows the same ownership rule as removal: a file is not destroyed merely because its name matches
something the installer writes. The installer changes `Farhelm.app` only when it can show it built it, and replaces
copies of `farhelm` and `farhelm-desktop` that an earlier layout put in `~/.local/bin` only when their checksums match
what that layout's ownership record says it put there. Anything else at `~/.local/bin/farhelm`, whether the user's own
file or a Farhelm from before the record existed, is kept under a visible name and reported, and the install proceeds;
no crash or interruption point may lose it.

Installing a release older than the installer's handoff floor runs that release's own published installer, with the
protections it had then. Those installers do not protect an unrelated app put at `~/Applications/Farhelm.app` while they
build its replacement.

"Built it" covers the whole bundle. Once the installer recognizes `Farhelm.app` as one it made, everything inside it is
the installer's to replace or remove, old version folders included, along with anything a user put in them. The same
holds for the names the installer reserves for its own temporary files: `.farhelm-new.*` inside the app and
`.farhelm-link.<pid>` beside the Terminal link. This does not change how a bundle is recognized, or how anything outside
the bundle and those names is treated.

An installation must have an equally discoverable removal path. The installation instructions document
`farhelm uninstall` alongside installation, and a successful installer run prints that command. Users of releases
without uninstall support on macOS may need to upgrade once before using it. The current installer cannot supply that
upgrade on Linux. Supporting those older binaries directly, or providing a separately downloaded uninstaller, is not
required initially.

### Uninstall scope and interaction

`farhelm uninstall` removes the selected local standalone installation for the current user: on the Mac the app with
every version kept in it and the terminal link, and on Linux an installation an earlier installer made, in the default
installation directory or a custom one. It must identify the installation being removed rather than assume that
whichever files happen to be in the default directory are the intended targets. Ambiguous ownership or an unsupported
installation must produce an actionable refusal before changes begin.

The command shows the files and services it intends to remove and the data it will retain, then asks for confirmation.
`--yes` skips that confirmation, not ownership checks. Without an interactive confirmation channel, the command requires
`--yes` rather than proceeding implicitly. `--dry-run` reports the proposed actions (including stopping the desktop
app's own supervisor; see [Operator prerequisites and failure behavior](#operator-prerequisites-and-failure-behavior))
and any blockers without changing files, stopping processes, or disabling services. Confirmed 2026-10-11: those blockers
are ownership, recognition and ambiguity refusals, not predictions about locks held at preview time, such as an open
desktop app or a running installer. The preview warns that an install or upgrade running now makes uninstall refuse; the
real run refuses with a clear message for any held lock.

Removal covers the recognized installer-created macOS app bundle and its terminal link, the installed CLI on Linux, and
Linux user services owned by `farhelm helm setup` for this installation. Recognize services through the existing setup
marker and the executable recorded in their unit files, and reuse setup's service-removal behavior. A unit edited by
hand is outside this recognition guarantee and may be attributed to another installation; see
[Supported host setup](#supported-host-setup). Those services are stopped and disabled before their executables are
removed. Operator-authored services and service drop-ins are not deleted; retained integration files are reported. The
user must stop services outside Farhelm's removal authority. Basic uninstall does not analyze effective service
overrides or inspect their processes.

Persistent user data is retained, including session history, attachments, credentials, the host registry, preferences,
logs, cached payloads, repository caches, and prepared checkouts inside the configured checkout roots. Completion names
the retained data locations; retaining data must not be presented as erasing all traces of Farhelm. There is no
`--purge` in this first version. Project directories, agent harness installations and their own data, independently
installed dependencies, and unrelated files are untouched. Uninstall does not remove shared parent directories or change
their permissions.

The command affects only the selected local installation. It does not contact registered hosts, uninstall remotely
provisioned supervisors (the hosts panel's uninstall does that; see [Topology](#topology)), or remove a separate
provisioning-owned local installation. Removing a helm does not stop sessions on its remote hosts.

### Operator prerequisites and failure behavior

Before uninstalling, the user must stop local sessions and their additional terminals, quit the desktop app, and stop
manually started Farhelm processes. Sessions may survive a supervisor exit, so quitting the app or supervisor alone is
not enough. A one-time stop/restart when upgrading to the first uninstall-capable release is acceptable.

Basic uninstall operates on files. It does not walk processes, discover runtime sockets, reconstruct session ownership,
or introduce a runtime registry. One deliberate exception: on macOS it stops the supervisor the desktop app started, if
that supervisor is still running, because the app's supervisor keeps running after the app quits (see Concepts), and
would otherwise make the uninstall refuse until the user found a process they never started themselves. It stops only
the process the app recorded starting, after checking that the live process is exactly that one and that it runs a
program inside the app being removed; a supervisor started any other way is never stopped and refuses as before, with a
message saying to stop it. The stop is the supervisor's orderly one, never a kill; a supervisor that does not exit
within a bounded wait refuses the uninstall with nothing removed. It happens after confirmation and after an open app
has been ruled out, so an uninstall that refuses because the app is open stops nothing, and `--dry-run` lists the stop
without performing it. The other runtime facts it reads on macOS are locks: whether the supervisor's and helm's
state-directory locks are held, to keep an open desktop app from running against an installation being removed (see
Concurrent and interrupted runs), and whether any version of Farhelm kept in the app is locked by a program still
running it. A version in use refuses with nothing removed, naming the version and an `lsof` command that shows what
holds it, since a lock says only that something holds it, not what (it need not be Farhelm). A held lock is reported as
such, not as proof of what is running. On a volume where those locks cannot be taken, uninstall refuses and says how to
remove Farhelm by hand, because it cannot tell what still uses a version. It neither forcibly terminates agents nor
claims to prove that all processes have stopped. The command explains the stopping prerequisite before removal,
including with `--yes`.

Refusals identify the actual filesystem check, the path and observed result or concrete error. A file's existence must
never be described as proof that a process is running. Diagnostics appear in ordinary output without requiring verbose
mode and contain enough evidence to investigate a suspected false positive.

Ownership checks precede removal. A foreign file or bundle must not be deleted merely because its name matches an
expected artifact, and symlinks must not redirect removal into unrelated files or directories. Missing artifacts are
acceptable when the remaining installation can still be identified safely.

Removal can fail partway through. Failures return a nonzero status, distinguish completed actions from remaining work,
and explain how to retry. Completed removal steps need not be rolled back, but retry must tolerate them and continue
cleanup safely. On macOS the app records that an uninstall is under way before anything is removed, so an interrupted
uninstall stays recognizable: an update refuses to finish into it, the app's updater pauses and says to run uninstall
again, and rerunning uninstall finishes it. The CLI remains available until the other required removal steps succeed so
an ordinary partial failure does not remove the user's retry command. Successful removal does not promise that the
now-removed command remains invocable.

Uninstall running at the same time as installation, updates, setup, desktop startup, or session creation falls under
[Concurrent and interrupted runs](#concurrent-and-interrupted-runs) like any other overlap: the outcome must be correct,
and refusing is acceptable. Uninstall takes the locks those operations already use (on macOS the installer's
installation lock on `~/Applications` together with the lock folder older installers use, and the supervisor's and
helm's state-directory locks; on Linux the installer's install-directory lock, and setup's unit-directory lock when
setup's services or unit directory exist) after confirmation, and then re-checks what it is about to remove under them;
a lock already held, or a plan that changed since confirmation (including a different app put at the same place),
refuses with nothing removed. Only the installation lock is waited for, a few seconds; the others are taken without
waiting. An install or update that starts while uninstall holds the locks refuses in its own way, and so does a setup
whenever uninstall holds setup's lock.

### Concurrent and interrupted runs

Confirmed 2026-09-28: the installer, `farhelm helm setup`, and `farhelm uninstall` must always leave a correct, defined
state when a run is interrupted at any point (Ctrl-C, a closed terminal, a crash, a reboot) and when two runs of these
commands overlap, including uninstall overlapping desktop startup or session creation.

A correct state is one of these: the run completed; the run refused or failed before changing anything; or the run
stopped partway and the command its message names, normally the same command run again, either finishes the job or
restores the previous installation. Refusing is always acceptable, including refusing a second run while another holds
the installation, and refusing to continue until the user runs the recovery command. Doing the wrong thing is not
acceptable: deleting or replacing a file the command does not own, reporting a result that did not happen, leaving
binaries, the macOS app bundle and their ownership records disagreeing with each other, leaving the helm and supervisor
services configured against different state directories, or leaving a state that no documented command recovers from.
Reach this with the simplest mechanism that works, and prefer a refusal to machinery that tries to carry on.

Confirmed 2026-10-11: a run killed between two adjacent atomic writes may leave state that rerunning the same command
repairs. That is an accepted stopped-partway state and the expected recovery, even if the intermediate state is one of
the wrong outcomes above, such as the helm and supervisor services configured against different state directories.

### Acceptance coverage

On macOS, automated tests must exercise the actual installer and installed uninstall command through fresh installation
and update followed by removal. They must verify paths containing spaces and quotes, dry-run and confirmation behavior,
retained data and unrelated files, foreign artifacts and symlinks, the stopping prerequisite, and partial-failure
retries. While the installer refuses Linux, end-to-end acceptance coverage of installer-made Linux installations,
including service-failure ordering during removal, is absent. This does not remove Linux uninstall behavior or its
lower-level tests. Concurrent lifecycle operations are outside this initial acceptance scope.

Filesystem-refusal tests must verify the diagnostic's evidence against the fixture that caused the refusal. An assertion
that output merely says "unsafe" or names an artifact does not establish this contract.

macOS file behavior must be tested on a native macOS runner; a simulated macOS layout on Linux alone is insufficient.
Bundle construction and removal can be tested headlessly without rendering the UI. These focused checks must pass before
shipping uninstall support.

## Sessions

### Creation

Session creation is one action, not a wizard. Choose an existing directory or explicitly request a managed checkout; the
agent choice is independent of that destination:

- **Runs on** offers a three-way switch above Host with **Existing host**, **New container** and **New sandbox**. A
  segment with no usable place is hidden, and the switch is hidden when only Existing host remains. Existing host shows
  the ordinary Host picker. New container shows Recipe and Container host, defaulting to the last container host used.
  Hosts with unusable Podman are disabled with their reason, and the note says whether the recipe is ready there or
  builds on first launch. New sandbox shows Recipe and retains its built-snapshot and account prerequisites below.
  Recipes unsupported by the selected provider are disabled with a reason. Templates, Clone, quick launch and recent
  setups carry the recipe and place together. Missing recipes or removed place hosts make that placement inapplicable.
  The template editor offers the same three-way placement choice.
- Host: The session launcher's host picker shows each ordinary host's mark and name in the list's existing order,
  including unavailable hosts labelled with their phase. It is a single-choice keyboard list: arrows, Home and End
  browse; Enter or Space chooses; typeahead finds a name; Escape closes without changing the destination. Enter on the
  closed field launches with the shown target. The picker is disabled for the whole launch round trip, and its value is
  empty before hosts arrive. Session-owned environment hosts are omitted from ordinary destination choices.

- Working directory: an existing directory on the target host, named by an absolute path (a relative path would resolve
  against the supervisor process rather than the client, and would shift meaning across supervisor restarts). `~` and
  `~/path` are also accepted and resolve against the home of the user running the supervisor on the target host —
  concretely, the supervisor's own `HOME` environment, resolved once at supervisor start; a supervisor with no usable
  `HOME` refuses `~` with an error naming that, rather than guessing at an account database. The expansion happens once,
  at creation, and the session stores the expanded absolute path, so the anti-drift property above is preserved; `~user`
  forms are refused. The create form prefills the field for the target host: the selected session's directory when that
  session is on the target host's installation, else the directory of the newest recent setup on that installation that
  launched into a plain folder (a fresh managed checkout does not count), else `~`, which each host expands to its own
  home. Changing the host re-seeds a folder the user has not typed or picked in that draft; a refresh of the recent
  setups never rewrites an open draft's folder. A remembered folder that no longer exists is used anyway, and the create
  is refused as for any missing folder. A directory picker/completer against the target host's filesystem is provided.
- Title: optional; auto-generated when omitted. Renameable later. A title is a single-line label, so a title you supply
  is refused if it contains control characters (escape sequences, newlines, tabs); an auto-generated one has any such
  character replaced with U+FFFD rather than being refused, since the directory it comes from is legitimate and you did
  not choose the label. A supplied title is bounded in size too, and by the same rule for both verbs: the text you send
  on creation — working directory, command and resume command, title, and the rest of the launch — must fit in 64 KiB
  between them, and a rename's title alone is held to that same bound. Renaming has no conflict detection: two renames
  of one session both succeed, and the later write is the title that sticks.
- Launch composer: New opens a dialog on the agent launch kind with no selected agent type. The two launch kinds are
  shown as two tabs, Agent and Command, each showing only its own fields; switching tabs keeps each tab's draft. Codex,
  Claude, Muse, Cursor, Grok, Goose, Pi, OpenCode, and OMP agent launches carry an agent type plus model, effort, and
  permission choices where that agent type supports them, and an optional wrapper prefix. The prefix is a shell-style
  word list placed before both start and resume commands, including before Goose's `env` words. Each word retains its
  quotes and escapes exactly as typed. At execution the words are joined with spaces, followed by Farhelm's composed
  agent arguments as single-quoted literal words, and interpreted by non-login, non-interactive `bash -c` on the host or
  inside the container. This gives the prefix full Bash expansion and operator meaning. Stored wrappers use the same
  interpretation without migration. The editor refuses empty prefixes, NUL and Farhelm placeholder words; placeholders
  are not substituted in a wrapper. Templates, remembered prefixes, approvals and fingerprints retain the raw words, and
  editors display them joined with spaces. Farhelm does not inspect the wrapper's behavior: the user asserts that it
  runs the selected agent with Farhelm's arguments and environment passed through. A launch that needs custom agent
  arguments, a directory placeholder in the wrapper, or a separately written resume command is a command launch; visible
  permission vocabulary is `Guarded`, `Agent's own setting`, `Custom`, `Approve`, `Smart approve`, `Chat`, and `YOLO`.
  Custom is offered for Claude, Codex, Grok, Muse, Cursor and OMP: the user supplies permission flags instead of
  Farhelm's own. The launcher splits quotes and escapes into literal words and previews those words; it evaluates no
  expansions. Empty words lists and incomplete quotes are refused. The user is responsible for keeping these flags
  limited to permissions; Farhelm neither checks their effect nor infers a safety mode, and every custom session counts
  as YOLO. Templates and recent setups retain the words, Clone, Replace with and Restart with allow editing them, and
  plain Restart or Replace reuses the stored command. Changing agent type clears custom and its words; a later template
  setting another permission clears them. Custom never becomes the remembered default permission. Guarded means an agent
  can approve actions through a safety mechanism that can still refuse or escalate (a classifier, AI reviewer, sandbox
  or asking the user), not that it asks about everything. YOLO means no such check. For every agent type that supports
  Guarded, Guarded is the default permission when none is chosen. This does not change when defaults apply or override a
  remembered choice. Claude and Grok use `--permission-mode auto`, Codex uses `-a on-request -s workspace-write`, Muse
  uses `--permission-profile :auto-review`, and OMP uses `--approval-mode write`. Every supported resume retains those
  flags. Cursor's omission stays under its own setting; Pi, OpenCode and Goose omissions remain YOLO. Model and effort
  omissions still leave their flags out. Model selection is optional for every harness; OpenCode, Cursor, and Grok offer
  no effort choice, and Grok accepts no model choice. The helm owns the released model catalog and validates every
  agent-launch choice, so the GUI never turns a model identifier into an argv fragment. Typing a model never changes a
  selected harness: the typed id is read as that harness spells it (for OpenCode, a bare Zen name means
  `opencode/<model>`), a model only other harnesses offer is refused with a message naming them, and choosing another
  harness's model from the full model list switches the harness on purpose. With no harness selected yet, a typed known
  model fills in its owning harness (a bare `gpt-6-luna` picks Codex); a custom model needs an explicit harness, and an
  id several harnesses offer asks for one. Replacing a harness clears incompatible choices and a permission equal to the
  previous harness's default. When an agent switch automatically makes the shown permission more dangerous, a red banner
  slides down across the top of the launcher, naming both modes and explaining that the new agent does not support the
  previous mode. Undo restores the draft before the switch; dismiss closes the banner. The banner lasts about eight
  seconds, paused while hovered or focused, and is announced to screen readers. The permission control gets a thick red
  outline that fades after about two and a half seconds. Reduced motion replaces slide and fade with a static outline
  while the banner is shown. Chat and approve are safest, then smart approve and Guarded, then YOLO, custom flags and
  the agent's own setting; only a move to a later group triggers the banner. Choosing a permission directly never
  triggers it. Choosing the first agent triggers it only when a permission was explicitly chosen before that agent; a
  displayed default or a remembered permission alone does not. An invalid combination cannot launch. New normally
  preselects no harness or model. The permissions mode remembers the last successful agent launch, helm-wide across
  every client, except that a harness's default or forced YOLO clears that memory instead of preselecting YOLO for
  another harness. Explicit YOLO on a harness with a non-YOLO default is remembered. The last wrapper prefix from a
  successful user agent launch is remembered per agent type, helm-wide; a launch without one clears that agent's
  remembered prefix. Switching agent types loads the new type's remembered prefix. Agent-origin launches and command
  launches do not change this memory. Workspace trust for Codex, Muse and Pi comes from separate off-by-default Settings
  switches for managed checkouts (below). There is no per-launch trust choice, and the old remembered trust value is
  neither read nor written. Those switches add per-launch trust arguments without writing vendor trust state; the
  separate opt-in for Claude managed checkouts below is the one exception. Without an override, the harness retains its
  own trust behavior; Codex, Muse, Claude, and Cursor may still ask for directory trust (Cursor even with `--force`).
  Farhelm does not send answers to their terminal prompts. "Reset choices" returns permissions and the wrapper prefix to
  their remembered values, and a recent setup's saved choices override them when used. A recent setup's historical trust
  value is ignored. The launch-composer search matches harnesses, launch templates (`tl:name`), models scoped by the
  chosen harness, effort words offered by that harness and model plus `effort:default`, bare `yolo` and `guarded` plus
  the `perms:` choices for those modes, custom, own setting and supported approval modes, host and name actions, folders
  and recent setups. On the Agent tab, Guarded and YOLO can also be chosen before a harness, by their bare words or
  `perms:` choices. Other permission actions are offered only when the selected harness can represent them; own setting
  is offered only on Cursor. The Command tab offers no permission actions. `name:foo` applies the entire value as the
  session name. `host:foo` filters host choices, and `host:local` selects the helm-local host even if it has an alias.
  The default local host label in the GUI is `Local (this machine)`. Accepting a result applies it and clears the box
  while keeping focus there, except custom moves focus to its flags editor. Enter on an empty box launches only a
  complete, valid selection through the ordinary Launch path; an incomplete attempt says why beside Launch, as does a
  refused Launch press, and the greyed-out button's hover names what is missing. Enter when the launcher itself has
  focus uses that same Launch path and refusal. Request refusals appear there too, within the initial dialog viewport.
  Unmatched unlabelled text offers one visible **Set session name** row; accepting it sets the name and clears the box
  without launching. Scoped queries with no results show a live reason in the results area: a missing name, no matching
  choice, a pending model or template list, or the owner/repo form a repository needs. Unknown model ids belong in the
  model field. On the agent tab, an unmatched effort or permission word, or an `effort:` or `perms:` query with no
  offered row, asks for a harness first when none is chosen. Input refusals and request errors in the launcher use
  yellow warning colors; loading messages and typing hints keep the ordinary text color, while permission danger stays
  red. Spaces alone ask for a search or an empty box and never launch. Escape or a click outside search closes its rows
  or message without clearing the query. Enter on a nonempty query with the area closed first reopens it; a separate
  Enter picks the visible highlighted row, or leaves the explanation in place when there is none. Enter on a choice
  button (launch-kind tab, harness, effort, or permissions) applies that choice and then launches through the ordinary
  Launch path, or shows its refusal beside Launch; custom instead selects the mode and focuses its required flags
  editor. Enter on the closed host picker, agent-type or effort dropdown, YOLO radios, or Resume checkbox launches with
  the values shown, without changing them; those native controls retain their Space and arrow behavior. Action buttons
  keep their normal press. The search box, model field, YOLO confirmation, and save-as-template panel keep their own
  Enter rules. In the editable folder field, Enter chooses an arrow-highlighted dropdown entry without launching; with
  no highlighted entry it launches as before. A held Enter never launches.
- Conversation selector: New, Clone and Replace with offer an optional one-time selector for every agent launch. Claude,
  Codex, Grok and Muse label it conversation id; Goose and OpenCode session id; Cursor chat id; Pi and OMP session file.
  Empty preserves an ordinary fresh start. The selector is not saved in templates, history, defaults or the session's
  reusable launch, so a later Clone starts fresh. It is passed literally as one argument, without a shell or checking
  that the conversation exists. Leading dashes, reserved locator tokens and Farhelm placeholders, spaces, quotes,
  backslashes, control characters and oversized values are refused by the existing conversation-id checks. Session-file
  paths may be absolute or relative to the session directory; a leading tilde is refused because it does not expand. New
  managed-checkout destinations and command launches refuse a selector. Goose, Cursor and OpenCode show that Farhelm
  will not capture the named conversation and later Restart will be unavailable.
- Initial prompt: the agent tab offers a resizable multiline **Initial prompt** box directly after the conversation
  selector for supported agents. Its help says "Sent once, as &lt;agent&gt;'s first message when it starts. Restart,
  Clone and Replace do not send it again." The summary adds `Prompt: N lines` for a nonempty message. Enter edits the
  message without launching. Goose, Pi and command launches offer no box. The draft survives recent-setup selection and
  switching tabs; unsupported selections omit it from the request. A Clone or Replace-with prefill clears it. Template
  messages with invisible formatting are shown escaped and read-only until explicitly cleared; ordinary newlines and
  tabs remain editable. The message travels with the create request, including managed checkouts and named conversations
  where supported, and for a create the command line or an agent sends with an idempotency key, its exact value is part
  of that key's retry identity.
- Command launch: a command launch takes a command line run through `bash -c`, non-login and non-interactive. The text
  reaches Bash unchanged except for Farhelm's placeholders, so `~`, variables, globs, newlines, comments, compound
  commands, pipes and redirection have Bash meaning. Each placeholder must stand as its own unquoted word; a quoted
  placeholder, or one joined to other text where it cannot be substituted safely, is refused on save and again at spawn
  for an older stored command. Farhelm inserts each filled value as a single-quoted word, or one such word per argument
  for `{farhelm_args}`. The command may reference the session's working directory as `{cwd}` and takes a required YOLO
  assertion: the user states whether the command bypasses human, classifier, AI-reviewer and sandbox safety checks, and
  Farhelm believes the statement. Farhelm never reads a command line to decide whether it is YOLO, what agent it runs,
  or how to resume it. A command launch may optionally declare the agent type it runs. A declared agent type requires
  `{farhelm_args}` as a whole argument exactly once in the command, which marks where Farhelm places that agent type's
  own launch integration arguments (including the conversation-reporting hook, instructions pointer and Codex's
  `--no-daemon`, see Durability); it is replaced by nothing for an agent type that takes no such arguments, so the rule
  never depends on which agent type was declared. `{farhelm_args}` without a declared agent type is refused. Farhelm
  passes whatever turns its integration on as arguments, never through the environment: every process the agent starts
  inherits the environment, including another agent of the same type that it shells out to, so a hook carried there
  would reach runs that are not the session's. The environment carries only settings that Farhelm's own reporter reads
  once an argument has turned it on, set on the launched process directly and never written into the command. Declaring
  an agent type gives the session that agent type's status reading and conversation reporting. It also allows opting
  into Resume, which requires a resume command containing `{conversation}` and `{farhelm_args}`, each as a whole
  argument exactly once; Farhelm fills those and `{cwd}` in and otherwise runs the resume command as written. Codex gets
  `--no-daemon` even with hooks off. A custom Codex command that puts `{farhelm_args}` after `exec`, passes `--remote`,
  or runs `codex agents` or `codex queue` fails because Codex refuses that flag there; Farhelm does not detect these
  incompatible commands. `{conversation}` in the start command is refused. The one exception to "as written" in either
  command: Codex's internal workspace-trust markers, `{codex:trusted-cwd}` and `{codex:untrusted-cwd}`, are filled
  wherever one stands as a whole argument, exactly as in a composed Codex agent launch (see SPEC_impl.md). A command
  launch with no declared agent type, or one that does not opt into Resume, can never be restarted (see Lifecycle
  operations). The YOLO assertion covers the resume command too. Search while the command launch kind is active still
  offers agent types, models, and recent setups, and accepting one switches to the agent launch kind; accepting a
  folder, host, or name keeps the current launch kind, and a template keeps it unless the template sets one. The command
  tab's YOLO assertion, declared agent type and Resume choice are set in that tab's own fields, not through search.
- Launch templates: `tl:name` in the search box offers templates by name, and accepting one applies it. Applying a
  template makes the edits it contains, in the same way and with the same effects as making them by hand, in a fixed
  order: launch kind first, then agent type, then every other field. It can switch the launch kind, and choosing an
  agent type clears the choices incompatible with it exactly as picking that agent type by hand does. A template's
  agent-launch fields and command-launch fields apply to the launch kind active once its own launch kind, if any, is
  applied. A template may set any launcher field: launch kind, agent type, model, effort, permissions (including custom
  and its words together), wrapper prefix, initial prompt, command, YOLO assertion, resume command, host, destination (a
  folder or a managed checkout), and session name. A template's session name is a base name by default, including names
  in existing templates: for a managed checkout it is used as-is when free, otherwise numbered `-2` through `-50` until
  a free folder is found. The session title and checkout folder use the same number. A fixed name keeps its exact
  spelling and reports a clash. Ordinary folder destinations use a base name as-is without numbering. Fields a template
  leaves out keep whatever the launcher already holds, so templates stack: applying `my-codex` and then
  `myproject-webbuilder` applies both, the later one winning where they overlap. A wrapper-prefix field has three
  states: absent leaves the current prefix, including a remembered one; present with words replaces it; present with no
  prefix clears it. Quick launch applies those same edits before starting the agent. A template whose field does not
  apply to what the launcher holds when it is applied, such as a model while the launch kind is command or a model the
  chosen agent type does not offer, is refused with a message naming the field, and nothing from it is applied. So is a
  template that changes a field the current launcher mode holds fixed, such as the host in Replace with or a field held
  by Restart with (Lifecycle operations below). A template names an ordinary host by its recorded install identity,
  exactly like the host default below, so a registry row retargeted to another install makes the template's host field
  inapplicable rather than silently aiming at the successor. An environment target uses a separate immutable recipe name
  and place. It never reinterprets a host identity string. A launch made after applying templates is, for remembered
  defaults and recent setups, exactly the launch the same hand edits would have made; managed-checkout previews
  additionally apply a template base name's numbering policy. The resulting choices count as the user's explicit
  selection, and a template that changes host, installation, destination, title, or agent invalidates a pending checkout
  preview as the equivalent hand edit does. The last template setting a name decides both its text and kind. In a
  managed checkout the Name field shows the resolved name and a "From template · numbered if taken" pill; an advanced
  name also shows "(<base> is taken)". Typing in Name or accepting `name:` makes it the user's exact name, removes the
  pill and disables numbering. Because a template pins only what it contains, a field the launch requires and no
  template set still has to be filled in by hand; that friction is accepted. New, Clone and Restart with also offer
  "Save as template": an inline name field and checklist capture the current tab's choices, host, destination, and
  session name. Agent type, model and effort are checked whenever set, including Clone's launch choices; a nonempty
  initial prompt is checked too, shown by its escaped first line with an ellipsis when more lines follow. Command, YOLO
  and opted-in resume are checked when set. Permissions and a wrapper prefix are checked only after an explicit choice;
  host only after a hand pick; a folder only after typing or picking it; session name after typing it or applying a
  template that sets it. The session-name row offers base or fixed name, defaulting to base for a typed name and
  retaining a template-supplied kind. A template base is saved as its original text, never the numbered name displayed
  by the launcher. Base is absent from the saved definition and reply. A GitHub repository is always checked. Remembered
  defaults and Clone's inherited host and folder are offered unchecked. The saved template contains exactly the checked
  fields and always the active launch kind. A host without a usable recorded install identity is omitted with an
  explanation. A taken template name is refused without overwriting. Enter in the name field saves without launching;
  Escape returns to the launcher. Successful saving closes the launcher and opens the new template in the Templates
  editor; errors preserve the launcher draft. Replace with does not offer this action. Templates are created, edited,
  duplicated, and deleted in the Templates dialog beside New. A list of names and summaries sits beside the selected
  template's editor; at phone width, opening a template shows the editor with a back link. The editor shows only fields
  the template sets, with labeled controls and explicit add/remove actions. The session-name label row offers "base name
  | fixed name" with help explaining numbering and conflicts. Adding the name starts on base; removing it removes its
  kind. Duplicate and delete undo retain the kind, and saving a base name leaves no unsaved changes. Its "switches
  launcher to" choice is agent, command, or don't switch; don't switch offers only host, destination and session name.
  Agent and command choices store their matching launch kind, so applying the template switches tabs. Existing kindless
  templates keep applying as stored; opening one infers its switch in an unsaved draft, and saving stores that switch.
  Choices follow the agent type when one is set, while model text may be arbitrary and other fields need no agent type.
  The initial prompt uses a resizable multiline textarea, offered on the agent tab when the template's agent supports it
  or no agent is set. It explains that the message is sent on every launch from the template and can be edited in the
  launcher first; the list summary mentions it. Newline and tab remain editable. Invisible formatting is shown escaped
  and read-only, retaining its raw value through unrelated saves; removing and adding the field replaces it.
  Incompatible fields stay visible and prevent saving until corrected or removed. Saving is explicit; Save is greyed out
  and does nothing while there are no unsaved changes, and keeps keyboard focus after saving. Leaving an unsaved draft
  asks Keep editing, Discard, or Save in that reading order, with initial focus still on Save. Its shared header
  supplies Close; the complete editor keeps scrolling within the dialog, including its editor-local action footer.
  Template names are unique. Creating a template or renaming one refuses any whitespace in its name, including Unicode
  whitespace; existing names with whitespace remain usable, can keep their name when other fields are saved, and retain
  delete undo. The agent's template create command follows the same new-name rule. Duplicate opens an unsaved copy named
  `<name>-copy`, then `<name>-copy-2` and so on until free, replacing whitespace in the source name with `-` in that
  suggestion. A workspace-trust field in an older template remains readable but is ignored when applied and omitted on
  every save, including duplicate and delete undo. Delete takes effect immediately and offers undo inside the dialog for
  about ten seconds; undo refuses if its old name has been taken. For a stored template that fails current save rules,
  the control says "Delete permanently" before deletion and no undo is offered. The label the user saw decides: a click
  on "Delete permanently" never offers undo, and a click on plain "Delete" always does, even if the template list
  changed in between. Closing the dialog ends its undo notice.
- Quick launch: at most one saved template is marked with ⚡ in Templates. Its list button shows a muted outline bolt
  when unmarked and a filled amber bolt when marked, at the same size. Cmd+P on macOS or Ctrl+Shift+P elsewhere starts
  it directly in the desktop app when no dialog is open, without showing the launcher on success. While the session
  launcher is the only visible dialog, the same chord applies its template to that draft and leaves Launch to the user.
  Consecutive presses apply in order; a refusal changes nothing from that press and does not discard a later press. A
  missing mark or failed read shows its refusal where `tl:` does, without opening Templates. During a launch or while
  saving a template, the chord is consumed without changing the draft. Applying by shortcut leaves focus and search text
  alone, unless its edits remove the focused control, when focus returns to the launcher's search. Other visible dialogs
  keep the keyboard. A hidden quick-launch launcher still consumes the chord without applying it. Applying it directly
  uses exactly New's defaults and ordinary launch path: the open session's host, else this machine, and New's default
  folder for that host (Working directory above), with the template overriding only its own fields. Success selects the
  created session only if the user has pressed no key or pointer button since dispatch or their last answer to the
  launcher, under the launch-completion rules below. When New was never shown and success selects its session, the
  rename dialog opens over it while its agent is already starting, unless Settings' on-by-default **Name sessions on
  quick launch** choice is off. This applies to the bolt shortcut, number shortcuts, composed sequences, the
  quick-switcher row and the Cmd+J template list, even when a template supplied the name. The actual created name is
  focused and fully selected. Enter with the name unchanged or Escape keeps it without sending a rename; a changed name
  uses ordinary rename. Closing returns to the new terminal, which takes no input while the dialog is open. A launch
  that revealed New gets no naming dialog, even if a later submission succeeds. A quick launch the user moved on from
  keeps its generated name without opening the naming dialog; it can be renamed from its row. A held chord starts only
  once; the chord never reaches the terminal and visible modals other than the launcher keep the keyboard. With no
  usable mark it opens Templates with an explanation. An edited marked template that no longer supplies what its active
  launch kind needs opens New with it applied and a prominent banner saying it needs more choices before it can launch.
  The same banner accompanies an incomplete Cmd+J or quick-switcher pick, without recommending the sequence prefix. A
  launch refusal or YOLO question reveals New with the same draft, error or confirmation, or offers it in a notice once
  the user has moved on (see [Keyboard focus](#keyboard-focus)); nothing bypasses host admission, and the shortcut never
  retries itself. The mark is a nullable template name on the helm's shared preferences, independent of the template
  fields. Clicking another bolt moves it; clicking the lit bolt clears it. An agent-launch template needs an agent to be
  marked; a command-launch template needs a nonblank command. With no launch kind, it is judged as an agent launch. The
  same rule decides whether a template or combined sequence can launch directly. A marked template that no longer
  qualifies can still be unmarked. Renaming repoints the mark before deleting the old name; deleting that name clears
  its mark through the shared delete path, and Undo never restores the mark. The editor says whether its saved template
  is marked. The shortcut reads the mark and templates fresh on each use, so an agent's or the command line's change
  takes effect at once. Templates' hover and a separate quick-switcher row advertise the chord and marked name. An agent
  may set or clear the mark through `farhelm agent template create` or `edit`, behind a template approval card. The helm
  applies the same eligibility rule to an agent's write as the Templates editor does.
- Template number shortcuts: a template may hold one of ten shared keys, 1–9 then 0, independently of ⚡. Cmd+digit on
  macOS, or Ctrl+Shift+digit on Linux, starts its template through the same direct launch path as quick launch, and
  applies it to an open session launcher draft under the same focus, ordering and refusal rules as the ⚡ chord. It
  reads bindings and templates fresh each time. Unbound keys open Templates with a binding hint; an incomplete template,
  refusal or YOLO question uses quick launch's ordinary New fallback. Other visible modals retain the keyboard, held
  keys act once, and the chord never reaches the terminal. Matching uses the physical digit key, independent of the
  printed character; the numeric keypad is excluded. Each row's key button opens a menu with No shortcut, then 1–9 and
  0; every option has a description, occupied keys name their template there, and choosing one moves it. Accessible
  command names remain No shortcut or the key chord; ownership and binding help are separate accessible descriptions. A
  template has at most one key and each key has at most one template. Any template can hold a number key, including one
  that cannot launch alone; only the combined setup must be launchable. Pressing such a key alone opens New with the
  template applied and a prominent alert banner at the top explaining that it cannot launch on its own and how to
  combine it with the sequence prefix. The banner expires about six seconds after application finishes, or when the user
  starts editing. Both key and bolt hover explain launching, their uniqueness and the current choice; partial-template
  key hover explains composition. The bolt retains its existing eligibility rule. Bindings are a whole-map shared
  preference, replaced on write; concurrent windows may lose one map edit, and a GUI save, which re-reads the bindings
  first, can undo an agent's or the command line's binding change that lands between that read and its write (One GUI at
  a time). Names must be syntactically valid but need not exist at write time. Rename keeps a key, delete frees it
  through both delete paths, and Undo never restores it. An agent may assign or clear a number key through the same
  template verbs and approval card. Assigning an occupied key moves it from its previous template, as the Templates
  editor does; a key can still belong to a partial template.
- Quick-launch sequences compose templates without changing the single-chord shortcuts. Cmd+X on macOS, or Ctrl+Shift+X
  on Linux, starts a sequence, except in a text field outside the launcher where it retains its normal meaning. In the
  launcher's own text fields it starts a sequence only when no text is selected; with a selection it keeps Cut. A
  focused terminal counts as the terminal, not as a text field. Bare physical number keys or their usual template chord
  queue templates in order; the ordinary ⚡ chord queues the marked template. Printable characters, including Shift for
  capitals, build a word. Space (with or without Shift) queues that word; an empty Space does nothing. A bare digit
  extends a word already in progress, but with no word queues its number key, so a name beginning with a digit is
  reachable only by its number key or the launcher. A number or ⚡ chord first queues any unfinished word, then its
  bound template. Backspace removes the last character of the unfinished word, never a queued entry. Tab queues the full
  name when the word uniquely identifies a template; an exact match wins over longer prefixes. It does nothing while the
  advisory catalog is loading or unavailable, or when the word is empty, unknown or ambiguous. Tab can queue an existing
  name containing spaces as one entry. With both `my project` and `myproject` stored, `my project` is reachable only by
  its number key or the launcher; no quoting or candidate cycling is offered. Enter first queues any unfinished word.
  Typed words resolve against current templates: a case-sensitive exact name, then a unique case-insensitive exact name,
  then a unique case-insensitive prefix, using Rust's Unicode lowercase. Unknown or ambiguous words refuse the whole
  sequence, naming the word and, for ambiguity, the competing candidates; a word's refusal wins over an unbound number
  key in the same sequence. Outside the launcher it opens New with that error; in the launcher it appears beneath search
  without changing the draft. While armed, typed characters never reach the terminal's program. Enter applies the
  templates in that order, later edits winning, and directly launches through the same defaults, validation and
  admission as quick launch. In the session launcher, Enter instead applies the queued templates to the current draft
  without launching; each completed sequence is one refusal unit, and the pill says "apply" instead of "launch". An
  unbound key opens Templates at Enter with the binding hint outside the launcher; inside the launcher it shows a
  refusal beneath the search box without changing the draft. A refusal names its template and reveals New with the draft
  outside the launcher; an incomplete result opens New with a prominent banner saying "This sequence doesn't make a
  complete setup; complete it below", whether one or several templates were queued. A YOLO question uses the same
  fallback. The banner follows the number-key banner's expiry and dismissal rules. Bindings refresh when the prefix
  starts; Enter captures the bound names currently shown in the pill. Enter with nothing queued cancels without opening
  anything. A non-modal amber status pill is centered over the terminal area, or the main area with no session open,
  showing queued entries as chips and the word in progress. Its hint names typing, Tab, Enter and Escape. While a word
  is in progress, a list below the pill shows at most five matching template names, exact matches before prefixes and
  catalog order within each tier; non-matches are absent. Candidates and Tab use an independent catalog read when the
  prefix starts, which never gates Enter or decides what Enter launches. Neither the pill nor the list takes focus or
  pointer input. While the launcher is open, the pill stays at the bottom of the main area so it does not cover the
  launcher. It is larger, with a soft amber halo and a short fade/scale entrance that is disabled for reduced motion; it
  never loops or takes pointer input. There is no timeout. Escape cancels and is consumed; a click, switching windows or
  opening a dialog cancels. Non-sequence keys, including arrows, other chords and Shift+Enter, cancel and reach their
  normal target; Meta, Control and Shift on their own do not cancel. Repeated held keys add nothing and never launch
  again.
- Recent setups: the helm remembers bounded successful agent-launch combinations and used folders per target-install
  identity. A recent row fills every saved choice and destination; clicking it never launches, and pressing Enter on a
  focused row launches the filled setup through the ordinary Launch path. A retargeted registry row cannot expose the
  replaced install's history. Folder search uses that bounded history, not a recursive filesystem walk; explicit
  browsing asks the selected supervisor for one bounded directory level. Recent-setup rows appear only when the current
  filter actually matches history; the dialog reserves no space for them when it does not. The editable folder field
  opens a grouped dropdown on focus or ArrowDown: up to eight remembered folders, newest first, under "Recent", and
  "Home" (~ on this host) and "Local home" (~ on this machine) under "Reset". Untouched prefilled fields show the newest
  eight; typing filters by case-insensitive substring. Long labels retain the path's end, with the whole path accessible
  and available on hover. Only arrow navigation highlights an entry. Choosing fills the destination without launching;
  local home also pins the local host. Escape closes an open dropdown first, then can close the launcher. Tab, blur and
  choosing close the list; typing reopens it. Enter that submits a launch also closes the list, whether the launch is
  accepted or refused, leaving a refused launch's controls reachable. Managed-checkout mode has no folder dropdown.
- Folder check: in plain-folder mode on a connected host, the launcher checks the folder 150 ms after the host or path
  stops changing. A missing folder or a path that is not a directory marks the field and says so underneath, naming the
  host. The warning is advisory and never disables Launch; the supervisor still validates the folder at launch time.
  Managed-checkout paths are not checked this way. Permission errors, timeouts, busy hosts and other inconclusive
  answers show no warning. An answer for an earlier host, connection or path is discarded.
- Host: defaults to the host of the currently open session, else the helm's own host. "The host of the currently open
  session" means the install the user was looking at, not merely its registry row id: a row retargeted or adopted onto a
  different install after the session was selected falls back to the helm's own host rather than silently aiming the
  create at the successor. The comparison is by the registry's recorded install identity, so it protects exactly the
  installs that have one: a host that has never identified itself to the registry is compared by row id alone, and its
  replacement by another identity-less install goes unnoticed — an accepted residual, since absent identities carry no
  continuity evidence in either direction. When the open session owns an environment, New preselects its recipe and
  place for a new environment, using that environment's home Folder default.

Creation launches an interactive agent. A supported agent can receive a one-time initial prompt as part of its start
argv, after Farhelm's own arguments have been composed, without waiting for terminal readiness. With no initial prompt,
you type the first message into its terminal. An initial prompt is not part of the stored launch, so relaunching or
copying that launch never repeats it; templates may supply one on every new create. `farhelm agent prompt`, which would
send a message into an already-running Claude or Codex session, is turned off in this version; Agent-spawned sessions
has the refusal and the design kept for re-enabling it.

A custom model id is typed into the model field and applied with Enter; it needs a chosen harness. The optional session
name sits in the destination block under host and folder, visible without any disclosure. The launcher has a shared
header with Close, a scrolling body and a pinned footer. Search and recent setups precede labeled destination rows
(Where, Host, Folder or Repository, Name), followed by the Agent and Command tabs and their fields. The footer shows the
chosen agent type or command, host and destination beside a compact Launch, Replace or Restart action; its accessible
name retains that operation and target. Agent launches also retain the model, effort, permissions and nonempty prompt's
line count in the footer summary. Long summaries may truncate visually; their full text remains accessible. Cancel comes
first in the action row, then Reset choices where offered, Save as template where offered, and the main action last.
Native Tab follows Close, the fields in their existing relative order, then the footer actions. Ordinary launches still
start focus on search; Restart starts on Cancel. Refusals, destructive warnings and the inline YOLO question remain
adjacent to the actions in a bounded scroll region, so long content cannot displace those actions. The save-as-template
panel remains inline in the scrolling body. Project registration (associating metadata with a directory) is optional
convenience and never a prerequisite.

After submitting a launch, including Restart with or Replace with, the launcher hides while the request is pending. The
user can switch sessions, type in their terminals and act on other sessions. The same launcher remains busy and cannot
accept a second launch or be replaced with another draft. Launches and changes to the host registry still exclude each
other. Suspend and Resume are not changes to the host registry: from Hosts or from a sandbox session's row, top bar or
Stop question (Stop and suspend), they are refused only while this window's own launch targets or is waking that same
host, and their buttons are disabled for exactly that. Launches on other hosts and host changes elsewhere proceed while
one is pending, and a launch on that host is left to the launcher's availability rule below and to the host's own
lifecycle lock. Preparation and required confirmations remain modal until the launch is submitted. A failure or
follow-up question restores the same launcher with its draft if the user has not moved on since submitting it. Otherwise
a notice offers it, and the launcher returns when the user opens that notice (see [Keyboard focus](#keyboard-focus)).
Whether such a notice keeps New and other launches refused until it is opened is not settled yet. Switching to a session
on another host does not change the submitted launch's destination. The acted-on session's progress, actions, terminal
attachment and row outcome settle as soon as the reply arrives; only the launcher's return depends on the user.
Successfully waking a suspended sandbox continues to create without revealing the launcher, restoring its page isolation
or waiting for another dialog. A fresh GitHub checkout then waits for this window to see the woken host connect, for at
most 60 seconds. When that runs out, nothing is created: the launcher returns with its draft and says the window did not
see the host connect, and a connection that shows up later creates nothing. The wait for the checkout's preview that
follows is not part of that limit.

Once the request has waited about two seconds, a small pill at the bottom of the window shows a spinner and a status:
Launching, Restarting or Replacing, the captured host's name and elapsed seconds, updated about once a second, including
the session name when supplied. The pill takes neither focus nor pointer input. It disappears when the launcher returns
or closes; replies handled within the threshold show no pill. Assistive technology gets one polite announcement when the
status appears, not one for each elapsed second. A successful New or Clone opens its session only while the submitted
launcher opening is still current and the user has pressed no key or pointer button anywhere in the page since their
last answer to it. Quick launches use their dispatch as that answer until a later launcher answer. Auto-repeat, a lone
modifier and an accidental double-click or repeated Enter on the launcher's own submit do not count. Waking the host,
preparing the create and automatic selection after Delete do not reset or advance the count. A naming dialog waiting
behind another dialog checks again before opening.

When the launch opens its session, ordinary sidebar selection applies, including closing Hosts, and terminal reveal
decides focus. Otherwise the session appears in the sidebar, the selected session and its remembered selection stay as
they were, and focus, panels, menus and dialogs stay as the user left them. Any quick-launch naming dialog is skipped. A
display-only pill says **Started “name”** for about four seconds whenever nothing navigates, including when a filter or
Hosts hides the new row. This brief report does not keep the launcher or its draft busy; New and shortcuts can start a
fresh launch immediately. Replace and Replace with open their replacement only while their source remains selected;
Restart with never navigates.

Creation guards against accidental double submission (a double-click): one intended create yields one session or a clear
error, never two silently. Around a lost connection to the helm a duplicate is accepted instead (see
[Losing the connection to the helm](#losing-the-connection-to-the-helm)). Deliberately creating several sessions with
identical parameters — same directory, same launch — is a sanctioned workflow, not a duplicate to be suppressed.

The idempotency keys the command line and agents send with a create (Agent-spawned sessions) are the create's retry
keys; the GUI sends none. A create's retry key lasts as long as the session it created, including a retained error
session; a refusal without a surviving session is replayed for 24 hours. Delete removes the session's retry record along
with the session, including an unfinished create's record. After Delete or refusal expiry, the same key is a fresh
request that re-validates and, for an agent request, needs the approval any new request needs.

Controls that perform exactly the same operation on the same target behave identically and share that operation's state,
including a pending confirmation, differing only where the GUI strictly requires it.

A YOLO launch on a host that asks before YOLO launches (see the host settings under Topology) needs an explicit
confirmation. An agent launch counts as YOLO when its stored permission is custom or YOLO, including omitted permissions
on Pi, OpenCode, OMP and Goose. A stored omission on Claude, Codex, Grok, Muse or Cursor is the agent's own setting, not
guarded, and is conservatively treated as YOLO by the host guard. A command launch counts as YOLO when its YOLO
assertion says so, and only then: Farhelm does not parse command lines to second-guess the assertion. Because an agent's
assertion cannot be checked either, an agent may not start a command launch, or a YOLO launch, on such a host at all
(see Agent-spawned sessions). The helm enforces the confirmation, so no client can skip it: every create, clone,
replace, replace with, and restart with that reaches it on such a host without the override is refused before any
supervisor is contacted, and nothing is started. The GUI answers that refusal with a prominent confirmation, shown with
the control or surface that started the launch and scrolled into view, that names the host, says what YOLO means and why
this launch is one (YOLO was chosen where the agent type offers other modes, the agent type has no guarded mode, the
launch uses the agent's own setting which Farhelm cannot override, custom flags whose effect Farhelm does not check, or
the command was asserted to be YOLO), and retries with the override only when the user confirms. Besides a one-off
confirmation it offers to stop asking for that host: that answer first sets the host to start YOLO sessions without
asking, exactly as its settings would, and then retries with the override; if changing that setting fails, nothing is
started and the confirmation stays up with the reason. In the session launcher, an explicit YOLO choice on an agent that
offers guarded mode also offers starting the same launch guarded instead, without YOLO confirmation; Restart with offers
it only when the resulting draft still differs from the stored launch. The `farhelm` command line has no override: a
request from inside a session is an agent's, and agents follow their own rule under Agent-spawned sessions, whatever the
request claims. A plain restart relaunches the session's own stored launch and needs no YOLO confirmation again. The
question stays inline and has no header Close: its answers read Cancel, the guarded option when offered, the outlined
permanent YOLO answer, then the filled one-time YOLO answer. Tab follows that same order; Cancel keeps initial focus,
except that a plain Replace's question, which arrives after its reply, takes focus only if the user has not moved on
since answering Replace (see [Keyboard focus](#keyboard-focus)). Plain Replace, agent clone and inherited spawn also
copy the stored command; pre-upgrade no-flag sessions therefore remain own-setting, with plain Replace asking on hosts
that ask before YOLO launches and agent copies refused there. Clone, Replace with and Restart with compile new choices
and get the guarded flags. Remote hosts must be updated after this protocol change before launches resume; older
releases cannot read launches stored as guarded.

Failures split cleanly in two: precondition failures (nonexistent directory, unknown template, unreachable host) fail
the create with a visible error and no session; launch failures of a session that was successfully created surface on
the session itself — **Error** when the program handed to exec could not be started at all. For a command launch or a
wrapped agent that program is Bash: a missing command inside the line instead exits with Bash's code 127 and its message
in the terminal. **Exited** means the program started and then ended, however quickly, with its exit code visible, or
the user stopped the session before its agent finished starting, with that annotation. In the session launcher every
refused launch (New, Clone, Replace with, quick launch) shows as a bordered alert with a warning icon beside Launch,
starting "Not launched." before the refusal's own words, and announced to assistive technology. Restart mode uses the
same alert beside Restart, with "Restart error." before the refusal's words. Errors that are not a refused launch, such
as "Save as template" refusing a wrapper, keep the plain line. When the refusal says the folder does not exist, is not a
directory or is not usable, the folder field is also marked and says so under it, naming the host, for as long as the
form still names that host, its connection and that folder.

The GUI has no per-request deadline. A command the helm leaves unanswered too long counts as a lost connection (see
[Losing the connection to the helm](#losing-the-connection-to-the-helm)).

### Sandbox sessions

A sandbox session is an ordinary session in a named Tensorlake sandbox created from a recipe for that launch. Its
session-owned host uses the ordinary Tensorlake connection, credentials, tunnels, updates and lifecycle machinery. It
has no permanent host-rail icon, is not an ordinary launch destination, and is omitted from the host listings of agents
outside it. Its sessions appear under All. Opening Hosts lists the sandbox with its ordinary diagnostics and applicable
controls; updates, problems and progress also appear in the existing sidebar attention area. Update all and automatic
updates include it under their ordinary rules. No separate session-only host-management surface is required.

The launcher's **New sandbox** segment lists recipes with a cube mark and CPU and memory size. A recipe without a built
snapshot is disabled with **No snapshot yet**. A missing API key or absent or unreadable local Farhelm SSH key also
disables it. The selected recipe shows the existing copyable SSH-key registration command and explains that Farhelm
cannot check account registration. If SSH later refuses the key, launch fails with that remedy and attempts to delete
the new sandbox and snapshots. Brief billing and visible failed cleanup are accepted. Other failures retain their own
reason. Launch never starts a missing software snapshot build; Add host keeps its existing build-if-missing behavior.

The footer names **new &lt;recipe&gt; sandbox**. Its note explains that the sandbox is for this session, bills while
running, Suspend stops CPU and memory billing, and deleting its last session removes the sandbox and its files. Managed
checkout works normally, including the recipe's GitHub token for private clones. Folder is fixed to the new sandbox's
home, without Browse or recent folders.

Launching asks **Create a sandbox for this session?**, naming recipe, CPU, memory and disk, linking Tensorlake pricing,
and explaining suspension, the idle timeout and irreversible file deletion. Cancel creates nothing; **Create and
launch** proceeds. Only **Create Tensorlake sandboxes without asking** skips this question. Recipe templates, their
quick launches and Clone use the same launch and confirmation path.

The helm records the session-owned host and a name beginning `farhelm-sess-<helm tag>-` before creating the sandbox. It
ensures a fresh prepared snapshot, creates the named sandbox with a 30-minute idle timeout and automatic resume off,
connects, uses ordinary credential-delivery and tunnel setup, and creates the session. Credential and tunnel problems
keep their ordinary warning behavior. No placeholder session is listed. Existing launch progress names **Preparing
&lt;recipe&gt; for sessions…**, when needed with the first-launch/24-hour/update explanation, **Creating a
&lt;recipe&gt; sandbox…**, **Starting Farhelm in the sandbox…**, and ordinary session launch, each with elapsed time.
The task belongs to the helm; closing a client does not cancel it or its cleanup.

If an ordinary new launch fails before the helm has confirmed session creation, including a lost create reply, it
attempts to delete that newly created sandbox and snapshots and shows **Not launched.** with the reason. This may end an
agent that started before its reply was lost. Failed cleanup or a helm crash leaves the named resource available in
Hosts, or under **Sandboxes not on this helm** when the host record is absent. After confirmed creation, an agent, clone
or hook failure remains the ordinary session error. Nothing resumes, continues or cleans up an interrupted launch
automatically at helm startup. Transparent recovery of an ambiguous launch is not promised.

An open supervisor connection counts as Tensorlake activity. After the helm goes away, the existing two-minute silence
limit ends that connection; Tensorlake's 30-minute idle timeout then suspends the sandbox. Farhelm adds no idle timer.
Named sandboxes retain files on suspension, and snapshot storage still bills. A helm restart reconnects a running
sandbox and leaves a suspended one suspended. Farhelm never resumes one automatically.

Templates store an immutable recipe name and Tensorlake place separately from an installed-host identity. The choices
are mutually exclusive, and missing recipes are inapplicable. A recipe with a non-home Folder is inapplicable. Clone
targets the recipe for a fresh sandbox with the source's launch and repository intent, never a copy of its disk.
Restart, Restart with, Replace and Replace with stay in the existing sandbox. Recent setups restore the recipe and
place, obtaining a fresh environment. Save as template from an environment launch and bulk Suspend/Resume/Stop and
suspend remain deferred. When a sandbox session is open, New and quick-switcher creation preselect its recipe for a new
sandbox and use the new sandbox's home Folder default, never the disposable host or its old folder. A missing recipe
stays unavailable rather than silently choosing another host. The normal creation confirmation still applies.

Replace creates its replacement before removing the old session. That internal source removal is session-only and never
destroys the sandbox or files. A failed or ambiguous Replace preserves the existing sandbox, reports what is known, and
identifies its name. Partial failures can leave several sessions; rare failures may require manual data recovery.
Farhelm adds no transparent recovery to hide this case. If a Replace's new session is admitted before a user Delete of
its source, that Delete is not the sandbox's last and removes only the source, and Replace's own removal of the source
finding it already gone counts as success. In the other order the sealed sandbox (below) refuses Replace's create, and
Replace reports that the sandbox is being deleted.

**Delete removes the sandbox together with its last session.** Only the Farhelm running inside the sandbox knows which
sessions it holds. Farhelm's own list can miss some: a Replace whose reply was lost, or sessions started there while the
helm was not connected. So no Delete decides from the helm's list, a cache or anything stored on the helm's side that a
session is the last one. When the helm is connected to the sandbox's Farhelm, Delete asks it, in one step that waits for
the session's in-flight Farhelm commands to finish, to delete this session and, if no other session of any kind remains
(one still launching or restarting counts), to **seal** the sandbox. A sealed sandbox permanently refuses every new
session and every new run, from any source and with or without a helm connected: creates, Replace's replacement, agent
spawns and creates, the retry of an earlier create, recovery of a managed-checkout launch, Restart, Restart with and new
terminal tabs. The seal survives a restart of its Farhelm and is never lifted.

The confirmation states both outcomes: the session and its checkout are deleted, and if it is the last session in the
sandbox, the sandbox, every process and file in it and its snapshots are deleted too, with **Delete session**. It is
never skipped, since it may destroy the sandbox. If other sessions remain, only the chosen session is removed, with its
checkout discarded rather than moved to the checkout trash (unless another session still uses that checkout), and the
result says the sandbox stays and how many other sessions it holds, as its Farhelm counted them. If it was the last, the
helm destroys the sandbox through the ordinary Tensorlake deletion path. A sealed sandbox's Farhelm reports the seal
whenever it lists its sessions, and the helm finishes destroying any sealed sandbox it sees, whether or not it sent the
Delete: after a lost reply, a failed destruction or its own restart. While that runs, and if it fails, Hosts and the
sidebar attention area show the sandbox as being deleted, with the reason and **Retry**, and its sessions show as being
deleted with their actions unavailable. Deleting a sandbox's last two sessions at once (from the window and an agent,
say) can briefly show "the sandbox stays" for one Delete just before the other seals it; the list then shows the truth.

When the helm cannot ask the sandbox's Farhelm (the sandbox is suspended or unreachable, or its Farhelm is a version
this helm cannot talk to until it is updated), or the request gets no answer, nothing is decided. Delete does not guess
and never resumes the sandbox: it offers the whole-sandbox Delete instead, whose confirmation warns that every session,
process, file, checkout, unpushed change and snapshot in the sandbox will be removed, including sessions Farhelm does
not list because they were started while it was not connected, and that resuming or updating the sandbox first lets
Farhelm delete just this session. **Delete session and sandbox** proceeds; it works while suspended or disconnected
through the ordinary Tensorlake deletion path. Sandbox and snapshot cleanup finish before the host and its session
entries are forgotten; failure keeps a visible reason and allows retry. A sandbox that was sealed and then suspended or
became unreachable before the helm read the seal shows no sessions until the helm reaches it again; **Delete sandbox**
in Hosts is the way out meanwhile. **Delete sandbox** in Hosts always destroys the whole sandbox after its own
confirmation, whether or not its Farhelm answers.

Bulk Delete sends one such request per marked session of a sandbox the helm is connected to; its Farhelm takes them one
at a time, and the last one seals if nothing else remains. Marked sessions of a sandbox the helm could not ask when the
modal opened get the whole-sandbox Delete, once per sandbox, and the modal says which rows are which. A refusal or a
missing answer partway through leaves that row with its reason and never turns into a whole-sandbox Delete without that
warned confirmation. Whole-sandbox deletion moves nothing to the checkout trash and produces no archive cue or
not-archived notice; its checkout tooltip and Delete consequence explain sandbox destruction instead.

The session toolbar offers **Suspend** immediately left of Stop, with a pause glyph. Plain Suspend freezes processes
without stopping them; Resume continues them. Both affect the whole sandbox, including leftover sessions. Reuse the
ordinary host Suspend activity check and confirmation. While suspended, Resume is the filled primary control with a play
glyph; Stop, Restart and Replace are disabled, while Clone and Delete remain available. The unavailable card says
**Session suspended**, explains preserved processes and snapshot-storage billing, and offers Resume. Existing host
wording covers transitions and the time suspension was noticed without inventing who suspended it.

The host label is **&lt;recipe&gt; sandbox**, with the cube mark in session metadata. Suspended rows have grey titles
and a pause-marked **Suspended** badge. The row menu offers Suspend/Resume and explains that Clone creates a new
sandbox. Plain Stop ends the chosen agent and tabs but keeps its sandbox running and billing. Its inline question says
so and offers **Stop and suspend** as well. That action finishes ordinary Stop before requesting host Suspend; a failed
or incomplete Stop does not suspend. The row menu always offers Stop and suspend, so it remains reachable when ordinary
confirmations are skipped. Delete's confirmations are never skipped.

Agents in a sandbox session act only within their own sandbox, and agents elsewhere launch sandboxes only behind a card
that always asks; [Agents and environments](#agents-and-environments) has the rules. The helm enforces them at the
ordinary relay, which fails closed when the owning helm is disconnected. No supervisor-local agent restriction is
stored. Local help and conversation reporting remain unchanged; the global prompts-off refusal keeps its existing
precedence. Recipe templates stay GUI-only for listing and writing: agents cannot list, read or write them, and agent
template verbs naming one are refused, while `farhelm agent create --template` may launch one from an ordinary host.
Ordinary creates from the GUI or from agents on other hosts cannot target the disposable host; its initial launch,
internal Restart/Replace paths and its own agents' creates use it. User Delete of a session in it follows the rule
above: the sandbox goes only with its last session, or through the warned whole-sandbox Delete.

### Managed checkouts

The user-facing name is managed checkout: a repository clone Farhelm made for a session, rather than an ordinary
directory the user supplied. Interfaces do not assume GitHub will remain the only repository source: wherever a
repository is selected or requested, its source is explicit, such as the launcher's `gh:` prefix or the CLI's `--gh`
flag. Session rows with a current checkout association show a branch glyph and the repository in place of the directory.
Compact rows put the glyph after the name. Its tooltip names the repository and actual checkout path and explains that
the directory moves to the trash when its last session is deleted. The session view shows the same mark beside its
directory. Sessions using another session's checkout, including its subdirectories, carry the mark too; requesting a
checkout at launch does not establish current membership.

The launcher and template editor use the same segmented destination control: folder or managed checkout, with a folder
glyph or branch glyph. Selecting managed checkout reveals a repository field and explains the directory's lifetime.
Empty, invalid or unselected repository input cannot launch using a previous folder. Selecting a repository from the
field's suggestions, or selecting `gh:owner/repo` in the composer, explicitly requests a new checkout on the selected
host. It works with both launch kinds. Selecting a folder or editing the ordinary directory returns to an
existing-directory launch without changing the agent choice. Clone of a session in a managed checkout starts on a
managed checkout of that repository, preferring its current checkout association over its launch provenance. This
includes sessions opened in an existing checkout and replacements of the session that created it. A source in a checkout
subdirectory clones to the new checkout's top level. Clone of other sessions, Replace with, and plain Replace start from
the source's actual directory; a managed checkout then requires an explicit repository selection, a saved repository
setup, or a template that sets one. `farhelm agent clone` keeps its existing-directory behavior.

The helm owns a working-copy root and optional post-clone command, globally with per-host overrides. There is no default
root. The default checkout folder and all-hosts post-clone command can be set or cleared under **Checkouts** in Settings
or with the command line. Each host's settings also offers its own checkout folder; per-host post-clone overrides remain
command-line only. The default is resolved at launch, never copied when a host is added. Settings saves each field
explicitly, shows a refused value's reason, and clears it when an all-whitespace field is saved. The command field
preserves all other spacing and newlines. An explicitly disabled command displays as an empty field, but opening
Settings never rewrites any stored value. Configuration replies the GUI reads may contain the default folder, a host's
own folder and the all-hosts command, never per-host command overrides. A missing root and its parents are created on
the target host at the first checkout, using the ordinary umask; configuration, discovery and preview create nothing.
Existing symlink ancestors resolve to their canonical directory, while unresolved `..` in a missing suffix and dangling
symlinks are refused. A root whose resolution changed since preview is still refused. `~` expands there, using the
supervisor's captured home. Clearing an override restores inheritance; an empty hook override disables the inherited
hook. Configuration changes affect new attempts, not an already accepted attempt or its retries.

Settings offers an off-by-default, all-hosts choice under **Agent trust** to answer Claude's "trust this folder?" for
new managed checkouts, including agent-requested checkouts and checkouts initially started with another agent. It never
applies to a directory the user selects themselves. After clone and the post-clone command succeed, immediately before
the agent starts, Farhelm marks that checkout trusted in Claude's own configuration on that host. Turning it on affects
future checkouts only, and changing it does not invalidate an already displayed path preview.

The help leads with the mechanism: Claude has no supported switch, so Farhelm edits its settings file (normally
`~/.claude.json`), which also holds sign-in, settings and per-directory state. It changes only that directory's trust
key, with an atomic replacement, and makes a best-effort removal when the checkout is archived or retired as missing. An
archive refusal leaves the directory and trust in place. A launch or Delete never fails because this edit failed. The
caution explains that Claude's unlocked exit save can lose either writer's update, future Claude versions may change the
file or locking without notice, and Farhelm cannot promise continued safety. Farhelm waits on Claude's advisory lock and
never takes over a stale one. A writer stalled past Claude's stale-lock threshold can still lose an update in either
direction when another writer takes over. This is an accepted trade-off of editing Claude's file. Trust allows
repository hooks and commands just as answering Yes does. The mechanism is explicitly opt-in and best effort.

While Claude's trust question is visible in the agent terminal of a managed checkout, the setting is off, and the hint
has not been dismissed, an informational band appears above the terminal, below its tabs. It takes layout space rather
than covering the question, is announced without taking focus, and explains that answering automatically would require
editing Claude's own settings with caveats. Settings and caveats opens **Agent trust** in Settings at Claude's switch;
Don't show again stores a shared dismissal, reversible beside the switch. × hides only the current occurrence, until the
question leaves the screen. A change made elsewhere updates the hint band as it happens; Settings reads the current
shared values when opened. A failed Don't show again save restores the band with an adjacent error while that session is
still shown; leaving the session drops the error. If the corrective settings read also fails, only the save error
appears until trust is known to be off again, without the normal explanation or its actions.

The **Agent trust** section also offers separate off-by-default, all-hosts trust switches for Codex, Muse and Pi. Each
applies only when Farhelm composes an agent launch into a managed checkout, including a checkout an agent requests and
an existing managed checkout selected for New, Clone, Restart with or Replace with. An inherited spawn recomputes trust
for its destination under the same rule as clone, rather than retaining its parent's trust choice. Codex receives a
per-launch trusted directory override, Muse receives `--trust-workspace`, and Pi receives `--approve` for project
settings and extensions. These switches never edit the agent's saved configuration. Their help states that the
repository's hooks, settings and commands may then run as if the user had approved them. Ordinary user-selected
directories receive no trust override, even when the client or a cloned source carries an old trust choice. Plain
Restart and plain Replace keep their stored launch. Settings changes affect newly composed attempts; an accepted
fresh-checkout retry retains its original trust choice, and an ordinary keyed retry whose resolved launch changes may
refuse the reused key.

Codex and Muse show the same nonmodal discovery hint while their recognized trust question is current in a managed
checkout's agent terminal, their switch is off, and their own hint has not been dismissed. The hint names the agent,
offers Settings at its switch in **Agent trust** and Don't show again, and has an occurrence-only ×. Each shared
dismissal is independent and reversible beside its switch. Failed settings reads never count as an off switch. Pi has a
switch but no hint band: its project-resource approval is not currently recognized as a trust question. Older stored
session snapshots default the new hint evidence to absent; the legacy Claude hint remains Claude-only.

When the selected host has no checkout folder in effect, the launcher's managed-checkout destination offers the same
explicit-save field, explaining that it sets the default for hosts without their own folder and can be changed later in
Settings → Checkouts or overridden in each host's settings. Refusal keeps the draft editable; success refreshes
repository suggestions and the checkout preview without reopening the launcher. Selecting a repository before setup does
not hide the field. A host with its own folder keeps that folder and is never offered this setup. The template editor
keeps its ordinary destination control.

Farhelm is not designed for a working-copy root that other local accounts can write to, such as a group-shared folder or
a sticky, `/tmp`-style directory. Keeping the root, and so its checkouts and the archive folder inside it, in a location
only the user can write is the user's responsibility. Farhelm does not check the root's owner or mode, and the ownership
and archive promises in this section assume no other account can create, replace, or take over entries in it.

Before Launch becomes available, the composer shows the exact host and path. Fresh GitHub checkouts on a suspended host
are an exception: the composer shows no path and says the folder is picked automatically when the host is ready (or
named after the session when a name is supplied). One press resumes, obtains a fresh preview and creates the session.
Direct template launching is the other exception: marking a template with ⚡ or giving it a number key is consent to its
managed-checkout destination, including when combined in a quick-launch sequence, so it obtains the same exact preview
and launches without showing the path first. A recipe-target environment launch is another exception. The user accepts
its recipe, place, repository and naming policy before the environment exists. After connecting, the helm obtains the
ordinary exact preview on that new installation and uses the existing admission rules without a second confirmation. It
never borrows another host's preview or relaxes accepted-preview admission for ordinary installed-host launches. For a
recipe target, a destination conflict before session acceptance is a failed launch. It cleans up the new environment and
shows **Not launched.** with the conflict, rather than offering a preview for the destroyed host. On an existing host, a
lost race still reveals New with the conflict and new preview rather than submitting again. Preview creates no directory
and reserves nothing. An unnamed checkout uses the lowest available positive `repo-N` and that basename as its session
title. Naming the session after quick launch changes only its title: the checkout keeps its original `repo-N` directory
name. An explicit title at creation keeps its printable display spelling while the path uses `repo-` followed by its
lowercase ASCII slug; runs outside letters and digits become hyphens. A title already starting with `repo-` does not
repeat that prefix. An empty slug or a component over 200 bytes is refused, and so is a checkout path too long to
archive later (the archive location adds up to 65 bytes to it, and the whole must stay within the system's path limit:
4096 bytes on Linux, 1024 on macOS). Existing files, directories and symlinks all occupy a name. If another create wins
the displayed path, Launch reports the conflict and obtains a new preview; it never submits automatically or silently
chooses another directory. A typed name or a template's fixed name remains a conflict rather than gaining a suffix. A
template base name uses its first free candidate, from the base itself through `-2` to `-50`; the cap leaves the
conflict visible. A lost-race base-name launch obtains a new preview and may advance, but requires another Launch click.
For Clone into the source's repository, an untouched nonempty copied title defaults to `<source title>-clone`, then the
lowest free `<source title>-clone-N` for N ≥ 2. The name field shows the effective name, and preview and launch use it
alike. Changing hosts or repositories restarts the suffix search; editing the name makes it explicit and disables
automatic suffixes. A lost-race launch re-previews and may advance the default name, but still needs another click on
Launch for the newly shown path.

Replace with into a managed checkout, Clone into a different repository, and Clone of a source with no repository ignore
an untouched copied title: the session is unnamed and gets the lowest available `repo-N`. An empty source title also
remains unnamed. Whenever a checkout launch is unnamed, the name field shows the `repo-N` it will get as placeholder
text. Choosing an existing folder again restores the source's original directory and copied title; choosing the source
repository again restores Clone's default name, unless the user has edited it.

Repository input is a GitHub owner/repository pair, not a URL, branch selector or shell fragment. The owner has 1–39
ASCII letters, digits or hyphens, starts and ends alphanumeric, and has no consecutive hyphens. The repository has 1–100
ASCII letters, digits, underscores, dots or hyphens, excluding `.` and `..`. Identity is lowercase. Clone uses the
constructed HTTPS GitHub URL, with the target user's ordinary credentials. On the ordinary clone path, the clone stage
first creates or refreshes the host's cache of that repository's branches and tags, then clones from GitHub using its
cached objects. The cache lives under the supervisor's state directory on that host, is always used on this path, and is
removed at supervisor startup after more than 30 days without a successful checkout. Concurrent checkouts of the same
repository take turns on the cache. Every checkout is independent: its origin remains the constructed GitHub URL, and
removing the cache cannot break it. A cache failure fails the clone stage with the cache path in the error; there is no
fallback to a clone without it. Cache refresh, clone, configured hook, and agent run in that order inside the session
terminal, so progress, authentication prompts and failures are visible. A failed stage prevents later stages and
preserves partial content. A checkout is not permission to run a hook unless the maintainer configured that hook.
Farhelm disables automatic Git gc and maintenance for the cache refresh, clone, spare fetch and fast-forward that a
launch waits on. This command-local policy does not change the user's later Git commands or configured hook.

With **Keep a checkout ready for faster launches** on (the default), a managed-checkout create may instead take a
complete independent clone prepared earlier for that repository on that host. The reserved hidden container is
`<checkout root>/.farhelm-spares/<owner>/<repository>`; allocation treats its name as occupied and repository discovery
skips it. Each create freezes the switch and checkout root alongside its post-clone command and trust choice. Taking a
spare preserves the newly allocated directory's identity, fetches from GitHub visibly in the session terminal, moves the
spare's contents into that directory, then fast-forwards there. The terminal says an earlier checkout is being updated.
The configured post-clone command, optional Claude trust and agent still run only at launch, in the final directory, in
their existing order.

The resulting checkout is a clone made earlier and then updated: its reflog records that clone and update, `FETCH_HEAD`
and `ORIG_HEAD` exist, unchanged files retain older modification times, and objects from branches deleted upstream can
remain until Git's own cleanup. Git hooks and checkout filters, including Git LFS and process filters, ran at build time
in another directory. At launch they run only for the update in the final directory, with `post-merge` rather than
`post-checkout`. The checkout retains the GitHub origin and no dependency on the cache.

An empty, unexpected or structurally unusable spare falls back to the ordinary cache-and-clone path before any content
moves. A deleted upstream branch falls back too, and so does one rewound or rewritten so that the spare's commit is no
longer in its history. Git 2.48 and later detects a changed default branch; older take-capable Git accepts its
clone-time default branch if it still exists upstream. Git below the take's required capabilities uses ordinary clone. A
failed spare fetch fails the clone stage visibly, without another clone or credential prompt. A failure after any entry
has moved retains partial content and refuses automatic repetition. The shim never recursively deletes a claimed tree;
supervisor cleanup owns leftovers.

After a new managed-checkout launch's clone stage has ended, including a recorded clone failure, the supervisor makes
one background cache/spare attempt, whether spares are on or off and whether a spare was taken. When a spare is needed,
its cache refresh consolidates accumulated packs before cloning; otherwise the attempt only consolidates the cache.
Cleanup runs under the cache lock, so a later launch of that repository can still wait for it. It runs through the
user's login shell, without a terminal or credential prompts, at low CPU priority, with a deadline of about ten minutes.
Ordinary Git hooks and filters run; Farhelm's post-clone command, trust edit and agent do not. Failure is logged; a
failed build leaves no spare, so a later launch can clone normally. One live supervisor builds one spare at a time
across its host; pending or running requests for the same repository coalesce, with no automatic retry after failure. A
spare becomes available only as a complete clone; concurrent launches can take it only once.

Turning the global switch off affects future creates. After each repository's next clone stage ends on that host, the
background attempt discards its spare instead of building, and a create using another checkout root discards the spare
at the old root first. Accepted creates and retries keep their frozen choices. Startup removes leftover building and
claimed directories except claims still in a live launch's clone stage, and spares are evicted with their caches after
more than 30 days unused. Discard renames owned content to private trash names and uses the existing identity-checked
confined removal, which never follows symlinks or crosses mounts. Nothing under `.farhelm-spares` that Farhelm did not
put there is protected: a spare or building folder there that was removed and recreated, or otherwise replaced, is out
of band, and Farhelm may adopt it and delete it (see the dedicated-ownership paragraph under Supported user
environments).

Once allocation has occurred, failures retain the session and its checkout association for inspection and Delete. A
completed preparation permits ordinary restart without repeating clone or hook. Interrupted, missing, corrupt or
ambiguous preparation refuses automatic repetition. Retrying an accepted create with its original key reconciles the
original allocation and configuration, including after a helm restart or configuration edit. Authenticated
reconciliation can return the accepted result or a refusal that prevents this key from allocating for 24 hours, after
which the key re-validates as a new request.

In managed-checkout mode, the composer's folder field shows the current preview's effective path read-only, or a pending
or error state while no path is available. The folder segment deliberately leaves checkout mode and restores the prior
editable path, Browse and recent folders. Typing into the checkout path cannot turn a managed checkout into an
existing-folder launch.

An interruption after mkdir but before durable identity capture leaves ownership unestablished. Recovery retains a
visible error session and refuses to adopt or prepare the unknown directory. Explicit Delete may retire that unresolved
session and plan; the directory remains untouched for manual inspection, and Delete's result tells the user so and names
the preserved path, as below.

Farhelm identifies a directory it created by its inode number plus its creation time, and uses the device number only
where no creation time is available. On supported setups such as btrfs subvolumes (Fedora's default `/home`), NFS,
overlayfs and some device-mapper configurations, the device number is assigned when the filesystem is mounted and can
change across an ordinary reboot or remount while the folder is untouched. A check that compares device numbers would
then refuse the user's own folder as replaced and permanently break the operation it guards, while inode plus creation
time still detects a folder actually replaced at the same path, which is what these checks exist for. Every check of
such a directory against an identity recorded earlier follows this rule rather than comparing device numbers. Confirmed
2026-10-22: a checkout recorded without a creation time, whose folder still reports none, cannot be told from a folder
removed and re-cloned at the same path, so Farhelm treats it as unconfirmed: Delete releases it without archiving it,
with the notice below; the trash never deletes it permanently; and a new session started in it does not trust its agent
automatically. A checkout recorded before creation times were kept, on a filesystem that reports them now, keeps the
inode and device comparison.

Ownership follows use, not the lifetime of the session that first requested the checkout. Ordinary sessions in a managed
directory or its canonical subdirectories also retain references, including references to managed ancestors. Stopped,
exited, and errored sessions still count. Delete releases its reference; only the final reference causes the recorded
checkout to move into `.farhelm-trash` under its original root, using its original basename plus a timestamp and
collision handling. This is a no-overwrite move, never recursive deletion or a cross-device copy fallback. A foreign
object replacing the recorded path must remain untouched. The UI calls this archive the trash.

Existing trash in `farhelm-archived-working-copies` moves automatically to `.farhelm-trash` before that root's trash is
next archived into, recovered, listed or deleted from. A root that becomes reachable later follows the same rule. The
move never replaces an existing new trash directory or merges their contents; when both names exist, only the new one is
used. An older managed checkout overlapping the legacy directory prevents migration and is left untouched, with the
blocked trash operation reported rather than its records forgotten. A session started inside old trash keeps its
recorded working-directory spelling, so restarting it can fail after the move; permanent deletion still recognizes its
reference and refuses that archive.

The session list's header places Trash, Templates and New in that order. Trash shows an outline can when empty and an
open-lid can and count when it holds checkouts on reachable hosts. Its dialog groups archives newest first by host,
showing repository, archive time, bounded disk usage and the directory to copy work out of before cleanup. Unreachable
hosts remain visible but cannot be checked or deleted. Unknown sizes make totals explicitly partial. Per-host and global
delete ask once in place, naming the confirmed hosts, checkout count and size, and warning that uncommitted or unpushed
work is permanently lost. The global action skips unreachable hosts and waits until every reachable host can be read.
There is no restore action or automatic cleanup. Each checkout also has a delete button that removes it immediately,
without confirmation or a warning about uncommitted or unpushed work; it is unavailable under the same host-listing and
operation guards as bulk cleanup. The pinned header keeps the title, total and a close control visible while the archive
list scrolls. The pinned footer holds Close, then Delete all; phone widths stack these actions as full-width rows.
Global confirmation appears in its own bounded, keyboard-scrollable region directly above the footer; its actions are
brought into view and focused when it opens, and cancelling returns focus to Delete all. Per-host confirmations stay
inside their host sections. Initial focus stays on footer Close; sequential Tab follows header close, archive body, then
footer actions. Confirming a bulk delete closes the dialog immediately and sends one request per host with its frozen
IDs; the hosts progress independently. An entry's own Delete keeps the dialog open. On a host that reports accepted
cleanup, the row stays marked as removing until a later listing confirms it is gone; older hosts keep their existing
row-disappearance behavior. Close, header close and Escape remain available during that request. A trash delete is long
work the helm accepts at once: it reports the deletion's progress and each entry's result as ordinary state, so nothing
in the window waits on the request. Trash deletes do not block other actions, while the trash control prevents
overlapping deletes of its own. The listing is rechecked after the deletes settle. Refusals the helm reports remain
visible on the next dialog opening, and the trash button's tooltip tells the user to open the trash to see why. Notices
for a host disappear when a successful host registry refresh confirms that the host has been removed; disconnection or a
failed refresh does not acknowledge them.

While a trash delete the helm accepted is still running or a host reports that it is freeing space, only the trash
button's can turns upside down and bobs (static but still upside down under Reduce Motion). Its tooltip names the hosts
freeing space. The window rechecks only hosts that last reported active removal, every few seconds until they report
completion; no poll runs for idle hosts. A failed read retains the last removal report rather than implying that removal
finished.

Only the client that issued a session Delete plays an archive cue, and only when the host confirms that a checkout
moved: the row's dashed outline flies into the trash, which then wiggles briefly. Reduced motion skips both parts.

A trash delete, whether an immediate single-checkout delete or a confirmed bulk delete, is the permanent-cleanup
exception: it removes only recorded archived checkouts whose opened root and directory still match their ownership
evidence. It does not follow symlinks or cross mounts, and refuses an archive containing a retained session's working
folder; this protection does not cover a working folder reached through a symlink the user retargets after launch.
Missing or mismatched records can be discarded without touching a replacement folder; other failures are reported and
remain available for retry.

After those checks, permanent cleanup moves each selected folder aside inside the same trash directory and answers
without waiting for recursive removal. Its existing working-copy record remains until the files are gone. The trash
keeps that folder listed while removal is queued or running; its files may already be partly removed, so it is no longer
an untouched archive to recover from. One background remover frees the space sequentially without blocking launches,
restarts or session Deletes. A restarted supervisor resumes accepted removals. A failed removal keeps its folder and
record listed with the reason and failure time. **Retry** asks for one more attempt through the same cleanup checks;
Farhelm never changes permissions or removes files as another user. There is no timed retry in a running supervisor. The
listing reports queued or running removal separately from a retained failure. Older supervisors omit these facts and
keep their existing trash behavior.

A cleanup failure also appears in the navigation rail's notification bell, with its host, checkout, last known size,
reason and failure time. Its **Retry now** and **Open Trash** actions use the same cleanup checks and Trash dialog.
Retry acceptance alone never marks the notice resolved; a later successful listing must confirm the checkout is gone.

New sessions, including fresh checkouts whose configured root is inside the trash, cannot enter a directory accepted for
removal or its unresolved original location. Canonical aliases follow the same refusal. Startup also preserves an
accepted directory containing any retained session's working path instead of queuing its removal.

Confirmed 2026-09-28: archiving a checkout never blocks deleting its session. When the checkout cannot be archived
safely, for any reason (its folder or root no longer matching what was recorded, the move failing, or the outcome of an
earlier move attempt being impossible to establish), Delete still removes the session, releases the checkout from
Farhelm's management, and leaves the folder where it is. That outcome must never be silent: Delete completes with a
visible notice that the checkout was not archived, naming the path left behind and, where known, why. Wherever Delete's
result is shown, the notice is shown with it.

Repository suggestions combine successful repository launches for the same host installation with immediate Git clones
under the configured root. Discovery does not confer ownership, contact GitHub, fetch, recurse, run repository code, or
return credential-bearing origin URLs. Incomplete discovery is visible and does not prevent selecting a valid manually
entered pair. A saved repository setup remembers repository intent and agent choices: using it obtains a new preview and
creates a new checkout, rather than reopening its prior directory. Fresh creates do not populate ordinary folder history
with ephemeral paths; an explicit existing-directory launch can still do so.

Unlabelled search retains ordinary matching. Leading `name:`, `host:`, `harness:`, `model:`, `effort:`, `perms:`,
`folder:`, `recent:`, `tl:` and `gh:` labels offer or filter their respective actions, case-insensitively; unknown
labels and colons inside model IDs retain ordinary meaning. When no ordinary result matches, unlabelled text (including
an unknown label) offers a visible name action instead; it never silently becomes a name on submit. Accepting a name or
host action clears search without launching. Invalid `gh:` input cannot launch a hidden existing directory. Host,
installation, destination, title, agent choice and observed configuration changes invalidate an undispatched preview.
Late responses cannot restore its authority or steal focus.

### Lifecycle operations

The client supports: create, open, rename, restart, restart with, clone, replace, replace with, stop, delete. Sandbox
and container sessions additionally offer Suspend, Resume and Stop and suspend, with their environment-wide consequences
under [Sandbox sessions](#sandbox-sessions) and [Container sessions](#container-sessions). Retired containerized
sessions support only Delete.

Pending Stop, Restart, Replace and Delete requests do not block opening another session from the sidebar or quick
switcher, marking rows, clearing marks, or acting on other sessions. Each session excludes its own competing actions: an
open lifecycle confirmation or a pending request refuses another lifecycle action, New terminal or Clone on that
session. Opening the session remains possible, including while its Delete is pending. A Stop shows a spinner and
**Stopping…** on its row and at the top of the open session until it finishes.

Actions and their results survive switching sessions. A failure or partial result appears on the source session's row
and at the top when it is opened again. A view opened during Stop or Restart observes that operation's completion,
including the terminal refresh or reattachment it needs. A completed Replace opens its replacement only if the source is
still selected; otherwise it appears in the list and the user stays where they are.

A list rename opens in a modal editor owned by the list rather than by the row-actions popup. Listing updates may
reorder, filter, or temporarily fail without moving its textarea, so its draft, selection, composition, and source
session identity stay intact. A successful rename closes that editor; a refusal leaves its draft and error visible for
correction. A complete, authoritative listing that no longer contains the source disables submission without discarding
the draft, because a filtered, truncated, failed, or stale listing is not proof that the session disappeared. The editor
has a shared title and Close header, a scrolling body containing the current title, draft and refusal, and a Cancel/Save
footer. Tab follows Close, the title field, Cancel, Save; initial focus remains on the title field. Close uses the same
cancellation and busy-state rules as Cancel.

Quick launch uses the same editor in naming mode after creation, without holding up agent startup. Only that mode
selects the whole initial name and treats an unchanged submission as keeping the default without a request. Saving,
unchanged Enter, Escape and Cancel all return to the new session's terminal. Ordinary list rename keeps its existing
submission and focus-return behavior.

- **Stop** terminates the agent and its entire process tree — MCP servers, dev servers, and other descendants included.
  It then closes the session's terminal tabs and their processes, under the same lifecycle claim. The session remains
  with its agent terminal still viewable. With tabs open, Stop asks "Stop the agent and close N terminal tabs?", or asks
  about the tabs alone when the agent has already ended, unless session confirmations are skipped (below). A tab that
  cannot be closed does not prevent the other tabs from closing; the reply names the failed tabs and the client shows
  that notice. Restart and an agent exiting on its own leave tabs running. Tabs can be opened after Stop; another Stop
  closes them too.
- **Restart** relaunches the agent in the same working directory and resumes the session's own conversation (see
  Durability for the exact promise). Confirmed 2026-10-03: Restart always means the conversation is preserved. It is
  offered only when Farhelm knows the session's agent type, has captured its conversation, and can resume it: an agent
  launch of an agent type with conversation reporting, or a command launch that declared such an agent type and opted
  into Resume. When Farhelm knows it cannot resume, Restart is unavailable rather than starting fresh or running some
  other command, and Replace or Replace with is how such a session starts over: a Restart that silently loses the
  conversation is a bug, not a fallback. This is the only relaunch mechanism: the resume offered when opening an
  interrupted session is this same operation, not a separate feature. There is no fresh-restart variant — for a clean
  conversation, use Replace, or create a new session in the same directory. Restart on a session whose agent is working
  (its status reads working) confirms, stops the agent, then relaunches; a live agent that is idle, waiting for input,
  or whose status is unknown is stopped and relaunched without asking, because a prompt shown on every live agent gets
  clicked through unread. Harnesses with weak activity detection may therefore restart a busy agent unasked; that cost
  is accepted. The supervisor applies the same rule from its own reading at the moment of the restart, so an agent that
  started working after the user clicked is refused rather than stopped unconfirmed, and the next attempt asks unless
  session confirmations are skipped (below). Replace keeps its confirmation whatever the agent is doing unless session
  confirmations are skipped (below). Restart keeps whatever scrollback the session's agent terminal retained: the
  terminal is replaced by a new one for each run, which starts with the old one's scrollback above a blank screen in
  default modes. What is carried over is the scrollback's text with its colors, styles and links, up to the new
  terminal's limits, the oldest lines left out first (the terminal then says that some earlier output could not be
  recovered). If the new terminal does not start and Farhelm confirms that the failed start has ended, the session keeps
  its previous terminal, its agent stopped, and Restart can be tried again; when Farhelm cannot confirm that, the
  session is left with no terminal and its next Restart starts an empty one. The new terminal starts empty instead, with
  a log entry, when the old one's scrollback cannot be read or copied. When the terminal no longer exists (the process
  that kept it ended, or the host rebooted), Restart starts with an empty one. Restart does NOT preserve the previous
  run's last visible screen: the terminal is blank until the new agent draws, and a full-screen program's final frame
  (which was never in scrollback to begin with) is gone. Losing it is accepted. Farhelm must never capture a terminal's
  screen and paint it back into a relaunched terminal ahead of the new process — a frame with no process behind it looks
  live, accepts typing, and is overwritten when the real program draws, which is worse than blank. Restart touches the
  agent terminal only: the session's open terminal tabs keep running in their own terminals until they are closed.
  Restart may carry changed launch settings (Restart with, below); the launch kind, agent type, host, and working
  directory remain fixed. If a harness rejects a changed model or other setting while resuming, that is an ordinary
  launch failure; restart again with settings the harness accepts. After a Restart request has waited about two seconds,
  an adjacent status shows a spinner, the host and elapsed seconds, updated about once a second until the reply arrives.
  Restart makes no promise that a second press is harmless.
- **Restart with** opens the session launcher in restart mode before resuming the session's own or a user-named
  conversation. It edits model, effort, permissions, wrapper prefix and conversation for an agent launch, or command,
  resume command and YOLO assertion for a command launch. Launch kind, agent type, host, folder or destination, session
  name and initial prompt are held: their editing controls are hidden. Templates and template shortcuts may repeat a
  held value as a no-op; an actual change is refused without applying any edits. Kind and agent compare with the active
  launch kind and its agent, host with its usable recorded install identity, folder with the same text ignoring trailing
  slashes, and name with the raw title byte for byte. Paths are not expanded or resolved. Initial prompts and clearing a
  command's resume choice remain refused. Search results and recent setups that set a held field are still refused even
  when they name its current value. A template containing only equal held values applies silently as nothing. The held
  agent type, host and folder remain visible. Replace with can change the harness or folder, and Clone can change the
  host. The launcher shows the current settings and marks edited fields, and its primary action is inactive until a
  setting changes or a conversation selector is entered. For an agent launch with a stored resume command, it is
  available even without a captured conversation; then its conversation field is required and explains why. Cursor and
  OpenCode have no stored resume command, so they do not gain Restart with. Muse launches created before conversation
  tracking also retain their missing resume command until recreated. The launcher's ordinary controls, templates,
  validation and Enter rules apply. Its primary action reads "Restart", or "Stop and restart" while the shown agent is
  working, with the agent type, host and folder beside it. A working agent is stopped first with the user's confirmation
  on that action; a live agent in any other status is stopped first without it, as for Restart. The edited launch is
  validated exactly as a create validates it before anything is stopped, so an invalid edit leaves the agent running and
  the dialog open with the reason. A refusal leaves the dialog and its edits visible with the reason. The held folder
  never uses or updates the per-host folder default; its advisory check still applies. Restart with runs the resume
  command, so an edited start command of a command launch takes effect the next time it is cloned or replaced. Command
  launches offer this action exactly when Restart is. Its entry in the header's Restart menu remains visible but greyed
  out otherwise, with a hover tooltip and accessible description explaining why.
- **Clone** opens an ordinary, editable create form pre-filled from an existing session's host, title, and launch — the
  fresh-conversation counterpart to restart's resumed one unless the user names a conversation for this start. A session
  in a managed checkout starts on a managed checkout of the same repository, named `<title>-clone` with the first free
  numeric suffix when taken, as described under Managed checkouts. Other sessions keep their working directory; choosing
  an existing folder restores the source's directory and title. The source session is untouched: cloning starts a
  brand-new, independent create through the same form and the same confirmation described under Creation and identity
  above, so every field can be edited before submitting and the request can be cancelled like any other create. The
  source's stored launch is carried into the launcher verbatim: an agent launch's choices including omitted default
  fields, never rediscovered by parsing its composed command, or a command launch's command, YOLO assertion, declared
  agent type, and resume command. Cloning does not deduplicate titles — a duplicate is allowed, the same as any other
  create.
- **Replace** creates a new session — new id, fresh conversation, same host, working directory, title, and launch,
  copied exactly as stored — and then DELETES the source. For a command launch, "fresh conversation" means only that
  Farhelm starts the stored command again rather than a resume; a command that itself continues a conversation, such as
  `claude --continue`, does so. Confirmed directly from the row menu, with one inline confirmation and nothing to edit
  first, since the whole point is the same settings. Contrast restart, which keeps the session's own id and its
  conversation: restart continues a session, replace starts one over under the same settings. If the create fails, the
  source is untouched. If the create succeeds and the removal that follows fails, the reply names both sessions; whether
  the source is still there depends on how the removal failed, and the user checks or removes it by hand. Replace is
  offered wherever clone is offered. After about two seconds waiting for the reply, the sidebar, session header and
  interrupted-session card share a spinner, the host and elapsed seconds, updated about once a second. The status
  disappears on the reply. A refusal keeps its ordinary explanation, including partial failure of the source's removal.
- **Replace with** opens the same editable create form clone opens, pre-filled with the source's host, existing
  directory, title, and launch even when the source belongs to a GitHub checkout, so every field can be edited before
  launching — the key use is starting an equivalent session on a different harness or effort. Launching creates the new
  session and then deletes the source, with exactly Replace's create-then-delete contract and failure reporting (the
  same asymmetry: an untouched source on a failed create, both ids named on a failed removal). Unlike clone, it keeps
  the source's own host — clone is the way to start a session on a different host. Offered wherever clone and replace
  are offered. Clone, replace with, and New are one launcher — same layout, same controls, same search, same validation
  — differing only in what is pre-filled when they open and in what launching does (create; create then delete the
  source). Its launch button is the confirmation of that delete: while the source has anything alive, the launcher shows
  Replace's warning about the source beside it, following the source's state as the client sees it while the launcher
  stays open, and launching carries the precondition matching what the launcher showed at the click (see the
  confirmation rule below). A source that has more alive by then, such as one restarted while the launcher was open, is
  kept: the new session is still created first, with no liveness check before it, and the user gets Replace's
  both-sessions-exist error.
- **Delete** removes the session and its stored state, in any state, terminating the agent and tabs if running — with
  confirmation that says so when anything is still alive, unless session confirmations are skipped (below). Deletion may
  make partial progress before failing, including removing attachment files while retaining the session row for retry.
  There is no rollback guarantee. Report the failure visibly and allow a later Delete to finish cleanup; a retained row
  does not mean previously removed state has been restored. Stopping a live agent can take a few seconds, since it gets
  its own chance to exit first, and a session's row stays until the supervisor confirms the whole process tree is gone.
  From the moment a delete is committed until that answer arrives, the client that started the delete shows it
  prominently, as an expected wait rather than an error: the row shows a spinner and what is happening in place of its
  title (stopping the agent, stopping its terminal tabs when only those are alive, or, when nothing was alive,
  deleting), the open session's header shows the same in place of its actions, and a scrim over its terminal says it
  again. The terminal's own "Detached" notice, which the delete itself causes, is held back meanwhile; if the delete
  fails, the row and header come back with the refusal shown, and a held notice appears.

The off-by-default **Skip session confirmations** switch in Settings skips session row, header and bulk Stop, session
row and bulk Delete, row Replace, header Restart, Replace and Delete, interrupted-card Replace, and terminal-tab Close.
Each runs on the first click through the ordinary action path, with consent from the state shown by that render. It does
not skip an environment session's Delete in either form, including bulk Delete: neither the Delete that asks the
environment's Farhelm and may remove the environment with its last session, nor the whole-resource Delete. Nor does it
skip YOLO, host or trash questions, or the Restart with or Replace with launcher. Host setup and removal retain their
separate choices. Environment Suspend and Stop and suspend keep the host-wide activity confirmation described under
Sandbox sessions and Container sessions.

Where keyboard focus starts in a confirmation, and how modal dialogs keep it, is under
[Keyboard focus](#keyboard-focus).

Confirmed 2026-10-01: a destructive confirmation authorizes only what the prompt the user answered said would happen.
This covers every destructive confirmation (Stop, Delete, Restart, Restart with, Replace and Replace with, and permanent
deletion in the archived-checkout trash, whose answer applies only to the hosts and checkouts it was asked about),
including a prompt that rewords itself while it is open because the session changed underneath it: the answer applies to
the wording on screen at the click, never to a state the user was not shown. When the session setting skips a prompt,
the click authorizes only the state the screen showed, under the same guards. The request carries the matching
precondition, and the supervisor refuses, at the moment it acts, when the session has more alive than that: a Delete or
Replace whose prompt said nothing was alive (or that asked nothing, because the session showed nothing alive) is refused
if the agent or any terminal tab is running; one whose prompt warned only about open terminal tabs is refused if the
agent is running again; a Stop whose click showed no tabs is refused if any tab is open, and a Stop whose click showed
an ended agent is refused if the agent is alive again or cannot be confirmed ended, before stopping anything; a Restart
whose prompt did not say it stops a running agent is refused if the agent is working (as for a Restart with no prompt,
an idle, waiting or unknown agent is still stopped unasked). A refusal deletes, replaces or stops nothing (a Replace has
already created its new session, and reports both, as above), and the next attempt asks again from the current state,
skipping its question again if the setting remains on. No refusal opens a confirmation by itself. These races are
reachable in ordinary use because the command line and agents act on sessions while a GUI is open (see One GUI at a
time). Accepted: a terminal tab opened between a prompt that named open tabs (alone, or together with stopping the
agent) and the click is closed with the others; the prompt need not name the exact tabs it showed.

Process-tree ownership is session-wide. Restart reaps leftover descendants within the containment limits below before
relaunching. Stop and delete reap the agent's processes within those same limits. An agent exiting on its own does not
trigger a hunt for daemonized survivors; the session's next restart or its teardown does. Operations that need the
working directory — restart, opening a terminal tab — fail with a clear error naming the directory if it has vanished
since creation; the session itself remains, and delete still works.

This cleanup covers ordinary agent descendants, including accidentally daemonized processes, rather than hostile
same-account processes deliberately escaping cleanup. Confirmed 2026-11-08: for a launch or terminal tab Farhelm put in
a systemd scope, cleanup trusts that scope rather than scanning every process on the host. It does not stop a program
that deliberately moves itself into a separate group (for example with `systemd-run --user --scope`), or one that
carries the session's environment marker without ever having been in the scope, such as a manually started program or a
daemon left from before markers existed. This trades that backstop for cheaper cleanup. Farhelm omits the machine-wide
sweep only when containment is established for every target, at least one named scope can be checked through a usable
manager, every named scope is observed live then confirmed gone, and no explicitly identified process remains alive
afterwards. Confirmed 2026-11-10: jobs a user-run tmux moves outside those groups may survive Stop, Restart, Replace,
Delete or tab close; this is accepted as the intent of running them under tmux. Its server inside the scope still ends.
Cleanup does not inspect scopes for tmux or retain the sweep because of it. Delete also keeps the sweep when tab
discovery is incomplete, including a session with no terminal, and one that has been restarted at least once (an earlier
release's Restart could have moved it from an old tmux terminal, leaving tabs nothing lists any more). Unscoped or
uncertain containment, a scope failure or a surviving explicit process likewise keeps the full sweep, as does a scope
already gone before cleanup.

For a launch or tab without a scope (including macOS and a Linux launch made while no usable systemd user manager
answered), there is no cgroup to contain a detached descendant, and the guarantee narrows to the processes Farhelm can
still identify: those still descended from the session's terminal, and those whose environment marker it can still read.
Confirmed 2026-09-28: a detached process that hides or overwrites its environment survives stop and delete there. That
covers a process whose environment the system withholds from its own user, such as a non-dumpable `ssh-agent` or a
setuid program, and a daemon that rewrites its process title, which overwrites the environment the marker is read from
(nginx with its default settings, or Postgres started through `pg_ctl`, for example). A server that stays in the
foreground under the agent remains in the terminal's process tree and is still reaped. Likewise, with no cgroup to
consult, a create retried after a supervisor crash that also lost the session's terminal may launch again while detached
processes from the first attempt keep running. macOS offers no cgroup equivalent that an unprivileged process can use,
so these are accepted limits of such hosts, not defects to engineer around. The same limit applies to a terminal tab's
processes when the tab is closed, reaped, or deleted. Detached services started by shell initialization before the agent
launches are outside the cleanup guarantee: they may serve the user's login environment beyond this session.

Confirmed 2026-09-28: when Farhelm cannot confirm that the cleanup an operation needs has finished, the operation fails
visibly instead of reporting success or carrying on. Restart does not relaunch, and Stop, Delete and closing a tab
report the failure; a later attempt may retry the cleanup. "Confirmed" is judged by every mechanism the processes in
question were placed under: when a launch or tab ran in a systemd scope, the scope must be confirmed gone, even if the
portable sweep found nothing, because the scope can hold processes the sweep cannot see. A current belief that the host
has no usable systemd user manager does not excuse skipping a scope that such a launch or tab may have. A process
Farhelm tried to reap but could not examine counts as unconfirmed, never as gone. The accepted limits of hosts without a
usable user manager described above are not unconfirmed cleanup: processes Farhelm has no way to identify do not make an
operation fail.

Confirmed 2026-10-10: these cleanup and resume rules cover single ordinary failures. They do not extend to a supervisor
crash that lands at one exact moment of an operation, or to two independent rare faults coinciding (a crash and a failed
systemd probe, a transient database read failure and a host that reports no identity, a lost report and a reused process
number, and the like). Under those conditions it is accepted that a process on any host, the helm machine's own
included, is left untracked, is later killed without the usual warning, or keeps running beside a relaunch or a new
launch (Replace, New or Clone) in the same folder; that a session's recorded conversation is wrong until the agent next
reports one, so Restart may resume a different conversation of that agent; or that the helm reconnects a host to its
previous destination until the next reload. These are exceptions to Restart reaping leftover descendants before it
relaunches, to failing visibly when cleanup is unconfirmed, to Restart resuming the conversation the agent last
reported, and to a corrected destination taking effect, for those conditions only. In the maintainer's words, they are
accepted when they are of the "the process on the remote host got killed" kind. Conversation tracking also has the
accepted limits under Durability and resume, which need no crash or coinciding faults. Corrupting the supervisor's or
the helm's database, bricking an installation, or similar lasting damage is never accepted, however unlikely the
trigger.

### Session view

Sandbox and container sessions add their environment lifecycle controls and suspended card here, as described under
[Sandbox sessions](#sandbox-sessions) and [Container sessions](#container-sessions).

A launch — a new agent, Restart, Replace or a terminal tab — starts in the session's working directory. If it cannot
enter that directory, it fails clearly with an error naming it, without starting the requested program elsewhere.
Resolving a symlink to its target is acceptable. Agent launches enter the session directory after login-shell startup,
overriding a startup-file `cd`, and supply a matching `PWD`. Terminal tabs enter before their shell starts, so a `cd` in
the shell's own startup files still applies as it would in an ordinary terminal.

For opening a terminal tab, the session's working directory is a path, not a tracked inode or a preserved symlink
destination. Use that path with normal filesystem resolution. If a symlink along it now points elsewhere, opening the
tab there is explicitly acceptable. Do not add directory-identity tracking or symlink-change refusal for this operation.
A missing or unusable directory still produces a clear error. This does not change the separate agent-restart identity
check.

Opening a session shows the agent's real TUI, live. The session view supports additional terminal tabs: plain shells
spawned in the session's working directory, for poking at the workspace next to the agent. Tabs survive client
disconnects and supervisor restarts exactly like the agent terminal, but they are not durable metadata: after a host
reboot, tabs are gone and the user re-adds them; nothing recreates them automatically. A tab can be closed individually,
which kills that shell and its processes — that is the whole per-tab operation set in v1. A tab's processes include
whatever its shell's startup files start: containment begins before those files run, so a shared service (an
`ssh-agent`, an editor daemon, a detached personal tmux server) that a tab's startup files are the first to start
belongs to that tab and dies when the tab is closed, reaped, or ended by Stop or Delete (within the cleanup guarantee's
limits on hosts without a usable systemd user manager; see above). This is deliberately unlike the agent launch, which
leaves detached services started by shell initialization before the agent outside its cleanup guarantee (see above),
though cleanup may still reach one that stays in the agent's process tree. The agent's launch has normally already run
the same startup files, so a tab usually finds such a service running and only reuses it. That includes a tmux autostart
guarded by `TMUX` being unset: a session's terminal (see [Terminal experience](#terminal-experience)) is not tmux and
does not pretend to be: no `TMUX` is set at any point, so the autostart runs during the agent's launch as it would in
any terminal outside tmux. One that execs tmux replaces the launch, so the agent does not start and the session's
terminal shows the user's tmux instead; keeping a login shell's startup files from doing that is the user's to arrange.
A tab whose process exits on its own is reaped automatically and silently: the tab disappears as if closed, its
scrollback is discarded, and no notice or exit code is shown. This is deliberately NOT the agent terminal's contract —
an exited agent stays viewable with its scrollback — because a tab's shell exiting is the user being done with the tab.
A shell that dies before the tab's open completes still refuses the open loudly, with the shell's last words as the
error.

When a session's terminal contents no longer exist on a reachable host after a reboot, opening it shows the session's
metadata and says why there is no terminal, rather than an empty pane.

Container sessions' tabs, file links and downloads use the ordinary supervisor rules inside their container. Retired
containerized sessions offer no terminal or tab access.

### Session list

Environment-session marks, metadata, suspended rows and menu actions follow [Sandbox sessions](#sandbox-sessions) and
[Container sessions](#container-sessions). Their session-owned hosts have no permanent rail entry. Their sessions appear
under All, while Hosts and the attention area retain ordinary diagnostics and controls.

When hosts need attention, a compact host list sits below the app bar and above the session list's title and status
filter. It stays visible while sessions scroll; its own bounded row region scrolls when needed. It is absent when
nothing needs attention and while Hosts is open.

The chosen order is the list's title: **Recently active**, **Newest created** or **Title A–Z**, with a menu offering
those orders and **Compact**. Trash with its count, Templates and New are compact icon buttons beside the title; their
hover text and accessible names explain the actions, and New remains the sidebar's one filled control.

The open session is separate from the **marked** sessions. Shift+click on a row marks the inclusive range from the open
session to that row in displayed order, without opening it; each range click replaces the previous marking. When the
open session is not displayed, it marks only the clicked row. Cmd+click or Ctrl+click toggles one row on every platform,
including the open row. Shift takes precedence when both kinds of modifier are held. A plain accepted row click opens
that session and clears all marks. Shift marking does not extend the page's text selection, and ordinary dragging still
selects text. Row controls keep their own actions. The open row keeps its blue fill; a distinct side marker and tint
identify marked rows.

Marks belong to the window and are not saved. Counts, offered actions and targets include only marked rows displayed
now. Filtering or opening Hosts retains hidden marks but never counts or acts on them; failed listings expose no
targets. A complete authoritative listing prunes vanished session keys, while a partial, filtered, failed or
non-authoritative reply does not establish disappearance.

With any effective marks, **N marked**, **Stop**, **Delete**, a metadata menu and Clear replace the ordinary title,
count and icon buttons; the filters and any truncation or unsupported-filter warning remain available. Stop and Delete
apply to every marked row displayed at acceptance. **Pin** applies only to unpinned marked rows and **Unpin** only to
pinned ones. **Mark read** and **Mark unread** apply only to marked rows whose own menus offer that action. Metadata
choices reuse the ordinary writes and row-local errors, and clear marks once their eligible writes have been dispatched.

Each bulk Stop or Delete batch runs one request at a time on each host, with different hosts progressing independently;
a second batch started while the first is still running has its own pace, so it can send to a host the first batch is
also working on. Accepted queued rows remain pending until their completions finish; filtering or a listing failure does
not cancel accepted work or add new targets. A failure remains on its own row without undoing successful siblings. Each
refused row gets the ordinary action-specific not-started message, including a row engaged by its own question or
rename. An accepted batch clears its own targets when every completion has finished, including when some rows failed or
were refused, only if the user has made no newer marking choice or opened a marked-actions menu or bulk question.
Otherwise it leaves the current marks alone, including its original targets, even if that newer menu or question was
then cancelled. Pending rows do not prevent opening sessions, marking other rows, Clear or actions on other rows.
Automatic selection after Delete waits until every admitted Delete has settled, queued rows and other hosts included,
and never opens a session that is still being deleted. Pending Stop batches do not delay automatic selection.

Bulk Stop and Delete ask once when any target would need its ordinary confirmation, unless **Skip session
confirmations** is on. A batch containing an environment Delete always asks. Their structured modal lists one row per
target with status, name, full host and what the action will do, with the same conditional managed-checkout consequence
for Delete. Environment rows say which Delete they get. Rows of a sandbox or container the helm is connected to state
both outcomes: the session and its checkout are deleted, and if it is the last session there, the sandbox or container
and all its files go too. Rows of one the helm could not ask when the modal opened name the whole-resource Delete
consequence, including when several marked sessions share it; that Delete runs once per resource. Sandbox rows also name
continued billing for Stop. Stop names the agent and terminal tabs it ends, with a separate no-tabs precondition for
every target whose displayed state showed no tabs. The answer authorizes only those listed identities and drawn
consequences. An accepted changed target rewords the question and replaces its opening; authoritative disappearance can
remove it, while a filtered, partial, failed or non-authoritative omission cannot. An old answer cannot authorize a
replacement question, and answer-time admission may only remove targets no longer displayed or marked. Changed facts or
confirmation preferences never dispatch a destructive action by themselves. Cancel, Close and Escape dismiss their
current question without clearing marks. After confirmation closes the question, keyboard focus returns to the marked
count, which remains programmatically focusable. Pending launches and actions on other sessions do not lock the bulk
buttons; each action still refuses its own busy targets. Completion and failures use the ordinary rows and open-session
surfaces.

Escape clears marks only from the session list, without preventing default handling, and yields to a menu, dialog or
terminal that owns the key. Quick switching and shortcuts keep their existing meaning: session shortcuts act only on the
open session, whatever is marked. Each row's menu still acts only on itself. Rename, Clone, Replace, Replace with and
Restart remain single-session actions.

Below the list title, a single-status segmented choice offers **All**, **Running**, **Waiting** and **Idle**, without
counts. It combines with the host choice in the helm's query and is not persisted. Ended sessions appear only under All.
A narrowed list keeps its visible "N of M" count, including when every session matches. Hiding the selected session
changes neither that selection nor its open terminal.

A chosen host appears beside the status segments as a removable chip with its mark and short name, matching the rail.
Its tooltip gives the name and destination, without state or attention details. A removed host keeps the rail's
unregistered placeholder, with removal still available. The chip truncates long names and wraps below the status
segments when needed. Removing it chooses All exactly as the rail does, without changing status, sort or the open
session.

An empty filtered list names its filters: "No sessions on [host mark] devbox.", "No waiting sessions on [host mark]
devbox." or "No waiting sessions." It uses the same host identity as the rail, including aliases, Local and removed
hosts. Buttons styled as links below the message clear each applicable filter: **Show all hosts** and **Show all
statuses**. After a clear button or the chip removes its focused control, focus returns to the rail's All button. While
a new filter is loading, the empty message keeps describing the result still on screen; the chip and status segments
follow the current choice. An unfiltered empty fleet still says "No sessions", and a truncated listing keeps its
qualified no-match message.

One flat list across all registered hosts, with an always-visible host rail. **All** starts selected, followed by each
registered host in registry order and **Add host**. Each avatar uses that host's chosen icon and color, with its short
name in a small caption underneath, truncated to the 56px rail. The local caption is `Local` unless it has an alias. The
collapsed rail is 56px wide and never widens on hover or keyboard focus. Each avatar's tooltip and accessible name carry
its display name, its destination when different, and its state, including any existing last-read or attention
explanation. The local display name is `Local (this machine)` unless aliased, with no destination suffix. An ordinary
keyboard-reachable button below Hosts, Settings and Help shows » and "Expand navigation rail" while collapsed, or «
Collapse and "Collapse navigation rail" while expanded, and exposes its expanded state. It expands the whole rail, its
foot included, to 220px beside the session list. Expanded host rows show the display name and a smaller destination
underneath when different; long names truncate and remain available in full in the tooltip. Captions disappear, All
hosts, Add host and the foot controls show labels, and each whole row highlights and selects as one click target.
Picking a host keeps the rail expanded. Badges, menus and actions retain their behavior. Each device remembers expansion
locally, independently of the helm; a fresh device starts collapsed. Touch screens follow the same rules.

Choosing a host sends a server-side query immediately and is not persisted, while All widens it again. Status, sort and
the selected session remain unchanged. Unavailable hosts retain their last-known sessions; a removed selected host
remains explicitly selected and unavailable until another host or All is chosen. The **Hosts** control above Settings
and Help swaps session content for complete host management; hosts needing attention remain visible in the compact list
above sessions while management is closed. Sessions is shown initially; opening or closing Hosts keeps the current
query, sort, selected session and details choice. The control keeps focus and exposes its pressed and expanded state.
All or a host avatar returns to Sessions, as does an accepted quick-switcher pick or New handoff; a refused handoff
leaves the panel as it was. Host reading, session reading, sounds and in-flight host work continue while their ordinary
content is concealed. Rail menus and host dialogs remain available over Sessions. The list can be ordered independently
by most recent activity, by creation time, or by title; the order someone picks is remembered by the helm as one
preference shared by every client, together with the last-selected session, visit history and compact-row choice, and
most recent activity is what a client shows until someone picks otherwise. The same shared preference row carries
terminal web-link click ownership, the quick-launch template name and number-key bindings, the session-confirmation
opt-out, the quick-launch naming choice, terminal-frame coloring, the desktop download-opening types, automatic
remote-host updates, and whether host setup or host removal confirmation should be skipped after an explicit permanent
answer. A change to these preferences, by an agent or the command line included, reaches the open window as it happens;
automatic session selection also rereads the shared visits immediately before choosing, template shortcuts read their
bindings on each invocation and the desktop helm reads the current file-opening list for each hover and open request.
The gear at the foot of the host rail opens a settings dialog. Its left sidebar lists **Sessions**, **Appearance**,
**Hosts**, **Recipes**, **Tensorlake**, **Containers**, **Credentials**, **Terminal**, **Checkouts**, **Agent trust**
and **Notifications**, followed by **App** only in a desktop app whose updater is active. Each section has an icon; the
selected one uses the solid selection fill. One selected section appears beside the list, with a matching heading;
notifications keeps **Sounds** and **ntfy alerts** sub-headings. The dialog keeps one height across sections, capped by
the window; short sections leave space below their content, and longer sections scroll inside the pinned header and
footer. At 620px wide and below, the dialog shows either the selected section or the list; **‹ Settings** returns to the
list. The gear always opens sessions and focuses its first switch, including at phone width. Trust hints instead open
agent trust at the relevant switch and scroll its block into view. Section buttons follow the platform's normal Tab
rules. Switching sections preserves unsaved fields, outcome lines and pending saves; closing discards unsaved fields.
Escape and either close control return focus to the gear.

**Credentials**, with a lock icon, holds named GitHub tokens, agent subscription logins and API endpoint tokens in three
lists. Names use the recipe-name grammar, are unique across all kinds, and cannot be changed after creation; neither can
the kind of a saved credential. Lists show the name, recipe use and saved or replaced time. Editing opens in place with
Back, Name, a write-only secret field, Delete, Cancel and Save. The agent-login editor selects Claude Code or Codex:
Claude accepts the token from `claude setup-token`; Codex accepts a pasted or selected `auth.json`, validated only as a
JSON object of at most 64 KiB. An empty replacement keeps the saved value. Settings never returns a saved secret; a
failed save keeps the draft and shows its reason. Closing discards unsaved text without a recipe-style departure
question. Recipes select API endpoint tokens, GitHub tokens and Claude or Codex subscription logins by name; setup,
connection and replacement delivery resolve current saved values. A successful save with failed deliveries closes and
clears the secret editor, and shows the delivery failures on the Credentials overview. The Codex login editor explains
that copies share a rotating refresh token: when one copy renews, the other copies cannot renew and need a fresh
sign-in, so use one login per host. It recommends a proxy or load balancer through an API endpoint for shared inference
access and makes the user responsible for following the provider's terms of service. Delete is unavailable while a
recipe uses a credential, with the recipe names shown. Reference changes refresh an open credential editor without
discarding its unsaved replacement text.

Credentials includes the same Tensorlake API key form as Tensorlake, with Save, Test and Clear, labelled **Tensorlake
API key** there and **API key** in Tensorlake. The Credentials overview and every credential editor each show one
prominent danger-tinted **Not encrypted on disk** warning explaining that anyone who can read the helm's state directory
can read the values. The overview's warning names the Tensorlake API key, so the key form there shows none of its own;
in Tensorlake the key form shows its own warning, worded for that one key. A save or clear in either updates both forms'
saved-key metadata immediately; each keeps its own unsaved replacement draft. Credentials does not rename or duplicate
the stored Tensorlake key.

**Appearance** chooses **System**, **Dark** or **Light** for this device, with System as the default. System follows the
device's current appearance, including changes while Farhelm stays open; an explicit choice overrides it. The desktop
app remembers the choice locally, independently of the helm. The resolved palette applies before application content
appears, and changes live. The terminal keeps its dark colors in both palettes. Sessions has an off-by-default **Skip
session confirmations** switch. Its help names Stop, Delete, Replace, Restart and tab Close and excludes YOLO, host and
trash questions (see Lifecycle operations). Beside it, **Name sessions on quick launch** is on by default, including
when the helm has never stored a choice. Turning it off disables the naming prompt for quick launches. Beside it,
**Color terminal frames by host** starts off and colors every session's thin terminal frame to match its host icon when
turned on. The **Hosts** section has the switches `Set up new hosts without asking` and `Remove hosts without asking`,
on when the respective confirmation is skipped. Turning one off restores that confirmation; turning one on makes the
same choice as the host dialog's permanent answer. Changes take effect immediately in the current client. Each switch
explains the current behavior, and each host dialog's permanent answer points to the gear as the place to undo it. Hosts
also has **Update remote hosts automatically**, off unless explicitly enabled, with help explaining that the helm
updates remote hosts as soon as updates are available, with no window open, and never updates its own machine this way.
This shared choice survives helm restarts, and the helm acts on a committed change immediately. A failed save restores
the last known value and shows an adjacent error. The **Tensorlake** section holds the helm's Tensorlake account, for
hosts and sessions that are Tensorlake sandboxes. It opens, above the API key, with a large red warning that begins
"EXPERIMENTAL:", says Tensorlake support is early and experimental and will change between releases, and makes the user
responsible for sandboxes not running and billing when they do not want them to; it shows whether or not the section's
settings load. Its API key is explicitly saved helm configuration, shared by every client and kept outside the
preference row like the ntfy token: the helm stores it unencrypted in its state directory, and the section says so
plainly. The key is never returned; the form shows only whether one is saved. Pasting a key and pressing **Save** stores
or replaces it (surrounding whitespace is dropped), a blank field keeps the saved key, and **Clear** removes it.
**Test** lists the account's sandboxes with the saved key and reports how many it can see, or why Tensorlake refused,
inline. Test uses the saved key, never unsaved text in the field. Until Farhelm's own Tensorlake SSH key (see
[Security](#security)) exists, the section offers **Create SSH key**; opening Settings never creates it. Once it exists,
the section shows the one-time command that registers it with the user's Tensorlake account,
`tl sbx ssh keys add --name farhelm-<tag> '<public key>'`, with a **Copy** button. The command carries the public key's
text, so it works on any machine where Tensorlake's `tl` CLI is signed in. The section says that Farhelm cannot check
the registration, because an API key cannot list SSH keys. `<tag>` is the helm tag: four lowercase letters or digits
derived from the helm's registry, stable for the life of its state, which the names of the sandboxes this helm creates
also carry. Recipes and their editor live in the separate **Recipes** section (see [Topology](#topology)). **Sandboxes
not on this helm** keeps its heading and Refresh visible even when none are found. The off-by-default **Create
Tensorlake sandboxes without asking** switch saves only when the helm acknowledges it and shows only the stored choice;
`Set up new hosts without asking` never covers creating a sandbox. The section also notes that on Tensorlake's Usage
Credits plan Tensorlake suspends an ordinary host sandbox after 24 hours without a connection from Farhelm. Session
sandboxes are always created with a fixed 30-minute idle timeout. The creation switch's help covers both hosts and
sessions and explains that timeout. The **Terminal** section chooses whether web links open in Farhelm without passing
the click on (the default), open in Farhelm and also pass the click on, or only pass the click on. The latter two
choices explain the risk of a second browser tab and that a program without mouse reporting cannot act on a click. This
choice is the helm's and takes effect in every mounted terminal immediately. In the desktop app, the **Terminal**
section also has tunnel consent and a free-form, comma-separated **File types to open with Cmd-click** field (Ctrl-click
outside macOS). It defaults to `md, txt, json, png, jpg, jpeg, gif, webp, bmp, avif`; saving an empty list disables
opening while downloads still save. This field requires an explicit, acknowledged save, but its value lives in the
shared preference row. The dialog shows one prominent warning beside the field for each listed type outside `md`, `txt`,
`json`, `log` and the image types in that default; HTML-like types have a browser-specific warning, and all other
unrecognized types have a generic warning. The warnings follow the draft as typed, including when another entry is
malformed. A refused save leaves the draft and current helm policy unchanged and shows the reason inline. Under
**Notifications**, sounds has three device switches (see Status), and **ntfy alerts** has an off-by-default switch,
server URL (default `https://ntfy.sh`), topic, optional access token, and priority (Min, Low, Default, High, Urgent;
default Default). These are explicitly saved helm configuration, shared by every client, outside the preference row.
Save refusals appear inline. The stored token is never returned: the form shows whether one exists and allows replacing
or clearing it. Changing to another server clears a saved token unless a new one is entered, and the form says so before
saving. **Send test** uses the saved configuration, even while disabled, and reports success or failure inline. The
**Checkouts** section holds the explicitly saved **Default checkout folder** and, below it, a monospace multi-line
post-clone command field and the default-on **Keep a checkout ready for faster launches** switch (see
[Managed checkouts](#managed-checkouts)). The switch's help explains one extra checkout's disk space per repository and
host, promptless login-shell preparation, earlier file times and Git hooks/filters, and discard on the next create or
after 30 days unused; it links the full earlier-clone caveats. The folder's help explains expansion on each host,
first-use folder creation, and that the default applies to hosts without their own folder, set in each host's settings.
The command's help explains that it runs in each new checkout on its host, after cloning and before the agent starts,
that changes affect future checkouts only, and that command-line host overrides keep their command. The installed
desktop app's dialog also has an **App** section with a switch for installing updates automatically (see
[Installation and updates](#installation-and-updates)); unlike the host choices it is a setting of that app
installation, kept with the app rather than by the helm. The two host confirmation choices apply to every host and
client of this helm; existing controls with a natural place in the main UI stay there. No client keeps its own copy:
every client reads the helm's preference once after connecting and writes it on change, so a window opens in the same
order and on the same session after a reconnect as before it. Per-client persistence — a desktop state file, anything
that lets two clients remember different answers — is not wanted for these shared preferences, including welcome-card
and announcement dismissal (appearance, sidebar width, host rail expansion, terminal text size and sound switches, by
contrast, are deliberately per device; see Session list, Terminal experience and Status). A client that asks the helm
for no particular order gets creation time. No mandatory hierarchy. Sessions may carry an optional parent reference
usable by the API, but parentage does not nest the list and implies nothing about VCS state. Parent tracking is not
comprehensive: `farhelm spawn --parent` can record it, while `farhelm agent create` and `clone` need not record the
asking session.

The sidebar can be resized by dragging its right edge, between 296px and 656px, with 396px as the default. Its 56px host
rail leaves the session column between 240px and 600px, with 340px as the default. Double-click the edge to reset it, or
focus the separator and use Left and Right to step by 10px. Its width is remembered per device, like terminal text size:
it is about the screen in front of the user, rather than a preference all clients should share. The main pane keeps its
320px floor; a narrower window scrolls horizontally. Expanding the rail adds 164px to the sidebar's shown width and
takes that space from the main pane, preserving the session column's width when there is room. In narrower windows the
sidebar grows only as far as the window allows while leaving the main pane its minimum width; the session column gives
up the remainder. Collapsing gives the width back. The stored resize width and its range keep their meaning, independent
of rail expansion.

Host setup, removal, uninstall and refusal dialogs normally pin their header and answers. A window too short to fit them
scrolls the whole card, so every explanation and control stays reachable. A shortened heading retains the full
destination in the body.

Pinned sessions appear first in the sidebar in a manual order shared by every client of the helm, independent of the
list's chosen sort. A new pin goes at the bottom of the pinned group; unpinning and pinning again puts it there again.
Move up and Move down in the row menu rearrange pins relative to the displayed neighbor. Unpinned rows keep the chosen
list order below. Pins work even when the session's host is unreachable; host selection and other filters still apply
without changing the stored order. Under most recent activity, even a stale pin comes before connected running and
waiting work. The quick switcher keeps its existing activity order and does not group pins first. Deleting a session
removes its pin; Replace and Replace with carry the pin and its position to the new session, and Restart keeps it on the
same id.

Hovering an eligible pinned row reveals a grip beside its name. Dragging that grip within the pinned group shows the row
lifted and a drop line; release moves it relative to the pin at that line. Escape, interrupted pointer capture or
releasing outside the pinned group cancels. Dragging never pins or unpins a row, crosses the separator, or rearranges
unpinned rows. The row menu remains the keyboard way to rearrange pins. A newly pinned row waiting for the order hold to
end has neither a grip nor Move actions until it joins the movable group.

Among unpinned sessions, the option labelled most recent activity sorts connected running and waiting sessions first,
then every other session — idle, unclassified, ended, and anything on an unreachable host. Inside each group the order
is the most recent observed START of a work burst: a session promotes when it moves from known idle or waiting into
running (both idle-to-running and waiting-to-running count) and when it starts waiting on the user, since a question for
the user is work the user should see; later output inside that burst, and a question that stays on screen, leave the
position alone. Only a change the supervisor itself observed between two of its own looks counts: the first look after a
supervisor restart, a session restart, or a recovery from failed screen captures only establishes where the session
stands. Moving between the groups is what idling and completion do — a finished session drops below still-running work
without its burst key moving, and a session returning to running rejoins the first group on that same key. Creation time
is the stable fallback where an older supervisor has no work-start observation. The last-activity time shown in the row
and the seen/unseen comparison stay independent of the order: they describe when the agent was last working (see Status)
and neither moves a row. The grouping and the key are authoritative helm and supervisor data, so every client agrees
without keeping a private rank.

A client holds back reordering while the pointer is over the list, so a row does not move out from under the pointer on
its way to a click. The hold is display-only and its snapshot is discarded when it ends: the shared pin order and chosen
unpinned order stay authoritative, and the client shows them again as soon as the pointer leaves the list or has been
completely still over it for 5 seconds (a scroll counts as movement). During a hold, rows keep updating in place; a
session that disappears is removed at once, since a deleted session must not stay clickable; and a new session appears
at the bottom of the list, moving no row above it, until the hold ends and it takes its place. Changing the list's host,
filter, or order is asking for a different list, so the hold does not apply to it. While a session row's menu is open,
the hold is kept until the menu closes, even if the pointer leaves the list or stays still, because releasing it would
move the very row the menu belongs to; pointer movement over the list starts the 5 seconds of stillness over. Pins
present in both the original held snapshot and the current reply follow their shared manual order within the slots those
pins already hold, so a manual move takes effect without waiting for the hold to end. Other pin and unpin changes update
in place during a hold and move to their grouped positions when it ends. Newly pinned rows offer no Move actions until
then, including a new pin contiguous with the leading pinned run; original pins cannot use that new pin as a Move
neighbor during the hold. A muted pin glyph immediately before the title names a pinned row in its tooltip and
accessible text. A heavier separator with a small gap below follows the displayed leading contiguous run of pinned rows,
only when an unpinned row follows; there is no Pinned heading. A newly pinned row held below that run does not move the
separator there, and unpinning a row inside the run shortens it immediately.

Without effective marks, the list carries a count beside its title: **N**, or **N of M** when narrowed, where N matched
and M is the whole fleet. A truncated reply or an unsupported filter retains its explicit warning rather than claiming a
complete matching count.

The list is served and rendered WHOLE. The fleet this product is for is tens of sessions across a few hosts, not
thousands, and the design assumes that scale outright: every supervisor answers a listing with its entire list in one
reply, the helm holds every host's list in memory and sorts and filters the whole fleet there, and a client receives one
array and renders all of it. Each of those replies is cut at a fixed cap of a few hundred rows, and a list the cap cut
says so in the same place the count lives — "could not read to the end" means exactly that the cap was hit, never that
something went wrong reading — rather than presenting a partial list as the whole one. No pagination, cursors, streaming
or incremental listing, or per-order server-side indexing is wanted at any layer, and it is fine for the helm and every
client to hold and sort the entire fleet in memory; a fleet that outgrows the cap is outside what this product is built
for, and the notice is the whole of the answer to it.

A row's first line shows its status (drawn as described under Status), locality mark, title, agent label, a notification
bell when the session has notifications (see Status), and last activity time; status and locality marks occupy aligned
columns. A local row carries its host's chosen icon and color, defaulting to the red laptop; a remote row carries its
host's chosen icon and color, defaulting to the cloud in the ordinary foreground. Compact ended rows replace the live
dot with a distinct ended-state icon, preserving complete status details in accessible text and a tooltip. Noncompact
rows show the complete ended status on a separate full-width line that wraps instead of truncating. Times share a
right-aligned column before the row menu. A session whose host cannot yet be placed either way marks neither, rather
than guessing. Its second line shows the host's short name when that host is in the current registry read, otherwise the
full host name supplied by the helm, then `:`, then its working directory. Without either a registry match or a supplied
host name it shows only the directory, rather than inventing a local identity or a dangling separator. Other surfaces,
including host management, details, dialogs, tooltips and agent output, keep the display name. The second line is hidden
when the helm-wide compact preference is on, which defaults off and is shared at the next preference seed across
clients. The working directory and launch command remain abbreviated only where shown, with the directory in the row's
hover summary card and the full invocation on its agent mark on web and desktop; an abbreviation is never the only place
a value is recorded. The same card appears from the title or any unmarked part of the open button, in either density and
on keyboard focus. Every session-row hover, including the summary card and notification bell, sits just right of the
sidebar, vertically centred on its target and shifted only to fit the window. A left-edge notch points back to the
target's centre, clamped to the box's straight edge, and a thin accent ring identifies that target while its help shows.
When there is no room to the right, it uses ordinary above/below placement without a notch or ring. Tooltips outside
session rows, including the actions menu's items, keep their placement without these cues. The summary card names the
title, status, agent, host, checkout when managed, directory, start and last-activity times, and what a click does;
meaningful marks keep their own hover text. The summary may clip very long values or whole rows in a short window; the
session view provides those details in full. Its agent line retains the stored launch facts and adds "YOLO observed"
when the shield has status-line evidence, including when the launch was already YOLO. The actions-menu header describes
only the stored launch. A row's own actions menu, beyond the lifecycle operations above, also offers a mark read / mark
unread toggle — reachable there or by clicking the dot itself — that sets the session's seen state directly (see
Status). Directly after it in the menu's first metadata group, **Pin to top** / **Unpin** toggles the shared pin.
Directly after Unpin, **Move up** and **Move down** rearrange eligible pins relative to the displayed neighbor; an
unavailable direction is hidden, and unpinned rows offer neither action. Accepting a move closes the menu and keeps
keyboard focus on the moved row's menu button. Every session row carries one permission mark. An amber slashed shield
means the session's launch is YOLO by the rule under Creation: its effective agent-launch permission is custom or YOLO,
or its command was asserted to be YOLO. A green plain shield means an agent launch in guarded or an approval mode, or a
command asserted not to be YOLO. A session created before launch kinds existed that was not an agent launch has no
assertion and carries an amber question mark. Hover and screen-reader text name the specific agent-launch mode (guarded,
custom, approve, smart approve, chat, or YOLO), or say that the mark is the assertion made when the command was
launched, by the user or by an agent, and not something Farhelm checked, or that a pre-existing session's command was
never classified. A grey dashed shield means the agent's own setting, including older sessions started without guarded
flags. Its hover says Not classified and that the host guard treats it as YOLO. For Claude, Grok, Muse and Cursor,
Farhelm also tries to read the agent's permission mode from its status line. A definite no-checks reading changes any
shield other than YOLO or asserted-YOLO to the YOLO shield. Hover and screen-reader text name the observed mode and say
it was seen on screen, while retaining the launch mode and command. Once observed, YOLO stays for the rest of the agent
run: a recognized non-YOLO mode, an unreadable or obscured line, or an unrecognized mode cannot clear it. This errs
toward safety: restoring a safer shield would risk leaving it there if a later switch back to YOLO went unnoticed. This
never makes the shield safer than its launch choice. Restart, Replace and resume start a new run with no observed
evidence; a supervisor restart also clears it until new definite samples arrive. Command launches with a declared agent
type are included; legacy sessions and the other agents are not read. This is advisory and best effort, not a promise of
detection: a missed or late reading is outside the guarantee. It cannot see allow rules, hooks or managed policy. It is
display only; host YOLO guards, spawned-session permission inheritance and approval decisions still use the stored
launch alone. Every guarded hover names the agent's mode and meaning, shows the actual stored command under Launched
with, and explains that allow rules, hooks, plugins, always-allow answers, sandbox settings and in-session switches can
loosen it; agents may change behavior between versions. A custom hover says the flags were the user's, that Farhelm does
not check them and counts the session as YOLO, and shows both the flags and the full stored command under Launched with.
The launcher's permission choice has a separate ? button with that help, opening the docs section on permission modes.

Agent marks use distinct identity colors where their owners' terms allow: a published owner hue where one exists,
otherwise a Farhelm choice, with shades suited to each theme. Codex, Goose and the ordinary Terminal mark stay grey.
These colors say which agent it is, never its activity or permission mode.

On a solid selected row, host, agent and permission marks use the inverse foreground while retaining their shapes, hover
explanations and accessible meanings.

Hovering a live status dot, agent mark, or permission mark explains that mark. A clickable dot also names its mark read
or mark unread action. The hover text uses the same status and permission meaning the row exposes to assistive
technology. Exactly how a row lays out its lines and pixels is an implementation choice, covered in SPEC_impl.md rather
than here.

In the web and desktop clients, hover text appears about 300 ms after the pointer rests on a control or icon that has
it, or keyboard focus reaches one, and at once when moving on to another while one is showing; it sits above the control
so the pointer does not cover it, except in session rows and host progress, whose hovers sit beside the sidebar. Every
clickable control and every icon that carries meaning has hover text saying what it is or does, plain-word buttons
included, saying more than the label where it can. The one exception is an item in a row's actions menu that already
shows a description line under its label.

Per-host connection state is always visible on each rail avatar, with its name and phase available on hover and to
screen readers. A connection dot keeps the phase's meaning; an available update adds a badge, setup/update/uninstall
adds a ring, and an unreachable host's identity mark is dimmed. Host colors identify hosts independently of state. A
setup or Update warning has its own mark and hover explanation, alongside the connection dot, available-update badge and
any running ring; acknowledging it does not change the host's diagnostic or connection state. An uncertain initial Add
marks only its authoritative matching host, if one has registered; otherwise its existing Add notice remains. A
destination or installation change that leaves an Update outcome uncertain warns that same host entry; a Connected state
or a current version alone does not acknowledge the mark. A failed registry refresh retains the avatars and names their
state as last-read; a failed first read invents no hosts. The rail scrolls independently while Hosts, Settings and Help
remain reachable at its foot. A compatible supervisor whose build is older than the helm's is still connected and
usable, but its row says `Old version` as an advisory; an incompatible protocol handshake remains `Needs update`. On an
SSH host with Update available, an outlined `↑ Update` button accompanies those words: amber for an optional
compatible-build update, red for a required update when the host's protocol is lower. The dot keeps its color and the
accessible status keeps its words. Hover names both builds (and both protocols for a skew), says whether updating is
optional or required, and explains that clicking updates the host to the helm's version. Clicking starts the menu's
Update without confirmation; inline progress replaces the button. Local hosts, hosts with update options still loading,
and hosts occupied by setup or another run retain the plain words. The helm's own machine is not updated from the panel:
its row menu shows Update greyed out wherever it would otherwise be offered, saying to run the installer again, and
choosing it sends nothing. The installed desktop app is the exception: there the local row always offers Update, and
choosing it checks for a newer release and installs it (see [Installation and updates](#installation-and-updates)). A
host whose supervisor is newer than the helm (a newer build on the same protocol, or a higher protocol version) says
`Too new` instead, is not offered Update, and its hover names the host's version, the helm's, and their protocol
versions. If either build string cannot be parsed as a semantic version, or the helm is an unreleased development build,
age is unknown and the row stays `Connected`. The host count, its unpersisted details switch, and the secondary add
action share one header row in Hosts, alongside Update all. Every expanded host row shows its phase words, including
Connected. Host actions open on demand from the row menu or the same menu on its rail avatar, with the older-host update
button also available inline; details reveals the version, identity, session count, remedies, diagnostics, and
provisioning progress under every row. Templates use the neutral secondary tier for routine row actions and the primary
tier for popup affirmatives; the rail uses native buttons with an explicit selected state, and session creation remains
the filled primary action. Destructive confirmations use the danger tier, and explicit menu, tab, and composer controls
retain their purpose-built styling. Sessions on an unreachable host stay in the list from the helm's last-known
knowledge (which survives helm restarts), with a neutral cause mark such as `Waiting for supervisor` or
`Host needs update`, rather than vanishing. Compact rows carry the same cause in accessible text and hover help.

The Hosts control at the foot of the rail carries an `↑` whenever any rail avatar has an available-update badge. It is
red when any update is required, amber otherwise; its accessible name and hover help give the number of hosts.

The attention list above sessions has one row per host, with its rail mark and short name, its state, and the action
that state offers. Live setup, update or uninstall progress takes precedence over a problem, which takes precedence over
an available update. Working rows say **Setting up**, **Updating** or **Uninstalling**, followed by the current step's
name when known, ellipsized to fit. Hovering anywhere on a working row opens the identity button's shared progress card,
including its bar, gaps and padding. Moving within that row keeps the card in place, and pressing anywhere dismisses it.
Keyboard focus on the identity button opens the same card, with the same detail as the Hosts progress line. Before step
data arrives, the whole row shares the button's plain tooltip. Their marks and bars have no separate plain tooltip.
Step-name changes do not invalidate session menus below the list. Working rows show a progress bar, filled as steps
complete when a step count is available and otherwise left empty and static, which keeps the row's height; they have no
action button. A connection interruption during that work does not replace its progress with a problem. Problems appear
first, then working hosts, then updates, in rail order within each group.

Problem rows offer **Details**: Unreachable (worded **Unreachable, retrying**), Identity mismatch, Identity unverified,
Duplicate, Retired, Unrecognized, a protocol skew without an available Update or with the host's higher protocol, a
environment resume or tunnel failure, or an unacknowledged setup or Update warning. That last case reads **Setup or
update needs attention**. Without another warning, tunnel failure or live work, Connecting, a connected current host, a
usable connected host running a newer build, and an environment host ordinarily suspended, suspending or resuming do not
need a row. A host leaves when its condition clears. **Details**, or choosing a row outside its action button, opens
Hosts with that host's row in view and its menu button focused, without changing the session filter. This uses the
existing acknowledgement of warnings when Hosts opens; Hosts never opens by itself.

Update rows use the rail's available-update test and offer **Update** for that host. Required updates say that sessions
on the host stay unreachable until it is updated. The header reads **N hosts need attention** and offers **Update all**
only when more than one displayed row offers Update; it sends the ordinary authorized Update request for exactly those
displayed Update hosts, independently. The broader **Update all** inside Hosts still considers every remote host whose
individual Update action is available. These actions retain their existing confirmations and refusals.

While **Update remote hosts automatically** is off and at least one displayed row offers Update, the attention header
also offers **Always update**. It requests the same displayed hosts as Update all, then turns on that preference using
Settings' optimistic save, newest-choice rollback and corrective read. Its tooltip explains that later updates install
without asking and that Settings → Hosts changes the choice. A failed save leaves the requested updates running and,
while the preference still reads off, shows an error below the heading explaining that later updates will still wait,
with **Try again** saving only the preference. The error survives opening and closing Hosts while attention remains, and
stops applying after a newer choice for that setting. It does not keep an otherwise empty attention list visible. A
successful save hides Always update and makes Settings show the enabled switch. Button and error visibility changes
retire session menus anchored below the attention list, just as changes in its rows do; ordinary progress ticks still do
not.

A close control dismisses only the optional Update rows; required updates, problems and working hosts cannot be
dismissed. The dismissal leaves the rail badges and hides those reminders until a host outside the dismissed update set
gains an update or the helm build changes. It lives only in interface memory, so a restart or a reset after a lost
connection shows optional reminders again. If no undismissed host needs attention, the list disappears. This list
replaces the required-update banner and optional-update card.

Lifecycle operations against an unreachable host are refused with a clear error; nothing queues for later delivery in
v1. Environment Suspend/Resume and Delete use their provider paths under Sandbox sessions and Container sessions. A
session on an unreachable host shows one calm accent card naming the cause, with the helm's remedy and smaller
diagnostics beneath it. Its metadata — title, directory, last-known status — remains available, with last-known status
and age in exactly one band. A terminal already open keeps its last screen and tab strip dimmed and inert beneath the
card, clearly no longer live. A session opened while unreachable has no terminal or tab strip. When the host returns,
retained terminals retry promptly and automatically without displacing another client. A rebooted session still shows
its interrupted card. A dropped terminal stream between the window and the helm uses the same card with
`Session unavailable — reconnecting` and a neutral `Reconnect now` button; recoverable drops show no red detach banner.
Final takeover and stall decisions retain their explicit banners. Changes made from any client — creates, renames,
stops, deletes, status transitions — appear in all other connected clients automatically; the agent-spawn behavior below
is one instance of this general rule, not a special case.

### Status

Each client can play distinct sounds when a session starts waiting, when a new agent approval request arrives, and when
an agent goes from running to idle. Claude and Codex report Waiting; Muse reports it for workspace trust only, with
in-conversation questions and approvals still unverified. Such sessions can show Idle while Muse waits for an answer.
Settings has sounds controls under **Notifications**, with one switch per event, remembered per device: waiting and
approval default on, turn finished off. The open session stays quiet while its window is active; the state loaded when
the window starts or resets makes no sound. Status sounds follow the sidebar's current host selector and filters;
changing that view silently records its existing statuses. New approval requests sound regardless of those filters,
because their cards are always shown. Each observation plays only its most urgent enabled, non-quiet event: approval,
waiting, then turn finished. Every open client makes its own sounds.

Each session shows one of: **Running** (agent actively working), **Waiting** (a detected pending question or approval
directed at the user), **Idle** (agent alive and at rest, no pending ask), **Exited** (process ended), **Interrupted**
(the host rebooted, or an update ended the old terminal the agent ran in, while the session was last known live — an
explicit lost-track state; see Durability and Terminal experience), **Error** (the agent process could not be started at
all). Exited sessions show their exit code when known; an exit that happened while the supervisor was down shows the
code only when the surviving terminal genuinely retains it, and an explicit unknown otherwise — never a guess (see
Durability). A user-initiated stop yields exited with an annotation — "stopped" is not a distinct status. If the stop
was recorded before launch classification and the agent never consumed its launch spec, the annotation reads "stopped
before the agent finished starting"; otherwise it reads "stopped by user". While a stop is pending, a dead terminal
alone never becomes a launch error: direct evidence from the launch shim can still report a real failure until an
annotated exit commits. Host unreachability is per-host connection state, not a session status.

After a supervisor restart, a live terminal keeps its last cached status while the supervisor gathers new screen
evidence. The first new screen alone does not turn an idle or waiting session into running. A witnessed exit, launch
error, changed screen, recognized wait, or enough unchanged samples replaces the old answer as soon as observed. A
session with no cached status remains unclassified during that gap. An unclassified session restarts without asking,
like any agent that does not read working (see Lifecycle operations), even though its terminal may be live.

An interrupted session stays interrupted until the user acts: opening it and declining resume leaves it interrupted;
restart or delete are the ways out.

How a status is DRAWN depends on how much it has to say. The three live states are a color-coded dot beside the
session's title — running pulses, waiting and idle do not — with the status word itself always present as text for
screen readers and anything else that reads rather than looks, never replaced by the color. Outside compact mode, ended
states keep their complete wording visible, including exit code, stop annotation, and launch failure details. Compact
mode uses distinct stopped, exited, interrupted, and error icons; those details remain in accessible text and tooltips
without expanding the row. Ended icons do not have the live dot's mark-read action. The pulse is a claim about the
present, so it stands down wherever the status is a last-known report rather than a live one: a session on an
unreachable host shows a grey, still dot whatever its status says. Looping indicators such as the running pulse change
in a few discrete steps per second rather than continuously, and all of them stand still while the Farhelm window is not
focused or not visible, to save CPU and battery.

The dot's colour is one of four, not three: running pulses green; waiting is red, since it is the one live status that
is a request directed at a human and belongs with the other attention colours rather than beside "nothing is wrong";
idle is grey when its last output has been seen; idle is blue when it has not. On a solid selected row, inverse marks
keep the same shapes instead of their ordinary colors: running circle, waiting triangle, unseen idle diamond and seen
idle ring. Running still pulses, while a last-known report remains still. Seen state is one stored fact per session — an
activity stamp recorded the last time some client looked at it, or nothing at all if no client ever has — kept by the
helm and shared by every client the same way the list order and the last-selected session are (see Session list): no
client keeps its own copy, and opening the same session from a second client sees the same colour the first one does. A
session reads UNSEEN whenever its most recent activity is newer than that recorded stamp, or nothing is recorded at all;
otherwise it reads SEEN.

Pins, seen stamps and notification read/cleared marks belong to the session on its particular host entry. They survive
editing, retargeting or adopting that entry, but removing it and adding the host again starts those values fresh. On
upgrading to this storage, state the helm cannot attribute to one host starts fresh too.

The stamp is set automatically in two moments — opening a session records its current activity as seen, and that
session's activity advancing while it stays open re-records the new value — with whether the window has focus ignored: a
session open in a window behind other apps is still recorded as seen the same way one in a focused window is. It can
also be set or cleared by hand: a "Mark unread"/"Mark read" toggle, reachable from the session's row, sets or clears the
stamp directly. A manual mark unread on the session currently open STICKS — the automatic rule does not immediately
re-record it as seen — until the user leaves and returns to it, or new output arrives. A failed automatic mark is logged
but silent, the same best-effort exception the list-order/last-selection preference gets (see Errors and diagnostics); a
failed manual toggle surfaces to the row like any other operation.

Beside the status, a session shows how long ago it was last active as a short relative age (`2m`, `3h`), with the full
timestamp available on the row. Last active means the last time the agent was seen working, or the moment it started
waiting on the user: a question left unanswered for three hours shows three hours, and what an agent redraws on its own
while idle (a hint, a summary, a usage counter) does not count. The same time decides unseen, so such a redraw does not
turn a seen session unseen either. The age is not list position: the recently-active order groups by reported status
first and compares burst starts inside each group, so a row can sit above another whose age is newer. The age is a
difference between two machines' clocks and is only as good as they are, so it is never the only place the underlying
time is recorded. It is also independent of the status beside it: a session nothing has classified yet shows no status
and still shows its age.

Two cases have no age to show, and both show nothing rather than a guess. A helm predating the last-activity field sends
no stamp, and the session's creation time stands in as the displayed age. A session with neither stamp gets no age at
all, never one counted from 1970.

Running/waiting/idle discrimination for raw TUIs is inherently heuristic, and the waiting/idle boundary especially so.
The bar: best-effort observation of the agent's screen, through one screen reader per agent type. The generic reader,
used by every agent without a dedicated one, knows nothing about the agent: a screen that keeps changing is running, one
that stopped changing is idle, and it never reports waiting. Claude Code, Codex and Muse have dedicated readers that
recognize what those agents draw — their busy indicators, their input prompt at rest, and verified question dialogs
(workspace trust only for Muse) — and answer from that, so an idle agent's own redraws read idle. A screen they
recognize as carrying no state (a menu the user opened) leaves the previous status in place, and a screen they do not
recognize at all falls back to the generic reader. Their rules are held to real screens captured from the vendors'
current releases; see `docs/agent-screen-fixtures.md`. A reader never creates lifecycle state. Wrong status must be
cosmetic only — status detection must never gate or delay the user's interaction with the terminal. Agent-requested
prompt delivery is a narrow exception: the owning supervisor refuses a target reading waiting or unknown rather than
risk answering a dialog (see Agent-spawned sessions). A mistaken reading may therefore refuse that request or let it
reach an unrecognized dialog; it never gates the user's own typing. Status also decides whether Restart and Restart with
ask before stopping a live agent (see Lifecycle operations), where a wrong reading costs at most a skipped or an extra
confirmation. A recognized dialog also defers the missing-conversation warning's clock until an Enter outside that
dialog, as described below; this does not delay delivering input. Farhelm-supplied integration must not make vendor
configuration a condition of launching an agent. Grok is the explicit opt-in exception for conversation capture: users
install its three documented hook entries themselves, while an unconfigured Grok still launches normally and cannot be
restarted. OMP and Grok both use generic activity only. Their approval prompts show the generic running/idle
classification, never waiting — a settled scope decision, not a reader waiting to be written.

**Ntfy alerts** optionally reach the user away from the screen. Once enabled and configured in settings, the helm sends
one alert when a session becomes waiting and remains so for 10 seconds, once per waiting spell. Leaving and re-entering
waiting starts another spell. A host's first successful observation after helm start or reconnect is a quiet baseline;
new sessions first observed waiting afterwards count. Unknown readings add no evidence and do not send. Detection
remains heuristic, with the same agent limits as the waiting sound. There are no alerts for Farhelm approval cards,
finished turns or session-tracking notifications. Window focus does not suppress ntfy alerts.

The alert contains the session's title and the host's display name, with control and other invisible or reordering
characters removed and each capped at 200 characters: "<title> on <host> is waiting for you". A title generated from a
directory is still sent as the title; no screen contents, prompts, command lines or other paths are added. Its ntfy
title is Farhelm and its urgency is the saved priority. At most five background sends are attempted per minute across
all hosts. Excess alerts and full-queue alerts are dropped; the next permitted alert can mention the number dropped by
the rate cap, but there is no later summary alert. Failed sends are logged without the topic, token or title and
dropped, never retried. Sending does not hold up host refreshes or terminal interaction. **Send test** bypasses these
background limits and waits for one send under the short timeout. Anyone who knows an ntfy.sh topic can read it; keep
the topic private. A standalone helm sends without a window open. The desktop app's helm stops when its window closes,
so desktop-only alerts need that window open, though it can be in the background.

When Farhelm loses track of a session's agent conversation in a way that will matter later, most often because Restart
will then be unable to resume it, it says so on that session as a notification rather than only in a log. In this
version every notification but the old-terminal ones (below) is a session-tracking problem: a launch whose conversation
hook should have reported which conversation the agent is in and has not, a minute after the first Enter the user sent
it (an Enter inside a paste, or Shift+Enter, inserts a newline and does not count, and an Enter while the latest screen
reading shows a recognized dialog does not count either; an outdated dialog reading can defer the clock to the next
Enter; with no reading yet, the ordinary Enter rule applies), for an agent that reports by its first prompt (Pi and OMP
report only after their first reply, which can take as long as the first turn, so their silence proves nothing and is
not reported); a launch Farhelm could not add its conversation hook to, unless the user turned hooks off for that agent
themselves; a Codex or Grok conversation record that Restart finds missing, which withdraws the resume offer; and an OMP
launch whose conversation reporter does not match the running Farhelm, the one refused conversation report that
notifies; and a Claude, Codex or Muse launch whose first conversation report came from a nested process of that agent,
corrected when a recorded ancestor reports. That correction says which conversation a Restart before now would have
resumed and that Farhelm now follows the conversation of the agent process that started it, without claiming no further
ancestor can correct it. Every other refused report (from a subagent, from an agent process the session's agent started
itself, overtaken by a relaunch or delete, or turned away by a passing error) is the system working and never notifies.
A launch that still carries a conversation Farhelm captured earlier is never told Restart cannot resume it, because
Restart can. After a supervisor restart, a launch that received the hook starts its silent-hook clock at the next
qualifying Enter; launches stored before this behavior was introduced stay unchecked. Each notification says what
happened and what the user can do about it, or, when nothing can be done, what they lose; it never points at a log. The
silent-conversation-hook warning names the agent and when it normally reports its conversation, using the same timing as
Restart's explanation. It says Restart cannot resume until that report arrives, and asks users of custom commands to
check that they pass on `{farhelm_args}`; otherwise it invites feedback from the help (?) menu. There is at most one
notification of each such problem per launch of the session's agent. The warning that the agent never reported its
conversation and the warning that Restart stopped offering to resume are marked resolved when Restart can resume that
launch's conversation again, including after a supervisor restart. The other session-tracking kinds and notifications
from earlier launches stay as they are. If a resolved problem returns in the same launch, its existing notification
reopens at the top as new and unread, even if the user had cleared it; repeated checks while it remains unresolved do
not repeat it.

The other notifications are about old terminals (see Terminal experience). Earlier releases gave a session in which
something still ran in an old terminal the old-terminal warning: it asked for the session to be restarted, replaced,
stopped or deleted as soon as possible, because an upcoming release would forcibly terminate its old terminals. That has
happened (see Terminal experience), so no warning is recorded any more, and every supervisor start marks any unresolved
one resolved. Each session whose agent was still launching, running or being stopped in an old terminal when a
supervisor's start ended it gets one notice instead, at that start, saying that its old terminal was ended by the
update, so its agent is no longer running and its last screen is gone, and that Restart starts the agent again in a new
terminal (resuming its conversation), or Replace when Restart is unavailable; it ends with a link to the docs page on
new terminals. Nothing resolves the notice: it stays unresolved, and unread until the user reads it, across later
supervisor starts, until it is cleared. A session whose agent had already ended, or that lost only old tabs, gets none.

A session with notifications shows a bell on its sidebar row (see Session list); a row without notifications shows none.
The bell is grey when everything in it has been read and unmistakable when something is unread, and its accessible name
says how many are unread. Clicking it opens the session's notifications in a list in the style of the row's actions
menu, never clipped by the sidebar, and does not open the session. The list is newest first, each entry saying how long
ago it happened, with the full time available on the entry the way the row's activity age has it, and the entries new
since the list was last opened set apart. An entry's text is shown as the supervisor wrote it, except that a link to a
page of Farhelm's docs (an address starting `https://farhelm.io/docs/`) is clickable and opens the way the app's other
external links do; any other address stays plain text, since a host's supervisor must not be able to put a link to
anywhere it likes in front of the user. Resolved entries remain visible, greyed and marked "Resolved", and count as read
even if the user has never opened them; they never make the bell loud or appear as new. Closing the list, by clicking
away, pressing Escape, or clicking the bell again, marks everything it showed as read; there is no mark unread. A clear
button removes them all, and the bell disappears until a new one arrives. Notifications persist across supervisor and
helm restarts and window resets; each session keeps its 10 most recent, with no time-based expiry, and deleting a
session deletes them. Read and cleared state is kept by the helm and shared by every client, the same way seen state is.
Other than ntfy waiting alerts, desktop or operating-system notifications are not part of v1.

Notifications about a host or the whole app have a separate bell at the top of the navigation rail's footer, above
Hosts. It uses the same 16px size and quiet colors as its neighboring controls. Unread items make it filled red with an
unread count; its accessible name reports that count too. Clicking opens newest-first history beside the bell. Escape,
clicking outside, clicking the bell again or leaving the panel with keyboard focus closes it and marks only the items it
showed as read. The bell then goes quiet; a failed best-effort mark may leave it loud until another close. Resolved
items are quiet even if never opened, stay greyed in the panel, and can be removed with **Clear resolved**. Read and
cleared state is kept by the helm across resets and restarts. Removing a host removes its notices; an unreachable host
keeps its last known notices.

The initial kind reports trashed checkouts whose background cleanup failed. It names the host and checkout, shows how
much space was last measured when known and a short reason, and explains that Farhelm tries again when that host's
supervisor restarts. **Retry now** asks for one attempt through the existing cleanup checks; it is unavailable without a
current connected host listing showing that same checkout stuck, or while another guarded operation prevents it. **Open
Trash** opens the existing dialog, including while host management is visible or sessions are marked, and closing that
dialog returns focus to the visible rail bell. A notice keeps its identity and read state while the failure recurs; an
unresolved problem never re-alerts merely because Retry or a supervisor restart reports it again. A folder being removed
stays unresolved. Only a successful listing from a host capable of reporting accepted cleanup can resolve a checkout it
no longer contains; failed reads, older hosts and cleanup receipts cannot. Host setup/update attention, release
announcements and session bells keep their existing separate presentation. No toast or new notification kind ships with
this behavior.

An older helm or UI shows resolved entries as ordinary unresolved notifications. A newer helm connected to an older
supervisor receives no resolutions; the rest of the notification behavior stays available without a protocol change.

## Terminal experience

The real TUI is the primary and, in v1, the only interaction surface. Typing goes straight to the agent's terminal;
whatever the agent renders is what you see. There is no composer, no message abstraction, no send button in v1.

- Full fidelity: colors, cursor movement, alternate screens, resize. If it works over plain SSH it must work here. The
  default palette, foreground, and background are Ghostty's defaults, and launched agents see `COLORTERM=truecolor`.
- Settings can color the thin frame around every session's terminal with that session's host identity color. This is off
  by default and shared by every device. When on, a host with the default color uses the ordinary foreground color;
  every other host uses its chosen icon color at full strength. A host color change updates open session frames through
  the existing hosts feed, and a switch change applies immediately, from whichever surface made it. The terminal
  background, text palette and size do not change.
- The quick-switcher's Cmd+K and template list's Cmd+J on macOS, or Ctrl+Shift+K and Ctrl+Shift+J elsewhere, never reach
  the terminal's program when Farhelm accepts them; plain Ctrl+K off macOS keeps the terminal program's own behavior.
- Shift+Enter (the exact chord — no other modifier held, not mid-IME-composition) is sent as ESC CR in a SINGLE write,
  in every terminal tab alike, agent and shell. Single-write delivery is part of the promise, not an implementation
  detail: a lone ESC arriving in its own read is indistinguishable from the Escape key to line editors that disambiguate
  by read boundary or a short timeout (Codex's input stack; zsh with a small KEYTIMEOUT), which turns the chord into
  Escape-plus-submit. Delivered whole, the sequence is the newline binding Claude Code and Codex honor in place of
  submit (verified against both, 2026-08-19). Other programs receive the same bytes and interpret them per their own
  line editing — stock emacs-mode zsh inserts a newline, bash's default quietly ignores the pair — the same outcomes
  those shells give under a reference terminal that encodes the chord identically (Ghostty).
- Terminal text size is adjustable: Cmd+Shift with + or − on macOS, Ctrl+Shift elsewhere, or the magnifier − / + buttons
  at the start of the action group on the right of the session's top row, step every open terminal's text size together,
  hidden tabs included, within a fixed range, and each terminal is resized so the program inside sees its new rows and
  columns. The buttons share one border with a fixed-width, display-only readout of the current size in points between
  them. The readout follows button and shortcut changes, resets and rebuilt session controls without adding a tab stop.
  While a terminal is open the shortcut works wherever focus is and never reaches the terminal's program. Cmd/Ctrl
  with = or − and no Shift are not app shortcuts and reach a focused terminal like any other key: there is no interface
  zoom (decided 2026-11-13). On Linux this takes over Ctrl+Shift+− (Ctrl+_, undo in readline and emacs) inside the
  terminal, accepted 2026-10-02. The keys are the ones at the US-layout positions of = and −. The size is remembered per
  device, deliberately unlike the session list's preference: it is about the screen in front of the user, not a choice
  every client should share. Only the terminal text changes; the rest of the UI keeps its size.
- Scrollback is whatever the host-side terminal naturally retains, and it survives client disconnects: detach, reconnect
  a day later, and the buffer is still there. Farhelm avoids storing terminal output except where it directly helps the
  user, such as error reporting. When an agent never starts, its durable error may include the failed launch's last five
  nonblank lines, bounded to 500 UTF-8 bytes after unsafe characters are escaped. Farhelm omits the excerpt when it
  cannot safely attribute the text to that launch. This is an error detail, not a separate conversation history: a host
  reboot loses the terminal's contents, and recovering the conversation is the agent's job (resume). A stopped or exited
  session's terminal stays viewable while its host is up, since the terminal outlives the process. Viewable means what
  the terminal itself holds. A full-screen program that leaves its full-screen display as it exits leaves nothing of its
  last frame; one that does not (because it was killed, say) leaves that frame showing, and the terminal keeps it across
  supervisor restarts like the rest of its contents. No picture of a screen is ever taken to be painted into another
  terminal (see Restart under [Lifecycle operations](#lifecycle-operations)).
- Every terminal runs in a **terminal host**: a small process of its own on the host that keeps the terminal and its
  program running across supervisor restarts and updates. Programs in a terminal are told it is Farhelm's own:
  `TERM_PROGRAM=farhelm` with Farhelm's version as `TERM_PROGRAM_VERSION` (the version the terminal also reports when a
  program asks it), with `TERM=xterm-256color` and `COLORTERM=truecolor`, all four set whatever the supervisor's own
  environment holds, and nothing that names tmux: no `TMUX` or `TMUX_PANE`. Container sessions use this same terminal
  environment inside their container. A terminal keeps its scrollback to the floor below, and keeps it across supervisor
  restarts and updates as well as client disconnects: its terminal host holds the program's output while no supervisor
  reads it (up to a bound, next), and the next supervisor rebuilds the terminal from it. That output is held in memory
  only, never on disk. A viewer that attaches or reconnects is sent the current screen and then the whole scrollback
  once, oldest first; from then on it is sent only what changes: the screen as it is now, and the lines that entered the
  scrollback, appended. The scrollback stays while a full-screen program shows and is there when the program exits. When
  the scrollback changes in a way an append cannot show (a resize rewraps it, the program erases it or redefines one of
  the terminal's palette colors, or lines the viewer was still owed fell off the scrollback's far end), the viewer is
  sent the whole scrollback again once the change has settled, so a window dragged to a new size sends it once, or for a
  long drag once during it and once when it stops, not at every step.
- A terminal host keeps a bounded amount of the program's output beyond the supervisor's latest checkpoint of the
  terminal: 32 MiB by default, a few minutes of an agent redrawing as fast as it can, hours of a shell. When more than
  that arrives while no supervisor reads it (a supervisor that crashed and is not started again soon; the Mac desktop
  app's local supervisor returns only when the app or binary is started), the oldest of it is dropped, which leaves a
  **gap**; the program runs on unhindered. A gap can also happen while a supervisor runs, rarely: a single escape
  sequence longer than the bound, or output the supervisor cannot checkpoint as fast as it arrives, such as full
  scrollback colored cell by cell at high sustained rates. At about 11 MiB/s on the measured hardware, encoding one such
  snapshot alone can fill the remaining journal space; catching up and machine load can make a gap possible at lower
  rates. The dropped output is everything since the latest checkpoint that no longer fits, so it can include output a
  supervisor had already shown before it crashed. After a gap the terminal carries on from its state before the lost
  output (after a supervisor restart, its latest checkpoint) with the output kept after it, the supervisor asks the
  program to redraw by making its terminal one row smaller and back, and the terminal shows "Some earlier output could
  not be recovered" above its scrollback until the supervisor next restarts. What a gap cannot bring back: the lines the
  lost output put into the scrollback; the screen, until the program redraws it (agents and other full-screen programs
  redraw when their terminal is resized; a shell at its prompt does not, so its screen can show older contents with
  newer output drawn over them); any mode the lost output changed, such as the alternate screen, mouse reporting,
  bracketed paste or application cursor keys, and the same for the other terminal settings it made, such as the title,
  redefined colors, line-drawing characters, the scroll region and the cursor's style. A program sets these again only
  when it chooses to, and many set their modes only when they start, so such a setting can stay wrong until the program
  is restarted; meanwhile keys, pastes and mouse input can reach the program encoded for the wrong mode. A multi-line
  paste into a shell may then run line by line instead of arriving as one block. A query in the lost output that no
  supervisor had answered before the bytes were lost is never answered (the program waits out its own timeout), and a
  clipboard write there is not carried out during the recovery; either may already have happened, if a supervisor saw
  that output before it crashed. A gap never restarts a program or throws away a working terminal. Separately, a
  checkpoint too large for the terminal host (16 MiB, far more than a full scrollback of ordinary output takes) can
  leave out old title-stack entries, the oldest scrollback lines and, if that is not enough, the screen's contents. The
  running terminal is not affected, but a recovery from that checkpoint rebuilds the terminal without them: it shows the
  same notice when scrollback was left out, which then stays until the program clears its scrollback or the scrollback
  has turned over, and asks the program to redraw when the screen was left out.
- Opening a session attaches to it — and opening a CLIENT counts as opening a session. A freshly loaded client and a
  client whose open session disappears use the same choice: the first currently listed session in the helm's shared
  visit history, excluding the session just removed, else the newest-created listed session. While history is empty, the
  retained last selection counts as one visit under those same rules. A filter or the listing's cap narrows candidates;
  exited sessions and sessions on disconnected hosts still count. The helm keeps the 32 most recent distinct visits
  across resets and restarts. Row clicks, quick switching, creation, replacement and automatic selection record visits.
  Immediately before an automatic pick the client rereads shared preferences, falling back to its current in-memory copy
  on failure, and remembers the result. Removing an unselected session leaves the pane alone; removing the last session
  leaves it empty. Existing removal detection still needs a complete, unfiltered fleet reply to infer an external
  deletion or host removal; filtering or a capped reply does not unselect an already-open session. Replace selects its
  replacement if its source is still selected when it finishes, and closing a terminal tab is not a session removal.
  Launching the app is itself the deliberate act the attach semantics below key off. Opening a second client therefore
  takes over its chosen session exactly as clicking that session there would. The attached client owns input and
  terminal dimensions: the PTY resizes to that client, and the last size sticks when nothing is attached. Reconnecting
  replays the terminal so the session looks as it would have had the client stayed attached, modulo redraws caused by
  dimension changes. The floor: the host-side terminal retains, and replay covers, at least the current screen plus
  10,000 lines of scrollback. The sidebar visibly marks the selected session's row whenever that session is listed, so
  which session the main pane is interacting with is readable at a glance rather than only from the bottom bar's session
  name. A filter that excludes the selected session leaves no row to mark — the bottom bar's session name remains the
  identifier in that state, and the main pane deliberately stays put (filtering the list is not deselecting).
- The typical session view has one top row, with terminal tabs and the desktop tunnel pill on the left and text size
  (zoom out, size in points, zoom in), Stop, Restart ▾, Replace ▾, Clone and Delete on the right, followed by the
  terminal and a slim details bar below it. Restart and Replace are split buttons: the main part performs the ordinary
  action, including its existing confirmation; the caret opens a menu offering that action and Restart with… or Replace
  with…. The icon-and-label controls keep their compact height. Text size, Stop, both parts of each split button, Clone,
  Delete and the tunnel pill remain fully visible and keyboard reachable from a 650px main pane; only the tab strip
  takes the remaining width and scrolls horizontally when needed. Narrower panes may clip the row, which never wraps.
  The bottom bar is about one tab-strip row high and shows, in order, status, session name, host, age, the
  managed-checkout branch mark when present, directory, and command line. The host's icon follows the session list's
  rules; its name is the full display name, prominent beside the session name. A missing host name omits the host rather
  than inventing one. Directory display still abbreviates the home directory to `~`. Keyboard order follows the top row,
  terminal, then bottom bar. The terminal loses no height in the steady state. Conditional notices stay between the top
  row and terminal; a stale session's status appears only in its stale notice, not again in the bottom bar. Restart and
  its menu's matching entries are greyed out when the session cannot resume its conversation (see Lifecycle operations);
  their tooltips and accessible descriptions explain the specific reason, such as an agent type without conversation
  reporting, a command launch that declared no agent type or did not opt into Resume, or a conversation not captured
  yet. For an agent that reports conversations, a running session's explanation says, naming the agent, when Restart
  normally becomes available for it (Codex, for instance, once the first prompt is submitted), and an ended session's
  says its conversation was never captured; both invite the user to send feedback if that is not what happened, since
  Farhelm cannot tell a report still to come from one that never will. The supervisor's refusal of such a restart and
  the agent instructions state the same timing, without the invitation. Directory and command line are muted
  click-to-copy buttons that take the width their values need and ellipsize before the session name or host when the bar
  runs out of room; a click confirms locally for about 1.5 seconds. A hidden-characters copy warning opens upward from
  the bar and stays within the window. Clipboard writes use the system clipboard and fail silently. Replace has its own
  anchored danger confirmation and neutral cancellation. Delete is styled as the danger action and asks first in the
  same anchored way, even for a session that has ended, with the same consequence text the row's delete prompt shows.
  Restart's confirmation is likewise anchored under its top-row button; Escape cancels these questions and returns focus
  to the terminal. The session-confirmation setting skips the top row's Stop, Delete, Replace and Restart questions, the
  interrupted-card Replace question, and tab Close, using the screen's consent and existing guards. Split menus follow
  the app's shared keyboard rules: mouse or pen movement focuses the hovered choice without scrolling, arrows and
  Home/End move between choices, and only the focused enabled choice has the highlight fill. Leaving the menu keeps that
  focus; Escape or Tab closes the menu and returns focus to its caret. Cancelling Restart with returns to that
  persistent caret. An unavailable plain Restart does not hide an independently available Restart with; the menu remains
  available to inspect reasons.
- Old terminals: releases from before terminal hosts ran each terminal (the agent's and each tab's) in the host's
  private tmux server instead, and later ones kept those old terminals running beside terminal hosts, with a loud band
  on such a session and the old-terminal warning (see Status) asking the user to move them. A supervisor that finds that
  tmux server when it starts ends it, and with it everything that still ran in an old terminal, without needing a tmux
  program. A session whose agent was still launching, running or being stopped in one becomes interrupted, as after a
  reboot, and gets the notice Status describes; one whose agent had already ended keeps its outcome, but its terminal's
  last screen, which lived in the server, is gone; old tabs are closed. Programs the old agents and tabs left running
  are stopped, for an agent by the session's own record (its launch scope and process markers, as a Stop of an agent
  that already exited finds them) and for a tab wherever its id can be found; what cannot be attributed to an old tab is
  left for the session's Delete, which stops everything of the session. A session whose agent or tabs already run in
  terminal hosts keeps them. A failure to end the server or to stop what ran there is logged and never stops the
  supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start
  a second agent beside one still running. Restart, Clone and Replace start such a session in a new, empty terminal.
- One attached client per session, enforced by the supervisor: attaching from a second client visibly detaches the
  first, which keeps a non-live snapshot and an explicit take-control action. No shared-input mirroring in v1.
- A viewer that is slow is served slowly, down to a floor below which it is detached as stalled. No viewer can stop a
  session's work outright, and no viewer can slow one it is not attached to; no viewer slows the agent at all. The
  terminal takes in everything the program writes whatever its viewers do, and a viewer that falls behind is sent the
  screen as it is when it can take more, skipping the screens in between, while every line that entered the scrollback
  meanwhile is still sent, appended. Output to a viewer pauses when a backlog builds up for it at the helm, wherever it
  is slow (sooner when the backlog is many small messages), and one that has not caught up within the stall timeout,
  about a minute counted from the moment output to the viewer paused, is detached as stalled. In practice a paused
  viewer must take in a full scrollback at about 50 KB/s locally, or about 110 KB/s over ssh (the figure moves with how
  much the link holds), and live output that falls behind slowly at about 10 KB/s (SPEC_impl.md, "Helm ↔ supervisor
  transport: system ssh + stdio protocol", has the arithmetic).
- A viewer that stops consuming output entirely (a hung window, a machine asleep past its connection's lifetime) is
  detached with a visible stall reason rather than honored forever, as the floors above describe — the same surface as a
  takeover detach, with reattach behaving exactly as any reconnect does. Flow control never drops terminal output:
  whatever bound it degrades to is the same replay floor above, never a silent gap. Skipped screens are no exception,
  since the screen a viewer is sent is the program's current one and the scrollback arrives whole, and no viewer can
  cause a gap (above): those come only from the supervisor falling behind the program. One exception, accepted
  2026-11-04, is extreme cumulative applied resizes on a quiet terminal: they can exhaust its recovery journal without
  triggering a save, so a later crash or an orderly stop that skips its save can lose terminal history (SPEC_impl.md,
  "Across supervisor restarts"). This does not relax the slow-viewer or flow-control guarantees.
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
  even one identical to what is already on the clipboard. When a plain drag goes to a program under mouse reporting and
  the program copies nothing (no OSC 52 write follows), the terminal shows a notice that the program handles selection
  itself: use its own copy command, or hold the forcing modifier while dragging. It appears on every such drag, with its
  bottom edge three terminal rows above the release pointer when it fits inside the terminal's pane and the window.
  Otherwise its top edge sits three rows below; if neither fits, it falls back to six pixels above, clamped inside the
  window. It stays centred horizontally on the pointer, clamped inside the window. It stays for 30 seconds, or until its
  × is clicked or the next mouse press in that terminal. Dismissal preserves terminal focus and sends no input to the
  program; the rest of the notice is pointer-transparent. For a Codex session's agent terminal the notice names Codex's
  own copy key instead. It is guidance about the program, not a report of a clipboard failure. Any completed program
  clipboard write, including a primary-selection write, suppresses it during the grace window. Clipboard operations are
  explicitly best-effort and silent on failure — the operating system's clipboard behavior is outside this system's
  control — a deliberate, named exception to the Errors and diagnostics section's surface-every-error rule below, not a
  lapse in it. Farhelm assumes the host's system clipboard works. A broken, hung or slow clipboard is not something
  Farhelm adds complexity to support well: copies may be dropped while it is in that state, but nothing else in Farhelm
  may stall because of it.
- Dragging across a terminal link never opens its target, downloads a file or opens a saved file, including while a
  program has mouse reporting on. Small pointer jitter still counts as a click.
- Links in terminal output open their http(s) target on click, with no confirmation or prompt of any kind, unless the
  **Terminal** settings section's link choice is set to pass the click on only. By default, a primary-button press on a
  web link Farhelm would open is withheld from a program with mouse reporting active, together with its release and any
  drag starting there, so programs that open links themselves cannot open a second browser tab from that same click. The
  forcing modifier still selects locally. A withheld press dismisses the drag-copy notice and never raises it. File
  links, other buttons, the wheel and all other mouse gestures keep their existing behavior. The alternative settings
  either let Farhelm open the link and also send the click to the program (which may open it again), or leave the click
  entirely to the program; without mouse reporting, the latter opens nothing. Program-initiated URL forwarding and its
  host permissions are independent and unchanged. A hyperlink a program emits (OSC 8) can underline text that differs
  from where it goes, so hovering it shows the exact target first, with its host emphasized. When the underlined text is
  itself a web address (it has a scheme, starts with `www.`, or is a dotted host ending in a name of two or more letters
  followed by `/`) and names a different place, ignoring only a trailing slash, the display becomes a prominent warning
  that shows the text beside the real target. Link text that is not a web address, such as a file name or "click here",
  keeps the ordinary display. Consecutive rows of the same hyperlink are compared together, including links an agent
  continues after indentation with cursor movement. Restarting a hyperlink without an explicit id right where its
  previous piece ended, with the same target, continues that hyperlink, across soft wraps too. An honest wrapped web
  address keeps the ordinary display, and a lookalike split across rows of one hyperlink is judged on its whole text.
  Repeated complete addresses that each independently match the target keep the ordinary display. If an adjacent
  continuation cannot be joined unambiguously within the bounds, a piece that looks like a web address warns rather than
  counting as a match. The warning is judged when the hover starts and need not follow later redraws of other rows while
  the pointer rests; the destination display stays authoritative. The warning is an aid, not a guarantee: unlinked text
  around the hyperlink is outside the comparison, a program that chooses its own line breaks can still split a lookalike
  into pieces that are not joined (separate hyperlinks, say) and that do not look like web addresses on their own, and
  the ordinary display, which always names the real host, is what remains. The hover display is the whole safeguard: it
  counts as disclosing the target only while fully visible, inside the window and not covered by another Farhelm surface
  such as an approval card or tunnel-error notice. A click whose hover is not fully visible is refused with an
  explanation; this also applies to Cmd-click tunnel opens. Plain URLs printed as text need no such display, because
  what is shown is what opens, apart from the remote-localhost tunnel hint below.

- In the Mac app, a loopback link in a remote session's terminal offers Cmd-click to tunnel and open it in the browser.
  Its hover names the host and port, explains that a plain click still opens on the Mac, and preserves any OSC hyperlink
  mismatch warning. The offer applies to `localhost` and its subdomains, `127.0.0.0/8`, `::1`, `0.0.0.0` and `::`, not
  LAN addresses or other single-label names. A tunnel listens on the Mac's `127.0.0.1` at a port the operating system
  picks, never the remote port number, so a remote service cannot take an address a Mac program or a standalone helm
  uses. The opened URL uses that IPv4 literal and the actual local port, preserving the rest of the URL. Bytes travel
  over Farhelm's existing helm-to-supervisor connection to the supervisor's loopback; this is raw TCP, including
  WebSockets, with no HTTP rewriting. Named localhost and wildcard addresses try IPv4 then IPv6 loopback; an explicit
  loopback IP is used as given. Sessions on the local host have no offer, and a standalone helm refuses tunnel requests.
  Before the first tunnel opens, the user must accept the security warning in Security below. This choice is a shared
  helm preference, off until accepted; Settings shows it and can turn it off, closing existing tunnels. A later tunnel
  request then needs the warning again. Each tunnel is kept only in helm memory until the session is deleted, Farhelm
  quits, or the user closes it. Restarting or stopping the agent does not close it. Reopening a link for the same
  session and loopback target reuses its tunnel. A host disconnect fails new connections until the host returns; an
  existing TCP stream is never replayed. A distinctively colored pill beside the terminal tabs shows the local-port →
  host-port mapping for one listener, or a count for several. Both ports remain visible; a long host name ellipsizes,
  with the full mapping in its tooltip and accessible name. Opening the pill shows a list of mappings with a close
  button for each listener and Close all. Close all attempts the listeners shown at activation in order, continuing
  after failures and retaining failed listeners with their reasons. The list and close actions stay available while the
  host is disconnected. No pill appears when no listeners remain. Read and close failures appear in a dismissible notice
  below the top row without adding a permanent row, including when no pill is present; refreshing the list does not
  dismiss a close failure. Escape closes the list and returns focus to the pill; Tab follows its ordinary controls and
  leaving the list closes it without reclaiming focus. This feature does not forward login callbacks.

- File paths in terminal output can be downloaded from the session's host, including the local host. Absolute and `~/`
  paths, relative paths containing a slash or a file extension, and `file://` hyperlinks emitted by a program count;
  plain words, version numbers and plain scheme-bearing text do not. Detection does not try to support file names that
  contain whitespace. As a deliberate exception to conservative recognition, a path cut by a program's line break links
  to the joined path from either half when the head ends in an unclosed path-like token and the next row starts with its
  continuation (indentation is ignored), and that continuation would not fit in the head row's remaining columns after
  allowing up to four columns for a TUI's right margin. Padding after the head is ignored. Both the head's final token
  and the joined token must meet the ordinary path rules; a plain word cannot donate a head. This exception covers one
  isolated pair of rows, not chains of hard breaks; ordinary terminal wraps still join as before. Diagnostic line/column
  suffixes and trailing punctuation are excluded. Relative paths use the session's directory, not a shell's later
  directory. Hover immediately underlines the path and shows a checking spinner, then the host, full resolved path and
  size or refusal. Each hover checks afresh; a redraw beneath a pointer resting on the same path text and target at the
  same position is not a new hover. Leaving cancels it. Only a successful check over link text unchanged since that
  hover admits a click (OSC hyperlinks may show a pointer earlier). A partial redraw that changes the link text
  withdraws transfer and opening permission even if the terminal library retains its old target. A click cannot transfer
  or open a file while the full hover disclosure does not fit in the window or is covered by another Farhelm surface
  such as an approval card or tunnel-error notice. Only a fully visible hover counts as disclosing the target; otherwise
  the click is refused with a visible explanation to move the pointer or enlarge the window and hover again. While the
  pointer stays on a refused link, its explanation may cover the top of an approval card, including its heading and the
  requesting session and host. The card stays answerable from the keyboard meanwhile; the cover ends when the pointer
  leaves the link. File disclosures and transfer messages make presentation-unsafe characters visible, distinguish
  literal escape notation from escaped characters, and preserve repeated spaces. They do not guarantee that names
  differing only by leading or trailing spaces, other Unicode spaces, or lookalike letters render differently (confirmed
  2026-10-17): a trailing space only makes a file less openable, and in the usual case the program that could plant such
  a name can already write the file it imitates. Files must be readable regular files no larger than 100 MB, including
  while transferring; directories are refused. The desktop app saves into Downloads under a unique name and shows where
  it went, with no partial file after failure or interruption. Status appears at the top of that terminal: success fades
  after a few seconds, while a failure stays until the next click or download there. Plain click only downloads. In the
  desktop app, Cmd-click on macOS (Ctrl-click elsewhere) also opens the completed save with the default app when the
  saved file's final extension is in the helm's shared list, case-insensitively. The list defaults to .md, .txt, .json
  and the common image types .png, .jpg, .jpeg, .gif, .webp, .bmp and .avif, and the saved file must still have the same
  final extension as the path disclosed at hover. Other types are saved without opening; an opening failure leaves the
  save successful and says why it was not opened. The hover uses a file icon and bright name, the host and full
  directory on a second line, and decimal size with the action hints on a third. A left stripe and size color show under
  1 MB green, 1–10 MB blue stripe with neutral size, and 10–100 MB amber; refusals are red and checking has a neutral
  stripe. Only an eligible desktop file has the opening hint. Any file the session's account can read is in reach,
  within the existing shell authority; hover discloses the agent-provided target before an explicit click. Only the helm
  can ask the supervisor for file reads. OSC hyperlink schemes are explicitly limited to http, https and file; every
  other scheme is ignored.

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
path; the window never needs to reach any machine but the helm's.

Transfer must not block the terminal: you can keep typing while it runs, and the path is inserted at whatever cursor
position is current when the transfer completes. For a typical screenshot this is imperceptible.

Upload failures must be visible; an attachment must never disappear silently. Confirmed 2026-10-10: that covers
Farhelm's own failures and crashes. Whether an attachment survives a power loss or system crash right after its upload
completed is best effort.

## Keyboard focus

NOTE: This section is the rulebook for where keyboard focus (where typing goes) moves in the GUI. It takes precedence
over older focus wording elsewhere in this document. Parts of the current UI predate it; where they differ, this section
is the intended behavior and the UI is behind.

Confirmed 2026-11-10, after a walk through thirty concrete scenarios. Focus had been decided one bug at a time, mostly
in code that ran when something finished later, and each fix answered only the case in front of it. These rules answer
the question once, in product terms, so they hold whatever toolkit draws the GUI.

Here's the short version: the selected session's terminal is home, and only the user moves focus. Things that finish
later, and things the user did not start, wait to be reached. The one exception is a result the user sat and waited for.
Nothing closes what it did not open, and nothing changes what the user is typing.

### Who may move focus

Something the user started that finishes later (a launch's reply, a terminal that finishes loading, a question that
arrives after a reply) first asks whether the user has moved on since starting it. Moving on means any key press or any
click inside Farhelm. A modifier pressed alone, a repeat of the gesture that started it (auto-repeat, a double-click or
repeated Enter on the same submit), scrolling, pointer movement, and switching to another app and back do not count.

Once the user has moved on, the result moves no focus, opens no dialog and switches no session. It shows up where it
belongs (the session's row, a question card on its session) or as a notice the user can click. A launch that fails after
the user moved on shows a notice, and clicking it brings back the launcher with its draft and error.

If the user has not moved on, the result may take focus, and lands somewhere safe: a new session opens with its terminal
focused, and a question starts on its harmless answer (Cancel), never on the risky one. A launch that finishes while the
user is in another app has not been moved on from, so it is open when they come back.

Things the user did not start never take focus and never open over what the user is doing, even while a dialog is open.
That covers agent approvals, an agent's request to add a host, and anything else another window, an agent or a host
begins. They announce themselves and wait. The user reaches a waiting item with the pointer, or with a keyboard shortcut
that answers it or moves focus to it. While a terminal has focus, Tab, Escape, Enter and ordinary keys keep going to its
program. How each kind of item announces itself, and which shortcuts reach it, is not settled here.

### Where focus rests

The selected session's terminal is home. On the main screen, with no dialog, menu or popup list open, whenever nothing
holds focus it goes to the selected session's terminal. The cause does not matter: the user closed or deleted the
focused control, it went away on its own (the last notice was dismissed, a shell tab exited), or Farhelm selected the
next session after a Delete, including a Delete from another window or by an agent. The terminal never takes focus from
a control that holds it, such as the session filter or a header button. This accepts that a key can occasionally land in
a terminal unexpectedly, in exchange for never having to click back into it. A short delay before the terminal takes
focus back is a possible later mitigation, not a rule.

With no terminal on screen, focus rests on New when there are no sessions, and on a stopped session's card when the
selected session has stopped. Enter on that card does nothing, so typing that was already under way cannot restart
anything; Tab reaches its Restart and Replace.

Choosing a session, including the one already selected, focuses its terminal.

### Opening and closing

A confirmation the user just opened starts on the action they asked for: focus starts on its enabled confirming button,
so a new Enter goes ahead (from a row's menu, Delete then Enter deletes). If that button is disabled, focus starts on
Cancel instead. This covers session Stop, Delete, Replace, Restart and tab close, host setup, removal and uninstall, and
permanent trash deletion. A risky action that cannot be undone starts on Cancel instead, as the YOLO confirmation does.
The Restart with launcher also starts on Cancel. Repeated or composing Enter cannot answer the new question. Escape
cancels the header's Stop, Restart, Replace or Delete question and returns focus to the terminal. Questions that arrive
later always start on their harmless answer, as above.

Every modal dialog keeps Tab and Shift+Tab within its visible, enabled controls and keyboard scroll regions. The page
behind it cannot receive focus or clicks, while independent agent approval cards remain usable. Hidden quick-launch
preparation blocks page input without focusing its hidden fields; revealing the launcher supplies its normal focus. Once
a submitted launch is pending, its hidden launcher releases that isolation until it needs to show an outcome or
question.

A primary click on empty space keeps focus on the control in use in the session launcher, Restart with, the dialogs that
share the host and Settings installer, and the session row and header Stop, Restart, Replace and Delete questions; text
can still be selected. Rename, tab close, the interrupted card's Replace question and the host update confirmation do
not have this rule yet. If a dialog container itself has focus, Enter presses its enabled main action, including a
destructive confirmation. A dialog without one main action does nothing on container Enter. Held or composing Enter
cannot confirm.

Closing something the user opened returns focus to where it was before it opened, on every platform, including those
where clicking the button that opened it did not move focus to the button.

Inside a dialog or menu, focus stays inside. When the focused control goes away, focus moves to the closest remaining
control in the same dialog or menu, never out to the page or a terminal behind it.

Nothing closes a dialog, menu, popup list or confirmation that it did not open. Each goes away when the user closes it,
when whatever opened it closes it, or when the thing it is about is gone (a deleted session's open menu, say). A bulk
Stop that finishes, for example, leaves a Delete confirmation open on another row alone.

### Things changing underneath

Focus follows the thing, not the position. When the session list reorders, a focused row, or its open menu, moves with
its session.

A refresh never changes what the user is typing. Text, caret and focus stay as they are, and only what the user is not
editing updates. If the new information makes the input invalid, the field shows that and saving refuses, but focus does
not move.

A menu has one highlight. Whichever moved last, the pointer or the keys, sets it, and Enter acts on the highlighted
item. Keys typed before a menu finishes appearing apply in order, as if typed slowly.

Confirmed 2026-10-17: when the user moves between controls (menu items, buttons, switches) by different means within
tens of milliseconds of each other, such as an arrow key and a pointer or screen reader at nearly the same instant, a
race may leave focus or selection somewhere other than where they meant. That is accepted. Such a race is still worth
fixing when the fix is reasonably simple, but it is not a defect to engineer around otherwise. The bound is firm: a race
never runs a command, or applies a destructive action, that the user did not activate on a control that was visibly
focused when they activated it. Keys alone do not race this way, per the menu rule above.

### Reaching everything by keyboard

Every scrollable region in the GUI can be reached and scrolled with the keyboard alone in every engine Farhelm ships on,
WebKit included. A region whose content holds nothing focusable gets its own tab stop with an accessible name. Pop-up
suggestion lists and menus use their own arrow keys, the pointer-only file hover cannot be scrolled by anyone, and the
terminal scrolls with Shift+PageUp/PageDown. On macOS, keyboard-only users reach buttons with Keyboard navigation or
Option-Tab (the default Tab order includes only text fields and pop-ups), so button-only regions need no extra stop.

Where a specific control starts with focus, or which control a specific dialog returns focus to, is described with that
feature elsewhere in this document. Those details apply these rules; they do not override them.

### Cases that settled disputed points

These came out of the walkthrough and are worth keeping as acceptance cases, because each one has gone wrong before:

- The user submits a launch, then clicks another session and types. When the launch finishes, their typing keeps going
  to that session; the new session appears in the list with a brief **Started** report.
- The user clicks a plain Replace and touches nothing; the YOLO question arrives and Cancel takes focus. Had they typed
  into the terminal meanwhile, the question would appear without focus and wait.
- The user selects a session and, before its terminal finishes loading, types in the session filter. The filter keeps
  focus when the terminal arrives.
- The user's session is deleted from another window while they type in it. The next session is selected and its terminal
  takes the rest of their typing (the terminal is home).
- A shell tab exits on its own while the user types in it. The session's agent tab takes focus.
- An agent asks to add a host while the user types in a terminal or fills in the launcher. A notice offers the request;
  nothing opens until the user opens it.
- Escape typed into a terminal while a question waits on that session goes to the terminal's program, not to the
  question.

## Desktop window chrome

The macOS desktop window integrates its native title bar with the app header: native traffic-light controls sit in the
sidebar's top row beside the version readout, with no separate visible app-title strip. The Templates control sits
beside New in the session list header. The merged row of terminal tabs and session actions continues the app's surface
to the top edge, aligned with the sidebar's app bar at 48px high. Together the two rows form the window's title band,
which spans the whole width of the window and behaves like a classic title bar: pressing anywhere on its empty
background and moving drags the window, and double-clicking there zooms it (repeating the gesture restores the prior
frame); a single click without movement never zooms. Empty background is everything in the band except its controls
(buttons, tabs, the version readout, the sidebar's resize edge) and its text; the wordmark counts as background, like a
title, and so do the gaps between and around controls and the band's top and bottom edges. With no session open, the
main pane's half of the band is empty background too. Every such press drags, whatever input device made it. Controls,
text in the band, and the terminal retain their own interactions. The Linux window layout retains its existing
appearance and gains no drag or zoom behavior. In narrow macOS windows, the app-level row stays fixed above both
scrolling panes so horizontal scrolling cannot move application controls underneath native window buttons. Startup,
authentication errors, and build-mismatch notices also keep their content clear of native controls.

Cmd+N in the Mac desktop app opens New with the same prefill, even from a focused terminal, unless a modal is open or
New is disabled. New's hover text names the shortcut in the Mac desktop app only; the Linux app has no such shortcut.

Cmd+W in the Mac desktop app, or Ctrl+Shift+Backspace on Linux, invokes the open session's header Delete action with its
ordinary confirmation and busy rules. With no session open it does nothing. The Mac app's Window menu has no Close
Window item, leaving Cmd+W to session deletion; the window close button and Cmd+Q still close the app. The Linux chord
leaves text editors other than the terminal alone. A visible modal keeps keyboard ownership.

Cmd+L in the Mac desktop app, or Ctrl+Shift+L on Linux, opens Rename for the open session, including when its row is
filtered out of the sidebar. Closing that shortcut-opened dialog returns focus to the terminal. Both session shortcuts
work from terminal focus without sending their chord to the program, ignore repeat and composition, and appear in the
relevant tooltips: Delete on the header button, Rename only on the open session's row-menu item.

Cmd+0–9 in the Mac desktop app, or Ctrl+Shift+0–9 on Linux, directly launches templates with number keys (see Creation).

Cmd+X in the Mac desktop app, or Ctrl+Shift+X on Linux, begins a quick-launch template sequence (see Creation). Text
fields outside the session launcher keep their normal Cut or text-direction key; the terminal accepts the sequence
prefix. In the launcher's own fields, selected text keeps that normal action and an unselected field accepts the prefix.

Cmd+K on macOS, Ctrl+Shift+K on Linux, opens a quick switcher, including from a focused terminal. It works during a
pending launch, whose hidden launcher is no longer modal. It does nothing while another modal is open or before
preferences finish loading. Type a case-insensitive subsequence to find sessions across all hosts, independent of the
sidebar's host selector: title matches come before matches only in host name or directory, with most recent activity
first within each group. With nothing typed, sessions appear in most recent activity order. Every match is reachable by
scrolling; if the helm's listing cap cut the snapshot, the switcher says only the most recently active sessions were
searched. Rows show status, agent, host (with its chosen mark) and directory, with matching characters marked. Arrow
keys choose and Enter opens the selected session, including while a session action is pending; the first match is
initially selected. Opening a session hidden by the host choice or status filter resets both to All after the open is
accepted; a refused pick keeps both choices. A pick already visible through both leaves them in place. Escape or a click
outside closes and restores the previous focus. A loading list says loading, rather than no matches, and Enter picks
nothing until a reply arrives; a failed read says so.

While text is typed, a pinned row below the scrolling session matches offers "New session named X". It follows the
sessions in arrow-key order and is selected when no session matches, except while the session list is still loading,
when arrow keys cannot reach it. Picking it opens New with that name and the default launch target and directory
(Creation), plus remembered permission defaults. A pinned templates section normally shows only a `tl:` hint. With a
leading `tl:` (case-insensitive), sessions and the New row are replaced by matching templates, using the session
launcher's own template search; `tl:` alone lists every template. Picking one opens New with its edits applied, without
using the query as the session name. New shows a cancellable loading state until the template inputs are ready, before
allowing draft edits. Neither name nor template pick launches a session: Launch remains the user's action. With no
query, a separate final "Quick launch: <template>" row uses the direct quick-launch path, or opens Templates to mark
one; the first recent session stays initially selected. After a failed session read, New and template search remain
usable.

Cmd+J on macOS, Ctrl+Shift+J on Linux, opens a template-only list when no dialog is open. It follows the quick
switcher's focus, Escape, loading and modal rules. Typing filters template names as `tl:` does, without a prefix. Rows
show the template name, summary and its ⚡ or number shortcut when assigned. Enter starts the highlighted template
through the fresh quick-launch path; clicking a row does the same. A name exactly equal to the trimmed query
(case-sensitively) is highlighted over an earlier substring match, and Enter starts it; arrowing to another row changes
that choice. With no match, Enter does nothing. A partial template opens New with its choices applied. A pick uses the
template currently holding that name, including a template edited or recreated since the list opened; a name missing
from the current catalog shows a preparation error in New. The chord never reaches the terminal's program and does
nothing while any modal is open, including New, Clone, Replace with and hidden quick-launch preparation, or before
preferences finish loading.

Cmd+/ on macOS, Ctrl+Shift+/ on Linux, opens **Keyboard shortcuts**, including from a focused terminal. Matching uses
the physical Slash key, with no extra modifiers; repeat and composition do not open it. The chord never reaches the
terminal's program, and does nothing while another modal is open or before preferences finish loading. The same dialog
is also available from the **?** menu. Its Sessions, Launching and Terminal groups show every app-wide shortcut and the
keys inside a quick-launch sequence, with action names beside key chips. New appears only in the Mac desktop app. Each
shortcut shows only the form this client handles: quick switcher, template list, ⚡, terminal text size and this dialog
follow the operating system, while New, Delete, Rename, number keys and the sequence prefix use Command only in the Mac
desktop app. The dialog names the current ⚡ template, or says none is marked, and lists assigned number keys with their
template names. It refreshes those shared bindings once each time it opens; a failed read keeps cached names. Keys used
only inside other dialogs are omitted. Escape, Close or a click outside closes the ordinary modal and restores the
opener's focus, including the terminal or the **?** button. The documentation's Keyboard shortcuts page lists both
platform forms.

The native desktop remembers its last ordinary window rectangle and whether it was maximized when it closed. On the next
launch it restores that rectangle only when it fits on a currently connected display; otherwise it opens at a safe size
centered on a current display. On Wayland, which does not expose reliable global window positions, Farhelm keeps a
usable saved size and the maximized state while leaving placement to the compositor. Native fullscreen is not persisted.

## Durability and resume

The runtime guarantees below do not establish support for every historical data schema or a downgrade path between
arbitrary versions. The upgrade floor governs the update mechanics; data compatibility is decided with the maintainer
per feature, as specified in the maintainer-confirmed decisions below; agents must surface potential data/state loss
before proceeding with such changes. A schema version bump on its own, where older releases merely refuse the newer data
and nothing is lost, need not be raised.

Sessions depend on exactly one thing staying up: their host. For environment sessions that host is their sandbox or
container. Suspension freezes its processes and Resume continues them, whereas deleting its last session, or the
environment itself, ends it and removes its files. A container host reboot stops its containers, whose automatic start
and interrupted sessions follow Container sessions. Every other component is disposable, with the sandbox idle-timeout
exception below:

- Clients and the helm can close, crash, or restart freely — closing the app, network loss, Mac sleep, a helm upgrade —
  and sessions keep running, local and remote. Sandbox sessions have an explicit exception: after the remote connection
  ends, Tensorlake's 30-minute idle timeout freezes their processes until the sandbox is resumed.
- The supervisor itself restarting — crash, upgrade, manual restart — must not interrupt its sessions. Terminals and
  agent processes outlive the supervisor process; only a host reboot takes sessions down. This is what makes updates,
  the desktop app's automatic ones included, routine instead of scary.

Local sessions have full behavioral parity with remote ones; what differs is availability, not behavior. The Mac desktop
app's local supervisor is started by the app and keeps running after the app quits (or is started manually — see
Topology); nothing starts it at login or restarts it after a crash before the app is opened again. A remote Mac set up
over SSH has its supervisor restarted by launchd after a crash, and an Update replaces only that supervisor: its
terminal hosts, agents and terminal-tab shells keep running. After a reboot, choose Update in the host menu to start the
job; automatic loading at screen login is unverified. Per the rule above, sessions keep running while the supervisor is
down and reattach when it returns. While no supervisor runs, a terminal keeps only a bounded amount of its program's
output; past that bound the oldest of it is lost, as [Terminal experience](#terminal-experience) describes under gaps.
Sessions persist across app restarts.

Conversation tracking keeps working while a host's supervisor is down: when an agent reports which conversation it is in
(a new session, a `/clear`), the report waits on the host and the supervisor applies it when it returns, so a later
Restart uses the conversation Farhelm has recorded, subject to the tracking limits below. Asking Farhelm for something
from inside a session does not work while the supervisor is down, and that is expected for now: an agent's
`farhelm spawn` and `farhelm agent` commands fail until the supervisor is running again. They also need the helm, which
on the Mac lives in the desktop app, so there they fail whenever the app is closed, although the supervisor keeps
running.

Confirmed 2026-10-14: after a session or its conversation is replaced, by clicking, `/clear` in the agent, or similar,
Restart resuming the previous conversation within a brief window is acceptable, and so is Farhelm then continuing with
the conversation Restart resumed instead of the one the agent had switched to. Separately, a new conversation Farhelm
never learns of, leaving it undiscoverable long term, is a defect; the existing session-tracking notifications under
Status mitigate failures to learn a conversation. When Farhelm already has a conversation recorded and tracking silently
stops updating underneath it, continuing to resume that recorded conversation is an accepted consequence of whatever bug
stopped the updates, not a separate defect in itself. Such tracking bugs are still to be avoided; having a recorded
conversation does not mean Farhelm can detect every later failure to update it.

Confirmed 2026-11-04: a conversation Farhelm learned in the last moments before an OS crash or power loss may be
forgotten, like other recent state. Restart may then have no conversation to resume or may resume the previously
recorded conversation instead.

The environment contract: a session process behaves as if the user had SSHed into the host and typed the command in
their interactive shell — PATH, rc-file variables, locale included — even though the supervisor starts at boot. That
SSH-and-type test is the contract when shell sourcing subtleties (login vs. non-login, `.profile` vs. `.bashrc`) would
otherwise leave room for argument. A bare `claude` in a command launch must work exactly as it does from the user's own
shell; "command not found because a daemon launched it" is a bug, not a caveat. One deliberate exception: the directory
holding the Farhelm binary that launched the session comes first on the session's `PATH`, so `farhelm` run inside a
session reaches that exact build; in the Mac app's side-by-side version layout that directory holds the app's forwarder
instead, so `farhelm` reaches the version of Farhelm now running, which after an update and restart is the newer one.
Other programs in the same directory take precedence over the user's own `PATH` order as a result. The environment is
evaluated at each launch: edit your rc files and the next launch or restart sees the change; already-running sessions do
not. Command and resume-command text, and an agent launch's wrapper prefix followed by safely quoted composed arguments,
are then parsed by non-login, non-interactive Bash, regardless of the login shell. Ordinary aliases and unexported
functions from the login shell are not inherited. Bash honors an inherited `BASH_ENV` startup file and exported Bash
functions, which can change its subsequent environment. The shim still supplies its restored PATH, markers and working
directory to Bash. An agent launch without a wrapper and a legacy session keep direct exec.

When a host reboots, its supervisor starts automatically on Linux hosts with the system-integration layer; the desktop
app's local Mac supervisor returns when the app or binary is next started. A remote Mac's reboot behavior is stated in
the setup plan (see Topology). Interruption is classified whenever the supervisor comes back. After a boot, sessions
last known running show as **Interrupted** — explicitly a lost-track state, not a claim about what happened in between:
the agent may have exited on its own moments before the reboot, the supervisor cannot know, and interrupted says exactly
that. Sessions already known exited (including user-stopped ones) keep their status; an exit during supervisor downtime
with no reboot involved shows as exited — with the true exit code when the surviving terminal still holds it, unknown
code otherwise (reporting a code the terminal genuinely retains is not guessing; inventing one where nothing retains it
would be). Interrupted sessions' terminal contents are gone — there is no conversation history store (see Terminal
experience) — but the conversation itself is recoverable. Opening an interrupted session shows a centered neutral card
in the empty terminal area explaining that a host reboot or a Farhelm update ended the agent and that it needs an
intentional restart. The card offers Restart when the session can resume its conversation (see Lifecycle operations),
and Replace, which starts a fresh session under the same settings and keeps its inline destructive confirmation; a
session that cannot resume offers only Replace and says why. Nothing respawns unattended — an agent (especially one
launched with permissive flags) only restarts or replaces when the user chooses and confirms. The system must not
presume the original OS process survived the reboot.

The resume promise is per-session: for agents with conversation-identity integration, the supervisor captures which
agent conversation belongs to each session, and restart resumes exactly that conversation (e.g.
`claude --resume <conversation-id>`) — even when several sessions share a working directory. Claude Code, Codex, Goose,
Pi, Muse, and Grok integrations at this level are required. Confirmed 2026-10-01: Farhelm identifies an agent's
conversation only from the harness's own explicit report, through a hook, plugin, extension, or whatever reporting
mechanism that harness needs. A selector entered in New, Clone, Replace with or Restart with is a one-time start
argument, never a captured identity. Claude, Codex, Grok, Pi, Muse and OMP can report the resulting conversation through
the ordinary path. A named Restart with keeps the old capture until that report replaces it; Codex and Muse report at
the first prompt, so plain Restart before then still enters the old conversation. Goose cannot report on this resume, so
a named Restart with clears its old capture in the relaunch transaction; a definitive pre-spawn abort restores it, an
ambiguous failure does not. The same clearing applies when Farhelm's conversation hooks are turned off for the agent
(`FARHELM_AGENT_HOOKS`) or when the conversation reporter's extension cannot be prepared, since no report can then
replace the old capture; Grok's own callbacks are not affected by that switch. Cursor, OpenCode and named Goose starts
do not gain later plain Restart from this choice. Heuristics that cannot be relied upon, such as correlating the
vendor's files on disk with a launch, are not supported, because a wrong match resumes, and appends to, a conversation
that is not the session's own. Checking the exact file a report names is verification of that report, not
identification. A launch whose harness cannot report, or whose report never arrived, has no captured identity and cannot
be restarted: Restart says why, and Replace starts the session over. What capture never does is write to the agent's own
configuration or record directories. The opt-in managed-checkout Claude trust edit is a separate, narrowly scoped
exception to the no-agent-configuration rule; it does not participate in capture. A hook passed on the command line for
one launch is allowed because it writes nothing the vendor owns — no configuration file, no conversation record, no
trust state — and cannot outlive the launch that carried it. It is not invisible in the absolute: the report it delivers
is written to a file in farhelm's own state directory and from there lands in farhelm's own database, and every run
leaves a line in farhelm's own hook log. Vendor-owned state is the boundary the no-agent-configuration rule from Status
is protecting, and that rule's own example — hooks written into the agent's configuration — still stands. Grok is the
documented opt-in exception: the user installs its hook entries, and Farhelm itself never writes, edits, or removes
them. Every integrated agent type identifies conversations only through accepted reports; Farhelm never selects a
conversation by scanning vendor state. Historical stored identities remain usable under the same per-type Resume rules,
regardless of their source. A reporting credential alone does not establish which Codex conversation is in the
foreground. Goose persists a credential-free named MCP reporter with the conversation and reuses it on resume; Pi loads
a private static extension from Farhelm's state directory on every launch. A Pi report without a session file withdraws
the old resume target. Before a Pi resume, Farhelm reads the bounded first record of that exact file without following
symlinks and requires its session ID to match. A failed check withdraws the durable resume offer, which makes Restart
unavailable, and rejects the stale Resume request so the user can refresh; it never silently launches fresh under that
request.

Codex's first admitted selecting report pins the native Codex process that emitted it, under the session's agent
terminal. Later reports must come from that process, not a Codex process below it that inherited its credential.
Resident launchers and other processes above it are accepted. If a recorded Codex ancestor later reports a verified
conversation, Farhelm corrects the pin and resume target to that ancestor and posts a notification once per launch,
explaining what a Restart before correction would have resumed. An ancestor's source-less `Stop` can make this
correction too; an unverified record changes nothing, and a not-yet-resumable ancestor `Stop` is acknowledged without
effect. A dead pin is released for a new Codex whose ancestry does not include the pinned process, after considering
ancestor correction; a descendant whose report still contains the pinned process stays refused even after that process
exits. The pin survives a supervisor restart but clears on every relaunch and is not restored by an aborted relaunch.
Farhelm also verifies the exact reported transcript's root-session metadata; process ancestry alone cannot distinguish
threads sharing a process. The durable locator keeps runtime session identity separate from persistent thread identity,
and resume uses the latter. Custom Codex homes work through the exact reported path; Farhelm does not search another
home or choose a newer file. A legitimate reported `/clear` switches the current identity even when its transcript is
not yet persisted: the old conversation stops being offered, and only the new conversation's exact file may make it
resumable. A later attributed `Stop` for that same runtime confirms a pending clear once the file exists; unless it is
correcting a pin from a recorded ancestor, it cannot select another conversation or recheck an already resumable
binding. Selection and confirmation both survive a supervisor outage and are applied in that order. Codex sessions
launched before this subscription was added must be relaunched before conversations begun with `/clear` can become
resumable. An unrelated rejected report leaves the foreground identity untouched. Compaction preserves the conversation,
and a verified new conversation replaces it. Unverifiable historical bare IDs remain stored but are not offered as exact
resume targets. Missing or changed transcript evidence refuses Resume rather than silently launching fresh or selecting
a different historical conversation.

Agent files on an execution host are trusted host state. Farhelm reads them only when a decision needs information the
report does not provide: report admission, confirmation of a pending conversation, or Restart's exact-target check.
Periodic capture passes, session lists and info, and replayed creates do not inspect admitted Codex transcripts or Grok
record pairs. Arbitrary later file changes are noticed at Restart rather than through defensive polling. A missing file
withdraws Resume and records its notification then; a different conversation withdraws silently. An I/O read failure
refuses that Restart with a retry message and preserves Resume. None of these refusals launches an agent.

Claude and Muse attribution is positional: the first process above the hook's narrow shell trampolines is its emitter,
at any depth below the session's agent terminal's root process. Farhelm does not recognize either agent by executable,
command line, or environment; those are vendor details that change independently of Farhelm. Resident wrapper chains
therefore keep reporting. The first admitted selecting report pins its emitter by pid and start token and records every
ancestor through the terminal's root process. Later reports from that process may change the conversation; a nested
Claude that inherited the credential cannot replace or withdraw it. Reports from the same emitter preserve the original
recorded ancestry. A recorded ancestor's report corrects both the pin and conversation and posts a notification once per
launch explaining what Restart before the correction would have resumed. If a nested Claude reports first, it may be
followed until the parent next reports at start, resume, `/clear`, compact, or the end of a turn. Claude's injected
`Stop` hook reports the current conversation and is an ordinary selecting report under the same pin: it can establish
the first pin, update the pinned process's conversation, or correct a child-first selection when a recorded ancestor
reports. A `Stop` refused by the pin is acknowledged silently, with no warning or mutation. A proven gone, reused, or
non-running pin releases only for an emitter whose report chain does not contain it; unavailable process evidence never
releases it. The pin survives a supervisor restart but clears on relaunch and is not restored by an aborted relaunch.
Sessions running across the upgrade start unpinned, so their first selecting report may come from a nested child until
an ancestor reports. Running launches retain their old command line without `Stop` until relaunch; no upgrade machinery
compensates for that gap. Farhelm subscribes to Claude's `SessionStart` and `Stop`, not `SubagentStop`. In the observed
Claude Code 2.1.310, a native sub-agent completed through `SubagentStop`, while `Stop` carried the parent's
conversation. Any report naming a sub-agent is refused for every agent type. Claude requires no transcript check and
takes no versioned ownership proof, so its existing captures stay resumable across the change.

Muse uses the same process pin, ancestor correction, dead-pin release and relaunch rules, with a Muse notification once
per launch when a recorded ancestor corrects the followed conversation. Its `SessionStart` and `Stop` both select the
current UUID; a rival `Stop` is acknowledged silently without changing capture. Muse 1.4.14 fires `SessionStart` at the
first prompt, with source `startup` or `resume`, and a source-less `Stop` at turn end. `/clear` and `/new` change the
UUID but defer `SessionStart` until the next prompt; until a report arrives, the previous captured conversation can
remain the Restart target. In the observed same-process reminder runs, native children emitted marked `SubagentStart`
and `SubagentStop` events, while the subscribed `SessionStart` and `Stop` named the parent. That observation does not
cover every possible Muse child. Muse reports carrying `agent_id`, `subagent_id` or `child_session_id` child markers are
refused. Muse requires no transcript check or versioned ownership proof; its pinned captures use ownership version 0.
Farhelm never discovers a Muse conversation from its session files or index.

Grok reports must come from one native `grok` process under the agent's terminal, launched with `--no-leader` before any
real end-of-options boundary. The only admitted descendants are the documented reporter command and its narrow shell
trampoline; a nested Grok or another session-hosting runtime cannot replace the parent selection. `SessionStart` with
source `new` or `load` selects a UUID. `UserPromptSubmit` and `Stop` may add saved-record evidence only for that
selected UUID, and child-marked callbacks never select a top-level conversation. A selection stores the callback's
canonical RFC3339 event timestamp in the Grok locator: another UUID must be strictly newer, while repeats for the same
UUID may advance the timestamp but cannot lower it. Equal or older competing selections fail closed, including after
supervisor restart, without a general event log or clock-recovery protocol.

The selected UUID is resumable only while the exact reported absolute `updates.jsonl` begins with a supported record
whose method and `params.sessionId` match, and its sibling `summary.json` is a complete bounded JSON document with the
same UUID at `info.id`. Farhelm checks that pair at report admission and immediately before Resume. At Restart, missing
or mismatched evidence withdraws the offer while preserving the selected UUID and timestamp; Farhelm never derives a
path, scans Grok history, or chooses another conversation. A fresh `/new` normally remains pending until a later
subscribed event supplies its path. If Grok exits or crashes before the replacement `SessionStart` callback arrives, the
previous UUID can remain Farhelm's last known selection; this accepted delivery race does not weaken validation of
callbacks that do arrive.

OMP reports must come from the foreground OMP runtime under the session's agent terminal — the Bun-executed bundle or
source-tree entry, or the compiled target — reached through the launch's own launcher and trampoline shapes and nothing
else: not a nested OMP process that inherited its credential, not a Node-executed entry, not an unknown wrapper. Farhelm
also requires the session's durable launch record to show the current gated reporter asset was installed for that
launch, with the installed file's bytes re-verified; the gated reporter emits only from the interactive context, so a
delegated task, workpool, or revival child stays silent, while a separately launched interactive child — genuinely
interactive — is refused by process attribution instead. Sessions launched under the old gateless asset fail closed,
runnable with no capture, until a relaunch installs the current asset. A parent lineage field never rejects: legitimate
forks carry one. OMP captures admitted under the proof carry version 1 like Codex, with no historical exception: every
older OMP row offers no Resume until its first proven report. Every conversation-identity report carries a closed vendor
discriminator naming the adapter that produced it — the injected hook command, the Goose helper, or a shipped asset —
and a report addressed to a session of another agent type is refused before any vendor state is consulted. The
discriminator routes; it does not prove. Codex and Grok admission require foreground and record proofs, and their exact
resume additionally requires versioned proof that the binding was admitted under those proofs, with the historical Codex
exception described in SPEC_impl.md. Claude and Muse admission require the positional attribution and pin checks above
but no versioned proof; Claude's resume rules are unchanged. Goose and Pi retain their existing admission and resume
rules; for them the discriminator alone adds no foreground protection. Old senders that predate the discriminator fail
closed rather than reporting untagged. A refused report changes nothing: no stored identity, no offer, no pending state.
Resume is never silently turned into fresh, and historical captures are never rewritten to look proven.

Confirmed 2026-10-22, an accepted exception to restart resuming exactly the session's own conversation (above): Goose's
reporter does not tell a native Goose sub-agent, or a nested Goose run that inherits the reporter, from the session's
foreground conversation, so such a run can replace the session's Resume target. Likewise, a child Pi that loads
Farhelm's reporter (passed on to it by a third-party extension, say) can replace or withdraw the session's Resume
target.

Farhelm-launched Codex sessions run without Codex's shared background server, so their commands keep the session's own
environment and process tree. This applies even with conversation hooks off; legacy sessions keep their old injection.

A compound command or wrapper prefix can keep Bash resident above its agent or wrapper. Claude, Muse and Codex tolerate
that additional process. Grok and OMP refuse an unclassified intermediary when another resident wrapper sits below it,
so such a command can run without capturing a conversation and leave Restart unavailable. Farhelm does not infer or
repair the command's process layout.

OMP (the `omp` program, the `@oh-my-pi/pi-coding-agent` CLI) is another report-only integration beside Pi. An OMP agent
launch gets Farhelm's private extension, and so does a command launch that declares OMP, where its `{farhelm_args}`
stands; Farhelm does not inspect the command to decide whether the extension fits, so a command declared as OMP that
cannot take it is the user's to fix. An OMP report carries the same durable locator shape under an `omp:` prefix instead
of Pi's `pi:`, and the two are never interchangeable: an OMP locator is never accepted for a Pi session or the reverse,
and neither passes as a plain conversation id for the other agent types. Before an OMP resume, Farhelm reads a bounded
prefix of the reported file without following symlinks, skips at most one leading shape-checked title slot (a
fixed-width 256-byte record OMP rewrites in place), and requires the next record to be the session header at
`version: 3` carrying the reported id — anything else refuses. The refusal fails closed the same way Pi's does: the
durable resume offer is withdrawn and the stale Resume request is rejected, never silently launched fresh. A session
header with no title slot cannot be told apart from a Pi-shaped file by its bytes, so OMP's vendor isolation lives at
the locator and report boundary, not in file bytes. Two OMP limitations are stated rather than smoothed over. OMP can
move an active conversation's file without any event Farhelm subscribes to, so an immediate exit after such a move can
leave a stale locator until the next subscribed event — pre-resume verification is what keeps that offer from resuming
an absent file. And a conversation stored somewhere other than a session file, or compressed into a `.jsonl.gz` archive,
has nothing Farhelm can verify, so its resume offer withdraws — fail closed, not a silent fresh start. One OMP
difference works in the user's favor: OMP 18.2.4 persists a new conversation eagerly, so after `/new` the fresh
conversation can be resumable at once instead of waiting for a first assistant message.

Farhelm composes an agent launch's resume command from the same choices as its start command: Claude adds
`--resume <conversation-id>`, Codex and Muse `resume <conversation-id>`, Goose
`session --resume --session-id <conversation-id>`, Pi `--session <verified-absolute-file>`, OMP
`--resume <verified-absolute-file>`, and Grok `--no-leader --resume <verified-conversation-id>`. A command launch that
opts into Resume supplies its own resume command, and Farhelm never derives one from its start command: it fills in
`{conversation}`, `{cwd}`, and `{farhelm_args}` as single-quoted words and hands the rest unchanged to `bash -c`. For Pi
and OMP, `{conversation}` means the verified file path, not Farhelm's internal durable locator; for Codex it means the
verified persistent thread ID, not the runtime session ID or encoded locator; for Grok it means the verified UUID, not
the `grok:` locator or either evidence path.

Anything farhelm attaches to a launch — an agent launch, or a command launch that declares an agent type — must be
invisible from inside the session when it works AND when it fails: no output on the agent's terminal, no non-zero exit,
no error the agent's own UI can show. A hook that cannot do its job gives up silently within a bounded time and leaves
its diagnostics in farhelm's own state directory, never in the user's session. The bound is on the part the agent waits
for — reading the vendor's payload and reporting the result — because that is the whole of what can hold the agent up;
writing the diagnostic happens afterwards, is best-effort, and is not itself bounded. One accepted exception to the
invisibility rule is a line the vendor itself prints because of a flag we pass (Codex's hook-trust warning), which must
be documented. Another accepted limitation is a custom Codex command that cannot accept `--no-daemon` where it placed
`{farhelm_args}` (see command launches): it fails visibly. Codex before 0.156.0 also refuses the flag; older harnesses
follow the support window under "Supported user environments" rather than getting compatibility code.

The other exception is farhelm's own, and it is deliberate rather than tolerated: on a launch that gets the hook, the
hook prints exactly one line for the AGENT to read — that `$farhelm <request>` in the user's message means "use the
`farhelm agent` CLI", and that `farhelm agent instructions` explains the rest. Nothing reaches the user's terminal and
nothing is written to disk. It is on by default, because an agent that has never heard of the CLI will not go looking
for it, and `FARHELM_AGENT_INSTRUCTIONS=off` in the supervisor's environment removes it while leaving identity capture
exactly as it was. A launch with no hook has no pointer either, for the plain reason that there is nothing to print it.
The instructions themselves are printed only when that command is run, so a session where the user never mentions
farhelm pays one line and nothing more. Grok's manually configured hooks omit `--announce`: Grok ignores the relevant
stdout, so its integration delivers no instructions pointer.

Muse's `SessionStart` hook receives that pointer as developer context; its `Stop` hook omits the announcement. The
observed Muse 1.4.14 hooks printed nothing on the terminal on success or failure, so no vendor-output exception is
needed for Muse.

A resume command is never run with `{conversation}` unfilled: no captured conversation means no Restart, with the reason
shown, not a garbled command line. `{cwd}` is always filled where it stands as a whole argument, on every launch and
restart; there is no launch without a working directory.

Existing Muse agent launches derive the Muse integration from their stored harness and gain its hooks through their
stored `{farhelm_args}` at the next start. They retain their original missing resume command: Restart explains that
there is no resume command until the session is recreated. No backfill or migration synthesizes one. This is a
feature-level compatibility choice under "Upgrade compatibility and client scale", not a change to the supported
installation update path.

Sessions created before launch kinds existed keep working after the upgrade. One that was created from structured
choices becomes an agent launch: its stored commands are Farhelm-composed and never contain a bare `--`, so the upgrade
places `{farhelm_args}` exactly where the previous release appended Farhelm's arguments, and from then on it behaves
like any other agent launch. Confirmed 2026-10-03: every other pre-existing session (one created from a profile or a
typed command) stays a legacy session rather than being converted. A legacy session keeps its stored command, agent
type, and resume command, and Farhelm's arguments are still added where the previous release added them, but only for a
legacy session. Restart follows the rule above: it is offered only when the stored resume command can resume a captured
conversation, so a legacy session that relied on a fresh restart or a resume command without `{conversation}` can no
longer be restarted. Plain Replace, `farhelm agent clone`, `farhelm spawn --inherit-agent`, and Restart with refuse a
legacy session with a remedy naming Replace with or a new launch. Clone and Replace with open the launcher on the
command launch kind with the stored command filled in and nothing else, for the user to finish; an interrupted legacy
session that cannot be restarted offers Replace with in place of Replace. A legacy session has no YOLO assertion and
carries the unclassified permission mark.

Profiles are removed outright at the upgrade, built-in and stored alike, with no conversion into templates, and with
them the remembered default profile and every session's profile snapshot. Confirmed 2026-10-03: downgrading across this
change is not supported. An older release cannot read the new launch records, and removed profiles are not restored; the
release notes say so.

## VCS neutrality

Ordinary session operation is version-control-agnostic. Explicit managed-checkout creation is the bounded exception:

- Sessions launch in any existing directory: detached HEAD, no `.git`, colocated or pure `jj` workspaces, nested
  repositories, and plain directories all work identically.
- No requirement of branches, one-branch-per-session, Git worktrees, default-branch workflows, or any PR topology.
  Stacked changes work because the control plane stays out of the way, not because it models them.
- Outside requested managed-checkout preparation and its bounded spare builds below, the system never performs VCS
  mutations implicitly. Repository state is owned by the agent, repository instructions (`AGENTS.md` and kin), and
  user-chosen tools (`jj`, Graphite, plain Git, whatever).
- An explicit GitHub checkout request runs the repository-cache refresh, clone and configured post-clone command
  described above, or updates an earlier independent clone. With the default-on prepared-checkout switch, it also
  authorizes one spare build for repositories the user has already requested on that host: through the user's login
  shell without a terminal or credential prompts, at low CPU priority and with a roughly ten-minute deadline. Failure is
  logged and leaves the ordinary visible clone available. This creates no branch/worktree workflow, and later session
  operations do not infer one. Last-reference Delete moves the owned directory intact; it does not inspect, reset or
  clean its repository state.
- The working directory and the running agent are authoritative; the UI never presents a cached branch model as truth.
  VCS-specific UI, if any exists, is informational and degrades to hidden when not applicable.

## Agent-spawned sessions

Agents in sandbox and container sessions act only within their own environment, and agents elsewhere launch new
environments only behind a card that always asks; both are described under
[Agents and environments](#agents-and-environments) at the end of this section.

A running agent must be able to create sessions itself, via a stable CLI or local API available inside its session,
e.g.:

```
farhelm spawn --cwd /home/user/ws/auth-followup \
  --title auth-followup --agent claude \
  --parent "$FARHELM_SESSION_ID"
```

The spawn CLI talks to the session's own supervisor, authenticated by a per-session credential present in the session's
environment — any process inside the session may spawn, and that is the point. `farhelm spawn` targets the session's own
host, and only that host, however its launch is chosen; `farhelm agent create` and `farhelm agent clone` below reach any
host in the fleet. Like every acting `farhelm` command, spawn needs the helm that owns the host, which asks the user
before it is carried out (see the permission prompts below), so it is refused when no helm is connected to the host,
`--inherit-agent` included. The helm learns of new sessions automatically; they appear in all clients without manual
registration or refresh.

The CLI's contract, since agents will script against it: on success it prints the child session id to stdout and exits
zero, and success means the session exists — a child whose agent then fails to launch still exists, in error or exited
status. Precondition failures exit nonzero with a message on stderr. `--cwd` is required, and the launch is either
`--inherit-agent` or the launch flags shared with `farhelm agent create` below. `--inherit-agent` explicitly reuses the
asking session's stored launch, which the session's own supervisor supplies, except that workspace trust follows the
destination's managed-checkout membership and Settings. The launch flags are resolved by the helm, which owns templates
and composes agent launches. Either is refused with a remedy when no helm is connected. The title is generated when
omitted. An optional idempotency key makes retries safe: re-running spawn with the same key after a timeout or ambiguous
outcome returns the existing child rather than creating another. Keys are scoped to the asking session on its host and
live as long as the child session does: the same key from another session is an unrelated request, never a replay of
someone else's child, and a replay never returns the asking session itself. The same caller scoping and created-session
lifetime apply to the idempotency keys of `farhelm agent create` and `farhelm agent clone`, including when they target
another host. Refusals recorded before any session exists are replayed for 24 hours for all three verbs. Guaranteed
Farhelm-injected environment: the session id (`$FARHELM_SESSION_ID`) and the per-session credential; other
Farhelm-specific variables are illustrative, not contract. (The user's login-shell environment is separately guaranteed;
see Durability.)

Session references handed to clients and agents are full IDs: the host entry's helm-assigned UUID in canonical
36-character lowercase form, a colon, and the supervisor's opaque session ID. The separator is at that fixed position;
the suffix may itself contain colons. Create, clone and spawn print full IDs, and list, detail and mutation replies use
them for the session, its optional parent and its checkout's origin session. Agent discovery uses the entry UUID as its
host ID, and its JSON envelope is schema version 7 with a full `caller.session_id`. The own-session `*` marker compares
the entry and session together; the own-host marker compares the entry. Acting host selectors still use display names as
described below.

Every session route requires the full ID, including terminal attachment, files, tunnels and viewer metadata. A bare ID,
such as one from an out-of-date client, is refused. Agents must also use full targets, except that exact equality with
their authenticated own bare `$FARHELM_SESSION_ID` is accepted on their requesting entry. That equality wins even when
the bare string resembles a full ID. Every other bare target is refused with a pointer to `farhelm agent sessions`. Full
self-Delete and self-Prompt are refused by comparing both entry and session, just like their bare self spellings. The
credential and helm-to-supervisor protocol stay bare; older supervisors keep working without an update for this.

The helm accepts bare session IDs up to 40 UTF-8 bytes, allowing the supervisor's 36-byte UUID and a small margin; a
full ID is at most 77 bytes. Empty, control-containing and dot-segment bare IDs are refused. This also applies to parent
and checkout-origin IDs and to previously cached rows before public display. Supervisors mint ordinary UUIDs; longer
values can only come from a foreign or hand-edited database. A host cannot choose another entry's prefix by reporting a
full-looking suffix: the helm always adds the reporting entry's prefix.

A session can also ASK, not only create. `farhelm agent <verb>`, run inside a session with the same injected credential
spawn uses, reaches the helm rather than the session's own supervisor: the supervisor forwards the question over the
connection of the helm that owns it (each supervisor and its sessions are owned by exactly one helm) and relays the
answer back, because a session has no way to reach the helm's machine directly. Whether the asking session is on screen
anywhere plays no part: an agent in a session nobody is looking at, such as a monitor that launches and deletes sessions
in the background, is answered exactly as one on screen. (An earlier rule asked only the helm displaying the session,
which refused every such agent and reported a connected helm as missing.) The verbs are answered with the HELM's view —
every host and template it knows, and every session it knows, whichever machine they are on — with the asking session
and its host marked. That is deliberately wider than spawn's own-host-only rule above, which stands unchanged: creating
is a local act, asking is not. Every verb goes this way, including questions about the session's own host, so there is
one answer to what an agent sees. The failure this defines is "no helm is connected to this host", reported as such at
once, with opening Farhelm and checking that this host is connected as the remedy (the supervisor cannot tell Farhelm
being closed from Farhelm being unable to reach the host) — never a silent fallback to what the supervisor alone could
have answered. The verbs may also ACT — rename, stop, restart, delete — on any session named by id, including the asking
session when the caller deliberately supplies its id (except Delete), with the helm applying its ordinary rules to the
operation exactly as it would for a client request. Rename also requires the title the caller observed; the owning
supervisor compares and changes it atomically, so a stale agent cannot overwrite a concurrent rename. Restart requires
an explicit `--session` target that can resume its conversation (as in the GUI, there is no other kind of restart), and
an explicit `--stop-if-running` consent when the target is working (an idle, waiting, or unknown target is stopped
without it, as in the GUI). It runs only that session's stored launch on its owning host and accepts no replacement
command. The owning supervisor revalidates both the resume offer and the target's status at handling time, and refuses
rather than launching anything else when the conversation can no longer be resumed. An explicit self restart warns
before dispatch that it can interrupt the invoking CLI, lose its acknowledgement, and leave resumed task continuation
unconfirmed; it never prints an unobserved completion as success. There is no `farhelm agent replace`: an agent
replacing its own session would be killing itself mid-request, which is a design question this version leaves open
rather than answers by accident.

`farhelm agent delete --session <id> [--stop-if-running]` removes a session with the same teardown and checkout notices
as the UI's Delete. Without the flag it refuses any agent or terminal tabs still alive; with it, the approval card names
what would be stopped, and deletion is held to that displayed state just as a UI confirmation is. A session cannot
delete itself: its relay holds its own request fence until the reply, while deletion must take that same fence before
revoking its credential. Self-stop remains available. A delete also refuses a target with an agent command still in
flight rather than waiting, so two sessions deleting each other cannot stall on each other's fences. A lost reply means
outcome unknown; it never claims that the session survived.

An approved `farhelm agent stop --session <id>` closes the session's terminal tabs as well as stopping its agent. Its
approval card says so; this path needs no no-tabs precondition because the approval authorizes closing all tabs.

NOTE: `farhelm agent prompt` is turned off in this version. The helm refuses every such request, as a deliberate refusal
that a retry or approval does not change, before it reads the target, shows a card or contacts any supervisor. The CLI
still parses the verb, so a valid request gets that refusal rather than a usage error, but hides it from help, and the
agent instructions do not mention it. It was turned off before it ever shipped. One gap remains: the "waiting on a
question" refusal reads a status sample up to about two seconds old, so a permission or trust question that appears in
that window (or right after a launch) can receive the message and its submit key as its answer. Re-enabling it means
fixing or knowingly accepting that gap. The next three paragraphs are the design kept for that, not current behavior.

`farhelm agent prompt --session <id> --prompt TEXT` sends a message to another session's running Claude or Codex agent
on any host. `--prompt-file PATH` supplies the same message from a UTF-8 file (`-` reads stdin); exactly one source is
required. The initial-prompt rule applies again at the owning supervisor: at most 64 KiB, with newline and tab allowed
and other control characters refused. The command line, helm and owning supervisor refuse a message that is empty after
trimming whitespace; accepted messages keep their literal whitespace. A self target is refused. The verb holds the
asking session's one-change-at-a-time fence, and asks through the ordinary command approval card unless the asking
host's commands-without-asking setting is on. The YOLO launch rule does not apply because this starts no agent.

The card names both sessions and hosts, shows the full escaped message as labelled data, and shows the target's
permission shield by the session list's rules, including observed permission bypass. It says when the target session is
open in a client. Both the permission shield and that notice are snapshots and may change while the card waits. Approval
authorizes sending even when a user is typing there: the message may mix with or follow text already in the agent's
composer.

At handling time, the owning supervisor requires a live agent reading idle or running and a terminal with bracketed
paste enabled. That reading is its latest status sample, so a question that appears within about one sampling interval
before delivery can still receive the message. Waiting, unknown, starting, stopped or exited agents, and every agent
other than Claude or Codex, are refused with the reason named. A supported request sends one bracketed paste followed by
a separate submit key; the agent owns queueing while busy. Claude uses Enter and Codex uses Tab, each at idle and while
working. The checked Claude 2.1.302 and Codex 0.163.0 queue it after the current turn; Claude's editor expands pasted
tabs to spaces. Success means the agent's terminal took the paste and submit (its terminal host wrote them to the
agent's terminal), not that the agent started or finished the work. There is no idempotency key: a retry sends the
message again. A failed delivery may have sent part or all of the input and is not proof that nothing arrived. Agent
instructions explain these limits and treat cross-agent messages as untrusted input, with no secrets sent through this
channel.

The verbs also CREATE, and this is where reaching the helm buys something no supervisor-local design could offer.
`farhelm agent create` makes a session on an explicitly named host, and `farhelm agent clone` copies an explicitly named
source session onto an explicitly named host. Host selectors use the display NAME the hosts listing reports; stable host
IDs are also exposed so duplicate names remain visibly distinct, but acting commands do not silently reinterpret a name
as an ID. Both print the new session's id on stdout and nothing else, matching spawn's contract, with the human-readable
confirmation on stderr. The preconditions are the helm's ordinary ones: a directory that does not exist on the target is
that supervisor's own refusal, reported verbatim rather than paraphrased on the way back, and an unreachable target is
refused with its state named. The new session appears in every client the way any other create does.

Every acting verb of `farhelm spawn` and `farhelm agent` asks the user first. `farhelm spawn` in every form,
`farhelm agent create`, `clone`, `rename`, `stop`, `restart`, `delete`, `prompt` and `open-url`, and the template writes
below are carried out only once the user approves them in the GUI, including when an agent acts on its own session. The
read-only verbs (the hosts, sessions and templates listings, and `farhelm agent instructions`) never ask, and neither
does Farhelm's own plumbing that runs in a session, such as the hooks that report a conversation's identity, which is
not an agent verb. Farhelm cannot tell an agent from a person typing in a session's shell and does not try: whoever runs
the command in a session asks as that session. The helm decides, because the requesting host and its supervisor are not
trusted to (see Local authority and trust between hosts), and it decides by the host connection the request arrived on,
never by anything the request or its supervisor claims. A request from a host whose "run farhelm commands from this host
without asking" setting is on (see Topology) is not asked about for commands. Opening links uses its own two permissions
below; neither link permission grants command authority, and command authority grants neither link permission. Spawn is
the exception that proves the rule: it is the one action the requesting host's own supervisor carries out, and the
launch `--inherit-agent` reuses comes from that supervisor, so a compromised supervisor could carry out a spawn without
asking, show a different launch on the card than the one it starts, or ignore the YOLO rule below. That is accepted,
because spawn acts only on its own host, which the threat model already trusts. Waiting for the user holds nothing other
operations wait on, apart from the asking session's own next change, which is refused while an earlier one is still
waiting. A link request takes no session-change lock: it neither blocks another session command nor waits for one. While
it is turned off, `prompt` is refused without asking.

**Manage sessions on this host without asking** is a narrower, independent permission on the requesting host. It waives
the card for create (with or without an initial prompt), spawn, clone, rename, stop, restart and delete only when the
target is on that same host. Clone also requires its source session to be on that host. The helm compares the host IDs
it resolved, never display names or an owner the requesting supervisor claims. Requests for other hosts, template writes
and deletes, host changes, links and the dormant `prompt` verb are outside this permission. Actions requiring their own
Allow still ask, and the separate YOLO rule for create, clone and spawn still applies. Plain restart is exempt from that
YOLO rule, including a restart of a YOLO session. The helm must remain reachable to decide; the permission supplies no
supervisor-side or offline authority.

The host settings Permissions section places **Manage sessions on this host without asking** between **Start YOLO
sessions without asking** and **Run farhelm commands without asking**. Its help explains the covered session actions,
the requests that still ask, and that the fleet-wide permission already covers these actions. Both switches always show
and change their own stored values, even when fleet-wide authority makes the narrower grant redundant.

`farhelm agent open-url -- <url>` asks to open an absolute HTTP or HTTPS URL with a host, at most 4096 bytes long. Raw
whitespace and control characters are refused before parsing, matching terminal links; the full original URL is shown as
labelled data on the approval card. The command waits and exits zero after the desktop's native browser opener accepts
the request, not after the destination loads. Denial, dismissal, refusal, timeout and a native opener failure exit
nonzero with a short message.

On remote hosts only, Farhelm supplies `BROWSER=<absolute-session-farhelm> agent open-url -- %s` to agents at their next
launch and to new terminal tabs immediately. Existing agents and tabs keep their environments. This is a conditional
convenience, not part of the guaranteed injected environment above: any BROWSER already supplied by the inherited
environment or shell startup wins, including an empty value. The helm's own machine receives no new BROWSER, and
checkout preparation receives none. A path containing whitespace, colon, percent, quotes, backslash or glob characters
is omitted with a diagnostic, because common readers cannot agree on a quoting convention.

Callers that wait for that command, such as Python's webbrowser and xdg-open, observe its exit result. Rust's webbrowser
crate launches it in the background and reports success before a user answers; Codex's browser-login path uses that
crate and cannot report a later refusal. Not every harness reads BROWSER as a command template: Claude Code 2.1.296 on
Linux passes its entire value as an executable name, so this template does not forward its native opener; an agent can
still invoke `farhelm agent open-url` directly. A login redirect to localhost on the remote host is not forwarded
automatically; opening a link does not itself connect a remote login callback to the user's browser. Login-callback
forwarding remains outside the localhost-tunnel feature.

**Open links without asking** covers public addresses. **Open local-network links without asking** covers literal
loopback, private and link-local IPv4/IPv6, IPv4 0/8, CGNAT and multicast/reserved high ranges, unspecified and
multicast IPv6, mapped local IPv4, IPv6 that embeds a local IPv4 address (IPv4-compatible, the NAT64 well-known prefix
and 6to4), the local-use NAT64 prefix and site-local IPv6, single-label names, and localhost, .localhost, .local,
.internal, .lan, .home and .home.arpa names (including a trailing dot). Classification uses the parsed host without
resolving DNS: a public-looking name may still resolve to a private address on the user's network. A local-network link
always asks unless its own permission is on, regardless of the public permission, and its card explains that boundary.
Always allow on a link card grants only its category, and only while the host still has the installation identity that
asked.

Without the matching permission, Allow and Always allow open the link after the card's ordinary pause. The window opens
nothing itself: its embedded helm opens the link through the desktop's native opener after approval. With the matching
permission on, the desktop opens immediately without a card. Dismiss refuses the request. A standalone helm never opens
a browser on the server.

For a loopback URL from a remote session in the Mac app, the card offers **Tunnel and open** first, alongside the
ordinary raw-link answer. It names the remote host and port, and explains that opening directly reaches the Mac instead.
The tunnel answer follows the same consent warning and lifetime as a terminal Cmd-click; the helm opens only the
rewritten tunnel URL. It grants no host-wide link or command permission. An answer on a noneligible card or without
recorded consent is refused without settling the pending request. Existing permissions that open a native link without a
card still open the raw URL; they do not automatically create tunnels.

While requests wait, the GUI shows one expanded card centered at the top of the main area, below the session's tabs when
a session is open and clear of the sidebar and the terminal's prompt. The oldest request opens first. Other requests
appear as labelled headers below it; selecting a header expands that request instead, and it stays selected until
answered or expired, after which the oldest remaining request opens. The card shows the total waiting count. Every kind
uses the same width and top anchor, with one aligned list of labelled facts instead of narrow columns. Labels may stack
above values in a narrow pane. Paths, repository names, SSH destinations and session IDs stay on one line and scroll
sideways within their own values; URLs and permission flags wrap in full. Commands, prompts, resume commands and wrapper
text each have a bounded, keyboard-scrollable box, preserving all their text.

The request region takes at most about three quarters of the main pane and the space below its top anchor. The card's
header and answer bar stay visible within that cap. Text boxes shrink to about two lines before the fact list needs its
own vertical scroll. Other waiting requests have a separate scrolling list below the card, showing about three headers
at once; a late-answer notice stays above it. Deny sits apart at the right of the answer bar, and buttons wrap in narrow
panes without scrolling out of reach. If session chrome leaves less than the pinned header and answer bar need, the
region uses the whole viewport and may cover that chrome. A viewport shorter than those pinned controls is the one
exception: the region then scrolls as a whole so every control remains reachable. At ordinary sizes the rest of the app
stays usable, and cards remain answerable over open dialogs. Answer buttons pause for 700ms whenever the waiting list or
the expanded request changes, so a click already aimed at the previous request cannot answer the next one. It names the
requesting session and its host, the action, and its target. For a launch it shows the target host, the folder (the
exact previewed directory and source-qualified repository for a managed checkout), the agent type and its choices or the
full command and resume command text, and whether the launch is YOLO. For a template write it shows the whole resulting
template, including the command and resume command text that the templates listing withholds from agents. When the write
changes ⚡ or a number key, the card shows each assignment or removal and names the template a moved mark or key leaves.
For a template delete it shows the template being deleted. Text an agent controls (session titles, folders, command
lines) is shown as labelled data, never in a way that could pass for Farhelm's own wording. Link cards likewise show the
entire URL as data. The card offers Allow, Always allow from the requesting host, and Deny (Open and Dismiss for a
preapproved web link). Always allow turns that host's setting on and then approves the request; if turning the setting
on fails, nothing is approved and the card stays up with the reason. It approves only the request it was answered on:
other cards already waiting from that host stay until they are answered. Asking, target and clone-source session IDs
remain distinct from titles and host names. The compact From line shows an isolated host, a quoted, ellipsized session
title with its full text on hover, and the start and end of the requester's bare ID, without splitting an escape
spelling. Its hover and accessible name carry the complete entry-prefixed ID with spaces, formatting characters and
literal escape notation spelled distinctly. Copy ID copies that full raw reference. Target and clone-source IDs stay in
full in single-line identity facts with Copy. Every copy uses the raw reference, including characters shown as escapes.
Answering the card still uses its approval ID.

Launch and restart cards show one Permissions row using the session list's classification: a warning YOLO chip for YOLO
or Custom, the vendor's guarded mode name, Approve, Smart approve, Chat, or Unknown for a launch predating
classification. Own setting explains that Farhelm treats it as YOLO. A command launch's Asserted YOLO or Asserted not
YOLO is explicitly the agent's unchecked claim, never Farhelm's verdict. Stored workspace trust has its own Trusted
workspace or Not trusted chip; trusted managed-checkout wording names Settings as the source. Permission flags remain a
separate, fully wrapped fact. Prompt cards use the same chip presentation; template cards keep their stored permission
labels and YOLO assertions without interpreting an omitted permission as own setting.

A qualifying session card offers Allow, **Always allow on <host> only**, **Always allow from <host>**, and Deny. The new
button grants only **Manage sessions on this host without asking** to the requesting install and approves that request.
It uses the requesting host's display name as peer text and explains that other-host requests still ask. It is absent on
nonqualifying cards and on any card requiring a direct Allow. The helm refuses that answer on an ineligible card without
settling it, and checks the requesting install's identity under the same host write lock as the fleet-wide grant.
Failure to store the grant leaves the request answerable; already-waiting cards still need their own answers. An older
UI ignores the new eligibility field and keeps its existing buttons. The new answer is a distinct variant so an older
helm rejects it rather than interpreting it as a fleet-wide grant.

If no Farhelm window is open to show the card (no UI holds the helm's seat; see
[One GUI at a time](#one-gui-at-a-time)), a request that needs approval is refused at once, with a message saying there
is no Farhelm window to ask in and to open Farhelm and retry. Otherwise it waits for the user's answer for at most nine
minutes, then is refused as not answered and its card disappears; nine rather than ten so that an agent whose shell
commands are cut off at ten minutes, as Claude Code's are, still sees the answer. Declined, not answered, and no GUI are
distinct messages on stderr with a nonzero exit, and nothing is carried out after any of them. While it waits, the CLI
itself says once on stderr, after a couple of seconds without an answer, that it is waiting for the user to approve the
request in Farhelm. Deleting a session first denies the requests it has waiting. An approval holds only for the host
connection the request arrived on: a request whose host was replaced or re-identified while it waited is refused rather
than carried out. An approval that arrives after the asking CLI was killed still carries out exactly what the card
showed. While its session still exists, an approved managed-checkout create's keyed retry returns or finishes that
attempt's session without another card; a request never approved still asks as usual. Other retries are new requests and
are asked about like one; when one carries the first request's idempotency key and that session still exists, approving
it returns the session the first approval created rather than creating another. After Delete, every keyed retry is a
fresh request with the ordinary approval rules. A host's sessions can keep only a few cards waiting at a time; a
misbehaving host that keeps asking is answered by denying its cards or removing the host, and that nuisance is accepted.

A waiting card's session targets and GUI Delete and Replace confirmations keep the full reference they showed. Removing
and re-adding a host creates a new entry UUID, so an old confirmation cannot reach it even if it reports the same bare
session ID. Retargeting or adopting behind the same entry preserves its UUID and the existing same-entry rules.

On a host that asks before YOLO launches, an agent may only start what runs the way the user already approved. The helm
refuses, without asking, an agent's create, clone or spawn (`--inherit-agent` included) whose launch is YOLO, and any of
them that is a command launch whatever its YOLO assertion says, because Farhelm cannot check the assertion. It applies
that rule again when the user approves, so a host whose setting changed during the wait is judged as it is then. The
refusal names the host and says that turning on "Start YOLO sessions here without asking" for it is how to let agents do
this. A plain restart re-runs the session's own stored launch, which the user approved or a host setting allowed when
the session was created (sessions agents created before these prompts existed included), so it is allowed and gets the
ordinary card unless either applicable command permission waives it, and so do agent launches whose permission is not
YOLO. An agent launch counts as one only when its command lines are exactly what this version of Farhelm composes for
its choices: a clone copies its source's launch as the source's supervisor reports it, which the helm does not trust,
and a session started by an older Farhelm may have been composed differently, so such a launch is refused like a command
launch. There is no override an agent can pass: the command line has no `--confirm-yolo`, and the helm ignores one in
any request from a session.

A wrapper prefix is not tied to an agent type: whoever composes the launch, directly or through an agent, chooses both
and is responsible for matching them. A named template's prefix remains user-configured if a later command-line flag
changes the agent type; that combination needs no separate wrapper approval.

A wrapper prefix from a named template is part of the composed agent command. A prefix an agent supplies in its own
flags, or inherits through clone or `--inherit-agent`, is the agent's own choice: on a host that asks before YOLO
launches, it needs an Allow or Deny card even when ordinary agent commands from that host run without asking. The card
shows the complete command and labels the prefix as the agent's; Always allow is not offered. Approval satisfies the
prefix check on later guard checks, while YOLO agent launches and command launches remain refused on that host. A host
allowed to start YOLO sessions without asking has no prefix approval gate. An agent's template write that leaves a
wrapper prefix in the template always shows a card with Allow or Deny only, even when that host runs Farhelm commands
without asking, because a later launch from that template counts the prefix as user-configured.

For a keyed managed-checkout retry, an interrupted attempt may still start its original command. Farhelm checks that
retained command and checkout before recovery. If the host now asks before YOLO launches and the agent's prefix has not
received an Allow, the card shows those original details and asks before starting it. A prior Allow for that attempt
remains valid across another interrupted retry; a launch first accepted while the host allowed YOLO without asking has
no such Allow to reuse. A completed attempt returns its recorded result without another card.

`farhelm agent create` and `farhelm spawn` take the launcher's fields as flags. `--template <name>` applies a template
by its exact name, and may be repeated to apply several in order. `--agent <type>` sets the agent type; `--model`,
`--effort`, `--permissions` and `--wrapper` set an agent launch's choices; `--wrapper` is one shell-style command line
split into raw prefix words with quotes and escapes retained, interpreted by Bash and with no placeholders. `--command`
makes it a command launch, which needs `--yolo` or `--no-yolo` as its assertion and may add `--agent <type>` as its
declared agent type and `--resume-command` to opt into Resume. Templates are applied first, in order, and the other
flags then act as further edits, so `--template my-codex --model gpt-6-luna` is `my-codex` with a different model. The
result is validated exactly like a GUI launch, with the same refusals naming the field, and the remembered GUI defaults
never fill a gap, so the same templates can launch differently from the CLI than from a GUI that preselected a
remembered permission. A flag required by the verb, such as `--cwd` or the target host, may be omitted when an applied
template sets that field, and an explicit flag wins over a template. `farhelm spawn` targets its own host, so a template
that sets a host is refused there. `farhelm agent create` accepts `--gh <owner/repo>` (`--github` is an alias), mutually
exclusive with `--cwd`, or a template whose destination is a managed checkout. It uses the launcher's checkout root,
post-clone command, repository spellings, naming and refusals: `--gh acme/web --title "Fix parser"` creates
`web-fix-parser`; without a title it chooses the next free `web-N`. A template's base name uses the literal title when
free, otherwise `-2` through `-50`, the same series as the launcher; the approval card shows that resolved title and
directory. Fixed template names and an agent's own `--title` stay exact, with a taken or over-long title-derived name
refused without a suffix. A base name's invalid or over-long candidate is also refused. Only previews search: a create
that loses its approved directory reports the conflict and never submits another create automatically. Keyed recovery
precedes any new name preview. `farhelm spawn` and `farhelm agent clone` do not request fresh checkouts; spawn refuses a
checkout template. `--inherit-agent` is exclusive with every launch flag. `--trust` is no longer accepted for create,
spawn, or template writes; managed-checkout trust follows Settings. A command flag repeated or contradicted (`--yolo`
with `--no-yolo`) is refused rather than resolved by order. The removed selectors `--profile` and `--profile-id`, and
`farhelm agent restart --mode`, are refused with a message naming what replaced them, and `--agent` given something that
is not an agent type is refused with the list of agent types. While its retry record is retained, a retry under an
idempotency key is matched by the request as the agent sent it, not by what that request resolved to. Retrying
`farhelm agent create` or `farhelm spawn` with the same key, host, template names, flags and spawn placement, or
`farhelm agent clone` with the same key, source, host and `--cwd`/`--title` overrides (an override given counts as
different from one left out), returns the first attempt's session, or the refusal its host recorded, even if a template
it names or the clone's source was edited in between, for as long as the request still names something the helm can
resolve. A retry that no longer resolves (a template deleted or edited into something the CLI refuses, an unconnected
host, a clone source that is gone, unreachable or legacy) is refused, and never starts a second session. The same key
with a different request is refused as key reuse, including when a template's destination changed between a folder and a
managed checkout. For managed-checkout creates an approved first attempt is reconciled before another directory preview
or card, including its interrupted start; the agent YOLO rule still checks today's resolved launch. Other creates retain
the rule below. The approval card and the agent YOLO rule judge a keyed retry by what its request resolves to now, so a
retry whose templates changed into a launch the host's YOLO rule refuses is refused even where a replay would have been
harmless, and a retry whose first attempt was interrupted before its session started is refused, rather than started
with what the first attempt resolved to, when its launch, folder or title differs. `farhelm spawn --inherit-agent` is
the exception: its retry is compared by the launch it inherits after destination trust normalization, so a Settings
change can refuse the reused key. `farhelm agent create` requires `--host` with an idempotency key, because the key's
record lives only on the host the first attempt reached, and a host taken from a template could change between an
attempt and its retry. A host name that comes to mean another machine between the two (hosts renamed or reassigned) is a
known, accepted gap: the retry is then a new create on that machine and starts a second session, with every check a
fresh create gets. `farhelm agent clone` copies its explicitly selected source's stored launch onto an explicitly named
host, changing only destination-dependent workspace trust. Clone carries no translation between hosts: a command written
for one machine may name a binary that is absent, a different build, or one that takes different flags on another.

`farhelm agent create` and ordinary `farhelm spawn` accept `--prompt TEXT` or `--prompt-file PATH`, mutually exclusive;
`-` reads the file contents from stdin. Templates carry the same prompt field. The prompt shares the 64 KiB launch
budget, allows newline and tab, and refuses other control characters. It is sent once as literal command-line data at
creation, including for a managed checkout, and stays separate from the stored launch: Restart, Clone, Replace and
recent setups never send or remember it again. A template supplies its prompt on every create using it. Command launches
and inherited spawns refuse prompt flags. Claude, Codex, Cursor, Grok, Muse, OMP and OpenCode support prompts; Goose and
Pi do not. Muse refuses a named conversation plus a prompt. A create's approval card shows the full escaped message in a
scrollable block, under a distinct card kind an older UI cannot decode. Template cards carry the prompt in their fields
and likewise fail an older decoder. A completed keyed create still replays when its template's prompt changes; an
interrupted ordinary agent create refuses a changed resolved prompt before relaunching.

Agents can also write templates, behind the same card, since a template can carry a command line that every host the
helm manages may later run. `farhelm agent template create <name>` sets the fields given by `--cwd` or
`--gh`/`--github`, `--title`, `--title-kind base|fixed`, `--host` and the launch flags other than `--template`; `--host`
names a host by its display name, is resolved when the template is written, and is stored as that host's install. Create
writes the agent launch kind when any agent-launch choice is given, including an agent type alone, and the command kind
for command-launch choices. A template setting only host, destination and session name does not switch launch kind.
Templates already stored without a kind keep applying as stored. Edit, `farhelm agent template edit <name>`, takes the
same flags and sets only the fields it is given. A new session name defaults to base. On edit, `--title` alone keeps the
saved name kind; `--title-kind` alone changes only the kind of an existing session name. A kind without a session name
is refused. There is no way to unset a field: the templates listing withholds command text, so an agent cannot reproduce
a template it did not write, and replacing the whole template would silently drop that text. To drop a field, delete the
template with `farhelm agent template delete <name>` and create it again; the same goes for turning an agent-launch
template into a command-launch one or back, which an edit refuses, since a template mixing the two could never apply. A
command line given to a template write needs `--yolo` or `--no-yolo` with it, since a YOLO assertion is about one
command line. A managed checkout destination is allowed, with the same repository spellings as create. Create and edit
also accept `--quick-launch` and `--key DIGIT` (`1`–`9` or `0`); edit accepts `--no-quick-launch` and `--no-key`,
including as its only change. Assigning ⚡ or a key moves it from another template, which the card names. Clearing a
binding this template does not hold is refused. ⚡ needs a template that chooses an agent or a nonblank command after
the write; a number key has no such requirement. These writes go through the same path as the GUI's template editor,
with one difference: an agent's write or delete lands only on the template its card showed, and is refused, with nothing
written, if that template changed or appeared while the card waited. This is the one exception to last-write-wins for
templates (Concepts), and it exists because an approval covers exactly what the card showed. A write that changes ⚡ or
a key also refuses if the mark or any number-key binding changed while its card waited; the template and bindings are
written together or neither is written. Template deletion keeps clearing its bindings through the shared delete path.

Templates are ordinary fleet metadata exposed by `farhelm agent templates`: each template's name, whether it holds ⚡,
its number key if any, and the fields it sets, with their values except a command line or resume command, which are
listed as set without their text. A session name is listed with its effective base or fixed kind in both the table and
JSON. Discovery also has `--json` forms with a versioned envelope, exact names, the caller's host identity, and
completeness fields. The templates listing includes a prefix's words when one is set. Prefixes belong to the launch argv
and are also visible in process listings, so templates must not hold secrets in them.

Agents can also add, change and remove hosts. `farhelm agent host add <destination> [--alias <name>]` asks to add an SSH
host. Every visible window shows a card whose action opens the ordinary add host dialog there, prefilled with
destination and alias and labelled with the asking host and session. The request never opens the dialog by itself,
because the user did not start it (see [Keyboard focus](#keyboard-focus)). Opening is not approval. An existing draft or
another modal is kept. Listing refreshes preserve edits. Add answers Allow with the destination then in the dialog, and
Cancel, header close and Escape answer Deny through the ordinary approval route. Only the window whose Allow is accepted
starts the ordinary discovery-first add flow, with the edited destination and alias, the same duplicate and alias
refusals, and the same SSH second-factor help. A late or refused answer starts nothing. The command reports the actual
approved destination as accepted as soon as the user answers, without waiting for discovery or setup. **Accepted is not
installed:** adding may still fail, need setup, or be cancelled at the setup question. The agent checks
`farhelm agent hosts` and the host's state before relying on it.

The user's confirmation-skipping settings spare confirmation clicks on actions the user started in the UI: they avoid
accidental clicks, not grant permissions. An agent's request is inherently different. An agent add always shows the
setup confirmation when installation is needed, whatever the setup preference says, including after a refused setup
submission is reopened. Cancel after acceptance is ordinary add cancellation and does not send another approval answer.

`farhelm agent host edit <host> [--alias <name>|--no-alias] [--icon <icon>] [--color <color>]` changes only a host's
alias, icon and color: never its permissions, which would let an agent grant itself more, and never its destination,
since retargeting keeps a host's settings and permissions and would move that trust to a machine the agent chose.
`farhelm agent host remove <host>` forgets a host exactly as the GUI's remove does. `<host>` is the display name the
hosts listing reports, resolved as `--host` is for `farhelm agent create`. All three are acting verbs, and unlike the
others they always need the user's own Allow: the requesting host's "Run farhelm commands from this host without asking"
setting never covers them, and their cards offer only Allow and Deny, never Always allow. An edit's card shows each
field it changes with its old and new value; an edit whose fields already have the requested values is refused as having
nothing to change, without a card, so no edit ever succeeds without an explicit Allow. A remove's card repeats the GUI
remove dialog's own words. Removing this machine is refused as the GUI refuses it, and removing the asking session's own
host is refused because it would cut the connection the request is waiting on, the same reason a session cannot delete
itself; both are refused without a card. On Allow, an edit writes through the same routes as the host settings dialog
and a remove through the same route as the GUI's remove, with their refusals. An edit or remove whose host was
retargeted while its card waited, or an edit whose card showed a field the GUI has since changed, is refused with
nothing changed, even when that change lands just after the Allow; a change the GUI makes to a field an edit's card
leaves alone is kept, and a remove still goes ahead after a rename.

`farhelm agent instructions` (also spelled `farhelm agent help`) prints the agent-facing account of all of the above:
the verbs, the `*` marker, that a session's own credential is what authorizes the question, what to do about "no helm is
connected to this host", that acting verbs may wait up to nine minutes for the user (so run them with a long enough
timeout, and give creates an idempotency key so a retry returns the same session), and what each refusal means. It is
the one verb that reaches nothing — no supervisor, no helm, no credential — because it is what an agent runs first, and
a manual that fails while no helm is connected is a manual nobody reads at the moment they need it. The verb list it
prints is derived from the CLI itself, so it cannot describe a set of verbs that does not exist.

`$farhelm help` in a conversation asks the agent for a brief introduction, the available user-level actions, and a few
natural-language examples, not a raw CLI help dump. The agent derives the actions from that generated inventory and does
not query the fleet or mutate anything merely to explain them. This conversational convention does not change ordinary
shell `--help`.

The instructions must identify session titles, working directories, and agent labels in fleet listings as externally
supplied data, not instructions to follow. The helm relaying those values does not make their authors trusted. This is a
short interpretation rule for the reading agent, not a guarantee that Farhelm prevents model prompt injection.

Spawning starts an interactive agent, optionally with the one-time initial prompt above; later interaction uses the
child's terminal through the user's own typing; the approved `farhelm agent prompt` verb for Claude and Codex is turned
off in this version.

The parent reference is optional organizational metadata only and implies no VCS relationship. It is acceptable for
agent-created sessions to have no parent reference; the parent filter need not identify everything an agent created. The
control plane creates no worktree, workspace, or branch as part of spawning — if the agent wants a `jj workspace` first,
the agent creates it.

### Agents and environments

An agent in a sandbox or container session (see [Sandbox sessions](#sandbox-sessions) and
[Container sessions](#container-sessions)) can ask Farhelm to act, but only within its own environment. It cannot reach
another host any further than it could from anywhere else, change templates or hosts, or create a new environment. The
helm decides this by the session-owned host entry the request arrived on, as it decides everything else about an agent
request, and the rules are the same for every provider. Within that scope the ordinary rules of this section apply:
approval, the YOLO rule, idempotency keys and the environment host's own permission settings. An environment host starts
with the defaults of a new Tensorlake or container host (see [Topology](#topology)): YOLO launches are allowed without
asking, and every acting command from its agents asks until the user changes that host's settings.

- `farhelm spawn` creates in the asking session's environment, as it does on any host. `farhelm agent create` creates
  there too when it names no host or names the environment's own host by the name its hosts listing reports. Any other
  `--host`, a template that sets a host or a recipe target, and `--recipe` are refused.
- Rename, stop, restart and delete act only on sessions in the same environment; a session still cannot delete itself.
  Deleting another session in the environment removes that session only, through the ordinary session teardown, and
  never the environment: the asking session is still in it, so it cannot be the environment's last session.
- `farhelm agent hosts` lists only the environment's own host entry, under its own name, and `farhelm agent sessions`
  only the sessions in that environment.
- Clone is refused, whatever its target: copying a session out of an environment creates a new environment, and a
  session in the same environment is what spawn and create are for.
- The template verbs (`templates` and the template writes) and the host verbs are refused.
- `farhelm agent open-url` keeps its ordinary rules, under the environment host's link permissions.

Every refusal says **Agents in sandbox and container sessions can only act within their own sandbox or container.** No
notification is added, and when no helm is connected the ordinary relay refusal applies.

An agent on another host keeps its ordinary approval rules for sessions in environments, except that Delete of a sandbox
or container session is refused, since only the user deletes one, and that no create or template may put a session into
an existing environment's host entry: those entries are absent from agents' host listings and are refused as `--host`
values. Clone of a sandbox or container session creates a new environment, as Clone in the GUI does, behind the card
below.

`farhelm agent create --recipe <name>` launches a session in a new environment built from that recipe: a Tensorlake
sandbox, or with `--container-host <host>` a container on that host, named by the display name the hosts listing
reports. `--recipe` is exclusive with `--host`, and `--container-host` needs `--recipe`. A template whose target is a
recipe launches the same way through `--template`, and `--recipe` replaces a template's target. The destination follows
the launcher's rules for a new environment: a folder is the new environment's home, and `--gh` clones the repository
inside it. `farhelm agent clone` of a sandbox or container session takes no `--host` or `--cwd`; it targets the source's
recipe at the source's place, with the source's launch and repository intent, never a copy of its disk.

Each of these always shows its own card, whatever the requesting host's permissions say, with Allow and Deny only. The
card names the recipe and where the environment will be created (Tensorlake, or the container host), says that a
Tensorlake sandbox bills while it runs, and otherwise shows what any launch card shows. Its Allow is the creation
confirmation, so a Tensorlake launch shows no separate billing question even when **Create Tensorlake sandboxes without
asking** is off; Deny creates nothing. It is a card kind an older UI cannot decode. After Allow the helm runs the same
launch task as the launcher, whose named progress appears in the host's attention area like any other environment
launch. The command waits for that task and prints the new session's ID. Its wait covers the nine-minute approval wait
and thirty minutes more, counted together from when the request reaches the helm, so a slow answer leaves less time for
the launch; a launch still unfinished then is reported as an unknown outcome while the task carries on, and the session
appears in listings once it exists. The YOLO rule judges these launches by the defaults the new environment host will
start with, which allow YOLO launches, so it refuses none of them. An idempotency key is refused: its record would live
inside the environment the first attempt created, where a retry could not find it before creating a second environment.

## Errors and diagnostics

- Every failed operation surfaces a concrete, actionable error in the client. A dialog must never close as though an
  operation succeeded when it failed. Best-effort exceptions log a failure but stay silent rather than surfacing it: the
  helm-side preference (list order, last selection, compact layout, and the host setup and removal confirmation choices,
  turning the session-confirmation opt-out on, and the feedback contact), because losing next-launch convenience must
  not turn a choice that already took effect into a failed current operation, and a helm that lost the preference falls
  back to the defaults (when its save fails, the choice stays in effect only until the next read from the helm and is
  never resent, an exception to the 2026-10-22 rule under One GUI at a time that a failed GUI save is reported and
  discarded); and the automatic "mark seen" a session's own opening or activity advance triggers (see Status), because a
  lost automatic mark costs nothing worse than a dot that is one open-and-close cycle behind, corrected by the next
  successful write; and marking a session's notifications read when their list closes (see Status), for the same reason:
  a lost mark costs a bell that stays loud until the list is next closed; marking the welcome card seen, because a
  failed save only lets it appear at a later window start or reset; the automatic announcement check and recording a
  dismissed batch, because a failed read or save only postpones those announcements to a later window start or reset;
  and a trust hint's Don't show again save after its session has left the screen, because that session's adjacent error
  no longer has a place to appear. The manual "Mark unread"/"Mark read" toggle and the notification list's clear button
  are not covered — a failed toggle or clear surfaces like any other operation. Turning the session-confirmation opt-out
  off restores confirmations immediately in the current client; if that save fails while Settings is still open,
  Settings shows an adjacent error saying confirmations may still be off for this helm and to try again. Closing
  Settings before the save settles drops that error, so the failure is then only logged. A newer switch choice clears
  the error and supersedes older save results. Quick-launch naming, terminal-frame colors, terminal link-click behavior
  and trust-hint visibility instead put back the saved choice and show an adjacent error when their newest save fails.
  For naming, frame colors and link clicks, closing Settings drops only the alert: the shared choice still reverts, so
  mounted terminals and later actions cannot keep using a failed save. Superseded results neither report nor revert a
  newer choice. A trust-hint choice copied by a reopened Settings dialog or another mounted hint also stops being
  current when its save fails; reopening while that save is pending does not keep the failed choice alive or inherit the
  closed dialog's alert. A failed corrective read leaves trust settings unavailable rather than treating unknown trust
  as off.
- Connection state per host is always visible on its rail avatar, with name and phase on hover and to screen readers;
  reconnection uses bounded retries followed by periodic low-frequency re-probing, so a host that comes back overnight
  resurfaces by itself. Hosts swaps session content for the complete management surface. Each row and its rail avatar
  share one actions menu, with the older-host update button also available inline in Hosts, while the global details
  switch shows the evidence and remedies behind the phase. Concealing management never stops a host operation or hides
  its confirmation dialog.
- Logs are available for: the helm, each supervisor, session creation, process/PTY lifecycle, attachment transfer,
  reconnection, and resume attempts.
- Long-lived input/output/paste paths have health checks, so "typing goes nowhere" is detected and reported rather than
  left for the user to infer.
- Mixed versions across helm and supervisors are a normal steady state, since each host is updated on its own schedule
  (remote hosts when the user updates them, the Mac app when it restarts into an installed update). Incompatible
  versions refuse to connect with a clear, actionable error; there is no silent degradation.
- There is no compatibility across protocol versions. When a release changes the helm–supervisor protocol version, the
  user upgrades the supervisors it manages; until then those hosts refuse to connect and show that they need an update.
  Builds that share a protocol version interoperate (the mixed steady state above); builds on different protocol
  versions never exchange anything beyond the greeting that refuses them. Code and tests therefore do not decode,
  exercise, or shim another protocol version's messages, and reviews should push back on additions that do. Data a build
  persisted is a separate question, decided per feature; the upgrade floor governs which installed versions the update
  mechanics support (see Upgrade compatibility and client scale).

## Feedback

The host rail's foot has a **?** button below the settings gear. It opens a small menu with these items, in order:
**Send feedback**, **Chat on Discord**, **Documentation**, **Keyboard shortcuts**, **Release notes**, **SSH needs 2FA?**
and **Welcome**. Chat on Discord opens the project's Discord invite (`https://discord.gg/G3bukxTjXR`) the same way as
Documentation. Documentation opens the docs site (`https://farhelm.io/docs/`) in the system browser. Keyboard shortcuts
opens the dialog described in [Desktop window chrome](#desktop-window-chrome). Release notes opens the general release
notes page (`https://farhelm.io/docs/release-notes/`) the same way, whether or not an update is waiting. After this helm
has shown an announcement, **Announcements** appears directly after **Release notes**, with the description **Shows
previously shown announcements again**; it reopens the shown history described below. The installed desktop app's menu
adds **Check for updates** last (see [Installation and updates](#installation-and-updates)). Welcome shows the welcome
card again.

**SSH needs 2FA?** shows the two shell commands for each registered SSH host: **Sessions connection** and **Setup and
updates connection**, each with a copy button. Run them on the helm's machine, as its user (on this Mac in the Mac app);
each prompts normally, then stays connected in the background. The dialog recommends taking a screenshot and asking an
agent to automate the commands. With no SSH hosts, it points at Add host. Add host also links to this dialog with
commands for its typed destination; closing it returns to the intact draft. An empty draft asks for a destination, and
an invalid destination is refused before composing commands. Commands use the helm's actual socket paths and connection
overrides, including disabling port forwards: clients cannot cancel forwards belonging to a master someone already
started. When the state path is too long for either socket, contains literal `${`, or is not UTF-8, the dialog names the
affected connection and reason once at the top, including the path and byte limit where applicable, instead of commands
for that connection. It never starts SSH or registers a host.

### Welcome card

Once per helm, a welcome card introduces Farhelm and points at the Get started guide's first-session page and private
feedback. Every helm starts unseen, including existing helms after updating. Each window start or reset decides once,
after a successful shared-preferences read; a failed or timed-out read does not open the card. It is a modal in the
app's dialog style, titled **Welcome to Farhelm**, with focus inside and other global chords inactive. Escape, the ×,
**Got it**, and **Send feedback now** close it and mark it seen in the helm's shared preferences. A failed write still
closes it and may let it appear again at a later window start or reset.

**Read Get started** opens `https://farhelm.io/docs/get-started/first-session/` in the user's browser through the same
opener as Documentation and leaves the card open. **Send feedback now** closes it and opens the feedback dialog, without
sending anything. The footer says it is available again as **Welcome** in the **?** menu. Reopening does not reset or
rewrite seen. Closing a reopened card returns focus to the help toggle; the first-open card has no originating control
and restores ordinary initial focus.

### Announcements

The helm keeps a durable queue of announcements and a history of what it has shown. Each announcement has a stable,
never-reused id, title, plain-text body and optional display details or a labelled link. Enqueueing the same id again
does nothing, even after dismissal, and never replaces the text this helm first received. The queue, dialog and history
do not depend on where an announcement came from.

Once per window start or reset, after the initial preference decision, the client reads pending announcements and shown
history. Initial Welcome, including its Send feedback now handoff, closes first. A failed preference read does not
prevent this separate check. An empty queue opens nothing; a failed announcement read is logged silently and retried
only at a later window start or reset. A batch ready behind another modal waits for it to close. The read-and-show
operation can also be called by a later trigger; there is no live announcement event reader today.

The dialog is titled **Announcements**, with **Updates and information from Farhelm.** underneath. Pending entries
appear newest first, each with its title, body and supplied extras. Blank lines separate plain-text paragraphs; text is
never interpreted as markup. Focus moves inside, global chords are inactive, and **Got it**, the × and Escape all close
it. Dismissal records only the entries that dialog showed, moving them into the helm's shared history with a shown time.
A failed write still closes the dialog and leaves them queued for a later window start or reset. The helm keeps this
state.

After at least one successful showing, **Announcements** stays in the **?** menu directly after **Release notes**. It
opens the same dialog with all shown announcements, newest shown first, and reopening writes nothing. Closing this
history view returns focus to Help; closing an automatic batch restores ordinary initial focus.

The first and only producer is a compiled-in release table, initially empty. It contains only announcements explicitly
chosen by the maintainer during the release procedure, never everything under Breaking in the changelog. Before the helm
serves clients, the producer enqueues entries newer than the version that first created this helm and no newer than its
running build, in full semver precedence including RC and dev versions. Thus skipping releases includes their
announcements, including intermediate release candidates. A fresh installation receives nothing from its own version or
earlier. An older helm without a recorded creation version, or one created by a build from main, counts as older than
every entry. A running build from main (`0.0.0-unreleased`) enqueues none. Stable entries include their release's notes
link; prereleases omit it because curated notes need not exist yet. Only this producer interprets release versions or
builds release-notes links.

### Sending feedback

Send feedback is a lightweight, private way to tell the maintainer something. It opens a dialog with a required message,
an optional field for how to reach the user, and a plain display of everything else that will be sent: the Farhelm
version, that this is the desktop app, and the operating system the user is on. That display is the whole submission;
nothing else is attached (no logs, session content, host names, or paths). Nothing leaves the machine until the user
presses Send. Feedback goes only to the project's maintainer, not to a public issue tracker, and needs no account;
opening a public issue stays possible for anyone who prefers that. On success the dialog thanks the user and closes. On
any failure (offline, the feedback service unreachable or refusing) it says sending failed, in plain words, and keeps
the typed text so the user can retry or copy it. Nothing is queued or retried later.

Under the header summary, the dialog offers "Rather talk in the open? Chat on Discord". Its **Chat on Discord** link
opens the same invite through the same browser opener as the menu item, leaving the dialog and its draft open.

While the contact field is not blank, the dialog shows **Re-use for future feedback**, checked by default, with hover
text. A successful send remembers exactly the contact sent when checked; when unchecked, or when the user emptied a
prefilled field, it clears the remembered contact. A failed send changes nothing. The helm stores the contact in its
shared preferences, so every client shares it. Each dialog starts with the current remembered contact and reuse checked.
Every successful send writes the preference, even when it looks unchanged locally. The feedback submission itself is
unchanged; remembering or clearing happens in a separate best-effort preference write after success.

The helm sends the submission on the UI's behalf; the window never sends it itself. Sending feedback is a UI action
only. Farhelm offers agents no way to send feedback, and no agent request or `farhelm` CLI command may be added that
does: feedback sent that way would leave the user's machine without the user pressing Send. This is about what Farhelm
offers, not containment; anything that runs commands as the user can reach a public URL.

The submission travels over HTTPS to a service the project runs on Vercel, which sees the helm machine's IP address
(used only to rate-limit sends, never put into the feedback), and is stored as an issue in a private GitHub repository
that only the maintainer can read. An intercepting proxy whose certificate the operating system trusts can read the
message and contact information; feedback's privacy does not extend to it.

## Security

The [maintainer-confirmed decisions](#maintainer-confirmed-decisions) below define local account authority, directional
trust between hosts, and the user's approval that agent-requested actions across hosts need. Apply those boundaries when
interpreting the transport and credential rules here.

Steady-state operation between Farhelm's own components uses the desktop app's deliberately local loopback edge between
its window and its embedded helm, and the helm to each supervisor (local IPC, SSH or Podman exec). User-opened localhost
tunnels add a loopback listener on the Mac and a supervisor connection to a loopback service on the session host; the
bytes between machines still ride the existing helm-to-supervisor connection. Besides provisioning's release downloads
(see [Topology](#topology)), three more kinds of outbound connection exist. When the user sends feedback, the helm posts
that submission, and nothing else, to the project's feedback endpoint (see [Feedback](#feedback)); that happens only on
explicit user action. And the installed desktop app's updater (see
[Installation and updates](#installation-and-updates)) asks get.farhelm.io which stable release is the latest, reads
`/advisories` and its signature for release withdrawals, and, when it is newer, verifies that release's signed checksums
and its installer against the keys compiled into the app before running the installer, which downloads the release's
archives from get.farhelm.io and checks them against those verified checksums. The updater does this automatically, at
startup and about daily, unless the user turned automatic updates off, and also whenever the user checks for updates. It
sends nothing about the user or the machine beyond what any HTTPS request to get.farhelm.io carries. A release that
fails that verification is not installed, and the app then says that it can no longer verify its updates and must be
reinstalled with the installer, which, like any fresh install, trusts get.farhelm.io over TLS alone. When ntfy alerts
are configured, the helm also posts waiting alerts and explicit test alerts to the saved server and topic, with the
optional access token (see Status). This connection sends session titles and host display names; it follows no
redirects. Settings allow HTTP for a private server, which sends those values and any token without TLS protection. When
a Tensorlake API key is saved, the helm calls Tensorlake's REST API at `https://api.tensorlake.ai` with it as a bearer
token, for the Tensorlake actions the user takes (Settings' **Test** lists the account's sandboxes; a snapshot build or
preparation creates, snapshots and deletes sandboxes and snapshots; a sandbox-session launch creates its own sandbox,
whose Delete terminates it and removes its snapshots). These requests follow no redirects and are not retried, and the
key appears in no reply, error message or log. Snapshot builds, preparation and sandbox-host connections reach the
sandbox with `ssh` through Tensorlake's gateway. Public-key authentication offers Farhelm's registered ed25519 key
first, excluding configured certificates and hardware-token keys that could take priority. Other configured key files
may follow it. The user's proxy and jump configuration still applies. The gateway's host key is recorded on first use in
a known_hosts file in the helm's state directory, never in the user's own.

- **Client to helm**: the desktop app's window reaches its embedded helm over loopback only. The helm chooses a fresh
  loopback port at each launch, binds only the IPv4 literal `127.0.0.1`, and accepts only the credentials minted in
  memory for that launch and handed to the window directly. They never reach the database, and no request can obtain
  one, so the window never has a credential to lose or renew. TLS serving and binding non-loopback addresses are
  post-v1, together with any remote helm. The helm serves no browser UI and no token exchange. Every helm, embedded or
  standalone, refuses a request a browser page sends: one that carries an `Origin`, or a `Sec-Fetch-Site` of
  `cross-site` or `same-site`, has no authority over the helm, so a page in the user's browser on the same machine
  cannot reach it. The credential keeps other local processes and users out; it does not let the window tell the helm
  apart from another local user's process that binds the same port while the helm is down. That gap is accepted because
  it barely exists for the desktop app: its embedded helm holds the port for the app's whole life, and the app exits
  when its helm ends, so no window is left talking to a port the helm gave up. `docs/security.md` records the reasoning.
  Nothing that establishes the desktop app as a client may rely on anything but the credential the app's own process
  holds; hardening that keeps other software from passing for the app goes through that credential. A standalone helm
  (see Topology) accepts no UI connection.
- **Helm to supervisor**: ordinary remote supervisors use the user's SSH access. Containers use a Podman exec pipe, with
  SSH to the container host when remote. The helm's own machine's supervisor uses local IPC. These all carry the same
  supervisor protocol. Passwordless SSH authenticates with the user's keys, with the Tensorlake exception below.
  Supervisors listen on no network port except IPv4 loopback listeners claimed for configured environment recipe
  tunnels, owned by that full-authority connection. The helm limits claims to environment hosts. The supervisor does not
  know its provider or host kind, and there is no supervisor token to manage.
- **Session to supervisor (spawn)**: the spawn CLI reaches its own supervisor over local IPC only, never the network.
  Its per-session credential identifies the asking session and dies with that session. It does not restrict permitted
  operations to that session: fleet operations follow the Agent-spawned sessions contract and the confirmed trust
  boundaries below. This is an interface credential, not containment against processes with the same Unix account
  authority.

Further requirements:

- SSH is Farhelm's network transport integration for reaching remote hosts. Provisioning, updates and supervisor
  communication share the same route, including Podman exec for containers. How SSH itself reaches the host (a tailnet,
  a LAN, whatever) is the user's business. No public relay or third-party rendezvous service is used. Container traffic
  passes through the container host's Podman process, with no supervisor-to-supervisor relay.
- Provisioning rides the user's existing SSH access — their keys, agent, and config, with the Tensorlake identity policy
  above as the exception. Farhelm stores no SSH credentials of its own, with one exception for Tensorlake sandboxes:
  Tensorlake's SSH gateway admits only keys registered with the user's Tensorlake account, and only a user login, not an
  API key, can register one. For those hosts alone the helm keeps an ed25519 key pair of its own in its state directory
  (private key mode 0600, no passphrase), generated with `ssh-keygen` only when the user asks for it in Settings and
  never replaced while it exists; Settings then shows the command that registers it. Farhelm never offers this key to
  any other host. Confirmed 2026-10-01: the config governs reaching and authenticating to the host (keys, the agent used
  to authenticate, ProxyJump, Match blocks), not what rides Farhelm's connections. Whatever the config says, Farhelm's
  own ssh connections, the supervisor connection and every provisioning step alike, disable SSH agent, X11 and port
  forwarding, and override the settings that would replace or wrap Farhelm's own remote command: a `RemoteCommand`, a
  forced terminal, and a `LocalCommand`. A remote host therefore cannot reach the helm machine's ssh agent, display or
  local ports through SSH forwarding on a connection that stays up around the clock. Explicit localhost tunnels are a
  separate, consented raw-TCP relay inside Farhelm's protocol, from the Mac to the session host's loopback only.
  Accepted: right after an upgrade, a helm started within a minute of the previous version stopping (the desktop app
  reopened, or a helm run by hand) can attach to a shared ssh connection the previous version left open, and port
  forwards that connection set up last until it closes.

  Environment recipe tunnels are raw-TCP relays over the host's existing supervisor connection, in the other direction.
  The supervisor listens on the configured environment IPv4 loopback ports, and the helm connects only to destinations
  in its own recipe snapshot, as reached from the helm's machine. Anything in the environment, agents included, can use
  them. They stop when the host disconnects, is suspended or is removed, and retry with backoff while it stays
  connected. The host details show their status and each mapping; a failure makes the host's dot amber and shows the
  reason. Recipe edits take effect on the next connection. If the helm process dies, listeners and held connections
  close within seconds; if its machine goes silent, the supervisor connection's existing stdin watchdog closes them
  within about two minutes. Disconnecting leaves no separate tunnel connection that could keep the sandbox awake.
- Agent credentials normally live on the host running the agent, in the agent's own standard configuration. The Settings
  credential catalog is an explicit exception: the helm stores logins and tokens the user enters there, unencrypted, and
  never returns their saved values. For environment recipes it sends selected values only to those hosts, into their
  usual configuration or Farhelm's own state there. Farhelm never reads credentials out of any machine's agent
  configuration, proxies them, logs them or passes them as command-line arguments. Claude Code authenticates directly
  with a consumer subscription, unmodified.
- Updates are never forced: the desktop app installs new stable releases in the background by default, but the user can
  turn that off and then update by hand or pin a version with the installer; an installed update takes effect at the
  next start of Farhelm, which Restart to update only brings forward; remote hosts are updated by an explicit Update
  action or the separate, off-by-default automatic remote-host update setting.

## Non-goals for v1

- Desktop or operating-system notifications other than configured ntfy waiting alerts.
- Terminal history persistence beyond what the live terminal retains. A durable history store is a possible future
  add-on; v1 leans on agent conversation resume instead.
- A prompt composer or message-level abstraction over the terminal.
- Multi-writer session sharing, or multiple concurrent helms.
- A remote helm: the desktop app attaches only to the helm it embeds, so the window-to-helm edge needs no TLS, Tailscale
  integration or tunneling. (Automatic SSH transport for the helm-to-supervisor edge is in scope; see Topology.)
- IDE functionality, diff viewers, code review UI.
- Built-in Git branch management, stacked-PR tooling, or a `jj` graph editor.
- Multi-user collaboration and cloud-hosted execution beyond Tensorlake sandboxes used as hosts or individual sessions.
- Container providers other than rootless Podman on Linux, container resource limits and container network policy.
- Reimplementing or wrapping agent internals (ACP or structured agent protocols may come later but must never displace
  raw terminal mode).

## Acceptance test

The first usable version is complete when all of the following pass:

1. From the helm on the Mac (desktop app) — with helm-side access to the configured release source (get.farhelm.io by
   default) or a staged payload directory — given nothing but passwordless SSH to a fresh Ubuntu host, provision it in
   one action: supervisor installed and started without root, host registered, sessions operable with no further network
   setup.
2. Create and launch an official Claude Code session in one action, in an existing `jj` workspace where Git reports
   detached HEAD.
3. Create a local (Mac) session the same way; both appear in one list.
4. Paste a Mac screenshot into the remote session's terminal; the path appears at the cursor and Claude reads the file.
5. Quit and relaunch the app: both sessions are still running, terminal state intact, exactly as left. Then reboot the
   Mac: the remote session is untouched; the local session shows interrupted, and opening it offers resume that restores
   the conversation.
6. With the remote session open, break or stall the window's connection to its helm (the test harness can): the window
   goes inert behind "Reconnecting to the helm…", rebuilds, shows the generic connection-lost notice, and the remote
   session and its terminal come back as left.
7. Ask Claude to create a new `jj workspace` and spawn a child session via the provided CLI, and approve its card; the
   child appears in the client without refresh.
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

Confirmed 2026-11-04: a full disk is included in this allowance. Accepted outcomes include losing durable state, such as
a Resume offer whose restoration after a failed Restart with also fails.

The boundary is host isolation: a remote host's broken filesystem must not freeze or break the helm or prevent it from
serving other hosts. Requests involving the affected host may fail or remain pending, but the helm must otherwise
continue to function. This allowance concerns filesystem errors and hangs, not ordinary cancellation or disconnection
while the filesystem is healthy.

Confirmed 2026-10-11: for a malicious or arbitrarily broken host, the denial-of-service acceptance under
[Remote input, session defaults, and availability](#remote-input-session-defaults-and-availability) supersedes this
availability guarantee. The protections against corrupting the helm's state or affecting other hosts' security or data
still apply.

### Power-loss durability of terminal file downloads and checkouts

Confirmed 2026-10-14: files saved by downloading a file path from a session's terminal (see
[Terminal experience](#terminal-experience)) and managed checkouts are not required to survive a power loss or system
crash, including immediately after an operation reports success. Only critical state, such as the installation the
installer writes and the helm's and supervisors' own SQLite databases, may be held to such a requirement, under its
existing contracts; this decision adds no new durability requirement for that state. This decision exempts only those
downloads and managed checkouts: release and provisioning downloads are not covered, and attachments keep their own
best-effort rule under [Attachments](#attachments). It does not weaken the existing prohibitions on database corruption,
bricking an installation or similar lasting damage.

### Waiting between operations on one host

Confirmed 2026-09-28: session-management operations on the same host (create, restart, delete, and agent-requested
spawn, create and clone) may wait for one another. A supervisor may run them one at a time, so one can be delayed by
another's teardown, including its kill grace periods. Editing a host's registration, other than removing it, may
likewise wait for an install or update running on that host. These waits are expected to last seconds, or for an install
or update, as long as it runs. Queueing of this kind is accepted and is not a defect on its own; do not add
finer-grained locking solely to remove it.

Instead of making a management request wait, a supervisor may refuse it at once when the host already has as many in
progress as the supervisor allows. The refusal says the host is busy and to try again, and it comes before the request
has changed anything, so trying again is safe. Like the wait it replaces, this is accepted and is not a defect on its
own, including when requests that do wait (a large batch of deletes, say) keep the host at its limit for a while and
everything else is refused until they drain. Nothing has to retry a refused request automatically.

The following must not wait on any of those operations, on a host install or update, or on other slow work such as
release downloads or clipboard writes:

- terminal input and output, attaching, detaching and resizing, for every session;
- session status and the session list, for every session, including replies to operations that have already taken
  effect;
- other hosts;
- removing a host, which must respond promptly whatever that host is doing, if only to refuse because it is busy.

"Must not wait" is about the length of those operations, not about every moment of them. Terminal input and output,
attaching, detaching and resizing may wait briefly on bounded local work that one of those operations does under a lock
the terminals share, such as a delete's renames, directory syncs and final database commit, or the bounded shutdown of
the deleted session's own terminal connections; that work is expected to take moments on a healthy disk. They must never
wait on the long parts: a clone, an install or update, a download, or the grace period a session's processes get before
they are killed.

Waits caused by a failing or hung filesystem are governed by [Healthy local filesystems](#healthy-local-filesystems),
and waits caused by a slow host by [Slow hosts](#slow-hosts), not by this section.

### Slow hosts

Confirmed 2026-10-01: a slow host is slow. Do not add complexity to compensate for a slow remote host or a slow machine
running the helm, whether the slowness is in the filesystem, the network or elsewhere, as long as the helm itself stays
usable: it must not freeze, and one slow host must not stop the helm serving the others. Work that is brief on a healthy
host with ordinary amounts of data, such as scanning a folder for a checkout preview or a repository search, may take as
long as the host, or the size of what it scans, makes it take, and it may hold up that host's other work meanwhile,
terminal input and output included. That is accepted, not a defect.

The boundary is work that is long even on a healthy host. Such an operation, whether one session's (such as a git clone)
or the host's own (such as an install or update), must never block another session's terminal input or output; see
[Waiting between operations on one host](#waiting-between-operations-on-one-host). Failing or hung filesystems are
governed by [Healthy local filesystems](#healthy-local-filesystems).

### Paths that are not valid UTF-8

Confirmed 2026-10-01: Farhelm does not support paths that are not valid UTF-8, whether a working directory, a checkout
root, the location of the farhelm program or its state directory. Every surface that meets one refuses it with a clear
message naming the path as well as it can be shown, and nothing ever silently converts such a path into a different one,
for example by replacing the bytes it cannot decode and then using the result as a path. Showing such a path in a log
line or a message with those bytes replaced is fine; acting on the replaced text is not.

### Evidence after resumability is withdrawn

Conversation resume is a core feature while Farhelm can safely identify the session's conversation. Once the current
session can no longer resume its conversation, Farhelm is not required to preserve every remaining capture field through
a definitive failed restart or other recovery transition. A later retry may therefore have less capture evidence when
preserving it would add meaningful complexity. This allowance does not permit turning a valid `Resume` offer into a
fresh launch, or silently substituting another conversation.

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

### Provisioning transfers time out only on stalls

Confirmed 2026-10-01: moving Farhelm's payloads while adding, installing on or updating a host, whether the helm is
downloading a release or sending its files to the host, has no fixed overall time limit. A transfer times out only when
it stops making progress for a while. A release is tens of megabytes, and a fixed limit fails a slow but working
connection at the same point on every retry, with nothing pointing at the link speed as the cause; a stall limit still
catches a transfer that has stopped receiving bytes. This is a deliberate exception to [Slow hosts](#slow-hosts): a
payload transfer is long on any link, so a fixed limit there is a failure, not a missed accommodation. The stall limit
for a transfer to a host starts once the file being sent first exists on that host. A host that stops answering before
then is bounded only by ordinary ssh and TCP behavior, with no time limit of Farhelm's own; that is accepted (confirmed
2026-10-01).

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

### Container sessions

Container sessions are experimental disposable environments built from the shared recipe catalog. Each launch creates a
rootless Podman container on a registered local or ordinary SSH Linux host. The container boots systemd and runs its own
Farhelm supervisor under a login user's systemd user manager. The managed checkout is cloned inside it. Folder
destinations use its home, without Browse or recent folders. Agent and command launches use the ordinary launch rules.

A usable container host runs Podman 4.9.3 or later, rootless, with cgroup v2, the systemd cgroup manager and a working
`systemd-run --user`. A background probe records Linux, architecture, version and each requirement, with a timestamp.
Opening the launcher refreshes probes in the background. No per-host opt-in is needed, and an environment host cannot
itself host containers. The launcher and Settings → Containers show why a host is unusable.

Built software and prepared images are kept per recipe and container host. The first launch there builds a missing
software image and ensures a fresh prepared image. Preparation becomes stale after 24 hours, a Farhelm version change or
a rebuilt source, as for Tensorlake. Launch progress names **Preparing &lt;recipe&gt; on &lt;host&gt;…**, when needed
with its first-launch/24-hour/update explanation, **Creating a &lt;recipe&gt; container on &lt;host&gt;…**, **Starting
Farhelm in the container…**, and the ordinary session launch. Closing the client does not cancel the task.

The helm records the session-owned row and reserved-prefix container name before creation. It connects directly through
`podman exec -i`, locally or through the container host's SSH connection. Credentials, subscription logins, recipe
tunnels, updates, hooks, conversation capture, attachments, terminal tabs, file links and downloads use the ordinary
supervisor paths inside the container. Readable files anywhere in that account's container filesystem are available
under the ordinary file rules. For localhost tunnels the environment is remote, even on the helm's own machine, and
loopback means the container's loopback. No host checkout, Farhelm socket or host account home is bind-mounted.

Container sessions follow Sandbox sessions' ownership, launch cleanup, Clone, Restart, Replace, Delete and visibility
contracts. Clone creates a new container from the recipe at its selected place, with repository intent and no disk copy.
Restart and Replace stay in the existing container. Replace creates first and removes only its source session. Failed or
ambiguous Replace preserves the container and files, reports what is known and identifies the container. An ordinary
failed initial launch may discard its newly created container, including after a lost session-create reply. Failed
cleanup and interrupted launches remain discoverable for explicit deletion through Hosts or **Containers not on this
helm**. Nothing automatically continues an interrupted launch at helm startup.

User Delete follows Sandbox sessions' rule: it asks the container's Farhelm to delete the session and seal the container
if no other session remains, removes the container together with its last session, and otherwise reports that the
container stays and how many other sessions are in it. The confirmation states both outcomes and is never skipped. When
the helm cannot ask the container's Farhelm (the container is paused or unreachable, or its Farhelm is a version this
helm cannot talk to) or gets no answer, it offers the whole-container Delete, whose confirmation warns that the
container, every session and process in it, its checkouts, files and unpushed work are destroyed, including sessions
Farhelm does not list. That Delete works while paused or while its supervisor is disconnected when the container host
can be reached, without Resume. Confirmed container absence precedes forgetting its row and cached sessions. Failure
keeps the resource and reason visible for retry; a sealed container whose destruction failed shows as being deleted,
with the reason and Retry. Host-level **Delete container** always destroys the whole container after its confirmation.
Whole-container deletion moves no checkout to the trash.

Suspend uses Podman pause, preserving processes and memory. Resume unpauses them. The helm closes its exec connection
before pausing, since a paused exec pipe can hang. Controls, the suspended card, rows and Stop and suspend follow the
sandbox pattern, with wording that freezing frees CPU while memory stays in use on the container host. Plain Stop ends
the selected agent and tabs and leaves the container running. Stop and suspend finishes ordinary Stop before pausing the
whole container. Suspend and Resume affect any leftover sessions too.

A container host reboot stops the container while retaining its files. Whenever the helm finds a container Stopped while
connected to its container host, it attempts one automatic start. This also covers an external `podman stop` or
out-of-memory kill. Stopped has its own state and is never labelled Suspended. While starting, the ordinary unavailable
card explains that Farhelm is starting the container again. A failed start shows the reason and offers Resume.
Successful start brings back its supervisor with ordinary session classification. A host reboot interrupts sessions last
known live, while known exits keep their outcome. A container stopped and started on the same host boot returns ended
sessions through ordinary exit classification. Captured resumable conversations offer Restart, and agents never restart
automatically. A helm restart reconnects running containers and leaves paused containers paused.

Session metadata names **&lt;recipe&gt; container on &lt;host&gt;**, with a container mark distinct from the sandbox
cube. The footer names the new container and its host. Its session-owned host appears in Hosts, update eligibility and
the existing attention area, with no permanent rail icon or ordinary launch/template/agent destination choice. A
container host cannot be removed or retargeted while any environment row references it, including stopped environments
and cleanup leftovers, or while recipe state records images or helpers there. The refusal names the environments or
recipes to clean up first. This keeps the route available for confirmed resource cleanup.

Recent setups retain the recipe, container host when applicable, explicit agent selection and repository-or-home intent
after Delete removes the environment. Applying one creates a fresh environment. A missing recipe or container host stays
unavailable until the user changes that choice.

Containers inherit proxy variables from the container host's systemd user manager environment at creation. Farhelm
disables Podman's implicit HTTP-proxy inheritance and writes those values into the container user's `environment.d`, so
its user manager and every session inherit the same settings. Later host changes affect newly created containers.
Rootless Podman supplies the containment. Internet, LAN, tailnet and cloud metadata remain reachable, and this version
adds no network policy or CPU/memory limits. The container host's Podman configuration still applies. Agents in
containers follow the same rules as sandbox agents ([Agents and environments](#agents-and-environments)), including
Delete requests and Clone from other hosts.

Farhelm may pull the recipe's image when missing and the pinned managed Ubuntu base. Superseded Farhelm-built images are
removed after successful replacement once no container uses them. Rebuild at a place and recipe deletion clean up that
place's owned images. User-owned base images are retained, and a checkpoint image Farhelm manages cannot be used as a
recipe base. Settings → Containers shows probe status per host and a refreshable **Containers not on this helm** list
with confirmed Delete and retryable failure, following the sandbox-leftovers pattern. There is no Add host → Podman,
Docker support or macOS container host support.

### Retired containerized sessions

The earlier experimental model, whose supervisor stayed outside the container and bind-mounted a host checkout, is
retired. Existing sessions support Delete only. Opening terminals or tabs, exec, Stop, Restart, Clone, Replace and
Replace with are refused. Delete retains the legacy container-removal path and confirms absence. Missing Podman counts
as nothing left to remove. Stored sessions still decode their old container configuration, including `image_file`.
Templates, recent setups and history with that choice still load. Using them drops or refuses the retired choice with an
explanation and cannot create another old-style session. The old launcher switch and account-selection flow are gone.

### Local authority and trust between hosts

The local security boundary is the Unix account on a particular host. Processes the user runs on the target host,
including same-account processes that can reach Farhelm's terminal hosts (the processes that keep its terminals), are
trusted by this threat model. Farhelm does not provide strong same-account isolation against deliberate interference
with agents or those terminal hosts, and session credentials do not provide same-account containment. Running agents
without permission checks in disposable remote environments is an intended use. Farhelm still keeps simple local guards
against accidental interference where their identity is available, such as telling a session's agent terminal and its
tabs apart by their hosts' tags in tab discovery and destructive tab cleanup. Container sessions have their own rootless
Podman containment contract above.

Farhelm defends against what an attached host or an agent's output can do, and against mistakes a user plausibly makes
in ordinary use of Farhelm or their machine. The test is whether a reasonable user would hit the trigger by accident.
Conditions the user, or whoever administers their machine, deliberately set up outside Farhelm count as the user's
intent: their shell environment, the OS certificate store and proxies, or symlinks and files they reshape by hand. If
the trigger takes a deliberate setup and the consequence follows from what that setup means, guarding against it is not
Farhelm's job. For example, an intercepting proxy whose certificate the OS trusts can read feedback sent through it.
This line between deliberate and accidental interference applies to files too: a hand-made imitation of an installation
record, a symlink whose target path contains a newline, a symlink planted at a path Farhelm reserves for its own marker
files, or every name the installer would use to keep a user's file already being taken. The accidental guards need not
handle those states. Likewise, when a session was launched through a symlink outside the checkout root and the user
later points that symlink at an archived checkout, a new terminal tab can open inside that archive, and deleting it from
the trash is not refused. This never relaxes the remote-host boundary, removes a protection this specification states
for a specific case (such as the foreign-replacement guard, the installer keeping a user's file, or uninstall refusing
unknown app-bundle entries), or accepts database corruption, a bricked installation or similar lasting damage.

A supervisor trusts its owning helm to administer it, launch processes, and forward user input. That trust is
directional: the helm and GUI must treat remote supervisor messages and agent-controlled output as untrusted. A remote
host must not gain unauthorized execution or access to secrets on the helm's machine or another host through Farhelm.
Existing redaction promises remain requirements even where the sender already has local account authority.

An agent in a sandbox or container session has Farhelm request authority only over its own environment
([Agents and environments](#agents-and-environments)). It reaches no other host through Farhelm any further than a
process anywhere else could, and it cannot change templates or hosts or create another environment, so a disposable
environment does not become a way into the rest of the fleet. The helm enforces that by the session-owned host entry the
request arrived on, through the ordinary owning-helm relay, adding neither a supervisor-local policy nor a notification.
Inside that scope, approval and the environment host's own permission settings apply as on any host. Agents on other
hosts cannot Delete a sandbox or container session or target a session-owned host entry. Creating a new environment for
an agent, through `--recipe`, a recipe template or Clone of an environment session, always needs the user's own Allow on
a card that is also the creation confirmation, whatever the requesting host's settings say, because it starts a new
machine and, for Tensorlake, bills. Other supported targeted operations keep their existing approval rules. User
operations follow Sandbox sessions and Container sessions.

Confirmed 2026-10-10: text a host supplied that the GUI shows outside a terminal is escaped so invisible and
direction-changing characters are visible, including in accessible names, wherever it appears and however it reached the
helm (a model name saved into a template from a cloned session, for example). Identity labels the user compares, such as
installation ids in the adopt prompt, also show spaces visibly, so two different identities never look alike. Terminal
output is not part of this rule; it is the remote program's own display.

Clicking a disclosed file path in a terminal transfers that file from the session's host to the user's machine. Any file
the session's Unix account can read is in reach; the hover shows the host, resolved path and size before the click can
download it. Confirmed 2026-10-18: only an explicit Cmd-click in the desktop app (Ctrl-click on non-macOS desktop
builds) also opens the completed local save with the default app. The helm checks the saved file's final extension,
ignoring ASCII case, against a shared user-chosen list that defaults to .md, .txt and .json (amended 2026-11-12 at the
maintainer's request: the default also lists .png, .jpg, .jpeg, .gif, .webp, .bmp and .avif). It also requires the saved
file to retain the final extension disclosed at hover. An empty list opens nothing; any listed type can open, including
HTML if the user adds it. A plain click only downloads. The desktop app deliberately does not set the macOS quarantine
flag on downloaded files, so these saves do not carry the "downloaded from the internet" prompt a browser save would
carry. The click authorizes this transfer across the host boundary, including for agent-controlled files. In the desktop
settings dialog, listed types outside `md`, `txt`, `json`, `log` and (since that amendment) the default's image types
get a prominent warning beside the field. For HTML-like types, it explains that an agent-written page can run scripts in
the browser, open web addresses outside Farhelm's link permissions, and send cross-site requests to services on the
user's machine, including localhost on other ports. Those requests may carry the service's cookies even though the page
cannot read them directly. For other unrecognized types, it explains that Farhelm does not restrict the default app's
handling of the file and that opening an agent-written file may run code.

Opening a link is another bounded effect across the host boundary. Only an admitted HTTP(S) URL may reach the user's
browser, after an Allow/Open click or under the requesting host's matching link permission. With **Open local-network
links without asking** on, a remote host can make the desktop user's browser load addresses on the user's own machine or
network without a click. Neither permission authorizes other commands, file access or clipboard reads. Login redirects
to `localhost` on the remote host are not forwarded automatically; forwarding those callbacks is outside the
localhost-tunnel feature.

Localhost tunnels deliberately extend that browser boundary. Before enabling them, a prominent warning must explain that
a tunneled page is served from `127.0.0.1` on the Mac: cookies are not isolated by port, so it can read cookies from
other services on that address; the browser treats it as a local page and skips private-network checks. The tunnel's
loopback listener has no authentication and accepts connections from any program on the Mac, like SSH port forwarding.
The user accepted these consequences for this feature; consent is off until they accept the warning, and Settings can
revoke it and close the tunnels. A tunnel does not give a supervisor an arbitrary network dialer: only the helm may open
a relay, and the supervisor independently confines its destination to loopback.

A program in an attached remote terminal may write the viewer machine's system clipboard through OSC 52, without a
separate local selection or copy gesture. This is an explicitly allowed, bounded effect across the remote-host boundary,
including when a malicious program replaces the clipboard. Programs must not read that clipboard through Farhelm. A
user-initiated paste is intentional delivery of the pasted content to the selected terminal, not permission for
program-initiated clipboard reads. Clipboard writes remain best-effort as specified in Terminal experience; an opt-out
control is not a current requirement.

An agent's acting `farhelm` commands other than opening links, whichever host they reach, are carried out only with the
user's approval or under the requesting host's own "Run farhelm commands from this host without asking" setting (see
Agent-spawned sessions). That is what keeps a session on one host from gaining execution on another through Farhelm:
creating, cloning, renaming, stopping, restarting and deleting sessions across hosts, sending prompts to running agents,
and writing and deleting the templates any host may launch from, are authorized by the user, not by the request. A user
who turns that setting on for a host grants that host's sessions exactly that authority over the fleet, including
changing what later launches from those templates run anywhere. A retried create is a new request and is asked about
like one, so how long agent-originated retry records are kept is not a security question. The one exception is a create
into a managed checkout: while its created session still exists, a retry with the same idempotency key returns, or
finishes, the attempt the user already approved, without asking again. The replay can only complete that exact approved
request, and only for the session, on the host, that asked. Delete removes that approval's retry record, so reusing the
key then needs the approval any new request needs. **Manage sessions on this host without asking** grants only the
same-host session actions defined there: both the target and a clone's source must belong to the requesting host. It
grants no cross-host or template authority and does not waive direct Allow or launch YOLO checks. Both permissions are
helm decisions, independent of what the requesting supervisor asserts. None of this waives correctness of user-initiated
GUI requests.

That setting never covers changes to the hosts themselves. An agent's `farhelm agent host add`, `host edit` or
`host remove` always needs the user's own Allow on its card (see Agent-spawned sessions), so turning the setting on for
a host does not let its sessions add, rename, restyle or remove any host, and even an approved edit reaches only a
host's alias, icon and color, never its permissions or destination.

Reading is separate from acting, and stays open for ordinary hosts for now; session-owned origins read only their own
environment, as above. Any other attached host may obtain any ordinary template's full contents, including a command
line and resume command it carries: an approved spawn that applies a template, or any spawn from a host the user lets
run commands without asking, delivers those contents to it. Until the read side is closed (TODO.md's
[Maybe later bucket](TODO.md#maybe-later)), templates are no place for secrets that must stay hidden from an attached
host. The fleet-wide session and host listings stay readable by every attached host except session-owned origins, which
see only their own environment. This acceptance is not a standing grant: it ends when reading other hosts' session and
template data is limited to explicitly trusted environments.

Confirmed 2026-09-28, under that same read-side acceptance: command lines are not secret from agents either. An agent
runs with the same account authority as its host's supervisor, so an agent may obtain any template's command line and
resume command (for example by applying it in a spawn) and any session's command line (for example by cloning that
session onto a host it can read). Until the read side is closed, neither templates nor session command lines are a place
for secrets. This ends with the same acceptance.

Do not add a way for one host to cause execution on another that does not go through the user's approval or that host's
setting, by analogy with the commands above or otherwise. Future agent-driven orchestration, such as setting up several
sessions on another host without a card for each, is wanted with an explicitly authorized launch policy; trusted
templates are a possible design, not a security property established for the current catalog.

### Client hardening

Confirmed 2026-09-28: the desktop app is held to a high bar. Hardening that narrows what a hypothetical flaw in it could
reach, or that keeps other software from passing for it, is wanted, as long as it stays proportionate.

### Losing the connection to the helm

Decided by the maintainer on 2026-11-12: the window and its helm behave as if joined by an infallible, ordered stream.
While the connection holds, the helm receives the window's commands in the order the user gave them, answers each one
promptly (a host that does not answer in time is an ordinary answer, such as that the host timed out), and every change
a command causes is already on screen when its answer arrives. In the maintainer's words: "We need an abstraction that
works as if there's an infallible, ordered stream between the UI and the Helm. If that is actually failing, that needs
some kind of UI reset ... so that we don't spread complexity all over the place."

When the connection does break, for whatever reason (the connection dropped, the machine slept, the helm stopped
answering), nothing tries to recover individual actions. The whole window resets instead, the same way however it broke:

- The window goes inert behind "Reconnecting to the helm…" and reconnects to the same helm, making at most one attempt
  at a time.
- Once reconnected, everything the window held from the helm is thrown away and rebuilt from what the helm reports now.
  Terminals re-attach and replay from their supervisors as on any reconnect.
- One generic notice covers the gap: "The connection to the helm was lost; something you did just before may or may not
  have happened." No action tracks its own fate across a reset, and there is no per-action report that an outcome is
  unknown. Work the helm already accepted carries on, since it is the helm's, and the session list shows its result.
- What survives a reset: this device's local preferences (sidebar width, theme, window size and the like) and the
  selected session, by id, if it still exists. Everything else starts over: open dialogs and menus, unsaved edits and
  half-typed launcher drafts included. Nothing is replayed, restored or merged back, and the user redoes what was lost.
- Duplicates around a reset are accepted. A launch sent just before the drop may or may not have happened; after the
  reset the user sees the real list, and launching again without looking can start a second session. The GUI keeps no
  launch request keys or retry rules to prevent that. Idempotency keys the command line and agents use are unaffected
  (Agent-spawned sessions).
- If the helm refuses the app's own credential, which takes something genuinely broken, the window says it cannot
  connect and offers Retry rather than reconnecting in a loop, without restarting the app.
- A reset never crashes the window or leaves it dead: it ends rebuilt, or on the cannot-connect state with Retry. The
  one exception is a helm that has ended inside the app: the app says its helm stopped and exits, since nothing restarts
  the helm inside a running app.

An action the user started is still never lost silently: while the connection holds it completes and reports its
outcome, and when the connection breaks first, the generic notice is its report. Terminal streams and file transfers
travel on their own connections. A dropped terminal stream or a failed transfer alone does not reset the window: the
stream reconnects on its own (Terminal experience), and a failed upload or download is that transfer's own visible
outcome. A transfer still in flight when the window resets is covered by the generic notice.

Decided 2026-10-02 and still in force: the desktop app never involves a user-visible credential. Its own credentials are
minted for each launch (see Security), so it never signs in.

### One GUI at a time

Decided by the maintainer on 2026-11-12, replacing the best-effort support for several GUIs at once confirmed
2026-10-01: a helm accepts one UI at a time, and enforces it. The UI's connection to the helm is its seat. A UI that
connects takes the seat, and the one holding it is displaced: the displaced window says that another window took over
and goes wholly inert, its terminals included. It does not reconnect on its own and none of its terminals re-attaches
until the user explicitly takes the seat back from that window, which displaces the other one in turn. Only the helm's
own word that another UI took the seat counts as a takeover; a connection that merely drops is a reset (Losing the
connection to the helm). Agent approvals follow the seat: a window holding it is an open Farhelm window for approval
requests, and a displaced or disconnected one is not (Agent-spawned sessions). Farhelm adds nothing to cope with several
UIs at once. If enforcing the seat ever turns complicated, the maintainer accepted deferring the enforcement, but not
the assumption of a single UI.

With the helm embedded in the app and its credential handed only to that app's window (see Security), a second UI cannot
arise today. The seat is enforced for a future remote helm, and the test harness exercises it.

The `farhelm` command line, and the agent skill through which agents act on the fleet, are a fully supported primary
surface alongside the GUI, including while a GUI is open (acting from inside a session needs a window open to approve
it, unless the host's setting says not to ask; see Agent-spawned sessions): operations they perform concurrently with a
GUI must behave correctly.

A GUI write must not undo a change an agent or the command line made while that view was open; the view's display may
lag until the next interaction.

Confirmed 2026-10-21, one exception: GUI saves of launch templates and of number-key bindings are last-write-wins
against agent and command-line edits too. The Templates editor saves its whole draft of a template, so saving any field
can put back a template field an agent changed while that editor was open (Concepts); a rename saves that draft under
the new name and then deletes the old one, and Undo of a delete restores the copy the Templates dialog had loaded rather
than the one deleted. A number-key change re-reads the bindings before it saves and then writes the whole map, so an
agent's or the command line's binding change that lands between that read and the write can be undone (Creation). That
overwriting is accepted.

Decided 2026-10-22: a GUI save that fails, for any reason, is reported to the user and then discarded. The GUI does not
keep the change it tried to save, does not treat it as the current state, and does not retry or replay it later; the
next view or save starts from what the helm reports, and the user redoes the change if they still want it. Reporting the
failure keeps the rule that an action is never lost silently (see Losing the connection to the helm).

Confirmed 2026-10-10: an action a user takes inside a sub-second window before the screen updates, or in a dialog hidden
behind another, may be carried out as delivered. A Cancel and a submit of the same dialog that arrive together before a
redraw may still submit, and confirming a dialog that another one covers confirms that dialog. Guards this spec names
explicitly, such as the approval cards' 700 ms pause and the preconditions destructive prompts carry, still hold.

### Remote input, session defaults, and availability

Agents may discover the helm catalog's template names and what each sets. Listing those in lookup suggestions is
explicitly allowed, not a confidentiality defect, and does not require a new discovery interface. Command lines are not
part of that listing, but they are not protected from agents either while the read-side acceptance in
[Local authority and trust between hosts](#local-authority-and-trust-between-hosts) lasts. Discovery also does not make
current templates trusted execution guardrails; the separate host-authority rules still apply.

Agent instructions must identify fleet session metadata as data, never instructions to follow; see
[Agent-spawned sessions](#agent-spawned-sessions) for the CLI contract. Merely echoing an agent's own input into its own
session terminal does not establish a security defect: the agent already controls that output. This does not excuse
unsafe rendering of remote input by the helm or GUI, secret disclosure, or violations of existing formatting contracts.

Only what the user explicitly selects in the GUI may affect the GUI's future defaults and suggestions: the remembered
permission mode and the recent setups the New dialog offers. Each is recorded from the user's own selection in the
request that succeeded, never from what a host replies or lists. A supervisor's create reply, the settings a plain
Replace copies from a listed row, and anything an agent creates do not qualify, because none of them is a choice the
user made in the GUI. A choice is still recorded only once its create succeeds; the host's success decides whether it is
recorded, never what.

Failures or malicious behavior from a remote host must not disrupt unrelated hosts or ordinary helm/GUI controls, apart
from the explicitly permitted operations above and the exceptions below. Supervisors need sensible recovery from
ordinary failures; they need not defend their availability against hostile processes with the same local account
authority. Choose proportionate remedies rather than assuming a quota or scheduling architecture is required.

A session reference binds the host entry's helm-assigned UUID and the supervisor's session ID. Nothing a host reports
can choose or reuse another entry's prefix, and a reference never reaches a different host entry. The helm resolves the
entry first and checks membership only there. Two entries reporting the same bare ID are two distinct sessions: both
appear in listings and counts, with separate viewer state and approval authority. An identity-less entry's live
membership is likewise checked only on that entry. Create, clone and interrupted-create replies can name only a session
on the entry that made them, even if cache publication fails or the connection ends after the reply.

Confirmed 2026-10-01: removing the host is the general remedy for a misbehaving host whose effect is limited to what the
helm and the GUI show. A host may crowd or clutter those views so that other sessions are hard to reach, for example by
reporting enough sessions that sort first to push every other host's sessions out of the merged all-hosts list (which is
capped), and so out of agents' fleet listings, which have no per-host option. In the GUI, per-host views still show the
others, routing by session id still reaches them, and removing the host restores the list. Farhelm is not required to
defend against that. What must still be prevented is a host compromising the helm, corrupting its own state, or
affecting other hosts' security or data.

Confirmed 2026-10-11: denial of service of the helm or the desktop app by a malicious or arbitrarily broken host is
accepted outright, whether or not Farhelm could avoid it. Farhelm adds no complexity to defend against those effects.
This does not accept a host corrupting the helm's own state or affecting other hosts' security or data. Existing
mitigations remain: a supervisor's "sessions changed" hints make the helm refresh that host at once, and each such
refresh makes every open client re-read, so the helm spaces the refreshes hints cause by the same minimum gap the
supervisor promises to keep between its hints, with at most one more pending. A host that hints without pause then costs
the helm no more than a busy honest one.

Confirmed 2026-10-10: two examples of accepted effects from a hostile or compromised host follow. Through the bundled
terminal library, escape sequences with huge repeat counts, link records or combining characters that pile up, and the
library's own caches can make a session's terminal slow or exhaust the viewer's memory; the remedy is upgrading the
library when upstream fixes such a problem, never a workaround in Farhelm. And a supervisor can fill the helm's log by
sending large unexpected messages, which Farhelm need not limit. Neither extends to a host gaining execution, reading
secrets, or changing what Farhelm does on other hosts.

### Ownership during cleanup and provisioning

Each terminal host, the process that keeps a terminal (see [Terminal experience](#terminal-experience)), and its socket
are an implementation detail, not an interface (the rule confirmed 2026-09-28 for the private tmux server earlier
releases used), and the product should keep them out of the user's way as far as practical. Interacting with one
directly is unsupported, whether the user does it by hand or a program running in a session does it: talking to one,
signalling it or removing its socket is outside every Farhelm guarantee, including cleanup on Stop, Restart, Delete and
tab close. Do not add code or complexity to detect, track, clean up after, or recover from that. Agents and terminal
tabs run without the `TMUX` and `TMUX_PANE` variables tmux sets in its panes, so a plain `tmux new-window` or
`tmux split-window` typed in a session behaves as it would over SSH, reaching the user's own tmux server or none. A
tab's shell starts without them, so a startup file that launches tmux when `TMUX` is unset does so in a tab as it would
in an SSH login. Shells an agent starts for its own work also run without them, as they would if the agent ran in a
terminal outside tmux. Farhelm's own operations must still handle their own objects going missing without crashing, and
the helm and GUI must still handle the resulting remote failures safely.

Session teardown covers ordinary agent descendants, including background servers. Detached services started by shell
initialization before the agent launches are outside that guarantee. Scope-only cleanup does not chase programs that
deliberately moved to a separate group or merely carry a session marker. Jobs a user-run tmux moves outside those groups
may survive too, while its server inside the scope ends; unscoped cleanup cannot find detached descendants whose
environment cannot be read or has been overwritten. See [Lifecycle operations](#lifecycle-operations).

Confirmed 2026-10-22: a per-user shared program, such as the user's own tmux server or an ssh ControlMaster, belongs to
whichever started it first. Agents and tabs run without `TMUX` (above), so a plain `tmux` there reaches the user's own
server, or starts it when none runs. One the agent started first ends with the agent's processes (Stop, Restart,
Replace, Delete); one a terminal tab started first ends with that tab (closing it, Stop, or Delete), as
[Session view](#session-view) describes. Within the cleanup limits above, it ends along with everything later run in it
or through it, including windows and work the user started in it from outside Farhelm, for example over an SSH login,
within the cleanup guarantee's limits on hosts without a usable systemd user manager. On scoped hosts, tmux pane jobs
moved outside the session's groups may survive (2026-11-10 decision). Farhelm cannot tell its own sessions' work in such
a server from the user's, and does not try; this is intended, not a defect. Confirmed 2026-10-23: the buttons named here
are examples; whatever tears down the agent or the tab as the user sees it, whichever button or flow does it, ends the
programs that agent or tab started first within these same cleanup limits.

After a failed Delete disconnects a viewer, either automatically reconnecting to a surviving terminal or remaining
detached until the user reconnects is explicitly acceptable. Choose the simpler implementation. Reviewers must not treat
either outcome alone as a bug or require additional recovery machinery to choose between them. The cleanup failure must
remain visible; recovery must not restart an agent or take control from another viewer. A retained session record does
not promise that cleanup preserved the terminal or its scrollback.

Provisioning may enforce permissions on directories dedicated to Farhelm: its private lib directory and the supervisor
state directory. It must preserve permissions on existing shared directories merely used to hold its executable or
service files, which are the systemd user-unit directory or the Mac's `~/Library/LaunchAgents` and, when the registered
binary lives outside the lib directory, that binary's own directory. A missing shared directory is created; an existing
one is left exactly as it is. If those permissions prevent installation, report the obstacle rather than silently
changing them. Trust in the helm does not authorize incidental changes to unrelated host configuration.

Farhelm is not designed for install directories that other local accounts can write to, whether provisioning's lib,
state, or binary directory on a host, a shared directory it writes into, or the standalone installer's install directory
(`~/.local/bin`). Keeping them writable only by the user is the user's responsibility, and Farhelm's installation,
update, recovery, and uninstall guarantees assume no other account can create or replace entries in them. A
group-writable or sticky shared directory is outside what the installer's lock, journal, and backup safeguards defend
against.

Confirmed 2026-10-10: Farhelm follows good permission practice for the files and directories it creates, for example
restricting a file right after creating it, whether or not this spec names that file. It does not spend significant
complexity on defence in depth against a user's broad umask, such as closing the brief window between creating a file
and restricting it.

On a Linux host provisioned from the hosts panel, the supervisor unit (`farhelm-supervisor.service`) has one owner. A
unit without `farhelm helm setup`'s managed-by marker belongs to provisioning: ADD and UPDATE may replace it, and
uninstall may remove it. A unit that carries the marker belongs to setup on that host: provisioning refuses to touch it,
both when planning and at the moment of writing or removing, and says setup manages it there, the same hand-off the
helm's own machine gets. A hand-written unit under that exact name on a host the user asks Farhelm to provision is the
user's to move aside first; provisioning does not try to tell it apart from its own.

A remote Mac uses the unmarked `io.farhelm.supervisor` launchd job instead. Its loaded plist must be the planned plist
before setup replaces a loaded job or uninstall stops it. The desktop-app refusal above keeps those operations from
sharing the app's installation. A hand-edited plist has the same best-effort status as a hand-edited systemd unit.

### First-class harnesses

Confirmed 2026-10-01: Claude Code and Codex are the first-class harnesses. A clear, definite gap in how Farhelm reads
their activity, such as a screen either agent draws in ordinary use that its reader classifies wrongly, is a defect and
gets fixed. Activity tracking, session tracking and similar integration features for every other harness (Goose, Pi,
OMP, Grok, and any added later) are intentionally partial; gaps there are expected and are not worth raising in code
review. That allowance stops at those features. It does not cover lost user work (resuming the wrong conversation
included), a session that fails to launch or run, an effect on other sessions, other hosts or the helm, or a security
consequence. This may change as those integrations mature.

The conversation-tracking limits under Durability and resume apply to every harness, first-class or not; a resume they
accept is not the lost user work this allowance excludes. They include the session's Resume target being replaced by a
Goose sub-agent, a nested Goose run, or a child Pi that loads Farhelm's reporter, or withdrawn by such a Pi.

### Supported user environments

Confirmed 2026-09-28: bash and zsh are the supported login shells, for agent launches and terminal tabs alike. Other
login shells are unsupported: Farhelm need not make them work, detect them, or refuse them with a tailored message. Bash
must also be available for command launches, their resume commands and agent wrapper prefixes even when the login shell
is zsh, and inside containers that run wrapped agents.

An agent harness version is supported until a newer release of that harness has been out for two weeks. Things move fast
in this space; Farhelm adds no code or complexity to support older versions.

A tmux the user runs in a session is one of their own programs, like any other they run there; Farhelm neither requires
nor looks for one on any machine.

Usernames of up to 20 characters must work on Linux and macOS with Farhelm's default state directory locations, on the
helm's machine and on every host, including wherever Farhelm places sockets or other files whose paths the system limits
in length. Beyond 20 characters, failures caused by those system limits are acceptable, but should say what limit was
hit rather than reporting an unrelated error.

Confirmed 2026-10-05: hosts that need an interactive approval (a second factor, say) for every new SSH login are
supported on a best-effort basis only, as an exception to the passwordless-SSH requirement, including when their SSH
server also allows only one session per connection. The helm keeps its supervisor connection and provisioning on two
separate shared SSH connections per host, so the two never compete for one connection's sessions, and such a host works
when the user starts both of the helm's shared-connection sockets for it by hand, for the same destination the host was
registered with, using the commands in **? → SSH needs 2FA?**, and the username is short enough for both sockets to fit
(the provisioning socket's limit is one character below the supervisor connection's). Only provisioning steps that run
one at a time are promised to work there. Restarting those connections when they drop is the user's job, and port
forwards set up by a connection the user started are theirs. Farhelm need not add complexity for such hosts. The
separate provisioning connection applies to every host: on an ordinary host it means one more SSH login when
provisioning starts with no provisioning connection open, which a user whose key needs a hardware touch or an agent
confirmation per login sees as one more prompt.

Confirmed 2026-10-14: for commands it offers for copying, Farhelm trusts the user's own configured host names, SSH
destinations and helm state directory path (including the working directory a relative one resolves against) not to
contain nonsensical characters that a shell or terminal interprets. Farhelm need not defend against such values in these
commands, nor detect or refuse them. This trust covers only that user configuration, never text a remote host supplies;
the escaping rule in [Local authority and trust between hosts](#local-authority-and-trust-between-hosts) still applies.

Confirmed 2026-10-10: a configuration Farhelm does not support is refused with a clear message rather than left to
undefined behavior, except where this section says otherwise (other login shells, filesystem aliasing outside the
home-folder exception below, out-of-band manipulation of managed checkouts and the trash, including uncommon filesystems
under them, a checkout root reachable from more than one host, the proxy case below, and hand-edited Farhelm service
units under [Supported host setup](#supported-host-setup)). An install whose `farhelm` program path contains `$` is
unsupported and refused, because systemd does not decode `$$` in a service's program position, so a service written for
that path could not start. The supervisor also refuses startup when the Farhelm program folder put on session `PATH`
contains `:`, naming the folder; the desktop app shows the same refusal before starting its supervisor, because a colon
would split that folder into unintended command-search directories.

Two host aliases that reach the same host, user and port through different `ProxyCommand` routes are unsupported. The
helm shares SSH connections by OpenSSH's connection hash, which does not include the proxy command, so such aliases may
share one connection, and an operation meant for one can reach the other machine. Farhelm does not look for this case.

Filesystem aliasing beyond symlinks is unsupported: bind mounts, and any similar mechanism that makes the same files or
folders appear at more than one path even after symlinks are resolved, including hard links. Farhelm compares canonical
paths (symlinks resolved, as the checkout rules above require) and treats each canonical path as the location it names.
A session that reaches a managed checkout, a working directory, or any other folder Farhelm tracks through such an alias
is not recognized as being there, and a file hard-linked between checkouts is not treated as shared. Farhelm need not
detect aliasing or add complexity to cope with it, with one narrow exception: uninstall from the hosts panel recognizes
the host account's home folder reached through a second path, such as a bind mount. It still refuses layouts it cannot
tell apart safely. Other aliases, including shared unit or installation folders, remain unsupported; sessions, working
directories and managed checkouts do not gain this exception.

Confirmed 2026-10-11: Farhelm has dedicated ownership of the managed checkouts it creates, their checkout roots and the
trash (`.farhelm-trash`) and reserved spare container (`.farhelm-spares`). Out-of-band manipulation of those paths is
unsupported, including restoring a filesystem snapshot over them, user-instigated kernel-level overrides, or using
uncommon filesystems such as FAT. Farhelm adds no complexity to detect or defend against these changes. The
foreign-replacement guard protects ordinary replacement, such as a folder removed and recreated or a re-clone; replacing
a folder inside the guard's check-then-move window is out of band too. The guard does not extend inside
`.farhelm-spares` (accepted 2026-10-21): anything in that container that Farhelm did not put there, including a folder
removed and recreated, is out of band, and Farhelm may adopt it and delete it. For example, a launch can take a replaced
spare as its own, and startup cleanup then deletes what the launch left behind. Everywhere else the guard applies as
stated. This acceptance does not extend to the state directory or installation, or change the ownership checks required
elsewhere: installer checksum matching, uninstall refusing unknown app-bundle entries, and the symlink and
canonical-path rules still apply. Ordinary reboots and remounts on supported filesystems such as btrfs, NFS and
overlayfs remain supported, as the managed-checkout identity rule above specifies.

Confirmed 2026-10-22: a checkout root reachable from more than one host (for example on a network mount two hosts both
see) is unsupported too. Each host's supervisor knows only its own sessions, so one host can archive, and later
permanently delete, a checkout a session on another host is working in. Farhelm does not detect a shared checkout root.

### Supported host setup

Confirmed 2026-09-28: for remote hosts, the supported, user-facing way to install and update a supervisor is the helm's
own setup and Update from the hosts panel, on the hosts that provisioning targets, and the way to remove one is the same
panel's uninstall. Every other way a host can end up with a supervisor works on a best-effort basis, mainly for people
working on Farhelm itself: a supervisor started by hand with `farhelm supervisor run`, one installed with `install.sh`,
one run by a unit the user wrote or changed with drop-ins, one at paths other than the layout setup installs, or a host
provisioning does not target. Do not spend significant complexity making setup or Update detect, adapt to, coexist with,
or preserve such setups; refusing with a clear message is enough. A remote Mac with the desktop app installed is refused
for setup, Update and uninstall as Topology specifies; it is not offered a manual-start workaround. Setup and Update may
treat the layout they install as their own, including re-applying their background-service settings and, on Linux, start
at boot and linger, on every run. The rules above about shared directories and unrelated host configuration still hold.

Confirmed 2026-10-14: editing a Farhelm service unit by hand voids Farhelm's guarantees about how operations that read
that unit identify its owning installation and treat its service. This applies to `farhelm helm setup` (including its
uninstall), `farhelm uninstall`, and the hosts panel's setup, Update and uninstall. An edited unit may be attributed to
the wrong installation, causing an operation to overwrite, stop or remove another installation's service. Farhelm need
not detect or defend against these edits; its ownership and refusal guarantees, including the service-type refusal
below, do not cover them. The guarantees for unedited Farhelm units remain unchanged. The same hand-edit exception
applies to a remote Mac's launchd plist.

Confirmed 2026-10-11: switching a remote host to `farhelm helm setup` or another best-effort setup while a panel setup
or Update is queued or running is best effort too. The panel run may act on the binary that setup now uses, and a
visible failure is enough; Farhelm need not re-check for such a switch mid-run.

Confirmed 2026-10-10: Farhelm's units use systemd's `simple` service type (the default, so a unit may also leave it
unset) with one start command. Setup and uninstall treat a Farhelm unit changed to any other type as one they do not
recognise, and refuse it, because Farhelm reads a unit's one start command to learn which program it runs, and only a
`oneshot` unit can list several. Such a change is a hand edit, so this refusal is best effort under the exception above:
an edit that hides the type or a start command from Farhelm's reading may not be refused.

A Linux machine running a helm remains supported, but the standalone installer temporarily does not cover that setup. On
a Mac the installer supplies the desktop app, which manages its own helm and local supervisor.

### Upgrade compatibility and client scale

Supporting a range of historical data schemas and hardening every upgrade/downgrade path are not current design goals.
During feature design, agents must alert the maintainer to potential loss of data or state before proceeding. The mere
fact that a schema version was bumped, so that older releases refuse the newer data, is not worth raising (the
maintainer, 2026-11-08). Compatibility is decided per feature; this is not blanket permission for destructive migrations
or ordinary runtime data loss. Revisit broader compatibility as the project matures, and record future breaking
transitions when there is an expectation of users beyond the maintainer.

The entry-bound session identity migration is accepted as one-way. Pins, seen state, notification marks and the shared
last selection survive only where migration can attribute their old bare ID to one cached entry; unattributable state
starts fresh. Agent IDs remembered before this update are refused except for the exact own-bare exception, with
`farhelm agent sessions` as the remedy. Agent host IDs and the meaning of discovery's caller and own-row markers change
with schema version 7. An approval card waiting during the update may get no answer. Existing sessions keep running, and
supervisors from older releases remain compatible through their normal update path.

The shared visit-history migration starts existing installations with no visits, while retaining their legacy last
selection. Until history exists, automatic choice treats that retained selection as one visit, subject to the same
listed-row and removed-session rules. Its normal selection save then records the visit. The new history column follows
the normal one-way schema migration.

Each session records whether the agent's terminal runs in tmux or in a small Farhelm process of its own that keeps that
one terminal alive without tmux. The supervisor database upgrade that adds this records every existing session's agent
terminal as a tmux one, and those sessions keep running. It is accepted as one-way, like other supervisor database
upgrades. A supervisor from before this change does not start on the upgraded database: it refuses it as having a newer
schema version than it understands, and that machine stays unreachable until a build at least as new runs there again.
That refusal is the point: such a supervisor would treat a session whose agent's terminal runs outside tmux as having
ended, record it as exited, and leave the process keeping that terminal running with nothing to end it. An older helm
from v0.31.0 on refuses to Update a host already on this change, naming the host's newer schema, rather than install a
supervisor that would refuse it. Going back otherwise needs a copy of the state directory taken before the upgrade,
restored together with the older version, losing whatever changed after the copy. A second supervisor database upgrade,
which changes nothing stored, comes with the first build that can be started with the outside-tmux terminal turned on.
Builds between the two record each session's terminal kind but do not yet handle an outside-tmux terminal everywhere, so
they would misread such a session the same way; the second upgrade makes them refuse the database in the same manner,
with the same consequences.

A third supervisor database upgrade stops storing tmux details for terminals that do not run in tmux: a session no
longer records a tmux session name (a tmux session's name follows from the session itself, so existing tmux sessions
keep running and are found as before), and where a session's terminal is recorded no longer borrows tmux's field for a
terminal outside tmux. Nothing a user sees changes, and every session survives the upgrade. It is accepted as one-way
with the same consequences: a supervisor from before it refuses the upgraded database as having a newer schema version
than it understands, so that machine stays unreachable until a build at least as new runs there again; an older helm
from v0.31.0 on refuses to Update a host already on it, naming the host's newer schema; and going back otherwise needs a
copy of the state directory taken before the upgrade, restored together with the older version. An upgrade that finds a
stored tmux session name other than the one the session's own identity gives (only a hand edit makes one) refuses,
naming the session (or the pending create request), rather than change it by a guess.

Every release has an **upgrade floor**, the oldest release it supports updating from, initially v0.23.0. Installations
at or above that floor must update through their normal path and come up on their existing state, and the new helm must
be able to update their supervisors. The periods between floor raises are **upgrade epochs**. Raise the floor only when
an update needs upgrade logic shipped by an earlier release: the new floor is the first release of the previous epoch,
so an installation can cross one epoch through its normal update path. The change that needs the raise changes the floor
in its own PR and names it in the PR and changelog fragment. Before a release proceeds, its release procedure shows the
maintainer the new floor, when it was promoted, its age and how many stable releases have shipped since; the maintainer
accepts it or chooses an earlier base. There is no automatic waiting threshold.

Below-floor installations are refused by the target release's installer, for both app-initiated and Terminal updates,
with an uninstall-then-reinstall remedy. The app recognizes that refusal and keeps a reinstall warning; older apps show
an ordinary install failure instead. The helm refuses Update of a below-floor supervisor, on either protocol path and
again when a retained plan executes, with the manual host recovery described in Topology. Development builds and
unparseable versions are not evidence that an installation or host is below the floor. Release candidates and dev
releases use ordinary SemVer precedence, so a floor release's own prereleases are below it.

The floor constrains update mechanics, not stored-data formats. Uninstall keeps local data, and the remote remedy lets
the user keep the host's data directory too; a reinstall does not promise that every feature can read that older data.
Whether a feature's own data survives remains decided per feature, as above.
