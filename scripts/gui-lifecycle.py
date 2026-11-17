#!/usr/bin/env python3
"""End-to-end lifecycle check of the Linux desktop app: boot, quit, relaunch, second launch, the saved window frame and
a replaced or refused program file, on headless Mutter, through the run machinery of `scripts/gui-agent.py` (`docs/gui-agent.md`).

NOTE: This is a script of its own, not steps in `scripts/gui-e2e-smoke.py`, because a quit or a relaunch ends an app
run, and the smoke's steps all share one. Here every step leaves the app where the next one begins (running, quit or
relaunched on the same state directory), so the steps run in order and the first failure ends the run: a later step
would only fail for the earlier one's reason.

    python3 -B scripts/gui-lifecycle.py [--no-build] [--keep]

Each step's docstring says what it checks and why; every outcome is read where it really lands (the process table, the
state directory's files, the helm's API, the app's own log, the window the bridge reports). "The supervisor" is always
the process `desktop-supervisor.json` in the state directory names, counted only while its command line names that
state directory and its start time is the one first seen, so a reused pid can never pass for it.

Which environments count as having no Wayland session is the unit test
`a_linux_launch_without_wayland_refuses_with_a_clear_line` in `crates/farhelm-desktop`; step 03 here only shows that a
launch really refuses before it starts anything. The supervisor replace and reuse decisions are unit tests in
`crates/farhelm-desktop` too (`docs/gpui/parity/desktop-integration-and-linux-packaging.md` lists them); steps 11
and 12 here show a replacement end to end and one refused before it stops anything.

The run directory is made under /tmp, as the smoke's is, deleted when every step passed and the run stopped cleanly,
and kept for its logs otherwise (each launch's app log is kept as `logs/app-launch-<n>.log`); `--keep` keeps it either
way. `--no-build` reuses the binaries already built. Ctrl-C or SIGTERM ends the steps and still stops the app run.
Needs what `gui-agent.py start` needs on Linux: `mutter`, `dbus-run-session` and lavapipe. Run it through
`scripts/record-test-run.py` (`.agents/validation.md` has the command).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
AGENT = REPO / "scripts/gui-agent.py"

# The harness itself, for its run machinery: launching through the bridge's command line with the run's app
# environment, process identity by pid and start time, and the helm request helper, rather than a second copy of them.
spec = importlib.util.spec_from_file_location("gui_agent", AGENT)
gui_agent = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(gui_agent)

# Where runs go: a short path, so the run's deepest socket fits the platform's limit (`gui-agent.py`'s `check_run_dir`).
RUNS_PARENT = Path("/tmp")

# How long one bridge command may take before it counts as hung (the bridge's own waits give up after 25 seconds).
STEP_TIMEOUT_SECONDS = 120

# How long a step waits for an effect it polls for itself: an app's exit, a log line, a written file.
EFFECT_TIMEOUT_SECONDS = 30

# The one session `start` seeds, a basic fake agent. Its row is drawn only once the window has read the helm's listing
# over its UI channel, so it shows that the window is up and connected, not merely open; and as a session the supervisor
# holds, it is what a relaunch must find again.
SEEDED_TITLE = "Fixture 1"

# The second launch's own line (`run` in crates/farhelm-desktop/src/lib.rs) and the open app's log line for it
# (`follow_activations_with` in crates/gui/farhelm-gui/src/desktop.rs).
FORWARDED_LINE = "Farhelm is already open; asked it to bring its window forward"
ACTIVATION_LOG = "a second launch asked to bring the window forward"

# The colour codes the app's log formatter may wrap a field's name and its `=` in.
ANSI = re.compile(r"\x1b\[[0-9;]*m")

# What the app logs once its embedded helm has stopped after a quit (`run`): an orderly quit, not a kill.
QUIT_LOG = "the embedded helm stopped"

# The ordinary frame step 09 saves before a relaunch, and step 10 expects kept: smaller than the default 1100 by 760
# first frame and not its aspect, so a window opened at the default can never pass for it, and above the 720 by 560
# minimum.
SAVED_SIZE = (900, 650)

# Set by `main` once the app is up: the run directory; and by step 01, the supervisor as `(pid, start)` and the seeded
# session's id.
RUN: Path | None = None
SUPERVISOR: tuple[int, str] | None = None
SESSION_ID: str | None = None

# How many times the app has been launched in this run; `relaunch` names the kept log of each earlier launch by it.
LAUNCHES = 1


# ---------------------------------------------------------------------------------------------------------------------
# Helpers


def agent(*args: str, check: bool = True) -> str:
    """One `gui-agent.py` command against this run. With `check` it must succeed, and its stdout alone comes back for
    the caller to parse; without, stdout and stderr together come back, for a failure message to quote."""
    result = subprocess.run(
        [sys.executable, str(AGENT), str(RUN), *args], capture_output=True, text=True, timeout=STEP_TIMEOUT_SECONDS
    )
    if check and result.returncode != 0:
        raise AssertionError(f"`{' '.join(args)}` failed:\n{result.stdout}{result.stderr}")
    return result.stdout if check else result.stdout + result.stderr


def record() -> dict:
    """The run's `run.json`: what `stop` needs, and what the bridge's command line needs to reach the app."""
    return json.loads((RUN / "run.json").read_text())


def state() -> Path:
    """The app's state directory, the same one for every launch of this run."""
    return RUN / "state"


