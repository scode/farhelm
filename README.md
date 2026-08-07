# farhelm

Run coding agents (Claude Code, Codex, other terminal agents) on machines you control and supervise all of them from one
interface, through their real TUIs. See SPEC.md for what this is and is not, SPEC_impl.md for how it is built and why,
and PLAN.md for where the build currently stands.

NOTE: This is milestone-7 software: several sessions at once, across every host in a registry the helm keeps. Hosts are
registered by SSH destination — through `--ensure-hosts` at startup, the API, or the UI's own hosts panel — and the
machine running the helm is always one of them without being registered at all; one flat session list spans them, each
row naming its host, with a host that goes dark keeping its sessions listed and marked stale. The list is filtered and
searched by host, parent session, directory, agent profile, status and title; archived sessions are excluded by default
and an "include archived" switch widens that same query. Every client updates itself: a create, rename, stop, archive,
delete, status change or host going down shows up in every open browser without a refresh, and while that channel is
healthy the browser polls for none of it (it falls back to refetching every few seconds while the channel is down).
Agents are launched from named profiles — per host, editable, with starter definitions for Claude Code and Codex on
every fresh supervisor — or from a command typed into the create dialog. The hosts panel shows every host's connection
state at all times and is where hosts are added, retargeted, removed, retried, where an identity change is decided, and
where each host's profiles are defined; opening a stale session shows its metadata behind a notice naming its host's
actual state instead of a terminal, and the create dialog picks which host a session is launched on. Reopening a session
lands at the tail of its history instead of replaying it as a scroll animation — for any replay within the client's
buffering bounds, which is every ordinary one; an unusually large replay, or one that stalls part-way, falls back to
showing the catch-up as it arrives instead of hiding it. A session can be renamed from either the list or its own view.
Sessions survive a supervisor restart (persisted metadata, and a still-viewable terminal whenever the private tmux
server survived too), a host reboot classifies previously-running sessions as interrupted rather than guessing, and a
user-stopped session keeps its "stopped by user" qualifier durably. A terminal that loses its connection — a closed
laptop, a network that went away — gets itself back without you closing and reopening the session. Restart is live too:
an interrupted (or exited, or errored) session relaunches its agent — resuming its own Claude Code or Codex conversation
where that conversation was captured, and saying plainly that it is launching fresh where it was not. On Linux hosts
with a systemd user manager, stopping a session also kills its launch's own cgroup before the portable process sweep,
which catches descendants that daemonized away from both (see SPEC_impl.md for what that does and does not promise).
Usable for real work, minimal in everything else. On first run the helm creates a recoverable web token. A browser
enters the value printed by `farhelm helm token show` once, receives an origin-and-port-scoped device secret, and sends
that secret explicitly on later REST and WebSocket requests; the helm stores only its SHA-256 digest.
`farhelm helm
token rotate` replaces the bootstrap token, invalidates every device secret, and closes authenticated live
sockets. The state directory remains private to the helm account, and a script running in the authenticated UI origin
has that device's API authority. One further caveat worth knowing before real work: every agent invocation entered
through the GUI, whether typed into the create dialog or stored in a profile, is ordinary argv, visible to every local
user via `ps`, so credentials do not belong in it.

## Trying it (M7)

NOTE: The Mac app is not signed or notarized in v1. macOS will treat it as an untrusted downloaded app; codesigning is
deliberately deferred until there is an Apple Developer identity to build with. After extracting the archive,
Control-click `Farhelm.app` in Finder, choose **Open**, then confirm **Open**. If macOS offers no confirmation there,
attempt one normal launch and use **System Settings → Privacy & Security → Open Anyway** for Farhelm before trying
again.

Use a release artifact. The Linux archive contains the helm, web bundle, provisioning payloads for x86_64 and aarch64
Linux hosts, and the user-level systemd units. The Mac archive contains `Farhelm.app`, its embedded helm, the managed
local supervisor, private tmux, the same Linux provisioning payloads, and the CLI at
`Farhelm.app/Contents/MacOS/farhelm`.

On Linux:

- Extract `farhelm-linux-x86_64.tar.gz` into `~/.local/lib/farhelm/`.
- Run `mkdir -p ~/.config/systemd/user`, then copy `~/.local/lib/farhelm/units/farhelm-helm.service` to
  `~/.config/systemd/user/farhelm-helm.service`.
- Run `systemctl --user daemon-reload && systemctl --user enable --now farhelm-helm.service`.
- Run `loginctl enable-linger "$USER"` so the user manager and helm can start at boot and survive logout. Some managed
  machines refuse this; in that case the unit starts only after login and stops when the last login session ends.
