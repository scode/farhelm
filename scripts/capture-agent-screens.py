#!/usr/bin/env python3
"""Re-capture real Claude Code and Codex screens as the supervisor's screen-reader fixtures.

The supervisor decides whether a Claude or Codex session is working, waiting on the user, or idle by recognizing what
the agent draws, so its rules go stale whenever a vendor changes its TUI. This tool is the single thing to run after
upgrading either agent on a development host: it launches the INSTALLED `claude` and `codex` in a private tmux server
sized like a production pane, drives each through the states the readers must tell apart, and writes every screen it
saw under `crates/farhelm-supervisor/tests/fixtures/screens/<harness>/<version>/`, then runs the fixture tests over
them. New fixtures that the tests reject are exactly the screens whose rules need a shipped update.

Each fixture is the raw `capture-pane -p` visible grid (the command the supervisor's sampler runs) in
`<expected>-<scenario>.txt`, with the pane title beside it in `<expected>-<scenario>.title`. The expected state is the
one this tool drove the agent into, never what a reader concluded, which is what makes the fixtures a test oracle.

What a run costs and touches:

- A few short turns per agent, on the vendor logins already configured on this host.
- Claude runs in its default permission mode and Codex in a read-only sandbox asking on request, so real approval
  prompts appear. Both answer their folder-trust dialogs for a fresh scratch directory, which the vendors record in
  their own configuration (the same accepted side effect as the `#[ignore]`d real-agent tests).
- Nothing of Farhelm's: the tmux server is private, on a random socket directly under `/tmp`, and uses the pinned
  `.ci-tmux/tmux`, never a live supervisor's.

Every scenario is best effort under its own deadline. A state the agent cannot be driven into on this host (Codex's
question tool reports itself unavailable in its default mode) is reported as not captured instead of aborting the run.

Screens are scrubbed of the home directory, user name, host name, the Git author name, and email addresses before they
are written, because the fixtures are committed to a public repository; a screen that still contains any of them after
scrubbing is refused.
"""

from __future__ import annotations

import argparse
import getpass
import os
import pathlib
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable
from dataclasses import dataclass, field

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
# Fixed rather than configurable: the fixture tests read exactly this tree, and a capture written anywhere else would
# never be validated.
FIXTURES_ROOT = REPO_ROOT / "crates/farhelm-supervisor/tests/fixtures/screens"
DEFAULT_TMUX = REPO_ROOT / ".ci-tmux/tmux"

# The production supervisor's pane geometry is chosen by its clients; this is the size observed on the development
# fleet. One size is enough because the readers anchor on short prefixes that survive a narrower pane's truncation.
PANE_COLUMNS = 133
PANE_ROWS = 42

# Mirrors the supervisor's generated tmux config (`TmuxDriver::config_body` plus the `focus-events` it sets on start),
# so vendors render exactly as they do under Farhelm. `focus-events on` matters beyond fidelity: Claude prints a
# one-line nag on its idle screen when it reads the option off.
TMUX_CONFIG = """\
set -s exit-empty off
set -s escape-time 0
set -s default-terminal 'xterm-256color'
set -as terminal-features ',xterm-256color:RGB'
set -s focus-events on
set -g status off
set -g prefix None
set -g history-limit 12000
setw -g remain-on-exit on
"""

POLL_SECONDS = 0.5
EXPECTED_STATES = ("working", "waiting", "idle", "unknown")

# Claude's spinner line: a rotating glyph, a present-participle verb ending in an ellipsis, then the elapsed timer.
CLAUDE_SPINNER = re.compile(r"^\S \S+… \(\d+s\b", re.MULTILINE)


class ScenarioTimeout(Exception):
    """A scenario's agent never reached the screen it was waiting for before its deadline."""


# --------------------------------------------------------------------------------------------------------------------
# Scrubbing
# --------------------------------------------------------------------------------------------------------------------


