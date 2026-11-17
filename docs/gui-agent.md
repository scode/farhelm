# Driving the GPUI app as an agent

NOTE: Debug builds only, and never against the live install. On Linux a run is headless; on a Mac it opens a real window
in the logged-in GUI session (see "On a Mac" below).

`scripts/gui-agent.py` launches the GPUI desktop app (headless on Linux) against a real embedded helm, a real local
supervisor and fake agents, and then lets you read the screen, click, type, press keys, wait and take screenshots, the
way a browser automation tool drives a web page. It is how an agent checks a GUI change it just made, and what
end-to-end GUI tests are built on.

## Quick start

- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 start`: builds what it needs, starts a private headless Mutter,
  launches the app on it and seeds two fake-agent sessions. The run directory must be absolute and must not exist yet;
  put it under your scratch directory (AGENTS.md, "Agent scratch space").
- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 snapshot -i`: the controls on screen, each with a ref (`@e3`), its
  role, its label, its states in brackets (`[disabled]`, `[checked]`, `[selected]` on a list's selected item) and the
  source line that built it.
- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 click @e3`, `... type @e6 "text"`, `... press enter`,
  `... wait --text "Saved"`, `... screenshot /abs/shot.png`.
- `scripts/gui-agent.py /tmp/farhelm-tmp/<id>/run1 stop`: quits the app and stops its supervisor, its terminal hosts and
  the compositor. Always run it, also after a failed `start`.

Every verb other than `start`, `stop` and `terminal-text` goes to the vendored `gpui-agent` command line unchanged, so
its README (`vendor/gpui-agent/README.md`, "Commands") is the full list. `gpui-agent skills get core`, run the same way,
prints its usage guide. `terminal-text` prints the focused terminal's text. While no terminal holds focus (after a click
on a button, say) it fails saying so rather than guessing which terminal was meant: click the terminal or Tab back into
it first. `terminal-text --wait TEXT` asks again until the focused terminal shows TEXT (30 seconds, or
`--timeout SECONDS`), and is the way to know a terminal you just opened has caught up: its screen arrives after the
click returns.

`start` takes `--sessions N` (basic fake-agent sessions to seed, default 2, 0 for none) or `--fleet KIND,...` (one
session per kind, in order: `basic`, `running`, `waiting`, `idle`, `exited` or `spawner`; see "Seeded sessions"),
`--size WxH` (Linux only: Mutter's virtual monitor, default 1600x1200; refused on a Mac, which uses its real screen),
`--no-build` (reuse the binaries already in the target directory), `--lock FILE` (hold an exclusive lock on FILE from
before the build until `stop`; see "On a Mac"), `--window-size WxH` (the window's first size, still fitted to the
screen; on Linux it must fit Mutter's monitor, and it includes the frame GPUI Kit draws inside the window there, a
20-pixel black shadow margin and a 1-pixel border on every side, and the 34-pixel title bar the window draws for itself
under Mutter, which `screenshot` captures too) and `--appearance light|dark` (start in that palette whatever the
system's appearance). The last two are test hooks of agent builds, passed to the app as environment variables that
`crates/gui/farhelm-gui/src/agent_startup.rs` reads, so a screenshot batch can capture each theme and size; they are not
the app's appearance setting, and a value the app cannot parse makes it refuse to start. `--start-dialogs` lets the
window's start dialogs open: the Welcome card, which a helm that has not seen it opens at window start, and the
announcement batch after it. Without the option `start` keeps them closed for the run (another agent-build variable the
same file reads), because every run's helm is fresh and the card would otherwise sit over the app and take the run's
first click or key; nothing is marked seen. `--templates` seeds a fixed set of launch templates with the ⚡ mark and
number keys over them (see "Seeded templates"). `--scripted-hosts` gives the app fake SSH hosts (see "Scripted hosts").
`--payload-dir DIR` is for a run that sets up or updates a real remote host: it hands the app's embedded helm a
directory of staged `farhelm` payloads (`farhelm-<target>.tar.gz`, as `scripts/devbuild-mac.sh` stages them) to install
from, as `FARHELM_HELM_PAYLOAD_DIR`, which every build of the app reads. A debug build has no release to download
payloads from, and `start` keeps the caller's own `FARHELM_*` variables out of the app, so this option is the way to
pass it in. DIR must be absolute and an existing directory; `start` refuses anything else before it creates the run.
`--remote-host DEST`, given once per host, makes the run a staged one with remote hosts (see "Staged runs").
`--user-session` (Linux only) gives the app your own runtime directory and session bus, so that its helm's Podman check
works and New container shows; such a run shares your systemd user manager and Podman storage (see "User-session runs").
It builds with whatever `CARGO_TARGET_DIR` you set. If `start` fails or is interrupted, run `stop` anyway: it releases
whatever `start` had already taken, the lock included.

To compare the app with the old app's reference pictures, `scripts/gui-screenshot-batch.py` drives these same commands
over a fixed scene list and writes one browsable page (`docs/gpui/screenshot-batch.md`). `scripts/gui-e2e-smoke.py` is a
committed end-to-end smoke test built on them, and `scripts/gui-lifecycle.py` checks the Linux app's quit, relaunch,
second launch and saved window frame, which end an app run and so cannot be smoke steps (`.agents/validation.md` has
both commands). To relaunch the app on one run it calls the bridge's `launch` again and records the new app in that
run's `run.json` itself, so that `stop` still finds it. That is the lifecycle check's own arrangement, not part of this
harness's interface: `run.json` belongs to `gui-agent.py`, and its layout may change with it; whoever changes it changes
`relaunch` in `gui-lifecycle.py` with it.

## Seeded sessions

Every seeded session is a command launch of the fake agent (`farhelm-fixtures fake-agent`), titled `Fixture 1`,
`Fixture 2` and so on in the order seeded, created through the real helm API. Each runs in `/tmp`, so the folder its row
shows is the same on every run and machine; the transcripts the fleet kinds replay stay in the run directory.
`--sessions` seeds the `basic` script, a prompt that echoes each line typed into it as `echo:<line>`; it is the one to
type into. Its status is whatever the supervisor reads, running while it prints and idle a few seconds later.

`--fleet` seeds sessions in given statuses for screenshots of them. Each kind other than `basic` runs the fake agent's
replay script, which prints a three-line transcript ending in `now holds its <kind> screen` and then holds the screen
the supervisor reads as that status: a spinner line redrawn every second (`running`), a yes/no question (`waiting`),
nothing more (`idle`), or an exit (`exited`). The waiting one is declared a Claude session, because only an agent type's
own reading recognizes a question; the rest are undeclared commands. `start` then waits (up to two minutes) until the
helm lists every one of them in its status, so a scene that follows can rely on it. The statuses are the supervisor's
own reading of each live screen, never written in by the harness. The `spawner` kind is not replayed and has no status
to wait for: it runs the fake agent's spawn script (by its bare binary name, which the session's launch shim puts on the
PATH, so the card that shows the command shows no build path), which prints `FAKE-AGENT READY` and turns each
`spawn <folder>` line typed into it into a real `farhelm spawn` of its own launch, so typing `spawn /tmp` makes an
agent's approval card appear in the window. That launch is a command Farhelm did not compose, which an agent may copy
only on a host that starts YOLO sessions without asking, so seeding a spawner turns that setting on for the run's local
host; the spawn still waits on its card.

## Seeded templates

`start --templates` stores five launch templates in the run's helm through its API (`PUT /api/templates/<name>`), after
any sessions, then sets the ⚡ mark and the number keys over them in one sparse `PUT /api/preferences`. The set is
`SEEDED_TEMPLATES` in `scripts/gui-agent.py`, which says what each is for and which scene it serves. Template and
quick-launch scenes start from it, so their steps only show the surface they are about. They reach the number keys
inside a quick-launch sequence, which reads the keys bare: the chord itself is Ctrl+Shift on Linux and Command on a Mac,
and the bridge cannot dispatch its action by name. No template names the machine: `fixture-run` calls the fake agent by
its bare binary name, which the session's launch shim resolves, and `in-tmp`'s folder is `/tmp`, as for seeded sessions.

## Scripted hosts

`start --scripted-hosts` gives the app fake SSH hosts: its helm runs every command it would run on a host through a
scripted provisioning backend instead of real ssh (`crates/farhelm-helm/src/provisioning/e2e.rs`). That covers setup,
Update and uninstall on SSH hosts, and credential delivery and preparation on environment hosts. Everything else is the
helm's own code (the routes, the one-use plans, registration, progress and the pushed collections), so Add host, setup,
its failures and Update can be driven end to end, including failures at a chosen step and races, which a real host
cannot stage on cue. Only debug builds of the helm read the switch, the same condition that keeps the bridge out of
releases and devbuilds.

What it does not fake is the connection to a host's supervisor: once a fake host is added, the helm dials it over real
ssh like any host, so it shows as unreachable. That is also why a run must use made-up destinations only (the `.example`
domain, say), never a real hostname.

The backend's directory is `<run>/state/scripted-hosts/`, inside the app's state directory because the helm refuses a
backend directory anywhere else. `start` writes its marker (`ENABLED`) and an empty `config.json`, under which every
destination has no supervisor (a probe offers a setup plan) and every setup step succeeds. A step changes what the fake
hosts do with `gui_agent.script_hosts(run, config)`, which replaces `config.json` whole: the backend rereads the file at
every probe and step, so one rewritten in place could be read half-written and fail a step by accident. For example:

```json
{
  "default": { "probe": "absent" },
  "targets": {
    "ssh:failing.example": { "fail_action": "install-farhelm", "message": "disk full" },
    "ssh:running.example": { "probe": "supervisor" },
    "ssh:slow.example": { "action_delay_ms": 1000 }
  }
}
```

`targets` keys are `ssh:<destination>`, `podman:<container>` or `local`, and `default` applies to every other target.
Each entry may set `probe` (`absent`, `supervisor` or `error`), `inspect` (`supported`, `manual` or `error`), `message`
(the text of an injected failure or manual refusal), `build_version`, `identity`, `dial_farhelm` and `dial_state_dir`
(what an answering supervisor reports), `home` and `user_unit_dir`, `github_cli` (whether the host has GitHub's command
line, true by default), `hold_actions` (true holds every step until it is set back to false, for at most 30 seconds),
`fail_action` (the step to fail, by the name the backend records for it in `events.jsonl`, such as `install-farhelm`;
the names are the labels in `e2e.rs`), `action_delay_ms` (a pause before each step, at most 30 seconds) and
`unit_removed` (the supervisor's unit file reported gone and its unit inactive, as after an uninstall that failed past
removing it: an uninstall then plans without a connection, which is the only way a fake host, never connected, can be
uninstalled; without it, uninstall refuses a fake host as not connected). The backend appends one line per probe and
step to `events.jsonl` beside it (`{"event": ..., "target": ...}`), which tells a step what the helm actually did.

## Staged runs

NOTE: A staged remote host is this machine, reached over ssh. Nothing a staged run does may reach the machine's own
Farhelm installation, its real agents or its real account; the arrangement below exists to make sure of that.

A staged run is one with `--remote-host` (and, once it exists, the scenario front end's option, which `staged_start` in
`scripts/gui-agent.py` is where to add it; a scenario with only the local host is staged too). A plain `start` with
neither is not staged and keeps the caller's HOME and PATH, until the private environment below has been tried on a Mac.

`start --remote-host DEST` (repeatable) stages a fleet of several hosts, which the README hero, the demo video and the
docs screenshots need. For each destination `start` starts one more supervisor from the run's `farhelm`, in the run's
own `r0`, `r1`, ... state directories (logs in `logs/r<N>.log`), and writes `ensure-hosts.json5` in the helm's
`--ensure-hosts` format, naming each destination with that build's `farhelm` and that state directory. The app's agent
build hands the file to its embedded helm (`FARHELM_DESKTOP_AGENT_ENSURE_HOSTS`, defined in
`crates/farhelm-desktop/src/agent.rs` and read by the app's startup in `lib.rs`), which registers the hosts before it
serves, so the window shows them from the start. Each destination must be a different spelling of this machine
(`localhost`, `you@localhost`), since the helm keys hosts on the destination string, and must take passwordless ssh:
`start` checks that first and refuses naming the one that failed. Nothing checks that DEST really is this machine; a
real remote would be dialed at this checkout's `farhelm` and a run directory under `/tmp`, neither of which exists
there. `stop` ends those supervisors and their terminal hosts, signalling each only while its pid, start time and state
directory still match the record. Only the agent build reads the variable; no other build of the app registers a host at
startup.

A staged run's sessions are launched by an agent harness's bare name (`claude`, `codex`, and every other program the
helm composes an agent launch with: `STAGED_AGENTS`, which the tests check against
`crates/farhelm-helm/src/launches.rs`), so that their rows read the way a real launch does, and those names must never
reach the vendors' real programs. So a staged run always runs the app, its supervisor, the remote supervisors and every
session with a private HOME and PATH inside the run directory (`staging/home` and `staging/bin`), with `/bin/bash` as
the login shell. The variables that point at the caller's configuration (`XDG_CONFIG_HOME`, `ZDOTDIR` and a few more)
are removed, and so are credentials and vendor settings, by name pattern: anything starting with a vendor's prefix
(`ANTHROPIC_`, `CLAUDE_`, `OPENAI_`, `CODEX_`, `AWS_`, which covers the cloud credentials Claude Code's Bedrock mode
uses, and a few more) or ending in `_KEY`, `_TOKEN`, `_SECRET` or `_PASSWORD`. There is no option to turn any of that
off. `staging/bin` holds a wrapper for every harness name that runs the fake agent: the script `.staged-agent` in the
session's working directory with `sh` when there is one (that is how a scenario gives each session its own replay), and
the basic script otherwise, with every argument passed on. The private HOME's login profile puts the wrappers first on
PATH again after the system's profile, because the supervisor starts sessions through a login shell and a system profile
may rebuild PATH (macOS's `path_helper` does). `start` refuses a build directory that holds a program of a harness's
name, because the launch shim puts the directory of the supervisor's `farhelm` ahead of everything else on a session's
PATH. `scripts/test-gui-agent.py` resolves every name through a staged run's PATH and through the machine's own login
shell and checks each reaches its wrapper.

A staged run also turns `--scripted-hosts` on, with no way to turn it off, also when `scripts/gui-lifecycle.py`
relaunches it (every launch through the bridge, `start`'s and a relaunch's, decides from the run's record;
`gui-lifecycle.py`'s `launch_aside` runs the binary outside the bridge with the caller's own environment, neither staged
nor faking provisioning, which is fine only because the lifecycle check never stages its run): a real setup, Update or
uninstall of a staged host would act on this machine's own Farhelm installation, so the helm fakes all of them. The
connection to each host's supervisor stays real.

Not done yet: the scenario front end (sessions with titles, hosts, statuses and transcripts). The seeded `--sessions`
and `--fleet` sessions run on the local host as before, their command an absolute path to the fake agent. A staged run
on a Mac has not been tried; there the vendors' credentials also live in the login Keychain, which a private HOME does
not hide. On Linux it was checked with two remote hosts.

## User-session runs

NOTE: A user-session run shares your systemd user manager and your account's Podman storage with every other run on the
machine and with the machine's own Farhelm installation. It must never touch a unit, container or process it did not
create, and nothing it does may reach the live install.

On Linux a run normally gives the app a private `XDG_RUNTIME_DIR` (the run's `runtime/`) and the compositor's private
session bus. That keeps the app off your desktop, but it also means the embedded helm's local Podman check fails:
`systemd-run --user` and `systemctl --user` cannot reach your user manager, and `podman info` looks for its run root in
the wrong place. So New container never shows in an ordinary run, and the `app-launcher-new-container` scene could not
work. `start --user-session` is for runs that need containers: it keeps your own `XDG_RUNTIME_DIR` and
`DBUS_SESSION_BUS_ADDRESS` for the app, and through it for its helm and local supervisor. Everything else stays as in an
ordinary run: the run's state directory and `farhelm`, your `FARHELM_*` variables kept out, lavapipe, and Mutter on its
own runtime directory and bus (it registers its service names on the bus it runs on, where they would compete with your
desktop's compositor). The app reaches Mutter by the absolute path of its Wayland socket. `start` refuses the option on
a Mac (a Mac run already uses the GUI session's environment) and when your shell has no absolute `XDG_RUNTIME_DIR` or no
bus address, and records both values, so a relaunch and `stop` use what `start` saw.

It does not combine with a staged run (`--remote-host`). A staged run's HOME is private, so Podman would set up a
storage of its own under it while its run root, under your runtime directory, stays the account's; two storages sharing
one run root share its mount records (`containers/overlay-layers/mountpoints.json`), which could disturb your real
containers. Lifting that needs a Podman storage configuration of the run's own in the private HOME, which nothing writes
yet.

What such a run creates on your account: the supervisor wraps each launch in a transient scope named from the session's
id, which the run's own supervisor minted (`farhelm-<session>-<generation>.scope`, and
`farhelm-<session>-tab-<tab>.scope` for a terminal tab); an ordinary run has none, because its supervisor cannot reach
the user manager. The helm names an environment session's container `farhelm-sess-<helm tag>-...` (labelled
`farhelm.helm=<helm tag>`), recipe images `localhost/farhelm-<helm tag>-...`, and short-lived build helpers
`farhelm-build-<helm tag>-...` and `farhelm-prepare-<helm tag>-...`. The tag derives from the helm database, fresh for
every run, so it is almost certainly the run's own (it is four characters). Podman's own scopes for a container, the
scope the helm runs a container's Podman commands in, and the probe scopes the supervisor and the helm make and stop at
once are not named for the run; a build also pulls the pinned Ubuntu base image into your shared image cache.

`stop` asks the run's helm, before the app quits, for the sessions on the local host and the containers it made there,
and adds their ids and names to `run.json`. Local only: a host added while the run is driven could be this machine's own
installation over ssh, whose sessions' scopes sit on the same user manager. Once the app, its supervisor and its
terminal hosts are down, `stop` runs `podman rm --force --ignore` on each recorded container (which takes Podman's
scopes for it along), lists the user manager's scope units without a pattern, stops by exact name each launch or tab
scope of a recorded session, and reports any still there. Nothing is matched by a `farhelm-*` pattern. If the app is
already gone, `stop` removes what an earlier `stop` recorded, and warns when nothing was (an app that crashed, say: then
look for the run's leftovers by its session ids and helm tag). It does not remove images: delete the run's recipe in the
app (Settings → Recipes) before `stop`, which removes the recipe's images through the helm. A build interrupted by
`stop` can leave a helper container carrying the run's helm tag; only that may be removed by hand.

Other things that go to your session rather than the run's while the option is on: the app's sounds (`pw-play` finds
your PipeWire through the runtime directory, and an app that is SIGKILLed leaves the player's `farhelm-sounds-<uuid>`
directory in your `/run/user/<uid>`, where an ordinary run kept it inside the run directory) and anything the app asks
your session bus for, such as a desktop portal.

## What a run is made of

Everything lives under the run directory: on Linux the compositor's runtime directory and Wayland socket, the bridge's
state and socket (in a 0700 directory, because the socket is unauthenticated), the app's state directory, logs (`logs/`
for Mutter, `agent/app/app.log` for the app) and `run.json`, which records what `stop` needs. The Wayland display name
is random per run, so runs in different directories never meet.

The app is `farhelm-desktop` built with the `agent` cargo feature, which compiles the bridge in. It starts its helm and
supervisor exactly as an installed app does, except that its state directory and its `farhelm` program are the run's
(`FARHELM_DESKTOP_STATE_DIR`, `FARHELM_DESKTOP_FARHELM`). In an agent build it also writes its helm's address and a
credential for its own process to the run's `helm.json`, which is how `start` seeds sessions through the real helm API.

On Linux, rendering is Mesa's software Vulkan driver (lavapipe), as for the screenshot goldens, so you need
`mesa-vulkan-drivers` and `libegl-mesa0`, plus `mutter` and `dbus-run-session`. The compositor runs on its own session
bus, which the app shares, where the supervisor cannot reach your systemd user manager, so sessions start without a
transient scope (the supervisor's ordinary fallback), and the helm's Podman check fails. A user-session run gives the
app your own bus instead (see "User-session runs").

## Why the bridge never ships

The bridge's socket is unauthenticated, and agent sessions run as the same user as the app, so with the bridge in a
build any agent could click Allow on its own approval card. `farhelm-gui` therefore refuses to compile the `agent`
feature into any build with debug assertions off, which covers releases (`--release` and the `dist` profile) and
devbuilds (`scripts/devbuild-mac.sh` builds with `--release`). Nothing else turns the feature on.

## Things to know

- The app connects to its embedded helm and shows the connection's states (connecting, another window took over with
  Take back, the connection-lost notice) and the session list, where each row is a list item holding a button, both
  named by its session's title (`click text:"Fixture 2"` opens that session). Opening a session shows its terminal,
  which takes focus; `terminal-text` reads the focused one, and `type --keys` types into it. Each row's actions button
  is named for its session (`click text:"Session actions for Fixture 2"`) and opens that row's menu, also on a row the
  pointer is not over, where the button is drawn invisible until hovered.
- After a click on a row the pointer stays over it, and the row's tooltip shows in a screenshot taken then; `hover`
  something without one (the list header, `text:"Recently active"`) first.
- Headless Mutter reports the window as inactive forever, so agent builds pin the animation ticker's notion of an active
  window on (SPEC_impl.md, "GPUI cookbook: traps"); otherwise nothing would animate in these runs.
- The same inactive window means no focus or blur listener ever fires in a headless run, and nothing pins that: GPUI
  drops the focus paths it hands those listeners while the window is inactive (`Window::draw` in
  `vendor/gpui-pre/src/window.rs`), and the flag it reads is GPUI's own, which only a vendored patch could pin. Focus
  still moves, so `snapshot`'s `[focused]`, `get focus` and `terminal-text` report it truthfully. What does not happen
  is anything the app does in reaction to focus arriving or leaving: a launcher field's list opening when the field
  takes focus, a list or popover closing because Tab took focus out of it, a tooltip shown for keyboard focus ending
  when focus moves on. A drive that sees none of those has hit this, not a bug; in-process tests are where those
  reactions are checked (SPEC_impl.md, "GPUI cookbook: traps").
- `stop` signals only processes it can tie to the run, and only while each is still the process that was recorded (its
  pid and its start time both match): the app as recorded right after launch, the supervisor through the app's record of
  it plus the state directory on its command line, the compositor as recorded when it started. A `start` whose
  compositor never comes up stops that compositor itself. If the script is killed partway, run `stop`; if `run.json` was
  never written, look for leftovers by the run directory in their command lines.

## On a Mac

The same commands work on a Mac, run on the Mac itself (over SSH is fine) as the user logged in at its console: the app
opens a real window in that GUI session, rendered with Metal, and there is no compositor to start. `start` refuses when
the console belongs to someone else or to no one, because a window could not open. Everything else is the same: the run
directory, the seeded fake-agent sessions, the bridge verbs, `terminal-text` and `stop`; the Linux-only parts (Mutter,
its bus and runtime directory, lavapipe, `--size`) do not apply. macOS limits a socket path to 103 bytes, so the run
directory may be a little shallower than on Linux.

Things that differ from a headless run:

- The window shares the screen with whatever else the session shows, and a process started over SSH is not brought to
  the front, so the window can open behind another app's window. Bridge screenshots are unaffected (the bridge renders
  the window itself), but a capture of the screen is not. A window that is not frontmost also draws at most about 30
  frames a second on macOS 26 (the display link of an inactive app is paced down), so measure frame pacing only with the
  app in front.
- A Mac's one GUI session is shared by every run on it. On a Mac several agents use, pass `--lock FILE` to `start` with
  the file every run there agrees on; on the migration's shared test Mac that is `~/.farhelm-gpui-mac.lock`, the same
  file its builds and measurements lock with `lockf -k`. `start` takes an exclusive `flock` on it before the build
  (waiting up to 30 minutes for another run) and holds it until `stop`, which releases it last, and only once nothing
  else of the run is left: if `stop` reports that something did not stop, the lock stays held until a second `stop`
  succeeds. A `start` killed while it waits leaves nothing holding the lock.
- Building GPUI on a small Mac is heavy: with `--lock`, the build already runs under the lock; cap its job count (for
  example `CARGO_BUILD_JOBS=6`). Do not also wrap `start` in `lockf -k` on the same file: the run would wait on itself.
- macOS may show a privacy or permission dialog (screen recording, accessibility, automation) the first time something
  captures the screen or posts input. The bridge itself needs none of these. If one appears, it is the machine owner's
  to answer; do not click it from a script.
