#!/usr/bin/env python3
"""Re-capture real Claude Code and Codex screens as the supervisor's screen-reader fixtures.

The supervisor decides whether a Claude or Codex session is working, waiting on the user, or idle by recognizing what
the agent draws, so its rules go stale whenever a vendor changes its TUI. This tool is the single thing to run after
upgrading either agent on a development host: it launches the INSTALLED `claude` and `codex` in a real Farhelm terminal
host sized like a production terminal, as a supervisor starts an agent (the supervisor's emulator answering the agent's
startup queries, the environment a supervisor gives an agent), drives each through the states the readers must tell
apart, and writes every screen it saw under `crates/farhelm-supervisor/tests/fixtures/screens/<harness>/<version>/`,
then runs the fixture tests over them. New fixtures that the tests reject are exactly the screens whose rules need a
shipped update.

Each fixture is the visible screen as text, as the emulator gives it to the supervisor's status sampler
(`Emulator::screen_text`), in `<expected>-<scenario>.txt`, with the terminal's title beside it in
`<expected>-<scenario>.title`. The expected state is the one this tool drove the agent into, never what a reader
concluded, which is what makes the fixtures a test oracle. Versions captured before 2026-11 were captured in a private
tmux pane with `capture-pane -p` instead; they are kept as captured (`docs/agent-screen-fixtures.md` says why).

What a run costs and touches:

- A few short turns per agent, on the vendor logins already configured on this host.
- Claude runs in its default permission mode and Codex in a read-only sandbox asking on request, so real approval
  prompts appear. Both answer their folder-trust dialogs for a fresh scratch directory, which the vendors record in
  their own configuration (the same accepted side effect as the `#[ignore]`d real-agent tests).
- Nothing of a live Farhelm's: the terminal host is started from the built `farhelm` into a random directory directly
  under `/tmp` (`scripts/agent_screen_host.py`), never a live supervisor's.

Every scenario is best effort under its own deadline. A state the agent cannot be driven into on this host (Codex's
question tool reports itself unavailable in its default mode) is reported as not captured instead of aborting the run.

Screens are scrubbed of the home directory, user name, host name, the Git author name, and email addresses before they
are written, because the fixtures are committed to a public repository; a screen that still contains any of them after
scrubbing is refused.
"""

from __future__ import annotations

import argparse
import fcntl
import getpass
import json
import os
import pathlib
import re
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from ipaddress import IPv4Address, IPv4Network, IPv6Address, IPv6Network

from agent_screen_host import HostError, HostRun, default_farhelm

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
# Fixed rather than configurable: the fixture tests read exactly this tree, and a capture written anywhere else would
# never be validated.
FIXTURES_ROOT = REPO_ROOT / "crates/farhelm-supervisor/tests/fixtures/screens"

# The production terminal's geometry is chosen by its clients; this is the size observed on the development fleet.
# One size is enough because the readers anchor on short prefixes that survive a narrower terminal's truncation.
TERMINAL_COLUMNS = 133
TERMINAL_ROWS = 42

