# farhelm

Run coding agents (Claude Code, Codex, other terminal agents) on machines you control and supervise all of them from one
interface, through their real TUIs. See SPEC.md for what this is and is not, SPEC_impl.md for how it is built and why,
and PLAN.md for where the build currently stands.

NOTE: This is milestone-3 software: several sessions at once, one host, argv-driven setup. Sessions survive a supervisor
restart (persisted metadata, and a still-viewable terminal whenever the private tmux server survived too), a host reboot
classifies previously-running sessions as interrupted rather than guessing, and a user-stopped session keeps its
"stopped by user" qualifier durably. Restart is live too: an interrupted (or exited, or errored) session relaunches its
agent — resuming its own Claude Code or Codex conversation where that conversation was captured, and saying plainly that
it is launching fresh where it was not. On Linux hosts with a systemd user manager, stopping a session also kills its
launch's own cgroup before the portable process sweep, which catches descendants that daemonized away from both (see
SPEC_impl.md for what that does and does not promise). Usable for real work, minimal in everything else. Two caveats
worth knowing before that real work: the helm's loopback API carries no authentication yet (the web token is a later
milestone), so any local account on the helm's machine can drive your sessions — treat multi-user hosts accordingly; and
every agent invocation (the startup one below, or one entered through the GUI's create dialog) is ordinary argv, visible
to every local user via `ps`, so credentials do not belong in it.

## Trying it (M3)

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
- Locally, against a supervisor on this machine:
  `target/debug/farhelm helm run --ui-dist target/dx/farhelm-ui/release/web/public --cwd ~/some/project --agent claude`
- Against a remote host (passwordless ssh assumed; copy the binary there first): add
  `--ssh user@host --remote-farhelm /path/to/farhelm` to the helm command. Use an absolute `--cwd` there — it names a
  directory on the target host, and your local shell would expand a `~` against the wrong home.
- Open the printed loopback URL in a browser: a session list (title, working directory, invocation, and a status —
  alive; exited with the code when known, qualified "stopped by user" when you stopped it; interrupted after a host
  reboot; error, with the reason, when the agent's own command could not start at all), refreshing on its own every few
  seconds. Click a row to open its terminal; a back control returns to the list. Close the tab, reopen it later: same
  session, scrollback intact, the agent never noticed.
- "new session" opens an inline form (working directory and agent command required, title optional); submitting launches
  the agent and takes you straight into its terminal. A bad working directory fails the create in place, with the
  supervisor's own error shown next to the form and nothing created. A bad EXECUTABLE (a typo'd command, a path that
  does not exist) is different: creation still succeeds — the working directory and terminal both exist — and the
  session then reports error once its agent's own exec fails, with the reason shown right in the list.
- Submitting the same form twice yields one session, not two. Every create from the form carries an idempotency key, so
  a submit retried after an ambiguous failure — the request landed but its reply did not, the supervisor restarted
  mid-create — returns the session the first attempt already made instead of launching a second agent, and a failed
  create replays the same error rather than re-evaluating it. Editing any field first starts a NEW intent, which is how
  you fix a bad working directory and try again; it is also how you deliberately create a second session with the same
  parameters (change a field and change it back, or use the API). A key whose session you have since deleted is spent:
  the form says so rather than quietly recreating it.
- Each row also has stop and delete. Stop kills the agent and its whole process tree; the session stays listed, its
  terminal still viewable. Delete removes the session and its stored state — with an inline confirmation first whenever
  the agent might still be alive.
- Opening a session leads with what restarting it would do to the conversation, and the control says which: "resume
  conversation" when this session's own agent conversation was captured, "restart (fresh launch)" when it was not. A
  restart reuses the session's terminal when it still exists — the previous run stays above the new one in scrollback —
  and builds a fresh one when the host rebooted out from under it. Restarting a session whose agent is still running
  confirms first, then stops the whole process tree before relaunching; leftover daemons from a previous run are reaped
  the same way. A working directory that has vanished (or that now resolves somewhere else) fails the restart by name
  and leaves the session, its stop annotation included, exactly as it was.
- Sustained heavy output (piping a huge file through the pane, say) is flow-controlled end to end rather than freezing
  the tab or silently dropping bytes. A viewer that stops consuming entirely — a wedged tab, a laptop asleep past its
  connection timeout — is detached, with a visible reason, after a bounded stall; the session keeps running unaffected,
  and reattaching picks it back up.

The desktop window is the same UI in a wry webview: `cargo run -p farhelm-ui --features desktop` with `FARHELM_URL`
pointing at the helm (default `http://127.0.0.1:7433`).

## Development

`AGENTS.md` has the conventions and the finish-work checks. End-to-end tests: `cargo test -- --show-output` (Rust,
including real-tmux integration; `--show-output` is what surfaces the skip reasons from tests that need a systemd user
manager), and `cd e2e && npx playwright test` (browser against a real stack — needs `npm install` and
`npx playwright install chromium` once). `lore/` holds historical decision records; read `lore/AGENTS.md` before
touching it.
