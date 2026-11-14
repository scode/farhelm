#!/usr/bin/env python3
"""Drive the GPUI desktop app headless on Linux: launch it against a real helm and supervisor, then look and click.

NOTE: Linux only, debug builds only, and never against the live install. Everything a run starts lives under the run
directory you name (which must not exist yet), and `stop` takes all of it down again.

    scripts/gui-agent.py RUN_DIR start [--sessions N] [--size WxH] [--no-build]
    scripts/gui-agent.py RUN_DIR snapshot -i          # any gpui-agent command: click, type, press, wait, screenshot ...
    scripts/gui-agent.py RUN_DIR terminal-text
    scripts/gui-agent.py RUN_DIR stop

`start` builds `farhelm-desktop` with the `agent` feature (the test-automation bridge), the `farhelm` CLI the app
starts its supervisor from, the fake agents (`farhelm-fixtures`) and the bridge's own command line; starts a private
headless Mutter on its own session bus; launches the app on it with its own state directory; and seeds `--sessions`
sessions that run the fake agent, through the real helm API. Rendering is Mesa's software Vulkan driver (lavapipe),
the same as the screenshot goldens. Any other verb goes to the vendored `gpui-agent` command line, already pointed at
this run's app; `gpui-agent --help` and vendor/gpui-agent/README.md list them. docs/gui-agent.md is the guide.

Why Mutter rather than the bridge's own Xvfb: the shipped app is built for Wayland only (the root Cargo.toml's
`gpui_platform` features), so runs use Wayland to match what users get. Agent builds do also compile GPUI's X11
backend (the vendored bridge crate turns it on), but driving that would test a path no shipped build has. Mutter also
exercises the client-side decoration path a GNOME user gets.

Names that could collide with another run (the Wayland display, the bridge socket, the runtime directory) all derive
from the run directory, so parallel runs from different directories never share anything (AGENTS.md, "Sharing the
machine with other agents"). The bridge socket is unauthenticated, so it sits in a 0700 directory.
"""

from __future__ import annotations

import json
import os
import shlex
import signal
import subprocess
import sys
import time
import urllib.request
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Mesa's software drivers, at the paths Debian and Ubuntu install them to (the same ones scripts/check-gui-goldens.py
# uses), so a machine's real GPU, if it has one, is never picked.
LAVAPIPE_ICD = "/usr/share/vulkan/icd.d/lvp_icd.json"
MESA_EGL_VENDOR = "/usr/share/glvnd/egl_vendor.d/50_mesa.json"

# The bridge command line's session name inside this run's private GPUI_AGENT_HOME. One run holds one app, so a fixed
# name is fine: the home directory is what keeps runs apart.
SESSION = "app"

# A Unix socket path holds at most 107 bytes on Linux. A run creates three kinds of socket under its directory: the
# bridge's, the supervisor's, and one per terminal host (named by 8 characters, `NAME_LEN` in the supervisor's
# hosts/names.rs). The longest suffix decides how deep a run directory may be; a socket that does not fit fails deep
# inside the app instead of as a usage error.
SOCKET_SUFFIXES = (f"/agent/{SESSION}/app.sock", "/state/supervisor.sock", "/state/hosts/" + "x" * 8)
LONGEST_SOCKET_SUFFIX = max(SOCKET_SUFFIXES, key=len)
MAX_SOCKET_PATH = 107

# The bridge session's files `stop` removes once the app is down, the same ones the bridge's own `close` removes.
SESSION_FILES = ("app.pid", "xvfb.pid", "app.sock", "socket", "display", "window", "refs.json")

# How long the app gets to answer on its socket: it starts a helm and a supervisor before the window opens.
LAUNCH_TIMEOUT_SECONDS = 120


class UsageError(Exception):
    """A mistake in how the script was called; printed without a traceback."""


# ---------------------------------------------------------------------------------------------------------------------
# Pure helpers (scripts/test-gui-agent.py covers these)


def check_run_dir(run: Path) -> None:
    """Refuse a run directory the run cannot own: relative, or so deep that its deepest socket path would not fit."""
    if not run.is_absolute():
        raise UsageError(f"the run directory must be an absolute path: {run}")
    if len(str(run)) + len(LONGEST_SOCKET_SUFFIX) > MAX_SOCKET_PATH:
        raise UsageError(
            f"the run directory is too deep: the socket {run}{LONGEST_SOCKET_SUFFIX} would exceed the "
            f"{MAX_SOCKET_PATH}-byte Unix socket path limit"
        )