POLL_SECONDS = 0.5
EXPECTED_STATES = ("working", "waiting", "idle", "unknown")
PUBLICATION_JOURNAL = ".capture-publish"

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
    # A URL that points into this machine or its private network: an agent shows one when its configuration sends it
    # through a local proxy (Codex warns that its base URL is overridden). It says how the capture host is set up, so
    # its scheme, host and port become a neutral placeholder. `host` is the scrubbed host name, so a URL to this
    # machine by name is caught after the word replacements too.
    # Keep the IPv4 ranges explicit. Shared CGNAT space can expose a tailnet address, while documentation ranges
    # belong in captured screens.
    LOCAL_IPV4 = tuple(
        IPv4Network(network)
        for network in (
            "0.0.0.0/32",
            "127.0.0.0/8",
            "10.0.0.0/8",
            "172.16.0.0/12",
            "192.168.0.0/16",
            "169.254.0.0/16",
            "100.64.0.0/10",
        )
    )
    # Keep the old scrubber's dotted spellings when IPv4Address rejects them, such as a zero-padded last octet.
    LEGACY_LOCAL_IPV4 = re.compile(
        r"(?:0\.0\.0\.0|127(?:\.\d{1,3}){3}|10(?:\.\d{1,3}){3}|192\.168(?:\.\d{1,3}){2}"
        r"|172\.(?:1[6-9]|2\d|3[01])(?:\.\d{1,3}){2})"
    )
    UNIQUE_LOCAL_IPV6 = IPv6Network("fc00::/7")
    URL = re.compile(
        r"\b[a-z][a-z0-9+.-]*://(?=([^\s/?#\"<>`]+)(/|[?#](?=[^\s\"'<>`)\]}]))?)",
        re.IGNORECASE,
    )
    URL_HOST_PORT = re.compile(
        r"(?:\[(?P<ipv6>[0-9a-f:.]+)(?:%[a-z0-9._~-]+)?\]|(?P<name>[a-z0-9._~-]+))(?::[0-9]*)?",
        re.IGNORECASE,
    )
    URL_PUNCTUATION = re.compile(r"[,;)'\]}]")
    LOCAL_URL_PLACEHOLDER = "http://proxy.invalid"
    # The fixture tests exempt exactly this address too, so a scrubbed screen passes both checks.
    EMAIL_PLACEHOLDER = "user@example.com"

    @classmethod
    def _local_ipv4(cls, address: IPv4Address) -> bool:
        """Apply the same fixture privacy policy to plain and IPv4-mapped addresses."""

        return any(address in network for network in cls.LOCAL_IPV4)

    @classmethod
    def _local_authority(cls, authority: str) -> bool | None:
        """Classify a complete host and port after the final userinfo separator, or None if they are incomplete."""

        endpoint = authority.rstrip(".").rsplit("@", 1)[-1]
        match = cls.URL_HOST_PORT.fullmatch(endpoint)
        if match is None:
            return None
        literal = match["ipv6"]
        if literal is None:
            name = match["name"].rstrip(".").lower()
            if name in ("localhost", "host"):
                return True
            try:
                address_v4 = IPv4Address(name)
            except ValueError:
                return cls.LEGACY_LOCAL_IPV4.fullmatch(name) is not None
            return cls._local_ipv4(address_v4)
        try:
            address = IPv6Address(literal)
        except ValueError:
            return None
        mapped = address.ipv4_mapped
        return (
            address.is_loopback
            or address.is_unspecified
            or address.is_link_local
            or address in cls.UNIQUE_LOCAL_IPV6
            or (mapped is not None and cls._local_ipv4(mapped))
        )

    @classmethod
    def _local_url_spans(cls, text: str) -> Iterator[tuple[int, int]]:
        """Find the private authorities to replace, together with any credentials they carry.

        A complete authority followed by a path or a nonempty query or fragment gives its userinfo context.
        In bare text a comma can end a URL and introduce prose. Prefer the longest private candidate so punctuation
        inside credentials cannot cause a partial replacement.
        """

        for url in cls.URL.finditer(text):
            authority, suffix = url.groups()
            complete = cls._local_authority(authority)
            if complete:
                yield url.start(), url.end() + len(authority.rstrip("."))
            elif not suffix or complete is None:
                for boundary in reversed(list(cls.URL_PUNCTUATION.finditer(authority))):
                    prefix = authority[:boundary.start()].rstrip(".")
                    if cls._local_authority(prefix):
                        yield url.start(), url.end() + len(prefix)
                        break

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
        hostname = socket.gethostname()
        if "." in hostname:
            # Replacing the first label alone leaves the private domain behind.
            # Literals run before word replacements, so the full name wins.
            scrubber.literal.append((hostname, "host"))
            # Match case variants before the short label erases the evidence
            # that the remaining domain belonged to the full host name.
            scrubber.words.append((hostname, "host"))
            scrubber.forbidden.append(hostname)
        for value, placeholder in ((getpass.getuser(), "user"), (hostname.split(".")[0], "host")):
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
        for start, end in reversed(list(self._local_url_spans(text))):
            text = text[:start] + self.LOCAL_URL_PLACEHOLDER + text[end:]
        return text

    def check(self, text: str) -> str | None:
        """Name the first identifying value still present in `text`, or None when it is clean."""

        lowered = text.lower()
        for value in self.forbidden:
            if value.lower() in lowered:
                return value
        if self.EMAIL.search(text.replace(self.EMAIL_PLACEHOLDER, "")):
            return "an email address"
        if next(self._local_url_spans(text), None) is not None:
            return "a URL into this machine or its network"
        return None