@dataclass
class Scrubber:
    """Replaces identifying strings in captured screens with neutral placeholders.

    Replacements keep the original length where the value sits inside layout (paths and the scratch directory), so a
    scrubbed status line still wraps and truncates where the real one did. Anything listed in `forbidden` must be gone
    after scrubbing; `check` is the backstop that turns a missed pattern into a refused fixture rather than a leak.
    """

    literal: list[tuple[str, str]] = field(default_factory=list)
    words: list[tuple[str, str]] = field(default_factory=list)
    forbidden: list[str] = field(default_factory=list)

    EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
    # The fixture tests exempt exactly this address too, so a scrubbed screen passes both checks.
    EMAIL_PLACEHOLDER = "user@example.com"

    @classmethod
    def for_host(cls, work_root: pathlib.Path) -> Scrubber:
        """Build the scrubber for this host's identity and this run's scratch directory."""

        scrubber = cls()
        work = str(work_root)
        # Same-length placeholder: the random mkdtemp suffix is the only part that changes between runs.
        scrubber.literal.append((work, "/tmp/fhc-" + "x" * (len(work) - len("/tmp/fhc-"))))
        home = str(pathlib.Path.home())
        scrubber.literal.append((home, "~"))
        scrubber.forbidden.append(home)
        for value, placeholder in ((getpass.getuser(), "user"), (socket.gethostname().split(".")[0], "host")):
            if value:
                scrubber.words.append((value, placeholder))
                scrubber.forbidden.append(value)
        git_name = subprocess.run(
            ["git", "config", "user.name"], capture_output=True, text=True, check=False
        ).stdout.strip()
        for part in git_name.split():
            if len(part) >= 3:
                scrubber.words.append((part, "Name"))
                scrubber.forbidden.append(part)
        return scrubber

    def scrub(self, text: str) -> str:
        """Return `text` with every known identifying value replaced."""

        for value, placeholder in self.literal:
            text = text.replace(value, placeholder)
        text = self.EMAIL.sub(self.EMAIL_PLACEHOLDER, text)
        for value, placeholder in self.words:
            text = re.sub(rf"(?i)\b{re.escape(value)}\b", placeholder, text)
        return text

    def check(self, text: str) -> str | None:
        """Name the first identifying value still present in `text`, or None when it is clean."""

        lowered = text.lower()
        for value in self.forbidden:
            if value.lower() in lowered:
                return value
        if self.EMAIL.search(text.replace(self.EMAIL_PLACEHOLDER, "")):
            return "an email address"
        return None


# --------------------------------------------------------------------------------------------------------------------
# The private tmux pane
# --------------------------------------------------------------------------------------------------------------------


class Pane:
    """One agent running in its own private tmux server.

    The server lives on a socket in a fresh random directory directly under `/tmp`: unix socket paths are limited to
    about 108 bytes, and concurrent runs on a shared machine must never meet on a fixed name.
    """

    def __init__(self, tmux: pathlib.Path, work_root: pathlib.Path, env: dict[str, str]) -> None:
        self.socket_dir = pathlib.Path(tempfile.mkdtemp(prefix="fhcap-", dir="/tmp"))
        config = work_root / "tmux.conf"
        config.write_text(TMUX_CONFIG)
        self.base = [str(tmux), "-S", str(self.socket_dir / "s"), "-f", str(config)]
        self.env = env
        self.target = "agent"

    def tmux(self, *args: str, check: bool = True) -> str:
        """Run one tmux command against the private server and return its stdout."""

        done = subprocess.run(
            [*self.base, *args], env=self.env, capture_output=True, text=True, timeout=30, check=False
        )
        if check and done.returncode != 0:
            raise RuntimeError(f"tmux {' '.join(args)} failed: {done.stderr.strip()}")
        return done.stdout

    def start(self, cwd: pathlib.Path, command: str) -> None:
        """Start the agent in a pane of the production size."""

        self.tmux(
            "new-session", "-d", "-s", self.target, "-x", str(PANE_COLUMNS), "-y", str(PANE_ROWS),
            "-c", str(cwd), command,
        )

    def screen(self) -> str:
        """The visible grid exactly as the supervisor's sampler captures it (`capture-pane -p`, no scrollback)."""

        return self.tmux("capture-pane", "-p", "-t", self.target)

    def title(self) -> str:
        """The pane title the agent last set through OSC 0/2."""

        return self.tmux("display-message", "-p", "-t", self.target, "#{pane_title}").rstrip("\n")

    def dead(self) -> bool:
        return self.tmux("display-message", "-p", "-t", self.target, "#{pane_dead}").strip() == "1"

    def keys(self, *keys: str) -> None:
        """Send named keys (`Enter`, `Escape`, `Down`, `C-u`)."""

        self.tmux("send-keys", "-t", self.target, *keys)

    def text(self, text: str) -> None:
        """Type literal text without submitting it."""

        self.tmux("send-keys", "-t", self.target, "-l", text)

    def wait_for(self, what: str, predicate: Callable[[str, str], bool], timeout: float) -> tuple[str, str]:
        """Poll until `predicate(screen, title)` holds; return that screen and title.

        Raises `ScenarioTimeout` with the last screen attached when the deadline passes, and fails fast when the agent
        process has died, since no later screen can satisfy anything.
        """

        deadline = time.monotonic() + timeout
        screen = title = ""
        while True:
            screen, title = self.screen(), self.title()
            if predicate(screen, title):
                return screen, title
            if self.dead():
                raise ScenarioTimeout(f"the agent exited while waiting for {what}:\n{screen}")
            if time.monotonic() >= deadline:
                raise ScenarioTimeout(f"timed out waiting for {what}; last screen:\n{screen}")
            time.sleep(POLL_SECONDS)

    def submit(self, prompt: str, typed: Callable[[str], bool], timeout: float = 30) -> None:
        """Type `prompt`, confirm it is visible, then press Enter until it leaves the input line.

        Learned from the real-agent tests: text and Enter in one burst read as a paste to Codex, which inserts the
        newline instead of submitting, and a single Enter after a fixed delay was observed to be lost on a loaded
        machine. Submission is a state the screen shows, so this asks the screen.
        """

        self.text(prompt)
        self.wait_for("the typed prompt", lambda s, _t: typed(s), timeout)
        deadline = time.monotonic() + timeout
        while typed(self.screen()) and time.monotonic() < deadline:
            self.keys("Enter")
            time.sleep(1.5)

    def close(self) -> None:
        self.tmux("kill-server", check=False)
        shutil.rmtree(self.socket_dir, ignore_errors=True)


