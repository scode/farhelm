"""The parts of gui-agent.py that decide what a run may touch: its paths, the app's environment, a staged run's private
HOME and PATH and its remote supervisors, the seeding requests and the processes `stop` signals; the waits scenes rely
on (a seeded fleet's statuses, `terminal-text --wait`); and how a request to the app's helm reads its reply.

These start no compositor and build nothing; the process tests start only `sleep`, Python children of their own and, for
a staged run's sessions, the machine's own login bash with a stand-in for the fake agent. The
end-to-end run is the check that the pieces work together (docs/gui-agent.md); these pin the rules that keep a run from
reaching outside its own directory: a socket path that cannot fit, a display or bus inherited from the caller's desktop,
a state directory that defaults to a real installation's, or a recorded pid that now names someone else's process.
"""

import fcntl
import http.server
import importlib.util
import json
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
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
        self.assertNotIn("FARHELM_DESKTOP_AGENT_WINDOW_SIZE", pairs)
        self.assertNotIn("FARHELM_DESKTOP_AGENT_APPEARANCE", pairs)
        self.assertNotIn(gui_agent.SCRIPTED_HOSTS_ENV, pairs)

    def test_start_dialogs_are_off_unless_the_run_asks_for_them(self):
        """Every run's helm is fresh, so a run that let the Welcome card open would act on the card with its first
        click or key instead of on the app. Specified: by default the app gets `off` under the name its agent-build
        startup overrides read, and `--start-dialogs` (`start_dialogs=True`) leaves the variable unset, which is the
        app's real start."""
        args = gui_agent.app_env_args(pathlib.Path("/tmp/run"), {"farhelm": "/t/f"})
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertEqual(pairs["FARHELM_DESKTOP_AGENT_START_DIALOGS"], "off")
        args = gui_agent.app_env_args(pathlib.Path("/tmp/run"), {"farhelm": "/t/f"}, start_dialogs=True)
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertNotIn("FARHELM_DESKTOP_AGENT_START_DIALOGS", pairs)

    def test_payload_dir_reaches_the_app_only_when_given(self):
        """A run that sets up or updates a real remote host needs the helm to have payloads to push, and the caller's
        own `FARHELM_*` variables never reach the app, so `--payload-dir` is the only way in. Specified: given a
        directory, the app gets it under the exact variable its embedded helm reads; without one the variable is
        unset, so an ordinary run has no payloads and pushes nothing."""
        args = gui_agent.app_env_args(pathlib.Path("/tmp/run"), {"farhelm": "/t/f"}, payload_dir=pathlib.Path("/p"))
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertEqual(pairs["FARHELM_HELM_PAYLOAD_DIR"], "/p")
        args = gui_agent.app_env_args(pathlib.Path("/tmp/run"), {"farhelm": "/t/f"})
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertNotIn("FARHELM_HELM_PAYLOAD_DIR", pairs)

    def test_payload_dir_must_be_an_existing_absolute_directory(self):
        """The helm looks into the payload directory only when it sets up or updates a host, long after `start`
        returned, so a bad path must be refused up front rather than surface as a failed Update mid-run. Specified: a
        relative path is refused as not absolute even when it names an existing directory, a missing absolute path and
        a plain file are refused as not an existing directory, and an existing absolute directory is accepted
        unchanged. Each case checks which refusal fired, since both are the same exception type."""
        with tempfile.TemporaryDirectory() as tmp:
            directory = pathlib.Path(tmp)
            (directory / "file").write_text("")
            relative = pathlib.Path(os.path.relpath(directory))
            self.assertTrue(relative.is_dir() and not relative.is_absolute(), "premise: a relative existing directory")
            with self.assertRaisesRegex(gui_agent.UsageError, "absolute"):
                gui_agent.check_payload_dir(relative)
            for bad in (directory / "missing", directory / "file"):
                with self.subTest(bad=bad), self.assertRaisesRegex(gui_agent.UsageError, "existing directory"):
                    gui_agent.check_payload_dir(bad)
            self.assertEqual(gui_agent.check_payload_dir(directory), directory)

    def test_scripted_hosts_reach_the_helm_from_inside_its_state(self):
        """`--scripted-hosts` is how end-to-end steps add, set up, update and fail fake SSH hosts through the real helm.
        The helm accepts the backend only from a private directory inside its own state directory holding its exact
        marker, so the run's directory must be there, carry that marker and an empty config (every host absent), and
        its path must reach the app under the variable the helm reads; without the option the variable stays unset, so
        an ordinary run sets hosts up for real."""
        with tempfile.TemporaryDirectory() as tmp:
            run = pathlib.Path(tmp) / "run"
            run.mkdir()
            directory = gui_agent.prepare_scripted_hosts(run)
            self.assertEqual(directory, run / "state" / "scripted-hosts")
            self.assertEqual((directory / "ENABLED").read_text(), "farhelm-e2e-provisioning-v1\n")
            self.assertEqual(json.loads((directory / "config.json").read_text()), {})
            for private in (run / "state", directory):
                self.assertEqual(private.stat().st_mode & 0o777, 0o700, private)
            args = gui_agent.app_env_args(run, {"farhelm": "/t/f"}, scripted_hosts=directory)
            pairs = dict(arg.split("=", 1) for arg in args[1::2])
            self.assertEqual(pairs[gui_agent.SCRIPTED_HOSTS_ENV], str(directory))
            self.assertEqual(gui_agent.SCRIPTED_HOSTS_ENV, "FARHELM_E2E_PROVISIONING_BACKEND_DIR")

    def test_scripting_hosts_replaces_the_config_whole(self):
        """The helm rereads the fake hosts' config at every step, so a config written in place could be read half
        written and fail a step on its own. Specified: `script_hosts` leaves exactly the new config at `config.json`,
        through a rename (the file's inode changes, so it was never rewritten in place), and leaves no staging file."""
        with tempfile.TemporaryDirectory() as tmp:
            run = pathlib.Path(tmp) / "run"
            run.mkdir()
            directory = gui_agent.prepare_scripted_hosts(run)
            before = (directory / "config.json").stat().st_ino
            config = {"targets": {"ssh:failing.example": {"fail_action": "install-farhelm"}}}
            gui_agent.script_hosts(run, config)
            self.assertEqual(json.loads((directory / "config.json").read_text()), config)
            self.assertNotEqual((directory / "config.json").stat().st_ino, before)
            self.assertEqual(sorted(path.name for path in directory.iterdir()), ["ENABLED", "config.json"])

    def test_window_size_and_appearance_reach_the_app(self):
        """A screenshot batch labels each capture with the theme and size it asked for, so `--window-size` and
        `--appearance` must reach the app under the names its agent-build startup overrides read."""
        args = gui_agent.app_env_args(pathlib.Path("/tmp/run"), {"farhelm": "/t/f"}, "900x700", "dark")
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertEqual(pairs["FARHELM_DESKTOP_AGENT_WINDOW_SIZE"], "900x700")
        self.assertEqual(pairs["FARHELM_DESKTOP_AGENT_APPEARANCE"], "dark")

    def test_a_malformed_size_is_a_usage_error(self):
        """A size typo is refused before anything starts, not handed to the app or Mutter."""
        self.assertEqual(gui_agent.parse_size("900x700"), (900, 700))
        for bad in ("900", "900x", "x700", "0x700", "-1x700", "900X700", "wide"):
            with self.assertRaises(gui_agent.UsageError, msg=bad):
                gui_agent.parse_size(bad)