# --------------------------------------------------------------------------------------------------------------------
# The agent's terminal
# --------------------------------------------------------------------------------------------------------------------


class Terminal:
    """One agent running in a terminal host of its own (`scripts/agent_screen_host.py`).

    The drivers below only read its screen and title and type at it, so nothing in them depends on the kind of
    terminal.
    """

    def __init__(self, farhelm: pathlib.Path) -> None:
        self.farhelm = farhelm
        self.run: HostRun | None = None

    def start(self, cwd: pathlib.Path, argv: list[str]) -> None:
        """Start the agent in a terminal of the production size."""

        self.run = HostRun.start(self.farhelm, cwd, TERMINAL_COLUMNS, TERMINAL_ROWS, argv)

    def _run(self) -> HostRun:
        assert self.run is not None, "the agent was not started"
        return self.run

    def state(self) -> tuple[str, str, bool]:
        """The visible screen as the supervisor's sampler reads it, the title, and whether the agent exited."""

        return self._run().state()

    def screen(self) -> str:
        return self.state()[0]

    def title(self) -> str:
        """The title the agent last set through OSC 0/2."""

        return self.state()[1]

    def dead(self) -> bool:
        return self.state()[2]

    def keys(self, *keys: str) -> None:
        """Send named keys (`Enter`, `Escape`, `Down`, `C-u`)."""

        self._run().keys(*keys)

    def text(self, text: str) -> None:
        """Type literal text without submitting it."""

        self._run().type(text)

    def wait_for(
        self, what: str, predicate: Callable[[str, str], bool], timeout: float, poll_seconds: float = POLL_SECONDS
    ) -> tuple[str, str]:
        """Poll until `predicate(screen, title)` holds; return that screen and title.

        Raises `ScenarioTimeout` with the last screen attached when the deadline passes, and fails fast when the agent
        process has died, since no later screen can satisfy anything. Short-lived states such as compaction can request
        a faster poll without making every ordinary readiness wait sample that often.
        """

        deadline = time.monotonic() + timeout
        screen = title = ""
        while True:
            screen, title, dead = self.state()
            if predicate(screen, title):
                return screen, title
            if dead:
                raise ScenarioTimeout(f"the agent exited while waiting for {what}:\n{screen}")
            if time.monotonic() >= deadline:
                raise ScenarioTimeout(f"timed out waiting for {what}; last screen:\n{screen}")
            time.sleep(poll_seconds)

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
        if self.run is not None:
            self.run.stop()


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
    # The controller stopped serving this drive. Earlier scrubbed frames still belong to this capture.
    drive_failed: bool = False
    # The terminal host could not be confirmed stopped. The screens it showed are still written; the run fails so
    # the kept run directory (named in the error) gets looked at.
    stop_failed: bool = False

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


def claude_compaction_spinner(screen: str) -> bool:
    """Recognize multi-word work with the reader's glyph, ellipsis and digit-led parenthesis guards.

    The ordinary turn pattern requires seconds and one word, so it misses minute-long compaction. This capture
    predicate searches the whole screen; the fixture tests independently require the saved indicator to be near the
    input box, where the supervisor actually reads it.
    """

    for line in screen.splitlines():
        line = line.strip()
        if len(line) < 2 or line[0].isalnum() or line[1] != " ":
            continue
        verb, separator, after = line[2:].partition("… (")
        if separator and verb.strip() and after and "0" <= after[0] <= "9":
            return True
    return False


