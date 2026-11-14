"""The parts of gui-agent.py that decide what a run may touch: its paths, the app's environment, the seeding request and
the processes `stop` signals.

These start no compositor and build nothing; the process tests start only `sleep` and Python children of their own. The end-to-end run is the check that the pieces work together
(docs/gui-agent.md); these pin the rules that keep a run from reaching outside its own directory: a socket path that
cannot fit, a display or bus inherited from the caller's desktop, a state directory that defaults to a real
installation's, or a recorded pid that now names someone else's process.
"""

import importlib.util
import json
import os
import pathlib
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
        """The deepest socket must fit the 107-byte Unix path limit; one byte over is refused before anything starts.

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
        env = gui_agent.bridge_env(base, run, "/tmp/run/runtime", "wayland-fh-x", "unix:path=/tmp/run/bus")
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
        """The walk through /proc parent links finds the root and its child, and nothing for a root that is gone."""
        parent, leaf = self.tree()
        self.assertEqual(gui_agent.descendants(parent.pid), sorted([parent.pid, leaf]))
        self.assertEqual(gui_agent.descendants(2**22 + 12345), [])

    def test_stop_tree_stops_every_process_under_a_matching_root(self):
        """With the root's recorded identity, the whole tree stops, the leaf included; with a stale one, nothing does."""
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
        record = state / "desktop-supervisor.json"
        record.write_text(json.dumps({"pid": ours.pid}))
        self.assertEqual(gui_agent.supervisor_to_stop(state), (ours.pid, gui_agent.proc_start(ours.pid)))
        record.write_text(json.dumps({"pid": stranger.pid}))
        self.assertIsNone(gui_agent.supervisor_to_stop(state))
        record.write_text(json.dumps({"pid": 2**22 + 12345}))
        self.assertIsNone(gui_agent.supervisor_to_stop(state))
        record.unlink()
        self.assertIsNone(gui_agent.supervisor_to_stop(state))


if __name__ == "__main__":
    unittest.main()