def display_name(token: str) -> str:
    """The run's Wayland display name. Mutter creates it inside the run's own runtime directory."""
    return f"wayland-fh-{token}"


def bridge_env(base: dict[str, str], run: Path, runtime: str, display: str, dbus: str) -> dict[str, str]:
    """The environment for the bridge's command line and, through it, the app.

    Starts from `base` with every display and bridge variable from the caller's session removed, so the app can only
    reach this run's Mutter and session bus, and pins rendering to lavapipe.
    """
    env = {
        key: value
        for key, value in base.items()
        if key not in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DBUS_SESSION_BUS_ADDRESS")
        and not key.startswith("GPUI_AGENT")
        and not key.startswith("FARHELM_")
    }
    env.update(
        {
            "XDG_RUNTIME_DIR": runtime,
            "WAYLAND_DISPLAY": display,
            "DBUS_SESSION_BUS_ADDRESS": dbus,
            "VK_ICD_FILENAMES": LAVAPIPE_ICD,
            "__EGL_VENDOR_LIBRARY_FILENAMES": MESA_EGL_VENDOR,
            "LIBGL_ALWAYS_SOFTWARE": "true",
            "GPUI_AGENT_HOME": str(run / "agent"),
            "GPUI_AGENT_SESSION": SESSION,
        }
    )
    # A debug build logs at debug level by default, tens of thousands of lines a minute from the helm alone; the
    # caller's own RUST_LOG still wins.
    env.setdefault("RUST_LOG", "info")
    return env


def app_env_args(run: Path, binaries: dict[str, str]) -> list[str]:
    """The `--env` arguments `gpui-agent launch` passes to the app only: its state, its CLI and the helm file."""
    pairs = {
        # The app's state directory: never the default one, which could be a real installation's.
        "FARHELM_DESKTOP_STATE_DIR": str(run / "state"),
        # The CLI the app starts its supervisor from: this build's, not one found beside the app or on PATH.
        "FARHELM_DESKTOP_FARHELM": binaries["farhelm"],
        # Where the agent build writes its helm's address and credential, for seeding (crates/farhelm-desktop/src/agent.rs).
        "FARHELM_DESKTOP_AGENT_HELM_FILE": str(run / "helm.json"),
    }
    args: list[str] = []
    for key, value in pairs.items():
        args += ["--env", f"{key}={value}"]
    return args


def seed_body(index: int, cwd: Path, fixtures: str) -> dict:
    """A `POST /api/sessions` body for the `index`th seeded session: a command launch of the basic fake agent."""
    return {
        "cwd": str(cwd),
        "title": f"Fixture {index}",
        "command": {
            "command": f"{shlex.quote(fixtures)} fake-agent --script basic",
            "yolo": False,
            "agent": None,
            "resume": None,
        },
    }


def proc_start(pid: int) -> str | None:
    """A process's start time in clock ticks since boot (field 22 of /proc/<pid>/stat), or None once it is gone.

    Together with the pid it names one process: a pid reused later has a different start time, so nothing here ever
    signals a stranger that inherited a recorded pid.
    """
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
    except (FileNotFoundError, ProcessLookupError):
        return None
    return stat[stat.rindex(")") + 2 :].split(" ")[19]


def running(pid: int, start: str) -> bool:
    """Whether the process `pid`/`start` names is still running: it exists, has that start time, and is not a zombie.

    A process that exited but whose parent has not reaped it yet keeps its /proc entry, start time included, until it is
    reaped; it is gone all the same, and signalling it again changes nothing.
    """
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
    except (FileNotFoundError, ProcessLookupError):
        return False
    fields = stat[stat.rindex(")") + 2 :].split(" ")
    return fields[19] == start and fields[0] not in ("Z", "X")


# ---------------------------------------------------------------------------------------------------------------------
# Processes


