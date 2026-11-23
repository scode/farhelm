"""The parts of gui-agent.py that decide what a run may touch: its paths, the app's environment, a staged run's private
HOME and PATH and its remote supervisors, the seeding requests and the processes `stop` signals; the waits scenes rely
on (a seeded fleet's statuses, `terminal-text --wait`); and how a request to the app's helm reads its reply.

These start no compositor and build nothing; the process tests start only `sleep`, Python children of their own and, for
a staged run's sessions, the machine's own login bash with a stand-in for the fake agent. The
end-to-end run is the check that the pieces work together (docs/gui-agent.md); these pin the rules that keep a run from
reaching outside its own directory: a socket path that cannot fit, a display or bus inherited from the caller's desktop,
a state directory that defaults to a real installation's, or a recorded pid that now names someone else's process.
"""

import argparse
import copy
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
import urllib.error
import urllib.parse

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
    (a constant, a call, a value on the next line), so such an entry fails the wrapper test instead of being skipped.
    The one exception is the custom harness's `unreachable!` arm: a custom agent runs its registered command, never a
    program this match names, and a staged run registers none."""
    body = source[source.index("fn program(harness: LaunchHarness)"):]
    body = body[: body.index("\n}\n")]
    programs = re.findall(r'LaunchHarness::\w+ => "([^"]+)"', body)
    no_program = len(re.findall(r"LaunchHarness::Custom => unreachable!\(", body))
    arms = body.count("=>")
    if arms != len(programs) + no_program:
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
        a token) is gone, and so is the caller's ssh agent (`SSH_AUTH_SOCK`), since the staged hosts are reached with
        the run's own key through its private sshd; whatever else the caller set passes through; and the caller's own
        dictionary is left as it was."""
        before = dict(self.caller)
        env = gui_agent.staged_env(self.caller, self.staging)
        self.assertEqual(self.caller, before)
        self.assertEqual(env["HOME"], str(self.run_dir / "staging" / "home"))
        self.assertEqual(env["PATH"], f"{self.run_dir / 'staging' / 'bin'}:{gui_agent.STAGED_SYSTEM_PATH}")
        self.assertEqual(env["SHELL"], gui_agent.STAGED_SHELL)
        dropped = (*gui_agent.STAGED_DROPPED_ENV, "CLAUDE_CONFIG_DIR", "CODEX_HOME", "ANTHROPIC_API_KEY",
                   "CLAUDE_CODE_OAUTH_TOKEN", "OPENAI_API_KEY", "OPENROUTER_API_KEY", "GITHUB_TOKEN", "AWS_PROFILE",
                   "SSH_AUTH_SOCK")
        for name in dropped:
            self.assertNotIn(name, env)
            self.assertNotIn(name, self.env)
        self.assertEqual(env["LANG"], "C.UTF-8")
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
        names are exactly the programs the helm composes a launch with (`fn program` in
        crates/farhelm-helm/src/launches.rs), read from that source, so a harness added there fails this until it is
        added here, either as an agent with its wrapper in the run's PATH directory or as one of the shells
        (STAGED_REAL_SHELLS), which get none, since a staged shell session runs the machine's own shell. An arm the
        reader cannot take a name from fails the read rather than being skipped, shown on a synthetic function with a
        constant arm."""
        source = (pathlib.Path(__file__).resolve().parent.parent / "crates/farhelm-helm/src/launches.rs").read_text()
        programs = launch_programs(source)
        self.assertGreater(len(programs), 5, "premise: the match was found and read")
        self.assertEqual(sorted(gui_agent.STAGED_AGENTS + gui_agent.STAGED_REAL_SHELLS), sorted(programs))
        for name in gui_agent.STAGED_AGENTS:
            self.assertTrue(os.access(pathlib.Path(self.staging["bin"]) / name, os.X_OK), name)
        for name in gui_agent.STAGED_REAL_SHELLS:
            self.assertFalse((pathlib.Path(self.staging["bin"]) / name).exists(), name)
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


def command_lines() -> list[str]:
    """Every process's command line, from `ps` (the same call on Linux and macOS)."""
    return subprocess.run(["ps", "-A", "-ww", "-o", "args="], capture_output=True, text=True, check=True).stdout.split("\n")


class PrivateSshdTest(unittest.TestCase):
    """A staged run reaches its remote hosts, which are this machine, through a private sshd of its own and an `ssh`
    shim first on its PATH, so it needs no passwordless ssh to itself and touches no account's `~/.ssh`. These start the
    machine's real `sshd` and `ssh` (the arrangement is only as good as the real programs' reading of the files the run
    writes), and pin what the shim reaches, what it refuses, and that `stop` leaves nothing of it running."""

    def setUp(self):
        for program in ("ssh", "ssh-keygen"):
            if not shutil.which(program):
                self.skipTest(f"SKIPPED: no {program} on this machine")
        if not (shutil.which("sshd") or pathlib.Path("/usr/sbin/sshd").is_file()):
            self.skipTest("SKIPPED: no sshd on this machine")
        # Short, because macOS limits a unix socket path to 104 bytes and the client's connection socket lives here.
        self.run_dir = pathlib.Path(tempfile.mkdtemp(prefix="ga-ssh-", dir="/tmp"))
        (self.run_dir / "logs").mkdir()
        self.bin = self.run_dir / "bin"
        self.bin.mkdir()
        self.addCleanup(shutil.rmtree, self.run_dir)
        self.user = gui_agent.pwd.getpwuid(os.getuid()).pw_name
        self.records: list[dict] = []

        def started(record: dict) -> None:
            # Cleanup is registered the moment an sshd exists, so one that `start_private_sshd` leaves by failing
            # later (a readiness timeout) is stopped too; setUp's failure skips tearDown, never addCleanup.
            if not self.records:
                self.addCleanup(gui_agent.stop_private_sshd, record)
            self.records.append(dict(record))

        self.sshd = gui_agent.start_private_sshd(
            self.run_dir, ["localhost", f"{self.user}@localhost"], self.bin, started
        )

    def shim(self, *args: str, timeout: float = 30) -> subprocess.CompletedProcess:
        """The run's shim with BatchMode on, as the helm dials (a password prompt would hang the test otherwise)."""
        return subprocess.run([str(self.bin / "ssh"), "-o", "BatchMode=yes", *args], stdin=subprocess.DEVNULL,
                              capture_output=True, text=True, timeout=timeout)

    def test_the_staged_destinations_reach_the_private_sshd_with_the_runs_own_files(self):
        """Specified: both staged spellings of this machine log in without a password through the shim, as the caller's
        own user, at the run's sshd (its port on 127.0.0.1); ssh resolves them to the run's key, known_hosts and no
        agent, so nothing in the account's `~/.ssh` is read; and the shim is the one the sshd record names. The sshd is
        recorded before `start` waits on it."""
        self.assertEqual(self.sshd["shim"], str(self.bin / "ssh"))
        self.assertTrue(gui_agent.running(self.records[0]["pid"], self.records[0]["start"]))
        for destination in ("localhost", f"{self.user}@localhost"):
            with self.subTest(destination=destination):
                result = self.shim(destination, "id", "-un")
                self.assertEqual((result.returncode, result.stdout.strip()), (0, self.user), result.stderr)
                resolved = self.shim("-G", destination).stdout.lower().splitlines()
                directory = str(self.run_dir / "sshd").lower()
                for line in ("hostname 127.0.0.1", f"port {self.sshd['port']}", "identityagent none",
                             f"identityfile {directory}/client_key", f"userknownhostsfile {directory}/known_hosts",
                             "globalknownhostsfile /dev/null", "identitiesonly yes"):
                    self.assertIn(line, resolved)

    def test_every_other_destination_is_refused(self):
        """The helm's dials from a staged run must reach only the run's sshd, also for a host an agent driving the window
        adds. Specified: a destination that is not one of the staged spellings fails through the shim without connecting
        anywhere."""
        result = self.shim("elsewhere.example", "true")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("a staged run reaches only its staged hosts", result.stderr)

    def test_stop_leaves_nothing_of_the_sshd_or_its_clients(self):
        """The helm's shared connections detach from the app (`ControlPersist`) and could outlive it. Specified: with a
        detached shared connection open through the shim, `stop_private_sshd` ends the sshd and its connection's
        processes and reports no problem, and the detached client, which `stop` never signals, ends by itself within 10
        seconds because its connection is gone; nothing naming the run directory is left running."""
        control = self.run_dir / "c"
        opened = self.shim("-o", "ControlMaster=auto", "-o", f"ControlPath={control}", "-o", "ControlPersist=60",
                           "localhost", "true")
        self.assertEqual(opened.returncode, 0, opened.stderr)
        self.assertTrue(any(line.startswith(f"ssh: {control}") for line in command_lines()),
                        "premise: a detached shared connection is running")
        self.assertEqual(gui_agent.stop_private_sshd(self.sshd), [])
        deadline = time.monotonic() + 10

        def left() -> list[str]:
            return [line for line in command_lines() if str(self.run_dir) in line]

        while time.monotonic() < deadline and left():
            time.sleep(0.05)  # sleep-ok: polling for processes the kernel is still reaping; there is no event
        self.assertEqual(left(), [])
        self.assertFalse(gui_agent.running(self.sshd["pid"], self.sshd["start"]))

    def test_the_configurations_name_only_the_runs_files(self):
        """The written files are the whole arrangement. Specified: the sshd listens on loopback only, lets in only the
        caller's user by public key with the run's authorized key (marked `restrict`), and offers no forwarding or
        terminal; the client file sends exactly the staged hosts (each host part once) to that port; and the shim runs
        the real ssh with that file and every argument."""
        directory = self.run_dir / "sshd"
        config = (directory / "sshd_config").read_text().splitlines()
        for line in ("ListenAddress 127.0.0.1", f"AllowUsers {self.user}", "PasswordAuthentication no",
                     f"AuthorizedKeysFile {directory}/authorized_keys", "AllowTcpForwarding no", "PermitTTY no"):
            self.assertIn(line, config)
        self.assertTrue((directory / "authorized_keys").read_text().startswith("restrict ssh-ed25519 "))
        client = gui_agent.ssh_client_config_text(directory, 4022, ["localhost", "me@localhost", "127.0.0.1"])
        self.assertEqual(client.splitlines()[1], "Host localhost 127.0.0.1")
        self.assertIn("  Port 4022", client.splitlines())
        shim = (self.bin / "ssh").read_text()
        self.assertIn(f"-F {directory}/ssh_config \"$@\"", shim)
        self.assertEqual(directory.stat().st_mode & 0o777, 0o700)