def eventually(check, what: str, timeout: float = EFFECT_TIMEOUT_SECONDS):
    """`check()`'s first truthy answer, asked again every quarter second, or a failure naming `what` after
    `timeout`."""
    deadline = time.monotonic() + timeout
    while True:
        answer = check()
        if answer:
            return answer
        if time.monotonic() > deadline:
            raise AssertionError(f"{what} did not happen within {timeout:.0f} s")
        time.sleep(0.25)  # sleep-ok: polling a process, a file or the helm; none offers a wait on this


def app_log() -> str:
    """The running (or last) launch's log: the bridge sends the app's stdout and stderr there, and truncates it at the
    next launch."""
    return (RUN / "agent" / gui_agent.SESSION / "app.log").read_text(errors="replace")


def assert_supervisor_kept(when: str) -> None:
    """The supervisor step 01 saw is still running, and the state directory's record still names it."""
    assert SUPERVISOR is not None
    now = gui_agent.supervisor_to_stop(state())
    if now != SUPERVISOR or not gui_agent.running(*SUPERVISOR):
        raise AssertionError(f"{when}: the supervisor {SUPERVISOR} is not the one recorded and running ({now})")


def local_host_connected() -> bool:
    """Whether the helm lists its local host as connected: the helm reached the local supervisor, which the window's
    listing alone does not show (the old smoke test's boot leg asked the same question)."""
    hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
    return any(host["kind"] == "local" and host["state"]["phase"] == "connected" for host in hosts)


# The statuses of a session whose agent is alive. The basic fake agent never exits on its own, so a kept session is in
# one of these; `interrupted` (its terminal lost), `error`, `unknown`, `exited` or no status at all are not "kept".
LIVE_STATES = ("running", "waiting", "idle")


def assert_session_kept(when: str) -> None:
    """The session step 01 saw is still listed by the helm under the same id, with its agent alive: it lives in the
    supervisor, so an app that kept the supervisor keeps it. Asked until it holds: right after a relaunch the helm may
    still list its cached copy or a status not yet read, and `relaunch` has only waited for the helm to reach the
    supervisor, not for the supervisor's next reading of the session."""

    def kept() -> bool:
        return gui_agent.listed_states(RUN).get(SESSION_ID) in LIVE_STATES

    eventually(kept, f"{when}: the helm listing the seeded session {SESSION_ID} as {', '.join(LIVE_STATES)}")


def assert_session_takes_typing(when: str) -> None:
    """The seeded session's terminal still reaches its program, through the window, the helm and the supervisor: a
    session listed live and whose terminal host still listens can still be one nothing can type into. Its terminal is
    focused (by a click on its row, unless a relaunch reopened the session and its terminal already holds focus: a click
    on the title then could land on another copy of it) and shows the fake agent's ready line once it has caught up; a
    line typed there, stamped so an earlier echo cannot pass for it, comes back as the agent's `echo:` of it, as in
    `gui-e2e-smoke.py`'s step 02."""
    try:
        if "no terminal" in agent("terminal-text", check=False):
            agent("click", f"text:{SEEDED_TITLE}")
        agent("terminal-text", "--wait", "FAKE-AGENT READY")
        line = f"typed-{os.urandom(4).hex()}"
        agent("type", "--keys", line)
        agent("press", "enter")
        agent("terminal-text", "--wait", f"echo:{line}")
    except AssertionError as error:
        raise AssertionError(f"{when}: typing into the seeded session: {error}") from None


def quit_socket() -> Path:
    """The second-launch socket in the state directory."""
    return state() / "desktop.sock"


def answers(path: Path) -> bool:
    """Whether something accepts connections at the Unix socket `path`. A socket file a killed app left behind refuses
    them, so only a live listener answers. A connection counts as a token-less second launch, which only brings the
    window forward."""
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        try:
            client.connect(str(path))
        except (ConnectionRefusedError, FileNotFoundError):
            return False
    return True