# --------------------------------------------------------------------------------------------------------------------
# Recording
# --------------------------------------------------------------------------------------------------------------------


@dataclass
class Recorder:
    """Collects one harness's scrubbed screens in a staging directory and reports what was and was not captured."""

    staging: pathlib.Path
    scrubber: Scrubber
    # Stop after the scenarios that need no model turn (trust, fresh prompt, menus the user opens).
    ui_only: bool = False
    captured: list[str] = field(default_factory=list)
    missed: list[str] = field(default_factory=list)

    def save(self, expected: str, scenario: str, screen: str, title: str) -> None:
        """Write one fixture pair, refusing it if scrubbing left anything identifying behind."""

        assert expected in EXPECTED_STATES, expected
        name = f"{expected}-{scenario}"
        body, head = self.scrubber.scrub(screen), self.scrubber.scrub(title)
        leak = self.scrubber.check(body + "\n" + head)
        if leak is not None:
            self.missed.append(f"{name} (refused: still contains {leak} after scrubbing)")
            return
        (self.staging / f"{name}.txt").write_text(body)
        (self.staging / f"{name}.title").write_text(head + "\n")
        self.captured.append(name)

    def attempt(self, label: str, body: Callable[[], None]) -> bool:
        """Run one scenario step; a timeout marks it not captured instead of ending the run."""

        try:
            body()
            return True
        except ScenarioTimeout as e:
            # Scrubbed like a fixture: the last screen is printed so the operator can see what the agent showed
            # instead, and logs get pasted into issues and PRs.
            detail = self.scrubber.scrub(str(e))
            self.missed.append(f"{label} (not captured: {detail.splitlines()[0]})")
            print(f"  {label}: not captured\n{detail}", file=sys.stderr)
            return False


# --------------------------------------------------------------------------------------------------------------------
# Claude Code
# --------------------------------------------------------------------------------------------------------------------


def claude_input(screen: str) -> str | None:
    """The text of Claude's input line: the last `❯` line directly below a horizontal rule.

    Claude echoes submitted prompts into the transcript with the same `❯` prefix, so only the line inside the ruled
    input box says what is still waiting to be submitted.
    """

    lines = [line for line in screen.splitlines() if line.strip()]
    found = None
    for above, line in zip(lines, lines[1:]):
        if line.startswith("❯") and above.startswith("──"):
            found = line[1:].strip(" \u00a0")
    return found