def descendants(root: int) -> list[int]:
    """Every live process under `root`, found through parent links in /proc (root included while it lives)."""
    parents: dict[int, int] = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text()
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        parents[int(entry.name)] = int(stat[stat.rindex(")") + 2 :].split(" ")[1])
    tree = {root} if root in parents else set()
    grew = True
    while grew:
        grew = False
        for pid, parent in parents.items():
            if parent in tree and pid not in tree:
                tree.add(pid)
                grew = True
    return sorted(tree)


def stop_process(pid: int, start: str, grace: float) -> bool:
    """SIGTERM the process `pid`/`start` names, then SIGKILL it after `grace` seconds. True when it is gone."""
    if not running(pid, start):
        return True
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return True
    deadline = time.monotonic() + grace
    while time.monotonic() < deadline:
        if not running(pid, start):
            return True
        time.sleep(0.1)  # sleep-ok: polling a foreign process for exit; there is no event to wait on
    if not running(pid, start):
        return True
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        return True
    time.sleep(0.2)  # sleep-ok: let the kernel finish the killed process before checking
    return not running(pid, start)


def stop_tree(root: int, root_start: str | None, grace: float) -> list[int]:
    """Stop `root` and everything under it, deepest first, while `root` is still the process `root_start` names.

    Each process's identity is read before any signal goes out, so a pid that exits and is reused meanwhile is left
    alone. Returns the pids that did not stop.
    """
    if root_start is None or proc_start(root) != root_start:
        return []
    tree = [(pid, proc_start(pid)) for pid in descendants(root)]
    return [pid for pid, ticks in reversed(tree) if ticks and not stop_process(pid, ticks, grace)]


def app_identity(run: Path) -> dict | None:
    """The app's pid and start time, from the pid file the bridge wrote at launch, or None if it is not running."""
    try:
        pid = int((run / "agent" / SESSION / "app.pid").read_text().strip())
    except (FileNotFoundError, ValueError):
        return None
    ticks = proc_start(pid)
    return {"pid": pid, "start": ticks} if ticks else None


def supervisor_to_stop(state: Path) -> tuple[int, str] | None:
    """The run's supervisor as `(pid, start)`, or None when there is none this run can claim.

    The app records the supervisor it manages in `desktop-supervisor.json`; that pid counts as the run's only while its
    command line names the run's state directory, so a pid reused by an unrelated process is never claimed.
    """
    try:
        pid = json.loads((state / "desktop-supervisor.json").read_text())["pid"]
    except (FileNotFoundError, KeyError, ValueError):
        return None
    ticks = proc_start(pid)
    if not ticks:
        return None
    try:
        argv = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
    except (FileNotFoundError, ProcessLookupError):
        return None
    return (pid, ticks) if str(state).encode() in argv else None


def wait_for(path: Path, seconds: float, what: str) -> None:
    """Wait until `path` exists, or fail naming `what`."""
    deadline = time.monotonic() + seconds
    while not path.exists():
        if time.monotonic() > deadline:
            raise RuntimeError(f"{what} did not appear at {path} within {seconds:.0f} s")
        time.sleep(0.1)  # sleep-ok: polling for a file another process creates


# ---------------------------------------------------------------------------------------------------------------------
# Verbs


def build(target: str) -> None:
    """Build the four programs a run needs into `target`."""
    subprocess.run(
        ["cargo", "build", "--locked", "-p", "farhelm-desktop", "--features", "agent", "-p", "farhelm", "-p",
         "farhelm-fixtures"],
        cwd=REPO,
        check=True,
    )
    # The bridge's command line has its own workspace and lockfile under vendor/; building it from the checkout root
    # uses the repository's toolchain (vendor/README.md, "Staying out of the workspace's checks").
    subprocess.run(
        ["cargo", "build", "--locked", "--manifest-path", "vendor/gpui-agent/Cargo.toml", "-p", "gpui-agent-cli",
         "--target-dir", target],
        cwd=REPO,
        check=True,
    )


def target_dir() -> str:
    """The workspace's target directory as cargo resolves it (CARGO_TARGET_DIR included)."""
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
        cwd=REPO,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)["target_directory"]


def mutter_command(display: str, size: str) -> list[str]:
    """The command that runs the run's compositor: headless Mutter, Wayland only, on a session bus of its own."""
    return ["dbus-run-session", "--", "mutter", "--headless", "--wayland", "--no-x11", "--virtual-monitor", size,
            "--wayland-display", display]