# Stand-ins for `systemctl` and `podman`, for what `stop` runs in a user-session run. `systemctl --user list-units ...`
# prints one `<unit> loaded active running stand-in` line per line of the file `units` beside it; `systemctl --user stop
# -- UNIT` appends UNIT to `stopped` and drops it from `units`, unless the file `stuck` names it. `podman ...` appends
# its arguments as one JSON line to `podman.log`. Both append the XDG_RUNTIME_DIR they were given to `runtimes`.
USER_MANAGER_STANDIN = """\
import json, os, pathlib, sys
here = pathlib.Path(sys.argv[0]).parent
args = sys.argv[1:]
with open(here / "runtimes", "a") as log:
    log.write(os.environ.get("XDG_RUNTIME_DIR", "") + "\\n")
if pathlib.Path(sys.argv[0]).name == "podman":
    with open(here / "podman.log", "a") as log:
        log.write(json.dumps(args) + "\\n")
    sys.exit(0)
units = (here / "units").read_text().split()
if args[:2] == ["--user", "list-units"]:
    print("".join(f"{unit} loaded active running stand-in\\n" for unit in units), end="")
elif args[:3] == ["--user", "stop", "--"]:
    with open(here / "stopped", "a") as log:
        log.write(args[3] + "\\n")
    stuck = (here / "stuck").read_text().split() if (here / "stuck").exists() else []
    if args[3] not in stuck:
        (here / "units").write_text("".join(f"{unit}\\n" for unit in units if unit != args[3]))
else:
    sys.exit(f"stand-in systemctl: unexpected {args}")
"""


