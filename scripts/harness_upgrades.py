"""Bring each installed agent CLI to its vendor's latest release before a drift run.

The drift suite exists to catch vendor changes that break Farhelm's launch commands, conversation reports or Restart.
A pass against a CLI that is weeks behind says nothing about what users get when they install today, so the drift
runner upgrades every selected harness first (`run-harness-drift.py --no-upgrade` skips it, for reproducing a failure
on the version that produced it, and `--upgrade-only` upgrades without running the suite).

Each harness is upgraded with the vendor's own updater, never by reinstalling through a package manager the agent
guesses at, because the updater knows how its CLI was installed (npm prefix, bun, a self-managed versions directory).
Two updaters were broken on 2026-11-22, so those harnesses fall back to the vendor's documented `curl | bash`
installer, and only after the updater fails:

- `cursor-agent update` (2026.10.14) failed with `[unauthenticated] Error` while `cursor-agent status` reported a
  working login. Cursor's installer upgraded it.
- `goose update` (1.60.2) panicked in Sigstore verification ("no sigstore-crypto CryptoProvider installed") after
  downloading the archive. Goose's installer does not do that verification, so the fallback trusts GitHub's HTTPS alone.

Every updater and installer runs in a transient systemd user scope of its own (`run_updater`), so nothing it starts in
the background can swap a binary after the upgrade step is over.

What this does NOT do: sign in, answer prompts, or set up hooks. Every updater runs with stdin closed, so one that
would ask a question fails instead of hanging, and a missing login still shows up as COULD NOT RUN in the drift case.
"""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from typing import NamedTuple
import urllib.error
import urllib.parse
import urllib.request
import uuid

# Updaters download tens of megabytes (goose's archive is about 57 MB) and npm-based ones resolve a dependency tree.
UPGRADE_TIMEOUT = 600
VERSION_TIMEOUT = 60


class Recipe(NamedTuple):
    """How one harness reports its version and upgrades itself.

    `binary` must match the binary the harness's drift profile launches (`real_agent_capture/drift/mod.rs`); the runner's
    tests check that every drift case has a recipe. `quiet_env` is added to every invocation, version probes included,
    and exists for launchers that would otherwise update in the background during a probe. `upgrade_env` is added only
    to the upgrade command. `installer_env` is the environment the fallback installer needs to run unattended.
    """

    binary: str
    upgrade: tuple[str, ...]
    quiet_env: dict[str, str] = {}
    upgrade_env: dict[str, str] = {}
    installer_url: str | None = None
    installer_env: dict[str, str] = {}


# Keyed by drift case name, which is not always the binary name (antigravity is `agy`, cursor is `cursor-agent`).
RECIPES: dict[str, Recipe] = {
    "claude": Recipe("claude", ("claude", "update")),
    "codex": Recipe("codex", ("codex", "update")),
    "antigravity": Recipe("agy", ("agy", "update")),
    # CONFIGURE=false stops the installer from running `goose configure`, which reads its answers from /dev/tty.
    "goose": Recipe(
        "goose",
        ("goose", "update"),
        installer_url="https://github.com/aaif-goose/goose/releases/download/stable/download_cli.sh",
        installer_env={"CONFIGURE": "false"},
    ),
    "grok": Recipe("grok", ("grok", "update")),
    # The Muse launcher has no update subcommand. It checks for a new release at most hourly and then updates in the
    # background, which would race the drift case; MUSE_SYNC_UPDATE=1 makes it update the launcher and the binary in
    # the foreground before it runs `--version`. MUSE_NO_AUTO_UPDATE=1 keeps the plain version probes from starting
    # that background update.
    "muse": Recipe(
        "muse",
        ("muse", "--version"),
        quiet_env={"MUSE_NO_AUTO_UPDATE": "1"},
        upgrade_env={"MUSE_NO_AUTO_UPDATE": "0", "MUSE_SYNC_UPDATE": "1"},
    ),
    # `--self` updates pi itself, which is also what a bare `pi update` does; installed extensions are left alone.
    "pi": Recipe("pi", ("pi", "update", "--self")),
    "omp": Recipe("omp", ("omp", "update")),
    "cursor": Recipe("cursor-agent", ("cursor-agent", "update"), installer_url="https://cursor.com/install"),
    "opencode": Recipe("opencode", ("opencode", "upgrade")),
}