def drive_claude(term: Terminal, rec: Recorder, work: pathlib.Path) -> None:
    """Capture trust, menus, idle, work, permission and question dialogs, the `/clear` hint, then compaction.

    Compaction comes last because the earlier turns supply the context it compacts.
    """

    # The idle hint normally needs 75 idle minutes and 100k tokens of context; both thresholds are Claude's own
    # environment variables, so the tool lowers them rather than waiting.
    term.start(
        work,
        [
            "env", "CLAUDE_CODE_IDLE_THRESHOLD_MINUTES=1", "CLAUDE_CODE_IDLE_TOKEN_THRESHOLD=0",
            "claude", "--permission-mode", "default", "--model", "sonnet",
        ],
    )

    def answer_trust(screen: str) -> None:
        # The menu can default to "No, exit"; move only when the selection proves Enter would refuse.
        if "❯ No, exit" in screen:
            term.keys("Down")
            time.sleep(0.5)
        term.keys("Enter")

    if not rec.attempt(
        "fresh prompt",
        lambda: reach_prompt(
            term, rec, lambda s: "Yes, I trust this folder" in s, answer_trust, lambda s, _t: claude_prompt_box(s)
        ),
    ):
        return

    # Screens that carry no state (the reader must answer "can't tell"), reachable without a model turn.
    def model_picker() -> None:
        term.submit("/model", lambda s: (claude_input(s) or "").startswith("/model"))
        screen, title = term.wait_for("the model picker", lambda s, _t: "Enter to set as default" in s, 30)
        rec.save("unknown", "model-picker", screen, title)
        term.keys("Escape")
        term.wait_for("the prompt box", lambda s, _t: claude_prompt_box(s), 30)

    rec.attempt("model picker", model_picker)

    def transcript() -> None:
        term.keys("C-o")
        screen, title = term.wait_for(
            "the transcript view", lambda s, _t: "Showing detailed transcript" in s, 30
        )
        rec.save("unknown", "transcript", screen, title)
        term.keys("C-o")
        term.wait_for("the prompt box", lambda s, _t: claude_prompt_box(s), 30)

    rec.attempt("transcript view", transcript)
    if rec.ui_only:
        return

    def thinking_then_permission() -> None:
        term.submit(
            "Use your Bash tool to run exactly this command: sleep 20 && echo capture-done > marker.txt",
            lambda s: "run exactly this command" in (claude_input(s) or ""),
        )
        screen, title = term.wait_for(
            "the spinner or the permission prompt",
            lambda s, _t: claude_spinner(s) or "Do you want to proceed?" in s,
            120,
        )
        if claude_spinner(screen):
            rec.save("working", "thinking", screen, title)
        screen, title = term.wait_for(
            "the permission prompt", lambda s, _t: "Do you want to proceed?" in s and "Esc to cancel" in s, 120
        )
        rec.save("waiting", "permission", screen, title)
        term.keys("Enter")

    if not rec.attempt("permission prompt", thinking_then_permission):
        return

    def tool_running() -> None:
        for sample in (1, 2):
            screen, title = term.wait_for("the tool-run spinner", lambda s, _t: claude_spinner(s), 60)
            rec.save("working", f"tool-{sample}", screen, title)
            time.sleep(4)

    rec.attempt("tool run", tool_running)

    def after_turn() -> None:
        screen, title = term.wait_for(
            "the finished turn",
            lambda s, _t: re.search(r" for \d+[ms]", s) is not None and claude_prompt_box(s) and not claude_spinner(s),
            120,
        )
        rec.save("idle", "after-turn", screen, title)

    if not rec.attempt("idle after a turn", after_turn):
        return

    # Independent of the question below: a vendor dropping its idle hint must not cost the waiting capture.
    def clear_hint() -> None:
        screen, title = term.wait_for("the /clear hint", lambda s, _t: "/clear to save" in s, 180)
        rec.save("idle", "clear-hint", screen, title)

    rec.attempt("idle /clear hint", clear_hint)

    def question() -> None:
        term.wait_for("the prompt box", lambda s, _t: claude_prompt_box(s), 60)
        term.keys("C-u")
        term.submit(
            "Use your AskUserQuestion tool to ask me one question: tea or coffee? Two options. Do nothing else.",
            lambda s: "AskUserQuestion tool" in (claude_input(s) or ""),
        )
        screen, title = term.wait_for(
            "the question form", lambda s, _t: "Enter to select" in s and "Esc to cancel" in s, 120
        )
        rec.save("waiting", "question", screen, title)
        term.keys("Escape")

    rec.attempt("question form", question)

    def compaction() -> None:
        """Capture the first observed compaction spinner; minute timers are accepted too.

        The question step dismisses its dialog without waiting for the input box to return. Compaction must start from
        that box, and its multi-word spinner must not use the ordinary turn predicate, which only accepts seconds.
        Submission still has its usual 1.5-second polling interval; the faster observation afterward cannot catch a
        compaction that has already finished during submission.
        """

        # A returned input box can coexist with the question turn still finishing. Wait for that work to end so its
        # spinner cannot be saved as the compaction fixture before /compact starts.
        term.wait_for(
            "the idle prompt box before compaction",
            lambda s, _t: claude_prompt_box(s) and not claude_compaction_spinner(s),
            60,
        )
        term.keys("C-u")
        term.submit("/compact", lambda s: (claude_input(s) or "").startswith("/compact"))
        screen, title = term.wait_for(
            "the compaction spinner",
            lambda s, _t: claude_compaction_spinner(s),
            120,
            poll_seconds=0.1,
        )
        rec.save("working", "compacting", screen, title)

    rec.attempt("compaction", compaction)


