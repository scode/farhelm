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

## Install

```
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | sh
```

The script detects your platform (Linux x86_64 or aarch64, macOS on Apple silicon), downloads the matching release from
GitHub, verifies its SHA-256 against the release's `SHA256SUMS`, and puts `farhelm` — and on macOS also
`farhelm-desktop` — into `~/.local/bin` (set `FARHELM_INSTALL_DIR` to choose another directory; set `FARHELM_VERSION` to
`X.Y.Z`, `vX.Y.Z`, or a `-rc.N` prerelease of one, e.g. `v1.2.3-rc.1`, to pin a version). That is all it does. It does
not touch systemd, launchd, or your shell configuration, and leaves nothing behind outside that directory (it stages
downloads in a temporary directory inside it and deletes that); if `~/.local/bin` is not on your `PATH` it tells you and
leaves the fix to you. Read it before you run it — it is short, and it is the same file for every platform. It needs
`curl`, `tar`, and one of `sha256sum`, `shasum`, or `openssl`.

`FARHELM_VERSION` and `FARHELM_INSTALL_DIR` need to be set for the `sh` at the far end of the pipe, not for `curl` —
`FARHELM_VERSION=v1.2.3 curl ... | sh` sets it for `curl` and does nothing useful. Put the assignment after the pipe
instead:

```
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | FARHELM_VERSION=v1.2.3 sh
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | FARHELM_INSTALL_DIR="$HOME/farhelm/bin" sh
```

No token, account, or GitHub login is needed. The binaries are static (Linux) or single-file (macOS), unsigned, and
carry no installer or updater of their own (this may change in a future version).

NOTE: Unsigned is fine for the curl path specifically — a file `curl` writes to disk carries no macOS quarantine
attribute, so there is no Gatekeeper prompt to get past. A `farhelm-desktop` binary downloaded through a browser instead
would need the Control-click-then-confirm dance a browser-downloaded `.app` or `.zip` always needs.

### Update

Run the same command again. It downloads and verifies the newest release first, then replaces the old binaries; if
replacing the second binary fails it puts the first one back, so you never end up with a mixed pair. Then restart what
is running:

- Linux: `systemctl --user restart farhelm-supervisor farhelm-helm`.
- macOS: quit and reopen `farhelm-desktop`. The app owns the embedded helm and the local supervisor it started — those
  are its child processes and stop with it — so reopening it starts them from the new binaries. If the app found a
  supervisor already running when it started (one you started by hand with `farhelm supervisor run`), it reuses it;
  restart that one by hand.

Running sessions survive either way: they live in a tmux server that neither restart touches, and the new supervisor
reattaches to them. Linux hosts you added from the hosts panel keep running their old supervisor until you update them
from that panel (its existing "update" action), which pushes the helm's own version. To go back, run the script with
`FARHELM_VERSION=vX.Y.Z`.

### After installing: which kind of machine is this?

- **It should run your helm and host sessions itself** (the usual single-machine setup, Linux): run
  `farhelm helm setup`. It writes the helm and supervisor user units into `~/.config/systemd/user/`, enables and starts
  them, and prints the `loginctl enable-linger` line. `--dry-run` shows what it would write; `--uninstall` removes the
  unit files setup wrote (each carries a marker line; it refuses to touch a unit it did not write). It refuses if no
  tmux ≥ 3.7c is installed, and tells you how to install one; it never installs tmux for you. On a helm machine, the
  hosts panel's local row never installs a supervisor by itself: it points you at `farhelm helm setup`.
- **It runs the desktop app** (a Mac laptop): run `farhelm-desktop`. The app starts its own helm and a local supervisor
  as child processes — the Mac is a host already — and remote machines are added as hosts from its hosts panel. Do not
  run `farhelm helm setup` here; the app is the helm.
- **It is a Linux machine that only hosts sessions, added from another helm's hosts panel**: install nothing here by
  hand. The helm you add it from copies `farhelm` (and a tmux, if the host has none at or above the floor) over SSH and
  writes the supervisor unit itself. Hosts keep today's layout (`~/.local/lib/farhelm` on the host, written by the
  helm). A Mac used as a host is still set up by hand (`farhelm supervisor run`), as today.