- Open `http://127.0.0.1:7433/`. At first use, run `~/.local/lib/farhelm/farhelm helm token show` and paste the printed
  token into the page.
- Open the hosts panel. The local row offers setup when no supervisor is present; review the concrete file and unit
  plan, then confirm it. Add remote Ubuntu hosts the same way. Farhelm inspects first and does nothing until you confirm
  the displayed plan.

On macOS:

- Extract `Farhelm-macos-aarch64.zip` and start `Farhelm.app`. The app starts its embedded helm and managed local
  supervisor; both stop when the app exits in v1. The embedded web UI is always `http://127.0.0.1:7433/`, including
  after an app restart. If another process already owns that port, the app refuses to start instead of choosing an
  undiscoverable origin; stop the conflicting service before relaunching Farhelm.
- For token management, run `Farhelm.app/Contents/MacOS/farhelm helm token show` or
  `Farhelm.app/Contents/MacOS/farhelm helm token rotate`. There is no in-app token-management surface.
- Add remote Ubuntu hosts from the hosts panel. Setup uses your existing passwordless SSH configuration, writes only
  user-owned files and user-level systemd units, and requires no root.

Automatic setup checks tmux and installs Farhelm's pinned private build when the host does not already have a supported
one. The manual Mac runtime, clipboard-facts, and remote-paste checks are in
[`docs/manual-mac-checklist.md`](docs/manual-mac-checklist.md).

NOTE on tmux versions: 3.3 or newer runs everything. Two fidelity details depend on the version, and neither stops a
session working:

- Restoring bracketed paste when you reattach needs 3.7, because that is when tmux gained the `bracket_paste_flag`
  format — below it the supervisor logs a warning (once, the first time a session is attached) and everything else still
  works.
- On 3.3, `capture-pane -N` does not preserve trailing styled padding, so the snapshot a stopped session replays can
  lose background colour painted past the last character of a row (a full-width status bar, say). Content and layout are
  unaffected. 3.4 and newer keep it.

Ubuntu 24.04 ships 3.4.

- Sessions are normally created in the UI. A process already inside a Farhelm session can also run
  `farhelm spawn --cwd PATH`; the launch injects the session credential and supervisor socket that authorize it. Spawn
  joins a relative path to that process's current directory and otherwise preserves the path's lexical spelling; UI
  creation sends paths literally and requires an absolute path on the selected host. Neither form expands `~` or shell
  variables.
- The authenticated page is a hosts panel above a session list. Every registered host is listed with its connection
  state in the helm's own words — connecting, unreachable-reprobing, connected, version-skew, identity-mismatch,
  identity-unverified, duplicate, retired — plus the evidence behind it (both versions on a skew, both identities on a
  mismatch) and, where there is one, what to do about it. "add host" first discovers the destination. An answering
  supervisor is registered as-is; positive absence shows the exact setup plan and does nothing until you confirm it;
  unsupported hosts keep the concrete manual fallback. A confirmed run registers its row before execution and retains
  step-by-step progress and failures there, so a reload or another browser can follow or rerun it. Each registered row
  also has an explicit update action with the same plan-then-confirm handshake. Optional remote Farhelm and
  state-directory fields still describe installs that are not on the remote's `PATH` or use a non-default state
  directory. Each ssh row can be retargeted in place or removed, every row can be retried, and a host reporting an
  identity that does not match the one on record offers to adopt it. Removing forgets the host and the helm's cached
  view of its sessions — the supervisor and its agents keep running, and re-adding the destination finds them again.