def reach_prompt(
    term: Terminal,
    rec: Recorder,
    is_trust: Callable[[str], bool],
    answer_trust: Callable[[str], None],
    ready: Callable[[str, str], bool],
    timeout: float = 120,
    settle: float = 3,
) -> None:
    """Get from launch to a settled input prompt, answering the folder-trust dialog whenever it appears.

    One loop rather than "wait for the dialog, then wait for the prompt": an agent can paint a frame that looks like
    its prompt before the dialog arrives, and a dialog that swallows the answering keystroke must be answered again.
    The prompt counts as ready only once it has stayed ready for `settle` seconds, so the saved `idle-fresh` screen is
    the settled first paint rather than a half-drawn one. The dialog is saved once, as `waiting-trust`; an agent that
    reaches its prompt without showing it (its configuration already trusts the scratch directory or a parent of it,
    `/tmp` say) has the trust screen reported as not captured, like any other missed scenario.
    """

    deadline = time.monotonic() + timeout
    trust_saved = False
    ready_since: float | None = None
    while True:
        screen, title, dead = term.state()
        # Checked before any branch can `continue`: a dialog that never accepts its answer (an agent ignoring its input
        # entirely) must still end in a timeout, and an exited agent keeps showing its last screen.
        if dead:
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
        if ready(screen, title):
            ready_since = ready_since or time.monotonic()
            if time.monotonic() - ready_since >= settle:
                rec.save("idle", "fresh", screen, title)
                if not trust_saved:
                    rec.missed.append(
                        "waiting-trust (not captured: no trust dialog appeared; the agent's own configuration may "
                        "already trust the scratch directory or a parent of it)"
                    )
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


def codex_ready(screen: str, title: str) -> bool:
    """Whether Codex sits idle at its composer: the composer drawn and no Braille spinner in the title.

    Codex 0.166.0 was seen drawing its composer while its title still spun after starting, which the Codex reader
    reads as working. A fresh screen saved then would be labelled
    idle while showing work.
    """

    return codex_composer(screen) and BRAILLE_SPINNER.match(title) is None


