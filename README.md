# farhelm

Run coding agents (Claude Code, Codex, other terminal agents) on machines you control and supervise all of them from one
interface, through their real TUIs. See SPEC.md for what this is and is not, SPEC_impl.md for how it is built and why,
and PLAN.md for where the build currently stands.

NOTE: This is mid-milestone-6 software: several sessions at once, across every host in a registry the helm keeps. Hosts
are registered by SSH destination — through `--ensure-hosts` at startup, the API, or the UI's own hosts panel — and the
machine running the helm is always one of them without being registered at all; one flat session list spans them, each
row naming its host, with a host that goes dark keeping its sessions listed and marked stale. The hosts panel shows
every host's connection state at all times and is where hosts are added, retargeted, removed, retried, and where an
identity change is decided; opening a stale session shows its metadata behind a notice naming its host's actual state
instead of a terminal, and the create dialog picks which host a session is launched on. Reopening a session lands at the
tail of its history instead of replaying it as a scroll animation — for any replay within the client's buffering bounds,
which is every ordinary one; an unusually large replay, or one that stalls part-way, falls back to showing the catch-up
as it arrives instead of hiding it. A session can be renamed from either the list or its own view. Sessions survive a
supervisor restart (persisted metadata, and a still-viewable terminal whenever the private tmux server survived too), a
host reboot classifies previously-running sessions as interrupted rather than guessing, and a user-stopped session keeps
its "stopped by user" qualifier durably. Restart is live too: an interrupted (or exited, or errored) session relaunches
its agent — resuming its own Claude Code or Codex conversation where that conversation was captured, and saying plainly
that it is launching fresh where it was not. On Linux hosts with a systemd user manager, stopping a session also kills
its launch's own cgroup before the portable process sweep, which catches descendants that daemonized away from both (see
SPEC_impl.md for what that does and does not promise). Usable for real work, minimal in everything else. Two caveats
worth knowing before that real work: the helm's loopback API carries no authentication yet (the web token is a later
milestone), so any local account on the helm's machine can drive your sessions — treat multi-user hosts accordingly; and
every agent invocation entered through the GUI's create dialog is ordinary argv, visible to every local user via `ps`,
so credentials do not belong in it.

## Trying it (M6, in progress)

