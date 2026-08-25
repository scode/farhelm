> [!WARNING]
> **This repository is public, but you probably should not use it.** I use farhelm as my daily driver, and it is public
> so that I can work on it in the open, not because it is ready for anyone else. It is not recommended for general use
> at this time, for a variety of reasons. Do not expect user-friendliness, do not expect the documentation to be correct
> or helpful, and do not expect anything else either. This notice goes away when that changes.

# farhelm

Run coding agents (Claude Code, Codex, other terminal agents) on machines you control and supervise all of them from one
interface, through their real TUIs. Two pieces: a per-host **supervisor** that owns sessions and their terminals, and a
**helm** that connects to every supervisor — remote ones over your existing SSH access, the one on its own machine
directly — and serves the UI. This README is the how; see SPEC.md for what the system is and is not, and SPEC_impl.md
for how it is built and why.

NOTE: Every agent invocation entered through the GUI, whether typed into the create dialog or stored in a profile, is
ordinary argv — visible to every local user via `ps`. Credentials do not belong in it.

NOTE: Farhelm starts Codex with `--dangerously-bypass-hook-trust` so it can pass its own conversation-identity hook on
the command line. Two consequences you should know about before you use a Codex profile, both of them limited to the
launches that actually receive the flags — a launch farhelm skips, or one you have opted out of, carries no bypass:
Codex prints a warning line about the bypass, and any hook in Codex's active configuration home (`$CODEX_HOME` when it
is set, `~/.codex` otherwise) that you have NOT trusted will run during those sessions. Claude Code sessions get an
equivalent hook without a trust bypass. See [docs/agent-hook-injection.md](docs/agent-hook-injection.md) for what the
hook does, what it never does, and how to turn it off.

NOTE: The Mac app is not signed or notarized. macOS will treat it as an untrusted downloaded app; codesigning is
deferred until there is an Apple Developer identity to build with.

## Quickstart: desktop app plus one remote host