def drive_codex(term: Terminal, rec: Recorder, work: pathlib.Path) -> None:
    """Drive Codex through trust, its model menu, idle, streaming, approval, a tool run, a recap, and a question form."""

    term.start(work, ["codex", "-s", "read-only", "-a", "on-request"])

    if not rec.attempt(
        "fresh composer",
        lambda: reach_prompt(
            term, rec, lambda s: "Trust this folder?" in s, lambda _s: term.keys("Enter"), codex_ready
        ),
    ):
        return

    # A screen that carries no state (the reader must answer "can't tell"), reachable without a model turn.
    def model_picker() -> None:
        term.submit("/model", lambda s: (codex_input(s) or "").startswith("/model"))
        screen, title = term.wait_for("the model picker", lambda s, _t: "Select Model and Effort" in s, 30)
        rec.save("unknown", "model-picker", screen, title)
        term.keys("Escape")
        term.wait_for("the composer", lambda s, _t: codex_composer(s), 30)

    rec.attempt("model picker", model_picker)
    if rec.ui_only:
        return

    def streaming_then_approval() -> None:
        prompt = "Run exactly this shell command: sleep 20 && echo capture-done > marker.txt"
        term.submit(prompt, lambda s: prompt in (codex_input(s) or ""))
        # While Codex streams prose there is no on-screen working widget; only the title's Braille spinner says so.
        screen, title = term.wait_for(
            "streaming or the approval prompt",
            lambda s, t: (BRAILLE_SPINNER.match(t) is not None and "Working (" not in s) or "esc to cancel" in s,
            120,
        )
        if "esc to cancel" not in screen:
            rec.save("working", "streaming", screen, title)
        screen, title = term.wait_for(
            "the approval prompt", lambda s, _t: "Press enter to confirm or esc to cancel" in s, 180
        )
        rec.save("waiting", "permission", screen, title)
        term.keys("Enter")

    if not rec.attempt("approval prompt", streaming_then_approval):
        return

    def tool_running() -> None:
        for sample in (1, 2):
            screen, title = term.wait_for("the Working widget", lambda s, _t: "Working (" in s, 60)
            rec.save("working", f"tool-{sample}", screen, title)
            time.sleep(4)

    rec.attempt("tool run", tool_running)

    def after_turn() -> None:
        term.wait_for(
            "the finished turn",
            lambda s, t: "Worked for" in s and "Working (" not in s and not BRAILLE_SPINNER.match(t),
            180,
        )
        time.sleep(2)
        rec.save("idle", "after-turn", term.screen(), term.title())

    if not rec.attempt("idle after a turn", after_turn):
        return

    # Independent of the question below: a vendor dropping `/recap` must not cost the waiting capture.
    def recap() -> None:
        term.submit("/recap", lambda s: (codex_input(s) or "").startswith("/recap"))
        screen, title = term.wait_for("the recap", lambda s, _t: "Recap:" in s and codex_composer(s), 180)
        rec.save("idle", "recap", screen, title)

    rec.attempt("idle recap", recap)

    def question() -> None:
        term.wait_for("the composer", lambda s, _t: codex_composer(s), 60)
        term.keys("C-u")
        term.submit(
            "Use your request_user_input tool to ask me one question: tea or coffee? Offer exactly two options. "
            "Do nothing else.",
            lambda s: "request_user_input tool" in (codex_input(s) or ""),
        )
        # Codex 0.159 reports this tool unavailable in its default mode; the scenario stays for versions that allow it.
        screen, title = term.wait_for(
            "the pending question", lambda s, t: "1 question" in s or "Action Required" in t, 120
        )
        rec.save("waiting", "question", screen, title)

    rec.attempt("question form", question)


# --------------------------------------------------------------------------------------------------------------------
# Running a harness and publishing its fixtures
# --------------------------------------------------------------------------------------------------------------------