def claude_prompt_box(screen: str) -> bool:
    """Whether Claude's input box is drawn with no dialog footer on screen."""

    return claude_input(screen) is not None and "Esc to cancel" not in screen


def claude_spinner(screen: str) -> bool:
    return CLAUDE_SPINNER.search(screen) is not None


def drive_claude(pane: Pane, rec: Recorder, work: pathlib.Path) -> None:
    """Drive Claude Code through trust, stateless menus, idle, working, both kinds of waiting, and its `/clear` hint."""

    # The idle hint normally needs 75 idle minutes and 100k tokens of context; both thresholds are Claude's own
    # environment variables, so the tool lowers them rather than waiting.
    pane.start(
        work,
        "env CLAUDE_CODE_IDLE_THRESHOLD_MINUTES=1 CLAUDE_CODE_IDLE_TOKEN_THRESHOLD=0 "
        "claude --permission-mode default --model sonnet",
    )

    def answer_trust(screen: str) -> None:
        # The menu can default to "No, exit"; move only when the selection proves Enter would refuse.
        if "❯ No, exit" in screen:
            pane.keys("Down")
            time.sleep(0.5)
        pane.keys("Enter")

    if not rec.attempt(
        "fresh prompt",
        lambda: reach_prompt(pane, rec, lambda s: "Yes, I trust this folder" in s, answer_trust, claude_prompt_box),
    ):
        return

    # Screens that carry no state (the reader must answer "can't tell"), reachable without a model turn.
    def model_picker() -> None:
        pane.submit("/model", lambda s: (claude_input(s) or "").startswith("/model"))
        screen, title = pane.wait_for("the model picker", lambda s, _t: "Enter to set as default" in s, 30)
        rec.save("unknown", "model-picker", screen, title)
        pane.keys("Escape")
        pane.wait_for("the prompt box", lambda s, _t: claude_prompt_box(s), 30)

    rec.attempt("model picker", model_picker)

    def transcript() -> None:
        pane.keys("C-o")
        screen, title = pane.wait_for(
            "the transcript view", lambda s, _t: "Showing detailed transcript" in s, 30
        )
        rec.save("unknown", "transcript", screen, title)
        pane.keys("C-o")
        pane.wait_for("the prompt box", lambda s, _t: claude_prompt_box(s), 30)

    rec.attempt("transcript view", transcript)
    if rec.ui_only:
        return

    def thinking_then_permission() -> None:
        pane.submit(
            "Use your Bash tool to run exactly this command: sleep 20 && echo capture-done > marker.txt",
            lambda s: "run exactly this command" in (claude_input(s) or ""),
        )
        screen, title = pane.wait_for(
            "the spinner or the permission prompt",
            lambda s, _t: claude_spinner(s) or "Do you want to proceed?" in s,
            120,
        )
        if claude_spinner(screen):
            rec.save("working", "thinking", screen, title)
        screen, title = pane.wait_for(
            "the permission prompt", lambda s, _t: "Do you want to proceed?" in s and "Esc to cancel" in s, 120
        )
        rec.save("waiting", "permission", screen, title)
        pane.keys("Enter")

    if not rec.attempt("permission prompt", thinking_then_permission):
        return

    def tool_running() -> None:
        for sample in (1, 2):
            screen, title = pane.wait_for("the tool-run spinner", lambda s, _t: claude_spinner(s), 60)
            rec.save("working", f"tool-{sample}", screen, title)
            time.sleep(4)

    rec.attempt("tool run", tool_running)

    def after_turn() -> None:
        screen, title = pane.wait_for(
            "the finished turn",
            lambda s, _t: re.search(r" for \d+[ms]", s) is not None and claude_prompt_box(s) and not claude_spinner(s),
            120,
        )
        rec.save("idle", "after-turn", screen, title)

    if not rec.attempt("idle after a turn", after_turn):
        return

    # Independent of the question below: a vendor dropping its idle hint must not cost the waiting capture.
    def clear_hint() -> None:
        screen, title = pane.wait_for("the /clear hint", lambda s, _t: "/clear to save" in s, 180)
        rec.save("idle", "clear-hint", screen, title)

    rec.attempt("idle /clear hint", clear_hint)

    def question() -> None:
        pane.wait_for("the prompt box", lambda s, _t: claude_prompt_box(s), 60)
        pane.keys("C-u")
        pane.submit(
            "Use your AskUserQuestion tool to ask me one question: tea or coffee? Two options. Do nothing else.",
            lambda s: "AskUserQuestion tool" in (claude_input(s) or ""),
        )
        screen, title = pane.wait_for(
            "the question form", lambda s, _t: "Enter to select" in s and "Esc to cancel" in s, 120
        )
        rec.save("waiting", "question", screen, title)
        pane.keys("Escape")

    rec.attempt("question form", question)


