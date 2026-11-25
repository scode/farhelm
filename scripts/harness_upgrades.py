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

What this does NOT do: sign in, answer prompts, or set up hooks. Every updater runs with stdin closed, so one that
would ask a question fails instead of hanging, and a missing login still shows up as COULD NOT RUN in the drift case.
"""

from __future__ import annotations

from pathlib import Path
import shutil
import subprocess
import tempfile
from typing import NamedTuple
import urllib.error
import urllib.parse
import urllib.request

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


def run_updater(argv: list[str], env: dict[str, str]) -> tuple[bool, str]:
    """Run one update command with stdin closed, returning whether it exited 0 and why not."""
    try:
        done = subprocess.run(
            argv,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
            timeout=UPGRADE_TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return False, f"{argv[0]} did not finish within {UPGRADE_TIMEOUT} seconds"
    except OSError as error:
        return False, str(error)
    return done.returncode == 0, last_line(done.stdout) if done.returncode else ""


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
    """Upgrade one harness and report its versions. Never raises for a vendor-side failure."""
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