HARNESSES: dict[str, tuple[list[str], Callable[[Terminal, Recorder, pathlib.Path], None]]] = {
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


class CapturePublicationError(RuntimeError):
    """A failed publication needs its validated staging directory for recovery or retry."""


@contextmanager
def lock_fixture_version(target: pathlib.Path):
    """Use the version directory as the lock, so readers can share it and no lock file needs committing."""
    try:
        directory = os.open(target, os.O_RDONLY | os.O_DIRECTORY)
        try:
            fcntl.flock(directory, fcntl.LOCK_EX)
            yield
        finally:
            os.close(directory)
    except (OSError, ValueError) as error:
        raise CapturePublicationError(f"could not finish fixture publication in {target}: {error}") from error


def fixture_pair_names(stem: str) -> tuple[str, str]:
    """Names in a recovered journal must be fixture basenames in the locked directory."""
    if not re.fullmatch(r"(?:working|waiting|idle|unknown)-[a-zA-Z0-9_-]+", stem):
        raise ValueError(f"invalid capture fixture stem {stem!r}")
    return f"{stem}.txt", f"{stem}.title"


def publication_path_exists(path: pathlib.Path, file_type: int) -> bool:
    """Only a missing entry means absence. An unreadable journal or pair must stay recoverable."""
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return False
    if stat.S_IFMT(mode) != file_type:
        raise ValueError(f"unexpected file type in fixture publication: {path}")
    return True


def recover_fixture_publication(target: pathlib.Path) -> None:
    """Roll back an interrupted swap using backups kept intact until every pair is restored."""
    journal = target / PUBLICATION_JOURNAL
    if not publication_path_exists(journal, stat.S_IFDIR):
        return
    manifest = journal / "manifest.json"
    if publication_path_exists(manifest, stat.S_IFREG):
        prior = json.loads(manifest.read_text())
        if not isinstance(prior, dict) or not all(isinstance(value, bool) for value in prior.values()):
            raise ValueError("invalid capture publication manifest")
        pairs = [(fixture_pair_names(stem), existed) for stem, existed in prior.items()]
        for names, existed in pairs:
            if existed and not all(
                publication_path_exists(journal / "old" / name, stat.S_IFREG) for name in names
            ):
                raise ValueError(f"missing backup for capture fixture pair {names}")
        restore = journal / "restore"
        for names, existed in pairs:
            for name in names:
                destination = target / name
                if existed:
                    # Link the saved inode without allocating the contents again after a disk-space failure.
                    # Keep the backup link through replace, so a second interruption can retry rollback.
                    replacement = restore / name
                    replacement.unlink(missing_ok=True)
                    os.link(journal / "old" / name, replacement)
                    os.replace(replacement, destination)
                else:
                    destination.unlink(missing_ok=True)
        manifest.unlink()
    # With no manifest, either the replacement never started or a complete pair set is already visible.
    shutil.rmtree(journal)


def publish_fixture_pairs(rec: Recorder, target: pathlib.Path) -> None:
    """Stage both sides of every replacement before allowing a version's first fixture to move."""
    try:
        target.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        raise CapturePublicationError(f"could not create fixture directory {target}: {error}") from error
    with lock_fixture_version(target):
        recover_fixture_publication(target)
        journal = target / PUBLICATION_JOURNAL
        journal.mkdir()
        try:
            new = journal / "new"
            old = journal / "old"
            new.mkdir()
            old.mkdir()
            (journal / "restore").mkdir()
            prior = {}
            pairs = []
            for stem in sorted(set(rec.captured)):
                names = fixture_pair_names(stem)
                existing = [publication_path_exists(target / name, stat.S_IFREG) for name in names]
                if any(existing) and not all(existing):
                    raise ValueError(f"fixture {stem!r} already has an incomplete pair")
                prior[stem] = all(existing)
                pairs.append(names)
                for name in names:
                    shutil.copy2(rec.staging / name, new / name)
                    if prior[stem]:
                        shutil.copy2(target / name, old / name)
            manifest = journal / "manifest.json"
            pending = journal / "manifest.pending"
            pending.write_text(json.dumps(prior))
            os.replace(pending, manifest)
            for names in pairs:
                for name in names:
                    os.replace(new / name, target / name)
            # Removing the manifest commits the swap. A crash during removal of backups now only needs cleanup.
            manifest.unlink()
        except BaseException:
            recover_fixture_publication(target)
            raise
        try:
            shutil.rmtree(journal)
        except OSError as error:
            raise CapturePublicationError(
                f"new fixture pairs are installed in {target}. Run capture again to finish journal cleanup: {error}"
            ) from error


def capture_harness(
    name: str, farhelm: pathlib.Path, work_root: pathlib.Path, scrubber: Scrubber, ui_only: bool
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
    target = FIXTURES_ROOT / name / version
    try:
        if publication_path_exists(target, stat.S_IFDIR):
            with lock_fixture_version(target):
                recover_fixture_publication(target)
    except (OSError, ValueError) as error:
        raise CapturePublicationError(f"could not check for unfinished fixtures in {target}: {error}") from error
    work = work_root / name
    work.mkdir()
    staging = work_root / f"{name}-fixtures"
    staging.mkdir()
    rec = Recorder(staging, scrubber, ui_only=ui_only)
    term = Terminal(farhelm)
    print(f"{name} {version}: capturing", file=sys.stderr)
    try:
        drive(term, rec, work)
    except HostError as error:
        detail = scrubber.scrub(str(error))
        print(f"{name} {version}: drive failed: {detail}", file=sys.stderr)
        rec.missed.append(f"terminal host drive failed: {detail}")
        rec.drive_failed = True
    finally:
        # A stop that cannot be confirmed must not throw away screens already captured with real vendor turns, nor
        # skip the remaining harnesses: report it, keep the staged screens, and fail the run at the end.
        try:
            term.close()
        except HostError as error:
            print(f"{name} {version}: {error}", file=sys.stderr)
            rec.missed.append(f"terminal host stop unconfirmed: {error}")
            rec.stop_failed = True
    if not rec.captured:
        print(f"{name} {version}: nothing captured; leaving existing fixtures alone", file=sys.stderr)
        return rec
    publish_fixture_pairs(rec, target)
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
        "--selection", "screen reader fixtures", "--concurrency", "4 nextest slots; retries 0",
        "--", "cargo", "nextest", "run", "-p", "farhelm-supervisor", "-E", "test(screen_fixtures)",
    ]
    print("running: " + " ".join(command), file=sys.stderr)
    return subprocess.run(command, cwd=REPO_ROOT, env=env, check=False).returncode


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--harness", action="append", choices=sorted(HARNESSES), help="capture only this agent (repeatable)"
    )
    parser.add_argument(
        "--farhelm", type=pathlib.Path, default=default_farhelm(), help="the built farhelm (default: %(default)s)"
    )
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
    if not args.farhelm.is_file():
        print(f"{args.farhelm} is missing; build it with `cargo build -p farhelm`", file=sys.stderr)
        return 2
    # Short on purpose: the scratch path appears in agent banners and status lines, and a long one wraps them.
    work_root = pathlib.Path(tempfile.mkdtemp(prefix="fhc-", dir="/tmp"))
    scrubber = Scrubber.for_host(work_root)
    results: dict[str, Recorder] = {}
    retain_work = args.keep_work
    try:
        for name in args.harness or sorted(HARNESSES):
            results[name] = capture_harness(name, args.farhelm, work_root, scrubber, args.ui_only)
    except CapturePublicationError as error:
        retain_work = True
        print(error, file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        retain_work = True
        raise
    finally:
        if retain_work:
            print(f"scratch kept at {work_root}", file=sys.stderr)
        else:
            shutil.rmtree(work_root, ignore_errors=True)

    for name, rec in results.items():
        print(f"{name}: captured {', '.join(rec.captured) or 'nothing'}")
        for miss in rec.missed:
            print(f"{name}: {miss}")
    status = 0 if all(rec.captured and not (rec.drive_failed or rec.stop_failed) for rec in results.values()) else 1
    if not args.no_test:
        status = run_fixture_tests() or status
    return status


if __name__ == "__main__":
    sys.exit(main())