class UserSessionTest(unittest.TestCase):
    """A `--user-session` run gives the app the caller's runtime directory and session bus, so its helm's Podman check
    reaches the user's systemd manager, and in return shares that manager and the account's Podman storage. These pin
    both halves: exactly those two variables cross over and nothing else of the run's isolation moves, and what `stop`
    removes from the shared account is exactly what the run's own helm and supervisor named, never another run's or the
    live install's."""

    SESSION = "2b1f0e4c-0000-4000-8000-000000000001"
    TAB = "9c3d5a71-0000-4000-8000-0000000000ff"
    CONTAINER = "farhelm-sess-k3x9q2m7a1bz-dev-1"
    NONCE = "0123456789abcdef0123456789abcdef"

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir)
        self.caller = {
            "PATH": "/usr/bin:/bin",
            "HOME": "/home/caller",
            "XDG_RUNTIME_DIR": "/run/user/1234",
            "DBUS_SESSION_BUS_ADDRESS": "unix:path=/run/user/1234/bus",
            "WAYLAND_DISPLAY": "wayland-0",
            "DISPLAY": ":0",
            "FARHELM_DESKTOP_STATE_DIR": "/home/caller/.local/state/farhelm",
        }
        # The compositor's facts, as `start` records them on Linux.
        self.record = {"runtime": "/tmp/run/runtime", "display": "wayland-fh-x", "dbus": "unix:path=/tmp/run/bus"}

    def test_the_option_passes_the_callers_runtime_and_bus_through(self):
        """Specified: the app gets the caller's XDG_RUNTIME_DIR and DBUS_SESSION_BUS_ADDRESS, as `check_user_session`
        took them from the caller at `start`; its Wayland display is the run's own compositor, named by the socket's
        absolute path since a bare name would be looked up in the caller's runtime directory; and the caller's own
        display and `FARHELM_*` variables are still dropped, with rendering still pinned to lavapipe."""
        user = gui_agent.check_user_session(self.caller, "linux", staged=False)
        self.assertEqual(user, {"runtime": "/run/user/1234", "dbus": "unix:path=/run/user/1234/bus"})
        env = gui_agent.run_env(self.dir, dict(self.record, user_session=user), base=self.caller, platform="linux")
        self.assertEqual(env["XDG_RUNTIME_DIR"], "/run/user/1234")
        self.assertEqual(env["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/run/user/1234/bus")
        self.assertEqual(env["WAYLAND_DISPLAY"], "/tmp/run/runtime/wayland-fh-x")
        self.assertNotIn("DISPLAY", env)
        self.assertNotIn("FARHELM_DESKTOP_STATE_DIR", env)
        self.assertEqual(env["VK_ICD_FILENAMES"], gui_agent.LAVAPIPE_ICD)
        self.assertEqual(env["HOME"], "/home/caller")

    def test_a_plain_run_still_isolates_the_runtime_and_bus(self):
        """Without the option nothing changes: the app gets the run's private runtime directory, its compositor's bus
        and the display's bare name, never the caller's runtime directory or bus. Specified against the same caller
        environment the option's test uses, so a change that leaked the caller's session into every run fails here."""
        env = gui_agent.run_env(self.dir, self.record, base=self.caller, platform="linux")
        self.assertEqual(
            (env["XDG_RUNTIME_DIR"], env["DBUS_SESSION_BUS_ADDRESS"], env["WAYLAND_DISPLAY"]),
            ("/tmp/run/runtime", "unix:path=/tmp/run/bus", "wayland-fh-x"),
        )

    def test_the_option_is_refused_where_it_cannot_work(self):
        """Specified: refused on a Mac (whose runs already use the GUI session's environment), for a caller without an
        absolute runtime directory or without a bus, and for a staged run, whose private HOME would give Podman a
        storage of its own sharing the account's run root. Each refusal is checked by its message, since all are the
        same exception type."""
        cases = [
            (self.caller, "darwin", False, "Linux only"),
            (self.caller, "linux", True, "a staged run"),
            ({**self.caller, "XDG_RUNTIME_DIR": "run/user/1234"}, "linux", False, "needs XDG_RUNTIME_DIR"),
            ({"XDG_RUNTIME_DIR": "/run/user/1234"}, "linux", False, "needs XDG_RUNTIME_DIR"),
        ]
        for base, platform, staged, message in cases:
            with self.subTest(message=message, platform=platform), self.assertRaisesRegex(gui_agent.UsageError,
                                                                                            message):
                gui_agent.check_user_session(base, platform, staged)

    def test_a_staged_start_with_the_option_is_refused_before_anything_starts(self):
        """The combination fails as a usage error before `start` creates the run or dials any destination over ssh.
        Driven through `start` itself with the ssh check replaced by a recorder (a module attribute, not the
        environment), so a refusal moved after the ssh checks or the run's creation fails here."""
        dialed = []
        real_check = gui_agent.check_ssh
        gui_agent.check_ssh = dialed.append
        try:
            with self.assertRaisesRegex(gui_agent.UsageError, "--user-session"):
                gui_agent.start(self.dir / "run", ["--user-session", "--remote-host", "localhost"])
        finally:
            gui_agent.check_ssh = real_check
        self.assertEqual(dialed, [])
        self.assertFalse((self.dir / "run").exists())

    def test_only_the_runs_own_units_match(self):
        """The user manager is shared with every other run and with the live install, whose units all start with
        `farhelm-` too. Specified: a recorded session's launch scopes (any generation) and tab scopes match; another
        session's scopes, a near miss on the name, the retired container scope, a Podman command's scope, the throwaway
        probe scopes and services do not."""
        ours = [
            f"farhelm-{self.SESSION}-0.scope",
            f"farhelm-{self.SESSION}-12.scope",
            f"farhelm-{self.SESSION}-tab-{self.TAB}.scope",
        ]
        other = "5e6f7a8b-0000-4000-8000-000000000002"
        strangers = [
            f"farhelm-{other}-0.scope",
            f"farhelm-{self.SESSION}.scope",
            f"farhelm-{self.SESSION}-tab-x.scope",
            f"farhelm-{self.SESSION}-0x.scope",
            f"farhelm-{self.SESSION}-0.service",
            f"farhelm-{self.SESSION}-container.scope",
            f"farhelm-env-{self.CONTAINER}-{self.NONCE}.scope",
            f"farhelm-env-probe-{self.NONCE}.scope",
            f"farhelm-probe-{other}.scope",
            "farhelm-supervisor.service",
        ]
        for unit in ours:
            self.assertTrue(gui_agent.run_owned_unit(unit, [self.SESSION]), unit)
        for unit in strangers:
            self.assertFalse(gui_agent.run_owned_unit(unit, [self.SESSION]), unit)

    def test_stop_records_only_the_local_hosts_sessions_and_containers(self):
        """A user-session run keeps the caller's HOME and ssh setup, so a host added while it is driven can be this
        machine's own installation over ssh, whose sessions' scopes sit on the same user manager: recording them would
        make `stop` end the live install's sessions. Specified: of the sessions the helm lists as `<host entry>:<id>`
        (the shape a live run's helm answered with), only the bare ids of those on the `kind: local` host are recorded,
        and only UUID-shaped ones; of the containers, only those placed on the local host with the helm's name prefix;
        a podman row without `environment` is skipped rather than losing the record; and a later stop that finds the
        helm gone keeps what was recorded, with a warning."""
        entry, other = "313fe647-22b9-4525-acd6-7f4519854719", "5e6f7a8b-0000-4000-8000-000000000002"

        def placed(host_id: int, name: str, place: int) -> dict:
            target = {"kind": "environment", "recipe": "dev", "place": {"kind": "podman_host", "host": place}}
            return {"id": host_id, "kind": "podman", "environment": {"name": name, "target": target}}

        def answer(_run, _method, path, _body=None):
            if path == "/api/sessions":
                return {"sessions": [{"id": f"{entry}:{self.SESSION}", "host": 1},
                                     {"id": f"{entry}:not-a-uuid", "host": 1},
                                     {"id": f"{entry}:{other}", "host": 2},
                                     {"id": f"{entry}:{self.TAB}", "host": 3}]}
            return {"hosts": [{"id": 1, "kind": "local"}, {"id": 2, "kind": "ssh"},
                              placed(3, self.CONTAINER, 1), placed(4, "farhelm-sess-k3x9-dev-2", 2),
                              placed(5, "someone-elses", 1), {"id": 6, "kind": "podman"}]}

        def gone(*_args, **_kwargs):
            raise urllib.error.URLError("connection refused")

        run = self.dir / "run"
        run.mkdir()
        (run / "helm.json").write_text("{}")
        record = {"user_session": {"runtime": "/run/user/1234", "dbus": "unix:path=/b"}}
        self.assertIsNone(gui_agent.record_user_session_objects(run, record, request=answer))
        self.assertIn("removing only what an earlier stop recorded",
                      gui_agent.record_user_session_objects(run, record, request=gone))
        user = record["user_session"]
        self.assertEqual((user["sessions"], user["containers"]), ([self.SESSION], [self.CONTAINER]))
        self.assertEqual(json.loads((run / "run.json").read_text()), record)

    def test_stop_removes_exactly_what_was_recorded(self):
        """What `stop` takes from the shared account is what it recorded from the run's helm before the app quit.
        Specified: removal runs `podman rm --force --ignore` once per recorded container and stops by exact name every
        listed unit of a recorded session and no other, with the recorded runtime directory; a unit that will not stop
        is reported; and with nothing recorded it runs nothing, so an unreachable user manager cannot fail `stop` (and
        keep its lock) for a run that made nothing. The premise is checked first: the listing holds both the run's units
        and strangers'."""
        user = {"runtime": "/run/user/1234", "dbus": "unix:path=/b", "sessions": [self.SESSION],
                "containers": [self.CONTAINER]}
        bin_dir = self.dir / "bin"
        systemctl = write_executable(bin_dir / "systemctl", f"#!{sys.executable}\n{USER_MANAGER_STANDIN}")
        podman = write_executable(bin_dir / "podman", f"#!{sys.executable}\n{USER_MANAGER_STANDIN}")
        ours = [f"farhelm-{self.SESSION}-0.scope", f"farhelm-{self.SESSION}-tab-{self.TAB}.scope"]
        strangers = ["farhelm-5e6f7a8b-0000-4000-8000-000000000002-0.scope",
                     f"farhelm-env-{self.CONTAINER}-{self.NONCE}.scope"]
        (bin_dir / "units").write_text("\n".join(ours + strangers) + "\n")
        listed = subprocess.run([systemctl, "--user", "list-units"], capture_output=True, text=True).stdout
        self.assertEqual([line.split()[0] for line in listed.splitlines()], ours + strangers, "premise")
        # The premise listing ran in this process's own environment; only the removal's runtimes are checked.
        (bin_dir / "runtimes").unlink()

        problems = gui_agent.remove_user_session_objects(user, {"PATH": "/usr/bin:/bin"}, str(podman), str(systemctl))
        self.assertEqual(problems, [])
        self.assertEqual([json.loads(line) for line in (bin_dir / "podman.log").read_text().splitlines()],
                         [["rm", "--force", "--ignore", "--", self.CONTAINER]])
        self.assertEqual((bin_dir / "stopped").read_text().split(), ours)
        self.assertEqual((bin_dir / "units").read_text().split(), strangers)
        self.assertEqual(set((bin_dir / "runtimes").read_text().split()), {"/run/user/1234"})

        (bin_dir / "units").write_text(ours[0] + "\n")
        (bin_dir / "stuck").write_text(ours[0] + "\n")
        problems = gui_agent.remove_user_session_objects(user, {"PATH": "/usr/bin:/bin"}, str(podman), str(systemctl))
        self.assertEqual(problems, [f"the run's units are still on the user manager: {ours[0]}"])

        nothing = {"runtime": "/run/user/1234", "dbus": "unix:path=/b"}
        missing = str(self.dir / "no-such-systemctl")
        self.assertEqual(gui_agent.remove_user_session_objects(nothing, {}, missing, missing), [])


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


# The checkout's scenario files: the README hero's, which `start --scenario` exists for first, and the two other flows'.
REPO = pathlib.Path(__file__).resolve().parent.parent
HERO = REPO / "docs" / "readme-hero" / "scenario.json5"
SCENARIOS = [HERO, REPO / "docs" / "readme-video" / "scenario.json5", REPO / "docs" / "docs-shots" / "scenario.json5"]


def scenario_text(hosts: str, sessions: str) -> str:
    """A small scenario file's text with the given host and session entries, for the refusal and field tests."""
    return f"{{ viewport: {{ width: 800, height: 600, scale: 1 }}, hosts: [{hosts}], sessions: [{sessions}] }}"


# A one-host scenario whose only session is open and replays `t.txt`; tests swap pieces of it for a mistake.
LOCAL_HOST = '{ key: "local", alias: "here", kind: "local" }'
OPEN_SESSION = ('{ title: "a", host: "local", cwd: "~/a", wrapper: "claude", status: "running", age: 0, open: true, '
                'transcript: "t.txt" }')


class ScenarioTest(unittest.TestCase):
    """`start --scenario FILE` stages a docs-media scenario's fleet: these pin what it reads from the file (which hosts,
    which sessions with which fields), what it refuses before anything starts, and the requests it turns the scenario
    into. Each is a picture that would otherwise look plausible and be wrong: a session on the wrong host, a YOLO launch
    shown as guarded, the account's name in a public file, or the window opening some other session than the
    scenario's."""

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir)
        (self.dir / "t.txt").write_text("a line\n")

    def write(self, hosts: str = LOCAL_HOST, sessions: str = OPEN_SESSION, name: str = "scenario") -> pathlib.Path:
        """Write a scenario named `name` beside `t.txt` and return its path."""
        path = self.dir / f"{name}.json5"
        path.write_text(scenario_text(hosts, sessions))
        return path

    def test_the_hero_scenario_becomes_its_hosts_and_sessions(self):
        """The README hero is what the option is for. Specified: its three hosts in order with their aliases, the local
        one and two remotes at `localhost` and `$USER@localhost` with `$USER` expanded to the account given, which are
        the run's `--remote-host` destinations; its seven sessions in the file's order, each on its host, with its
        wrapper, YOLO launch, status (and the replay shape that holds it), seen state and transcript resolved beside the
        file; and exactly one open, created first."""
        plan = gui_agent.load_scenario(HERO, "someone")
        self.assertEqual(
            [(host["key"], host["alias"], host["kind"], host.get("ssh")) for host in plan["hosts"]],
            [("local", "this machine", "local", None), ("build-box", "build-box", "remote", "localhost"),
             ("gpu-1", "gpu-1", "remote", "someone@localhost")],
        )
        self.assertEqual(gui_agent.scenario_remote_hosts(plan), ["localhost", "someone@localhost"])
        rows = [(s["title"], s["host"], s["wrapper"], s["yolo"], s["status"], s["then"], s.get("seen"), s["open"])
                for s in plan["sessions"]]
        self.assertEqual(rows, [
            ("fix flaky attach test", "build-box", "claude", True, "running", "spin", None, True),
            ("migrate auth tokens", "local", "codex", False, "waiting", "menu", None, False),
            ("readme rewrite", "gpu-1", "claude", True, "idle", "quiet", False, False),
            ("bench tokenizer on H100", "gpu-1", "claude", False, "running", "spin", None, False),
            ("triage crash reports", "local", "claude", False, "idle", "quiet", True, False),
            ("release 0.3.1", "build-box", "codex", False, "exited", "exit", None, False),
            ("port ci to nextest", "build-box", "codex", True, "idle", "quiet", False, False),
        ])
        self.assertEqual([s["transcript"].name for s in plan["sessions"]][:2], ["transcript.txt", "filler-codex.txt"])
        for session in plan["sessions"]:
            self.assertEqual(session["transcript"].parent, HERO.parent)
            self.assertTrue(session["transcript"].is_absolute() and session["transcript"].is_file())
        self.assertEqual(gui_agent.scenario_creation_order(plan), list(range(7)))

    def test_every_committed_scenario_is_accepted(self):
        """The demo video and the docs screenshots will stage their scenarios the same way, so the checks must accept
        all three files as they are (they are frozen designs; a check that refused one would be the check's bug)."""
        for path in SCENARIOS:
            with self.subTest(path=path.parent.name):
                plan = gui_agent.load_scenario(path, "someone")
                self.assertGreater(len(plan["sessions"]), 1)
                self.assertEqual(sum(session["open"] for session in plan["sessions"]), 1)

    def test_the_open_session_is_created_first(self):
        """The window opens the first session its list shows and keeps it, so the scenario's open session must be the
        first created whatever its place in the file. Specified: the open one first, then every other in file order."""
        sessions = ", ".join([
            OPEN_SESSION.replace('title: "a"', 'title: "first"').replace(", open: true", ""),
            OPEN_SESSION.replace('title: "a"', 'title: "second"'),
            OPEN_SESSION.replace('title: "a"', 'title: "third"').replace(", open: true", ""),
        ])
        plan = gui_agent.load_scenario(self.write(sessions=sessions), "someone")
        self.assertEqual(gui_agent.scenario_creation_order(plan), [1, 0, 2])

    def test_each_session_becomes_an_agent_launch_of_its_wrappers_harness(self):
        """The helm records a session as an agent session, and the details bar shows the command it composed rather
        than a stored command with its `{farhelm_args}` placeholder, only when the create is an agent launch, the one
        the launcher's Agent tab sends; the migration decided on 2026-11-16 that the staging launches the way a user
        does. Specified for each hero session: an agent launch (`launch`, no `command`) of the wrapper's harness with
        no model or effort, YOLO and confirmed only on a `yolo` session and Guarded on every other, titled, on the
        helm's id of its host, in the given directory, with exactly the fields the helm's create takes."""
        plan = gui_agent.load_scenario(HERO, "someone")
        host_ids = {"local": 1, "build-box": 2, "gpu-1": 3}
        launches = []
        for session in plan["sessions"]:
            body = gui_agent.scenario_session_body(session, host_ids[session["host"]], pathlib.Path("/w/0"))
            self.assertEqual(set(body), {"host", "cwd", "title", "launch", "confirm_yolo"})
            self.assertEqual((body["title"], body["host"], body["cwd"]),
                             (session["title"], host_ids[session["host"]], "/w/0"))
            self.assertEqual(body["confirm_yolo"], session["yolo"])
            launches.append(body["launch"])
        self.assertEqual(launches, [
            {"harness": harness, "model": None, "effort": None, "permissions": permissions}
            for harness, permissions in (
                ("claude", "yolo"), ("codex", "guarded"), ("claude", "yolo"), ("claude", "guarded"),
                ("claude", "guarded"), ("codex", "guarded"), ("codex", "yolo"),
            )
        ])
        # The wrapper the session's replay script sits behind must be the program the helm runs for its harness
        # (`fn program`, keyed here by the harness's snake_case wire name), or the launch would miss the replay.
        source = (pathlib.Path(__file__).resolve().parent.parent / "crates/farhelm-helm/src/launches.rs").read_text()
        body = source[source.index("fn program(harness: LaunchHarness)"):]
        programs = {re.sub(r"(?<!^)([A-Z])", r"_\1", variant).lower(): name
                    for variant, name in re.findall(r'LaunchHarness::(\w+) => "([^"]+)"', body[: body.index("\n}\n")])}
        self.assertEqual({wrapper: programs[harness] for wrapper, harness in gui_agent.SCENARIO_HARNESSES.items()},
                         {wrapper: wrapper for wrapper in gui_agent.SCENARIO_HARNESSES})

    def test_each_session_replays_its_transcript_through_its_staged_agent_script(self):
        """Every scenario session is launched as `claude` or `codex`, and the run's wrapper of that name runs the
        session directory's STAGED_AGENT_SCRIPT, which is how sessions with the same command replay different
        transcripts. Specified, by running the script with `sh` against a stand-in fake agent that prints its
        arguments: the replay script in the wrapper's dialect, the session's transcript, the replay shape its status
        needs (an exit with the scenario's exit code), then every argument the wrapper passed on, also one with a
        space; a `then` in the scenario replaces the shape its status implies."""
        fixtures = write_executable(self.dir / "fixture dir" / "farhelm-fixtures", FIXTURES_STANDIN)
        plan = gui_agent.load_scenario(HERO, "someone")
        ran = []
        for session in plan["sessions"]:
            script = self.dir / gui_agent.STAGED_AGENT_SCRIPT
            script.write_text(gui_agent.staged_agent_script(session, str(fixtures)))
            out = subprocess.run(["/bin/sh", str(script), "--settings", "a b"], capture_output=True, text=True,
                                 check=True).stdout.strip()
            ran.append(out)
        transcript = str(HERO.parent / "transcript.txt")
        self.assertEqual(ran[0], f"fixture: fake-agent --script replay --dialect claude --transcript {transcript} "
                                 "--then spin --settings a b")
        self.assertIn("--dialect codex --transcript", ran[1])
        self.assertTrue(ran[1].endswith("--then menu --settings a b"), ran[1])
        self.assertTrue(ran[4].endswith("--then quiet --settings a b"), ran[4])
        self.assertTrue(ran[5].endswith("--then exit --exit-code 0 --settings a b"), ran[5])

        plan = gui_agent.load_scenario(self.write(sessions=OPEN_SESSION.replace(
            'status: "running"', 'status: "exited", exit_code: 3, then: "quiet"')), "someone")
        self.assertIn("--then quiet", gui_agent.staged_agent_script(plan["sessions"][0], "/fx"))
        self.assertNotIn("--exit-code", gui_agent.staged_agent_script(plan["sessions"][0], "/fx"))
        plan = gui_agent.load_scenario(self.write(sessions=OPEN_SESSION.replace(
            'status: "running"', 'status: "exited", exit_code: 3')), "someone")
        self.assertIn("--then exit --exit-code 3", gui_agent.staged_agent_script(plan["sessions"][0], "/fx"))

    def test_the_window_is_the_viewport_plus_the_linux_frame(self):
        """A scenario's viewport is the size of the app's own drawing, as a capture shows it once the window frame is
        cropped. Specified: on Linux the window is larger by the frame on every side and the title bar on top, the
        numbers the screenshot batch crops (21 and 34 pixels); on a Mac it is the viewport. The scale is not applied."""
        plan = gui_agent.load_scenario(HERO, "someone")
        self.assertEqual(gui_agent.scenario_window_size(plan, "linux"), "1322x876")
        self.assertEqual(gui_agent.scenario_window_size(plan, "darwin"), "1280x800")

    def test_mistakes_in_a_scenario_are_refused(self):
        """A scenario is edited by hand, and a mistake must fail `start` naming the file and what is wrong, not stage
        a fleet that differs from the design. Specified, each by its message: a session on a host the file does not
        define, a transcript that is not a file beside it, `$USER` with no account to expand it to, and the old
        loader's other checks on what is staged (a second local host, two hosts under one alias, a duplicated title, no or two open sessions, an
        idle session without seen state, an unknown wrapper or status, and for the values the app pins on a row, a
        missing or empty `cwd`, an `age` that is missing, negative, fractional, a string or a boolean, and an age
        younger than the one above it)."""
        remote = '{ key: "r", alias: "r", kind: "remote", ssh: "$USER@localhost" }'
        second_local = LOCAL_HOST.replace("local", "l2", 1).replace('"here"', '"there"')
        same_alias = remote.replace('alias: "r"', 'alias: "here"')
        closed = OPEN_SESSION.replace(", open: true", "")
        second_open = OPEN_SESSION.replace('title: "a"', 'title: "b"')
        cases = [
            ("unknown host 'nowhere'", LOCAL_HOST, OPEN_SESSION.replace('host: "local"', 'host: "nowhere"')),
            ("transcript 'missing.txt' is not a file", LOCAL_HOST,
             OPEN_SESSION.replace('transcript: "t.txt"', 'transcript: "missing.txt"')),
            (r"uses \$USER but USER is not set", f"{LOCAL_HOST}, {remote}", OPEN_SESSION),
            ("exactly one host must be kind: local", f"{LOCAL_HOST}, {second_local}", OPEN_SESSION),
            ("duplicate host alias here", f"{LOCAL_HOST}, {same_alias}", OPEN_SESSION),
            ("duplicate session title", LOCAL_HOST, f"{OPEN_SESSION}, {closed}"),
            ("exactly one session must carry open", LOCAL_HOST, closed),
            ("exactly one session must carry open", LOCAL_HOST, f"{OPEN_SESSION}, {second_open}"),
            ("needs seen", LOCAL_HOST, OPEN_SESSION.replace('"running"', '"idle"')),
            ("wrapper must be one of", LOCAL_HOST, OPEN_SESSION.replace('"claude"', '"cursor-agent"')),
            ("status must be one of", LOCAL_HOST, OPEN_SESSION.replace('"running"', '"busy"')),
            ("cwd is required", LOCAL_HOST, OPEN_SESSION.replace('cwd: "~/a", ', "")),
            ("cwd is required", LOCAL_HOST, OPEN_SESSION.replace('"~/a"', '""')),
            ("age must be a non-negative integer", LOCAL_HOST, OPEN_SESSION.replace("age: 0", "age: -1")),
            ("age must be a non-negative integer", LOCAL_HOST, OPEN_SESSION.replace("age: 0", "age: 1.5")),
            ("age must be a non-negative integer", LOCAL_HOST, OPEN_SESSION.replace("age: 0", 'age: "2m"')),
            ("age must be a non-negative integer", LOCAL_HOST, OPEN_SESSION.replace("age: 0", "age: true")),
            ("age must be a non-negative integer", LOCAL_HOST, OPEN_SESSION.replace("age: 0, ", "")),
            ("ages must not decrease down the list", LOCAL_HOST,
             OPEN_SESSION.replace("age: 0", "age: 60") + ", " + closed.replace('title: "a"', 'title: "b"')),
        ]
        for message, hosts, sessions in cases:
            path = self.write(hosts, sessions)
            user = None if "USER" in message else "someone"
            with self.subTest(message=message), self.assertRaisesRegex(gui_agent.UsageError, message) as raised:
                gui_agent.load_scenario(path, user)
            self.assertTrue(str(raised.exception).startswith(f"{path}: "), raised.exception)

    def test_the_pins_file_carries_each_sessions_title_age_and_folder(self):
        """A scenario's sessions really run in scratch directories under the run and were active moments ago; the app
        shows the scenario's folder and age instead only for the sessions the pins file names, by title. Specified for
        the README hero: the file `start` writes into the run directory lists all seven sessions in the scenario's order
        with their title, age and folder exactly as the scenario gives them (the folder unexpanded, as the row shows
        it), and every host by its alias with empty staged values, since the hero stages none (so no staged session's
        scratch folder shows in its launcher either); and the path reaches the app under the variable its agent build
        reads (`SESSION_PINS_ENV` in `agent_startup.rs`, read from that source); a run that writes none leaves the
        variable unset, so every other run shows sessions as the helm pushes them."""
        source = (REPO / "crates/gui/farhelm-gui/src/agent_startup.rs").read_text()
        names = re.findall(r'^pub const SESSION_PINS_ENV: &str = "([^"]+)";$', source, re.MULTILINE)
        self.assertEqual(names, [gui_agent.SESSION_PINS_ENV])
        plan = gui_agent.load_scenario(HERO, "someone")
        run = self.dir / "run"
        run.mkdir()
        path = gui_agent.write_session_pins(run, plan)
        self.assertEqual(path, run / "session-pins.json")
        self.assertEqual(json.loads(path.read_text()), {"sessions": [
            {"title": "fix flaky attach test", "age": 120, "cwd": "~/src/farhelm"},
            {"title": "migrate auth tokens", "age": 480, "cwd": "~/src/api"},
            {"title": "readme rewrite", "age": 2460, "cwd": "~/work/docs"},
            {"title": "bench tokenizer on H100", "age": 3600, "cwd": "~/work/tok"},
            {"title": "triage crash reports", "age": 10800, "cwd": "~/src/api"},
            {"title": "release 0.3.1", "age": 86400, "cwd": "~/src/farhelm"},
            {"title": "port ci to nextest", "age": 172800, "cwd": "~/src/ci"},
        ], "hosts": [
            {"alias": alias, "launches": [], "folders": [], "trash": [], "files": []}
            for alias in ("this machine", "build-box", "gpu-1")
        ]})
        args = gui_agent.app_env_args(run, {"farhelm": "/t/f"}, session_pins=path)
        pairs = dict(arg.split("=", 1) for arg in args[1::2])
        self.assertEqual(pairs[gui_agent.SESSION_PINS_ENV], str(path))
        args = gui_agent.app_env_args(run, {"farhelm": "/t/f"})
        self.assertNotIn(gui_agent.SESSION_PINS_ENV, dict(arg.split("=", 1) for arg in args[1::2]))

    def test_the_pins_file_stages_each_hosts_history_trash_and_files(self):
        """The docs screenshots show the launcher's recent setups and folders, the trash and a file hover, none of which
        a staged run can answer honestly, so the app substitutes the scenario's for each host, which it finds by alias
        (`agent_startup.rs`, "Staged host values"). Specified for the docs scenario: each host's entry lists exactly its
        own values in the file's order, a recent setup's choices in the wire's `LaunchSelection` shape (model, effort and
        permissions null unless given, workspace trust only when given), the trash entries and files with their fields
        as written; a host the scenario gives nothing has every list empty."""
        plan = gui_agent.load_scenario(REPO / "docs" / "docs-shots" / "scenario.json5", "someone")
        hosts = {host["alias"]: host for host in gui_agent.session_pins_body(plan)["hosts"]}
        self.assertEqual(list(hosts), ["this machine", "build-box", "gpu-1"])
        local = hosts["this machine"]
        self.assertEqual(local["launches"][:3], [
            {"cwd": "~/src/farhelm", "selection": {"harness": "claude", "model": None, "effort": None,
                                                   "permissions": None}, "age": 600},
            {"cwd": "~/src/farhelm", "selection": {"harness": "claude", "model": None, "effort": None,
                                                   "permissions": None}, "age": 7200},
            {"cwd": "~/src/farhelm", "selection": {"harness": "codex", "model": None, "effort": None,
                                                   "permissions": "yolo"}, "age": 3600},
        ])
        self.assertEqual(local["launches"][3]["selection"]["workspace_trust"], True)
        self.assertEqual([folder["cwd"] for folder in local["folders"]], ["~/src/farhelm", "~/src/api", "~/work/notes"])
        self.assertEqual([entry["checkout"] for entry in local["trash"]], ["web-fix-login", "api-2"])
        self.assertEqual(local["trash"][0], {"repository": "acme/web", "checkout": "web-fix-login",
                                             "root": "/Users/you/work", "age": 5400, "bytes": 48200000})
        self.assertEqual(local["files"], [{"path": "e2e/attach.spec.ts",
                                           "shown_path": "/Users/you/src/farhelm/e2e/attach.spec.ts", "size": 6412}])
        self.assertEqual(hosts["build-box"], {"alias": "build-box", "launches": [], "folders": [], "files": [],
                                              "trash": [{"repository": "acme/api", "checkout": "api-1",
                                                         "root": "/home/dev/work", "age": 86400, "bytes": 187000000}]})
        self.assertEqual(hosts["gpu-1"], {"alias": "gpu-1", "launches": [], "folders": [], "trash": [], "files": []})

    def test_the_launch_choices_are_the_wires(self):
        """A recent setup's choices reach the app in the wire's names, which it reads strictly, so the harness's lists
        must be the wire's own, or a scenario the harness accepts would fail the app's launch (or one it refuses would
        have worked). Specified against crates/farhelm-proto/src/launch.rs: the harnesses are every `LaunchHarness`
        variant but `custom`, which needs a registered custom agent a scenario cannot name, the efforts every
        `LaunchEffort`, and the permissions every `LaunchPermission` but `custom`, which needs flags a scenario cannot
        give, each in snake_case."""
        source = (REPO / "crates/farhelm-proto/src/launch.rs").read_text()

        def variants(name: str) -> list[str]:
            body = source[source.index(f"pub enum {name} {{"):]
            body = body[: body.index("\n}")]
            words = re.findall(r"^\s*([A-Z]\w*),", body, re.MULTILINE)
            return [re.sub(r"(?<!^)([A-Z])", r"_\1", word).lower() for word in words]

        self.assertEqual(sorted(gui_agent.LAUNCH_HARNESSES + ("custom",)), sorted(variants("LaunchHarness")))
        self.assertEqual(sorted(gui_agent.LAUNCH_EFFORTS), sorted(variants("LaunchEffort")))
        self.assertEqual(sorted(gui_agent.LAUNCH_PERMISSIONS + ("custom",)), sorted(variants("LaunchPermission")))

    def test_mistakes_in_the_staged_host_values_are_refused(self):
        """The staged host values are edited by hand like the rest of the scenario, and a typo would stage nothing and
        leave a picture that looks plausible and is wrong. Specified, each by its message naming the entry: a field
        that is not a list, an entry that is not an object, an unknown or missing key, an empty string, a negative or
        boolean number, an unknown host, a harness, effort or permission the wire does not know (`custom` included),
        and a trash root that is not absolute."""
        setup = '{ host: "local", cwd: "~/a", harness: "claude", age: 1 }'
        trash = '{ host: "local", repository: "a/b", checkout: "c", root: "/w", age: 1, bytes: 2 }'
        cases = [
            ("launch_history must be a list", "launch_history: {}"),
            (r"folders\[0\] must be an object", "folders: [1]"),
            (r"launch_history\[0\]: unknown field colour", f"launch_history: [{setup[:-2]}, colour: 1 }}]"),
            (r"terminal_files\[0\]: size is required",
             'terminal_files: [{ host: "local", path: "a", shown_path: "/a" }]'),
            (r"folders\[0\]: cwd must be a non-empty string", 'folders: [{ host: "local", cwd: "", age: 1 }]'),
            (r"folders\[0\]: age must be a non-negative integer",
             'folders: [{ host: "local", cwd: "~/a", age: -1 }]'),
            (r"checkout_trash\[0\]: bytes must be a non-negative integer",
             f"checkout_trash: [{trash.replace('bytes: 2', 'bytes: true')}]"),
            (r"launch_history\[0\]: unknown host 'nowhere'",
             f"launch_history: [{setup.replace('local', 'nowhere')}]"),
            (r"harness must be one of", f"launch_history: [{setup.replace('claude', 'cursor-agent')}]"),
            (r"effort must be one of", f'launch_history: [{setup[:-2]}, effort: "huge" }}]'),
            (r"permissions must be one of", f'launch_history: [{setup[:-2]}, permissions: "custom" }}]'),
            (r"workspace_trust must be true or false", f"launch_history: [{setup[:-2]}, workspace_trust: 1 }}]"),
            (r"root must be an absolute path", f"checkout_trash: [{trash.replace('/w', '~/w')}]"),
        ]
        for message, field in cases:
            path = self.dir / "staged.json5"
            path.write_text(scenario_text(LOCAL_HOST, OPEN_SESSION)[:-1] + f", {field} }}")
            with self.subTest(message=message), self.assertRaisesRegex(gui_agent.UsageError, message) as raised:
                gui_agent.load_scenario(path, "someone")
            self.assertTrue(str(raised.exception).startswith(f"{path}: "), raised.exception)

    def test_the_scenarios_home_folders_are_made_in_the_private_home(self):
        """The launcher checks a folder on its host and warns under the field when it does not exist, and New starts in
        the open session's staged folder, which exists on no machine; so staging makes every `~` folder the scenario
        names inside the run's private home, which every staged supervisor expands `~` against. Specified for the docs
        scenario: the sessions', recent setups', recent folders' and templates' `~` folders, each once in the file's
        order, made as directories under the home and nowhere else; and a `~` folder with a `..` part, or another
        user's `~name`, is refused naming it."""
        plan = gui_agent.load_scenario(REPO / "docs" / "docs-shots" / "scenario.json5", "someone")
        self.assertEqual(gui_agent.scenario_home_dirs(plan),
                         ["src/farhelm", "src/api", "work/tok", "work/notes"])
        home = self.dir / "home"
        home.mkdir()
        made = gui_agent.create_scenario_dirs(home, plan)
        self.assertEqual(made, [home / "src/farhelm", home / "src/api", home / "work/tok", home / "work/notes"])
        self.assertTrue(all(directory.is_dir() for directory in made))
        self.assertEqual(sorted(path.name for path in self.dir.iterdir()), ["home", "t.txt"], "nothing outside home")
        for cwd in ("~/../escape", "~/a/./b", "~other/a"):
            with self.subTest(cwd=cwd), self.assertRaisesRegex(gui_agent.UsageError, "must be ~ or under ~/"):
                gui_agent.load_scenario(self.write(sessions=OPEN_SESSION.replace('"~/a"', f'"{cwd}"')), "someone")

    def test_a_scenario_run_is_staged(self):
        """A scenario's sessions are launched as `claude` and `codex` by bare name, which only a staged run's wrappers
        keep from the vendors' real programs, so a scenario run must be staged even when every host is local.
        Specified: `staged_start` is true for `--scenario` alone and false for a plain start; and through `start`
        itself, a scenario run with `--user-session` (refused for staged runs) and one that also names
        `--remote-host` are both refused before the run is created or any destination dialed."""
        self.assertTrue(gui_agent.staged_start(argparse.Namespace(remote_host=[], scenario=pathlib.Path("s.json5"))))
        self.assertFalse(gui_agent.staged_start(argparse.Namespace(remote_host=[], scenario=None)))
        local_only = self.write()
        unknown_host = self.write(sessions=OPEN_SESSION.replace('"local"', '"x"'), name="bad")
        dialed = []
        real_check = gui_agent.check_ssh
        gui_agent.check_ssh = dialed.append
        self.addCleanup(setattr, gui_agent, "check_ssh", real_check)
        # A Mac refuses --user-session before it asks whether the run is staged.
        user_session = "does not combine with a staged run" if sys.platform.startswith("linux") else "Linux only"
        refusals = [
            (["--user-session", "--scenario", str(local_only)], user_session),
            (["--scenario", str(HERO), "--remote-host", "localhost"], "does not take --remote-host"),
            (["--scenario", str(unknown_host)], "unknown host"),
        ]
        for args, message in refusals:
            with self.subTest(args=args), self.assertRaisesRegex(gui_agent.UsageError, message):
                gui_agent.start(self.dir / "run", args)
        self.assertEqual(dialed, [])
        self.assertFalse((self.dir / "run").exists())

    def test_seen_state_is_set_for_idle_sessions_only(self):
        """An idle row's dot is blue while its output is unseen and grey once seen, and the scenario says which. Only
        idle sessions are touched, as in the old staging. Specified: a seen idle session is stamped with its row's own
        activity (its creation time when it has none), an unseen one cleared, and any other status left alone."""
        plan = gui_agent.load_scenario(HERO, "someone")
        by_title = {session["title"]: session for session in plan["sessions"]}
        row = {"last_activity_at": 1700000100, "created_at": 1700000000}
        self.assertEqual(gui_agent.seen_body(by_title["triage crash reports"], row),
                         {"seen_activity_at": 1700000100})
        self.assertEqual(gui_agent.seen_body(by_title["triage crash reports"], dict(row, last_activity_at=0)),
                         {"seen_activity_at": 1700000000})
        self.assertEqual(gui_agent.seen_body(by_title["readme rewrite"], row), {"seen_activity_at": None})
        self.assertIsNone(gui_agent.seen_body(by_title["bench tokenizer on H100"], row))

    def test_the_shield_check_tells_the_shields_apart(self):
        """The old staging's last check: each row shows exactly one permission shield, slashed for a YOLO session and
        plain for every other. Specified against agent-track names in the window's words for an agent launch: the
        permission heading each shows is read as its shield (YOLO or Guarded), a YOLO observation on the status line
        reads as slashed whatever the launch chose, a session whose node is missing or doubled is not drawn yet, and a
        wrong shield is reported naming the session."""
        plan = gui_agent.plan_scenario(
            {"viewport": {"width": 1, "height": 1}, "hosts": [{"key": "l", "alias": "l", "kind": "local"}],
             "sessions": [{"title": t, "host": "l", "cwd": "~/a", "wrapper": "claude", "status": "running", "age": 0,
                           "yolo": yolo, "open": t == "y"} for t, yolo in (("y", True), ("n", False))]},
            self.dir, None, "synthetic")
        yolo = "YOLO: the agent bypasses its approval and sandbox safety checks\n\nLaunched with: x"
        guarded = "Guarded · Claude auto mode\nClaude's classifier approves or blocks actions\n\nLaunched with: x"

        def entry(key: str, name: str) -> dict:
            return {"name": name, "key": {"path": f"a/session-item-{key}/{gui_agent.agent_track_id(key)}"}}

        ids = {0: "e:y", 1: "e:n"}
        right = [entry("e:y", f"Claude — {yolo}"), entry("e:n", f"Claude — {guarded}")]
        self.assertEqual(gui_agent.shield_problems(plan, ids, right), ([], []))
        observed = [right[0], entry("e:n", f"YOLO observed on the agent's status line: x.\n\nLaunched as: Claude — "
                                           f"{guarded}")]
        self.assertEqual([w[:3] for w in gui_agent.shield_problems(plan, ids, observed)[1]], ["n: "])
        swapped = [entry("e:y", f"Claude — {guarded}"), right[1]]
        self.assertEqual([w[:3] for w in gui_agent.shield_problems(plan, ids, swapped)[1]], ["y: "])
        self.assertEqual(gui_agent.shield_problems(plan, ids, [right[0]])[0], ["n (0 shields)"])
        self.assertEqual(gui_agent.shield_problems(plan, ids, [*right, right[1]])[0], ["n (2 shields)"])

    def stand_in_helm(self, plan: dict):
        """A stand-in for the run's helm for `stage_scenario`, and what it saw: every request in order (with a
        ("SELECTED", "", None) marker where it first reports the window's selection), the create bodies, and counts of
        its host and preference reads. A remote host reads as connecting on the first host read and connected after;
        the window's selection of the first created session shows on the third preference read; every created session
        is listed at the status its scenario session names."""
        sent: list[tuple] = []
        created: list[dict] = []
        state = {"host_reads": 0, "preference_reads": 0}

        def helm(_run, method, path, body=None):
            sent.append((method, path, body))
            if path == "/api/hosts":
                state["host_reads"] += 1
                phase = "connected" if state["host_reads"] > 1 else "connecting"
                return {"hosts": [{"id": 1, "kind": "local", "state": {"phase": "connected"}},
                                  {"id": 2, "kind": "ssh", "destination": "localhost", "state": {"phase": phase}},
                                  {"id": 3, "kind": "ssh", "destination": "someone@localhost",
                                   "state": {"phase": "connected"}}]}
            if method == "POST" and path == "/api/sessions":
                created.append(body)
                return {"id": f"e:{len(created)}"}
            if path == "/api/preferences" and method == "GET":
                state["preference_reads"] += 1
                if state["preference_reads"] < 3:
                    return {"last_selected": None}
                sent.append(("SELECTED", "", None))
                return {"last_selected": "e:1"}
            if path == "/api/sessions":
                by_title = {s["title"]: s for s in plan["sessions"]}
                return {"sessions": [{"id": f"e:{n}", "status": {"state": by_title[b["title"]]["status"]},
                                      "last_activity_at": 1700000000 + n} for n, b in enumerate(created, start=1)]}
            return None

        return helm, sent, created, state

    @staticmethod
    def window_of(created: list[dict], yolo_of=None) -> list[dict]:
        """A window snapshot's entries with one agent track per created session, named as the session list names an
        agent launch's (`yolo_of(key, body)` says which permission it shows; the create body's own by default)."""
        yolo_of = yolo_of or (lambda _key, body: body["launch"]["permissions"] == "yolo")
        return [{"name": f"Claude — {'YOLO: bypasses checks' if yolo_of(key, body) else 'Guarded · auto mode'}",
                 "key": {"path": gui_agent.agent_track_id(key)}}
                for key, body in ((f"e:{n}", body) for n, body in enumerate(created, start=1))]

    def test_staging_waits_for_the_window_to_open_the_open_session_before_creating_the_rest(self):
        """The window keeps the first session it opens, so if another session were created before the window had
        opened the scenario's open one, the window could open that one instead (and mark it seen). Specified against a
        stand-in helm: every host is waited for until connected and then named by its alias; the list preferences are
        set; the open session is created first and no other create is sent until the helm reports the window's own
        selection of it; each session is created on its host in its own scratch directory holding its replay script;
        only idle sessions get a seen write; and the keys come back by scenario index."""
        plan = gui_agent.load_scenario(HERO, "someone")
        run = self.dir / "run"
        run.mkdir()
        helm, sent, created, state = self.stand_in_helm(plan)

        def window():
            return self.window_of(created)

        ids = gui_agent.stage_scenario(run, plan, "/fx", window, request=helm, timeout=10, interval=0)
        self.assertEqual(ids, {index: f"e:{index + 1}" for index in range(7)})
        creates = [index for index, (method, path, _) in enumerate(sent)
                   if (method, path) == ("POST", "/api/sessions")]
        self.assertEqual(state["preference_reads"], 3, "premise: the window's selection came only on the third read")
        selected = sent.index(("SELECTED", "", None))
        self.assertLess(creates[0], selected)
        self.assertLess(selected, creates[1], "no second create before the window's selection was reported")
        self.assertEqual(created[0]["title"], "fix flaky attach test")
        self.assertEqual([body["host"] for body in created], [2, 1, 3, 3, 1, 2, 2])
        aliases = [(path, body) for method, path, body in sent if path.endswith("/alias")]
        self.assertEqual(aliases, [("/api/hosts/1/alias", {"alias": "this machine"}),
                                   ("/api/hosts/2/alias", {"alias": "build-box"}),
                                   ("/api/hosts/3/alias", {"alias": "gpu-1"})])
        self.assertGreater(state["host_reads"], 1, "premise: the first host read found a host still connecting")
        self.assertLess(max(i for i, (_, p, _) in enumerate(sent) if p == "/api/hosts"),
                        min(i for i, (_, p, _) in enumerate(sent) if p.endswith("/alias")))
        self.assertIn(("PUT", "/api/preferences", gui_agent.SCENARIO_PREFERENCES), sent)
        for body in created:
            cwd = pathlib.Path(body["cwd"])
            self.assertEqual(cwd.parent, run / "work")
            self.assertIn("fake-agent --script replay", (cwd / gui_agent.STAGED_AGENT_SCRIPT).read_text())
        seen = {path for method, path, _ in sent if method == "PUT" and path.endswith("/seen")}
        idle_keys = [ids[i] for i, s in enumerate(plan["sessions"]) if s["status"] == "idle"]
        self.assertEqual(seen, {f"/api/sessions/{urllib.parse.quote(key, safe='')}/seen" for key in idle_keys})


    def test_staging_fails_on_a_wrong_shield_or_status_naming_the_session(self):
        """A run whose rows show other shields or statuses than the scenario asks for must fail rather than be
        photographed, naming the session as the scenario does. Specified against the same stand-in helm: a window whose
        row for one session shows the opposite assertion fails staging naming that session's title, a window that
        never draws one row's shield fails it once the wait runs out, and so does a session the helm keeps listing at
        another status, each naming the session's title."""
        plan = gui_agent.load_scenario(HERO, "someone")
        run = self.dir / "run"
        run.mkdir()
        helm, _, created, _ = self.stand_in_helm(plan)
        flipped = lambda key, body: (body["launch"]["permissions"] == "yolo") != (key == "e:2")
        with self.assertRaisesRegex(RuntimeError, "wrong permission shield: migrate auth tokens: .* does not say"):
            gui_agent.stage_scenario(run, plan, "/fx", lambda: self.window_of(created, flipped), request=helm,
                                     timeout=10, interval=0)

        run = self.dir / "run2"
        run.mkdir()
        helm, _, created, _ = self.stand_in_helm(plan)
        with self.assertRaisesRegex(RuntimeError, r"no single shield yet for readme rewrite \(0 shields\)"):
            gui_agent.stage_scenario(run, plan, "/fx", lambda: [e for e in self.window_of(created)
                                                               if not e["key"]["path"].endswith("e:3-agent")],
                                     request=helm, timeout=0.2, interval=0)

        unsettled = copy.deepcopy(plan)
        unsettled["sessions"][2]["status"] = "running"
        run = self.dir / "run3"
        run.mkdir()
        helm, _, created, _ = self.stand_in_helm(unsettled)
        with self.assertRaisesRegex(RuntimeError, "reach its status: readme rewrite=running$"):
            gui_agent.stage_scenario(run, plan, "/fx", lambda: self.window_of(created), request=helm, timeout=0.2,
                                     interval=0)