def window_size() -> tuple[int, int]:
    """The main window's size as the bridge reports it: GPUI's viewport size, the same measure the frame record keeps
    (`observed` in crates/gui/farhelm-gui/src/desktop.rs)."""
    # Not picked by title: on Wayland GPUI reports every window's title as empty. The app has one window here.
    windows = json.loads(agent("windows", "--json"))
    if len(windows) != 1:
        raise AssertionError(f"expected the app's one window, the bridge lists {windows}")
    return round(windows[0]["width"]), round(windows[0]["height"])


def frame_record() -> dict | None:
    """The saved window frame (`desktop-window.json`), or None while there is none."""
    try:
        return json.loads((state() / "desktop-window.json").read_text())
    except FileNotFoundError:
        return None


def write_frame_record(size: tuple[int, int]) -> None:
    """Save an ordinary, not maximized, frame for the next launch in the app's own format (version 3, logical pixels;
    `window_state.rs`). The position is a placeholder: on Wayland none is restored."""
    body = {"version": 3, "normal": {"x": 0, "y": 0, "width": size[0], "height": size[1]}, "maximized": False}
    (state() / "desktop-window.json").write_text(json.dumps(body))


def eventually_framed(check, what: str, also: str = "") -> None:
    """`eventually` for a step about the saved frame, whose failure also reports the record and the window as they
    are then (and `also`), so a timeout tells "the window did not change" from "the app did not save the change"."""
    try:
        eventually(check, what)
    except AssertionError as error:
        try:
            window = "x".join(map(str, window_size()))
        except AssertionError as no_window:
            window = f"unreadable ({no_window})"
        raise AssertionError(f"{error}: the record is {frame_record()}, the window {window}{also}") from None


