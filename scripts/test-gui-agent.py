"""The parts of gui-agent.py that decide what a run may touch: its paths, the app's environment, the seeding request and
the processes `stop` signals.

These start no compositor and build nothing; the process tests start only `sleep` and Python children of their own. The
end-to-end run is the check that the pieces work together (docs/gui-agent.md); these pin the rules that keep a run from
reaching outside its own directory: a socket path that cannot fit, a display or bus inherited from the caller's desktop,
a state directory that defaults to a real installation's, or a recorded pid that now names someone else's process.
"""

import fcntl
import importlib.util
import json
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest

spec = importlib.util.spec_from_file_location("gui_agent", pathlib.Path(__file__).with_name("gui-agent.py"))
gui_agent = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui_agent)

# A parent that starts one `sleep` child, writes the child's pid to the file named by its first argument, and then
# sleeps itself: a two-level process tree whose leaf's pid the test can read without searching /proc.
TREE_PARENT = (
    "import subprocess, sys, time\n"
    "child = subprocess.Popen(['sleep', '60'])\n"
    "open(sys.argv[1], 'w').write(str(child.pid))\n"
    "time.sleep(60)\n"
)


def wait_for_file(path: pathlib.Path, seconds: float = 10) -> str:
    """The contents of `path` once a child process has written it, or a test failure after `seconds`."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if path.exists() and path.read_text():
            return path.read_text()
        time.sleep(0.05)  # sleep-ok: polling for a file a child process writes; there is no event to wait on
    raise AssertionError(f"{path} was not written within {seconds} s")


def wait_for_exec(pid: int, marker: str, seconds: float = 10) -> None:
    """Wait until a just-started child's command line shows `marker`, i.e. until it has finished `exec`.

    Right after `Popen` returns the child may still be between fork and exec, when its command line reads empty; a
    test that reads it at that moment fails without anything being wrong (seen under load).
    """
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if marker in (gui_agent.command_line(pid) or []):
            return
        time.sleep(0.05)  # sleep-ok: polling a child until its exec completes; there is no event to wait on
    raise AssertionError(f"process {pid} did not show {marker} in its command line within {seconds} s")


def kill_quietly(pid: int) -> None:
    """Clean-up for a test-owned process that may already be gone."""
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


class RunDirectoryTest(unittest.TestCase):
    """A run directory must be absolute and shallow enough for the bridge socket inside it."""

    def test_relative_run_directory_is_refused(self):
        """A relative path would resolve against wherever the next command runs, splitting one run in two."""
        with self.assertRaises(gui_agent.UsageError):
            gui_agent.check_run_dir(pathlib.Path("run"))

    def test_socket_path_limit(self):
        """The deepest socket must fit the platform's Unix socket path limit (107 bytes on Linux, 103 on macOS); one
        byte over is refused before anything starts.

        The limit follows the longest socket a run creates (the supervisor's), not the bridge's, which is shorter: a
        directory sized to the bridge's socket alone would pass here and then fail inside the app.
        """
        self.assertEqual(gui_agent.LONGEST_SOCKET_SUFFIX, "/state/supervisor.sock")
        for suffix in gui_agent.SOCKET_SUFFIXES:
            self.assertLessEqual(len(suffix), len(gui_agent.LONGEST_SOCKET_SUFFIX))
        fits = "/" + "a" * (gui_agent.MAX_SOCKET_PATH - len(gui_agent.LONGEST_SOCKET_SUFFIX) - 1)
        gui_agent.check_run_dir(pathlib.Path(fits))
        with self.assertRaises(gui_agent.UsageError):
            gui_agent.check_run_dir(pathlib.Path(fits + "a"))


class EnvironmentTest(unittest.TestCase):
    """The app reaches only this run's compositor, bus and renderer, whatever the caller's session holds."""

    def test_caller_session_is_replaced(self):
        """Display, bus, bridge and Farhelm variables from the caller never reach the app; the run's own do."""
        base = {
            "PATH": "/usr/bin",
            "DISPLAY": ":0",
            "WAYLAND_DISPLAY": "wayland-0",
            "WAYLAND_SOCKET": "7",
            "DBUS_SESSION_BUS_ADDRESS": "unix:path=/run/user/1/bus",
            "GPUI_AGENT_SOCKET": "/elsewhere.sock",
            "FARHELM_DESKTOP_STATE_DIR": "/home/someone/.local/state/farhelm",
        }
        run = pathlib.Path("/tmp/run")
        env = gui_agent.bridge_env(
            base, run, "/tmp/run/runtime", "wayland-fh-x", "unix:path=/tmp/run/bus", platform="linux"
        )
        self.assertEqual(env["PATH"], "/usr/bin")
        self.assertNotIn("DISPLAY", env)
        self.assertNotIn("WAYLAND_SOCKET", env)
        self.assertNotIn("GPUI_AGENT_SOCKET", env)
        self.assertNotIn("FARHELM_DESKTOP_STATE_DIR", env)
        self.assertEqual(env["WAYLAND_DISPLAY"], "wayland-fh-x")
        self.assertEqual(env["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/tmp/run/bus")
        self.assertEqual(env["XDG_RUNTIME_DIR"], "/tmp/run/runtime")
        self.assertEqual(env["GPUI_AGENT_HOME"], "/tmp/run/agent")
        self.assertEqual(env["VK_ICD_FILENAMES"], gui_agent.LAVAPIPE_ICD)
        self.assertEqual(env["RUST_LOG"], "info")
        self.assertEqual(gui_agent.bridge_env({"RUST_LOG": "debug"}, run, "r", "d", "b")["RUST_LOG"], "debug")

    def test_a_mac_run_adds_no_display_and_still_drops_the_callers(self):
        """On a Mac the app opens its window in the GUI session it runs in, so the run adds no display, bus or renderer
        variables; the caller's bridge and Farhelm variables are still removed, and the run's bridge home is set.

        Why it matters: a Linux renderer override or an inherited Wayland variable would be meaningless there at best,
        and a caller's GPUI_AGENT_* or FARHELM_* variable would point the app at another run or a real installation.
        """
        base = {
            "PATH": "/usr/bin",
            "GPUI_AGENT_SOCKET": "/elsewhere.sock",
            "FARHELM_DESKTOP_STATE_DIR": "/Users/x/state",
        }
        env = gui_agent.bridge_env(base, pathlib.Path("/tmp/run"), None, None, None, platform="darwin")
        for key in ("WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "VK_ICD_FILENAMES"):
            self.assertNotIn(key, env)
        self.assertNotIn("GPUI_AGENT_SOCKET", env)
        self.assertNotIn("FARHELM_DESKTOP_STATE_DIR", env)
        self.assertEqual(env["GPUI_AGENT_HOME"], "/tmp/run/agent")
        self.assertEqual(env["PATH"], "/usr/bin")

    def test_app_state_and_cli_come_from_the_run(self):
        """The app's state directory and supervisor program are the run's, never a default or installed one."""
        args = gui_agent.app_env_args(pathlib.Path("/tmp/run"), {"farhelm": "/t/debug/farhelm"})
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertEqual(args[0::2], ["--env"] * len(pairs))
        self.assertEqual(pairs["FARHELM_DESKTOP_STATE_DIR"], "/tmp/run/state")
        self.assertEqual(pairs["FARHELM_DESKTOP_FARHELM"], "/t/debug/farhelm")
        self.assertEqual(pairs["FARHELM_DESKTOP_AGENT_HELM_FILE"], "/tmp/run/helm.json")


class SeedTest(unittest.TestCase):
    """Seeded sessions are command launches of the fake agent, quoted so an odd build path cannot split the command."""

    def test_seed_body_quotes_the_fixture_path(self):
        """A build path with a space stays one word of the command, and the session is an ordinary, non-YOLO launch."""
        body = gui_agent.seed_body(2, pathlib.Path("/tmp/run/work"), "/t dir/farhelm-fixtures")
        self.assertEqual(body["title"], "Fixture 2")
        self.assertEqual(body["cwd"], "/tmp/run/work")
        self.assertEqual(body["command"]["command"], "'/t dir/farhelm-fixtures' fake-agent --script basic")
        self.assertFalse(body["command"]["yolo"])


class ProcessIdentityTest(unittest.TestCase):
    """A recorded pid is acted on only while its start time still matches."""

    def test_start_time_names_this_process(self):
        """This process has a start time; a pid that does not exist has none, so nothing is signalled."""
        self.assertIsNotNone(gui_agent.proc_start(os.getpid()))
        self.assertIsNone(gui_agent.proc_start(2**22 + 12345))

    def test_stopping_a_mismatched_identity_signals_nothing(self):
        """A pid whose start time differs from the record counts as already gone, and is never signalled.

        Aimed at a child, not the test runner: if the guard regressed, the child would die and the test would fail,
        rather than the runner killing itself.
        """
        child = subprocess.Popen(["sleep", "60"])
        try:
            self.assertTrue(gui_agent.stop_process(child.pid, "not-a-start-time", 0.1))
            self.assertIsNone(child.poll(), "the child must still be running")
        finally:
            child.kill()
            child.wait()


class PsEnvironmentTest(unittest.TestCase):
    """On macOS, a process's recorded start time is only comparable if `ps` formats it the same way every time."""

    def test_ps_runs_in_the_c_locale_and_utc(self):
        """`start` and `stop` run in separate shells, maybe with different locales and time zones; `ps` gets the C
        locale and UTC regardless, so the same process always reads the same start time. The caller's other variables
        pass through, and the caller's environment itself is not changed."""
        base = {"PATH": "/usr/bin", "LANG": "de_DE.UTF-8", "LC_ALL": "de_DE.UTF-8", "TZ": "Europe/Berlin"}
        env = gui_agent.ps_env(base)
        self.assertEqual(env["LC_ALL"], "C")
        self.assertEqual(env["TZ"], "UTC0")
        self.assertEqual(env["PATH"], "/usr/bin")
        self.assertEqual(base["TZ"], "Europe/Berlin")


class ProcessTreeTest(unittest.TestCase):
    """What `stop` and a failed compositor start decide to signal: a process tree, and the run's supervisor."""

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        # Grandchildren by pid (killed if still there); direct children as Popen objects (killed and reaped).
        self.owned: list[int] = []
        self.children: list[subprocess.Popen] = []

    def tearDown(self):
        for pid in self.owned:
            kill_quietly(pid)
        for child in self.children:
            child.kill()
            child.wait()
        subprocess.run(["rm", "-rf", str(self.dir)], check=True)

    def tree(self) -> tuple[subprocess.Popen, int]:
        """A parent with one `sleep` child; returns the parent and the child's pid."""
        parent = subprocess.Popen([sys.executable, "-c", TREE_PARENT, str(self.dir / "leaf")])
        self.children.append(parent)
        leaf = int(wait_for_file(self.dir / "leaf"))
        self.owned.append(leaf)
        return parent, leaf

    def test_descendants_finds_the_whole_tree(self):
        """The walk through parent links (/proc on Linux, `ps` on macOS) finds the root and its child, and nothing for a
        root that is gone."""
        parent, leaf = self.tree()
        self.assertEqual(gui_agent.descendants(parent.pid), sorted([parent.pid, leaf]))
        self.assertEqual(gui_agent.descendants(2**22 + 12345), [])

    def test_stop_tree_stops_every_process_under_a_matching_root(self):
        """With the root's recorded identity, the whole tree stops, the leaf included; with a stale one, nothing
        does."""
        parent, leaf = self.tree()
        self.assertEqual(gui_agent.stop_tree(parent.pid, "not-a-start-time", 1), [])
        self.assertIsNotNone(gui_agent.proc_start(leaf), "a stale root identity must not stop the tree")
        self.assertEqual(gui_agent.stop_tree(parent.pid, gui_agent.proc_start(parent.pid), 5), [])
        parent.wait(timeout=10)
        # The leaf was the parent's child; once the parent is gone it is reaped by init, so it disappears entirely.
        self.assertIsNone(gui_agent.proc_start(leaf), "the leaf must be stopped too")

    def test_a_compositor_that_never_comes_up_is_stopped(self):
        """A failed compositor start stops everything it started before the error propagates.

        `run.json` does not exist yet at that point, so `stop` could not find the compositor later; without this, a
        failed `start` leaked the bus and the compositor.
        """
        (self.dir / "logs").mkdir()
        leaf_file = self.dir / "leaf"
        command = [sys.executable, "-c", TREE_PARENT, str(leaf_file)]
        with self.assertRaises(RuntimeError):
            gui_agent.start_mutter(self.dir, self.dir, "wayland-never", command, dict(os.environ), socket_wait=3)
        leaf = int(wait_for_file(leaf_file))
        self.owned.append(leaf)
        self.assertIsNone(gui_agent.proc_start(leaf), "the compositor's child must be stopped")

    def test_the_supervisor_is_claimed_only_while_it_names_the_state_directory(self):
        """The app's record of its supervisor counts only while that pid's command line names the run's state dir.

        A recorded pid reused by an unrelated process (here, one started without the state directory) is not claimed,
        and neither is a pid that no longer exists.
        """
        state = self.dir / "state"
        state.mkdir()
        sleeper = "import time; time.sleep(60)"
        ours = subprocess.Popen([sys.executable, "-c", sleeper, str(state)])
        stranger = subprocess.Popen([sys.executable, "-c", sleeper, str(self.dir / "elsewhere")])
        self.children += [ours, stranger]
        wait_for_exec(ours.pid, str(state))
        wait_for_exec(stranger.pid, str(self.dir / "elsewhere"))
        record = state / "desktop-supervisor.json"
        record.write_text(json.dumps({"pid": ours.pid}))
        self.assertEqual(gui_agent.supervisor_to_stop(state), (ours.pid, gui_agent.proc_start(ours.pid)))
        record.write_text(json.dumps({"pid": stranger.pid}))
        self.assertIsNone(gui_agent.supervisor_to_stop(state))
        record.write_text(json.dumps({"pid": 2**22 + 12345}))
        self.assertIsNone(gui_agent.supervisor_to_stop(state))
        record.unlink()
        self.assertIsNone(gui_agent.supervisor_to_stop(state))


# A stand-in for `start` waiting on the lock: loads gui-agent.py, starts the lock holder through `hold_lock`, writes the
# holder's identity to argv[2] as soon as it is known (the same point `start` records it in run.json), then waits.
WAITING_START = (
    "import importlib.util, json, pathlib, sys\n"
    "spec = importlib.util.spec_from_file_location('gui_agent', sys.argv[1])\n"
    "gui_agent = importlib.util.module_from_spec(spec)\n"
    "spec.loader.exec_module(gui_agent)\n"
    "run = pathlib.Path(sys.argv[3])\n"
    "record = lambda lock: pathlib.Path(sys.argv[2]).write_text(json.dumps(lock))\n"
    "gui_agent.hold_lock(run, pathlib.Path(sys.argv[4]), on_started=record, wait=600)\n"
)


class LockTest(unittest.TestCase):
    """`start --lock` holds the shared lock across the run's separate invocations, and nothing can strand it.

    Why it matters: on a machine whose one GUI session several runs share (the test Mac), two runs' windows and
    synthetic input must not overlap; the lock is what serializes them, so it has to be held for the run's whole life,
    not only while `start` runs, released when the run ends, and never left held by a process nobody can find (every
    later run on that machine would then wait and fail).
    """

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        (self.dir / "logs").mkdir()
        self.lock = self.dir / "shared.lock"
        self.holders: list[subprocess.Popen] = []

    def tearDown(self):
        for holder in self.holders:
            holder.kill()
            holder.wait()
        subprocess.run(["rm", "-rf", str(self.dir)], check=True)

    def locked_elsewhere(self) -> bool:
        """Whether another process holds the lock: a non-blocking exclusive flock from this process fails."""
        with open(self.lock, "a") as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return True
            fcntl.flock(handle, fcntl.LOCK_UN)
            return False

    def test_the_lock_is_held_until_the_holder_is_stopped(self):
        """After `hold_lock` returns, the lock is taken by its detached holder; stopping the holder releases it."""
        record, holder = gui_agent.hold_lock(self.dir, self.lock, wait=10)
        self.holders.append(holder)
        self.assertTrue(gui_agent.running(record["pid"], record["start"]))
        self.assertTrue(self.locked_elsewhere(), "the holder must hold the lock once hold_lock returns")
        self.assertTrue(gui_agent.stop_process(record["pid"], record["start"], 5))
        self.assertFalse(self.locked_elsewhere(), "stopping the holder must release the lock")

    def test_on_started_sees_the_holder_before_the_wait(self):
        """The holder's identity reaches the caller before the lock is taken, which is when `start` records it in
        run.json; a `start` killed after the holder gets the lock still leaves `stop` a record to act on."""
        seen = []
        record, holder = gui_agent.hold_lock(self.dir, self.lock, on_started=seen.append, wait=10)
        self.holders.append(holder)
        self.assertEqual(seen, [record])

    def test_a_holder_whose_start_is_killed_while_waiting_gives_up(self):
        """If `start` dies while it waits for another run's lock (SIGKILL here; an agent's timeout or a dropped SSH
        session in practice, neither of which runs Python's clean-up), its holder exits instead of taking the lock
        later and keeping it with nothing left to release it.

        The premise is checked first: the holder is alive and waiting while this test holds the lock.
        """
        with open(self.lock, "a") as ours:
            fcntl.flock(ours, fcntl.LOCK_EX)
            run = self.dir / "run"
            (run / "logs").mkdir(parents=True)
            identity = self.dir / "holder.json"
            waiter = subprocess.Popen(
                [sys.executable, "-c", WAITING_START, str(pathlib.Path(__file__).with_name("gui-agent.py")),
                 str(identity), str(run), str(self.lock)]
            )
            self.holders.append(waiter)
            holder = json.loads(wait_for_file(identity))
            self.assertTrue(gui_agent.running(holder["pid"], holder["start"]), "premise: the holder is waiting")
            self.assertFalse((run / "lock.held").exists(), "premise: the holder does not have the lock")
            waiter.kill()
            waiter.wait()
            deadline = time.monotonic() + 10
            while gui_agent.running(holder["pid"], holder["start"]) and time.monotonic() < deadline:
                time.sleep(0.05)  # sleep-ok: polling a detached process for exit; there is no event to wait on
            self.assertFalse(gui_agent.running(holder["pid"], holder["start"]), "the orphaned holder must give up")
        self.assertFalse(self.locked_elsewhere(), "nothing may hold the lock once both are gone")
        self.assertFalse((run / "lock.held").exists(), "the holder must never have taken the lock")

    def test_a_holder_that_cannot_open_the_lock_fails_start_at_once(self):
        """A lock path whose directory does not exist makes the holder exit at once; `hold_lock` reports the holder's
        own error immediately instead of waiting out the whole wait and blaming another run."""
        began = time.monotonic()
        with self.assertRaises(RuntimeError) as raised:
            gui_agent.hold_lock(self.dir, self.dir / "missing" / "shared.lock", wait=30)
        self.assertLess(time.monotonic() - began, 20, "a dead holder must fail the wait at once")
        self.assertIn("could not take the lock", str(raised.exception))
        self.assertIn("No such file or directory", str(raised.exception))

    def test_stop_releases_the_lock_when_the_run_stopped_cleanly(self):
        """`stop` ends the holder, and so frees the lock, when nothing else of the run is left (here the run never got
        past taking the lock, as when its build failed)."""
        record, holder = gui_agent.hold_lock(self.dir, self.lock, wait=10)
        self.holders.append(holder)
        status = gui_agent.stop(self.dir, {"state": str(self.dir / "state"), "lock": record})
        self.assertEqual(status, 0)
        self.assertFalse(self.locked_elsewhere(), "stop must release the lock")

    def test_stop_keeps_the_lock_while_part_of_the_run_may_still_run(self):
        """When an earlier step of `stop` reports a problem (here the terminal-host stop cannot confirm, faked with
        `false` as the program), the lock stays held and the failure is reported: releasing it would let the next run's
        window overlap whatever is still running. A second `stop` that succeeds then releases it."""
        record, holder = gui_agent.hold_lock(self.dir, self.lock, wait=10)
        self.holders.append(holder)
        (self.dir / "state" / "hosts").mkdir(parents=True)
        failing = {"state": str(self.dir / "state"), "lock": record, "binaries": {"farhelm": shutil.which("false")}}
        self.assertEqual(gui_agent.stop(self.dir, failing), 1)
        self.assertTrue(self.locked_elsewhere(), "the lock must stay held while the run may still be running")
        succeeding = dict(failing, binaries={"farhelm": shutil.which("true")})
        self.assertEqual(gui_agent.stop(self.dir, succeeding), 0)
        self.assertFalse(self.locked_elsewhere(), "a clean second stop releases the lock")


class CommandLineTest(unittest.TestCase):
    """`command_line` is how `stop` ties a recorded supervisor pid to the run, on Linux (/proc) and macOS (`ps`)."""

    def test_command_line_lists_a_running_processs_arguments(self):
        """A child started with a distinctive argument shows it; a pid that does not exist gives None."""
        marker = "/tmp/gui-agent-test-marker-dir"
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)", marker])
        try:
            wait_for_exec(child.pid, marker)
            argv = gui_agent.command_line(child.pid)
            self.assertIsNotNone(argv)
            self.assertIn(marker, argv)
        finally:
            child.kill()
            child.wait()
        self.assertIsNone(gui_agent.command_line(2**22 + 12345))


if __name__ == "__main__":
    unittest.main()