def reach_prompt(
    pane: Pane,
    rec: Recorder,
    is_trust: Callable[[str], bool],
    answer_trust: Callable[[str], None],
    ready: Callable[[str], bool],
    timeout: float = 120,
    settle: float = 3,
) -> None:
    """Get from launch to a settled input prompt, answering the folder-trust dialog whenever it appears.

    One loop rather than "wait for the dialog, then wait for the prompt": an agent can paint a frame that looks like
    its prompt before the dialog arrives, and a dialog that swallows the answering keystroke must be answered again.
    The prompt counts as ready only once it has stayed ready for `settle` seconds, so the saved `idle-fresh` screen is
    the settled first paint rather than a half-drawn one. The dialog is saved once, as `waiting-trust`.
    """

    deadline = time.monotonic() + timeout
    trust_saved = False
    ready_since: float | None = None
    while True:
        screen, title = pane.screen(), pane.title()
        # Checked before any branch can `continue`: a dialog that never accepts its answer (Codex's once ignored tmux
        # input entirely) must still end in a timeout, and a dead pane keeps showing its last screen.
        if pane.dead():
            raise ScenarioTimeout(f"the agent exited before its prompt was ready:\n{screen}")
        if time.monotonic() >= deadline:
            raise ScenarioTimeout(f"timed out waiting for a settled prompt; last screen:\n{screen}")
        if is_trust(screen):
            if not trust_saved:
                rec.save("waiting", "trust", screen, title)
                trust_saved = True
            answer_trust(screen)
            ready_since = None
            time.sleep(2)
            continue
        if ready(screen):
            ready_since = ready_since or time.monotonic()
            if time.monotonic() - ready_since >= settle:
                rec.save("idle", "fresh", screen, title)
                return
        else:
            ready_since = None
        time.sleep(POLL_SECONDS)


# --------------------------------------------------------------------------------------------------------------------
# Codex
# --------------------------------------------------------------------------------------------------------------------

BRAILLE_SPINNER = re.compile(r"^[⠀-⣿]")


def codex_input(screen: str) -> str | None:
    """The text of Codex's composer: the last `›` line, since the transcript echoes prompts with the same prefix."""

    found = None
    for line in screen.splitlines():
        if line.startswith("›"):
            found = line[1:].strip()
    return found


def codex_composer(screen: str) -> bool:
    """Whether Codex's composer is drawn with no dialog on screen.

    Codex's menus draw their selected option with the same `›` marker as the composer (`› 1. Trust and continue`), so
    a numbered option on the last `›` line, or any dialog footer, means a dialog owns the keyboard: text typed then
    lands in the menu (which can navigate away to its Agent Command Center).
    """

    text = codex_input(screen)
    if text is None or re.match(r"\d+\.", text):
        return False
    lowered = screen.lower()
    return not any(footer in lowered for footer in ("esc to cancel", "enter continue", "enter to confirm"))