- **You only want to look at a helm running elsewhere**: install nothing. Open the helm's `http://127.0.0.1:7433/`
  through an SSH tunnel in any browser and paste its token. There is no "desktop app against a remote helm" mode; the
  app always brings its own helm.

### What "add host" needs

When you add a Linux host, the helm downloads the host's `farhelm` and, if needed, `tmux` from the GitHub release that
matches the helm's own version, verifies the release's `SHA256SUMS` signature with a key built into farhelm, verifies
each file's SHA-256, caches them under the helm's state directory, and pushes them over SSH. The helm's machine
therefore needs to reach `github.com` at that moment; the host never does. A download that fails verification is
refused, never used. Offline or air-gapped helms can point `farhelm helm run --payload-dir` at a directory holding the
release's files exactly as downloaded from the releases page (the archives and the `tmux-*` files, unmodified); that
directory is trusted as-is — nothing in it is verified, because you put it there. A farhelm you built from source
yourself downloads nothing: "add host" tells you to pass `--payload-dir` (or install a release).

### What a release contains

Each `vX.Y.Z` on the releases page carries one archive per platform (`farhelm-<target>.tar.gz`, plus
`farhelm-desktop-aarch64-apple-darwin.tar.gz`, each holding exactly one bare binary), the two static `tmux` builds for
Linux hosts, and `SHA256SUMS` with its `.minisig` signature, plus cargo-dist's own metadata (`dist-manifest.json`, a
`.sha256` beside each archive, and a lowercase `sha256.sum`), none of which is signed or read by Farhelm. Do not mistake
`sha256.sum` for `SHA256SUMS`: the uppercase one, covering the six payloads and signed as `SHA256SUMS.minisig`, is the
only checksum file Farhelm and the install script verify against. The web UI is inside the `farhelm` binary; there is no
separate `web/` directory, no unit files to copy, and no `.app` bundle.

## Quickstart: desktop app plus one remote host