class Upgrade(NamedTuple):
    """What a harness looked like before and after the upgrade step.

    `status` is `upgraded`, `current` (the updater succeeded and the version did not change), `not_installed` (the
    drift case will report COULD NOT RUN on its own), `skipped` (versions probed, upgrade not requested) or `failed`.
    Only `failed` is a problem the runner reports as one: the suite then tests whatever version is still installed.
    `detail` is the updater's reason for failing, which a successful fallback installer keeps as a note.
    """

    status: str
    before: str
    after: str
    detail: str = ""


def version(recipe: Recipe, env: dict[str, str]) -> str:
    """The first line of `--version`, or a placeholder; a broken probe must not stop the upgrade or the drift case."""
    try:
        probe = subprocess.run(
            [recipe.binary, "--version"],
            env={**env, **recipe.quiet_env},
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
            timeout=VERSION_TIMEOUT,
        )
    except (OSError, subprocess.TimeoutExpired):
        return "unknown"
    lines = [line.strip() for line in probe.stdout.splitlines() if line.strip()]
    return lines[0] if probe.returncode == 0 and lines else "unknown"


def last_line(output: str) -> str:
    """The updaters print their reason for failing last; the rest is progress noise (OpenCode draws a spinner)."""
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    return lines[-1] if lines else "no output"


# How often the runner looks at an updater's scope while it waits for the scope to empty or to be stopped.
SCOPE_POLL = 0.1
# How long `systemctl --user stop` gives the scope's processes between SIGTERM and SIGKILL. An updater cut off at its
# deadline gets no grace worth waiting minutes for, so this is far below systemd's default 90 seconds.
SCOPE_STOP_TIMEOUT = 10


class ContainmentLost(Exception):
    """An updater's scope could not be confirmed stopped: something it started may still be able to swap a binary.

    Distinct from a failed upgrade, which the run reports and goes on from: a fallback installer, a version probe or a
    drift case started now could run beside a surviving worker, so the runner stops instead.
    """


class Cancelled(Exception):
    """SIGINT or SIGTERM reached the runner while an updater ran; carries the signal so the runner can exit 128 + it."""

    def __init__(self, signum: int):
        super().__init__(f"cancelled by signal {signum}")
        self.signum = signum


def scope_running(unit: str) -> bool:
    """Whether the transient scope still has processes: it goes inactive, then unloads (`--collect`), once it is empty.

    A scope that cannot be read is treated as running, so a broken probe never lets the runner go on beside an updater.
    """
    try:
        state = subprocess.run(
            ["systemctl", "--user", "show", "--property=ActiveState", "--value", unit],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=VERSION_TIMEOUT,
        )
    except (OSError, subprocess.TimeoutExpired):
        return True
    if state.returncode != 0:
        return True
    return state.stdout.strip() in ("active", "activating", "deactivating", "reloading", "refreshing")