def helm_status(helm: dict, path: str) -> int:
    """The HTTP status the embedded helm answers `path` with, under the credential in `helm` (a `helm.json` body) and
    at its `api_base`."""
    request = urllib.request.Request(
        f"{helm['api_base']}{path}", headers={"Authorization": f"Bearer {helm['credential']}"}
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status
    except urllib.error.HTTPError as error:
        return error.code


def wait_for_exit(app: dict, how: str, detail: str = "") -> None:
    """Wait for the app `app` (`run.json`'s record) to exit, never escalating: an app that ignores a quit fails here
    rather than being killed into passing."""
    try:
        eventually(lambda: not gui_agent.running(app["pid"], app["start"]), f"the app's exit on {how}")
    except AssertionError as error:
        raise AssertionError(f"{error}{detail}") from None


def quit_app(by_close_button: bool) -> None:
    """Quit the running app the orderly way, by the Linux title bar's Close or by SIGTERM, and check what every quit
    must leave: the app's log says its embedded helm stopped (not a kill), the second-launch socket is gone, so a later
    launch starts normally, and the supervisor still runs, so sessions keep running while no app is open."""
    app = record()["app"]
    if by_close_button:
        # Not checked: the app may quit before the bridge has answered the click. The exit below is the check.
        clicked = agent("click", "id:title-bar-close", check=False)
        wait_for_exit(app, "a click on the title bar's Close", f"; the click said: {clicked}")
    else:
        os.kill(app["pid"], signal.SIGTERM)
        wait_for_exit(app, "SIGTERM")
    if QUIT_LOG not in app_log():
        raise AssertionError(f"the app's log does not say {QUIT_LOG!r}: it did not quit the orderly way")
    if quit_socket().exists():
        raise AssertionError(f"{quit_socket()} is still there after the quit")
    assert_supervisor_kept("after a quit")


def launch_aside(state_dir: Path, drop: tuple[str, ...] = (), extra: dict | None = None) -> subprocess.CompletedProcess:
    """Run the app's binary beside the open app, on `state_dir`, outside the bridge, and wait for it to exit: the
    run's environment without the variables in `drop`, plus `extra`. The environment is built for the child alone;
    this process's own is never changed."""
    run = record()
    env = gui_agent.bridge_env(dict(os.environ), RUN, run["runtime"], run["display"], run["dbus"])
    for name in drop:
        env.pop(name, None)
    env.update({"FARHELM_DESKTOP_STATE_DIR": str(state_dir), "FARHELM_DESKTOP_FARHELM": run["binaries"]["farhelm"]})
    env.update(extra or {})
    return subprocess.run(
        [run["binaries"]["desktop"]], env=env, capture_output=True, text=True, timeout=STEP_TIMEOUT_SECONDS
    )


def relaunch() -> None:
    """Launch the app again on the run's state directory, the way `gui-agent.py start` did, and record the new app in
    `run.json` so `stop` still finds it.

    The previous launch's log is kept first (the bridge truncates it), and its `helm.json` removed: the agent build
    refuses to overwrite one, since a file already there would belong to another run's harness. Its `app.pid` goes too:
    the previous app is gone by now, and a pid file naming a pid since reused would make the bridge refuse ("already
    runs an app") and leave that stranger's pid to be recorded as the app, for `stop` to signal."""
    global LAUNCHES
    log = RUN / "agent" / gui_agent.SESSION / "app.log"
    if log.exists():
        shutil.copy(log, RUN / "logs" / f"app-launch-{LAUNCHES}.log")
    LAUNCHES += 1
    (RUN / "helm.json").unlink(missing_ok=True)
    (RUN / "agent" / gui_agent.SESSION / "app.pid").unlink(missing_ok=True)
    run = record()
    try:
        launched = gui_agent.cli(
            RUN,
            run,
            [
                "launch",
                "--timeout",
                str(gui_agent.LAUNCH_TIMEOUT_SECONDS),
                *gui_agent.launch_env_args(RUN, run),
                "--",
                run["binaries"]["desktop"],
            ],
            capture=True,
        )
    finally:
        # Recorded however the launch ended, as `start` does: one that timed out, or was interrupted after the bridge
        # spawned the app, can leave the app running, and `stop` finds it only through this record.
        run["app"] = gui_agent.app_identity(RUN)
        (RUN / "run.json").write_text(json.dumps(run, indent=2))
    if launched.returncode != 0:
        raise AssertionError(f"the relaunch failed:\n{launched.stdout}{launched.stderr}")
    agent("wait", "--text", SEEDED_TITLE)
    eventually(local_host_connected, "the relaunched helm reaching the local supervisor")


# ---------------------------------------------------------------------------------------------------------------------
# Steps, in order


def step_01_the_app_boots_and_connects() -> None:
    """Everything after this step relaunches into the state a first launch leaves, so it must be the real one.
    Specified: on a fresh state directory the window draws the seeded session's row, which it does only once its UI
    channel to the embedded helm has read the listing; the helm lists its local host as connected and the session as
    alive (the premise steps 07 and 08 rely on); and the app has started a local supervisor, recorded it in
    `desktop-supervisor.json` and left it running on the run's state directory."""
    global SUPERVISOR, SESSION_ID
    agent("wait", "--text", SEEDED_TITLE)
    eventually(local_host_connected, "the helm reaching the local supervisor")
    SUPERVISOR = gui_agent.supervisor_to_stop(state())
    if SUPERVISOR is None:
        raise AssertionError("no running supervisor of this state directory is recorded in desktop-supervisor.json")
    rows = gui_agent.helm_request(RUN, "GET", "/api/sessions")["sessions"]
    ids = [row["id"] for row in rows if row["title"] == SEEDED_TITLE]
    if len(ids) != 1:
        raise AssertionError(f"premise: the helm lists one {SEEDED_TITLE!r}, it lists {rows}")
    SESSION_ID = ids[0]
    assert_session_kept("premise: after start")


def step_02_a_bootstrap_failure_refuses_plainly() -> None:
    """A launch that cannot start (here, its state directory is a plain file) has no window to show why, so its one
    stderr line and its exit status are the whole report a launcher gets; the old native smoke test's first leg.
    Specified: the app exits 1 with a `farhelm-desktop: ` line and no panic, and the open app's supervisor is
    untouched (the failure comes before the app lock, the second-launch socket and any supervisor decision)."""
    bogus = RUN / "work" / "not-a-directory"
    bogus.parent.mkdir(exist_ok=True)
    bogus.write_text("")
    result = launch_aside(bogus)
    # Any line, not the first: the app's log lines (RUST_LOG=info) share stderr and may come before the refusal.
    refused = any(line.startswith("farhelm-desktop: ") for line in result.stderr.splitlines())
    if result.returncode != 1 or not refused or "panicked" in result.stderr:
        raise AssertionError(f"expected exit 1 and one plain line, got {result.returncode}:\n{result.stderr}")
    assert_supervisor_kept("after a failed launch beside the open app")


def step_03_a_launch_without_wayland_refuses_before_starting_anything() -> None:
    """Shipped Linux builds draw only through Wayland, and without a Wayland display GPUI starts headless and opens a
    window nobody sees, so a launch there must refuse, and before it has started a helm or a supervisor. Which
    environments count is the unit test's; this step shows the refusal is wired in where it must be. Specified: the
    binary launched with no `WAYLAND_DISPLAY` (and no `DISPLAY`) on a state directory that does not exist exits 1 with a
    `farhelm-desktop: ` line naming Wayland, and the state directory still does not exist, so nothing was started. The
    launch carries none of the bridge's variables: a launch that wrongly went on would otherwise join the open app's
    bridge session as a second app. Should the refusal regress, the supervisor such a launch starts on its own state
    directory is stopped before the step fails, since the run's `stop` knows only the run's state directory."""
    fresh = RUN / "work" / "no-wayland-state"
    try:
        result = launch_aside(fresh, drop=("WAYLAND_DISPLAY", "DISPLAY", "GPUI_AGENT_HOME", "GPUI_AGENT_SESSION"))
        refused = any(
            line.startswith("farhelm-desktop: ") and "Wayland" in line for line in result.stderr.splitlines()
        )
        if result.returncode != 1 or not refused:
            raise AssertionError(
                f"expected exit 1 and the Wayland refusal, got {result.returncode}:\n{result.stderr}"
            )
        if fresh.exists():
            raise AssertionError(f"the refused launch created its state directory {fresh}: it started before refusing")
    except BaseException:
        leaked = gui_agent.supervisor_to_stop(fresh)
        if leaked and not gui_agent.stop_process(*leaked, 20):
            print(f"gui-lifecycle: the supervisor (pid {leaked[0]}) on {fresh} did not stop", file=sys.stderr)
        raise


def step_04_the_window_frame_is_saved() -> None:
    """A frame that is never written cannot be restored, and the app writes it while it runs (a second after the window
    stops changing), not only at quit, which on macOS gets only GPUI's quit deadline. Specified: with the app left
    alone after its first launch, `desktop-window.json` appears as a version 3 record, not maximized, whose ordinary
    size is the window's size."""
    def saved_as_shown() -> bool:
        saved, size = frame_record(), window_size()
        return bool(saved) and saved == {**saved, "version": 3, "maximized": False} and (
            saved["normal"]["width"], saved["normal"]["height"]) == size

    eventually_framed(saved_as_shown, "a saved frame matching the window")


def step_05_a_second_launch_is_forwarded() -> None:
    """On Linux a second launch on the same state directory used to refuse, which was all a user got for launching an
    app that was already open behind other windows. Specified: with the app open, the same binary launched again on
    the same state directory exits 0 saying the app was asked to come forward (a line only the hand-over prints, before
    it starts anything); the open app logs that a second launch asked it to and is still running; and the supervisor is
    the same one. The launch carries an activation token, as a launcher's would, and the open app's log line says the
    request came with one. It also has no `WAYLAND_DISPLAY` or `DISPLAY`, as from an SSH login: a hand-over needs no
    display, so the app hands over before it refuses a launch without Wayland, and this launch would fail with that
    refusal if the order were reversed."""
    app = record()["app"]
    result = launch_aside(
        state(), drop=("WAYLAND_DISPLAY", "DISPLAY"), extra={"XDG_ACTIVATION_TOKEN": "gui-lifecycle-token"}
    )
    if result.returncode != 0 or FORWARDED_LINE not in result.stderr:
        raise AssertionError(f"the second launch exited {result.returncode}:\n{result.stderr}")
    line = eventually(
        lambda: next((entry for entry in app_log().splitlines() if ACTIVATION_LOG in entry), None),
        "the open app's log of the second launch",
    )
    if "token=true" not in ANSI.sub("", line):
        raise AssertionError(f"the open app logged the second launch without its token: {line!r}")
    if not gui_agent.running(app["pid"], app["start"]):
        raise AssertionError("the open app did not survive the second launch")
    assert_supervisor_kept("after a second launch")


def step_06_a_quit_by_the_close_button_leaves_the_supervisor_running() -> None:
    """The local supervisor is started detached and outlives the app, so sessions keep running while no app is open;
    and on headless Mutter, as on GNOME, the window's own title bar is the user's way to close it. Specified: a click on
    the Linux title bar's Close quits the app the orderly way, with what every quit leaves (`quit_app`). SIGTERM, the
    other ordinary quit, is step 09's."""
    quit_app(by_close_button=True)


def step_07_a_relaunch_reuses_the_supervisor() -> None:
    """A relaunch that started a supervisor of its own would cut every running session off from the one that holds
    them. Specified: a relaunch on the same state directory connects, its helm reaching the local supervisor, with the
    same supervisor recorded and running; the helm lists the seeded session again, not exited; it mints a new helm
    credential, and the previous launch's no longer works."""
    before = json.loads((RUN / "helm.json").read_text())
    relaunch()
    assert_supervisor_kept("after a relaunch")
    assert_session_kept("after a relaunch")
    after = json.loads((RUN / "helm.json").read_text())
    if after["credential"] == before["credential"]:
        raise AssertionError("the relaunch reused the previous launch's helm credential")
    stale = {**after, "credential": before["credential"]}
    if helm_status(after, "/api/hosts") != 200 or helm_status(stale, "/api/hosts") != 401:
        raise AssertionError("the new credential is not the only one the relaunched helm accepts")


def step_08_a_relaunch_after_a_kill_reuses_the_supervisor() -> None:
    """A crash or a kill runs no quit handler: the supervisor is not told anything and a stale second-launch socket
    stays behind. Specified: after SIGKILL the supervisor still runs and the socket file is still there, soon refusing
    connections; a relaunch is not taken for a second launch, connects, reuses the same supervisor, lists the seeded
    session again, and serves a live socket in place of the stale one; and the kept session's terminal, opened in the
    relaunched window, takes typing (`assert_session_takes_typing`), which a listing alone cannot show.

    "Soon", because `gui_agent.running` calls the app gone once its main thread is a zombie, and a SIGKILLed process's
    other threads can still be tearing down then: its open files, the listening socket among them, close only when the
    last thread exits. Under heavy load that took long enough for a single check to find the socket still accepting
    (FLAKES.md, the 2026-11-16 entry for this step), so the premise waits for the refusal instead. A wait that runs out
    says whether the killed process was still exiting then (`threads_left`), which is the premise not settling, or was
    gone, which means something else keeps the listener open: a real fault, not this race."""
    app = record()["app"]
    os.kill(app["pid"], signal.SIGKILL)
    wait_for_exit(app, "SIGKILL")
    assert_supervisor_kept("after the app was killed")
    try:
        eventually(lambda: not answers(quit_socket()), "premise: the killed app's socket refusing connections")
    except AssertionError as error:
        raise AssertionError(f"{error}; the killed app (pid {app['pid']}) {threads_left(app['pid'])}") from None
    # `answers` reads a missing file as refusing, so the refusal means a stale socket only if the file is there.
    if not quit_socket().exists():
        raise AssertionError("premise: after the kill, the socket file is not there refusing connections")
    relaunch()
    assert_supervisor_kept("after a relaunch following a kill")
    assert_session_kept("after a relaunch following a kill")
    if not answers(quit_socket()):
        raise AssertionError("the relaunched app left the stale socket in place: nothing answers at it")
    assert_session_takes_typing("after a relaunch following a kill")


def threads_left(pid: int) -> str:
    """What remains of the process `pid` as /proc shows it, for a failure message: its live thread count, or that it is
    gone. Two or more threads mean it was still exiting; one, or none, that something else holds what it left behind."""
    try:
        return f"has {len(os.listdir(f'/proc/{pid}/task'))} thread(s) left"
    except OSError:
        return "is gone"


def step_09_a_saved_size_is_restored() -> None:
    """The saved frame is what makes the window reopen as the user left it; on Wayland only its size (and whether it was
    maximized) can come back, since the compositor reports no positions. The harness cannot resize the window on
    headless Mutter (its only window commands act on the active window, which headless Mutter never reports), so the
    record is written between launches instead. Specified: SIGTERM quits the app the orderly way, with what every quit
    leaves (`quit_app`); a 900 by 650 ordinary frame saved then reopens the window at 900 by 650."""
    quit_app(by_close_button=False)
    write_frame_record(SAVED_SIZE)
    relaunch()
    if window_size() != SAVED_SIZE:
        raise AssertionError(f"the window opened at {window_size()}, not the saved {SAVED_SIZE}")


def step_10_a_maximized_window_reopens_maximized() -> None:
    """A maximized window must reopen maximized, and the ordinary frame from before it must survive, so that leaving
    maximized later goes back to it. The title bar's Maximize acts on its own window, so unlike a resize the harness can
    do this one for real. Specified: in the 900 by 650 window step 09 opened, a click on the title bar's Maximize makes
    the app save the maximized state with the 900 by 650 ordinary frame kept; after a quit by the title bar's Close and
    a relaunch, the window opens larger than that frame in both directions (maximized on the run's monitor); and quit
    again, by SIGTERM, the window reopened maximized still saves itself maximized with that ordinary frame, which a
    window GPUI maximized as it opened once failed to do on the Mac (`maximized` in
    crates/gui/farhelm-gui/src/desktop.rs)."""
    # The bar is drawn once the compositor has said it draws no decorations, which may come after the window opens.
    agent("wait", "id:title-bar-maximize")
    agent("click", "id:title-bar-maximize")

    def maximized_saved() -> bool:
        saved = frame_record() or {}
        normal = saved.get("normal", {})
        return saved.get("maximized") is True and (normal.get("width"), normal.get("height")) == SAVED_SIZE

    eventually_framed(maximized_saved, f"the app saving the maximized state over the {SAVED_SIZE} frame")
    quit_app(by_close_button=True)
    at_quit = frame_record()
    relaunch()
    eventually_framed(
        lambda: (size := window_size())[0] > SAVED_SIZE[0] and size[1] > SAVED_SIZE[1],
        "the window reopening maximized",
        f" (at the Close it was {at_quit})",
    )
    quit_app(by_close_button=False)
    if not maximized_saved():
        raise AssertionError(f"after quitting the window reopened maximized, the record is {frame_record()}")


# What the relaunched app logs when it stops the supervisor an earlier launch started, and when that supervisor had to
# be killed (`stop_own_supervisor` in crates/farhelm-desktop/src/lib.rs).
REPLACING_LOG = "replacing the supervisor an earlier launch started"
KILLED_LOG = "ignored the orderly stop and was killed"


def step_11_a_replaced_program_replaces_the_apps_own_supervisor() -> None:
    """An update, or a new build copied over the old, puts a new program file where the app starts its supervisor
    from; a launch that kept talking to the old supervisor would leave the user on the old build until they found and
    stopped it by hand. The old native smoke test's replaced-program leg. The run's program is the shared build output,
    which a run must not overwrite, so the step first moves the run onto a copy of it in the run (a launch from another
    file, itself a replacement) and then replaces that copy the way an install does, a new file renamed over the path.
    Specified, for each relaunch, with the app quit and its supervisor running from the previous file before it (the
    premise): the relaunched app logs that it replaced the supervisor an earlier launch started, and not that it had to
    kill it; that supervisor is gone; the record names a new, running supervisor whose running image is the run's copy
    as it is now (same inode); and the seeded session is listed live and takes typing."""
    global SUPERVISOR
    run = record()
    copy = RUN / "work" / "farhelm"
    copy.parent.mkdir(exist_ok=True)
    shutil.copy2(run["binaries"]["farhelm"], copy)
    # Every later launch, and `stop`, then use the copy: `run.json` is `gui-agent.py`'s, and `docs/gui-agent.md` records
    # this use of it beside `relaunch`'s.
    run["binaries"]["farhelm"] = str(copy)
    (RUN / "run.json").write_text(json.dumps(run, indent=2))
    for how in ("a launch from the run's copy", "a launch after the copy was replaced"):
        if how.endswith("replaced"):
            quit_app(by_close_button=False)
            before = os.stat(copy).st_ino
            shutil.copy2(copy, copy.with_name("farhelm.new"))
            os.replace(copy.with_name("farhelm.new"), copy)
            if os.stat(copy).st_ino == before:
                raise AssertionError("premise: the replaced copy is a new file")
        old, app = SUPERVISOR, record()["app"]
        if (app and gui_agent.running(app["pid"], app["start"])) or old is None or not gui_agent.running(*old):
            raise AssertionError(f"premise of {how}: the app quit and its supervisor {old} running")
        relaunch()
        # Before any other check can fail: once the record names another supervisor, `stop` would miss the old one.
        new = gui_agent.supervisor_to_stop(state())
        if gui_agent.running(*old) or new is None or new == old:
            if gui_agent.running(*old) and new != old:
                gui_agent.stop_process(*old, 20)
            raise AssertionError(f"{how}: the old supervisor {old} is not replaced by a running new one ({new})")
        log = app_log()
        if REPLACING_LOG not in log or KILLED_LOG in log:
            raise AssertionError(f"{how}: the app's log does not say it stopped its supervisor the orderly way")
        image, program = os.stat(f"/proc/{new[0]}/exe"), os.stat(copy)
        if (image.st_dev, image.st_ino) != (program.st_dev, program.st_ino):
            raise AssertionError(f"{how}: the new supervisor {new} does not run {copy} as it is now")
        SUPERVISOR = new
        assert_session_kept(how)
        assert_session_takes_typing(how)


# The refusal of a program folder holding ':' (`validate_session_program_path` in crates/farhelm-supervisor).
COLON_REFUSAL = "cannot be used as one PATH entry"


def step_12_a_replacement_that_fails_its_prerequisites_stops_nothing() -> None:
    """A replacement checks the new supervisor's prerequisites before it stops the old one; a launch that stopped first
    and then refused would leave the user with no supervisor at all. The old native smoke test's leg of the same name.
    The program is a copy of the run's in a folder whose name holds ':', which no session's PATH can hold, so the plan is
    a replacement (another file) and only the prerequisite check can refuse. Specified: with the app quit by SIGTERM and
    its supervisor recorded, running and accepting at its socket (the premise), the launch exits 1 with a
    `farhelm-desktop: ` line naming the folder and the refusal, and the same supervisor is still recorded, running and
    accepting."""
    quit_app(by_close_button=False)
    refused = RUN / "work" / "replace:refused" / "farhelm"
    refused.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(record()["binaries"]["farhelm"], refused)
    supervisor_socket = state() / "supervisor.sock"
    if not answers(supervisor_socket):
        raise AssertionError(f"premise: the supervisor does not accept at {supervisor_socket}")
    # Without the bridge's variables, as in step 03: a launch that wrongly went on must not join the run's bridge.
    drop = ("GPUI_AGENT_HOME", "GPUI_AGENT_SESSION")
    result = launch_aside(state(), drop=drop, extra={"FARHELM_DESKTOP_FARHELM": str(refused)})
    line = f"the Farhelm program folder, {refused.parent},"
    refusal = [entry for entry in result.stderr.splitlines() if entry.startswith("farhelm-desktop: ")]
    if result.returncode != 1 or not any(line in entry and COLON_REFUSAL in entry for entry in refusal):
        raise AssertionError(f"expected exit 1 and the folder's refusal, got {result.returncode}:\n{result.stderr}")
    assert_supervisor_kept("after a refused replacement")
    if not answers(supervisor_socket):
        raise AssertionError("after a refused replacement the supervisor no longer accepts at its socket")


STEPS = [
    step_01_the_app_boots_and_connects,
    step_02_a_bootstrap_failure_refuses_plainly,
    step_03_a_launch_without_wayland_refuses_before_starting_anything,
    step_04_the_window_frame_is_saved,
    step_05_a_second_launch_is_forwarded,
    step_06_a_quit_by_the_close_button_leaves_the_supervisor_running,
    step_07_a_relaunch_reuses_the_supervisor,
    step_08_a_relaunch_after_a_kill_reuses_the_supervisor,
    step_09_a_saved_size_is_restored,
    step_10_a_maximized_window_reopens_maximized,
    step_11_a_replaced_program_replaces_the_apps_own_supervisor,
    step_12_a_replacement_that_fails_its_prerequisites_stops_nothing,
]


def main(argv: list[str]) -> int:
    """Start the app, run the steps in order until one fails, stop the app run; exit 0 only when every step passed and
    the run stopped cleanly."""
    global RUN
    # SIGTERM takes Ctrl-C's path, so the `finally` below stops the app run (`gui-e2e-smoke.py` says why this matters).
    signal.signal(signal.SIGTERM, signal.default_int_handler)
    parser = argparse.ArgumentParser(prog="gui-lifecycle.py")
    parser.add_argument("--no-build", action="store_true", help="reuse the binaries already in the target directory")
    parser.add_argument("--keep", action="store_true", help="keep the run directory even when everything passed")
    options = parser.parse_args(argv)
    if not sys.platform.startswith("linux"):
        print("gui-lifecycle: the lifecycle check is for the Linux app", file=sys.stderr)
        return 2
    parent = Path(tempfile.mkdtemp(prefix="gui-lifecycle-", dir=RUNS_PARENT))
    RUN = parent / "run"
    passed = False
    stopped = False
    try:
        start = ["start", "--sessions", "1", *(["--no-build"] if options.no_build else [])]
        # No timeout: `start` may build, and is interrupted only by the caller.
        if subprocess.run([sys.executable, str(AGENT), str(RUN), *start]).returncode != 0:
            print("gui-lifecycle: the app did not start", file=sys.stderr)
        else:
            for step in STEPS:
                print(f"gui-lifecycle: {step.__name__} ...", flush=True)
                step()
                print(f"gui-lifecycle: {step.__name__} ok", flush=True)
            passed = True
    except (AssertionError, subprocess.TimeoutExpired) as error:
        print(f"gui-lifecycle: FAILED: {error}", file=sys.stderr)
    finally:
        # `stop` also after a failed or interrupted start: it releases whatever `start` had already taken.
        if RUN.exists():
            stopped = subprocess.run([sys.executable, str(AGENT), str(RUN), "stop"]).returncode == 0
            if not stopped:
                print(f"gui-lifecycle: `stop` failed; {RUN} may still be running", file=sys.stderr)
        if (passed and stopped and not options.keep) or not RUN.exists():
            shutil.rmtree(parent)
        else:
            print(f"gui-lifecycle: the run is kept at {RUN}", file=sys.stderr)
    if passed and stopped:
        print("gui-lifecycle: every step passed")
    return 0 if passed and stopped else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
