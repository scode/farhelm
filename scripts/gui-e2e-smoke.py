#!/usr/bin/env python3
"""End-to-end smoke test of the GPUI app: the real app, its embedded helm and a real supervisor with fake agents,
driven through the test-automation bridge (`scripts/gui-agent.py`, `docs/gui-agent.md`).

NOTE: This is a smoke test, not the end-to-end suite. It covers the first slice's path through the app once each
(the session list, a row's menu, Delete, Rename, and typing into the open session's terminal), so a break in the wiring
between the app, the helm and the supervisor shows up somewhere, plus the area steps added since (from 30, the
terminal's: a large paste, on a Mac only, and text an input method commits). The behavior itself is specified and
tested in process (`crates/gui/farhelm-gui/src/session_list/`, `session_pane.rs`, `terminal/attached/tests.rs`);
nothing here checks layout or wording beyond the names it clicks.

    python3 -B scripts/gui-e2e-smoke.py [--no-build] [--keep]

It starts one app run with three basic fake-agent sessions (`Fixture 1` to `Fixture 3`) and runs the steps below in
order against it, because each step leaves the app where the next one begins and a fresh app per step would repeat the
whole start (the build check, the app, its helm and supervisor) for each. Every step checks its outcome where it really
lands: a Delete or a Rename through the helm's own listing as well as in the window, and typing through the open
terminal's text. The run directory is made under /tmp (the same place `scripts/gui-screenshot-batch.py` makes its runs),
deleted when every step passed and the run stopped cleanly, and kept for its logs otherwise; `--keep` keeps it either
way. `--no-build` reuses the binaries already built. Ctrl-C or SIGTERM ends the steps and still stops the app run.

An area adds the steps for its own feature at the end of `Smoke` (the Test-automation bridge row of
`docs/gpui/foundation-ownership.md`). Two rules keep the steps of several areas from depending on each other, because
they all share the one app run:

- Steps run in name order, and unittest sorts names as strings, so a step is `test_<nn>_...` with a two-digit number
  after the last one taken (`test_10_` would sort before `test_1_`, since `0` sorts before `_`). `main` refuses to run
  a step named otherwise. Two areas taking the same number is harmless, since their steps share nothing.
- Each area's steps seed and use sessions of their own (created through the helm's API with `gui_agent.helm_request`,
  under titles no other step uses; `gui_agent.seed_body` always titles its session `Fixture <n>`, so a body built with
  it gets its title overridden), set up the open session and focus their steps depend on rather than assume what an
  earlier step left, and never touch a session another area's steps seed or use. The three `Fixture` sessions `start`
  seeds belong to steps 01 to 05, the first slice's, which leave Fixture 3 deleted, Fixture 1 renamed, and Fixture 2
  open with focus in its terminal (each step's `setUp` presses Escape, which reaches that terminal).

Needs what `gui-agent.py start` needs: on Linux `mutter`, `dbus-run-session` and lavapipe; on a Mac, a logged-in GUI
session of the same user. Run it through `scripts/record-test-run.py` (`.agents/validation.md` has the command).
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
AGENT = REPO / "scripts/gui-agent.py"

# The harness itself, for its helm request helper: the test reads the helm's listing with the same credential file and
# request shape `start` seeds with, rather than a second copy of them.
spec = importlib.util.spec_from_file_location("gui_agent", AGENT)
gui_agent = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(gui_agent)

# Where runs go: a short path, so the run's deepest socket fits the platform's limit (`gui-agent.py`'s
# `check_run_dir`), and never the checkout or the platform's deeper default temporary directory.
RUNS_PARENT = Path("/tmp")

# How long one bridge command may take before it counts as hung. The bridge's own waits give up after 25 seconds and
# `terminal-text --wait` after 30; this only stops a command that never returns from hanging the whole test.
STEP_TIMEOUT_SECONDS = 120

# How long a step waits for an effect it polls for itself (a session gone from the helm's listing, a new title in it).
EFFECT_TIMEOUT_SECONDS = 60

# The Delete menu item's description: visible text only the open row menu draws, so waiting for it (or for it to go)
# tells the menu's state apart from the list's.
MENU_DESCRIPTION = "Removes the session and its state"

# What every step's name must look like (the module docs say why); `main` checks it before starting the app.
STEP_NAME = re.compile(r"test_\d\d_")

# Set by `main` once the app is up; the test case reads it.
RUN: Path | None = None


def agent(*args: str, timeout: float | None = STEP_TIMEOUT_SECONDS) -> subprocess.CompletedProcess:
    """One `gui-agent.py` command against this test's run, its output captured for the failure message."""
    assert RUN is not None
    return subprocess.run(
        [sys.executable, str(AGENT), str(RUN), *args], capture_output=True, text=True, timeout=timeout
    )