# A stand-in for the fake agent (`farhelm-fixtures`) that prints the arguments it was run with, so a test can tell which
# program a session's `claude` reached and what it was given.
FIXTURES_STANDIN = '#!/bin/sh\necho "fixture: $*"\n'

# A stand-in for a vendor's real CLI, on the caller's PATH and in the caller's own login profile, which a staged session
# must never reach.
REAL_AGENT = '#!/bin/sh\necho "REAL AGENT REACHED"\n'

# A shell line printing where each harness's program resolves, one per line, in STAGED_AGENTS order.
RESOLVE_ALL = " ".join(f"command -v {name};" for name in gui_agent.STAGED_AGENTS)


def launch_programs(source: str) -> list[str]:
    """The program names `fn program` in crates/farhelm-helm/src/launches.rs (`source`) maps the launch harnesses to,
    read from its same-line `LaunchHarness::X => "name"` arms. Fails when the function has an arm written any other way
    (a constant, a call, a value on the next line), so such an entry fails the wrapper test instead of being skipped."""
    body = source[source.index("fn program(harness: LaunchHarness)"):]
    body = body[: body.index("\n}\n")]
    programs = re.findall(r'LaunchHarness::\w+ => "([^"]+)"', body)
    arms = body.count("=>")
    if arms != len(programs):
        raise AssertionError(f"{arms} arms but {len(programs)} same-line quoted names in `fn program`: {body}")
    return programs