def stop_scope(unit: str, child: subprocess.Popen | None, name: str) -> str:
    """Stop the scope and everything in it, and confirm it is gone; returns why not, or "" once it is.

    The top-level process (`name`, run through `systemd-run`) is killed and reaped as well: a cancellation can land
    before `systemd-run` has registered the scope, when stopping the unit reaches nothing and only the child itself can
    still create it. The unit is stopped again after that kill for exactly that case.

    "Gone" is systemd's word for it: the unit no longer reads active. A process that survives the scope's SIGKILL (one
    stuck in an uninterruptible wait) would leave the unit failed and unloaded while still in its cgroup, and this does
    not read the cgroup to tell. Such a process can at most finish the one system call it is blocked in, then dies; if
    that call is the write or rename that swaps a binary, the swap lands after the runner moved on. That is accepted
    as a limit.
    """
    def stop() -> str:
        try:
            done = subprocess.run(
                ["systemctl", "--user", "stop", unit],
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=SCOPE_STOP_TIMEOUT + VERSION_TIMEOUT,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            return f"could not stop {unit}: {error}"
        # A unit that is already gone (or never registered) is what this wants: systemctl exits 5, "not loaded".
        if done.returncode not in (0, 5):
            return f"could not stop {unit}: {last_line(done.stderr)}"
        return ""

    problem = stop()
    if child is not None and child.poll() is None:
        child.kill()
    if child is not None:
        try:
            child.wait(timeout=SCOPE_STOP_TIMEOUT)
        except subprocess.TimeoutExpired:
            problem = problem or f"{name} did not exit after SIGKILL"
    problem = stop() or problem
    deadline = time.monotonic() + SCOPE_STOP_TIMEOUT
    while scope_running(unit):
        if time.monotonic() >= deadline:
            return problem or f"{unit} still had processes after it was stopped"
        time.sleep(SCOPE_POLL)
    return problem


def user_manager_problem() -> str:
    """Why no transient user scope can be made here, or "" when the user's systemd manager answers."""
    try:
        done = subprocess.run(
            ["systemctl", "--user", "show-environment"],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=VERSION_TIMEOUT,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return str(error)
    return "" if done.returncode == 0 else last_line(done.stderr)


def manager_env() -> dict[str, str]:
    """The variables `systemd-run --user` needs to reach the user's manager, from the runner's own environment.

    With `--scope`, systemd-run execs the command with the environment it was given, so these must be in its copy.
    They override any the caller's `env` sets, so that `systemd-run` and the runner's own `systemctl --user` checks
    always talk to the same manager. They name only the user's own runtime directory and bus.
    """
    return {name: os.environ[name] for name in ("XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS") if name in os.environ}


def run_updater(argv: list[str], env: dict[str, str], timeout: float = UPGRADE_TIMEOUT) -> tuple[bool, str]:
    """Run one update command with stdin closed in a scope of its own, returning whether it exited 0 and why not.

    Updaters and installers start background workers (npm's, a self-updating launcher's, a detached `setsid` helper)
    that can still swap the CLI's binary after the command itself exits. Left alone, one of them could replace the
    binary while the runner probes its version or while a drift case runs, so a verdict would name the wrong release.
    So each command runs in a uniquely named transient `systemd-run --user --scope` unit: a cgroup, which a `setsid` or
    double fork cannot leave. After an ordinary exit the runner waits, within the same `timeout`, for the scope to
    empty, so a legitimate worker finishes its swap; if it does not, or the command itself overruns `timeout`, or
    SIGINT or SIGTERM arrives, the scope is stopped and confirmed gone before this returns. A cancellation then raises
    `Cancelled`, after the scope is gone, so the runner can exit the way it does for a cancelled case. A scope that
    cannot be confirmed stopped raises `ContainmentLost` instead of returning, since nothing may run after it.

    Drift tests are Linux-only (docs/harness-drift-tests.md); without a user systemd manager the command is not run at
    all and this reports why, rather than upgrading without containment.

    Output goes to a private file, not a pipe: a worker that kept a pipe open would hold the read until it exited.
    """
    problem = user_manager_problem()
    if problem:
        return False, f"drift upgrades need a systemd user manager to contain the updater ({problem})"
    unit = f"farhelm-drift-upgrade-{uuid.uuid4()}.scope"
    command = [
        "systemd-run", "--user", "--scope", "--quiet", "--collect", f"--unit={unit}",
        f"--property=TimeoutStopSec={SCOPE_STOP_TIMEOUT}", "--", *argv,
    ]
    deadline = time.monotonic() + timeout
    received: list[int] = []

    def note(signum, _frame):
        received.append(signum)

    previous = {signum: signal.signal(signum, note) for signum in (signal.SIGINT, signal.SIGTERM)}
    child = None
    try:
        with tempfile.TemporaryFile(mode="w+", errors="replace") as output:
            try:
                child = subprocess.Popen(
                    command,
                    env={**env, **manager_env()},
                    stdin=subprocess.DEVNULL,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                )
            except OSError as error:
                return False, str(error)
            outcome = ""
            # The command, then its scope: each phase ends early on a signal or the shared deadline.
            while not received and child.poll() is None and time.monotonic() < deadline:
                time.sleep(SCOPE_POLL)
            if child.poll() is None and not received:
                outcome = f"{argv[0]} did not finish within {timeout:g} seconds"
            while not received and not outcome and scope_running(unit):
                if time.monotonic() >= deadline:
                    outcome = f"{argv[0]} exited but its background processes did not finish within {timeout:g} seconds"
                    break
                time.sleep(SCOPE_POLL)
            if received or outcome:
                stopped = stop_scope(unit, child, argv[0])
                if received:
                    if stopped:
                        print(f"drift upgrade cancelled: {stopped}", file=sys.stderr)
                    raise Cancelled(received[0])
                if stopped:
                    raise ContainmentLost(f"{outcome}; {stopped}")
                result = False, outcome
            else:
                output.seek(0)
                returncode = child.returncode
                result = returncode == 0, last_line(output.read()) if returncode else ""
    finally:
        for signum, handler in previous.items():
            signal.signal(signum, handler)
    # A signal that came after the last look but before the handlers went back still cancels the run; the scope was
    # already empty, so there is nothing left to stop.
    if received:
        raise Cancelled(received[0])
    return result


def require_installer_https(url: str) -> None:
    """Installer bytes become shell code, so every URL used to fetch them requires TLS."""
    if urllib.parse.urlsplit(url).scheme != "https":
        raise urllib.error.URLError("installer URL must use HTTPS")


class InstallerRedirects(urllib.request.HTTPRedirectHandler):
    """Check every resolved target before urllib opens the next connection."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        require_installer_https(newurl)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def run_installer(url: str, env: dict[str, str]) -> tuple[bool, str]:
    """Run a vendor installer from a private file after verifying the download stayed on HTTPS."""
    with tempfile.TemporaryDirectory(prefix="fh-harness-installer-") as directory:
        script = Path(directory) / "install.sh"
        try:
            require_installer_https(url)
            opener = urllib.request.build_opener(InstallerRedirects())
            with opener.open(url, timeout=VERSION_TIMEOUT) as response:
                require_installer_https(response.geturl())
                script.write_bytes(response.read())
        except (OSError, ValueError) as error:
            return False, f"could not download {url}: {error}"
        return run_updater(["bash", str(script)], env)


def upgrade(recipe: Recipe, env: dict[str, str]) -> Upgrade:
    """Upgrade one harness and report its versions. Never raises for a vendor-side failure; raises `Cancelled` on SIGINT
    or SIGTERM, once the updater's scope is gone, and lets `ContainmentLost` through, running neither the fallback
    installer nor the second version probe."""
    if shutil.which(recipe.binary, path=env.get("PATH")) is None:
        return Upgrade("not_installed", "not installed", "not installed")
    before = version(recipe, env)
    ok, detail = run_updater(list(recipe.upgrade), {**env, **recipe.quiet_env, **recipe.upgrade_env})
    if not ok and recipe.installer_url is not None:
        ok, fallback = run_installer(recipe.installer_url, {**env, **recipe.installer_env})
        # Keep the updater's failure visible after a working fallback: the recipe's exception may have outlived the bug.
        detail = f"updater failed ({detail}), used the installer" if ok else f"{detail}; installer: {fallback}"
    after = version(recipe, env)
    if not ok:
        return Upgrade("failed", before, after, detail)
    return Upgrade("upgraded" if after != before else "current", before, after, detail)


def probe(recipe: Recipe, env: dict[str, str]) -> Upgrade:
    """Record the installed version without changing anything, for `--no-upgrade` runs."""
    if shutil.which(recipe.binary, path=env.get("PATH")) is None:
        return Upgrade("not_installed", "not installed", "not installed")
    installed = version(recipe, env)
    return Upgrade("skipped", installed, installed)


def describe(name: str, result: Upgrade) -> str:
    """One line per harness for the runner's output, failures spelled out so they cannot be mistaken for a pass."""
    if result.status == "failed":
        return f"UPGRADE FAILED {name:<13}  still {result.after}: {result.detail}"
    if result.status == "not_installed":
        return f"not installed  {name:<13}"
    note = f"  ({result.detail})" if result.detail else ""
    if result.status == "upgraded":
        return f"upgraded       {name:<13}  {result.before} -> {result.after}{note}"
    label = "already latest" if result.status == "current" else "not upgraded"
    return f"{label:<14} {name:<13}  {result.after}{note}"