def listed_titles() -> dict[str, str]:
    """Every session the helm lists, by id, with its title: what the helm holds, whatever the window shows."""
    assert RUN is not None
    return {row["id"]: row["title"] for row in gui_agent.helm_request(RUN, "GET", "/api/sessions")["sessions"]}


def eventually(check, what: str, timeout: float = EFFECT_TIMEOUT_SECONDS):
    """`check()`'s first truthy answer, asked again every half second, or a failure naming `what` after `timeout`."""
    deadline = time.monotonic() + timeout
    while True:
        answer = check()
        if answer:
            return answer
        if time.monotonic() > deadline:
            raise AssertionError(f"{what} did not happen within {timeout:.0f} s")
        time.sleep(0.5)  # sleep-ok: polling the helm's listing or the bridge; neither offers a wait on this


class Smoke(unittest.TestCase):
    """The first slice's path through the real app, one step per test, in name order against one app run (the module
    docs have the rules for adding a step).

    Each test starts by pressing Escape, so a step that failed with a menu or dialog open does not also fail the steps
    after it for that reason alone. Escape reaches the open terminal when nothing else is open; the basic fake agent
    ignores it.
    """

    def setUp(self) -> None:
        agent("press", "escape")

    def ok(self, *args: str) -> str:
        """Run a bridge command that must succeed; its stdout."""
        result = agent(*args)
        self.assertEqual(result.returncode, 0, f"`{' '.join(args)}` failed:\n{result.stdout}{result.stderr}")
        return result.stdout

    def test_01_the_list_shows_every_seeded_session(self) -> None:
        """The list is where every other step starts, and it is fed by the helm's push over the UI channel, not by
        anything this test seeds into the app. Specified: the helm lists the three seeded sessions, and the window
        shows a row for each by title."""
        titles = sorted(listed_titles().values())
        self.assertEqual(titles, ["Fixture 1", "Fixture 2", "Fixture 3"], "premise: the helm lists the seeded fleet")
        for title in titles:
            self.ok("wait", "--text", title)

    def test_02_the_open_sessions_terminal_takes_typing(self) -> None:
        """Typing into the open session's terminal crosses every layer this slice joined: the click opens the session
        and hands its terminal focus, the terminal attaches through the helm to the supervisor's terminal host, and
        keys travel back to the program. Specified: after a click on a row, the focused terminal shows the fake agent's
        ready line; a line typed there, stamped per run so an earlier run's echo cannot pass for it, comes back as the
        agent's `echo:` of it. (The old browser suite's `grid-terminal.spec.ts` case.) The run's state directory is
        fresh, so no session is open before the click and only the click can have focused a terminal."""
        self.ok("click", "text:Fixture 2")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        line = f"typed-in-{uuid.uuid4().hex[:8]}"
        self.ok("type", "--keys", line)
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", f"echo:{line}")

    def test_03_a_rows_menu_opens_from_its_button_and_escape_closes_it(self) -> None:
        """The row menu is how a session's commands are reached, and a menu left open would keep the keyboard from the
        terminal. Specified: a click on a row's actions button, found by its accessible name, opens that row's menu;
        Escape closes it again."""
        self.ok("click", "text:Session actions for Fixture 1")
        self.ok("wait", "--text", MENU_DESCRIPTION)
        self.ok("press", "escape")
        self.ok("wait", "--gone", MENU_DESCRIPTION)

    def test_04_delete_from_the_row_menu_deletes_the_session(self) -> None:
        """Delete is the slice's one command that changes the fleet, through the operation store and the helm's API.
        Specified: the menu's Delete asks first (the fake agent is still running), the answer Confirm delete removes
        the session from the helm's listing, and the window then draws no row for it. The listing is the oracle: the
        row shows a progress line in place of its title while the Delete runs, so the title leaving the screen proves
        nothing; the row's accessible name (its title) stays until the row goes."""
        ids = [id for id, title in listed_titles().items() if title == "Fixture 3"]
        self.assertEqual(len(ids), 1, "premise: the helm lists Fixture 3 once")
        self.ok("click", "text:Session actions for Fixture 3")
        self.ok("wait", "--text", MENU_DESCRIPTION)
        self.ok("click", "text:Delete")
        self.ok("wait", "--text", "Confirm delete")
        self.ok("click", "text:Confirm delete")
        eventually(lambda: ids[0] not in listed_titles(), "Fixture 3 leaving the helm's listing")
        eventually(lambda: '"Fixture 3"' not in self.ok("snapshot"), "Fixture 3's row leaving the window")

    def test_05_rename_from_the_row_menu_retitles_the_session(self) -> None:
        """Rename is the slice's other command, and the one that goes through a dialog with a text field. Specified:
        the menu's Rename opens the "Rename session" dialog; a new title filled in and saved closes it, the helm lists
        the session under the new title, and the window's row shows it."""
        ids = [id for id, title in listed_titles().items() if title == "Fixture 1"]
        self.assertEqual(len(ids), 1, "premise: the helm lists Fixture 1 once")
        self.ok("click", "text:Session actions for Fixture 1")
        self.ok("wait", "--text", MENU_DESCRIPTION)
        self.ok("click", "text:Rename")
        self.ok("wait", "--text", "Rename session")
        title = f"Renamed {uuid.uuid4().hex[:8]}"
        self.ok("fill", "text:New session title", title)
        self.ok("click", "text:Save")
        self.ok("wait", "--gone", "Rename session")
        eventually(lambda: listed_titles().get(ids[0]) == title, "the helm listing Fixture 1 under its new title")
        self.ok("wait", "--text", title)

    # ---- The terminal area's steps (30 onward), on sessions of their own ----------------------------------------------

    # NOTE: Wayland lets a client own the clipboard only while one of its windows has keyboard or pointer focus, from a
    # press serial the compositor gave it. Headless Mutter never focuses the app's window that way (the bridge injects
    # input inside the app), so GPUI drops the program's clipboard write and there is nothing to paste. The step runs
    # where the app can own the clipboard, which is a Mac; on Linux the in-process tests cover paste
    # (`crates/gui/farhelm-gui/src/terminal/attached/tests.rs`).
    @unittest.skipUnless(
        sys.platform == "darwin", "headless Mutter gives the app no clipboard ownership (see the NOTE above)"
    )
    def test_30_a_large_paste_arrives_whole_inside_its_brackets(self) -> None:
        """Paste is the user's way to hand a program a block of text, and it crosses every layer: the system
        clipboard, the app's paste action, the helm client's input group, the helm's relay and the supervisor's
        bracketed-paste encoding and sanitizing. The old app's large-paste test (`terminal-flood.spec.ts` at the
        reference commit) is the case: over a megabyte must arrive whole, which a single input message cannot carry.
        Specified: a receiver of this step's own puts more than 1 MiB on the clipboard itself (an OSC 52 write, so no
        clipboard tool is needed on the machine), with an end-of-paste marker in the middle; the platform's paste chord
        then delivers exactly that text, less the marker, between one pair of bracketed-paste markers. The receiver's
        own byte count and digest are the oracle, because a lost, repeated or reordered part, or a marker that ended
        the paste early, changes them."""
        work = RUN.parent / "paste"
        work.mkdir()
        # A 23-letter cycle, which no 32 KiB part boundary lines up with, so every part's text differs from its
        # neighbours'.
        size = 1024 * 1024 + 1
        text = ("abcdefghijklmnopqrstuvw" * (size // 23 + 1))[:size]
        (work / "payload").write_bytes((text[: size // 2] + "\x1b[201~" + text[size // 2 :]).encode())
        want = text.encode()
        (work / "receiver.py").write_text(PASTE_RECEIVER)
        title = f"Paste receiver {uuid.uuid4().hex[:8]}"
        command = f"python3 {work / 'receiver.py'} {work / 'payload'}"
        gui_agent.helm_request(
            RUN,
            "POST",
            "/api/sessions",
            {"cwd": str(work), "title": title, "command": {"command": command, "yolo": False}},
        )
        self.ok("wait", "--text", title)
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "PASTE-RECEIVER-READY")
        self.ok("type", "--keys", "copy")
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", "PASTE-RECEIVER-COPIED")
        # The premise: the program's clipboard write reached the system clipboard. Without this check a write the app
        # dropped shows up only as a paste that never arrives, which reads like a paste bug.
        copied = subprocess.run(["pbpaste"], capture_output=True, timeout=STEP_TIMEOUT_SECONDS).stdout
        self.assertEqual(
            hashlib.sha256(copied).hexdigest(),
            hashlib.sha256((work / "payload").read_bytes()).hexdigest(),
            "premise: the clipboard holds the receiver's payload",
        )
        self.ok("press", "cmd-v")
        screen = self.ok("terminal-text", "--wait", "PASTE-SHA")
        self.assertIn(f"PASTE-GOT {len(want)}", screen)
        self.assertIn(f"PASTE-SHA {hashlib.sha256(want).hexdigest()[:32]}", screen)

    def test_31_text_an_input_method_commits_reaches_the_program(self) -> None:
        """Text an input method commits (a Mac dead key's `é`, a Linux input method's string) goes to the platform's
        text input, not to a key listener, and reaches the program only through the terminal's input handler; before
        the terminal had one, such text was dropped. The bridge's `type` hands its text to the focused element's input
        handler the way an input method commits, so it exercises exactly that path, once the snapshot shows the
        focused terminal as a text input. Specified: in a session of this step's own running the prompt-and-echo fake
        agent, a non-ASCII string typed that way, then Enter, comes back as the agent's `echo:` of it."""
        record = json.loads((RUN / "run.json").read_text())
        title = f"Input method receiver {uuid.uuid4().hex[:8]}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, record["binaries"]["fixtures"])
        body["title"] = title
        gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", "--text", title)
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        # The premise: the focused terminal is a text input the bridge can commit to. Without an input handler the
        # bridge falls back to one keystroke per character, which the terminal's key listener would type as well, so
        # the echo alone would not show the input method path.
        self.assertRegex(self.ok("snapshot", "-i"), r"textbox .*ctx=Terminal \[focused\]")
        self.ok("type", "id:terminal-grid", "日本語 é")
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", "echo:日本語 é")


# The receiver for the large-paste step: once the step types `copy`, it writes its payload file to the clipboard with
# OSC 52, turns bracketed paste on and reads raw input until a paste's end marker, then reports how many bytes arrived
# between the markers and the first half of their SHA-256 on two short lines (short, so neither wraps in a narrow
# window's terminal: a wrapped line breaks the text the step looks for). Raw mode keeps the terminal from echoing a
# megabyte.
PASTE_RECEIVER = r"""import base64, hashlib, os, sys, termios, tty
payload = open(sys.argv[1], "rb").read()
print("PASTE-RECEIVER-READY", flush=True)
sys.stdin.readline()
saved = termios.tcgetattr(0)
tty.setraw(0)
os.write(1, b"\x1b]52;c;" + base64.b64encode(payload) + b"\x07\x1b[?2004hPASTE-RECEIVER-COPIED\r\n")
data = bytearray()
start = end = -1
while end < 0:
    chunk = os.read(0, 65536)
    if not chunk:
        break
    data += chunk
    start = data.find(b"\x1b[200~")
    if start >= 0:
        end = data.find(b"\x1b[201~", start + 6)
inside = bytes(data[start + 6 : end]) if start >= 0 and end >= 0 else b""
termios.tcsetattr(0, termios.TCSADRAIN, saved)
os.write(1, b"\x1b[?2004l")
print("PASTE-GOT %d" % len(inside))
print("PASTE-SHA %s" % hashlib.sha256(inside).hexdigest()[:32], flush=True)
sys.stdin.read()
"""


def main(argv: list[str]) -> int:
    """Start the app, run the steps, stop the app; exit 0 only when every step passed and the run stopped cleanly."""
    global RUN
    # SIGTERM takes the same path as Ctrl-C, so the `finally` below stops the app run. Python's default SIGTERM ends the
    # process without running it, which left the compositor, the app, its helm and supervisor and the terminal hosts
    # running (all in sessions of their own); SIGTERM is what the recorder forwards when it is interrupted and what a
    # build worker sends when its caller goes away. KeyboardInterrupt rather than SystemExit, because unittest records a
    # SystemExit raised inside a step as that step's error and goes on to the next, while it re-raises
    # KeyboardInterrupt.
    signal.signal(signal.SIGTERM, signal.default_int_handler)
    parser = argparse.ArgumentParser(prog="gui-e2e-smoke.py")
    parser.add_argument("--no-build", action="store_true", help="reuse the binaries already in the target directory")
    parser.add_argument("--keep", action="store_true", help="keep the run directory even when everything passed")
    options = parser.parse_args(argv)
    # A step named outside the two-digit pattern would sort out of place and run against state its neighbours did not
    # leave, so the run stops here, before anything starts.
    misnamed = [name for name in unittest.defaultTestLoader.getTestCaseNames(Smoke) if not STEP_NAME.match(name)]
    if misnamed:
        print(f"gui-e2e-smoke: steps must be named test_<nn>_...: {', '.join(misnamed)}", file=sys.stderr)
        return 1
    parent = Path(tempfile.mkdtemp(prefix="gui-smoke-", dir=RUNS_PARENT))
    RUN = parent / "run"
    passed = False
    stopped = False
    try:
        start = ["start", "--sessions", "3", *(["--no-build"] if options.no_build else [])]
        # No timeout: `start` may build, and is interrupted only by the caller.
        started = subprocess.run([sys.executable, str(AGENT), str(RUN), *start]).returncode == 0
        if started:
            suite = unittest.defaultTestLoader.loadTestsFromTestCase(Smoke)
            passed = unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful()
        else:
            print("gui-e2e-smoke: the app did not start", file=sys.stderr)
    finally:
        # `stop` also after a failed or interrupted start: it releases whatever `start` had already taken.
        if RUN.exists():
            stopped = subprocess.run([sys.executable, str(AGENT), str(RUN), "stop"]).returncode == 0
            if not stopped:
                print(f"gui-e2e-smoke: `stop` failed; {RUN} may still be running", file=sys.stderr)
        if (passed and stopped and not options.keep) or not RUN.exists():
            # Clean, or a start that failed before making the run directory, which leaves nothing to keep.
            shutil.rmtree(parent)
        else:
            print(f"gui-e2e-smoke: the run is kept at {RUN}", file=sys.stderr)
    return 0 if passed and stopped else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