def write_executable(path: pathlib.Path, text: str) -> pathlib.Path:
    """Write an executable file and return its path."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    path.chmod(0o755)
    return path


class StagedRunTest(unittest.TestCase):
    """A staged run launches its sessions by an agent harness's bare name (`claude`, `codex`, ...), so the only thing
    between a staged session and the vendor's real agent (with the caller's real account) is how those names resolve.
    These pin that every one resolves to the run's wrappers, which run the fake agent, whatever the caller's own PATH
    and login profile say, and that no credential of the caller's reaches the run."""

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir)
        # The caller's world, which a staged run must not reach: real agents on its PATH, a login profile that puts
        # them first, and variables that point at the caller's configuration.
        self.real_bin = self.dir / "caller" / "bin"
        self.real_home = self.dir / "caller" / "home"
        for name in gui_agent.STAGED_AGENTS:
            write_executable(self.real_bin / name, REAL_AGENT)
        self.real_home.mkdir(parents=True)
        (self.real_home / ".bash_profile").write_text(f'export PATH={self.real_bin}:"$PATH"\n')
        self.caller = {
            "PATH": f"{self.real_bin}:/usr/bin:/bin",
            "HOME": str(self.real_home),
            "SHELL": "/bin/zsh",
            "ZDOTDIR": str(self.real_home),
            "BASH_ENV": str(self.real_home / ".bash_profile"),
            "XDG_CONFIG_HOME": str(self.real_home / ".config"),
            "CLAUDE_CONFIG_DIR": str(self.real_home / ".claude"),
            "CODEX_HOME": str(self.real_home / ".codex"),
            "ANTHROPIC_API_KEY": "sk-ant-caller",
            "CLAUDE_CODE_OAUTH_TOKEN": "oauth-caller",
            "OPENAI_API_KEY": "sk-caller",
            "OPENROUTER_API_KEY": "or-caller",
            "GITHUB_TOKEN": "gh-caller",
            "AWS_PROFILE": "caller-bedrock",
            "SSH_AUTH_SOCK": "/tmp/agent.sock",
            "LANG": "C.UTF-8",
        }
        self.run_dir = self.dir / "run"
        self.run_dir.mkdir()
        self.fixtures = write_executable(self.dir / "target" / "debug" / "farhelm-fixtures", FIXTURES_STANDIN)
        self.staging = gui_agent.prepare_staging(self.run_dir, str(self.fixtures))
        record = {"staging": self.staging, "runtime": "/r/runtime", "display": "wayland-fh-x", "dbus": "unix:path=/b"}
        self.env = gui_agent.run_env(self.run_dir, record, base=self.caller)
        self.wrappers = {name: str(pathlib.Path(self.staging["bin"]) / name) for name in gui_agent.STAGED_AGENTS}

    def login_shell(self, env: dict[str, str], script: str, cwd: pathlib.Path) -> str:
        """Run `script` the way the supervisor starts a session: the environment's login shell, interactive and
        login (`-l -i`), with the launch shim's change on top (the directory of the supervisor's `farhelm`, here the
        build directory beside the fake agent, put first on PATH after the shell's startup). Returns its stdout."""
        farhelm_dir = self.fixtures.parent
        result = subprocess.run(
            [env["SHELL"], "-l", "-i", "-c", f'PATH={farhelm_dir}:"$PATH"; {script}'],
            env=env,
            cwd=cwd,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def test_a_staged_session_cannot_reach_a_real_agent(self):
        """The safety proof. Specified: in the environment a staged run gives the app, its supervisor and every remote
        supervisor, every harness's program (STAGED_AGENTS) resolves to the run's wrapper both through that PATH and
        inside a session's login shell (the machine's real bash, reading the private HOME's profile after the system's),
        even though the caller's PATH and its own login profile put real programs of those names first, and also when
        the system's profile has put other directories first; and running any of them reaches the fake agent with every
        argument, never the real program.

        The premise is checked first: with the caller's environment the same login shell does reach the real programs,
        so a staged run that leaked the caller's PATH or HOME would fail here."""
        session = self.dir / "session"
        session.mkdir()
        caller = dict(self.caller, SHELL=gui_agent.STAGED_SHELL)
        premise = self.login_shell(caller, RESOLVE_ALL, session).split()
        self.assertEqual(premise, [str(self.real_bin / name) for name in gui_agent.STAGED_AGENTS], "premise")

        for name, wrapper in self.wrappers.items():
            self.assertEqual(shutil.which(name, path=self.env["PATH"]), wrapper)
        resolved = self.login_shell(self.env, RESOLVE_ALL, session).split()
        self.assertEqual(resolved, [self.wrappers[name] for name in gui_agent.STAGED_AGENTS])
        # A system profile that rebuilds PATH with its own directories first (macOS's path_helper does) runs before the
        # private one, which must put the wrappers back in front. Acted out here, since this machine's system profile
        # may not do it: the real programs go first, then the private profile is read as a login shell reads it.
        rebuilt = self.login_shell(
            self.env, f'PATH={self.real_bin}:"$PATH"; . "$HOME/.bash_profile"; {RESOLVE_ALL}', session
        ).split()
        self.assertEqual(rebuilt, [self.wrappers[name] for name in gui_agent.STAGED_AGENTS])
        for name in gui_agent.STAGED_AGENTS:
            out = self.login_shell(self.env, f"{name} --dangerously-skip-permissions", session)
            self.assertNotIn("REAL AGENT", out)
            self.assertEqual(out.strip(), "fixture: fake-agent --script basic --dangerously-skip-permissions", name)

    def test_a_session_with_a_staged_agent_script_runs_it(self):
        """A scenario gives each staged session its own replay through a script in the session's working directory, so
        sessions that are all launched as `claude` can show different transcripts. Specified: with STAGED_AGENT_SCRIPT
        in the working directory, `claude` runs that script with every argument instead of the basic fake agent."""
        session = self.dir / "replaying"
        session.mkdir()
        (session / gui_agent.STAGED_AGENT_SCRIPT).write_text('echo "staged: $*"\n')
        out = self.login_shell(self.env, "claude --settings x", session)
        self.assertEqual(out.strip(), "staged: --settings x")

    def test_the_staged_environment_points_at_nothing_of_the_callers(self):
        """Specified: HOME, PATH and SHELL are the run's (HOME and the wrappers inside the run directory, the wrappers
        first on PATH, then only system directories); every variable that names the caller's configuration
        (STAGED_DROPPED_ENV) and every credential or vendor variable (a vendor's prefix, or a name ending like a key or
        a token) is gone; whatever else the caller set passes through, `SSH_AUTH_SOCK` included, since the helm's ssh
        to a staged host may need the caller's agent; and the caller's own dictionary is left as it was."""
        before = dict(self.caller)
        env = gui_agent.staged_env(self.caller, self.staging)
        self.assertEqual(self.caller, before)
        self.assertEqual(env["HOME"], str(self.run_dir / "staging" / "home"))
        self.assertEqual(env["PATH"], f"{self.run_dir / 'staging' / 'bin'}:{gui_agent.STAGED_SYSTEM_PATH}")
        self.assertEqual(env["SHELL"], gui_agent.STAGED_SHELL)
        dropped = (*gui_agent.STAGED_DROPPED_ENV, "CLAUDE_CONFIG_DIR", "CODEX_HOME", "ANTHROPIC_API_KEY",
                   "CLAUDE_CODE_OAUTH_TOKEN", "OPENAI_API_KEY", "OPENROUTER_API_KEY", "GITHUB_TOKEN", "AWS_PROFILE")
        for name in dropped:
            self.assertNotIn(name, env)
            self.assertNotIn(name, self.env)
        self.assertEqual(env["LANG"], "C.UTF-8")
        self.assertEqual(env["SSH_AUTH_SOCK"], "/tmp/agent.sock")
        self.assertNotIn(str(self.real_home), json.dumps(env))
        self.assertNotIn(str(self.real_bin), json.dumps(env))
        # The bridge's own command line and the app get it too, under the bridge's display and renderer variables.
        self.assertEqual((self.env["HOME"], self.env["PATH"]), (env["HOME"], env["PATH"]))
        self.assertEqual(self.env["WAYLAND_DISPLAY"], "wayland-fh-x")
        # An unstaged run keeps the caller's own HOME and PATH, as before.
        plain = gui_agent.run_env(self.run_dir, {}, base=self.caller)
        self.assertEqual((plain["HOME"], plain["PATH"]), (self.caller["HOME"], self.caller["PATH"]))

    def test_every_harness_the_app_launches_by_name_is_wrapped(self):
        """A harness missing from STAGED_AGENTS would launch the vendor's real program in a staged run. Specified: the
        names are exactly the programs the helm composes an agent launch with (`fn program` in
        crates/farhelm-helm/src/launches.rs), read from that source, so a harness added there fails this until it is
        added here; and each has its wrapper in the run's PATH directory. An arm the reader cannot take a name from
        fails the read rather than being skipped, shown on a synthetic function with a constant arm."""
        source = (pathlib.Path(__file__).resolve().parent.parent / "crates/farhelm-helm/src/launches.rs").read_text()
        programs = launch_programs(source)
        self.assertGreater(len(programs), 5, "premise: the match was found and read")
        self.assertEqual(sorted(gui_agent.STAGED_AGENTS), sorted(programs))
        for name in programs:
            self.assertTrue(os.access(pathlib.Path(self.staging["bin"]) / name, os.X_OK), name)
        synthetic = (
            "fn program(harness: LaunchHarness) -> &'static str {\n    match harness {\n"
            '        LaunchHarness::Codex => "codex",\n        LaunchHarness::Other => OTHER_PROGRAM,\n    }\n}\n'
        )
        with self.assertRaisesRegex(AssertionError, "2 arms but 1 same-line quoted names"):
            launch_programs(synthetic)

    def test_a_farhelm_directory_holding_an_agent_is_found(self):
        """The launch shim puts the supervisor's own directory first on a session's PATH, ahead of the wrappers, so a
        `claude` or `codex` there would be reached instead. Specified: `shadowing_agents` names each one present and
        nothing else."""
        build = self.dir / "build"
        build.mkdir()
        write_executable(build / "farhelm", "")
        self.assertEqual(gui_agent.shadowing_agents(build), [])
        write_executable(build / "codex", REAL_AGENT)
        self.assertEqual(gui_agent.shadowing_agents(build), ["codex"])


class RemoteHostsTest(unittest.TestCase):
    """What a staged run's remote hosts are: this machine over ssh under distinct spellings, each served by a supervisor
    the run started in its own directory."""

    def test_remote_hosts_are_distinct_plain_destinations(self):
        """The helm keys a host on its destination, so two equal spellings would be one host; a destination ssh would
        read as an option, or an empty one, is a usage error before anything starts."""
        self.assertEqual(gui_agent.parse_remote_hosts(["localhost", "me@localhost"]), ["localhost", "me@localhost"])
        for bad in (["localhost", "localhost"], [""], ["-oProxyCommand=x"], ["local host"]):
            with self.subTest(bad=bad), self.assertRaises(gui_agent.UsageError):
                gui_agent.parse_remote_hosts(bad)

    def test_a_staged_run_always_fakes_provisioning_also_after_a_relaunch(self):
        """A staged remote host is this machine, so a real setup, Update or uninstall of it would act on the machine's
        own Farhelm installation; the scripted backend is what stops that. Specified: a launch of a run whose record is
        staged gets the scripted backend's directory under the variable the helm reads, also when nothing asked for
        `--scripted-hosts`, both from `start`'s path (`launch_env_args`) and from `gui-lifecycle.py`'s relaunch, which
        is driven here against a stand-in bridge until it hands over its launch arguments; an unstaged run's launch
        gets none."""
        run = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, run)
        staged = {"binaries": {"farhelm": "/t/f", "desktop": "/t/d", "cli": "/t/c"}, "staging": {"home": "/h"}}
        backend = f"{gui_agent.SCRIPTED_HOSTS_ENV}={gui_agent.scripted_hosts_dir(run)}"
        self.assertIn(backend, gui_agent.launch_env_args(run, staged))
        unstaged = {"binaries": staged["binaries"]}
        self.assertFalse(any(arg.startswith(gui_agent.SCRIPTED_HOSTS_ENV) for arg in
                             gui_agent.launch_env_args(run, unstaged)))

        lifecycle_spec = importlib.util.spec_from_file_location(
            "gui_lifecycle", pathlib.Path(__file__).with_name("gui-lifecycle.py")
        )
        lifecycle = importlib.util.module_from_spec(lifecycle_spec)
        lifecycle_spec.loader.exec_module(lifecycle)
        (run / "agent" / gui_agent.SESSION).mkdir(parents=True)
        (run / "logs").mkdir()
        (run / "run.json").write_text(json.dumps(staged))
        launches = []

        def stand_in_bridge(_run, _record, args, capture=False):
            launches.append(args)
            return subprocess.CompletedProcess(args, 1, stdout="", stderr="stand-in bridge")

        lifecycle.RUN = run
        lifecycle.gui_agent.cli = stand_in_bridge
        lifecycle.gui_agent.app_identity = lambda _run: None
        with self.assertRaisesRegex(AssertionError, "the relaunch failed"):
            lifecycle.relaunch()
        self.assertEqual(len(launches), 1, "premise: the relaunch reached the bridge once")
        self.assertIn(backend, launches[0])

    def test_each_ensured_host_is_dialed_at_the_runs_own_supervisor(self):
        """A host registered without its paths would be dialed at the destination's default installation, which on
        this machine is the maintainer's own. Specified: each destination, in order, names the run's `farhelm` and its
        own `r<N>` state directory inside the run, both absolute, and the file reaches the app under the exact variable
        its agent build reads."""
        run = pathlib.Path("/tmp/run")
        body = gui_agent.ensure_hosts_body(run, ["localhost", "me@localhost"], "/t/debug/farhelm")
        self.assertEqual(
            body,
            {
                "hosts": [
                    {"ssh": "localhost", "remote_farhelm": "/t/debug/farhelm", "remote_state_dir": "/tmp/run/r0"},
                    {"ssh": "me@localhost", "remote_farhelm": "/t/debug/farhelm", "remote_state_dir": "/tmp/run/r1"},
                ]
            },
        )
        args = gui_agent.app_env_args(run, {"farhelm": "/t/f"}, ensure_hosts=run / "ensure-hosts.json5")
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertEqual(pairs["FARHELM_DESKTOP_AGENT_ENSURE_HOSTS"], "/tmp/run/ensure-hosts.json5")
        args = gui_agent.app_env_args(run, {"farhelm": "/t/f"})
        self.assertNotIn("FARHELM_DESKTOP_AGENT_ENSURE_HOSTS", dict(arg.split("=", 1) for arg in args[1::2]))