The quickest setup is the Mac app (Apple silicon) as your local helm, driving one remote Ubuntu host. Artifacts are on
the [releases page](https://github.com/scode/farhelm/releases). No Linux desktop app exists; on a Linux machine the helm
is a user service and the UI is a browser tab (see below).

NOTE: The macOS artifact is not published yet — the current release carries only the Linux archive. Until
`Farhelm-macos-aarch64.zip` appears on the releases page, this quickstart cannot be followed; use "Running the helm on
Linux" below instead.

- `brew install tmux`. The app does not bundle tmux. Homebrew's (3.7c at the time of writing) is the recommended way to
  meet the version floor, and the app looks for a tmux in the Homebrew and MacPorts prefixes itself, since GUI apps do
  not see your shell's `PATH`; `FARHELM_TMUX` in the app's environment names one explicitly. Without an acceptable tmux
  the managed supervisor refuses to start — today that refusal reaches the supervisor's log rather than a window, a gap
  TODO.md's macOS entry records — see the tmux NOTE below.
- Extract `Farhelm-macos-aarch64.zip` and start `Farhelm.app`. Because the app is unsigned, Control-click it in Finder,
  choose **Open**, then confirm **Open**. If macOS offers no confirmation there, attempt one normal launch and use
  **System Settings → Privacy & Security → Open Anyway** before trying again.
- The app starts its embedded helm and a managed local supervisor, so the Mac itself is already a host; both stop when
  the app exits. The window shows the web UI at `http://127.0.0.1:7433/`. If another process owns that port, the app
  refuses to start instead of choosing an undiscoverable origin; stop the conflicting service and relaunch.
- Add the remote host: open the hosts panel, choose "add host", and enter the host's SSH destination. Farhelm connects
  with your existing passwordless SSH configuration and inspects the host. A supervisor already running for your user is
  registered as-is; on a host without one, Farhelm shows the exact file-and-unit plan and does nothing until you confirm
  it. No root is involved at any point.
- Create a session: "new session", pick the host, pick an agent profile — every fresh supervisor ships with editable
  starters for Claude Code and Codex. The working directory starts at `~`, which expands once against that host's home
  at creation; any other directory must be an existing absolute path on that host — plain relative paths are rejected,
  and `~user` forms and variables never expand. Submitting drops you straight into the agent's terminal.
- If you start agents through a launcher that wants the directory as an argument (`my-wrapper run <dir> claude`), write
  `{cwd}` where the directory goes and set the profile's agent kind — see
  [docs/agent-wrappers.md](docs/agent-wrappers.md).

To use an ordinary browser instead of (or alongside) the app window, open `http://127.0.0.1:7433/` and paste the token
printed by `Farhelm.app/Contents/MacOS/farhelm helm token show`. `Farhelm.app/Contents/MacOS/farhelm helm token rotate`
replaces that token and invalidates every browser that has signed in.

## Setting up supervisors

Every host runs one supervisor per user. The helm reaches them over SSH (no ports are opened; supervisors listen on no
network interface), and "add host" is discovery-first: if a supervisor is already running for your user, it is
registered as-is and never restarted or replaced.

NOTE on tmux: sessions live in a private tmux server, and Farhelm requires tmux 3.7c or newer, in tmux's own release
spelling (`3.7c`, `3.8`; development builds such as `next-3.8` and distro-decorated versions are refused rather than
guessed about) — deliberately newer than many distros ship (Ubuntu 24.04 has 3.4, 26.04 about 3.6, Debian 13 3.5a; some
current Fedora releases do ship 3.7c). The floor is the exact version the crash-regression suite runs against, and it
moves only when that suite moves with it. This is not version snobbery: Farhelm drives tmux's control mode,
output-client teardown, and pane-death timing far harder than interactive use does, and older versions have crashed the
server under it, taking every session on the host with them. Automatic setup checks the host's tmux and installs
Farhelm's pinned static build when the host has nothing acceptable, and names whichever binary it accepted in the
service it writes, so a stale private build cannot shadow it later; on Linux, Linuxbrew's tmux is the documented way to
meet the floor without taking that build, and on macOS Homebrew's is the recommended one. A supervisor that finds an
older tmux refuses to start and says which binary it checked, what version it found, and what the floor is. The same
check covers a private tmux server that is already running for this state directory: a newer client does not silently
adopt an older server, because the server is the part that holds sessions and the part that has crashed — the supervisor
refuses, names both versions, and leaves the old server and its sessions alone for you to drain.

If you want to run a different tmux anyway, the override is one knob honored by every launch path, the desktop app
included: `farhelm supervisor run --tmux /path/to/tmux`, or `FARHELM_TMUX=/path/to/tmux` in the supervisor's environment
(the flag wins when both are set). The chosen binary is version-checked like any other and refused by name if it is
below the floor. Versions above the floor are accepted with a one-time warning that they are unaudited. Either way the
override means you own the substrate: it is a way to run something newer or differently built, not a supported
configuration.

### Linux hosts

Use the automatic path: add the host from the hosts panel and confirm the displayed plan. It targets Ubuntu on x86_64
and aarch64 with a systemd user manager, installs a static supervisor binary and user-level systemd units under your own
account, and offers `loginctl enable-linger` as an optional step so the supervisor starts at boot and survives logout
(some managed machines refuse linger; the unit then runs only while you are logged in). Each registered host row later
offers an update action with the same plan-then-confirm handshake.

Hosts the automatic path does not support keep a manual fallback: run `farhelm supervisor run` in a terminal (with a
tmux at or above the floor on `PATH`, or named with `--tmux`), or adapt `units/farhelm-supervisor.service.in` from the
Linux archive by filling in its `@…@` placeholders — `@FARHELM@`, `@STATE_DIR@`, `@PATH@`, and `@TMUX@`, the last being
the exact tmux executable the unit pins through `FARHELM_TMUX`. A supervisor set up either way is found by "add host"
like any other.

### macOS hosts

There is no system integration on the Mac yet: the supervisor runs while something runs it. On the machine where
Farhelm.app is your helm, the app already manages one. To make some other Mac a host for a helm running elsewhere,
`brew install tmux` there and start the supervisor by hand. Homebrew's `bin` is normally on a login shell's `PATH`, so
this is enough:

```
/Applications/Farhelm.app/Contents/MacOS/farhelm supervisor run
```

If it is not, name the binary: `farhelm supervisor run --tmux "$(brew --prefix)/bin/tmux"`.

Then add the Mac from the hosts panel: enter its SSH destination, and put
`/Applications/Farhelm.app/Contents/MacOS/farhelm` in the "remote farhelm (optional)" field — the discovery probe runs
`farhelm` over a fresh SSH login, whose `PATH` does not include the app bundle, so without that field (or a symlink onto
the login shell's `PATH`) a running supervisor is reported as unreachable. Discovery then registers it — automatic setup
is not offered for Macs. Stopping the supervisor does not stop its sessions: tmux keeps them running and they reattach
when the supervisor returns; only a reboot takes them down.

NOTE: do not start Farhelm.app itself on a Mac you mean to drive from another helm — the app has no helmless mode, so
launching it starts a second helm, and running more than one helm is unsupported.

## Running the helm on Linux

NOTE for coding agents: these steps modify the operator's live install. They are written for the human operator, and
AGENTS.md forbids agents from running them — or from restarting the helm's units — unless the user explicitly asks.

- Extract `farhelm-linux-x86_64.tar.gz` into `~/.local/lib/farhelm/`.
- Run `mkdir -p ~/.config/systemd/user`, then copy `~/.local/lib/farhelm/units/farhelm-helm.service` into that
  directory.
- Run `systemctl --user daemon-reload && systemctl --user enable --now farhelm-helm.service`.
- Run `loginctl enable-linger "$USER"` so the helm can start at boot and survive logout, where the machine allows it.
- Open `http://127.0.0.1:7433/` and paste the token printed by `~/.local/lib/farhelm/farhelm helm token show`.
- In the hosts panel, the local row offers supervisor setup when none is present; remote hosts are added exactly as in
  the quickstart above.

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