def start_mutter(
    run: Path,
    runtime: Path,
    display: str,
    command: list[str],
    base: dict[str, str],
    socket_wait: float = 30,
) -> dict:
    """Start the run's compositor (`command`, normally [`mutter_command`]); return what the app needs to join it.

    `dbus-run-session` gives Mutter a bus no desktop shares, and the bus address is read from Mutter's own environment
    because dbus-run-session does not print it. Under that bus the supervisor cannot reach the user's systemd manager,
    so its terminal hosts start without a transient scope (the supervisor's ordinary fallback).

    If the compositor does not come up (no Wayland socket within `socket_wait` seconds, or no bus address), everything
    it started is stopped before the error propagates: `run.json` does not exist yet, so `stop` could not find it.
    """
    env = {k: v for k, v in base.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS")}
    env["XDG_RUNTIME_DIR"] = str(runtime)
    logs = run / "logs"
    child = subprocess.Popen(
        command,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=(logs / "mutter.stdout").open("wb"),
        stderr=(logs / "mutter.stderr").open("wb"),
        start_new_session=True,
    )
    record = {"pid": child.pid, "start": proc_start(child.pid)}
    try:
        wait_for(runtime / display, socket_wait, "Mutter's Wayland socket")
        for pid in descendants(child.pid):
            try:
                argv = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
                if argv[0].endswith(b"mutter"):
                    environ = Path(f"/proc/{pid}/environ").read_bytes().split(b"\0")
                    for line in environ:
                        if line.startswith(b"DBUS_SESSION_BUS_ADDRESS="):
                            record["dbus"] = line.split(b"=", 1)[1].decode()
            except (FileNotFoundError, ProcessLookupError):
                continue
        if "dbus" not in record:
            raise RuntimeError(f"no session bus address in Mutter's environment; see {logs}/mutter.stderr")
    except BaseException:
        stop_tree(record["pid"], record["start"], 5)
        child.wait()
        raise
    return record


def seed_sessions(run: Path, count: int, fixtures: str) -> list[str]:
    """Create `count` fake-agent sessions through the embedded helm's API; return their ids."""
    wait_for(run / "helm.json", 30, "the app's helm file")
    helm = json.loads((run / "helm.json").read_text())
    work = run / "work"
    work.mkdir(exist_ok=True)
    ids = []
    for index in range(1, count + 1):
        request = urllib.request.Request(
            f"{helm['api_base']}/api/sessions",
            data=json.dumps(seed_body(index, work, fixtures)).encode(),
            headers={"Authorization": f"Bearer {helm['credential']}", "Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=60) as response:
            ids.append(json.loads(response.read())["id"])
    return ids


def cli(run: Path, record: dict, args: list[str], capture: bool = False) -> subprocess.CompletedProcess:
    """Run the bridge's command line against this run's app."""
    env = bridge_env(dict(os.environ), run, record["runtime"], record["display"], record["dbus"])
    return subprocess.run([record["binaries"]["cli"], *args], env=env, capture_output=capture, text=True)


def start(run: Path, args: list[str]) -> int:
    import argparse

    parser = argparse.ArgumentParser(prog="gui-agent.py RUN_DIR start")
    parser.add_argument("--sessions", type=int, default=2, help="fake-agent sessions to seed (default 2)")
    parser.add_argument("--size", default="1600x1200", help="Mutter's virtual monitor (default 1600x1200)")
    parser.add_argument("--no-build", action="store_true", help="use the binaries already in the target directory")
    options = parser.parse_args(args)
    if not sys.platform.startswith("linux"):
        raise UsageError("headless runs need Linux; on a Mac, see docs/gui-agent.md")
    missing = [path for path in (LAVAPIPE_ICD, MESA_EGL_VENDOR) if not Path(path).exists()]
    if missing:
        raise UsageError(f"the software renderer is not installed: {', '.join(missing)} missing")
    run.mkdir(mode=0o700)
    for sub in ("runtime", "agent", "logs"):
        (run / sub).mkdir(mode=0o700)
    target = target_dir()
    if not options.no_build:
        build(target)
    binaries = {
        "desktop": f"{target}/debug/farhelm-desktop",
        "farhelm": f"{target}/debug/farhelm",
        "fixtures": f"{target}/debug/farhelm-fixtures",
        "cli": f"{target}/debug/gpui-agent",
    }
    display = display_name(uuid.uuid4().hex[:8])
    record = {"runtime": str(run / "runtime"), "display": display, "binaries": binaries, "state": str(run / "state")}
    mutter = start_mutter(run, run / "runtime", display, mutter_command(display, options.size), dict(os.environ))
    record.update({"mutter": {"pid": mutter["pid"], "start": mutter["start"]}, "dbus": mutter["dbus"]})
    (run / "run.json").write_text(json.dumps(record, indent=2))
    launched = cli(
        run,
        record,
        ["launch", "--timeout", str(LAUNCH_TIMEOUT_SECONDS), *app_env_args(run, binaries), "--", binaries["desktop"]],
    )
    # The app's identity, recorded whether or not the launch succeeded (a launch that timed out can leave the app
    # running), so `stop` signals it only while that pid is still this app.
    record["app"] = app_identity(run)
    (run / "run.json").write_text(json.dumps(record, indent=2))
    if launched.returncode != 0:
        print(f"the app did not start; its log is {run}/agent/{SESSION}/app.log. Run `stop` to clean up.", file=sys.stderr)
        return launched.returncode
    if options.sessions:
        ids = seed_sessions(run, options.sessions, binaries["fixtures"])
        print(f"seeded {len(ids)} fake-agent sessions: {', '.join(ids)}")
    return 0


def terminal_text(run: Path, record: dict) -> int:
    """Print the focused terminal's text through the app's `terminal_text` probe, which the terminal registers."""
    result = cli(run, record, ["get", "probe", "terminal_text"], capture=True)
    if result.returncode != 0:
        print(f"no terminal yet (the app has no `terminal_text` probe): {result.stderr.strip()}", file=sys.stderr)
        return 1
    print(result.stdout, end="")
    return 0


def stop(run: Path, record: dict) -> int:
    """Take down everything `start` started: the app, its supervisor and terminal hosts, then Mutter and its bus.

    Each step runs even if an earlier one failed, and each acts only on processes it can tie to this run: the app
    through the pid and start time recorded after launch, the supervisor through the app's record of it plus the state
    directory on its command line, Mutter through the pid and start time recorded when it started. A pid that has since
    been reused by another process is never signalled.
    """
    problems = []
    # The app first, the ordinary way: SIGTERM, which the app turns into a quit (it then waits for its helm to stop),
    # and SIGKILL if it is still there after the grace. Not the bridge's `close`, which signals the recorded pid and its
    # process group without checking that the pid is still the app.
    app = record.get("app")
    if app and not stop_process(app["pid"], app["start"], 15):
        problems.append(f"the app (pid {app['pid']}) did not stop")
    for name in SESSION_FILES:
        (run / "agent" / SESSION / name).unlink(missing_ok=True)
    state = Path(record["state"])
    supervisor = supervisor_to_stop(state)
    if supervisor and not stop_process(supervisor[0], supervisor[1], 20):
        problems.append(f"the supervisor (pid {supervisor[0]}) did not stop")
    if (state / "hosts").exists():
        hosts = subprocess.run(
            [record["binaries"]["farhelm"], "internal", "stop-terminal-hosts", "--state-dir", str(state)]
        )
        if hosts.returncode != 0:
            problems.append("stop-terminal-hosts could not confirm every terminal host stopped")
    mutter = record.get("mutter")
    if mutter:
        for pid in stop_tree(mutter["pid"], mutter["start"], 10):
            problems.append(f"compositor process {pid} did not stop")
    for problem in problems:
        print(f"gui-agent: {problem}", file=sys.stderr)
    return 1 if problems else 0


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0 if argv[:1] in (["-h"], ["--help"]) else 2
    run, verb, rest = Path(argv[0]), argv[1], argv[2:]
    try:
        check_run_dir(run)
        if verb == "start":
            return start(run, rest)
        record = json.loads((run / "run.json").read_text())
        if verb == "stop":
            return stop(run, record)
        if verb == "terminal-text":
            return terminal_text(run, record)
        return cli(run, record, [verb, *rest]).returncode
    except UsageError as error:
        print(f"gui-agent: {error}", file=sys.stderr)
        return 2
    except FileNotFoundError as error:
        print(f"gui-agent: {error}; was `start` run for this directory?", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