The quickest setup is `farhelm-desktop` (Apple silicon only) as your local helm, driving one remote Linux host. No Linux
desktop app exists; on a Linux machine the helm is a user service and the UI is a browser tab (see "Running the helm on
Linux" below).

- Install with the curl command above; on macOS it puts both `farhelm` and `farhelm-desktop` into `~/.local/bin`.
- `brew install tmux`. Neither binary bundles tmux. Homebrew's (3.7c at the time of writing) is the recommended way to
  meet the version floor, and `farhelm-desktop` looks for a tmux in the Homebrew and MacPorts prefixes itself, since GUI
  apps do not see your shell's `PATH`; `FARHELM_TMUX` in its environment names one explicitly. Before starting its own
  managed supervisor, `farhelm-desktop` checks the tmux it is about to hand that supervisor against the version floor
  itself; without an acceptable one it refuses immediately with one plain message on its own stderr and quits, rather
  than opening a window and letting the managed supervisor fail later — see the tmux NOTE below. Launched from a
  terminal, that message is right there; launched from Finder there is no terminal for it to reach, which is the
  remaining gap TODO.md records.
- Start `farhelm-desktop` (double-click it in Finder, or run `~/.local/bin/farhelm-desktop` from a terminal). It starts
  its embedded helm and a managed local supervisor, so the Mac itself is already a host; both stop when the app exits.
  The window shows the web UI at `http://127.0.0.1:7433/`. If another process owns that port, the app refuses to start
  instead of choosing an undiscoverable origin; stop the conflicting service and relaunch.
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
printed by `farhelm helm token show`. `farhelm helm token rotate` replaces that token and invalidates every browser that
has signed in.

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

Use the automatic path: add the host from the hosts panel and confirm the displayed plan. It targets any Linux host with
a usable systemd user manager on x86_64 or aarch64; the distribution is not a requirement (nothing provisioning does is
distribution-specific), and the displayed plan names the one it found. CI exercises Ubuntu. It installs a static
supervisor binary and user-level systemd units under your own account, and offers `loginctl enable-linger` as an
optional step so the supervisor starts at boot and survives logout (some managed machines refuse linger; the unit then
runs only while you are logged in). Each registered host row later offers an update action with the same
plan-then-confirm handshake. RHEL-family hosts with SELinux enforcing are untested.

The binaries that plan installs are fetched, not carried: the helm downloads the host's `farhelm` — and a `tmux`, if the
host has none at or above the floor — from the GitHub release matching the helm's own version, checks the release's
`SHA256SUMS` against a signature it verifies with a key built into farhelm, checks each file's SHA-256, caches them
under the helm's state directory, and pushes them over SSH. So the helm's machine needs to reach `github.com` at that
moment; the host never does, before or after. Anything that fails verification is refused, never used. An offline or
air-gapped helm points `farhelm helm run --payload-dir` at a directory holding the release's files exactly as downloaded
from the releases page — that directory is trusted as-is, because you put it there. A farhelm you built from source
downloads nothing by default and says so instead (see "Development" below).

That GitHub release is the DEFAULT source, not the only one. `farhelm helm run --release-base-url <url>` (or
`FARHELM_RELEASE_BASE_URL`) points the same verified download at any server publishing the release's files under that
URL — a mirror, a staging server, a test fixture — on any build, including one you compiled yourself. The verification
is unchanged: the mirror's `SHA256SUMS` still has to carry a signature that farhelm's built-in key accepts, for this
helm's own version, so a mirror can serve the bytes but cannot alter them. `--payload-dir` still wins if both are given.

Hosts the automatic path does not support keep a manual fallback: run `farhelm supervisor run` in a terminal (with a
tmux at or above the floor on `PATH`, or named with `--tmux`), or write a systemd user unit by hand. For the unit,
`farhelm helm setup --dry-run` on that host prints both units Farhelm would install — take the
`farhelm-supervisor.service` one and ignore the helm unit, which belongs on the helm's machine rather than a host. The
templates behind them are in `crates/farhelm-helm/units/`. A supervisor set up either way is found by "add host" like
any other.

### macOS hosts

There is no system integration on the Mac yet: the supervisor runs while something runs it. On the machine where
`farhelm-desktop` is your helm, it already manages one. To make some other Mac a host for a helm running elsewhere,
`brew install tmux` there, install farhelm with the curl command above, and start the supervisor by hand:

```
~/.local/bin/farhelm supervisor run
```

If `~/.local/bin` is not on that shell's `PATH`, use the full path, or name the tmux explicitly:
`~/.local/bin/farhelm supervisor run --tmux "$(brew --prefix)/bin/tmux"` (the leading path locates farhelm; `--tmux`
separately locates tmux — the two are independent).

Then add the Mac from the hosts panel: enter its SSH destination, and put farhelm's path in the "remote farhelm
(optional)" field — the discovery probe runs `farhelm` over a fresh SSH login, whose `PATH` may not include
`~/.local/bin`, so without that field (or a symlink onto the login shell's `PATH`) a running supervisor is reported as
unreachable. NOTE: this field is shell-quoted before it reaches the remote login shell, so a leading `~` is sent as a
literal character rather than expanded — `~/.local/bin/farhelm` will NOT work here. Get the real path instead, e.g. by
running `echo ~/.local/bin/farhelm` on that Mac (or `ssh <destination> 'echo ~/.local/bin/farhelm'` from wherever you
are), and paste the absolute result (`/Users/<name>/.local/bin/farhelm`). Discovery then registers it — automatic setup
is not offered for Macs. Stopping the supervisor does not stop its sessions: tmux keeps them running and they reattach
when the supervisor returns; only a reboot takes them down.

NOTE: do not start `farhelm-desktop` itself on a Mac you mean to drive from another helm — the app has no helmless mode,
so launching it starts a second helm, and running more than one helm is unsupported.

## Running the helm on Linux

NOTE for coding agents: these steps modify the operator's live install. They are written for the human operator, and
AGENTS.md forbids agents from running them — or from restarting the helm's units — unless the user explicitly asks.

- Install with the curl command from "Install" above. It puts `farhelm` into `~/.local/bin`.
- Run `farhelm helm setup`. It writes the helm and supervisor systemd user units, enables and starts both, and prints
  the two hints below. `--dry-run` prints the units it would write and the commands it would run, and changes nothing;
  `--uninstall` removes the units it wrote. It owns only the files it wrote — a unit you wrote yourself makes it refuse
  rather than replace it — and it never installs tmux. The helm unit it writes serves the web UI out of the binary, so
  it expects a release build with the UI embedded. Beside the units it keeps two dot-files systemd ignores: a lock, so
  two setups cannot run at once, and a short-lived `.<unit>.restart-pending` marker that makes a rerun finish a restart
  an interrupted run still owed.
- Run `loginctl enable-linger "$USER"` so the helm can start at boot and survive logout, where the machine allows it.
- Open `http://127.0.0.1:7433/` and paste the token printed by `farhelm helm token show`.
- In the hosts panel, remote hosts are added exactly as in the quickstart above. The panel never installs a supervisor
  on the helm's own machine: with none running there it tells you to run `farhelm helm setup`, and with one this setup
  installed it says so and points at `systemctl --user`. A running local supervisor is discovered and used like any
  other host.

## Asking Farhelm about the fleet, or acting on it, from inside a session

An agent running in a Farhelm session can ask what else is out there:

```
farhelm agent hosts
farhelm agent sessions
```

Both must run INSIDE a Farhelm session. `farhelm` is on the session's PATH already, and the session credential Farhelm
injects is what authorizes the question — there is nothing to configure, and nothing to pass. Run outside a session,
they exit non-zero naming the environment variable that is missing.

The answer comes from the HELM, not from the session's own machine: every host and every session it knows, wherever they
are running, with the asking session and its host marked `*` in the first column. That is deliberately wider than
`farhelm spawn`, which is answered by the session's own supervisor and only ever creates on that host. Output is an
aligned table on stdout, one row per line, with anything else on stderr — so a script can capture the table alone.

```
$ farhelm agent sessions
  ID        HOST         TITLE   CWD          AGENT  STATUS
* session-1 this machine auth     /w/auth     claude running
  session-2 builder      docs     /w/docs     codex  idle (stale)
  session-3 builder      old      /w/old      codex  archived
```

`(stale)` means the row is the last thing the helm heard before that host went unreachable, not a live reading.
`archived` replaces the status word for a session you have archived. A very large fleet is cut — at 5,000 rows, or
sooner if the rows themselves are large enough to make the answer unsendable — and the cut is announced on stderr rather
than left to be mistaken for the whole answer.

The same relay also carries three lifecycle verbs, on the same credential:

```
farhelm agent rename <title> [--session <id>]
farhelm agent stop [--session <id>]
farhelm agent archive [--session <id>]
```

Omitting `--session` acts on the asking session itself; naming one acts on ANY session the helm knows, on any host — the
same wider-than-`spawn` authority the read-only verbs already have, since the feature's mental model is an agent talking
to the helm itself, which already has fleet-wide authority. Success prints one plain confirmation line on stdout
(`renamed <id> to "<title>"`, `stopped <id>`, `archived <id>`), escaped the same way the listing tables are. One case
never prints that line: a bare `stop`/`archive` (no `--session`) ends the ASKING session's own process tree, and the
host-wide sweep that reaches every process carrying that session's marker can SIGTERM the `farhelm agent` process itself
before it gets to print — stopping or archiving yourself is supposed to end the whole tree, calling CLI included, so a
script relying on that confirmation should target a session other than its own.

And it carries two verbs that CREATE, which is where going through the helm buys something `farhelm spawn` cannot do at
all:

```
farhelm agent create --cwd <dir> [--host <name>] [--profile <name> | --invocation <cmd>] [--title <t>]
farhelm agent clone [--host <name>] [--cwd <dir>] [--title <t>]
```

`create` makes a session on any host; `clone` copies the asking session onto any host — same directory, same title, same
agent — which is the "start another one of these over on the build box" that used to mean walking to the UI. `--host`
takes a name straight out of `farhelm agent hosts`; leave it off and you get the host you are already on. Both print the
new session's id on stdout and nothing else, with the confirmation on stderr, so
`id=$(farhelm agent clone --host
builder)` works.

The agent is resolved by profile NAME on the target host, and this is the part worth understanding: profile ids are
minted per machine, so a clone onto another host looks up a profile with the same NAME in that host's own catalog. No
such name there is a refusal saying so, never a quiet fall back to running the source's command line on a machine that
may not have that binary. A session you created from a raw command line has no profile name to resolve and clones as
that command line. A `create` naming neither `--profile` nor `--invocation` uses the target host's last-used profile,
the same default the create dialog offers.

The failure worth knowing about is `no helm is attached to this session`. The relay reaches the helm that currently
holds the session open, so a session no client is looking at has no route to ask: open the session in the Farhelm UI and
run the command again.

### Talking to Farhelm from inside a session

You should not have to explain any of the above to your agent. Write `$farhelm <whatever you want to do>` in a message
to it — "$farhelm what else is running?", "$farhelm which hosts are up?", "$farhelm clone this session onto builder" —
and it should reach for the CLI on its own.

That works because the `SessionStart` hook Farhelm already injects for conversation identity prints one line the agent
reads: that `$farhelm ...` means the `farhelm agent` CLI, and that `farhelm agent instructions` explains the rest. The
instructions themselves — the verbs, the `*` marker, what "no helm is attached" means — are printed only when the agent
runs that command, so a session where you never mention Farhelm costs one line of context and nothing else. Run
`farhelm agent instructions` yourself if you want to see exactly what your agents are told.

NOTE: an agent that was never hooked is never told. The pointer rides on the identity hook, so the launches that skip
injection (an invocation of your own that already passes `--settings` or configures Codex's hooks, one containing a bare
`--`, an agent Farhelm does not recognize, or `FARHELM_AGENT_HOOKS` turning it off) get no pointer either — `$farhelm`
will not mean anything to those agents until you tell them yourself.

To turn the pointer off and keep identity capture, set `FARHELM_AGENT_INSTRUCTIONS=off` in the supervisor's environment.
It is read once when the supervisor starts, so it takes effect for launches after a restart and changes nothing about
agents already running. Unset or empty means `on`; `on` and `off` are matched case-insensitively after trimming
surrounding whitespace, since this is a value typed into a shell profile rather than a wire format. Anything else is a
typo rather than an instruction: the supervisor warns, names what you wrote, and falls back to `on` — a switch whose off
position removes a feature must not be flipped by a typo.

## Development

Development builds carry no provisioning payloads by default. Asking one to install a host with neither payload flag
reports
`this farhelm was built from source and carries no provisioning payloads; pass --payload-dir <dir> holding the
release files, or install a release build (see README, "Install")`.
Either flag opts a source build back in: `--payload-dir <dir>` (or `FARHELM_HELM_PAYLOAD_DIR`) reads published release
files from a directory unverified, and `--release-base-url <url>` (or `FARHELM_RELEASE_BASE_URL`) downloads them from
any server with the full signature and checksum verification a release build performs. `--payload-dir` wins if both are
given. Build the web UI with `(cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release)` after
`cargo build`, then run the supervisor and helm manually when working on the browser surface. The desktop smoke harness
supplies those development paths while testing the app-owned bootstrap.

`AGENTS.md` has the conventions and the finish-work checks. End-to-end tests: `cargo test -- --show-output` (Rust,
including real-tmux integration; `--show-output` is what surfaces the skip reasons from tests that need a systemd user
manager), and `cd e2e && npx playwright test` (browser against a real stack, Chromium and WebKit both — needs
`npm install` and `npx playwright install chromium webkit` once). `lore/` holds historical decision records; read
`lore/AGENTS.md` before touching it.