Prerequisites: Rust with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`), tmux on every
host involved, and `cargo binstall dioxus-cli@0.7.9` (or `cargo install dioxus-cli@0.7.9` — match the workspace's dioxus
version) for the web UI build.

NOTE on tmux versions: 3.3 or newer runs everything. Two fidelity details depend on the version, and neither stops a
session working:

- Restoring bracketed paste when you reattach needs 3.7, because that is when tmux gained the `bracket_paste_flag`
  format — below it the supervisor logs a warning (once, the first time a session is attached) and everything else still
  works.
- On 3.3, `capture-pane -N` does not preserve trailing styled padding, so the snapshot a stopped session replays can
  lose background colour painted past the last character of a row (a full-width status bar, say). Content and layout are
  unaffected. 3.4 and newer keep it.

Ubuntu 24.04 ships 3.4.

- Build: `cargo build`, then `(cd crates/farhelm-ui && dx build --platform web --release)`.
- On the host that will run the agent (or this machine): `target/debug/farhelm supervisor run`.
- Locally: `target/debug/farhelm helm run --ui-dist target/dx/farhelm-ui/release/web/public`. The helm drives every host
  in its registry at once, and the machine it runs on is always one of them — so a supervisor started on this machine
  needs no registering at all.
- To include a remote host (passwordless ssh assumed; copy the binary there first), write a JSON5 file listing it and
  pass `--ensure-hosts <file>`. The helm guarantees those hosts are registered at startup, adding what is missing and
  changing nothing else:

  ```json5
  {
    hosts: [
      // remote_farhelm and remote_state_dir are optional; omit them to use
      // the remote's own defaults.
      { ssh: "user@host", remote_farhelm: "/path/to/farhelm" },
    ],
  }
  ```

  A registered host that is switched off is not an error: it is registered anyway, and its connection state says what
  was found.
- Sessions are created in the UI, not on the command line. The working directory must be an absolute path on the host
  you pick — the supervisor rejects a relative one outright, and against a remote host a `~` would be expanded by your
  local shell against the wrong home.
- Open the printed loopback URL in a browser: a hosts panel above a session list. Every registered host is listed with
  its connection state in the helm's own words — connecting, unreachable-reprobing, connected, version-skew,
  identity-mismatch, identity-unverified, duplicate, retired — plus the evidence behind it (both versions on a skew,
  both identities on a mismatch) and, where there is one, what to do about it. "add host" registers a destination (with
  optional remote farhelm path and state directory for an install that is not on the remote's `PATH` or uses a
  non-default state directory); each ssh row can be retargeted in place or removed, every row can be retried, and a host
  reporting an identity that does not match the one on record offers to adopt it. Removing forgets the host and the
  helm's cached view of its sessions — the supervisor and its agents keep running, and re-adding the destination finds
  them again.
- Under it, the session list: which host each session lives on, its title, working directory, invocation, and a status —
  alive; exited with the code when known, qualified "stopped by user" when you stopped it; interrupted after a host
  reboot; error, with the reason, when the agent's own command could not start at all — refreshing on its own every few
  seconds. Sessions on a host that is not connected stay listed, dimmed and badged "stale"; their controls still work
  and the helm refuses them by naming the host's state rather than failing silently, and opening one shows its metadata
  behind a notice naming that state instead of a terminal. Click a row to open its terminal; a back control returns to
  the list. Close the tab, reopen it later: same session, scrollback intact, the agent never noticed. Reopening shows a
  brief "connecting — catching up" line instead of the history flying past: the terminal appears once, already at the
  end of what you missed. A replay that outgrows the client's buffer, or that goes quiet part-way, degrades to showing
  the rest as it arrives — visible catch-up rather than a hidden terminal, and never dropped output.
- "new session" opens an inline form (host, working directory and agent command required, title optional); submitting
  launches the agent and takes you straight into its terminal. The host selector lists every registered host, defaulting
  to the one your open session runs on and otherwise to the machine running the helm; a host that is not connected is
  offered too, labelled with its state, and the create against it fails in place with the helm's own explanation rather
  than being hidden from the list. Paths are sent literally — nothing expands `~` or any variable at any point between
  the form and the host — so a working directory must be written out in full as it exists on the host it names. A bad
  working directory fails the create in place, with the supervisor's own error shown next to the form and nothing
  created. A bad EXECUTABLE (a typo'd command, a path that does not exist) is different: creation still succeeds — the
  working directory and terminal both exist — and the session then reports error once its agent's own exec fails, with
  the reason shown right in the list.
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
- Each row also has rename, stop and delete. Rename opens a field in place — the same control the session view's header
  has — and what you type is sent exactly as typed; a title the supervisor refuses (control characters in it) comes back
  with the supervisor's own words while the old name stays. Stop kills the agent and its whole process tree; the session
  stays listed, its terminal still viewable. Delete removes the session and its stored state — with an inline
  confirmation first whenever the agent might still be alive.
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

The desktop window is the same UI in a wry webview: `cargo run -p farhelm-ui --features desktop` with `FARHELM_URL`
pointing at the helm (default `http://127.0.0.1:7433`).

## Development

`AGENTS.md` has the conventions and the finish-work checks. End-to-end tests: `cargo test -- --show-output` (Rust,
including real-tmux integration; `--show-output` is what surfaces the skip reasons from tests that need a systemd user
manager), and `cd e2e && npx playwright test` (browser against a real stack, Chromium and WebKit both — needs
`npm install` and `npx playwright install chromium webkit` once). `lore/` holds historical decision records; read
`lore/AGENTS.md` before touching it.