def rect(x: float, y: float, w: float, h: float) -> dict:
    """A rectangle as the bridge reports one."""
    return {"x": x, "y": y, "w": w, "h": h}


def png_header(width: int, height: int) -> bytes:
    """The start of a PNG of the given size: its signature and IHDR chunk, all `png_size` reads."""
    import struct
    import zlib

    body = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + struct.pack(">I", len(body)) + b"IHDR" + body
            + struct.pack(">I", zlib.crc32(b"IHDR" + body)))


class CaptureTest(unittest.TestCase):
    """`capture` writes a scenario's picture only when nothing can spoil it (a pointer's tooltip or highlight, a
    terminal still replaying), nothing in it names the machine, and it has the scenario's size."""

    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir)

    def test_the_readiness_line_is_the_transcripts_last_printed_line(self):
        """The open session is settled when its terminal shows its transcript's last printed line. Specified: the hero's
        `Running…` line, trimmed and without escapes; comments and blanks skipped; a silent transcript refused."""
        self.assertEqual(gui_agent.last_transcript_line(HERO.parent / "transcript.txt"), "⎿  Running…")
        path = self.dir / "t.txt"
        path.write_text("## comment\n\\e[1mfirst\\e[0m\n\n## trailing comment\n")
        self.assertEqual(gui_agent.last_transcript_line(path), "first")
        path.write_text("## only a comment\n\n")
        with self.assertRaisesRegex(gui_agent.UsageError, "prints nothing"):
            gui_agent.last_transcript_line(path)

    def test_the_pointer_parks_only_where_nothing_reacts_to_it(self):
        """A pointer over a row or a link draws a highlight or a tooltip into the picture. Specified: a target qualifies
        only with its centre outside every drawn hover or tooltip element (an undrawn one counts for nothing), never
        without a drawn area itself, and only as the first entry of its id path, which `id:<path>` resolves to."""
        tree = {"nodes": [
            {"id": "session-list", "visible": rect(0, 0, 100, 100), "flags": {"hover": True}},
            {"label": "Help", "visible": rect(200, 0, 50, 50), "flags": {"tooltip": True}},
            {"visible": rect(300, 0, 0, 0), "flags": {"hover": True}},
            {"visible": rect(400, 0, 50, 50), "flags": {"mouse": True}},
        ]}
        entries = [
            {"key": {"path": "rows/row"}, "visible": rect(10, 10, 20, 20)},
            {"key": {"path": "rail/help"}, "visible": rect(210, 10, 20, 20)},
            {"key": {"path": "brand"}, "visible": rect(300, 0, 40, 40)},
            {"key": {"path": "brand"}, "visible": rect(0, 0, 20, 20)},
            {"key": {"path": "hidden"}, "visible": rect(500, 0, 0, 0)},
            {"key": {"path": "plain"}, "visible": rect(400, 0, 50, 50)},
            {"key": {"path": ""}, "visible": rect(600, 0, 50, 50)},
        ]
        self.assertEqual(gui_agent.park_targets(entries, tree), ["id:brand", "id:plain"])
        rejected = []
        self.assertEqual(gui_agent.park_targets(entries, tree, rejected=rejected), ["id:brand", "id:plain"])
        self.assertEqual(rejected, [
            ("id:rows/row", "its centre is inside the reacting element session-list"),
            ("id:rail/help", "its centre is inside the reacting element Help"),
            ("id:hidden", "the element has no visible area"),
        ])

    def needles(self) -> tuple[list[str], list[str]]:
        """The needles for a run in this test's directory, reached through a symlink so its two spellings differ."""
        real = self.dir / "real"
        (real / "run").mkdir(parents=True)
        (self.dir / "link").symlink_to(real)
        self.run_dir = self.dir / "link" / "run"
        record = {"binaries": {"farhelm": "/build/target/debug/farhelm"}}
        return gui_agent.capture_needles(self.run_dir, record, "someone", "/home/someone",
                                         ["localhost", "someone@localhost"], "box.example")

    def test_the_needles_are_the_machines_paths_anywhere_and_its_names_where_drawn(self):
        """Specified: refused anywhere are the run directory as given and as resolved, the checkout, the build
        directory and the account's home; refused where drawn are each staged destination naming the account, the
        account followed by `@`, and the hostname with and without its domain, never `localhost` itself."""
        anywhere, drawn = self.needles()
        self.assertEqual(anywhere, [str(self.run_dir), str(self.dir.resolve() / "real" / "run"), str(REPO),
                                    *([str(REPO.resolve())] if REPO.resolve() != REPO else []), "/build/target",
                                    "/home/someone"])
        self.assertEqual(drawn, ["someone@localhost", "someone@", "box.example", "box"])

    def test_a_leak_is_refused_where_the_picture_could_show_it(self):
        """Specified: a machine path is refused in drawn and hidden text, labels, field values and the terminal; the
        account's ssh spelling, `user@` and the hostname (as a whole word) where drawn and in the terminal, but not in
        labels or text out of view; source locations are not looked at."""
        anywhere, drawn = self.needles()
        run = str(self.run_dir)
        clean = {"title": "", "texts": [{"text": "gpu-1 in the sandbox", "visible": rect(0, 0, 9, 9)},
                                        {"text": "someone@localhost", "visible": rect(0, 0, 0, 0)}],
                 "nodes": [{"label": "gpu-1 (someone@localhost): Connected", "visible": rect(0, 0, 9, 9),
                            "source": "/home/someone/.cargo/registry/x.rs:1"}]}
        self.assertEqual(gui_agent.leaks(clean, "> fix the test", anywhere, drawn), [])
        cases = [
            ("drawn text", {"texts": [{"text": f"cd {run}/work/0", "visible": rect(0, 0, 9, 9)}]}, ""),
            ("drawn text", {"texts": [{"text": "gpu-1 (someone@localhost)", "visible": rect(0, 0, 9, 9)}]}, ""),
            ("drawn text", {"texts": [{"text": "box.example", "visible": rect(0, 0, 9, 9)}]}, ""),
            ("drawn text", {"texts": [{"text": "ssh someone@elsewhere", "visible": rect(0, 0, 9, 9)}]}, ""),
            ("drawn text", {"texts": [{"text": "on box", "visible": rect(0, 0, 9, 9)}]}, ""),
            ("hidden text", {"texts": [{"text": "/home/someone/src", "visible": rect(0, 0, 0, 0)}]}, ""),
            ("accessible label", {"nodes": [{"label": f"Folder {run}/work/2", "visible": rect(0, 0, 0, 0)}]}, ""),
            ("field value", {"nodes": [{"value": "someone@localhost", "visible": rect(0, 0, 9, 9)}]}, ""),
            ("terminal", {}, f"$ ls {run}"),
            ("terminal", {}, "someone@localhost:~$"),
        ]
        for where, tree, terminal in cases:
            with self.subTest(where=where, tree=tree, terminal=terminal):
                found = gui_agent.leaks({"title": "", "texts": [], "nodes": [], **tree}, terminal, anywhere, drawn)
                self.assertEqual(len(found), 1, found)
                self.assertTrue(found[0].startswith(f"{where}: "), found)

    def test_a_picture_of_the_wrong_size_is_refused_quoting_the_window(self):
        """The publish script checks the size again, but a wrong-size picture must not get that far looking like the
        hero. Specified for the hero (1280x800 at scale 2): 2560x1600 passes; anything else, and an unreadable shot, is
        refused with the size found, the size declared, and the window's points and scale, which distinguish a small
        display from an output scale that does not match the scenario."""
        plan = gui_agent.load_scenario(HERO, "someone")
        self.assertEqual(gui_agent.expected_capture_size(plan), (2560, 1600))
        mac = {"width": 1280.0, "height": 800.0, "scale": 2.0}
        self.assertIsNone(gui_agent.size_problem((2560, 1600), plan, mac))
        shrunk = gui_agent.size_problem((2048, 1360), plan, dict(mac, width=1024.0, height=680.0))
        for part in ("2048x1360 pixels", "declares 2560x1600", "window is 1024x680 points at scale 2.0",
                     "display is too small"):
            self.assertIn(part, shrunk)
        linux = gui_agent.size_problem((1280, 800), plan, {"width": 1322.0, "height": 876.0, "scale": 1.0})
        self.assertIn("at scale 1.0", linux)
        self.assertIn("the scenario's --scale", linux)
        self.assertIn("no readable PNG", gui_agent.size_problem(None, plan, mac))

    def stand_in_bridge(self, shot_size: tuple[int, int], window: dict, drawn: str = "gpu-1"):
        """A stand-in for the bridge's command line for `capture`, and the verbs it was asked for: the terminal holds no
        focus until it is clicked and then shows the hero transcript's end; the frame has one hover region with a
        target inside it and one plain target; `drawn` is the one drawn text; a screenshot writes a PNG header of
        `shot_size`."""
        asked: list[list[str]] = []
        state = {"focused": False}
        tree = {"title": "", "texts": [{"text": drawn, "visible": rect(0, 0, 9, 9)}],
                "nodes": [{"visible": rect(0, 0, 100, 100), "flags": {"hover": True}}]}
        entries = [{"key": {"path": "rows"}, "visible": rect(10, 10, 9, 9)},
                   {"key": {"path": "brand"}, "visible": rect(200, 0, 40, 40)}]

        def bridge(_run, _record, args, capture=False):
            asked.append(args)
            out = ""
            if args[:2] == ["get", "probe"]:
                out = json.dumps("...\n  ⎿  Running…\n") if state["focused"] else "null"
            elif args[0] == "click":
                state["focused"] = True
            elif args[0] == "snapshot":
                out = json.dumps({"entries": entries})
            elif args[0] == "tree":
                out = json.dumps(tree)
            elif args[0] == "windows":
                out = json.dumps([window])
            elif args[0] == "screenshot":
                pathlib.Path(args[1]).write_bytes(png_header(*shot_size))
            return subprocess.CompletedProcess(args, 0, stdout=out, stderr="")

        return bridge, asked

    def capture_with(self, bridge, *options: str) -> tuple[pathlib.Path, pathlib.Path]:
        """Run `capture` on a hero scenario run in this test's directory against `bridge`, as on a Mac (no frame to
        crop); returns the run directory and the output path. Flags share the same fixture as the default path."""
        run = self.dir / "run"
        run.mkdir()
        record = {"scenario": {"file": str(HERO)}, "binaries": {"farhelm": "/build/target/debug/farhelm"}}
        output = self.dir / "out" / "hero.png"
        import contextlib
        import io

        with contextlib.redirect_stdout(io.StringIO()):
            gui_agent.capture(run, record, [str(output), *options], platform="darwin", bridge_cli=bridge)
        return run, output

    def test_capture_settles_parks_checks_and_writes_only_a_right_sized_picture(self):
        """Specified against a stand-in bridge: the pointer is parked on the target outside the hover region before
        anything else (on a Mac the real pointer may rest on a terminal link whose tooltip covers the terminal); the
        terminal is then clicked for focus because the probe said none held it, and only then waited on; the pointer is
        parked again after the click and before the last idle wait; the leak check reads the frame before and after
        the shot; and a picture of the scenario's size is written to the output, byte for byte the shot."""
        bridge, asked = self.stand_in_bridge((2560, 1600), {"width": 1280.0, "height": 800.0, "scale": 2.0})
        run, output = self.capture_with(bridge)
        verbs = [args[0] if args[0] != "get" else "probe" for args in asked]
        click = verbs.index("click")
        self.assertEqual(asked[click], ["click", "id:terminal-grid"])
        hovers = [index for index, verb in enumerate(verbs) if verb == "hover"]
        self.assertEqual([asked[index] for index in hovers], [["hover", "id:brand"]] * 2)
        self.assertLess(hovers[0], click, "parked before the click, which a tooltip could otherwise cover")
        self.assertLess(click, verbs.index("probe", click), "the terminal is waited on after the click")
        self.assertLess(click, hovers[1], "parked again after the click")
        shot = verbs.index("screenshot")
        # The settling reads the tree after the second park; the bridge's own idle wait is not used (`settle`).
        self.assertIn("tree", verbs[hovers[1]:shot])
        self.assertIn("tree", verbs[shot:])
        self.assertNotIn("wait", verbs)
        self.assertEqual(output.read_bytes(), (run / gui_agent.CAPTURE_FILE).read_bytes())

    def test_capture_writes_nothing_when_a_check_fails(self):
        """A wrong picture must never land where the publish step looks. Specified: a shrunk shot fails and leaves the
        output unwritten, the shot kept in the run directory; a drawn account spelling fails before any shot."""
        bridge, _ = self.stand_in_bridge((2048, 1360), {"width": 1024.0, "height": 680.0, "scale": 2.0})
        with self.assertRaisesRegex(RuntimeError, "display is too small") as raised:
            self.capture_with(bridge)
        self.assertIn("Nothing was written", str(raised.exception))
        self.assertFalse((self.dir / "out").exists())
        self.assertTrue((self.dir / "run" / gui_agent.CAPTURE_FILE).exists())

        shutil.rmtree(self.dir / "run")
        bridge, asked = self.stand_in_bridge((2560, 1600), {"width": 1280.0, "height": 800.0, "scale": 2.0},
                                             drawn=f"gpu-1 ({gui_agent.pwd.getpwuid(os.getuid()).pw_name}@localhost)")
        with self.assertRaisesRegex(RuntimeError, "machine detail before the shot"):
            self.capture_with(bridge)
        self.assertNotIn("screenshot", [args[0] for args in asked])
        self.assertFalse((self.dir / "out").exists())

    def test_keep_focus_skips_terminal_input_and_readiness_wait(self):
        """A launcher shot must keep its field focused even while the terminal probe stays null. Specified: no click
        and no transcript poll, but both pointer parks, idle waits and leak probes still surround a successful shot."""
        bridge, asked = self.stand_in_bridge((2560, 1600), {"width": 1280.0, "height": 800.0, "scale": 2.0})
        run = self.dir / "run"
        run.mkdir()
        output = self.dir / "dialog.png"
        record = {"scenario": {"file": str(HERO)}, "binaries": {"farhelm": "/build/target/debug/farhelm"}}
        self.assertEqual(gui_agent.capture(run, record, [str(output), "--keep-focus"], platform="darwin",
                                          bridge_cli=bridge), 0)
        self.assertFalse(any(args[0] == "click" for args in asked))
        # Only the two leak checks may read the terminal; a readiness poll would add reads and time out on null.
        self.assertEqual(asked.count(["get", "probe", "terminal_text"]), 2)
        self.assertEqual(asked.count(["hover", "id:brand"]), 2)
        self.assertFalse(any(args[0] == "wait" for args in asked), "settled from the tree, not the bridge's wait")
        self.assertEqual(output.read_bytes(), (run / gui_agent.CAPTURE_FILE).read_bytes())

    def test_settling_ignores_the_terminal_and_nothing_else(self):
        """A running session's agent redraws a clock in its terminal every second, so a capture that also waited for
        the terminal to stop changing timed out whenever reading the window took longer than a second. Specified: text
        painted inside the terminal's element changes nothing the capture compares, while any other element's text
        does; settling returns once the rest has held still long enough and fails after the timeout otherwise."""
        def tree(clock: str, title: str) -> dict:
            nodes = [{"id": "root", "parent": None}, {"id": gui_agent.TERMINAL_ELEMENT, "parent": 0},
                     {"id": None, "parent": 1}, {"id": "title", "parent": 0}]
            return {"nodes": nodes, "texts": [{"text": clock, "node": 2}, {"text": title, "node": 3}]}

        self.assertEqual(gui_agent.settled_hash(tree("(2m 16s)", "A")), gui_agent.settled_hash(tree("(2m 17s)", "A")))
        self.assertNotEqual(gui_agent.settled_hash(tree("(2m 16s)", "A")), gui_agent.settled_hash(tree("(2m 16s)", "B")))

        now = [0.0]
        ticks = iter(range(1000))
        gui_agent.settle(lambda: tree(f"({next(ticks)}s)", "A"), timeout=5, clock=lambda: now[0],
                         pause=lambda seconds: now.__setitem__(0, now[0] + 0.1))
        self.assertLess(now[0], 1)
        now[0] = 0.0
        with self.assertRaisesRegex(RuntimeError, "did not stop changing"):
            gui_agent.settle(lambda: tree("x", f"{next(ticks)}"), timeout=2, clock=lambda: now[0],
                             pause=lambda seconds: now.__setitem__(0, now[0] + 0.1))

    def test_capture_refuses_a_run_without_a_scenario(self):
        """Only a scenario says what the picture's size and open session are."""
        with self.assertRaisesRegex(gui_agent.UsageError, "start this run with --scenario"):
            gui_agent.capture(self.dir, {}, [str(self.dir / "x.png")])

    def test_explicit_park_hovers_the_named_target_without_searching(self):
        """A modal shot knows its safe title even when automatic parking finds no spot. Both parking passes must
        hover that id directly, retain their idle waits and never search; a refused hover must name the target."""
        bridge, asked = self.stand_in_bridge((2560, 1600), {"width": 1280.0, "height": 800.0, "scale": 2.0})
        original = gui_agent.park_targets
        gui_agent.park_targets = lambda *_: self.fail("explicit parking must not search")
        self.addCleanup(setattr, gui_agent, "park_targets", original)
        run, _ = self.capture_with(bridge, "--keep-focus", "--park", "id:title")
        self.assertEqual([args for args in asked if args[0] == "hover"], [["hover", "id:title"]] * 2)
        self.assertFalse(any(args[0] == "snapshot" for args in asked))
        # Each park is followed by the settling's tree reads (`settle`), before the next park and before the shot.
        verbs = [args[0] for args in asked]
        hovers = [index for index, verb in enumerate(verbs) if verb == "hover"]
        self.assertIn("tree", verbs[hovers[0]:hovers[1]])
        self.assertIn("tree", verbs[hovers[1]:verbs.index("screenshot")])
        shutil.rmtree(run)

        def refused(run, record, args, capture=False):
            """Exercise a covered explicit spot without falling back to automatic discovery."""
            if args[0] == "hover":
                return subprocess.CompletedProcess(args, 1, stdout="", stderr="covered")
            return bridge(run, record, args, capture=capture)

        with self.assertRaisesRegex(RuntimeError, "hover id:title.*covered"):
            self.capture_with(refused, "--keep-focus", "--park", "id:title")

    def test_parking_failure_names_filtered_and_refused_candidates_with_a_bound(self):
        """A failed capture should identify both reacting elements and refusals from the actual hover command.

        A frame with many filtered candidates still shows the bridge's reason first. It reports the number of
        hidden entries and gives up before focusing the terminal or taking a screenshot.
        """
        bridge, asked = self.stand_in_bridge((2560, 1600), {"width": 1280.0, "height": 800.0, "scale": 2.0})
        long_path = "layer/" * 60 + "brand"

        def crowded(run, record, args, capture=False):
            response = bridge(run, record, args, capture=capture)
            if args[0] == "snapshot":
                entries = json.loads(response.stdout)["entries"]
                entries[1]["key"]["path"] = long_path
                entries.extend({"key": {"path": f"filtered-{i}"}, "visible": rect(10, 10, 9, 9)}
                               for i in range(13))
                response.stdout = json.dumps({"entries": entries})
            elif args[0] == "tree":
                tree = json.loads(response.stdout)
                tree["nodes"][0]["id"] = "foreground-row"
                response.stdout = json.dumps(tree)
            elif args[0] == "hover":
                self.assertEqual(args, ["hover", f"id:{long_path}"])
                response.returncode = 1
                response.stderr = f"{long_path} (button {'label ' * 90}) is covered by a foreground dialog"
            return response

        with self.assertRaises(RuntimeError) as failure:
            self.capture_with(crowded, "--keep-focus")
        message = str(failure.exception)
        self.assertIn("/brand: the bridge refused hover:", message)
        self.assertIn("is covered by a foreground dialog", message)
        self.assertIn("id:rows: its centre is inside the reacting element foreground-row", message)
        self.assertIn("... and 3 more rejected candidates", message)
        self.assertLess(len(message), 1600, "the bridge supplied a long reason")
        self.assertEqual([args for args in asked if args[0] == "hover"], [["hover", f"id:{long_path}"]])
        self.assertFalse(any(args[0] == "screenshot" for args in asked))


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
            gui_agent.start_mutter(self.dir, self.dir, "wayland-never", command, dict(os.environ), socket_wait=3, scale=2)
        leaf = int(wait_for_file(leaf_file))
        self.owned.append(leaf)
        self.assertIsNone(gui_agent.proc_start(leaf), "the compositor's child must be stopped")
        settings = self.dir / "mutter-config/glib-2.0/settings/keyfile"
        self.assertEqual(settings.read_text(), "[org/gnome/desktop/interface]\nscaling-factor=2\n")

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