# A stand-in for the `farhelm` CLI, for the two commands a staged run's lifecycle runs: `supervisor run --state-dir DIR`
# records the HOME, PATH and SHELL it was started with in DIR/env.json, binds DIR/supervisor.sock and then waits to be
# stopped (STANDIN_MODE `no-socket` waits without binding, `exit` exits at once); `internal stop-terminal-hosts
# --state-dir DIR` appends DIR to the file next to this script named `stopped-hosts` and succeeds.
FARHELM_STANDIN = """\
import json, os, pathlib, socket, sys, time
args = sys.argv[1:]
state = pathlib.Path(args[args.index("--state-dir") + 1])
if args[:2] == ["internal", "stop-terminal-hosts"]:
    with open(pathlib.Path(sys.argv[0]).parent / "stopped-hosts", "a") as log:
        log.write(str(state) + "\\n")
    sys.exit(0)
mode = os.environ.get("STANDIN_MODE", "")
if mode == "exit":
    sys.exit(3)
(state / "env.json").write_text(json.dumps({key: os.environ.get(key) for key in ("HOME", "PATH", "SHELL")}))
if mode != "no-socket":
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(str(state / "supervisor.sock"))
    listener.listen()
time.sleep(120)
"""


class RemoteSupervisorTest(unittest.TestCase):
    """A staged run's remote supervisors: started from the run's own directories with the staged environment, recorded
    before anything waits on them, and stopped by `stop` with their terminal hosts, never by a pid alone."""

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.run_dir = self.dir / "run"
        (self.run_dir / "logs").mkdir(parents=True)
        self.farhelm = write_executable(self.dir / "bin" / "farhelm", f"#!{sys.executable}\n{FARHELM_STANDIN}")
        self.staging = gui_agent.prepare_staging(self.run_dir, "/fx")
        record = {"staging": self.staging, "runtime": "/r/runtime", "display": "wayland-fh-x", "dbus": "unix:path=/b"}
        self.env = gui_agent.run_env(self.run_dir, record, base={"PATH": "/usr/bin:/bin"})
        self.recorded: list[list[dict]] = []

    def tearDown(self):
        for remotes in self.recorded:
            for remote in remotes:
                if remote["start"] and gui_agent.running(remote["pid"], remote["start"]):
                    kill_quietly(remote["pid"])
        shutil.rmtree(self.dir)

    def start(self, count: int, env: dict | None = None, socket_wait: float = 30) -> list[dict]:
        """Start `count` remote supervisors from the stand-in, recording every record `on_started` hands over."""
        destinations = ["localhost", "me@localhost", "127.0.0.1"][:count]
        return gui_agent.start_remote_supervisors(
            self.run_dir,
            destinations,
            str(self.farhelm),
            env or self.env,
            lambda remotes: self.recorded.append([dict(remote) for remote in remotes]),
            socket_wait=socket_wait,
        )

    def test_supervisors_start_staged_in_the_run_and_stop_with_it(self):
        """Specified: one supervisor per destination, in order, each serving its own `r<N>` directory inside the run
        and started with the staged HOME, PATH and SHELL (so its sessions resolve `claude` to the wrappers); each is
        recorded before `start` waits on any; and `stop` ends every one, then their terminal hosts (for a state
        directory that has any), and reports success."""
        remotes = self.start(2)
        self.assertEqual([remote["state"] for remote in remotes],
                         [str(self.run_dir / "r0"), str(self.run_dir / "r1")])
        self.assertEqual(len(self.recorded[0]), 1, "the first supervisor is recorded before the second starts")
        for remote in remotes:
            self.assertTrue(gui_agent.running(remote["pid"], remote["start"]))
            env = json.loads((pathlib.Path(remote["state"]) / "env.json").read_text())
            self.assertEqual(env, {"HOME": self.staging["home"], "PATH": self.staging["path"],
                                   "SHELL": gui_agent.STAGED_SHELL})
        (self.run_dir / "r1" / "hosts").mkdir()
        record = {"state": str(self.run_dir / "state"), "binaries": {"farhelm": str(self.farhelm)}, "remotes": remotes}
        self.assertEqual(gui_agent.stop(self.run_dir, record), 0)
        for remote in remotes:
            self.assertFalse(gui_agent.running(remote["pid"], remote["start"]), remote)
        self.assertEqual((self.dir / "bin" / "stopped-hosts").read_text().split(), [str(self.run_dir / "r1")])

    def test_a_supervisor_that_never_binds_fails_start_and_is_left_for_stop(self):
        """A supervisor that does not come up fails `start` naming its log, and is already in the record, so the
        `stop` every failed start is followed by ends it rather than leaking it."""
        with self.assertRaisesRegex(RuntimeError, "logs/r0.log"):
            self.start(1, env=dict(self.env, STANDIN_MODE="no-socket"), socket_wait=1)
        remote = self.recorded[-1][0]
        self.assertTrue(gui_agent.running(remote["pid"], remote["start"]), "premise: it is still running")
        record = {"state": str(self.run_dir / "state"), "binaries": {"farhelm": str(self.farhelm)},
                  "remotes": self.recorded[-1]}
        self.assertEqual(gui_agent.stop(self.run_dir, record), 0)
        self.assertFalse(gui_agent.running(remote["pid"], remote["start"]))

    def test_a_supervisor_that_exits_fails_start_at_once(self):
        """A supervisor that exits (a bad state directory, say) fails `start` without waiting out the socket wait."""
        began = time.monotonic()
        with self.assertRaisesRegex(RuntimeError, "exited before"):
            self.start(1, env=dict(self.env, STANDIN_MODE="exit"), socket_wait=60)
        self.assertLess(time.monotonic() - began, 30)

    def test_a_recorded_supervisor_is_claimed_only_while_it_is_still_the_runs(self):
        """`stop` signals a recorded remote supervisor only while its pid and start time match and its command line
        names its state directory. Specified: a pid now held by a process without that directory, a start time that no
        longer matches and a pid that is gone are all left alone."""
        state = self.run_dir / "r0"
        stranger = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)", str(self.dir / "elsewhere")])
        self.addCleanup(stranger.wait)
        self.addCleanup(stranger.kill)
        wait_for_exec(stranger.pid, str(self.dir / "elsewhere"))
        start = gui_agent.proc_start(stranger.pid)
        stranger_record = {"state": str(state), "pid": stranger.pid, "start": start}
        self.assertIsNone(gui_agent.remote_supervisor_to_stop(stranger_record))
        ours = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)", str(state)])
        self.addCleanup(ours.wait)
        self.addCleanup(ours.kill)
        wait_for_exec(ours.pid, str(state))
        start = gui_agent.proc_start(ours.pid)
        self.assertEqual(gui_agent.remote_supervisor_to_stop({"state": str(state), "pid": ours.pid, "start": start}),
                         (ours.pid, start))
        self.assertIsNone(gui_agent.remote_supervisor_to_stop({"state": str(state), "pid": ours.pid, "start": "x"}))
        self.assertIsNone(gui_agent.remote_supervisor_to_stop({"state": str(state), "pid": 2**22 + 1, "start": "1"}))


