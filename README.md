# farhelm

Run coding agents (Claude Code, Codex, other terminal agents) on machines you control and supervise all of them from one
interface, through their real TUIs. Two pieces: a per-host **supervisor** that owns sessions and their terminals, and a
**helm** that connects to every supervisor over your existing SSH access and serves the UI. This README is the how; see
SPEC.md for what the system is and is not, and SPEC_impl.md for how it is built and why.

NOTE: This is early software. Usable for real work, minimal in everything else.

NOTE: Every agent invocation entered through the GUI, whether typed into the create dialog or stored in a profile, is
ordinary argv — visible to every local user via `ps`. Credentials do not belong in it.

NOTE: The Mac app is not signed or notarized. macOS will treat it as an untrusted downloaded app; codesigning is
deferred until there is an Apple Developer identity to build with.

## Quickstart: desktop app plus one remote host

The quickest setup is the Mac app (Apple silicon) as your local helm, driving one remote Ubuntu host. Artifacts are on
the [releases page](https://github.com/scode/farhelm/releases). No Linux desktop app exists; on a Linux machine the helm
is a user service and the UI is a browser tab (see below).

- Extract `Farhelm-macos-aarch64.zip` and start `Farhelm.app`. Because the app is unsigned, Control-click it in Finder,
  choose **Open**, then confirm **Open**. If macOS offers no confirmation there, attempt one normal launch and use
  **System Settings → Privacy & Security → Open Anyway** before trying again.
- The app starts its embedded helm and a managed local supervisor, so the Mac itself is already a host; both stop when
  the app exits. The window shows the web UI at `http://127.0.0.1:7433/`. If another process owns that port, the app
  refuses to start instead of choosing an undiscoverable origin; stop the conflicting service and relaunch.
- Add the remote host: open the hosts panel, choose "add host", and enter the host's SSH destination. Farhelm connects
  with your existing passwordless SSH configuration, inspects the host, and shows the exact file-and-unit plan; it does
  nothing until you confirm it. No root is involved at any point.
- Create a session: "new session", pick the host, pick an agent profile — every fresh supervisor ships with editable
  starters for Claude Code and Codex. The working directory starts at `~`, which expands once against that host's home
  at creation; any other directory is written out as it exists on that host (`~user` forms and variables never expand).
  Submitting drops you straight into the agent's terminal.

To use an ordinary browser instead of (or alongside) the app window, open `http://127.0.0.1:7433/` and paste the token
printed by `Farhelm.app/Contents/MacOS/farhelm helm token show`. `farhelm helm token rotate` replaces that token and
invalidates every browser that has signed in.

## Setting up supervisors

Every host runs one supervisor per user. The helm reaches them over SSH (no ports are opened; supervisors listen on no
network interface), and "add host" is discovery-first: if a supervisor is already running for your user, it is
registered as-is and never restarted or replaced.

NOTE on tmux: sessions live in a private tmux server, and any tmux 3.3 or newer works. Automatic setup checks the host's
tmux and installs Farhelm's pinned private build when no supported one is present. Two minor fidelity details depend on
the version — below 3.7, bracketed-paste state is not restored on reattach (a one-time warning, nothing else affected),
and on 3.3 a stopped session's snapshot can lose background colour painted past the last character of a row. Ubuntu
24.04 ships 3.4.

### Linux hosts

Use the automatic path: add the host from the hosts panel and confirm the displayed plan. It targets Ubuntu on x86_64
and aarch64 with a systemd user manager, installs a static supervisor binary and user-level systemd units under your own
account, and offers `loginctl enable-linger` as an optional step so the supervisor starts at boot and survives logout
(some managed machines refuse linger; the unit then runs only while you are logged in). Each registered host row later
offers an update action with the same plan-then-confirm handshake.

Hosts the automatic path does not support keep a manual fallback: run `farhelm supervisor run` in a terminal (with a
supported tmux on `PATH`), or adapt `units/farhelm-supervisor.service.in` from the Linux archive by filling in its `@…@`
placeholders. A supervisor set up either way is found by "add host" like any other.

### macOS hosts

There is no system integration on the Mac yet: the supervisor runs while something runs it. On the machine where
Farhelm.app is your helm, the app already manages one. To make some other Mac a host for a helm running elsewhere, start
the supervisor by hand with the bundled tmux first on `PATH`:

```
PATH="/Applications/Farhelm.app/Contents/MacOS:$PATH" /Applications/Farhelm.app/Contents/MacOS/farhelm supervisor run
```

(Any tmux 3.3+ on `PATH` works, Homebrew's included.) Then add the Mac by SSH destination from the hosts panel;
discovery finds the running supervisor and registers it — automatic setup is not offered for Macs. Stopping the
supervisor does not stop its sessions: tmux keeps them running and they reattach when the supervisor returns; only a
reboot takes them down.

NOTE: do not start Farhelm.app itself on a Mac you mean to drive from another helm — the app has no helmless mode, so
launching it starts a second helm, and running more than one helm is unsupported.

## Running the helm on Linux

No desktop app here; the helm runs as a user service and you use a browser.

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