def drive_codex(pane: Pane, rec: Recorder, work: pathlib.Path) -> None:
    """Drive Codex through trust, its model menu, idle, streaming, approval, a tool run, a recap, and a question form."""

    pane.start(work, "codex -s read-only -a on-request")

    if not rec.attempt(
        "fresh composer",
        lambda: reach_prompt(
            pane, rec, lambda s: "Trust this folder?" in s, lambda _s: pane.keys("Enter"), codex_composer
        ),
    ):
        return

    # A screen that carries no state (the reader must answer "can't tell"), reachable without a model turn.
    def model_picker() -> None:
        pane.submit("/model", lambda s: (codex_input(s) or "").startswith("/model"))
        screen, title = pane.wait_for("the model picker", lambda s, _t: "Select Model and Effort" in s, 30)
        rec.save("unknown", "model-picker", screen, title)
        pane.keys("Escape")
        pane.wait_for("the composer", lambda s, _t: codex_composer(s), 30)

    rec.attempt("model picker", model_picker)
    if rec.ui_only:
        return

    def streaming_then_approval() -> None:
        prompt = "Run exactly this shell command: sleep 20 && echo capture-done > marker.txt"
        pane.submit(prompt, lambda s: prompt in (codex_input(s) or ""))
        # While Codex streams prose there is no on-screen working widget; only the title's Braille spinner says so.
        screen, title = pane.wait_for(
            "streaming or the approval prompt",
            lambda s, t: (BRAILLE_SPINNER.match(t) is not None and "Working (" not in s) or "esc to cancel" in s,
            120,
        )
        if "esc to cancel" not in screen:
            rec.save("working", "streaming", screen, title)
        screen, title = pane.wait_for(
            "the approval prompt", lambda s, _t: "Press enter to confirm or esc to cancel" in s, 180
        )
        rec.save("waiting", "permission", screen, title)
        pane.keys("Enter")

    if not rec.attempt("approval prompt", streaming_then_approval):
        return

    def tool_running() -> None:
        for sample in (1, 2):
            screen, title = pane.wait_for("the Working widget", lambda s, _t: "Working (" in s, 60)
            rec.save("working", f"tool-{sample}", screen, title)
            time.sleep(4)

    rec.attempt("tool run", tool_running)

    def after_turn() -> None:
        pane.wait_for(
            "the finished turn",
            lambda s, t: "Worked for" in s and "Working (" not in s and not BRAILLE_SPINNER.match(t),
            180,
        )
        time.sleep(2)
        rec.save("idle", "after-turn", pane.screen(), pane.title())

    if not rec.attempt("idle after a turn", after_turn):
        return

    # Independent of the question below: a vendor dropping `/recap` must not cost the waiting capture.
    def recap() -> None:
        pane.submit("/recap", lambda s: (codex_input(s) or "").startswith("/recap"))
        screen, title = pane.wait_for("the recap", lambda s, _t: "Recap:" in s and codex_composer(s), 180)
        rec.save("idle", "recap", screen, title)

    rec.attempt("idle recap", recap)

    def question() -> None:
        pane.wait_for("the composer", lambda s, _t: codex_composer(s), 60)
        pane.keys("C-u")
        pane.submit(
            "Use your request_user_input tool to ask me one question: tea or coffee? Offer exactly two options. "
            "Do nothing else.",
            lambda s: "request_user_input tool" in (codex_input(s) or ""),
        )
        # Codex 0.159 reports this tool unavailable in its default mode; the scenario stays for versions that allow it.
        screen, title = pane.wait_for(
            "the pending question", lambda s, t: "1 question" in s or "Action Required" in t, 120
        )
        rec.save("waiting", "question", screen, title)

    rec.attempt("question form", question)


# --------------------------------------------------------------------------------------------------------------------
# Running a harness and publishing its fixtures
# --------------------------------------------------------------------------------------------------------------------

HARNESSES: dict[str, tuple[list[str], Callable[[Pane, Recorder, pathlib.Path], None]]] = {
    "claude": (["claude", "--version"], drive_claude),
    "codex": (["codex", "--version"], drive_codex),
}


def harness_version(argv: list[str]) -> str:
    """The installed agent's version number, as the fixture directory name."""

    out = subprocess.run(argv, capture_output=True, text=True, check=True, timeout=60).stdout
    match = re.search(r"\d+\.\d+\.\d+\S*", out)
    if match is None:
        raise RuntimeError(f"cannot read a version from {' '.join(argv)!r}: {out!r}")
    return match.group(0)


def agent_env() -> dict[str, str]:
    """This process's environment minus anything that would tie the agent to an outer tmux or a Farhelm session."""

    return {
        key: value
        for key, value in os.environ.items()
        if key not in ("TMUX", "TMUX_PANE") and not key.startswith("FARHELM_")
    }