class SeedTest(unittest.TestCase):
    """Seeded sessions are command launches of the fake agent, quoted so an odd build path cannot split the command."""

    def test_seed_body_quotes_the_fixture_path(self):
        """A build path with a space stays one word of the command, and the session is an ordinary, non-YOLO launch."""
        body = gui_agent.seed_body(2, pathlib.Path("/tmp/run/work"), "/t dir/farhelm-fixtures")
        self.assertEqual(body["title"], "Fixture 2")
        self.assertEqual(body["cwd"], "/tmp/run/work")
        self.assertEqual(body["command"]["command"], "'/t dir/farhelm-fixtures' fake-agent --script basic")
        self.assertFalse(body["command"]["yolo"])

    def test_fleet_kinds_hold_the_screen_their_status_needs(self):
        """A scene of a waiting, idle or ended session shows it only if the supervisor reads its screen that way, so
        each kind runs the replay script with the shape that classifies as it (`ReplayThen`) and its own transcript.
        Only `waiting` declares Claude, because the generic reading never says waiting, and a declared agent type
        requires `{farhelm_args}` as one whole word exactly once; the others stay undeclared, without it, since the
        helm refuses the placeholder on an undeclared command."""
        for kind, then in gui_agent.REPLAY_THEN.items():
            transcript = pathlib.Path("/tmp/run/work/fixture-3.txt")
            body = gui_agent.seed_body(3, pathlib.Path("/tmp/run/work"), "/t dir/fx", kind, transcript)
            words = body["command"]["command"].split(" ")
            self.assertEqual(body["title"], "Fixture 3")
            self.assertIn("--script replay", body["command"]["command"])
            self.assertEqual(words[words.index("--then") + 1], then)
            self.assertEqual(words[words.index("--transcript") + 1], str(transcript))
            self.assertTrue(body["command"]["command"].startswith("'/t dir/fx' "), "the fixture path stays one word")
            if kind == "waiting":
                self.assertEqual(body["command"]["agent"], "claude")
                self.assertEqual(words.count("{farhelm_args}"), 1)
            else:
                self.assertIsNone(body["command"]["agent"])
                self.assertNotIn("{farhelm_args}", body["command"]["command"])

    def test_a_spawner_runs_the_spawn_script_undeclared(self):
        """A scene of an agent's approval card types `spawn /tmp` into a `spawner` session, so that session must run
        the fake agent's spawn script, the one whose lines become real `farhelm spawn` requests, and the card shows that
        command. Specified: its body is the spawn script named by its bare binary name, never the build path (which the
        card would show), as an undeclared, non-YOLO command (a declared agent type would need `{farhelm_args}`, which
        the spawn script has no use for)."""
        body = gui_agent.seed_body(1, pathlib.Path("/tmp"), "/t dir/fx", "spawner")
        self.assertEqual(body["command"]["command"], "farhelm-fixtures fake-agent --script spawn")
        self.assertIsNone(body["command"]["agent"])
        self.assertFalse(body["command"]["yolo"])
        self.assertEqual(gui_agent.parse_fleet("spawner"), ["spawner"])

    def test_a_spawner_lets_its_own_launch_be_copied_on_the_local_host(self):
        """The helm refuses an agent's copy of a command launch before any card on a host that asks before YOLO
        launches, so a spawner whose host asked would show the agent an error and the scene no card. Specified: the
        local host's "Start YOLO sessions without asking" is turned on through the helm's own route, and no other host
        is touched."""
        sent = []

        def fake_request(run, method, path, body=None):
            sent.append((method, path, body))
            if path == "/api/hosts":
                return {"hosts": [{"id": 7, "kind": "ssh"}, {"id": 3, "kind": "local"}]}
            return None

        real_request = gui_agent.helm_request
        gui_agent.helm_request = fake_request
        try:
            gui_agent.allow_copied_launches(pathlib.Path("/tmp/run"))
        finally:
            gui_agent.helm_request = real_request
        self.assertEqual(
            sent[1:], [("POST", "/api/hosts/3/yolo-without-asking", {"yolo_without_asking": True})]
        )

    def test_seeded_sessions_run_in_the_neutral_folder_and_keep_transcripts_in_the_run(self):
        """Every row shows its session's folder, and pictures of the app are compared from one run to the next;
        seeding into the run's own random directory made every capture differ and put a scratch path in front of the
        maintainer. Specified: every seeded session, basic or replayed, is created with `/tmp` as its folder, in the
        order given, and a replayed session's transcript is still written inside the run directory."""
        sent = []

        def fake_request(run, method, path, body=None):
            sent.append((method, path, body))
            return {"id": f"s{len(sent)}"}

        real_request = gui_agent.helm_request
        gui_agent.helm_request = fake_request
        try:
            with tempfile.TemporaryDirectory() as tmp:
                run = pathlib.Path(tmp)
                ids = gui_agent.seed_sessions(run, ["basic", "idle"], "/fx")
                self.assertEqual(ids, ["s1", "s2"])
                self.assertEqual([body["cwd"] for _, _, body in sent], ["/tmp", "/tmp"])
                self.assertEqual([body["title"] for _, _, body in sent], ["Fixture 1", "Fixture 2"])
                self.assertTrue(all(method == "POST" and path == "/api/sessions" for method, path, _ in sent))
                words = sent[1][2]["command"]["command"].split(" ")
                transcript = pathlib.Path(words[words.index("--transcript") + 1])
                self.assertEqual(transcript.parent, run / "work", "the transcript stays in the run directory")
                self.assertIn("now holds its idle screen", transcript.read_text())
        finally:
            gui_agent.helm_request = real_request

    def test_seeded_templates_are_stored_before_their_bindings_and_name_nothing_of_the_machine(self):
        """The template and quick-launch scenes start from `start --templates`, and their pictures are compared from
        one run and one machine to the next; a template that carried a build path, a user or a host name would show it
        in the Templates list, a launcher summary or a launched session's name. The ⚡ mark and keys must also be
        written after the templates they name, in one preference write, so no scene finds a binding to a template not
        yet stored. Specified: one `PUT /api/templates/<name>` per seeded template, in the set's order, with its fields
        as the body; then one `PUT /api/preferences` holding only the mark and the number keys, each naming a seeded
        template; and no template's fields hold a home directory, a scratch directory or a build directory."""
        sent = []

        def fake_request(run, method, path, body=None):
            sent.append((method, path, body))
            return None

        real_request = gui_agent.helm_request
        gui_agent.helm_request = fake_request
        try:
            run = pathlib.Path("/tmp/farhelm-tmp/run-of-a-test")
            names = gui_agent.seed_templates(run)
        finally:
            gui_agent.helm_request = real_request
        self.assertEqual(names, list(gui_agent.SEEDED_TEMPLATES))
        self.assertEqual(
            sent[:-1],
            [("PUT", f"/api/templates/{name}", fields) for name, fields in gui_agent.SEEDED_TEMPLATES.items()],
        )
        method, path, bindings = sent[-1]
        self.assertEqual((method, path), ("PUT", "/api/preferences"))
        self.assertEqual(set(bindings), {"quick_launch_template", "template_shortcuts"})
        bound = [bindings["quick_launch_template"], *bindings["template_shortcuts"].values()]
        self.assertTrue(set(bound) <= set(names), f"every binding names a seeded template: {bound}")
        text = json.dumps(gui_agent.SEEDED_TEMPLATES)
        # A build path is an absolute path under a home or a scratch directory; the bare fixture name is not one.
        for leak in ("/home", "/Users", "/tmp/farhelm-tmp", "target/debug"):
            self.assertNotIn(leak, text)

    def test_a_transcript_fits_a_narrow_terminal_and_ends_on_its_kind(self):
        """The screenshot batch waits for a fleet session's last transcript line and photographs it in a 900-pixel
        window, whose terminal is about 55 columns wide: a longer line wraps, and the wrap plus the row an exited
        session's exit line lifts away left only a stray letter on screen (seen in a batch). Specified: more than one
        line, each under 50 characters, the last `now holds its <kind> screen`."""
        lines = gui_agent.transcript(4, "exited").splitlines()
        self.assertGreater(len(lines), 1)
        self.assertTrue(all(len(line) < 50 for line in lines), lines)
        self.assertEqual(lines[-1], "now holds its exited screen")
        self.assertIn("Fixture 4", lines[0])

    def test_a_fleet_lists_known_kinds_only(self):
        """A typo in a fleet is refused before anything starts, rather than seeding a session that never settles."""
        self.assertEqual(gui_agent.parse_fleet("running,basic,exited"), ["running", "basic", "exited"])
        for bad in ("", "running,", "Running", "waiting,ended"):
            with self.assertRaises(gui_agent.UsageError, msg=bad):
                gui_agent.parse_fleet(bad)