- Each host row also opens its "profiles": the named agent definitions sessions on that host are launched from. A
  profile is a name, an invocation, an agent kind (Claude Code, Codex, or generic — the kind is what selects a
  supervisor's status heuristics and conversation capture, and is not user-authored beyond picking one), and an optional
  resume command. They belong to the supervisor, not to the helm, so each host has its own set and a fresh supervisor
  already ships with editable starters for Claude Code and Codex. Editing or deleting one changes what future sessions
  launch and nothing else: a session already created keeps the invocation and resume command it snapshotted, keeps
  running, and keeps naming the profile it came from — marked in the list once that profile has been renamed or deleted.
  A profile edited from another browser shows up here without a refresh, like everything else.
- Under the panel, the session list: which host each session lives on, its title, working directory, invocation, and a
  status. A live session reads running (the agent is working), waiting (it has asked you something and nothing has
  answered) or idle (it is at rest). The supervisor tells them apart by watching each session's terminal periodically: a
  screen that keeps changing reads running, one that has stayed the same for several looks reads idle, and a screen
  showing a recognized approval or question prompt from Claude Code or Codex reads waiting. It is a heuristic and it is
  allowed to be wrong — a status never gates anything, and typing into a session that is mislabelled works exactly as it
  always did. A finished one reads exited with the code when known, qualified "stopped by user" when you stopped it;
  interrupted after a host reboot; or error, with the reason, when the agent's own command could not start at all. A
  session created from a profile also names it. While its update channel is healthy the browser polls for nothing: it
  holds one connection the helm uses to say "something changed", and every open client re-reads when it hears that — a
  status flip, a rename from another window, a host going down. Lose that connection and the page falls back to
  refetching every few seconds until it is back, so the list is never stale for long either way. The helm itself is not
  push-driven all the way down: it refreshes each host's sessions from that host's supervisor every few seconds, so a
  status can be up to about that old before anyone is told. A session nothing has classified yet shows no status at all
  rather than guessing. Sessions on a host that is not connected stay listed, dimmed and badged "stale"; their controls
  still work and the helm refuses them by naming the host's state rather than failing silently, and opening one shows
  its metadata behind a notice naming that state instead of a terminal. Click a row to open its terminal; a back control
  returns to the list. Close the tab, reopen it later: same session, scrollback intact, the agent never noticed.
  Reopening shows a brief "connecting — catching up" line instead of the history flying past: the terminal appears once,
  already at the end of what you missed. A replay that outgrows the client's buffer, or that goes quiet part-way,
  degrades to showing the rest as it arrives — visible catch-up rather than a hidden terminal, and never dropped output.
- Above the list, filtering and search: host, parent session, working directory, agent profile, status, archived
  sessions, and a title search, applied when you submit rather than on every keystroke. The helm answers the query — the
  whole fleet is filtered before it is paginated, so the count above the rows ("N matching of M sessions") is about the
  fleet and not about the page you happen to be holding. Archived sessions are hidden by default; "include archived"
  widens the matching set without changing the fleet total. The parent field takes an exact session id. The profile
  field is free text rather than a menu on purpose: it matches a profile's id or the name a session snapshotted at
  creation, which is what keeps a deleted profile's sessions findable.
- "new session" opens an inline form: host, agent, working directory (required), title (optional, auto-generated when
  omitted). Submitting launches the agent and takes you straight into its terminal. The host selector lists every
  registered host and defaults to the machine running the helm (SPEC.md's rule names the host of the currently open
  session first, but this UI shows the list and a session as alternatives rather than side by side, so there is never
  one open while this dialog exists); a host that is not connected is offered too, labelled with its state, and the
  create against it fails in place with the helm's own explanation rather than being hidden from the list. The agent
  selector offers that host's profiles and preselects the one you last created a session from there; when that profile
  has since been deleted it preselects nothing and says so rather than quietly picking another. Pick "custom command"
  instead and the command field below is what runs — the two are exclusive, and choosing a profile greys the field out
  because the profile already says what to launch. Paths are sent literally — nothing expands `~` or any variable at any
  point between the form and the host — so a working directory must be written out in full as it exists on the host it
  names. A bad working directory fails the create in place, with the supervisor's own error shown next to the form and
  nothing created. A bad EXECUTABLE (a typo'd command, a path that does not exist) is different: creation still succeeds
  — the working directory and terminal both exist — and the session then reports error once its agent's own exec fails,
  with the reason shown right in the list.
- Submitting the same form twice yields one session, not two. Every create from the form carries an idempotency key, so
  a submit retried after an ambiguous failure — the request landed but its reply did not, the supervisor restarted
  mid-create — returns the session the first attempt already made instead of launching a second agent, and a failed
  create replays the same error rather than re-evaluating it. Editing any field first starts a NEW intent, which is how
  you fix a bad working directory and try again; it is also how you deliberately create a second session with the same
  parameters (change a field and change it back, or use the API). The chosen host is part of that intent — keys are
  scoped per host, so changing the target starts a new one too, whether you change it or a host leaving the registry
  moves it for you. A key whose session you have since deleted is spent: the form says so rather than quietly recreating
  it. The create API additionally accepts explicit agent-kind and resume-template overrides for invocations that
  basename recognition cannot classify (a wrapper script, `env claude`), which the form does not expose.
- Each row also has rename, stop, archive and delete. Rename opens a field in place — the same control the session
  view's header has — and what you type is sent exactly as typed; a title the supervisor refuses (control characters in
  it) comes back with the supervisor's own words while the old name stays. Stop kills the agent and its whole process
  tree; the session stays listed, its terminal still viewable. Archive is available on both the row and session view; it
  confirms whenever the agent, a prior process tree, or terminal tabs may still be destroyed, then removes the agent,
  tabs and terminal while retaining the session's metadata and attachments. Archived rows are hidden by default and
  restart is the only unarchive path. Delete removes the session and its stored state — with an inline confirmation
  first whenever the agent might still be alive.
- Opening a session leads with what restarting it would do to the conversation, and the control says which: "resume
  conversation" when this session's own agent conversation was captured, "restart (fresh launch)" when it was not. A
  restart reuses the session's terminal when it still exists — the previous run stays above the new one in scrollback —
  and builds a fresh one when the host rebooted out from under it. Restarting a session whose agent is still running
  confirms first, then stops the whole process tree before relaunching; leftover daemons from a previous run are reaped
  the same way. A working directory that has vanished (or that now resolves somewhere else) fails the restart by name
  and leaves the session, its stop annotation included, exactly as it was.
- A session view has a tab strip: the agent terminal, plus any number of plain shells opened in the session's working
  directory with "+ terminal". Every open tab stays attached while you are in the session, so switching between them is
  instant and a background tab keeps up with its own output. Tabs survive a reload and a supervisor restart with their
  scrollback, and a tab opened or closed from another client shows up here on its own; a host reboot or an archive takes
  them all, and nothing recreates them. Closing a tab confirms first, then kills that shell and everything it started —
  the agent and the other tabs are untouched.
- Opening a tab needs somewhere to open it: if the working directory has vanished, the open fails in place with the
  directory named, and on a session whose terminals a reboot or an archive already erased it fails telling you to
  restart the session first — a session with no terminal does not grow a tab-only one. Both messages are the
  supervisor's own, shown under the strip.
- Opening a second view of the same session takes over ALL of the first view's terminals at once, each saying so where
  it was. The displaced view then stops attaching anything — including tabs the new owner opens while it watches — until
  you press "take control" in its banner, which takes the session back the same visible way.
- Drop a file onto any of a session's terminals, or paste a screenshot into one, and it is transferred to the session's
  host and the resulting path typed in at the cursor — so the agent can read it without you copying anything by hand.
  Files keep their own names; a pasted screenshot gets a generated one (`pasted-1.png`). Several files at once insert
  several paths, each quoted when the path itself needs it. Plain text still pastes as text, including text that looks
  like a path, and a dropped folder is refused where you can see it. Transfers never block typing: keep working, and the
  path lands wherever the cursor is when the upload finishes. Attachments live beside the session's own state and go
  when it is deleted.
- Sustained heavy output (piping a huge file through the pane, say) is flow-controlled end to end rather than freezing
  the tab or silently dropping bytes. A viewer that stops consuming entirely — a wedged tab, a laptop asleep past its
  connection timeout — is detached, with a visible reason, after a bounded stall; the session keeps running unaffected,
  and reattaching picks it back up.
- A terminal whose connection drops reconnects on its own — including the case where nothing appears to have dropped,
  because a sleeping machine or a timed-out network path leaves a socket that looks fine and carries nothing. The
  terminal says which phase it is in (retrying for the first half-minute, then checking every 30 seconds for as long as
  it takes) and offers "reconnect now" throughout; a recovered terminal comes back where the session is now, not
  scrolling its history past again. Losing the session to another client, or being detached for stalling, deliberately
  does not reconnect: those are decisions, not dropped connections, and both keep their existing surface. Nor can a
  recovering terminal take the session BACK: if someone else opened it while you were disconnected, the automatic
  attempt is refused and you get the same "take control" button any displaced client gets.
- Upgrade the helm while a browser tab is open and the tab says so, in a line above everything else, instead of failing
  in ways nothing explains: the page carries the build it was made from and compares it against the helm's on every
  reply. It keeps working — the notice asks for a reload rather than taking the app away — but it stops doing anything
  UNATTENDED against a helm it does not match, so a terminal on a mismatched page waits for you to press "reconnect now"
  rather than reconnecting on its own.

## Development

Development builds intentionally carry no provisioning payloads; asking one to install a host reports
`this build carries no provisioning payloads`. Build the web UI with
`(cd crates/farhelm-ui && dx build --platform web --release)` after `cargo build`, then run the supervisor and helm
manually when working on the browser surface. The desktop smoke harness supplies those development paths while testing
the app-owned bootstrap.

`AGENTS.md` has the conventions and the finish-work checks. End-to-end tests: `cargo test -- --show-output` (Rust,
including real-tmux integration; `--show-output` is what surfaces the skip reasons from tests that need a systemd user
manager), and `cd e2e && npx playwright test` (browser against a real stack, Chromium and WebKit both — needs
`npm install` and `npx playwright install chromium webkit` once). `lore/` holds historical decision records; read
`lore/AGENTS.md` before touching it.