def capture_harness(
    name: str, tmux: pathlib.Path, work_root: pathlib.Path, scrubber: Scrubber, ui_only: bool
) -> Recorder:
    """Capture one harness and write what it captured into its version directory.

    Only the scenarios this run captured are replaced. An existing fixture whose scenario timed out or was refused this
    time is kept and reported, because a transient miss must not delete the evidence a previous run collected; removing
    a scenario for good is a deliberate edit to the fixture tree, never a side effect of re-capturing. Hand-derived
    fixtures (`*-derived-*`) are never touched either: they encode screens this tool cannot reach on the host, such as a
    rate-limit footer that only appears with a different login.
    """

    version_argv, drive = HARNESSES[name]
    version = harness_version(version_argv)
    work = work_root / name
    work.mkdir()
    staging = work_root / f"{name}-fixtures"
    staging.mkdir()
    rec = Recorder(staging, scrubber, ui_only=ui_only)
    pane = Pane(tmux, work_root, agent_env())
    print(f"{name} {version}: capturing", file=sys.stderr)
    try:
        drive(pane, rec, work)
    finally:
        pane.close()
    if not rec.captured:
        print(f"{name} {version}: nothing captured; leaving existing fixtures alone", file=sys.stderr)
        return rec
    target = FIXTURES_ROOT / name / version
    target.mkdir(parents=True, exist_ok=True)
    for new in staging.iterdir():
        shutil.copy2(new, target / new.name)
    retained = sorted(
        old.stem for old in target.glob("*.txt") if old.stem not in rec.captured and "-derived-" not in old.stem
    )
    print(f"{name} {version}: wrote {len(rec.captured)} fixtures to {target}", file=sys.stderr)
    for stem in retained:
        rec.missed.append(f"{stem} (kept from an earlier capture; not re-captured this run)")
    return rec


def run_fixture_tests() -> int:
    """Run the supervisor's fixture tests through the repository's test recorder with the pinned nextest."""

    nextest_dir = subprocess.run(
        [sys.executable, str(REPO_ROOT / "scripts/install-pinned-nextest.py")],
        capture_output=True, text=True, check=True, cwd=REPO_ROOT,
    ).stdout.strip()
    env = dict(os.environ, PATH=f"{nextest_dir}:{os.environ.get('PATH', '')}")
    command = [
        sys.executable, "scripts/record-test-run.py", "--runner", "nextest", "--kind", "development",
        "--selection", "screen reader fixtures", "--concurrency", "4 nextest slots; retries 0", "--tmux", "none",
        "--", "cargo", "nextest", "run", "-p", "farhelm-supervisor", "-E", "test(screen_fixtures)",
    ]
    print("running: " + " ".join(command), file=sys.stderr)
    return subprocess.run(command, cwd=REPO_ROOT, env=env, check=False).returncode


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--harness", action="append", choices=sorted(HARNESSES), help="capture only this agent (repeatable)"
    )
    parser.add_argument("--tmux", type=pathlib.Path, default=DEFAULT_TMUX, help="tmux binary (default: pinned)")
    parser.add_argument("--keep-work", action="store_true", help="keep the scratch directory for inspection")
    parser.add_argument("--no-test", action="store_true", help="skip running the fixture tests afterwards")
    parser.add_argument(
        "--ui-only",
        action="store_true",
        help="capture only screens that need no model turn (trust, fresh prompt, menus); keeps the other fixtures",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if not args.tmux.is_file():
        print(f"{args.tmux} is missing; build it with scripts/build-pinned-tmux-ci.sh", file=sys.stderr)
        return 2
    # Short on purpose: the scratch path appears in agent banners and status lines, and a long one wraps them.
    work_root = pathlib.Path(tempfile.mkdtemp(prefix="fhc-", dir="/tmp"))
    scrubber = Scrubber.for_host(work_root)
    results: dict[str, Recorder] = {}
    try:
        for name in args.harness or sorted(HARNESSES):
            results[name] = capture_harness(name, args.tmux, work_root, scrubber, args.ui_only)
    finally:
        if args.keep_work:
            print(f"scratch kept at {work_root}", file=sys.stderr)
        else:
            shutil.rmtree(work_root, ignore_errors=True)

    for name, rec in results.items():
        print(f"{name}: captured {', '.join(rec.captured) or 'nothing'}")
        for miss in rec.missed:
            print(f"{name}: {miss}")
    status = 0 if all(rec.captured for rec in results.values()) else 1
    if not args.no_test:
        status = run_fixture_tests() or status
    return status


if __name__ == "__main__":
    sys.exit(main())