class SettleTest(unittest.TestCase):
    """`start --fleet` returns only once the helm lists every seeded session at its status, because the scenes that
    follow photograph those statuses; a session that never gets there fails the start, naming it."""

    def test_the_wait_ends_once_every_target_is_listed_at_its_state(self):
        """Specified: earlier listings with a session still unclassified (None) or missing keep the wait going; the
        first listing with every target at its state ends it. A session that is not a target is never waited for."""
        answers = iter([
            {"a": None},
            {"a": "running", "b": "idle", "basic": "idle"},
            {"a": "waiting", "b": "idle", "basic": "running"},
        ])
        reads = []

        def read():
            reads.append(1)
            return next(answers)

        gui_agent.wait_for_statuses(read, {"a": "waiting", "b": "idle"}, timeout=10, interval=0)
        self.assertEqual(len(reads), 3)

    def test_a_session_that_never_settles_fails_the_wait_naming_it(self):
        """The error names each pending session and the state it was last listed in, `missing` when not listed, so a
        fixture that no longer reads as its status is found from the message alone."""
        with self.assertRaises(RuntimeError) as raised:
            gui_agent.wait_for_statuses(lambda: {"a": "running"}, {"a": "waiting", "b": "exited"}, timeout=0.2,
                                        interval=0.05)
        self.assertIn("a=running", str(raised.exception))
        self.assertIn("b=missing", str(raised.exception))


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


