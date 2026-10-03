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
- **Session**: the unit of supervision. A session has a working directory, a launch, a title, and a live terminal. A
  session is agent-centric: it has one main agent terminal, plus optional additional terminal tabs (plain shells) that
  open in the same working directory. Session metadata — title, parent reference, stop annotations, captured
  conversation identity — is durable and lives with the session's supervisor, so it survives helm loss and
  re-registration; terminal contents live only as long as the host-side terminal does (see Terminal experience).
- **Launch**: what a session runs, as the user chose it in the launcher. Every launch has one of two launch kinds. An
  **agent launch** names an agent type Farhelm knows (Claude Code, Codex, and the other harnesses under Topology) plus
  that agent type's own choices, such as model and permissions, and nothing else: no custom arguments. Farhelm composes
  the command, and the command it uses to resume a conversation, from those choices when the session is created (and
  again on Restart with), and stores them; Replace and agent clone copy the stored commands rather than composing them
  anew, while Clone and Replace with compose from the choices. A **command launch** is a command line the user writes,
  with the user's own assertion of whether it runs without approval prompts (YOLO). A command launch may also declare
  that it runs a given agent type, which buys that agent type's status reading and conversation reporting, and, with a
  resume command the user writes, Resume (see Creation and Durability). The agent type is the field the two kinds share:
  required for an agent launch, optional for a command launch. This spec also calls an agent type a harness, after the
  vendor program behind it.
- **Launch template**: a named, partial set of launcher edits. Applying a template is exactly the same as making its
  edits by hand in the launcher, and nothing more: a template sets only the fields it contains, the result can be edited
  further before launching, and several templates can be applied one after another. A session records the launch that
  resulted, never which templates produced it, so editing or deleting a template cannot affect any session. Template
  edits are last-write-wins: two clients editing the same template at once is not a case Farhelm guards, because it is
  one user's rare action, and no optimistic-concurrency check on template writes is wanted.

## Topology

One control plane, two ways to face it:

1. **Native Mac app**: `farhelm-desktop`, a bare binary that opens the UI in a native window and starts the helm and a
   local supervisor beside it, so the Mac itself is a host. This is the setup when the Mac is your main machine. It
   bundles no tmux: it requires Homebrew's (at or above the version floor SPEC_impl.md's terminal-substrate section
   defines), finds it in the Homebrew prefixes itself because GUI apps do not inherit the shell `PATH`, and refuses to
   start — naming the binary, the version found, and the floor — when none acceptable exists; `FARHELM_TMUX` overrides
   the choice.
2. **Web interface**: a standalone helm — wherever it runs — serves a browser UI with the same capabilities, so a helm
   running on a Linux host is fully usable with nothing installed on the client machine. The native app's embedded helm
   is an internal part of the app: it serves only its own window, on a loopback port chosen for that launch, and serves
   no browser UI.