class TerminalTextTest(unittest.TestCase):
    """`terminal-text` reads the probe of the terminal that holds focus. With none focused the probe answers null,
    which must fail with a message: printed as is, it would look like a terminal whose screen is empty."""

    def run_with(self, *stdouts: str, args: list[str] | None = None, returncode: int = 0) -> tuple[int, str, str]:
        """`terminal_text` against a stand-in bridge whose probe answers `stdouts` in turn, the last one from then on;
        returns the status, stdout and stderr."""
        answers = [subprocess.CompletedProcess([], returncode, stdout=stdout, stderr="") for stdout in stdouts]
        original = gui_agent.cli
        gui_agent.cli = lambda *_args, **_kwargs: answers.pop(0) if len(answers) > 1 else answers[0]
        try:
            import contextlib
            import io

            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                status = gui_agent.terminal_text(pathlib.Path("/unused"), {}, args, interval=0)
        finally:
            gui_agent.cli = original
        return status, out.getvalue(), err.getvalue()

    def test_a_focused_terminals_text_is_printed(self):
        self.assertEqual(self.run_with('"$ ls"\n'), (0, '"$ ls"\n', ""))

    def test_no_focused_terminal_fails_and_says_so(self):
        status, out, err = self.run_with("null\n")
        self.assertEqual((status, out), (1, ""))
        self.assertIn("no terminal holds focus", err)

    def test_wait_asks_again_until_the_text_shows(self):
        """A just-clicked terminal holds no focus yet, then shows an empty screen, then its program's output; `--wait`
        rides out both earlier answers and prints the one that has the text. The match is on the decoded JSON string,
        so text holding a quote matches although the bridge prints it escaped."""
        status, out, err = self.run_with("null\n", '""\n', '"say \\"hi\\"\\n"\n', args=["--wait", 'say "hi"'])
        self.assertEqual((status, out, err), (0, '"say \\"hi\\"\\n"\n', ""))

    def test_wait_times_out_saying_what_the_terminal_showed(self):
        """A terminal that never shows the text fails the wait, and the message carries its last screen, so the
        failure says whether the program printed something else or nothing at all."""
        status, out, err = self.run_with('"FAKE-AGENT READY"\n', args=["--wait", "echo:x", "--timeout", "0.1"])
        self.assertEqual((status, out), (1, ""))
        self.assertIn("does not show 'echo:x'", err)
        self.assertIn("FAKE-AGENT READY", err)