The two client forms have the same capabilities: terminal, attachments, lifecycle operations, host registration, and
launch-template management in the helm-owned catalog. They differ only in packaging. These are the only two faces the
product has — a standalone helm's web UI, or the local app's internal window. There is no remote native-app-to-helm
mode: a helm running on a Linux host is reached through its web UI, period.

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
registers the host — no separate host-side setup. The setup question names the concrete files and systemd unit it will
write, and that the supervisor runs persistently and starts at boot (or at login when lingering is refused). The user
may choose to skip this setup question for later additions from the same helm; that choice is shared by every client and
does not change discovery of an already-running supervisor. Passwordless SSH is the prerequisite on the HOST side, plus,
on the helm's own machine, access to the configured release source (GitHub by default) or a staged payload directory (a
developer-facing, best-effort option; see [Supported host setup](#supported-host-setup)). With that in place,
provisioning and everyday operation just work out of the box — reaching supervisors needs no port forwards, no opened
firewall ports, and no address configuration beyond the SSH destination. (The web UI's own loopback-plus-forward story
is separate; see Security.) Downloads go directly to that configured release-asset source — GitHub by default; neither
GitHub nor a configured mirror is a relay or rendezvous service for Farhelm sessions or connections, and the no-relay
guarantee below still holds. Nothing the supervisor does requires root: install, updates, and operation all happen as
the SSH user (user-level systemd, files in user-owned directories). If some optional step cannot be done without
privileges on a given host, provisioning says so and continues without it rather than escalating. Before touching the
host for initial setup, the helm states exactly what it is about to do in concrete terms — the files it will place and
where, the systemd units it will create, and that the supervisor will run persistently and start at boot — and proceeds
only on confirmation. Remote updates use the same one-use plan mechanism behind the same authority, but the user's
Update click is the authorization: no plan is shown for confirmation. Update never downgrades a host: it installs the
helm's own build, so a host whose supervisor reports a newer build is refused with both versions named, and the user
updates the helm instead. While an update is running, its host row stays folded and shows `updating…` until a progress
snapshot is available, then shows the current step, completed-step count, and client-measured elapsed time inline in the
status spot; hovering that status shows the count, the elapsed time, and every step of the run with its status and the
current one highlighted (in a window too short for the whole list, the list keeps the current step in view), since the
sidebar is too narrow to show a long step name, let alone the rest of the run. Reduced-motion settings replace the
animated indicator with a static one. Remote binary upload appears as a separate step before installation, so the user
can tell when network transfer is still underway. Success returns the status spot to its normal label and leaves the row
folded unless global details are on. Failure or an uncertain outcome still expands the row so its step list and
diagnostic remain visible. The Hosts header's `update all` action makes the same authorized Update request for each
remote host whose individual Update action is available at the click. A host already busy with setup or another run is
skipped rather than queued for a later update; the local host is excluded. Each remote host keeps its own validation,
progress, and result, so one failure does not hide or delay the others. V1 provisioning targets any Linux host with a
usable systemd user manager, on the two architectures cross-compiled supervisor binaries exist for. The distribution is
not a requirement — nothing provisioning does is distribution-specific — so the plan names whichever one it found rather
than refusing; CI exercises Ubuntu. Everything else — no usable systemd user manager, or an architecture with no payload
— falls back to the manual path (run the binary yourself), which always remains available, on the best-effort basis
described in [Supported host setup](#supported-host-setup).

Provisioning is idempotent and doubles as recovery: re-running it against an already-provisioned host — including from a
brand-new helm whose registry was lost — detects the existing supervisor and re-registers the host with all its sessions
intact. Losing the helm never strands a provisioned host.

Removing Farhelm from a remote host is a host action too, the uninstall item in that host's menu, with no command-line
counterpart. The helm plans the removal over SSH and shows it before touching the host: the supervisor's user service
and its unit file go, and so does Farhelm's private lib directory with the binary and any private tmux in it, each named
by its path on the host; the host's Farhelm data directory stays, named by its path, with a note that deleting it by
hand removes the data too. Nothing changes until the user confirms, and while the removal runs the host row shows its
progress the way an update's does, worded as uninstalling. Uninstall never stops or kills a session or terminal tab. It
refuses, naming them, while the host has any session that has not ended (an unknown status counts) or any open terminal
tab, and checks again at confirmation. It also refuses a host it cannot check because it is not connected, a host whose
supervisor unit `farhelm helm setup` manages there (that host's own `farhelm uninstall` removes it), and a supervisor
running from anywhere other than Farhelm's lib directory. Success removes the host from the list, cached sessions
included, as removing it does, and the window that confirmed says Farhelm was removed and where the data remains. A run
that fails partway keeps the host listed with its steps showing what is left, and choosing uninstall again continues
from there. Such a retry may go ahead without a connection only once the unit file is gone, since nothing can start the
supervisor again after that. Linger is left as it is. The helm's own machine has no uninstall in the panel: Farhelm
there is removed with `farhelm uninstall` (see [Uninstall scope and interaction](#uninstall-scope-and-interaction)).

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
host's identity or its sessions. Every host, the local one included, has a settings dialog in the GUI that holds its
destination (SSH hosts), its alias, and whether it starts YOLO sessions without asking. Every host asks before YOLO
launches until the user explicitly turns that off, including hosts that existed before the setting did. Adopting a new
identity for a host, described next, resets it to asking before YOLO launches: the setting is a judgment about the
install the user knew, and a new identity, whether a reinstall or a different machine, is one they have not judged. If a
destination turns out to present a different identity than recorded (a wiped and reinstalled host, a recycled address),
the helm says so and asks whether to adopt the new host or fix the destination — it never silently merges. An entry that
reaches a machine another entry already holds connects nothing: it says which entry holds the machine, by name, and asks
the user to remove that entry or change this one's destination and then press Retry. Farhelm never connects two entries
to one machine and never resolves this on its own. Last-known sessions of a host that is permanently gone are disposed
of by removing the host from the registry.

Exactly one helm runs at a time. Running several concurrently is unsupported in v1. The invariant supervisors enforce is
at most one attachment per session, last attach wins — so a second helm cannot corrupt a session, but it can seize one,
exactly like any other client taking control. "Exactly one" is an operating assumption, not something enforced; that
invariant is the backstop.

The helm is a plain command-line process too, with the same layering as supervisors: on Linux, v1 ships user-level
systemd units for it, so a reboot of the helm's machine brings the web UI back; on the Mac, the helm lives and dies with
the app.

Launch templates belong to the helm: one catalog applies to every host the helm manages, while a command a template
carries still has to exist on the host that runs it. Farhelm ships no built-in templates; the agent types themselves are
what a release supplies. Integrations are not user-authored: an agent type selects Farhelm's own status reading and
conversation-identity reporting for that harness, and a command launch with no declared agent type gets generic
treatment.

Cursor is an agent type launched as `cursor-agent`, and YOLO adds `--force`; Farhelm launches Cursor by that name, never
as the generic `agent`, which other tools also use. Its model is optional, with `auto`, `composer-2.5` and literal
custom IDs supported. Default permissions add no flag; YOLO preserves explicit Cursor denies. There is no separate
effort selector. Cursor uses generic activity status and has no conversation tracking, automatic Resume, configuration
editing, hooks or instruction injection. The launcher states that tracking and Resume are unsupported, so a Cursor
session cannot be restarted; Replace starts it over, and clone preserves launch intent. See
[Cursor](website/src/content/docs/docs/agents/cursor.md).

Grok is an agent type for the official `grok` CLI. A normal launch is `grok --no-leader`; YOLO adds `--always-approve`.
Farhelm exposes neither a Grok model picker nor an effort picker because those command-line contracts have not been
verified. Every generated fresh and resume command retains `--no-leader`: the shared leader is outside the tracked
process's ownership boundary, while a private leader still permits Grok's native subagents. Grok uses generic activity
status. The launch layer preserves its exact `grok --no-leader --resume <conversation-id>` argv, but it does not offer
Resume until the capture integration has verified an exact conversation. See
[Grok](website/src/content/docs/docs/agents/grok.md).

Muse support uses `muse` and `muse --yolo` with generic activity status. The yolo variant skips approval prompts and
sandboxing and trusts the workspace for the run. Muse-specific hooks, conversation capture/resume, and waiting-state
recognition are not implemented. Muse agent launches also offer a workspace-trust choice separate from tool permissions:
true adds `--trust-workspace` for that launch; false adds no trust flag. False does not undo trust already implied by
`--yolo` or vendor configuration, so a prompt is possible only when neither has granted trust.

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
level is not offered) and offers `yolo` (preselected, `--approval-mode yolo`) and `approve`
(`--approval-mode always-ask`); its `write` mode is not offered, and `smart approve`/`chat` are refused. OMP gets no
waiting-state recognition: an OMP approval prompt shows the generic running/idle status, a settled scope decision rather
than a detection gap. Provider capabilities may clamp or reject a requested effort.

Standard operation must never require falling back to SSH or a separate command line, with four v1 carve-outs:
transport, web-token bootstrap, bringing up the helm's own machine, and starting the v1 Mac supervisor by hand when a
Linux helm drives Mac agents. Reaching a remote helm's web UI takes a user-managed SSH port forward, and obtaining or
rotating that UI's token happens on the helm's machine (see Security) — accepted v1 friction, deliberately outside
"standard operation". Bringing up a Linux helm machine is `farhelm helm setup`, run once there: that machine's systemd
units are written by one owner rather than by whichever surface got there first, which is why the hosts panel refers to
it instead of installing a supervisor locally (see Topology). On provisionable hosts, install and updates are the helm's
job (over the user's SSH access); what may legitimately require manual host-side work is that same transport and token
pair, on hosts provisioning does not cover. Everything else — session operations, template management, directory
browsing — must work from the client. SSH otherwise remains an escape hatch, never a requirement.

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

Re-running the installer updates the installation. Its default is the latest stable release; `FARHELM_VERSION` selects a
specific release, including a prerelease. Updating the software preserves user data. Updating while Farhelm runs is
supported on the Mac: the running Farhelm keeps working on the version it started with, its sessions keep running, new
sessions start, and agents' `farhelm` commands and conversation tracking keep working; quitting and reopening Farhelm
finishes the update, after which sessions started before it use the new version too. An update of an installation
already in this layout, stopped at any point, leaves an app that launches either the old version or the new one.
Automatic updates are outside this initial scope, as are package-manager installations.

The installer intentionally trusts GitHub over TLS and the upstream repository: it downloads release archives and their
`SHA256SUMS` from the project's GitHub releases over HTTPS only (no plain-HTTP redirects) and checks archives against
those checksums, but it does not verify the release signature the helm checks when provisioning other hosts, because a
fresh machine has no pinned key or verifier to check it with. The helm's release mirror setting
(`FARHELM_RELEASE_BASE_URL`) has no effect on the installer; a mirror that is safe for the helm, which verifies
signatures, would not be safe for a download that does not.

Installation follows the same ownership rule as removal: a file is not destroyed merely because its name matches
something the installer writes. The installer changes `Farhelm.app` only when it can show it built it, and replaces
copies of `farhelm` and `farhelm-desktop` that an earlier layout put in `~/.local/bin` only when their checksums match
what that layout's ownership record says it put there. Anything else at `~/.local/bin/farhelm`, whether the user's own
file or a Farhelm from before the record existed, is kept under a visible name and reported, and the install proceeds;
no crash or interruption point may lose it.

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
`--yes` rather than proceeding implicitly. `--dry-run` reports the proposed actions and any blockers without changing
files, stopping processes, or disabling services.

Removal covers the recognized installer-created macOS app bundle and its terminal link, the installed CLI on Linux, and
Linux user services owned by `farhelm helm setup` for this installation. Recognize services through the existing setup
marker and the executable recorded in their unit files, and reuse setup's service-removal behavior. Those services are
stopped and disabled before their executables are removed. Operator-authored services and service drop-ins are not
deleted; retained integration files are reported. The user must stop services outside Farhelm's removal authority. Basic
uninstall does not analyze effective service overrides or inspect their processes.

Persistent user data is retained, including session history, attachments, credentials, the host registry, preferences,
logs, and cached payloads. Completion names the retained data locations; retaining data must not be presented as erasing
all traces of Farhelm. There is no `--purge` in this first version. Project directories, agent harness installations and
their own data, independently installed dependencies, and unrelated files are untouched. Uninstall does not remove
shared parent directories or change their permissions.

The command affects only the selected local installation. It does not contact registered hosts, uninstall remotely
provisioned supervisors (the hosts panel's uninstall does that; see [Topology](#topology)), or remove a separate
provisioning-owned local installation. Removing a helm does not stop sessions on its remote hosts.

### Operator prerequisites and failure behavior

Before uninstalling, the user must stop local sessions and their additional terminals, quit the desktop app, and stop
manually started Farhelm processes. Sessions may survive a supervisor exit, so quitting the app or supervisor alone is
not enough. A one-time stop/restart when upgrading to the first uninstall-capable release is acceptable.

Basic uninstall operates on files. It does not walk processes, discover runtime sockets, reconstruct session ownership,
or introduce a runtime registry. The one runtime fact it reads is whether the supervisor's and helm's state-directory
locks are held, on macOS, to keep an open desktop app from running against an installation being removed (see Concurrent
and interrupted runs); a held lock is reported as such, not as proof of what is running. It neither forcibly terminates
agents nor claims to prove that all processes have stopped. The command explains the stopping prerequisite before
removal, including with `--yes`.

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

Uninstall running at the same time as installation, updates, setup, desktop startup, or session creation falls under
[Concurrent and interrupted runs](#concurrent-and-interrupted-runs) like any other overlap: the outcome must be correct,
and refusing is acceptable. Uninstall takes the locks those operations already use (on macOS the installer's app-bundle
lock and the supervisor's and helm's state-directory locks; on Linux the installer's install-directory lock, and setup's
unit-directory lock when setup's services or unit directory exist) without waiting, after confirmation, and then
re-checks what it is about to remove under them; a lock already held, or a plan that changed since confirmation, refuses
with nothing removed. An install or update that starts while uninstall holds the locks refuses in its own way, and so
does a setup whenever uninstall holds setup's lock.

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
  on creation — working directory, command and resume command, title, and the rest of the launch — must fit in 64 KiB
  between them, and a rename's title alone is held to that same bound. Renaming has no conflict detection: two renames
  of one session both succeed, and the later write is the title that sticks.
- Launch composer: New opens a dialog on the agent launch kind with no selected agent type. The two launch kinds are
  shown as two tabs, agent and command, each showing only its own fields; switching tabs keeps each tab's draft. Codex,
  Claude, Muse, Cursor, Grok, Goose, Pi, OpenCode, and OMP agent launches carry an agent type plus model, effort,
  permission, and workspace-trust choices where that agent type supports them, and nothing else: an agent launch takes
  no custom arguments, and a launch that needs them is a command launch; visible permission vocabulary is `default`,
  `approve`, `smart approve`, `chat`, and `yolo`. Absent optional choices mean the selected harness's defaults and omit
  their flags, except an omitted Pi, OpenCode, OMP, or Goose permission means its YOLO mode. Model selection is optional
  for every harness; OpenCode, Cursor, and Grok offer no effort choice, and Grok accepts no model choice. The helm owns
  the released model catalog and validates every agent-launch choice, so the browser never turns a model identifier into
  an argv fragment. Typing a model never changes a selected harness: the typed id is read as that harness spells it (for
  OpenCode, a bare Zen name means `opencode/<model>`), a model only other harnesses offer is refused with a message
  naming them, and choosing another harness's model from the full model list switches the harness on purpose. With no
  harness selected yet, a typed known model fills in its owning harness (a bare `gpt-6-luna` picks Codex); a custom
  model needs an explicit harness, and an id several harnesses offer asks for one. Replacing a harness clears
  incompatible choices and a YOLO permission that was the previous harness's default. An invalid combination cannot
  launch. New normally preselects no harness or model. The permissions mode remembers the last successful agent launch,
  helm-wide across every client, except that a harness's default or forced YOLO clears that memory instead of
  preselecting YOLO for another harness. Explicit YOLO on a harness with a non-YOLO default is remembered; an explicit
  workspace-trust choice on Codex, Muse, or Pi is remembered separately after a successful user launch. `trust:true` and
  `trust:false` are single search actions on those harnesses. Codex true and false set that launch's exact working
  directory to `trusted` and `untrusted` through its per-run project configuration; a fresh checkout's path is filled
  only after the supervisor has resolved it. Muse true uses `--trust-workspace`; Pi true and false use `--approve` and
  `--no-approve` respectively. Each setting applies to one launch and never writes vendor trust state; Muse false adds
  no flag and cannot revoke trust from YOLO or vendor settings. Without a choice, the harness retains its own trust
  behavior; Codex, Muse, and Claude may still ask for directory trust. Farhelm does not silently answer their prompts.
  Claude, Goose, OMP, and Cursor have no supported interactive workspace-trust switch. "reset choices" returns both
  segments to their remembered values rather than to harness defaults, and a recent-setup row's own saved choice
  overrides it when used. The launch-composer search matches harnesses, launch templates (`tl:name`), models scoped by
  the chosen harness, effort words offered by that harness and model, `yolo` plus the `perms:yolo` and `perms:default`
  permission actions, supported trust actions, host and name actions, folders, and recent setups. Permission actions are
  offered only when the selected harness can represent them; `perms:default` is not offered when the harness's omitted
  mode is YOLO. `name:foo` applies the entire value as the session name. `host:foo` filters host choices, and
  `host:local` selects the helm-local host even if it has an alias. The default local host label in the GUI is
  `local (this machine)`. Accepting a result applies it and clears the box while keeping focus there. Enter on an empty
  box launches only a complete, valid selection through the ordinary Launch path; Enter on a non-empty query with no
  result never launches, and Escape closes the result list without clearing the query, so Enter after Escape does
  nothing until the box is emptied.
- Command launch: a command launch takes a command line, which may reference the session's working directory as `{cwd}`
  as a whole argument, and a required YOLO assertion: the user states whether the command runs without approval prompts,
  and Farhelm believes the statement. Farhelm never reads a command line to decide whether it is YOLO, what agent it
  runs, or how to resume it. A command launch may optionally declare the agent type it runs. A declared agent type
  requires `{farhelm_args}` as a whole argument exactly once in the command, which marks where Farhelm places that agent
  type's own launch arguments (the conversation-reporting hook and the instructions pointer, see Durability); it is
  replaced by nothing for an agent type that takes no such arguments, so the rule never depends on which agent type was
  declared. `{farhelm_args}` without a declared agent type is refused. Farhelm passes whatever turns its integration on
  as arguments, never through the environment: every process the agent starts inherits the environment, including
  another agent of the same type that it shells out to, so a hook carried there would reach runs that are not the
  session's. The environment carries only settings that Farhelm's own reporter reads once an argument has turned it on,
  set on the launched process directly and never written into the command. Declaring an agent type gives the session
  that agent type's status reading and conversation reporting. It also allows opting into Resume, which requires a
  resume command containing `{conversation}` and `{farhelm_args}`, each as a whole argument exactly once; Farhelm fills
  those and `{cwd}` in and otherwise runs the resume command as written. `{conversation}` in the start command is
  refused. A command launch with no declared agent type, or one that does not opt into Resume, can never be restarted
  (see Lifecycle operations). The YOLO assertion covers the resume command too. Search while the command launch kind is
  active still offers agent types, models, and recent setups, and accepting one switches to the agent launch kind;
  accepting a folder, host, or name keeps the current launch kind, and a template keeps it unless the template sets one.
  The declared agent type is chosen in the command tab's own field, not through search.
- Launch templates: `tl:name` in the search box offers templates by name, and accepting one applies it. Applying a
  template makes the edits it contains, in the same way and with the same effects as making them by hand, in a fixed
  order: launch kind first, then agent type, then every other field. It can switch the launch kind, and choosing an
  agent type clears the choices incompatible with it exactly as picking that agent type by hand does. A template's
  agent-launch fields and command-launch fields apply to the launch kind active once its own launch kind, if any, is
  applied. A template may set any launcher field: launch kind, agent type, model, effort, permissions, workspace trust,
  command, YOLO assertion, resume command, host, destination (a folder or a fresh GitHub checkout), and session name.
  Fields a template leaves out keep whatever the launcher already holds, so templates stack: applying `my-codex` and
  then `myproject-webbuilder` applies both, the later one winning where they overlap. A template whose field does not
  apply to what the launcher holds when it is applied, such as a model while the launch kind is command or a model the
  chosen agent type does not offer, is refused with a message naming the field, and nothing from it is applied. So is a
  template that sets a field the current dialog holds fixed, such as the host in Replace with; Restart with does not
  offer templates. A template names its host by the host's recorded install identity, exactly like the host default
  below, so a registry row retargeted to another install makes the template's host field inapplicable rather than
  silently aiming at the successor. A launch made after applying templates is, for remembered defaults, recent setups,
  and fresh-checkout previews, exactly the launch the same hand edits would have made: the resulting choices count as
  the user's explicit selection, and a template that changes host, installation, destination, title, or agent
  invalidates a pending checkout preview as the equivalent hand edit does. Because a template pins only what it
  contains, a field the launch requires and no template set still has to be filled in by hand; that friction is
  accepted. Templates are created, edited, and deleted in a Templates panel whose control sits beside New; its form
  offers every launcher field, each optional. Template names are unique.
- Recent setups: the helm remembers bounded successful agent-launch combinations and used folders per target-install
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
both do), add the prompt to the composed command — no readiness detection needed. Injecting a prompt into the terminal
of an already-running agent requires reliably detecting that it is ready for input, which is the same hard problem as
status detection; that route is only for agents without the argv affordance. Either way it is an additive change (an
optional field on create/spawn), which is why v1 can skip it safely.

A custom model id is typed into the model field and applied with Enter; it needs a chosen harness. The optional session
name sits in the destination block under host and folder, visible without any disclosure. Launch and Cancel are the
first controls in the launcher, above everything else, and Launch's label names the chosen agent type (or command),
host, and folder. Project registration (associating metadata with a directory) is optional convenience and never a
prerequisite.

Creation guards against accidental double submission (a double-click, a retry after a timeout): one intended create
yields one session or a clear error, never two silently. Deliberately creating several sessions with identical
parameters — same directory, same launch — is a sanctioned workflow, not a duplicate to be suppressed.

A YOLO launch on a host that asks before YOLO launches (see the host settings under Topology) needs an explicit
confirmation. An agent launch counts as YOLO when its effective permission is YOLO, including omitted permissions on Pi,
OpenCode, OMP and Goose, and is classified exactly. A command launch counts as YOLO when its YOLO assertion says so, and
only then: Farhelm does not parse command lines to second-guess the assertion, for the user or for an agent. Confirmed
2026-10-03: an agent can therefore start a YOLO command on such a host by asserting that it is not YOLO. This is
accepted until the permission prompts for actions requested through the `farhelm` CLI land (TODO.md); with them, an
agent's command launch on such a host asks whatever its assertion says. The helm enforces the confirmation, so no client
can skip it: every create, clone, replace, replace with, and restart with that reaches it on such a host without the
override is refused before any supervisor is contacted, and nothing is started. The GUI answers that refusal with a
prominent confirmation, shown with the control or surface that started the launch and scrolled into view, that names the
host, says what YOLO means and why this launch is one (YOLO was chosen where the agent type offers other modes, the
agent type has no mode with approval prompts, or the command was asserted to be YOLO), and retries with the override
only when the user confirms. Besides a one-off confirmation it offers to stop asking for that host: that answer first
sets the host to start YOLO sessions without asking, exactly as its settings would, and then retries with the override;
if changing that setting fails, nothing is started and the confirmation stays up with the reason.
`farhelm agent create`, `farhelm agent clone`, and `farhelm spawn` other than `--inherit-agent` take `--confirm-yolo` as
the override (its earlier name, `--allow-yolo-on-sensitive-host`, is still accepted but no longer shown in help). A
plain restart relaunches the session's own stored launch and is not asked again, and so does
`farhelm spawn --inherit-agent`, which reuses the asking session's launch and is answered by its own supervisor with no
helm involved.

Failures split cleanly in two: precondition failures (nonexistent directory, unknown template, unreachable host) fail
the create with a visible error and no session; launch failures of a session that was successfully created surface on
the session itself — **error** when the agent process could not be started at all (exec failure, command not found),
**exited** when it started and then ended, however quickly, with its exit code visible.

### Fresh GitHub checkouts

Selecting `gh:owner/repo` in the composer explicitly requests a new checkout on the selected host. It works with both
launch kinds. Selecting a folder or editing the ordinary directory returns to an existing-directory launch without
changing the agent choice. Ordinary Clone and Replace start from the source session's actual directory; a new checkout
requires an explicit repository selection, a saved repository setup, or a template that sets one.

The helm owns a working-copy root and optional post-clone command, globally with per-host overrides. There is no default
root and no configuration GUI. The root must already exist on the target host; `~` expands there, using the supervisor's
captured home. Clearing an override restores inheritance; an empty hook override disables the inherited hook.
Configuration changes affect new attempts, not an already accepted attempt or its retries.

Farhelm is not designed for a working-copy root that other local accounts can write to, such as a group-shared folder or
a sticky, `/tmp`-style directory. Keeping the root, and so its checkouts and the archive folder inside it, in a location
only the user can write is the user's responsibility. Farhelm does not check the root's owner or mode, and the ownership
and archive promises in this section assume no other account can create, replace, or take over entries in it.

Before Launch becomes available, the composer shows the exact host and path. Preview creates no directory and reserves
nothing. An unnamed checkout uses the lowest available positive `repo-N` and that basename as its session title. An
explicit title keeps its printable display spelling while the path uses `repo-` followed by its lowercase ASCII slug;
runs outside letters and digits become hyphens. A title already starting with `repo-` does not repeat that prefix. An
empty slug or a component over 200 bytes is refused, and so is a checkout path too long to archive later (the archive
location adds up to 82 bytes to it, and the whole must stay within the system's path limit: 4096 bytes on Linux, 1024 on
macOS). Existing files, directories and symlinks all occupy a name. If another create wins the displayed path, Launch
reports the conflict and obtains a new preview; it never submits automatically or silently chooses another directory. An
explicit name remains a conflict rather than gaining a suffix. A title that Clone or Replace with copied from the source
and the user has not edited is not an explicit name once a fresh checkout is the destination: the session is unnamed, so
it gets the lowest available `repo-N` rather than showing the ignored copy. Whenever a checkout launch is unnamed, the
name field shows the `repo-N` it will get as placeholder text. This includes a source renamed after creation and an
ordinary session cloned into a checkout; both get `repo-N` rather than a name derived from the copied title. Choosing an
existing folder again restores the copied title.

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
session and plan; the directory remains untouched for manual inspection, and Delete's result tells the user so and names
the preserved path, as below.

Farhelm identifies a directory it created by its inode number plus its creation time, and uses the device number only
where no creation time is available. On supported setups such as btrfs subvolumes (Fedora's default `/home`), NFS,
overlayfs and some device-mapper configurations, the device number is assigned when the filesystem is mounted and can
change across an ordinary reboot or remount while the folder is untouched. A check that compares device numbers would
then refuse the user's own folder as replaced and permanently break the operation it guards, while inode plus creation
time still detects a folder actually replaced at the same path, which is what these checks exist for. Every check of
such a directory against an identity recorded earlier follows this rule rather than comparing device numbers.

Ownership follows use, not the lifetime of the session that first requested the checkout. Ordinary sessions in a managed
directory or its canonical subdirectories also retain references, including references to managed ancestors. Stopped,
exited, and errored sessions still count. Delete releases its reference; only the final reference causes the recorded
checkout to move into `farhelm-archived-working-copies` under its original root, using its original basename plus a
timestamp and collision handling. This is a no-overwrite move, never recursive deletion or a cross-device copy fallback.
A foreign object replacing the recorded path must remain untouched.

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
`trust:`, `folder:`, `recent:`, `tl:` and `gh:` labels offer or filter their respective actions, case-insensitively;
unknown labels and colons inside model IDs retain ordinary meaning. Accepting a name or host action clears search
without launching. Invalid `gh:` input cannot launch a hidden existing directory. Host, installation, destination,
title, agent choice and observed configuration changes invalidate an undispatched preview. Late responses cannot restore
its authority or steal focus. An already ambiguous submission remains bound to its original request.

### Lifecycle operations

The client supports: create, open, rename, restart, restart with, clone, replace, replace with, stop, delete.

A list rename opens in a modal editor owned by the list rather than by the row-actions popup. Listing updates may
reorder, filter, or temporarily fail without moving its textarea, so its draft, selection, composition, and source
session identity stay intact. A successful rename closes that editor; a refusal leaves its draft and error visible for
correction. A complete, authoritative listing that no longer contains the source disables submission without discarding
the draft, because a filtered, truncated, failed, or stale listing is not proof that the session disappeared.

- **Stop** terminates the agent and its entire process tree — MCP servers, dev servers, and other descendants included.
  Terminal tabs keep running, and the session remains with its terminal still viewable.
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
  started working after the user clicked is refused rather than stopped unconfirmed, and the next attempt asks. Replace
  keeps its confirmation whatever the agent is doing. Restart reuses the session's terminal when it still exists —
  whatever scrollback the terminal itself retained is still there — and creates a fresh one when the terminal no longer
  exists, such as after a reboot. Restart does NOT preserve the previous run's last visible screen: the pane is blank
  until the new agent draws, and a full-screen program's final frame (which was never in scrollback to begin with) is
  gone. Losing it is accepted. Farhelm must never capture a terminal's screen and paint it back into a relaunched
  terminal ahead of the new process — a frame with no process behind it looks live, accepts typing, and is overwritten
  when the real program draws, which is worse than blank. Restart touches the agent terminal only; terminal tabs are
  unaffected. Restart may carry changed launch settings (Restart with, below); the launch kind, agent type, host, and
  working directory remain fixed. If a harness rejects a changed model or other setting while resuming, that is an
  ordinary launch failure; restart again with settings the harness accepts.
- **Restart with** opens a dialog for changing the launch before resuming the session's own conversation: the model,
  effort, permissions, or workspace trust of an agent launch, or the command, resume command, and YOLO assertion of a
  command launch. The launch kind, agent type, host, and folder stay fixed; Replace with can change the harness or
  folder, and Clone can change the host. The dialog shows the current settings and marks edited fields, and its primary
  action is inactive until a setting changes. A working agent is stopped first with the user's confirmation on that
  action; a live agent in any other status is stopped first without it, as for Restart. The edited launch is validated
  exactly as a create validates it before anything is stopped, so an invalid edit leaves the agent running and the
  dialog open with the reason. A refusal leaves the dialog and its edits visible with the reason. Restart with runs the
  resume command, so an edited start command of a command launch takes effect the next time it is cloned or replaced.
  This action is available exactly when Restart is. Its header button remains visible but greyed out otherwise, with a
  hover tooltip and accessible description explaining why.
- **Clone** opens an ordinary, editable create form pre-filled from an existing session's host, working directory,
  title, and launch — the fresh-conversation counterpart to restart's resumed one. The source session is untouched:
  cloning starts a brand-new, independent create through the same form and the same confirmation described under
  Creation and identity above, so every field can be edited before submitting and the request can be cancelled like any
  other create. The source's stored launch is carried into the launcher verbatim: an agent launch's choices including
  omitted default fields, never rediscovered by parsing its composed command, or a command launch's command, YOLO
  assertion, declared agent type, and resume command. Cloning does not deduplicate titles — a duplicate is allowed, the
  same as any other create.
- **Replace** creates a new session — new id, fresh conversation, same host, working directory, title, and launch,
  copied exactly as stored — and then DELETES the source. For a command launch, "fresh conversation" means only that
  Farhelm starts the stored command again rather than a resume; a command that itself continues a conversation, such as
  `claude --continue`, does so. Confirmed directly from the row menu, with one inline confirmation and nothing to edit
  first, since the whole point is the same settings. Contrast restart, which keeps the session's own id and its
  conversation: restart continues a session, replace starts one over under the same settings. If the create fails, the
  source is untouched. If the create succeeds and the removal that follows fails, the reply names both sessions; whether
  the source is still there depends on how the removal failed, and the user checks or removes it by hand. Replace is
  offered wherever clone is offered.
- **Replace with** opens the same editable create form clone opens, pre-filled the same way clone pre-fills it, so every
  field can be edited before launching — the key use is starting an equivalent session on a different harness or effort.
  Launching creates the new session and then deletes the source, with exactly Replace's create-then-delete contract and
  failure reporting (the same asymmetry: an untouched source on a failed create, both ids named on a failed removal).
  Unlike clone, it keeps the source's own host — clone is the way to start a session on a different host. Offered
  wherever clone and replace are offered. Clone, replace with, and New are one launcher — same layout, same controls,
  same search, same validation — differing only in what is pre-filled when they open and in what launching does (create;
  create then delete the source). Its launch button is the confirmation of that delete: while the source has anything
  alive, the launcher shows Replace's warning about the source beside it, following the source's state as the client
  sees it while the launcher stays open, and launching carries the precondition matching what the launcher showed at the
  click (see the confirmation rule below). A source that has more alive by then, such as one restarted while the
  launcher was open, is kept: the new session is still created first, with no liveness check before it, and the user
  gets Replace's both-sessions-exist error.
- **Delete** removes the session and its stored state, in any state, terminating the agent and tabs if running — with
  confirmation that says so when anything is still alive. Deletion may make partial progress before failing, including
  removing attachment files while retaining the session row for retry. There is no rollback guarantee. Report the
  failure visibly and allow a later Delete to finish cleanup; a retained row does not mean previously removed state has
  been restored. Stopping a live agent can take a few seconds, since it gets its own chance to exit first, and a
  session's row stays until the supervisor confirms the whole process tree is gone. From the moment a delete is
  committed until that answer arrives, the client that started the delete shows it prominently, as an expected wait
  rather than an error: the row shows a spinner and what is happening in place of its title (stopping the agent,
  stopping its terminal tabs when only those are alive, or, when nothing was alive, deleting), the open session's header
  shows the same in place of its actions, and a scrim over its terminal says it again. The terminal's own "detached"
  notice, which the delete itself causes, is held back meanwhile; if the delete fails, the row and header come back with
  the refusal shown, and a held notice appears.

Confirmed 2026-10-01: a destructive confirmation authorizes only what the prompt the user answered said would happen.
This covers every destructive confirmation (Delete, Restart, Restart with, Replace and Replace with), including a prompt
that rewords itself while it is open because the session changed underneath it: the answer applies to the wording on
screen at the click, never to a state the user was not shown. The request carries the matching precondition, and the
supervisor refuses, at the moment it acts, when the session has more alive than that: a Delete or Replace whose prompt
said nothing was alive (or that asked nothing, because the session showed nothing alive) is refused if the agent or any
terminal tab is running; one whose prompt warned only about open terminal tabs is refused if the agent is running again;
a Restart whose prompt did not say it stops a running agent is refused if the agent is working (as for a Restart with no
prompt, an idle, waiting or unknown agent is still stopped unasked). A refusal deletes, replaces or stops nothing (a
Replace has already created its new session, and reports both, as above), and the next attempt asks again from the
current state. These races are reachable in ordinary use because the command line and agents act on sessions while a GUI
is open (see One GUI at a time). Accepted: a terminal tab opened between a tabs-only prompt and the click is closed with
the others; the prompt need not name the exact tabs it showed.

Process-tree ownership is session-wide. Restart reaps any leftover descendants of the prior run before relaunching —
never alongside them. Stop and delete reap everything the agent started. An agent exiting on its own does not trigger a
hunt for daemonized survivors; the session's next restart or its teardown does. Operations that need the working
directory — restart, opening a terminal tab — fail with a clear error naming the directory if it has vanished since
creation; the session itself remains, and delete still works.

This cleanup covers ordinary agent descendants, including accidentally daemonized processes, rather than hostile
same-account processes deliberately escaping cleanup. On a host without a usable systemd user manager (macOS, or a Linux
host running the supervisor by hand where that manager is missing or broken), there is no cgroup to contain a detached
descendant, and the guarantee narrows to the processes Farhelm can still identify: those still descended from the
session's terminal, and those whose environment marker it can still read. Confirmed 2026-09-28: a detached process that
hides or overwrites its environment survives stop and delete there. That covers a process whose environment the system
withholds from its own user, such as a non-dumpable `ssh-agent` or a setuid program, and a daemon that rewrites its
process title, which overwrites the environment the marker is read from (nginx with its default settings, or Postgres
started through `pg_ctl`, for example). A server that stays in the foreground under the agent remains in the terminal's
process tree and is still reaped. Likewise, with no cgroup to consult, a create retried after a supervisor crash that
also lost the session's tmux may launch again while detached processes from the first attempt keep running. macOS offers
no cgroup equivalent that an unprivileged process can use, so these are accepted limits of such hosts, not defects to
engineer around. The same limit applies to a terminal tab's processes when the tab is closed, reaped, or deleted.
Detached services started by shell initialization before the agent launches are outside the cleanup guarantee: they may
serve the user's login environment beyond this session.

Confirmed 2026-09-28: when Farhelm cannot confirm that the cleanup an operation needs has finished, the operation fails
visibly instead of reporting success or carrying on. Restart does not relaunch, and Stop, Delete and closing a tab
report the failure; a later attempt may retry the cleanup. "Confirmed" is judged by every mechanism the processes in
question were placed under: when a launch or tab ran in a systemd scope, the scope must be confirmed gone, even if the
portable sweep found nothing, because the scope can hold processes the sweep cannot see. A current belief that the host
has no usable systemd user manager does not excuse skipping a scope that such a launch or tab may have. A process
Farhelm tried to reap but could not examine counts as unconfirmed, never as gone. The accepted limits of hosts without a
usable user manager described above are not unconfirmed cleanup: processes Farhelm has no way to identify do not make an
operation fail.

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
which kills that shell and its processes — that is the whole per-tab operation set in v1. A tab's processes include
whatever its shell's startup files start: containment begins before those files run, so a shared service (an
`ssh-agent`, an editor daemon, a detached personal tmux server) that a tab's startup files are the first to start
belongs to that tab and dies when the tab is closed, reaped, or deleted with its session (within the cleanup guarantee's
limits on hosts without a usable systemd user manager; see above). This is deliberately unlike the agent launch, which
leaves detached services started by shell initialization before the agent outside its cleanup guarantee (see above),
though cleanup may still reach one that stays in the agent's process tree; since the agent's launch has normally already
run the same startup files, a tab usually finds such a service running and only reuses it. A tab whose process exits on
its own is reaped automatically and silently: the tab disappears as if closed, its dead pane's scrollback is discarded,
and no notice or exit code is shown. This is deliberately NOT the agent terminal's contract — an exited agent stays
viewable with its scrollback — because a tab's shell exiting is the user being done with the tab. A shell that dies
before the tab's open completes still refuses the open loudly, with the shell's last words as the error.

When a session's terminal contents no longer exist on a reachable host after a reboot, opening it shows the session's
metadata and says why there is no terminal, rather than an empty pane.

### Session list

One flat list across all registered hosts, with an always-visible host selector. It starts at `ALL`; `This machine`
means the registered local host, and configured remote hosts follow in registry order, including unavailable hosts whose
last-known sessions remain useful. Choosing a host sends a server-side query immediately and is not persisted. The list
can be ordered independently by most recent activity, by creation time, or by title; the order someone picks is
remembered by the helm as one preference shared by every client, together with the last-selected session and compact-row
choice, and most recent activity is what a client shows until someone picks otherwise. The same shared preference row
carries whether host setup or host removal confirmation should be skipped after an explicit permanent answer; a client
that has already loaded its preferences keeps its previous behavior until it reloads. The gear immediately to the right
of the sidebar version opens a settings dialog with exactly two checkboxes: `set up new hosts without asking` and
`remove hosts without asking`, ticked when the respective confirmation is skipped. Unticking one restores that
confirmation; ticking one makes the same choice as the host dialog's permanent answer. Changes take effect immediately
in the current client. Each checkbox explains the current behavior, and each host dialog's permanent answer points to
the gear as the place to undo it. These choices apply to every host and client of this helm; existing controls with a
natural place in the main UI stay there. No client keeps its own copy: every client reads the helm's preference once
after authenticating and writes it on change, so a browser tab and the desktop app open in the same order and on the
same session. Per-client persistence — browser storage, a desktop state file, anything that lets two clients remember
different answers — is not wanted for these shared preferences (terminal text size, by contrast, is deliberately per
device; see Terminal experience). A client that asks the helm for no particular order gets creation time. No mandatory
hierarchy. Sessions may carry an optional parent reference usable by the API, but parentage does not nest the list and
implies nothing about VCS state. Parent tracking is not comprehensive: `farhelm spawn --parent` can record it, while
`farhelm agent create` and `clone` need not record the asking session.

The option labelled most recent activity sorts connected running and waiting sessions first, then every other session —
idle, unclassified, ended, and anything on an unreachable host. Inside each group the order is the most recent observed
START of a work burst: a session promotes when it moves from known idle or waiting into running (both idle-to-running
and waiting-to-running count) and when it starts waiting on the user, since a question for the user is work the user
should see; later output inside that burst, and a question that stays on screen, leave the position alone. Only a change
the supervisor itself observed between two of its own looks counts: the first look after a supervisor restart, a session
restart, or a recovery from failed screen captures only establishes where the session stands. Moving between the groups
is what idling and completion do — a finished session drops below still-running work without its burst key moving, and a
session returning to running rejoins the first group on that same key. Creation time is the stable fallback where an
older supervisor has no work-start observation. The last-activity time shown in the row and the seen/unseen comparison
stay independent of the order: they describe when the agent was last working (see Status) and neither moves a row. The
grouping and the key are authoritative helm and supervisor data, so every client agrees without keeping a private rank.

A client holds back reordering while the pointer is over the list, so a row does not move out from under the pointer on
its way to a click. The hold is display-only and remembers nothing: the helm's order stays authoritative, and the client
shows it again as soon as the pointer leaves the list or has been completely still over it for 5 seconds (a scroll
counts as movement). During a hold, rows keep updating in place; a session that disappears is removed at once, since a
deleted session must not stay clickable; and a new session appears at the bottom of the list, moving no row above it,
until the hold ends and it takes its place. Changing the list's host, filter, or order is asking for a different list,
so the hold does not apply to it. While a session row's menu is open, the hold is kept until the menu closes, even if
the pointer leaves the list or stays still, because releasing it would move the very row the menu belongs to; pointer
movement over the list starts the 5 seconds of stillness over.

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
toggle — reachable there or by clicking the dot itself — that sets the session's seen state directly (see Status). Every
session row carries one permission mark. An amber slashed shield means the session's launch is YOLO by the rule under
Creation: its effective agent-launch permission is YOLO, or its command was asserted to be YOLO. A green plain shield
means an agent launch in a non-YOLO mode, or a command asserted not to be YOLO. A session created before launch kinds
existed that was not an agent launch has no assertion and carries an amber question mark. Hover and screen-reader text
name the specific agent-launch mode (default, approve, smart approve, chat, or YOLO), or say that the mark is the
assertion made when the command was launched, by the user or by an agent, and not something Farhelm checked, or that a
pre-existing session's command was never classified.

Hovering a live status dot, agent mark, or permission mark explains that mark. A clickable dot also names its mark read
or mark unread action. The hover text uses the same status and permission meaning the row exposes to assistive
technology. Exactly how a row lays out its lines and pixels is an implementation choice, covered in SPEC_impl.md rather
than here.

Per-host connection state is always visible in the host list, which names each host and pins its current phase beside
it. A compatible supervisor whose build is older than the helm's is still connected and usable, but its row says
`old
version` as an advisory; an incompatible protocol handshake remains `needs update`. On an SSH host with Update
available, an outlined `↑ update` button replaces those words: amber for an optional compatible-build update, red for a
required update when the host's protocol is lower. The dot keeps its color and the accessible status keeps its words.
Hover names both builds (and both protocols for a skew), says whether updating is optional or required, and explains
that clicking updates the host to the helm's version. Clicking starts the menu's Update without confirmation; inline
progress replaces the button. Local hosts, hosts with update options still loading, and hosts occupied by setup or
another run retain the plain words. The helm's own machine is not updated from the panel: its row menu shows Update
greyed out wherever it would otherwise be offered, saying to run the installer again, and choosing it sends nothing. A
host whose supervisor is newer than the helm (a newer build on the same protocol, or a higher protocol version) says
`too new` instead, is not offered Update, and its hover names the host's version, the helm's, and their protocol
versions. If either build string cannot be parsed as a semantic version, or the helm is an unreleased development build,
age is unknown and the row stays `connected`. The host count, its unpersisted details checkbox, and the secondary add
action share one header row. Host actions open on demand from the row menu, with the older-host update button also
available inline; details reveals the version, identity, session count, remedies, diagnostics, and provisioning progress
under every row. Templates use the neutral secondary tier for routine row actions and the normal blue tier for popup
affirmatives, while the host selector stays a native control; session creation remains the blue primary action.
Destructive confirmations use the danger tier, and explicit menu, tab, and composer controls retain their purpose-built
styling. Sessions on an unreachable host stay in the list from the helm's last-known knowledge (which survives helm
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

After a supervisor restart, a live pane keeps its last cached status while the supervisor gathers new screen evidence.
The first new screen alone does not turn an idle or waiting session into running. A witnessed exit, launch error,
changed screen, recognized wait, or enough unchanged samples replaces the old answer as soon as observed. A session with
no cached status remains unclassified during that gap. An unclassified session restarts without asking, like any agent
that does not read working (see Lifecycle operations), even though its pane may be live.

An interrupted session stays interrupted until the user acts: opening it and declining resume leaves it interrupted;
restart or delete are the ways out.

How a status is DRAWN depends on how much it has to say. The three live states are a color-coded dot beside the
session's title — running pulses, waiting and idle do not — with the status word itself always present as text for
screen readers and anything else that reads rather than looks, never replaced by the color. Outside compact mode, ended
states keep their complete wording visible, including exit code, stop annotation, and launch failure details. Compact
mode uses distinct stopped, exited, interrupted, and error icons; those details remain in accessible text and tooltips
without expanding the row. Ended icons do not have the live dot's mark-read action. The pulse is a claim about the
present, so it stands down wherever the status is a last-known report rather than a live one: a session on an
unreachable host shows a still dot whatever its status says. Looping indicators such as the running pulse change in a
few discrete steps per second rather than continuously, and all of them stand still while the Farhelm window is not
focused or not visible, to save CPU and battery.

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
that stopped changing is idle, and it never reports waiting. Claude Code and Codex have dedicated readers that recognize
what those agents draw — their busy indicators, their input prompt at rest, and the dialogs in which they ask the user
something — and answer from that, so an idle agent's own redraws read idle. A screen they recognize as carrying no state
(a menu the user opened) leaves the previous status in place, and a screen they do not recognize at all falls back to
the generic reader. Their rules are held to real screens captured from the vendors' current releases; see
`docs/agent-screen-fixtures.md`. A reader never creates lifecycle state. Wrong status must be cosmetic only — status
detection must never gate or delay interaction with the terminal. Its one effect on behavior is whether Restart and
Restart with ask before stopping a live agent (see Lifecycle operations), where a wrong reading costs at most a skipped
or an extra confirmation. Farhelm-supplied integration must not make vendor configuration a condition of launching an
agent. Grok is the explicit opt-in exception for conversation capture: users install its three documented hook entries
themselves, while an unconfigured Grok still launches normally and cannot be restarted. OMP and Grok both use generic
activity only. Their approval prompts show the generic running/idle classification, never waiting — a settled scope
decision, not a reader waiting to be written.

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
- Terminal text size is adjustable: Cmd+Shift with + or − on macOS, Ctrl+Shift elsewhere, or the A− / A+ buttons at the
  right end of the session's terminal tab strip, step every open terminal's text size together, hidden tabs included,
  within a fixed range, and each terminal is resized so the program inside sees its new rows and columns. While a
  terminal is open the shortcut works wherever focus is and never reaches the terminal's program or the browser;
  Cmd/Ctrl with = or − and no Shift stay the browser's own page zoom. On Linux this takes over Ctrl+Shift+− (Ctrl+_,
  undo in readline and emacs) inside the terminal, accepted 2026-10-02. The keys are the ones at the US-layout positions
  of = and −. The size is remembered per device, deliberately unlike the session list's preference: it is about the
  screen in front of the user, not a choice every client should share. Only the terminal text changes; the rest of the
  UI keeps its size.
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
- The typical session header is one keyboard-reachable row ordered status, session name, age, directory, command line,
  then Restart, Restart with, Replace, Clone, Replace with, and Delete. All six actions remain in the row and are fully
  visible from a 650px main pane; narrower panes may clip the trailing actions. Restart and Restart with are greyed out
  when the session cannot resume its conversation (see Lifecycle operations); their tooltips and accessible descriptions
  explain the specific reason, such as an agent type without conversation reporting, a command launch that declared no
  agent type or did not opt into Resume, or a conversation that was never captured. Directory and command line are muted
  click-to-copy buttons that take the width their values need and ellipsize only when the row runs out of room; a click
  confirms locally for about 1.5 seconds. Clipboard writes use the native bridge first and `navigator.clipboard` second,
  with JSON serialization and silent failures. Replace has its own anchored danger confirmation and neutral
  cancellation. Delete is styled as the danger action and always asks first in the same anchored way, even for a session
  that has ended, with the same consequence text the row's delete prompt shows.
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
  even one identical to what is already on the clipboard. When a plain drag goes to a program under mouse reporting and
  the program copies nothing (no OSC 52 write follows), the terminal shows a brief notice, at most once per page load
  for each distinct text, that the program handles selection itself: use its own copy command, or hold the forcing
  modifier while dragging. For a Codex session's agent terminal the notice names Codex's own copy key instead. It is
  guidance about the program, not a report of a clipboard failure. Clipboard operations are explicitly best-effort and
  silent on failure — permission policy, secure-context requirements, and an engine's own clipboard behavior are outside
  this system's control — a deliberate, named exception to the Errors and diagnostics section's surface-every-error rule
  below, not a lapse in it. Farhelm assumes the host's system clipboard works. A broken, hung or slow clipboard is not
  something Farhelm adds complexity to support well: copies may be dropped while it is in that state, but nothing else
  in Farhelm may stall because of it.
- Links in terminal output open their http(s) target on click, with no confirmation or prompt of any kind. A hyperlink a
  program emits (OSC 8) can underline text that differs from where it goes, so hovering it shows the exact target first,
  with its host emphasized. When the underlined text is itself a web address (it has a scheme, starts with `www.`, or is
  a dotted host ending in a name of two or more letters followed by `/`) and names a different place, ignoring only a
  trailing slash, the display becomes a prominent warning that shows the text beside the real target. Link text that is
  not a web address, such as a file name or "click here", keeps the ordinary display. Only the part of a link on the
  hovered line is compared, so a long web address that wraps onto another line gets the warning even when it is honest;
  that false alarm is accepted (see SPEC_impl.md). The warning is an aid, not a guarantee: a program that chooses its
  own line breaks can split a lookalike so that no single line looks like a web address, and the ordinary display, which
  always names the real host, is what remains. The hover display is the whole safeguard. Plain URLs printed as text need
  no such display, because what is shown is what opens.

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
sidebar's top row beside the version readout, with no separate visible app-title strip. The Templates control sits
beside New in the session list header. The session header and terminal tabs continue the app's surface to the top edge.
Empty header space provides window dragging, and double-clicking it zooms the window (repeating the gesture restores the
prior frame); a single click without movement never zooms. Controls and terminal text retain their own interactions.
Browser and Linux window layouts retain their existing appearance and gain no drag or zoom behavior. In narrow macOS
windows, the app-level row stays fixed above both scrolling panes so horizontal scrolling cannot move application
controls underneath native window buttons. Startup, authentication errors, and build-mismatch notices also keep their
content clear of native controls.

The native desktop remembers its last ordinary window rectangle and whether it was maximized when it closed. On the next
launch it restores that rectangle only when it fits on a currently connected display; otherwise it opens at a safe size
centered on a current display. On Wayland, which does not expose reliable global window positions, Farhelm keeps a
usable saved size and the maximized state while leaving placement to the compositor. Native fullscreen and webview zoom
are not persisted.

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
otherwise leave room for argument. A bare `claude` in a command launch must work exactly as it does from the user's own
shell; "command not found because a daemon launched it" is a bug, not a caveat. One deliberate exception: the directory
holding the Farhelm binary that launched the session comes first on the session's `PATH`, so `farhelm` run inside a
session reaches that exact build; in the Mac app's side-by-side version layout that directory holds the app's forwarder
instead, so `farhelm` reaches the version of Farhelm now running, which after an update and restart is the newer one.
Other programs in the same directory take precedence over the user's own `PATH` order as a result. The environment is
evaluated at each launch: edit your rc files and the next launch or restart sees the change; already-running sessions do
not.

When a host reboots, its supervisor starts automatically on hosts with the system-integration layer; on the v1 Mac it
returns when the app or binary is next started, and interruption is classified at that point — whenever the supervisor
comes back. After a boot, sessions last known running show as **interrupted** — explicitly a lost-track state, not a
claim about what happened in between: the agent may have exited on its own moments before the reboot, the supervisor
cannot know, and interrupted says exactly that. Sessions already known exited (including user-stopped ones) keep their
status; an exit during supervisor downtime with no reboot involved shows as exited — with the true exit code when the
surviving terminal still holds it, unknown code otherwise (reporting a code the terminal genuinely retains is not
guessing; inventing one where nothing retains it would be). Interrupted sessions' terminal contents are gone — there is
no history store (see Terminal experience) — but the conversation itself is recoverable. Opening an interrupted session
shows a centered neutral card in the empty terminal area explaining that the host restart paused the session and that it
needs an intentional restart. The card offers Restart when the session can resume its conversation (see Lifecycle
operations), and Replace, which starts a fresh session under the same settings and keeps its inline destructive
confirmation; a session that cannot resume offers only Replace and says why. Nothing respawns unattended — an agent
(especially one launched with permissive flags) only restarts or replaces when the user chooses and confirms. The system
must not presume the original OS process survived the reboot.

The resume promise is per-session: for agents with conversation-identity integration, the supervisor captures which
agent conversation belongs to each session, and restart resumes exactly that conversation (e.g.
`claude --resume <conversation-id>`) — even when several sessions share a working directory. Claude Code, Codex, Goose,
Pi, and Grok integrations at this level are required. Confirmed 2026-10-01: Farhelm identifies an agent's conversation
only from the harness's own explicit report, through a hook, plugin, extension, or whatever reporting mechanism that
harness needs. Heuristics that cannot be relied upon, such as correlating the vendor's files on disk with a launch, are
not supported, because a wrong match resumes, and appends to, a conversation that is not the session's own. Checking the
exact file a report names is verification of that report, not identification. A launch whose harness cannot report, or
whose report never arrived, has no captured identity and cannot be restarted: Restart says why, and Replace starts the
session over. What capture never does is write to the agent's own configuration or record directories. A hook passed on
the command line for one launch is allowed because it writes nothing the vendor owns — no configuration file, no
conversation record, no trust state — and cannot outlive the launch that carried it. It is not invisible in the
absolute: the report it delivers lands in farhelm's own database, and every run leaves a line in farhelm's own hook log.
Vendor-owned state is the boundary the no-agent-configuration rule from Status is protecting, and that rule's own
example — hooks written into the agent's configuration — still stands. Grok is the documented opt-in exception: the user
installs its hook entries, and Farhelm itself never writes, edits, or removes them. Every integrated agent type
identifies conversations only through accepted reports; Farhelm never selects a conversation by scanning vendor state.
Historical stored identities remain usable under the same per-type Resume rules, regardless of their source. A reporting
credential alone does not establish which Codex conversation is in the foreground. Goose persists a credential-free
named MCP reporter with the conversation and reuses it on resume; Pi loads a private static extension from Farhelm's
state directory on every launch. A Pi report without a session file withdraws the old resume target. Before a Pi resume,
Farhelm reads the bounded first record of that exact file without following symlinks and requires its session ID to
match. A failed check withdraws the durable resume offer, which makes Restart unavailable, and rejects the stale Resume
request so the user can refresh; it never silently launches fresh under that request.

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

Claude reports must come from a hook run by the session's pane process or by that process's direct child, so a `claude`
started underneath the foreground one — a shelled-out sub-agent that inherited the session credential and loaded a
reporting hook from its own settings — cannot replace or withdraw the foreground's conversation. The rule is positional:
Farhelm does not recognize Claude's executable and does not read the injected hook out of anyone's command line, because
both are vendor details that change independently of Farhelm. A plain launch makes the pane process Claude itself and a
one-level wrapper command makes Claude its direct child, so both keep reporting; a wrapper chain deeper than that loses
hook capture, and without an accepted report has no captured identity and cannot be restarted. Native sub-agents never
report: Claude fires no `SessionStart` for them, and any report naming a sub-agent is refused for every agent type.
Claude takes no versioned ownership proof, so its existing captures stay resumable across the change.

Grok reports must come from one native `grok` process under the owned pane, launched with `--no-leader` before any real
end-of-options boundary. The only admitted descendants are the documented reporter command and its narrow shell
trampoline; a nested Grok or another session-hosting runtime cannot replace the parent selection. `SessionStart` with
source `new` or `load` selects a UUID. `UserPromptSubmit` and `Stop` may add saved-record evidence only for that
selected UUID, and child-marked callbacks never select a top-level conversation. A selection stores the callback's
canonical RFC3339 event timestamp in the Grok locator: another UUID must be strictly newer, while repeats for the same
UUID may advance the timestamp but cannot lower it. Equal or older competing selections fail closed, including after
supervisor restart, without a general event log or clock-recovery protocol.

The selected UUID is resumable only while the exact reported absolute `updates.jsonl` begins with a supported record
whose method and `params.sessionId` match, and its sibling `summary.json` is a complete bounded JSON document with the
same UUID at `info.id`. Farhelm checks that pair during reconciliation and again before Resume. Missing or mismatched
evidence withdraws the offer while preserving the selected UUID and timestamp; Farhelm never derives a path, scans Grok
history, or chooses another conversation. A fresh `/new` normally remains pending until a later subscribed event
supplies its path. If Grok exits or crashes before the replacement `SessionStart` callback arrives, the previous UUID
can remain Farhelm's last known selection; this accepted delivery race does not weaken validation of callbacks that do
arrive.

OMP reports must come from the foreground OMP runtime under the session's owned pane — the Bun-executed bundle or
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
exception described in SPEC_impl.md. Claude admission requires the positional check above but no versioned proof, so its
resume rules are unchanged. Goose and Pi retain their existing admission and resume rules; for them the discriminator
alone adds no foreground protection. Old senders that predate the discriminator fail closed rather than reporting
untagged. A refused report changes nothing: no stored identity, no offer, no pending state. Resume is never silently
turned into fresh, and historical captures are never rewritten to look proven.

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
`--resume <conversation-id>`, Codex `resume <conversation-id>`, Goose `session --resume --session-id <conversation-id>`,
Pi `--session <verified-absolute-file>`, OMP `--resume <verified-absolute-file>`, and Grok
`--no-leader --resume <verified-conversation-id>`. A command launch that opts into Resume supplies its own resume
command, and Farhelm never derives one from its start command: it fills in `{conversation}`, `{cwd}`, and
`{farhelm_args}` and runs the rest as written. For Pi and OMP, `{conversation}` means the verified file path, not
Farhelm's internal durable locator; for Codex it means the verified persistent thread ID, not the runtime session ID or
encoded locator; for Grok it means the verified UUID, not the `grok:` locator or either evidence path.

Anything farhelm attaches to a launch — an agent launch, or a command launch that declares an agent type — must be
invisible from inside the session when it works AND when it fails: no output on the agent's terminal, no non-zero exit,
no error the agent's own UI can show. A hook that cannot do its job gives up silently within a bounded time and leaves
its diagnostics in farhelm's own state directory, never in the user's session. The bound is on the part the agent waits
for — reading the vendor's payload and reporting the result — because that is the whole of what can hold the agent up;
writing the diagnostic happens afterwards, is best-effort, and is not itself bounded. One accepted exception to the
invisibility rule is a line the vendor itself prints because of a flag we pass (Codex's hook-trust warning), which must
be documented.

The other exception is farhelm's own, and it is deliberate rather than tolerated: on a launch that gets the hook, the
hook prints exactly one line for the AGENT to read — that `$farhelm <request>` in the user's message means "use the
`farhelm agent` CLI", and that `farhelm agent instructions` explains the rest. Nothing reaches the user's terminal and
nothing is written to disk. It is on by default, because an agent that has never heard of the CLI will not go looking
for it, and `FARHELM_AGENT_INSTRUCTIONS=off` in the supervisor's environment removes it while leaving identity capture
exactly as it was. A launch with no hook has no pointer either, for the plain reason that there is nothing to print it.
The instructions themselves are printed only when that command is run, so a session where the user never mentions
farhelm pays one line and nothing more. Grok's manually configured hooks omit `--announce`: Grok ignores the relevant
stdout, so its integration delivers no instructions pointer.

A resume command is never run with `{conversation}` unfilled: no captured conversation means no Restart, with the reason
shown, not a garbled command line. `{cwd}` is always filled where it stands as a whole argument, on every launch and
restart; there is no launch without a working directory.

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
status. Precondition failures exit nonzero with a message on stderr. `--cwd` is required, and the launch is either
`--inherit-agent` or the launch flags shared with `farhelm agent create` below. `--inherit-agent` explicitly reuses the
asking session's stored launch, and works with no helm attached. The launch flags are resolved by the attached helm,
which owns templates and composes agent launches, and are refused with a remedy when no helm is attached. The title is
generated when omitted. An optional idempotency key makes retries safe: re-running spawn with the same key after a
timeout or ambiguous outcome returns the existing child rather than creating another. Keys are scoped to the asking
session on its host and live as long as the child session does: the same key from another session is an unrelated
request, never a replay of someone else's child, and a replay never returns the asking session itself. Confirmed
2026-09-28, the same scoping applies to the idempotency keys of `farhelm agent create` and `farhelm agent clone`.
Guaranteed Farhelm-injected environment: the session id (`$FARHELM_SESSION_ID`) and the per-session credential; other
Farhelm-specific variables are illustrative, not contract. (The user's login-shell environment is separately guaranteed;
see Durability.)

A session can also ASK, not only create. `farhelm agent <verb>`, run inside a session with the same injected credential
spawn uses, reaches the helm rather than the session's own supervisor: the supervisor forwards the question to the helm
currently attached to that session and relays the answer back, because a session has no way to reach the helm's machine
directly. The verbs are answered with the HELM's view — every host and template it knows, and every session it knows,
whichever machine they are on — with the asking session and its host marked. That is deliberately wider than spawn's
own-host-only rule above, which stands unchanged: creating is a local act, asking is not. Every verb goes this way,
including questions about the session's own host, so there is one answer to what an agent sees. The failure this defines
is "no helm is attached to this session", reported as such, with opening the session in a client as the remedy — never a
silent fallback to what the supervisor alone could have answered. The verbs may also ACT — rename, stop, restart — on
any session named by id, including the asking session when the caller deliberately supplies its id, with the helm
applying its ordinary rules to the operation exactly as it would for a client request. Rename also requires the title
the caller observed; the owning supervisor compares and changes it atomically, so a stale agent cannot overwrite a
concurrent rename. Restart requires an explicit `--session` target that can resume its conversation (as in the GUI,
there is no other kind of restart), and an explicit `--stop-if-running` consent when the target is working (an idle,
waiting, or unknown target is stopped without it, as in the GUI). The owning supervisor revalidates both the resume
offer and the target's status at handling time, and refuses rather than launching anything else when the conversation
can no longer be resumed. An explicit self restart warns before dispatch that it can interrupt the invoking CLI, lose
its acknowledgement, and leave resumed task continuation unconfirmed; it never prints an unobserved completion as
success. There is no `farhelm agent replace`: an agent replacing its own session would be killing itself mid-request,
which is a design question this version leaves open rather than answers by accident.

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

`farhelm agent create` and `farhelm spawn` take the launcher's fields as flags. `--template <name>` applies a template
by its exact name, and may be repeated to apply several in order. `--agent <type>` sets the agent type; `--model`,
`--effort`, `--permissions`, and `--trust` set an agent launch's choices; `--command` makes it a command launch, which
needs `--yolo` or `--no-yolo` as its assertion and may add `--agent <type>` as its declared agent type and
`--resume-command` to opt into Resume. Templates are applied first, in order, and the other flags then act as further
edits, so `--template my-codex --model gpt-6-luna` is `my-codex` with a different model. The result is validated exactly
like a GUI launch, with the same refusals naming the field, and the remembered GUI defaults never fill a gap, so the
same templates can launch differently from the CLI than from a GUI that preselected a remembered permission. A flag
required by the verb, such as `--cwd` or the target host, may be omitted when an applied template sets that field, and
an explicit flag wins over a template. `farhelm spawn` targets its own host, so a template that sets a host is refused
there. A template whose destination is a fresh GitHub checkout is refused on the CLI, which does not create checkouts.
`--inherit-agent` is exclusive with every launch flag. A command flag repeated or contradicted (`--yolo` with
`--no-yolo`) is refused rather than resolved by order. The removed selectors `--profile` and `--profile-id`, and
`farhelm agent restart --mode`, are refused with a message naming what replaced them, and `--agent` given something that
is not an agent type is refused with the list of agent types. An idempotency key is bound to the launch the first
accepted request resolved its templates and flags into: a retry with the same key returns that session even if a
template has been edited since. `farhelm agent clone` copies its explicitly selected source's stored launch onto an
explicitly named host, verbatim. Clone carries no translation between hosts: a command written for one machine may name
a binary that is absent, a different build, or one that takes different flags on another. Agents can apply templates but
not create, edit, or delete them: template writes from agents wait for the permission prompts for actions requested
through the `farhelm` CLI (TODO.md), because a template can carry a command line that every host the helm manages may
later run.

Templates are ordinary fleet metadata exposed by `farhelm agent templates`: each template's name and the fields it sets,
with their values except a command line or resume command, which are listed as set without their text. Discovery also
has `--json` forms with a versioned envelope, exact names, the caller's host identity, and completeness fields.

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
  the helm-side preference (list order, last selection, compact layout, and the host setup and removal confirmation
  choices), because losing next-launch convenience must not turn a choice that already took effect into a failed current
  operation, and a helm that lost the preference falls back to the defaults; and the automatic "mark seen" a session's
  own opening or activity advance triggers (see Status), because a lost automatic mark costs nothing worse than a dot
  that is one open-and-close cycle behind, corrected by the next successful write. The manual "mark unread"/"mark read"
  toggle is not covered by either exception — a failed toggle surfaces like any other operation.
- Connection state per host is always visible in the host list; reconnection uses bounded retries followed by periodic
  low-frequency re-probing, so a host that comes back overnight resurfaces by itself. Actions stay in each row's menu,
  with the older-host update button also available inline, while the global details disclosure shows the evidence and
  remedies behind the phase.
- Logs are available for: the helm, each supervisor, session creation, process/PTY lifecycle, attachment transfer,
  reconnection, and resume attempts.
- Long-lived input/output/paste paths have health checks, so "typing goes nowhere" is detected and reported rather than
  left for the user to infer.
- Mixed versions across helm and supervisors are a normal steady state, since updates are user-controlled. Incompatible
  versions refuse to connect with a clear, actionable error; there is no silent degradation.
- There is no compatibility across protocol versions. When a release changes the helm–supervisor protocol version, the
  user upgrades the supervisors it manages; until then those hosts refuse to connect and show that they need an update.
  Builds that share a protocol version interoperate (the mixed steady state above); builds on different protocol
  versions never exchange anything beyond the greeting that refuses them. Code and tests therefore do not decode,
  exercise, or shim another protocol version's messages, and reviews should push back on additions that do. Data a build
  persisted is a separate question, decided per feature (see Upgrade compatibility and client scale).

## Feedback

The sidebar's top bar has a **?** button immediately to the right of the settings gear, in the desktop app and the web
UI alike. It opens a small menu with two items: **Send feedback** and **Documentation**. Documentation opens the docs
site (`https://farhelm.io/docs/`) in the user's browser: a new tab from the web UI, the system browser from the desktop
app.

Send feedback is a lightweight, private way to tell the maintainer something. It opens a dialog with a required message,
an optional field for how to reach the user, and a plain display of everything else that will be sent: the Farhelm
version, whether this is the desktop app or the web UI, and the operating system the user is on. That display is the
whole submission; nothing else is attached (no logs, session content, host names, or paths). Nothing leaves the machine
until the user presses Send. Feedback goes only to the project's maintainer, not to a public issue tracker, and needs no
account; opening a public issue stays possible for anyone who prefers that. On success the dialog thanks the user and
closes. On any failure (offline, the feedback service unreachable or refusing) it says sending failed, in plain words,
and keeps the typed text so the user can retry or copy it. Nothing is queued or retried later.

The helm sends the submission on the UI's behalf; the browser and the desktop webview never send it themselves. Sending
feedback is a UI action only. Farhelm offers agents no way to send feedback, and no agent request or `farhelm` CLI
command may be added that does: feedback sent that way would leave the user's machine without the user pressing Send.
This is about what Farhelm offers, not containment; anything that runs commands as the user can reach a public URL.

The submission travels over HTTPS to a service the project runs on Vercel, which sees the helm machine's IP address
(used only to rate-limit sends, never put into the feedback), and is stored as an issue in a private GitHub repository
that only the maintainer can read.

## Security

The [maintainer-confirmed decisions](#maintainer-confirmed-decisions) below define local account authority, directional
trust between hosts, and the exact temporary exceptions for agent-requested session creation and cloning. Apply those
boundaries when interpreting the transport and credential rules here.

Steady-state operation between Farhelm's own components has exactly two network edges — the browser to a standalone helm
(token-authenticated) and the helm to each supervisor (SSH) — plus the desktop app's deliberately local loopback edge.
Besides provisioning's release downloads (see [Topology](#topology)), one more outbound connection exists, only on
explicit user action: when the user sends feedback, the helm posts that submission, and nothing else, to the project's
feedback endpoint (see [Feedback](#feedback)).

- **Client to helm**: a standalone helm serves its web UI over plain HTTP bound to loopback only, with a required token.
  The helm refuses to bind non-loopback addresses in v1; TLS serving is post-v1. Reaching the UI from another machine
  means an SSH port forward the user sets up themselves — there is no built-in tunneling or Tailscale integration in v1.
  The browser therefore always talks to the loopback literal `http://127.0.0.1:<port>`, which is conveniently a secure
  context — the precondition the browser clipboard APIs require to be reachable at all. Eligibility is not the same as
  success: engine policy and per-request permission still apply on top of it, and a clipboard operation that the engine
  refuses fails silently by the Terminal experience section's own clipboard contract above, not with an error. The token
  still matters on loopback: it keeps other local processes and users out. The helm generates it on first run; the user
  views or rotates it on the helm's machine (`farhelm helm token show|rotate`), and the browser asks for it once per
  device and keeps a session thereafter. Rotating the token invalidates every browser device credential for new
  requests; already-admitted requests may finish. The native app's own credentials, which its embedded helm issues to it
  directly rather than through the token, are exempt from rotation, and its open connections stay up through one.
  Existing terminal and event-feed connections may remain usable or close on rotation, whichever keeps the
  implementation simpler; reconnecting requires a current credential. Rotation does not stop running agent sessions. The
  desktop app uses a different client boundary: its embedded helm serves no browser UI and exposes no token exchange. It
  chooses a fresh loopback port at each launch and accepts only the two credentials minted in memory for that launch;
  stored browser credentials in the shared state directory do not authenticate there. The token-control commands still
  operate on the shared durable token for a standalone helm started later. Dioxus itself also keeps a loopback WebSocket
  for its UI updates, protected by a random per-launch key. That framework listener is accepted; removing it would
  require maintaining a Dioxus fork. The desktop app embeds its helm; that edge is local. The token keeps other users
  OUT of a standalone helm; it does not let the browser tell the helm apart from another local user's process that binds
  the same port while the helm is down. That gap is accepted in v1: the browser UI is recommended only on a machine with
  no other, untrusted local users, and the native app is the preferred client wherever it is available.
  `docs/security.md` records the reasoning. The UI is served only under the IPv4 literal, never under the names
  `localhost` or `[::1]`: the helm binds only `127.0.0.1`, so another local account can bind `[::1]` on the same port at
  any time, even while the helm runs, and a browser that resolves `localhost` to `::1` would load that account's page
  under the origin holding the device secret. Refusing the names keeps any device secret from being stored under an
  origin another account can serve; a plain page load that names them and still reaches the helm is redirected to
  `127.0.0.1`. A device secret a browser stored under `localhost` before this rule remains exposed to such a squatter
  until the token is rotated, and a squatter on `localhost` can still show a lookalike token prompt, which falls under
  the gap accepted above. Whether the web token is stored in the user's password manager is the user's choice: the
  browser prompt is an ordinary password field, and Farhelm does not try to stop a browser from offering to save it or
  keep a synced store from holding it. A saved token being autofilled into a lookalike prompt is the same port-squatter
  gap. The helm's Origin check is a browser-side defense: it keeps pages in the user's browser, whose `Origin` the
  browser sets truthfully, away from the helm. The desktop app's embedded helm admits its webview's custom URL schemes
  (`dioxus://`, and `wry://` for the webview library underneath), which every desktop app built on that framework or
  library can present. A standalone helm does not admit those schemes: it has no custom-scheme page that needs a
  cross-origin API. Content displayed by another such application can still pass the embedded helm's Origin check. That
  is accepted for this browser-facing check only, because such content still has no credential; the embedded helm needs
  the exemption because the window's page is `dioxus://index.html`, cross-origin to the helm. That check is not the
  desktop app's identity. Nothing that establishes the native app as a client may rely on `Origin`; that rests on the
  credential the native process obtains itself, and hardening that keeps other software from passing for the native app
  goes through that credential.
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

- SSH is Farhelm's one transport integration for reaching hosts: given passwordless SSH access, the helm rides it
  automatically for provisioning, updates, and all supervisor communication. How the SSH connection itself is possible
  (a tailnet, a LAN, whatever) is the user's business. No public relay, no third-party rendezvous service.
- Provisioning rides the user's existing SSH access — their keys, agent, and config. Farhelm stores no SSH credentials
  of its own. Confirmed 2026-10-01: the config governs reaching and authenticating to the host (keys, the agent used to
  authenticate, ProxyJump, Match blocks), not what rides Farhelm's connections. Whatever the config says, Farhelm's own
  ssh connections, the supervisor connection and every provisioning step alike, never forward the agent, X11 or ports,
  and override the settings that would replace or wrap Farhelm's own remote command: a `RemoteCommand`, a forced
  terminal, and a `LocalCommand`. A remote host therefore cannot reach the helm machine's ssh agent, display or local
  ports through a connection that stays up around the clock. Accepted: right after an upgrade, a helm started within a
  minute of the previous version stopping (the desktop app reopened, or a helm run by hand) can attach to a shared ssh
  connection the previous version left open, and port forwards that connection set up last until it closes.
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
   quit the app, start a standalone `farhelm helm run` on the same state directory, and open that helm's web UI from a
   browser (token-authenticated). Stop the standalone helm and relaunch the app before continuing.
2. Create and launch an official Claude Code session in one action, in an existing `jj` workspace where Git reports
   detached HEAD.
3. Create a local (Mac) session the same way; both appear in one list.
4. Paste a Mac screenshot into the remote session's terminal; the path appears at the cursor and Claude reads the file.
5. Quit and relaunch the app: both sessions are still running, terminal state intact, exactly as left. Then reboot the
   Mac: the remote session is untouched; the local session shows interrupted, and opening it offers resume that restores
   the conversation.
6. Quit the app and start a standalone `farhelm helm run` on the same state directory. Attach to the remote session from
   one authenticated browser tab, then another; the first tab visibly detaches.
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

### Local authority and trust between hosts

The local security boundary is the Unix account on a particular host. Processes the user runs on the target host,
including same-account processes that can reach Farhelm's private tmux server, are trusted by this threat model. Farhelm
does not provide strong same-account isolation against deliberate interference with agents or that private session, and
session credentials do not provide same-account containment. Running agents without permission checks in disposable
remote environments is an intended use. Farhelm still keeps simple local guards against accidental interference where
their identity is available, such as excluding a session's recorded agent pane from tab discovery and destructive tab
cleanup. Future container or sandbox support would require a new, explicit isolation contract.

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

The same temporary exception covers template resolution. Any attached host may obtain any template's full contents,
including a command line and resume command it carries, because that exception already lets any host ask for any
template to be applied to a launch on itself, which delivers the same contents to it. Until the guardrails land,
templates are no place for secrets that must stay hidden from an attached host. This acceptance is not a standing grant:
it ends with the cross-host creation exception, when spawning sessions on other hosts and reading their session and
template data are limited to explicitly trusted environments.

Confirmed 2026-09-28, under that same temporary exception: command lines are not secret from agents either. An agent
runs with the same account authority as its host's supervisor, which can already obtain any template's contents, so an
agent may obtain any template's command line and resume command (for example by applying it in a spawn) and any
session's command line (for example by cloning that session onto a host it can read). Until the guardrails land, neither
templates nor session command lines are a place for secrets. This ends with the same exception.

Do not add other arbitrary cross-host execution capabilities by analogy with those exceptions. Future agent-driven
orchestration, such as setting up several sessions on another host, is wanted with an explicitly authorized launch
policy; trusted templates are a possible design, not a security property established for the current catalog.

### Client hardening

Confirmed 2026-09-28: for the browser UI, security work addresses concrete, practical attacks. Defense in depth against
a hypothetical flaw, such as a script-injection bug nobody has found, is not required there, and neither is protection
against actors this threat model already excludes: same-account processes, and other local accounts on a machine where
the browser UI is not recommended. The native app is the preferred client and is held to a higher bar: hardening that
narrows what a hypothetical flaw in it could reach, or that keeps other software from passing for it, is wanted, as long
as it stays proportionate.

### Signing in again

Confirmed 2026-10-01: the desktop app is the primary supported surface. Decided 2026-10-02: the desktop app never
involves a user-visible credential, and the split between its embedded helm and its window is an implementation detail.
Its own credentials are not revoked by browser sign-in token rotation or evicted by the cap on remembered client
credentials, so in ordinary operation it never signs in again. When a browser has to sign in again, for example after
the browser sign-in token was rotated or its device credential was evicted, that recovery may reset the page and lose
open forms, dialogs and drafts. Friction in the browser's token prompt is acceptable too. Do not spend significant
complexity preserving UI state across a sign-in.

Three things still hold. In the desktop app, an action the user started is never lost silently: it either completes and
reports its outcome, or reports that its outcome is unknown. The browser is excepted: an action still pending when its
token prompt opens may lose its report. The helm still carries the action out, since accepted actions are the helm's,
and the session list shows the result after sign-in. Decided 2026-10-02: the browser is best effort for rare problems
that lose a report but not work. Sign-in recovery never crashes the window or leaves it dead. If the desktop app's own
authentication fails anyway, which takes something genuinely broken rather than a rotation or a busy browser, the window
says so and offers a retry, without restarting the app. This principle may be revisited later.

### One GUI at a time

Confirmed 2026-10-01: a single GUI attached to the helm, one browser tab or the desktop app's window, is the supported
user surface. Several GUIs open on the same helm at once work on a best-effort basis: an action taken in one can race a
change made in another, and the outcome may reflect what that GUI last showed rather than the newest state. Fix such a
race when the fix is easy and adds little complexity; do not add significant machinery to keep concurrent GUIs
consistent. This does not withdraw the session view's multi-client rules (one attached client per session, takeover,
displaced clients), which still hold whenever more than one client opens a session, and the desktop app remains the
primary GUI (see Signing in again).

The `farhelm` command line, and the agent skill through which agents act on the fleet, are a fully supported primary
surface alongside the GUI, including while a GUI is open: operations they perform concurrently with a GUI must behave
correctly, and the best-effort qualifier above applies only to several GUIs at once.

### Remote input, session defaults, and availability

Agents may discover the helm catalog's template names and what each sets. Listing those in lookup suggestions is
explicitly allowed, not a confidentiality defect, and does not require a new discovery interface. Command lines are not
part of that listing, but they are not protected from agents either while the temporary exception in
[Local authority and trust between hosts](#local-authority-and-trust-between-hosts) lasts. Discovery also does not make
current templates trusted execution guardrails; the separate host-authority rules still apply.

Agent instructions must identify fleet session metadata as data, never instructions to follow; see
[Agent-spawned sessions](#agent-spawned-sessions) for the CLI contract. Merely echoing an agent's own input into its own
session terminal does not establish a security defect: the agent already controls that output. This does not excuse
unsafe rendering of remote input by the helm or GUI, secret disclosure, or violations of existing formatting contracts.

Only what the user explicitly selects in the GUI may affect the GUI's future defaults and suggestions: the remembered
permission mode and workspace-trust choice, and the recent setups the New dialog offers. Each is recorded from the
user's own selection in the request that succeeded, never from what a host replies or lists. A supervisor's create
reply, the settings a plain Replace copies from a listed row, and anything an agent creates do not qualify, because none
of them is a choice the user made in the GUI. A choice is still recorded only once its create succeeds; the host's
success decides whether it is recorded, never what.

Failures or malicious behavior from a remote host must not disrupt unrelated hosts or ordinary helm/GUI controls, apart
from the explicitly permitted operations above and the exceptions below. Supervisors need sensible recovery from
ordinary failures; they need not defend their availability against hostile processes with the same local account
authority. Choose proportionate remedies rather than assuming a quota or scheduling architecture is required.

Session ownership is one deliberate exception. When a host reports a session id that another host already owns, the helm
cannot tell which of the two is telling the truth, so it refuses to route any operation on that session (terminal, stop,
restart, rename, delete, Replace, uploads, detail) and says which two hosts claim it, rather than guess. A misbehaving
host can therefore make other hosts' sessions unreachable through Farhelm for as long as it keeps claiming their ids;
their agents keep running. That loss of access is the accepted response, because the alternative is a silent misroute:
after a host's cache is cleared by removing and re-adding it or by adoption, whichever host lists an id first would own
it, so a hostile host could quietly receive the terminal input, uploads, and stops meant for the real one. Removing the
misbehaving host is the remedy; the refusal clears on the next refresh after it stops claiming the ids.

Confirmed 2026-10-01: removing the host is the general remedy for a misbehaving host whose effect is limited to what the
helm and the GUI show. A host may crowd or clutter those views so that other sessions are hard to reach, for example by
reporting enough sessions that sort first to push every other host's sessions out of the merged all-hosts list (which is
capped), and so out of agents' fleet listings, which have no per-host option. In the GUI, per-host views still show the
others, routing by session id still reaches them, and removing the host restores the list. Farhelm is not required to
defend against that. What must still be prevented is a host breaking the helm itself or affecting the security of other
hosts.

Confirmed 2026-10-01: a misbehaving host degrading the helm's performance or availability, the way a denial-of-service
attack would, is accepted when it cannot easily be avoided. Farhelm avoids such effects where it reasonably can but does
not spend elaborate complexity on them. One it avoids: a supervisor's "sessions changed" hints make the helm refresh
that host at once, and each such refresh makes every open client re-read, so the helm spaces the refreshes hints cause
by the same minimum gap the supervisor promises to keep between its hints, with at most one more pending. A host that
hints without pause then costs the helm no more than a busy honest one.

### Ownership during cleanup and provisioning

Confirmed 2026-09-28: Farhelm's private tmux server is an implementation detail, not an interface, and the product
should keep it out of the user's way as far as practical. Interacting with it directly is unsupported, whether the user
does it by hand or a program running in a session does it (for example by pointing tmux at its socket). Farhelm keeps
the accidental path out of reach: agents and terminal tabs run without the `TMUX` and `TMUX_PANE` variables tmux sets in
its panes, so a plain `tmux new-window` or `tmux split-window` typed in a session behaves as it would over SSH, reaching
the user's own tmux server or none, not the private one. A tab's shell starts without them, so a startup file that
launches tmux when `TMUX` is unset does so in a tab as it would in an SSH login. Shells an agent starts for its own work
also run without them, as they would if the agent ran in a terminal outside tmux. Windows, panes, processes and
configuration changes made through the private server anyway are outside every Farhelm guarantee, including cleanup on
Stop, Restart, Delete and tab close. Do not add code or complexity to detect, track, clean up after, or recover from
them. Farhelm's own operations must still handle their own objects going missing without crashing, and the helm and GUI
must still handle the resulting remote failures safely.

Session teardown covers ordinary agent descendants, including background servers. Detached services started by shell
initialization before the agent launches are outside that guarantee, and so, on hosts without a usable systemd user
manager, are detached descendants whose environment cannot be read or has been overwritten; see
[Lifecycle operations](#lifecycle-operations).

After a failed Delete disconnects a viewer, either automatically reconnecting to a surviving terminal or remaining
detached until the user reconnects is explicitly acceptable. Choose the simpler implementation. Reviewers must not treat
either outcome alone as a bug or require additional recovery machinery to choose between them. The cleanup failure must
remain visible; recovery must not restart an agent or take control from another viewer. A retained session record does
not promise that cleanup preserved the terminal or its scrollback.

Provisioning may enforce permissions on directories dedicated to Farhelm: its private lib directory and the supervisor
state directory. It must preserve permissions on existing shared directories merely used to hold its executable or
service files, which are the systemd user-unit directory and, when the registered binary lives outside the lib
directory, that binary's own directory. A missing shared directory is created; an existing one is left exactly as it is.
If those permissions prevent installation, report the obstacle rather than silently changing them. Trust in the helm
does not authorize incidental changes to unrelated host configuration.

Farhelm is not designed for install directories that other local accounts can write to, whether provisioning's lib,
state, or binary directory on a host, a shared directory it writes into, or the standalone installer's install directory
(`~/.local/bin`). Keeping them writable only by the user is the user's responsibility, and Farhelm's installation,
update, recovery, and uninstall guarantees assume no other account can create or replace entries in them. A
group-writable or sticky shared directory is outside what the installer's lock, journal, and backup safeguards defend
against.

On a host provisioned from the hosts panel, the supervisor unit (`farhelm-supervisor.service`) has one owner. A unit
without `farhelm helm setup`'s managed-by marker belongs to provisioning: ADD and UPDATE may replace it, and uninstall
may remove it. A unit that carries the marker belongs to setup on that host: provisioning refuses to touch it, both when
planning and at the moment of writing or removing, and says setup manages it there, the same hand-off the helm's own
machine gets. A hand-written unit under that exact name on a host the user asks Farhelm to provision is the user's to
move aside first; provisioning does not try to tell it apart from its own.

### First-class harnesses

Confirmed 2026-10-01: Claude Code and Codex are the first-class harnesses. A clear, definite gap in how Farhelm reads
their activity, such as a screen either agent draws in ordinary use that its reader classifies wrongly, is a defect and
gets fixed. Activity tracking, session tracking and similar integration features for every other harness (Goose, Pi,
OMP, Grok, and any added later) are intentionally partial; gaps there are expected and are not worth raising in code
review. That allowance stops at those features. It does not cover lost user work (resuming the wrong conversation
included), a session that fails to launch or run, an effect on other sessions, other hosts or the helm, or a security
consequence. This may change as those integrations mature.

### Supported user environments

Confirmed 2026-09-28: bash and zsh are the supported login shells, for agent launches and terminal tabs alike. Other
login shells are unsupported: Farhelm need not make them work, detect them, or refuse them with a tailored message.

tmux is an implementation detail. The supported tmux is either the pinned build Farhelm itself installs or ships, or the
stock tmux package of the platform's standard package source (the Linux distribution, or Homebrew on a Mac), at or above
the version floor, run as the real binary. Using the distribution's package is a convenience that keeps its security
patches, not an integration promise. Anything else is unsupported, including wrapper scripts, custom or patched builds,
and configuration or behavior a stock package would not bring. `--tmux` and `FARHELM_TMUX` exist to choose among
supported binaries, not to support arbitrary programs. Farhelm need not add code or complexity to accommodate
unsupported shells or tmux programs.

Usernames of up to 20 characters must work on Linux and macOS with Farhelm's default state directory locations, on the
helm's machine and on every host, including wherever Farhelm places sockets or other files whose paths the system limits
in length. Beyond 20 characters, failures caused by those system limits are acceptable, but should say what limit was
hit rather than reporting an unrelated error.

Filesystem aliasing beyond symlinks is unsupported: bind mounts, and any similar mechanism that makes the same files or
folders appear at more than one path even after symlinks are resolved, including hard links. Farhelm compares canonical
paths (symlinks resolved, as the checkout rules above require) and treats each canonical path as the location it names.
A session that reaches a managed checkout, a working directory, or any other folder Farhelm tracks through such an alias
is not recognized as being there, and a file hard-linked between checkouts is not treated as shared. Farhelm need not
detect aliasing or add complexity to cope with it.

### Supported host setup

Confirmed 2026-09-28: for remote hosts, the supported, user-facing way to install and update a supervisor is the helm's
own setup and Update from the hosts panel, on the hosts that provisioning targets, and the way to remove one is the same
panel's uninstall. Every other way a host can end up with a supervisor works on a best-effort basis, mainly for people
working on Farhelm itself: a supervisor started by hand with `farhelm supervisor run`, one installed with `install.sh`,
one run by a unit the user wrote or changed with drop-ins, one at paths other than the layout setup installs, or a host
provisioning does not target. Do not spend significant complexity making setup or Update detect, adapt to, coexist with,
or preserve such setups; refusing with a clear message is enough. Setup and Update may treat the layout they install as
their own, including re-applying the settings they manage, such as start at boot and linger, on every run. The rules
above about shared directories and unrelated host configuration still hold.

A Linux machine running a helm remains supported, but the standalone installer temporarily does not cover that setup. On
a Mac the installer supplies the desktop app, which manages its own helm and local supervisor.

### Upgrade compatibility and client scale

Viewing and rotating the browser sign-in token through `farhelm helm token show|rotate` on the helm's machine is
sufficient for the current product. An app-UI token-management surface is not a current requirement.

Browser sign-in token rotation prevents old browser credentials from admitting new requests to a standalone helm. The
desktop helm accepts only its own launch's credentials, which are exempt; see "Client to helm". It does not require
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

The helm is optimized for a handful of browser/desktop clients, not a large device fleet. Retaining the 64 newest
browser credentials is acceptable even when an older credential is actively used. The desktop app's own credentials are
not counted among them and are never evicted. Beyond a few tens of enrollments, reauthentication friction is acceptable;
activity-based eviction is not required. Enrollments are credentials, not physical devices, and eviction does not delete
Farhelm sessions or terminate their agent processes.