class HelmRequestTest(unittest.TestCase):
    """`helm_request` is how the smoke test and the screenshot batch talk to the app's embedded helm, with the credential
    the app wrote to `helm.json`. Some routes answer without a body (a preference write answers 204), and decoding that
    as JSON raised, which broke a smoke step's cleanup."""

    def serve(self, status: int, body: bytes):
        """A helm stand-in on a loopback port of its own that answers every request with `status` and `body`, recording
        each request's method, path and Authorization header; a run directory whose `helm.json` points at it."""
        seen = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def answer(self):
                length = int(self.headers.get("Content-Length") or 0)
                self.rfile.read(length)
                seen.append((self.command, self.path, self.headers.get("Authorization")))
                self.send_response(status)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            do_GET = do_PUT = answer

            def log_message(self, *args):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        run = pathlib.Path(tempfile.mkdtemp(prefix="gui-agent-helm-"))
        self.addCleanup(shutil.rmtree, run)
        (run / "helm.json").write_text(
            json.dumps({"api_base": f"http://127.0.0.1:{server.server_address[1]}", "credential": "secret"})
        )
        return run, seen

    def test_a_reply_without_a_body_is_none(self):
        """A 204 with no body gives None rather than a decoding error, and the request carries the credential."""
        run, seen = self.serve(204, b"")
        self.assertIsNone(gui_agent.helm_request(run, "PUT", "/api/preferences", {"field": True}))
        self.assertEqual(seen, [("PUT", "/api/preferences", "Bearer secret")])

    def test_a_reply_with_a_body_is_decoded(self):
        """A reply with a JSON body gives the decoded value."""
        run, _ = self.serve(200, b'{"hosts": []}')
        self.assertEqual(gui_agent.helm_request(run, "GET", "/api/hosts"), {"hosts": []})


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
