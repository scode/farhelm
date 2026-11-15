#!/usr/bin/env python3
"""End-to-end smoke test of the GPUI app: the real app, its embedded helm and a real supervisor with fake agents,
driven through the test-automation bridge (`scripts/gui-agent.py`, `docs/gui-agent.md`).

NOTE: This is a smoke test, not the end-to-end suite. It covers the first slice's path through the app once each (the
session list, a row's menu, Delete, Rename, and typing into the open session's terminal), so a break in the wiring
between the app, the helm and the supervisor shows up somewhere, plus the area steps added since (30, 31 and 33, the
terminal's: a large paste, on a Mac only, text an input method commits, and scrollback; 32, the sidebar's quick
switcher; 34, the session list's Stop and Replace from a row's menu; 35, the sidebar's host rail; 36, the row menu's
Mark read and Mark unread; 37, the sidebar's resize divider; 38, Restart from the open session's header; 39, the host
rail's foot, Hosts and the Settings gear; 40, the settings-and-hosts area's scripted host setup; 41, the host rail's Add
host; 42, the session list's status segments; 43, the settings-and-hosts area's setup progress, failure and rerun in the
Hosts panel; 44, the settings-and-hosts area's Remove host; 45, the other-dialogs area's Help menu and SSH help; 46, the
session list's order menu; 47, the settings-and-hosts area's host Update, its progress, failure and rerun; 48, the
other-dialogs area's Send feedback, up to but never including Send; 49, the session list header's Trash; 50, the session
view's open session that a status filter hides, named and renamed; 51, the session list heading's New and the quick
switcher's New row; 52 and 53, the other-dialogs area's Welcome card, reopened from the Help menu and opened by a window
start that asks for it; 54, the session list's Replace meeting the YOLO question; 60 and 62 to 64, the approvals area's
cards for agents' requests: Allow, the host-only grant, Deny over the quick switcher, and an agent's host add through
the Add host dialog; 61, the settings-and-hosts area's host uninstall, its refusal, run and removal notice; 65, the
other-dialogs area's Keyboard shortcuts overview). The behavior itself is specified and tested in process
(`crates/gui/farhelm-gui/src/session_list/`, `session_pane.rs`, `session_pane/`, `terminal/attached/tests.rs`, `hosts/`,
`help_menu/tests.rs`, `feedback/tests.rs`, `trash/`, `approvals/`, `shortcuts/tests.rs`); nothing here checks layout or
wording beyond the names it clicks. A menu's commands are clicked by their ids (`id:session-menu-delete`), not their
labels: the open session's header has buttons with the same names (Stop, Replace, Delete), and a `text:` target picks
the first match in drawing order, which is the header's.

    python3 -B scripts/gui-e2e-smoke.py [--no-build] [--keep]

It starts one app run with three basic fake-agent sessions (`Fixture 1` to `Fixture 3`) and runs the steps below in
order against it, because each step leaves the app where the next one begins and a fresh app per step would repeat the
whole start (the build check, the app, its helm and supervisor) for each. Every step checks its outcome where it really
lands: a Delete or a Rename through the helm's own listing as well as in the window, and typing through the open
terminal's text. The run directory is made under /tmp (the same place `scripts/gui-screenshot-batch.py` makes its runs),
deleted when every step passed and the run stopped cleanly, and kept for its logs otherwise; `--keep` keeps it either
way. `--no-build` reuses the binaries already built. Ctrl-C or SIGTERM ends the steps and still stops the app run, and
a second run a step started (see the last rule below).

An area adds the steps for its own feature at the end of `Smoke` (the Test-automation bridge row of
`docs/gpui/foundation-ownership.md`). These rules keep the steps of several areas from depending on each other, because
they share the one app run, and say how the one step that needs a start of its own gets it:

- Steps run in name order, and unittest sorts names as strings, so a step is `test_<nn>_...` with a two-digit number
  after the last one taken (`test_10_` would sort before `test_1_`, since `0` sorts before `_`). `main` refuses to run
  a step named otherwise, and refuses a `test_` function written anywhere but directly in `Smoke`, so a module-level
  helper goes after the class, never between its steps. It also refuses two steps sharing a number or a name
  (`step_refusals` checks all three): two areas' steps meeting under one number in a merge would run in an order
  neither chose, and of two with one name only the last runs. Whoever brings them together renames one.
- Each area's steps seed and use sessions of their own (created through the helm's API with `gui_agent.helm_request`,
  under titles no other step uses; `gui_agent.seed_body` always titles its session `Fixture <n>`, so a body built with
  it gets its title overridden), set up the open session and focus their steps depend on rather than assume what an
  earlier step left, and never touch a session another area's steps seed or use. The three `Fixture` sessions `start`
  seeds belong to steps 01 to 05, the first slice's, which leave Fixture 3 deleted, Fixture 1 renamed, and Fixture 2
  open with focus in its terminal (each step's `setUp` presses Escape, which reaches that terminal).
- A step that needs a window start of its own (step 53's start with `--start-dialogs`, which the shared run cannot
  show again) starts a second app run, in a run directory inside the smoke's own parent directory (beside `RUN`), with
  `--no-build` (the smoke's start has built the binaries), and stops it in a `finally` that runs `stop` only once the
  run's `run.json` exists, with the `start` inside the `try`, so an interrupt during it still stops what it started.
  It never touches the shared run's sessions. On a Mac the second app opens a real window in the same GUI session, so
  a later step must not assume the smoke's own window is frontmost (the bridge drives each window by its own socket,
  whichever is in front).

Needs what `gui-agent.py start` needs: on Linux `mutter`, `dbus-run-session` and lavapipe; on a Mac, a logged-in GUI
session of the same user. Run it through `scripts/record-test-run.py` (`.agents/validation.md` has the command).
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import importlib.util
import json
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
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

# The quick switcher's footer: visible text only the open switcher draws, so its going tells that the switcher closed.
SWITCHER_FOOTER = "↑↓ choose · Enter pick · Escape close"

# The host chip's accessible name while the local host is chosen: the snapshot shows it only while the chip is drawn.
LOCAL_CHIP = "Filtered to host Local; remove filter"

# The sidebar's divider holding focus, as `snapshot -i` lists it: its role (AccessKit's focusable separator), its name
# and its state.
DIVIDER_FOCUSED = re.compile(r'splitter "Sidebar width".*\[focused\]')

# The Settings dialog's header summary: text only the open dialog draws, so waiting for it (or for it to go) tells
# whether Settings is open. The smoke app runs no updater, so App is not listed and the summary stops here; with App it
# goes on ", and for this device", which this text matches too.
SETTINGS_SUMMARY = "For every host on this helm"

# The Hosts panel's accessible name, quoted as the snapshot prints it: only the shown panel draws it.
HOSTS_PANEL = '"Host management"'

# The Add host dialog's header summary: text only the open dialog draws, so waiting for it (or for it to go) tells
# whether the dialog is open.
ADD_HOST_SUMMARY = "An SSH destination this helm can reach"

# The removal confirmation's explanation: text only the open question draws, so waiting for it (or for it to go) tells
# whether the question is open.
REMOVE_EXPLANATION = "Farhelm will forget this host."

# The Help menu's summary under its "Help" header, drawn only while the menu is open (step 45).
HELP_MENU_SUMMARY = "Farhelm guides and feedback"

# The SSH help dialog's summary, which the Help menu's SSH item shares as its description: an oracle for the dialog
# only once the menu has gone (step 45).
SSH_HELP_SUMMARY = "Start a host's SSH connections yourself"

# The feedback dialog's footer note before anything was sent: text only the open dialog draws (step 48).
FEEDBACK_NOTE = "Nothing is sent until you press Send"

# The feedback dialog's contact reuse choice, drawn only while the contact field is not blank (step 48).
FEEDBACK_REUSE = "Re-use for future feedback"

# The Welcome card's footer note: text only the open card draws, so the oracle for the card open and closed (steps 52
# and 53).
WELCOME_NOTE = "This card is in the ? menu as Welcome if you want it again."

# The Trash dialog's opening words: drawn whatever the hosts' trash holds, so the oracle for the dialog open and closed
# (step 49).
TRASH_INTRO = "Folders of managed checkouts whose last session was deleted."

# The YOLO question's one-off answer: drawn only while the question is up (step 54).
YOLO_ONE_OFF = "Start YOLO session anyway"

# The launcher's header summary in New: text only the open launcher draws (New's hover help says it with a lowercase
# "start", which this does not match), so waiting for it (or for it to go) tells whether the launcher is open (step 51).
LAUNCHER_SUMMARY = "Start an agent or a command on any host"

# A row only the open Keyboard shortcuts overview draws: the oracle for it open and closed (step 65).
SHORTCUTS_ROW = "Begin launch sequence"

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


def listed_id(title: str) -> str | None:
    """The id the helm lists the session titled `title` under, if it lists exactly one."""
    ids = [id for id, listed in listed_titles().items() if listed == title]
    return ids[0] if len(ids) == 1 else None


def last_selected() -> str | None:
    """The session the helm remembers as last selected: what the app writes when the user opens a session."""
    assert RUN is not None
    return gui_agent.helm_request(RUN, "GET", "/api/preferences").get("last_selected")


def sidebar_width() -> int | None:
    """The sidebar width the helm stores, `None` while it stores none (the app then draws the default)."""
    assert RUN is not None
    return gui_agent.helm_request(RUN, "GET", "/api/preferences").get("sidebar_width")


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
        self.ok("click", "id:session-menu-delete")
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
        self.ok("click", "id:session-menu-rename")
        self.ok("wait", "--text", "Rename session")
        title = f"Renamed {uuid.uuid4().hex[:8]}"
        self.ok("fill", "text:New session title", title)
        self.ok("click", "text:Save")
        self.ok("wait", "--gone", "Rename session")
        eventually(lambda: listed_titles().get(ids[0]) == title, "the helm listing Fixture 1 under its new title")
        self.ok("wait", "--text", title)

    # ---- The terminal area's steps (30), on sessions of their own ----------------------------------------------------

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
        # Waited for by the row's name, not its painted line: this title is long enough that the row cuts it short
        # beside its age, so the whole title is never a painted line.
        self.ok("wait", f"text:{title}")
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        # The premise: the focused terminal is a text input the bridge can commit to. Without an input handler the
        # bridge falls back to one keystroke per character, which the terminal's key listener would type as well, so
        # the echo alone would not show the input method path.
        self.assertRegex(self.ok("snapshot", "-i"), r"textbox .*ctx=Terminal \[focused\]")
        self.ok("type", "id:terminal-grid", "日本語 é")
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", "echo:日本語 é")

    # ---- The sidebar area's quick switcher (32), on sessions of its own ----------------------------------------------

    def test_32_the_quick_switcher_opens_a_session_found_by_typing(self) -> None:
        """The quick switcher is the keyboard's way to any session, and it crosses the app, the helm and back: the
        chord reaches the app from a focused terminal, the switcher reads the helm's listing over the UI channel, and a
        pick goes through the ordinary selection, which the helm remembers. Specified: with this step's own origin
        session open, its terminal focused and showing the echo of a stamped line typed into it, the platform's chord
        opens the switcher ("Recent sessions"); filling its search field with part of this step's target title and
        pressing Enter closes it, the helm records the target as the last selected session, and the focused terminal
        is the target's: the fake agent's ready line without the origin's echo. Both sessions are this step's own,
        titled with a per-run stamp so no other step's session can match."""
        stamp = uuid.uuid4().hex[:8]
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        titles = {role: f"Switcher {role} {stamp}" for role in ("origin", "target")}
        for title in titles.values():
            body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures)
            body["title"] = title
            gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", "--text", titles["target"])
        ids = {
            role: eventually(lambda t=title: listed_id(t), f"{title} in the helm's listing")
            for role, title in titles.items()
        }
        self.ok("click", f"text:{titles['origin']}")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        # Marks the origin's screen, so the terminal focused after the pick can be told apart from it: both sessions
        # run the same script and show the same ready line.
        line = f"origin-{stamp}"
        self.ok("type", "--keys", line)
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", f"echo:{line}")
        eventually(lambda: last_selected() == ids["origin"], "premise: the helm remembering the origin as selected")
        self.ok("press", "cmd-k" if sys.platform == "darwin" else "ctrl-shift-k")
        self.ok("wait", "--text", "Recent sessions")
        self.ok("fill", "text:Search sessions", f"target {stamp}")
        self.ok("press", "enter")
        self.ok("wait", "--gone", SWITCHER_FOOTER)
        eventually(lambda: last_selected() == ids["target"], "the helm remembering the picked target as selected")
        screen = self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        self.assertNotIn(f"echo:{line}", screen, "the focused terminal is the target's, not the origin's")

    # ---- The terminal area's scrollback (33), on a session of its own ------------------------------------------------

    def test_33_the_wheel_scrolls_history_and_typing_returns(self) -> None:
        """Scrollback is how the user reads what scrolled off (SPEC.md, "Terminal experience": scrollback stays
        viewable), and it is the terminal's own: on the ordinary screen the wheel moves the view into history and the
        program hears nothing, and the next key the user types brings the view back to the live screen. Specified: in
        a session of this step's own that printed 120 numbered lines and waits, the oldest line is off screen at
        first, the wheel over the terminal brings it into view, and a typed key shows the newest line again with the
        oldest gone."""
        record = json.loads((RUN / "run.json").read_text())
        work = RUN.parent / "scroll"
        work.mkdir()
        transcript = work / "transcript.txt"
        transcript.write_text("".join(f"line-{i:03}\n" for i in range(120)))
        title = f"Scrollback {uuid.uuid4().hex[:8]}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, record["binaries"]["fixtures"], "idle", transcript)
        body["title"] = title
        gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", "--text", title)
        self.ok("click", f"text:{title}")
        screen = self.ok("terminal-text", "--wait", "line-119")
        self.assertNotIn("line-000", screen, "premise: the oldest line has scrolled off")
        self.ok("scroll", "up", "5000", "--on", "id:terminal-grid")
        self.assertIn("line-000", self.ok("terminal-text"), "the wheel scrolled back to the oldest line")
        self.ok("type", "--keys", "x")
        screen = self.ok("terminal-text", "--wait", "line-119")
        self.assertNotIn("line-000", screen, "typing returned to the live screen")

    # ---- The session list's lifecycle step (34), on sessions of its own ---------------------------------------------

    def test_34_stop_and_replace_from_the_row_menu(self) -> None:
        """Stop and Replace are the row menu's lifecycle commands, each a request through the operation store to a
        helm route that reaches the supervisor (Replace creates a session and deletes another). Specified, on two
        basic fake-agent sessions this step creates under titles of its own: the menu's Stop on a session with no
        terminal tabs asks nothing and the helm then lists that session as exited; the menu's Replace asks, and once
        Confirm replace is clicked the helm lists a session under the same title with a new id, and no longer the old
        one. The helm's listing is the oracle for both, since the window's rows show either outcome only indirectly
        (a stopped row keeps its title, and a replacement has the same title as its source)."""
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        tag = uuid.uuid4().hex[:8]
        stopped, replaced = f"Lifecycle stop {tag}", f"Lifecycle replace {tag}"
        ids = {}
        for index, title in enumerate((stopped, replaced), start=1):
            body = gui_agent.seed_body(index, gui_agent.SESSION_CWD, fixtures)
            body["title"] = title
            ids[title] = gui_agent.helm_request(RUN, "POST", "/api/sessions", body)["id"]
        for title in (stopped, replaced):
            self.ok("wait", "--text", title)
        self.assertNotEqual(
            gui_agent.listed_states(RUN).get(ids[stopped]), "exited", "premise: the session to stop is not stopped"
        )

        self.ok("click", f"text:Session actions for {stopped}")
        self.ok("wait", "--text", MENU_DESCRIPTION)
        self.ok("click", "id:session-menu-stop")
        eventually(lambda: gui_agent.listed_states(RUN).get(ids[stopped]) == "exited", f"{stopped} listed as exited")

        self.ok("click", f"text:Session actions for {replaced}")
        self.ok("wait", "--text", MENU_DESCRIPTION)
        self.ok("click", "id:session-menu-replace")
        self.ok("wait", "--text", "Confirm replace")
        self.ok("click", "text:Confirm replace")

        def replacement() -> bool:
            titles = listed_titles()
            fresh = [id for id, title in titles.items() if title == replaced and id != ids[replaced]]
            return ids[replaced] not in titles and len(fresh) == 1

        eventually(replacement, f"{replaced} listed under a new id and not under its old one")

    # ---- The sidebar area's host rail (35), on a session of its own --------------------------------------------------

    def test_35_choosing_the_local_host_narrows_the_list_and_its_chip_clears_it(self) -> None:
        """Choosing a host on the rail is a query the helm serves over the UI channel (`PUT /ui/session-query`), not a
        filter the window applies, so only the real helm shows that the query took effect: it then counts the matches
        against the whole fleet, and the list's count line reads "N of M" only for a narrowed listing. Specified: with
        the helm's local host its only host and a session of this step's own listed, a click on the local host's avatar
        draws the chip named for the local host, the count line reads "N of N" (N the sessions the helm lists, all on
        that host) and this step's session is still listed; a click on the chip takes the chip away, the count line
        back to the plain count, and focus to the rail's All. The step leaves All chosen, as it found it, so no other
        step sees a narrowed list."""
        # The premise first, before this step changes anything. With another host registered, the first host listed
        # need not be the local one this step clicks, and if that host carried sessions, "N of N" would no longer hold;
        # a step that left a host behind fails here, naming what the helm lists, rather than later on a count.
        hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
        self.assertEqual(
            [host["kind"] for host in hosts],
            ["local"],
            f"premise: the local host is the helm's only host (did an earlier step leave one?); the helm lists {hosts}",
        )
        title = f"Rail {uuid.uuid4().hex[:8]}"
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures)
        body["title"] = title
        gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", "--text", title)
        self.assertNotIn(LOCAL_CHIP, self.ok("snapshot"), "premise: All is chosen")
        total = len(listed_titles())
        self.ok("click", f"id:host-rail-host-{hosts[0]['id']}")
        eventually(lambda: LOCAL_CHIP in self.ok("snapshot"), "the chip naming the local host")
        self.ok("wait", "--text", f"{total} of {total}")
        self.ok("wait", "--text", title)
        self.ok("click", "id:host-filter-chip")
        eventually(lambda: LOCAL_CHIP not in self.ok("snapshot"), "the chip going once All is chosen")
        self.ok("wait", "--gone", f"{total} of {total}")
        self.assertRegex(self.ok("snapshot", "-i"), r'button "All hosts" \[focused\]', "focus is on All")

    # ---- The session list's seen toggle (36), on a session of its own ------------------------------------------------

    def test_36_mark_read_and_unread_from_the_row_menu(self) -> None:
        """Mark read and Mark unread set or clear the session's seen stamp, which the helm keeps and shares with every
        client (SPEC.md, "Status"), through a request of the operation store to the helm's seen route. Specified, on a
        basic fake-agent session this step creates under a title of its own and leaves unopened, once the supervisor
        reads it idle (so its activity stamp no longer moves): the helm lists it with no seen stamp, so its menu offers
        Mark read ("Treat the latest output as seen"), which makes the helm list a stamp; the menu then offers Mark
        unread ("Show new output again"), which makes the helm list none again, and Mark read once more brings the
        stamp back. The helm's listing is the oracle. The item is clicked by its label, which no row's status dot shares
        (a dot's name carries its session's title)."""
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        title = f"Seen toggle {uuid.uuid4().hex[:8]}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures)
        body["title"] = title
        session = gui_agent.helm_request(RUN, "POST", "/api/sessions", body)["id"]
        self.ok("wait", "--text", title)
        eventually(lambda: gui_agent.listed_states(RUN).get(session) == "idle", f"{title} listed as idle")

        def stamp() -> int | None:
            rows = gui_agent.helm_request(RUN, "GET", "/api/sessions")["sessions"]
            [row] = [row for row in rows if row["id"] == session]
            return row["seen_activity_at"]

        self.assertIsNone(stamp(), "premise: no client has opened the session, so it has no seen stamp")
        # Each choice: the item's label, its description (which only the open menu draws), and whether the helm then
        # lists a seen stamp.
        for label, description, stamped in (
            ("Mark read", "Treat the latest output as seen", True),
            ("Mark unread", "Show new output again", False),
            ("Mark read", "Treat the latest output as seen", True),
        ):
            self.ok("click", f"text:Session actions for {title}")
            self.ok("wait", "--text", description)
            self.ok("click", f"text:{label}")
            self.ok("wait", "--gone", description)
            eventually(
                lambda stamped=stamped: (stamp() is not None) == stamped,
                f"the helm listing {title} {'with' if stamped else 'without'} a seen stamp",
            )

    # ---- The sidebar area's resize divider (37), on no session of its own --------------------------------------------

    def test_37_the_focused_sidebar_divider_steps_the_helms_width(self) -> None:
        """The sidebar's width is a helm preference shared by every client, so the divider's step is only real once the
        helm stores it: the in-process tests prove the window sends the write, and only the real helm shows that it
        accepts the width and keeps it. Specified: with the divider focused by a click on its grip (a separator takes
        focus on every press, on every platform), Right makes the helm store the width 10 pixels wider than before,
        focus stays on the divider, and Left makes it store the starting width again. The step leaves the default width
        stored when it found none, so later steps see the default sidebar."""
        before = sidebar_width()
        start = before if before is not None else 396
        # The press lands on the divider's grip, which lies over its strip and takes the pointer for it.
        self.ok("click", "id:sidebar-divider-grip")
        self.assertRegex(self.ok("snapshot", "-i"), DIVIDER_FOCUSED, "premise: the divider holds focus")
        self.ok("press", "right")
        eventually(lambda: sidebar_width() == start + 10, f"the helm storing the width {start + 10}")
        self.assertRegex(self.ok("snapshot", "-i"), DIVIDER_FOCUSED, "focus stays on the divider")
        self.ok("press", "left")
        eventually(lambda: sidebar_width() == start, f"the helm storing the width {start} again")

    # ---- The session view's header (38), on a session of its own ---------------------------------------------------

    def test_38_restart_from_the_header_resumes_the_session(self) -> None:
        """Restart is the open session's header action that only the header offers, and the one request whose route
        relaunches an agent in a new terminal. It is offered only for a session whose conversation Farhelm captured, so
        this step's session is a command launch declaring Muse (whose capture needs no record on disk) running the fake
        agent's hook-reporting script, which reports a conversation through the genuine hook the supervisor injected
        when the step types `report <id>` into it. Specified, on that session under a title of its own: once the helm
        lists it as resumable and then idle or waiting, a click on the header's Restart (answering its question if the
        window's row still read the agent working) relaunches it, the helm lists it live again, and its terminal shows
        the fake agent's `MUSE-RESUMED:<id>` marker, which only a resumed launch prints. The marker is the oracle that a
        restart happened at all, since the session's status reads live before and after."""
        record = json.loads((RUN / "run.json").read_text())
        fixtures = record["binaries"]["fixtures"]
        stamp = uuid.uuid4().hex[:8]
        home = RUN.parent / "muse-home"
        home.mkdir()
        invocation = (
            f"{shlex.quote(fixtures)} fake-agent --script muse-hook-report --record-home {shlex.quote(str(home))}"
        )
        title = f"Header restart {stamp}"
        body = {
            "cwd": str(gui_agent.SESSION_CWD),
            "title": title,
            "command": {
                "command": f"{invocation} {{farhelm_args}}",
                "yolo": False,
                "agent": "muse",
                "resume": f"{invocation} resume {{conversation}} {{farhelm_args}}",
            },
        }
        session = gui_agent.helm_request(RUN, "POST", "/api/sessions", body)["id"]
        self.ok("wait", "--text", title)
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        conversation = f"smoke-{stamp}"
        self.ok("type", "--keys", f"report {conversation}")
        self.ok("press", "enter")

        def listed() -> dict:
            rows = gui_agent.helm_request(RUN, "GET", "/api/sessions")["sessions"]
            return next((row for row in rows if row["id"] == session), {})

        eventually(lambda: listed().get("restart_offer") == "resume", f"{title} listed as resumable")
        # Restart asks only while the agent reads working, and the window decides that from its own pushed row at the
        # click, not from anything this step read. The fake agent reads working while it prints and idle once it stops,
        # so the step waits for the helm to list it idle or waiting first. The window's row follows that by a push, so
        # the click can still meet a question from the row before it; the step then waits for whichever the window
        # shows, the question (answered) or the resumed launch's marker, rather than guessing from a read.
        quiet = {"idle", "waiting"}
        eventually(lambda: (listed().get("status") or {}).get("state") in quiet, f"{title} listed idle or waiting")
        self.ok("click", "id:session-header-restart")
        marker = f"MUSE-RESUMED:{conversation}"

        def outcome() -> str | None:
            if marker in agent("terminal-text").stdout:
                return "resumed"
            return "asked" if "Confirm restart" in self.ok("snapshot") else None

        if eventually(outcome, "the restart's question or its resumed launch") == "asked":
            self.ok("click", "text:Confirm restart")
        self.ok("terminal-text", "--wait", marker)
        live = {"running", "waiting", "idle"}
        eventually(lambda: (listed().get("status") or {}).get("state") in live, f"{title} listed live again")

    # ---- The sidebar area's rail foot (39): Hosts and the Settings gear ----------------------------------------------

    def test_39_the_rail_foot_toggles_hosts_and_opens_settings(self) -> None:
        """The host rail's foot holds the app's ways into host management and Settings, and both hand focus back the way
        SPEC.md says: Hosts "keeps focus" as it swaps the session list for host management, and "Escape and either close
        control return focus to the gear", which "always opens sessions and focuses its first switch". The step seeds
        and touches no session: both are the helm's, not a session's. Specified: a click on Hosts shows the panel named
        "Host management" with Hosts focused, and a second click hides it with Hosts still focused; then, with Settings
        not open, a click on the gear opens it with its first switch, Skip session confirmations, focused, and Escape
        closes it with focus on the gear. The panel's name and the dialog's summary are the oracles for shown and gone,
        since only the shown panel and the open dialog draw them."""
        self.assertNotIn(HOSTS_PANEL, self.ok("snapshot"), "premise: Sessions shows")
        self.ok("click", "id:hosts-panel-toggle")
        eventually(lambda: HOSTS_PANEL in self.ok("snapshot"), "the Hosts panel showing")
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Hosts" \[focused\]', "focus stays on Hosts")
        self.ok("click", "id:hosts-panel-toggle")
        eventually(lambda: HOSTS_PANEL not in self.ok("snapshot"), "the Hosts panel going")
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Hosts" \[focused\]', "focus is still on Hosts")

        self.assertNotIn(SETTINGS_SUMMARY, self.ok("snapshot"), "premise: Settings is not open")
        self.ok("click", "id:app-settings-toggle")
        self.ok("wait", "--text", SETTINGS_SUMMARY)
        self.assertRegex(
            self.ok("snapshot", "-i"),
            r'switch "Skip session confirmations" \[focused\]',
            "focus is on Settings' first switch",
        )
        self.ok("press", "escape")
        self.ok("wait", "--gone", SETTINGS_SUMMARY)
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Settings" \[focused\]', "focus is back on the gear")

    # ---- The settings-and-hosts area's scripted host setup (40), on a host of its own --------------------------------

    def test_40_the_scripted_backend_sets_up_a_fake_ssh_host(self) -> None:
        """Every host flow the settings-and-hosts area ports (Add host, setup and its failures, Update) is driven end to
        end against fake SSH hosts that the helm's scripted provisioning backend simulates (`start --scripted-hosts`),
        because nothing else can make a host fail at a chosen step or race on cue. This step proves the seam in the real
        app before any of those screens exist: through the helm's own API, probing a destination nobody has added
        offers a setup plan, confirming it starts a run that the scripted backend carries to completion, and the
        backend's event log shows the helm used it rather than real ssh. Specified: the probe's result is
        `provisionable`, the confirmation is accepted for a new host entry the helm lists under that destination, and
        that host's run completes, having run a setup step (not just the probe) through the backend. The fake host it
        adds is removed again once its run has ended, so no later step finds a host it did not seed; when the step
        fails with the run still going (the helm refuses to remove a host while its run holds it), the step's own
        failure is what it reports, and the host left registered is named on stderr."""
        destination = f"scripted-{uuid.uuid4().hex[:8]}.example"
        probe = gui_agent.helm_request(
            RUN, "POST", "/api/hosts/probe", {"target": {"kind": "ssh", "destination": destination}, "new_entry": {}}
        )
        self.assertEqual(probe["result"], "provisionable", probe)
        accepted = gui_agent.helm_request(RUN, "POST", "/api/hosts/provision", {"probe_id": probe["probe_id"]})
        host, run_id = accepted["host_id"], accepted["run_id"]

        def settled() -> dict | None:
            """This run's progress once it has finished, either way. "completed" alone is also what the helm reports
            for a host with no run at all, so the run id is matched too."""
            view = gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning")
            if view.get("run_id") == run_id and view.get("status") in ("completed", "failed"):
                return view
            return None

        try:
            hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
            listed = {row["id"]: row.get("destination") for row in hosts}
            self.assertEqual(
                listed.get(host), destination, "the confirmation registered a new entry for the destination"
            )

            view = eventually(settled, f"the scripted setup of {destination} finishing")
            self.assertEqual(view["status"], "completed", f"the setup failed: {view.get('message')}")
            log = (gui_agent.scripted_hosts_dir(RUN) / "events.jsonl").read_text()
            events = [json.loads(line) for line in log.splitlines()]
            steps = {event["event"] for event in events if event["target"] == f"ssh:{destination}"}
            self.assertIn("install-farhelm", steps, "the helm ran the setup's steps through the scripted backend")
        finally:
            # Whether the step is already failing: then its own error is the report, and the cleanup must not replace
            # it with an error of its own.
            failing = sys.exc_info()[0] is not None
            try:
                # The helm answers 409 to a removal while the host's run holds it, so wait (bounded) for the run to
                # end first. The step's own wait above has already done this when the step got that far.
                eventually(settled, f"the scripted setup of {destination} ending before its host is removed")
                gui_agent.helm_request(RUN, "DELETE", f"/api/hosts/{host}")
            except (AssertionError, urllib.error.HTTPError) as error:
                if not failing:
                    raise
                print(f"gui-e2e-smoke: step 40 left host {host} ({destination}) registered: {error}", file=sys.stderr)

    # ---- The sidebar area's rail Add host (41) ----------------------------------------------------------------------

    def test_41_the_rails_add_host_opens_its_dialog_and_escape_returns_to_it(self) -> None:
        """The host rail's Add host is the app's way into adding a host, and closing the dialog it opens hands focus
        back the way SPEC.md says ("Closing something the user opened returns focus to where it was before it
        opened"). The step seeds and touches no session, types nothing and so sends no probe, leaving the host registry
        as it found it. Specified: with the dialog not open, a click on Add host opens it, and Escape closes it with
        focus on Add host. The dialog's summary is the oracle for open and closed, since only the open dialog draws
        it."""
        self.assertNotIn(ADD_HOST_SUMMARY, self.ok("snapshot"), "premise: the Add host dialog is not open")
        self.ok("click", "id:host-rail-add")
        self.ok("wait", "--text", ADD_HOST_SUMMARY)
        self.ok("press", "escape")
        self.ok("wait", "--gone", ADD_HOST_SUMMARY)
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Add host" \[focused\]', "focus is back on Add host")

    # ---- The settings-and-hosts area's setup progress, failure and rerun (43), on a host of its own -----------------

    def test_43_a_fake_hosts_setup_shows_its_progress_and_failure_and_reruns(self) -> None:
        """A host's setup takes a minute and can fail at any step, and the Hosts panel is where the user watches it and
        tries again (SPEC.md, "Topology": the running row "expands once to show the helm's steps", "failure opens it for
        the step and its reason"); the drive of the first Add host port found neither the progress nor the failure's
        reason shown. This step sets a fake host up through the app's own Add host and setup question, holding the
        scripted backend at the first step so the run is observed in progress, then lets it fail at installing the
        binary, then reruns it from the row's menu with the backend succeeding. Specified: with Hosts shown and "Set up
        new hosts without asking" off, Add host for a fresh `.example` destination asks "Set up host · <destination>",
        and Set up starts a run whose row reads "Setting up: 0 of ..." with "Create directories: Running"; released, the
        run fails, and the row shows "Setup provisioning Failed", a step marked Failed and the backend's reason "disk
        full"; the row's menu offers Re-run, which asks again, and Set up starts a second run that completes. The fake
        host is removed again once its last run has ended, the backend's script is emptied and Hosts is hidden, whatever
        happened; the snapshot's names of the question, the status and the steps are the oracles."""
        destination = f"setup-{uuid.uuid4().hex[:8]}.example"
        held = {"hold_actions": True, "fail_action": "install-farhelm", "message": "disk full"}
        gui_agent.script_hosts(RUN, {"targets": {f"ssh:{destination}": held}})
        # No step turns the preference on; the step relies on the question, so it checks that rather than writing it.
        self.assertNotEqual(
            gui_agent.helm_request(RUN, "GET", "/api/preferences").get("skip_host_setup_confirmation"),
            True,
            "premise: setup asks first",
        )
        host = None
        try:
            if HOSTS_PANEL not in self.ok("snapshot"):
                self.ok("click", "id:hosts-panel-toggle")
            eventually(lambda: HOSTS_PANEL in self.ok("snapshot"), "the Hosts panel showing")
            question = f"Set up host · {destination}"
            self.ok("click", "id:host-rail-add")
            self.ok("wait", "--text", ADD_HOST_SUMMARY)
            self.ok("fill", "id:add-host-destination", destination)
            self.ok("click", "id:add-host-submit")
            eventually(lambda: question in self.ok("snapshot"), "the setup question opening")
            self.ok("click", "id:prompt-confirm")

            def registered() -> int | None:
                """The registry id the helm filed the destination under, once it has."""
                hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
                return next((row["id"] for row in hosts if row.get("destination") == destination), None)

            host = eventually(registered, f"{destination} being registered")

            def showing(*texts: str):
                """A check for `eventually`: the snapshot, once it names every one of `texts`."""

                def check() -> str | None:
                    shot = self.ok("snapshot")
                    return shot if all(text in shot for text in texts) else None

                return check

            eventually(
                showing("Setting up: 0 of", "Create directories: Running"),
                "the row showing the held run's progress",
            )

            gui_agent.script_hosts(RUN, {"targets": {f"ssh:{destination}": {**held, "hold_actions": False}}})
            failed = eventually(showing("Setup provisioning Failed"), "the row showing the failed run")
            self.assertRegex(failed, r": Failed", "a step is marked Failed")
            self.assertIn("disk full", failed, "the backend's reason shows")

            gui_agent.script_hosts(RUN, {})
            first = gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning")["run_id"]
            self.ok("click", f"id:host-menu-toggle-{host}")
            self.ok("click", "id:host-menu-rerun")
            eventually(lambda: question in self.ok("snapshot"), "the rerun's setup question opening")
            self.ok("click", "id:prompt-confirm")

            def rerun_completed() -> bool:
                """Whether a run other than the failed one has completed on the host."""
                view = gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning")
                return view.get("run_id") not in (None, first) and view.get("status") == "completed"

            eventually(rerun_completed, f"the rerun of {destination}'s setup completing")
            # The row's side of the success: the inline status gives way to the phase words and the row folds.
            def folded() -> bool:
                """Whether the row shows neither the inline status nor the run's steps any more."""
                shot = self.ok("snapshot")
                return "Setting up" not in shot and "Setup provisioning" not in shot

            eventually(folded, "the row folding back to its phase words after the rerun")
        finally:
            failing = sys.exc_info()[0] is not None
            try:
                gui_agent.script_hosts(RUN, {})
                if host is not None:

                    def settled() -> bool:
                        """Whether the host has no run going on, so the helm lets it be removed."""
                        view = gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning")
                        return view.get("status") != "running"

                    eventually(settled, f"{destination}'s run ending before its host is removed")
                    gui_agent.helm_request(RUN, "DELETE", f"/api/hosts/{host}")
                agent("press", "escape")
                if HOSTS_PANEL in self.ok("snapshot"):
                    self.ok("click", "id:hosts-panel-toggle")
            except (AssertionError, urllib.error.HTTPError) as error:
                if not failing:
                    raise
                print(f"gui-e2e-smoke: step 43 left host {host} ({destination}) behind: {error}", file=sys.stderr)


    # ---- The session list's status segments (42), on sessions of its own ---------------------------------------------

    def test_42_the_status_segments_narrow_the_list_to_one_status(self) -> None:
        """The status segments are a query the helm serves over the UI channel (`PUT /ui/session-query`, with the
        status beside the rail's host), so only the real helm shows that a status took effect: it lists only the
        sessions its supervisors read in that status and counts them against the whole fleet. Specified, on a replayed
        fake agent this step seeds to hold a running screen and another to hold an idle one, both under titles of its
        own, once the helm lists each in its status: a click on Running lists the running one and not the idle one, with
        the count line "N of M" (N the sessions the helm lists as running, M all it lists); a click on Idle lists the
        idle one and not the running one, with the idle count; a click on All lists both again and the plain count. The
        counts are read from the helm's listing each time they are checked, since other steps' basic sessions may still
        move between running and idle. The step leaves All chosen, as it found it."""
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        tag = uuid.uuid4().hex[:8]
        work = RUN / "work"
        work.mkdir(exist_ok=True)
        ids, titles = {}, {}
        for index, kind in enumerate(("running", "idle"), start=1):
            transcript = work / f"status-{tag}-{kind}.txt"
            transcript.write_text(gui_agent.transcript(index, kind))
            body = gui_agent.seed_body(index, gui_agent.SESSION_CWD, fixtures, kind, transcript)
            body["title"] = titles[kind] = f"Status {kind} {tag}"
            ids[kind] = gui_agent.helm_request(RUN, "POST", "/api/sessions", body)["id"]
        gui_agent.wait_for_statuses(lambda: gui_agent.listed_states(RUN), {ids[kind]: kind for kind in ids})
        for title in titles.values():
            self.ok("wait", "--text", title)

        def shows(status: str | None, listed: list[str], hidden: list[str]):
            """A check that the window lists `listed`, not `hidden`, and counts what the helm lists in `status`
            against the whole fleet ("N of M"), or, for `None` (All), the plain total and no "N of M" count. The count
            is read from its accessible name ("Sessions: N of M"), since the snapshot nests the count's painted text
            under the named mark that draws it, and matched whole, quotes included, so "1 of 9" never matches "11 of
            9"."""

            def check() -> bool:
                states = gui_agent.listed_states(RUN)
                snapshot = self.ok("snapshot")
                if status is None:
                    counted = (
                        re.search(r'"Sessions: \d+ of \d+"', snapshot) is None
                        and f'"Sessions: {len(states)}"' in snapshot
                    )
                else:
                    matching = sum(1 for state in states.values() if state == status)
                    counted = f'"Sessions: {matching} of {len(states)}"' in snapshot
                return (
                    counted
                    and all(title in snapshot for title in listed)
                    and not any(title in snapshot for title in hidden)
                )

            return check

        running, idle = titles["running"], titles["idle"]
        self.ok("click", "id:session-status-Running")
        eventually(shows("running", [running], [idle]), "Running listing the running session and its count")
        self.ok("click", "id:session-status-Idle")
        eventually(shows("idle", [idle], [running]), "Idle listing the idle session and its count")
        self.ok("click", "id:session-status-All")
        eventually(shows(None, [running, idle], []), "All listing both sessions and the plain count")

    # ---- The settings-and-hosts area's Remove host (44), on hosts of its own ----------------------------------------

    def test_44_the_host_menus_remove_forgets_fake_hosts(self) -> None:
        """Removing a host is the one host action that only forgets an entry (SPEC.md, "Topology"), and its question,
        its permanent answer and removal without asking each lead the real app to the helm's own route. The step adds
        three fake SSH hosts of its own through the helm's API (made-up `.example` destinations, which the helm lists
        as unreachable), touches no session, and leaves "Remove hosts without asking" as it found it. Specified: once
        the Hosts panel draws all three rows, with the preference off, a host's menu Remove opens the question and
        Cancel closes it with the host still listed and focus on its row's menu toggle; Remove on the question makes the
        helm stop listing it; "Remove, don't ask again" makes the helm stop listing the second and store the preference
        on; and then the third's menu Remove opens no question and the helm stops listing it. The question's explanation
        is the oracle for open and closed, since only the open question draws it. Hosts the step added and did not
        remove are removed again at its end."""
        tag = uuid.uuid4().hex[:8]
        names = [f"remove-{n}-{tag}.example" for n in (1, 2, 3)]
        ids = [gui_agent.helm_request(RUN, "POST", "/api/hosts", {"ssh": name})["id"] for name in names]
        previous = gui_agent.helm_request(RUN, "GET", "/api/preferences").get("skip_host_remove_confirmation")

        def listed() -> set[int]:
            """The ids of every host the helm lists."""
            return {row["id"] for row in gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]}

        def remove_from_menu(host: int) -> None:
            """Open `host`'s row menu from its toggle and choose Remove."""
            self.ok("click", f"id:host-menu-toggle-{host}")
            self.ok("click", "id:host-menu-remove")

        try:
            gui_agent.helm_request(RUN, "PUT", "/api/preferences", {"skip_host_remove_confirmation": False})
            if HOSTS_PANEL not in self.ok("snapshot"):
                self.ok("click", "id:hosts-panel-toggle")
                eventually(lambda: HOSTS_PANEL in self.ok("snapshot"), "the Hosts panel showing")
            # The hosts reach the window through a push, so the premise is that all three rows' toggles are drawn.
            eventually(
                lambda: all(f'"Host actions for {name}"' in self.ok("snapshot", "-i") for name in names),
                "the three hosts' rows drawn",
            )
            first, second, third = ids

            remove_from_menu(first)
            self.ok("wait", "--text", REMOVE_EXPLANATION)
            self.ok("click", "id:prompt-cancel")
            self.ok("wait", "--gone", REMOVE_EXPLANATION)
            self.assertIn(first, listed(), "Cancel kept the host")
            self.assertRegex(
                self.ok("snapshot", "-i"),
                rf'button "Host actions for {re.escape(names[0])}" \[focused\]',
                "focus is back on the row's menu toggle",
            )

            remove_from_menu(first)
            self.ok("wait", "--text", REMOVE_EXPLANATION)
            self.ok("click", "id:prompt-confirm")
            self.ok("wait", "--gone", REMOVE_EXPLANATION)
            eventually(lambda: first not in listed(), "the helm forgetting the first host")

            remove_from_menu(second)
            self.ok("wait", "--text", REMOVE_EXPLANATION)
            self.ok("click", "id:prompt-alternative-0")
            self.ok("wait", "--gone", REMOVE_EXPLANATION)
            eventually(lambda: second not in listed(), "the helm forgetting the second host")
            eventually(
                lambda: gui_agent.helm_request(RUN, "GET", "/api/preferences").get("skip_host_remove_confirmation"),
                "the helm storing Remove hosts without asking",
            )

            remove_from_menu(third)
            eventually(lambda: third not in listed(), "the helm forgetting the third host without a question")
            self.assertNotIn(REMOVE_EXPLANATION, self.ok("snapshot"), "no question opened")
        finally:
            # Whether the step is already failing: then its own error is the report, and the cleanup must not replace
            # it with an error of its own.
            failing = sys.exc_info()[0] is not None
            try:
                gui_agent.helm_request(RUN, "PUT", "/api/preferences", {"skip_host_remove_confirmation": previous})
                for host in set(ids) & listed():
                    gui_agent.helm_request(RUN, "DELETE", f"/api/hosts/{host}")
                # Escape only when this step left its question or a host menu open: otherwise focus is in the selected
                # session's terminal, which belongs to another area's step, and Escape would reach its program.
                snapshot = self.ok("snapshot")
                if REMOVE_EXPLANATION in snapshot or "Forget this host" in snapshot:
                    agent("press", "escape")
                if HOSTS_PANEL in self.ok("snapshot"):
                    self.ok("click", "id:hosts-panel-toggle")
            except (AssertionError, urllib.error.HTTPError) as error:
                if not failing:
                    raise
                print(f"gui-e2e-smoke: step 44's cleanup failed: {error}", file=sys.stderr)

    # ---- The other-dialogs area's Help menu (45) --------------------------------------------------------------------

    def test_45_the_help_menu_opens_ssh_help_and_returns_to_the_button(self) -> None:
        """The rail foot's "?" button is the app's way into help, and SPEC.md's "SSH needs 2FA?" item opens SSH help,
        which, like every dialog the menu opens, hands focus back to the "?" button when it closes (the old app returned
        focus to its Help toggle). The step seeds and touches no session and opens no link, so nothing leaves the
        machine. Specified: a click on the "?" button opens the menu with focus on its first item, Send feedback;
        choosing SSH needs 2FA? closes the menu and opens SSH help; Escape closes SSH help with focus on the "?" button.
        The menu's summary and then the dialog's are the oracles for open and closed."""
        self.assertNotIn(HELP_MENU_SUMMARY, self.ok("snapshot"), "premise: the Help menu is not open")
        self.ok("click", "id:app-help-toggle")
        self.ok("wait", "--text", HELP_MENU_SUMMARY)
        self.assertRegex(
            self.ok("snapshot", "-i"), r'menuitem "Send feedback" \[focused\]', "focus starts on the first item"
        )
        self.ok("click", "id:help-menu-ssh-help")
        self.ok("wait", "--gone", HELP_MENU_SUMMARY)
        self.ok("wait", "--text", SSH_HELP_SUMMARY)
        self.ok("press", "escape")
        self.ok("wait", "--gone", SSH_HELP_SUMMARY)
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Help" \[focused\]', "focus is back on the ? button")

    # ---- The session list's order menu (46), on sessions of its own --------------------------------------------------

    def test_46_the_order_menu_reorders_the_list_by_creation_and_by_title(self) -> None:
        """Choosing an order is a write of the shared preference (`PUT /api/preferences`, `list_sort`), and the helm
        re-sorts the list it pushes; the window never sorts. So only the real helm shows that a choice took effect: it
        stores the word and serves the list in that order. Specified, on three basic sessions this step seeds with
        titles of its own (created in the order b, c, a, and titled so that alphabetical order is a, b, c), after a line
        typed, once all three are idle, into the one the helm lists last by creation makes it the most recently active,
        so that the helm's activity order of the three differs from its creation order (checked as the premise, with the
        window drawing the activity order): choosing Newest created from the title's menu makes the helm store
        `created`, the title name the order with focus on it, and the window show the three in the order the helm itself
        lists them by creation; choosing Title A–Z does the same for `title`, in the helm's title order, which differs
        from the creation order by the titles. The window's order is read from where each title first appears in the
        bridge's snapshot, which follows drawing order, top to bottom; their titles start with "A order" so that the
        title order draws them near the top of a list of many sessions. The step chooses Recently active again before it
        ends, which stores `activity`, the order every step before it saw."""
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Sort: Recently active"', "premise: Recently active")
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        tag = uuid.uuid4().hex[:8]
        titles = [f"A order {tag} {letter}" for letter in ("b", "c", "a")]
        ids = []
        for index, title in enumerate(titles):
            body = gui_agent.seed_body(index, gui_agent.SESSION_CWD, fixtures)
            body["title"] = title
            ids.append(gui_agent.helm_request(RUN, "POST", "/api/sessions", body)["id"])
        for title in titles:
            self.ok("wait", "--text", title)

        def stored() -> str | None:
            return gui_agent.helm_request(RUN, "GET", "/api/preferences").get("list_sort")

        def helm_order(word: str) -> list[str]:
            """This step's three titles in the order the helm lists them under `word` (`?sort=<word>`, its own sort)."""
            listed = gui_agent.helm_request(RUN, "GET", f"/api/sessions?sort={word}")["sessions"]
            return [row["title"] for row in listed if row["title"] in titles]

        def shows(word: str):
            """A check that the window draws this step's three sessions in the helm's `word` order, all three drawn."""

            def check() -> bool:
                expected = helm_order(word)
                snapshot = self.ok("snapshot")
                if len(expected) != len(titles) or not all(title in snapshot for title in titles):
                    return False
                return sorted(titles, key=snapshot.index) == expected

            return check

        # The premise that makes the Newest created half able to fail: three sessions seeded back to back can list in
        # the same order by activity and by creation, and then a window that never re-sorted would pass. Work in the
        # session the helm lists last by creation moves it to the front by activity while it stays last by creation.
        # That session is taken from the helm's own creation order, not from the order the step seeded them in: seeds
        # made within one second share a creation stamp, and the helm breaks that tie by id, so the first seeded need
        # not list last. Only a move from idle into running starts a work burst (SPEC.md, "Session list"), so the line
        # is typed once the supervisor reads all three idle.
        gui_agent.wait_for_statuses(lambda: gui_agent.listed_states(RUN), {id: "idle" for id in ids})
        oldest = helm_order("created")[-1]
        oldest_id = ids[titles.index(oldest)]
        self.ok("click", f"text:{oldest}")
        eventually(lambda: last_selected() == oldest_id, "premise: the helm remembering the session as selected")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        line = f"order-{tag}"
        self.ok("type", "--keys", line)
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", f"echo:{line}")
        orders = {}

        def orders_differ() -> bool:
            orders["activity"], orders["created"] = helm_order("activity"), helm_order("created")
            return orders["activity"] != orders["created"]

        try:
            eventually(orders_differ, "the helm's activity order of the sessions differing from their creation order")
        except AssertionError as failure:
            raise AssertionError(
                f"{failure}: by activity {orders.get('activity')}, by creation {orders.get('created')}"
            ) from failure
        eventually(shows("activity"), "the window drawing the sessions in the helm's activity order")

        for word, label, item in (
            ("created", "Newest created", "id:session-order-created"),
            ("title", "Title A–Z", "id:session-order-title"),
        ):
            self.ok("click", "id:session-order")
            self.ok("wait", "--text", "Order and density, shared by every client")
            self.ok("click", item)
            self.ok("wait", "--gone", "Order and density, shared by every client")
            eventually(lambda: stored() == word, f"the helm storing {word}")
            self.assertRegex(self.ok("snapshot", "-i"), rf'button "Sort: {label}" \[focused\]', "the title names it")
            eventually(shows(word), f"the window drawing the sessions in the helm's {word} order")

        self.ok("click", "id:session-order")
        self.ok("click", "id:session-order-activity")
        eventually(lambda: stored() == "activity", "the helm storing activity again")

    # ---- The settings-and-hosts area's host Update, its progress, failure and rerun (47), on a host of its own --------

    def test_47_a_fake_hosts_update_shows_its_progress_and_failure_and_reruns(self) -> None:
        """A host's Update runs without a confirmation and SPEC.md ("Topology") keeps its row folded while it runs,
        showing the step count and current step inline, while a failure "still expands the row so its step list and
        diagnostic remain visible in Hosts"; the menu then offers the failed Update again. Only the real helm and its
        one-use plans show that the menu's click goes through the plan request and its confirmation to a run. This step
        registers a fake host of its own through the helm's API (a scripted setup that completes), then updates it from
        its row's menu with the scripted backend holding the first step, then lets the Update fail at installing the
        binary, then reruns it from the menu with the backend succeeding. Specified: with Hosts shown and Details off,
        the menu's Update opens no question and the row reads "Updating: 0 of ..." with no step list; released, the run
        fails, and the row shows "Update provisioning Failed", a step marked Failed and the backend's reason "disk
        full"; the menu's Re-run starts a second Update run that completes, after which the row shows neither the
        inline status nor the step list. The fake host is removed again once its last run has ended, the backend's
        script is emptied and Hosts is hidden, whatever happened; the snapshot's names of the status and the steps are
        the oracles."""
        destination = f"update-{uuid.uuid4().hex[:8]}.example"
        gui_agent.script_hosts(RUN, {})
        probe = gui_agent.helm_request(
            RUN, "POST", "/api/hosts/probe", {"target": {"kind": "ssh", "destination": destination}, "new_entry": {}}
        )
        self.assertEqual(probe["result"], "provisionable", probe)
        host = gui_agent.helm_request(RUN, "POST", "/api/hosts/provision", {"probe_id": probe["probe_id"]})["host_id"]

        def run_view() -> dict:
            """The host's retained run as the helm serves it."""
            return gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning")

        try:
            eventually(lambda: run_view().get("status") == "completed", f"{destination}'s setup completing")
            setup = run_view()["run_id"]
            held = {"hold_actions": True, "fail_action": "install-farhelm", "message": "disk full"}
            gui_agent.script_hosts(RUN, {"targets": {f"ssh:{destination}": held}})
            if HOSTS_PANEL not in self.ok("snapshot"):
                self.ok("click", "id:hosts-panel-toggle")
            eventually(lambda: HOSTS_PANEL in self.ok("snapshot"), "the Hosts panel showing")

            def showing(*texts: str):
                """A check for `eventually`: the snapshot, once it names every one of `texts`."""

                def check() -> str | None:
                    shot = self.ok("snapshot")
                    return shot if all(text in shot for text in texts) else None

                return check

            self.ok("click", f"id:host-menu-toggle-{host}")
            self.ok("click", "id:host-menu-update")
            running = eventually(showing("Updating: 0 of"), "the row showing the held Update's progress")
            self.assertNotIn("Update provisioning", running, "the row stays folded while the Update runs")
            self.assertNotEqual(run_view()["run_id"], setup, "the Update started a run of its own")

            gui_agent.script_hosts(RUN, {"targets": {f"ssh:{destination}": {**held, "hold_actions": False}}})
            failed = eventually(showing("Update provisioning Failed"), "the row opening on the failed Update")
            self.assertRegex(failed, r": Failed", "a step is marked Failed")
            self.assertIn("disk full", failed, "the backend's reason shows")

            gui_agent.script_hosts(RUN, {})
            first = run_view()["run_id"]
            self.ok("click", f"id:host-menu-toggle-{host}")
            self.ok("click", "id:host-menu-rerun")

            def rerun_completed() -> bool:
                """Whether an Update run other than the failed one has completed on the host."""
                view = run_view()
                return (
                    view.get("run_id") not in (None, first)
                    and view.get("operation") == "update"
                    and view.get("status") == "completed"
                )

            eventually(rerun_completed, f"the rerun of {destination}'s Update completing")

            def folded() -> bool:
                """Whether the row shows neither the inline status nor the run's steps any more."""
                shot = self.ok("snapshot")
                return "Updating" not in shot and "Update provisioning" not in shot

            eventually(folded, "the row folding back to its phase words after the rerun")
        finally:
            failing = sys.exc_info()[0] is not None
            try:
                gui_agent.script_hosts(RUN, {})
                eventually(lambda: run_view().get("status") != "running", f"{destination}'s run ending")
                gui_agent.helm_request(RUN, "DELETE", f"/api/hosts/{host}")
                agent("press", "escape")
                if HOSTS_PANEL in self.ok("snapshot"):
                    self.ok("click", "id:hosts-panel-toggle")
            except (AssertionError, urllib.error.HTTPError) as error:
                if not failing:
                    raise
                print(f"gui-e2e-smoke: step 47 left host {host} ({destination}) behind: {error}", file=sys.stderr)

    # ---- The other-dialogs area's Send feedback (48) ----------------------------------------------------------------

    # NOTE: This step never presses Send. Nothing carries a stand-in endpoint into the app's embedded helm, so a Send
    # here would post real feedback to the project's endpoint. The send and its outcomes are covered in process
    # (`crates/gui/farhelm-gui/src/feedback/tests.rs`), and the helm's forwarding by its own tests against a stand-in.

    def test_48_send_feedback_opens_from_the_help_menu_and_cancel_returns_to_the_button(self) -> None:
        """SPEC.md's Send feedback is the Help menu's first item, and its dialog is how a user reaches the maintainer
        privately; closing it, like every dialog the menu opens, hands focus back to the "?" button. The step seeds and
        touches no session, never presses Send and opens no link, so nothing leaves the machine. Specified: choosing
        Send feedback from the menu opens the dialog without "Re-use for future feedback"; once a message and a
        contact are typed, the contact no longer blank, it appears (the snapshot shows no field's text, so Re-use is
        the oracle that the contact took the typing); Cancel closes the dialog with focus on the "?" button. The
        footer's note is the oracle for open and closed, since only the open dialog draws it."""
        self.assertNotIn(FEEDBACK_NOTE, self.ok("snapshot"), "premise: the feedback dialog is not open")
        self.ok("click", "id:app-help-toggle")
        self.ok("wait", "--text", HELP_MENU_SUMMARY)
        self.ok("click", "id:help-menu-send-feedback")
        self.ok("wait", "--gone", HELP_MENU_SUMMARY)
        self.ok("wait", "--text", FEEDBACK_NOTE)
        self.assertNotIn(FEEDBACK_REUSE, self.ok("snapshot"), "premise: no contact is remembered, so no Re-use")
        self.ok("fill", "id:feedback-message", "Smoke test feedback, never sent")
        self.ok("fill", "id:feedback-contact", "smoke@example.invalid")
        self.ok("wait", "--text", FEEDBACK_REUSE)
        self.ok("click", "id:feedback-cancel")
        self.ok("wait", "--gone", FEEDBACK_NOTE)
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Help" \[focused\]', "focus is back on the ? button")

    # ---- The other-dialogs area's Trash (49) ---------------------------------------------------------------------

    def test_49_the_headers_trash_opens_its_dialog_and_escape_returns_to_it(self) -> None:
        """The session list header's Trash button is the way into the archived-checkout trash, and its dialog reads
        each connected host's trash through the real helm and supervisor. The bridge's fleet holds no managed
        checkout (its sessions run in `/tmp`), so this step sees an empty trash and deletes nothing; it seeds and
        touches no session. Specified: with the dialog not open, a click on Trash opens it with focus on its footer
        Close and, once the local host's measured listing answers, the line saying the trash is empty on reachable
        hosts; Escape closes it with focus back on the Trash button. The dialog's opening words are the oracle for
        open and closed, since only the open dialog draws them."""
        self.assertNotIn(TRASH_INTRO, self.ok("snapshot"), "premise: the Trash dialog is not open")
        self.ok("click", "id:checkout-trash-button")
        self.ok("wait", "--text", TRASH_INTRO)
        self.assertTrue(trash_close_focused(self.ok("snapshot", "-i")), "focus starts on the dialog's footer Close")
        self.ok("wait", "--text", "The trash is empty on reachable hosts.")
        self.ok("press", "escape")
        self.ok("wait", "--gone", TRASH_INTRO)
        self.assertRegex(
            self.ok("snapshot", "-i"), r'button "Checkout trash, [^"]*" \[focused\]', "focus is back on Trash"
        )

    # ---- The session view's hidden open session (50), on a session of its own ----------------------------------------

    def test_50_a_status_filter_that_hides_the_open_session_keeps_its_pane_and_rename(self) -> None:
        """Hiding the open session with a filter changes neither the selection nor its terminal (SPEC.md, "Session
        list"), and Rename's shortcut renames it "including when its row is filtered out" ("Desktop window chrome").
        Only the real helm shows that this holds end to end: the window's query names the open session, and the helm
        keeps that session in the pushed collection outside the list, so the pane reads its current state. Specified,
        on a replayed fake agent this step seeds to hold an idle screen, under a title of its own, once the helm lists
        it idle: with it open, a click on Running takes its row (and its actions button) out of the list while its
        title stays on screen in the details bar; the platform's Rename chord opens "Rename session", and a new title
        saved there closes it, makes the helm list the session under that title and shows the new title, still with no
        row for it. The step leaves All chosen, as it found it."""
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        tag = uuid.uuid4().hex[:8]
        work = RUN / "work"
        work.mkdir(exist_ok=True)
        transcript = work / f"hidden-{tag}-idle.txt"
        transcript.write_text(gui_agent.transcript(1, "idle"))
        body = gui_agent.seed_body(1, gui_agent.SESSION_CWD, fixtures, "idle", transcript)
        title = body["title"] = f"Hidden {tag}"
        id = gui_agent.helm_request(RUN, "POST", "/api/sessions", body)["id"]
        gui_agent.wait_for_statuses(lambda: gui_agent.listed_states(RUN), {id: "idle"})
        row_actions = f'"Session actions for {title}"'
        renamed = f"Hidden renamed {tag}"
        try:
            self.ok("click", f"text:{title}")
            eventually(lambda: last_selected() == id, "premise: the helm remembering the session as selected")
            self.ok("click", "id:session-status-Running")
            eventually(lambda: row_actions not in self.ok("snapshot"), "the idle session's row leaving the list")
            self.assertIn(title, self.ok("snapshot"), "the details bar still names the hidden open session")

            self.ok("press", "cmd-l" if sys.platform == "darwin" else "ctrl-shift-l")
            self.ok("wait", "--text", "Rename session")
            self.ok("fill", "text:New session title", renamed)
            self.ok("click", "text:Save")
            self.ok("wait", "--gone", "Rename session")
            eventually(lambda: listed_titles().get(id) == renamed, "the helm listing the hidden session renamed")
            self.ok("wait", "--text", renamed)
            self.assertNotIn(f'"Session actions for {renamed}"', self.ok("snapshot"), "still no row for it")
        finally:
            # Whether the step is already failing: then its own error is the report, and the cleanup must not replace
            # it with an error of its own.
            failing = sys.exc_info()[0] is not None
            result = agent("click", "id:session-status-All")
            if result.returncode != 0 and not failing:
                self.fail(f"choosing All again failed:\n{result.stdout}{result.stderr}")

    # ---- The session list's New and the quick switcher's New row (51), on a session of their own ---------------------

    def test_51_new_and_the_switchers_new_row_open_the_launcher(self) -> None:
        """New is how a session starts, and both of the window's triggers for it reach the launcher through the helm's
        live state: the heading's button, and the quick switcher's "New session named X" row, which needs the switcher's
        own read of the helm's listing first. Neither may start a session (SPEC.md: "Launch remains the user's action").
        Specified, with this step's own session open (titled with a per-run stamp): a click on the heading's New opens
        the launcher (its "Start an agent or a command on any host" summary) and Escape closes it; the platform's chord
        opens the switcher, a name filled into its search field offers `New session named "<name>"`, and a click on
        that row closes the switcher and opens the launcher with the name in its Name field; Escape closes it, and the
        helm lists the same sessions as before and still remembers this step's session as the last selected."""
        stamp = uuid.uuid4().hex[:8]
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        title = f"New origin {stamp}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures)
        body["title"] = title
        gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", "--text", title)
        origin = eventually(lambda: listed_id(title), f"{title} in the helm's listing")
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        eventually(lambda: last_selected() == origin, "premise: the helm remembering the session as selected")
        before = set(listed_titles())

        self.ok("click", "id:new-session-button")
        self.ok("wait", "--text", LAUNCHER_SUMMARY)
        self.ok("press", "escape")
        self.ok("wait", "--gone", LAUNCHER_SUMMARY)

        name = f"named-{stamp}"
        self.ok("press", "cmd-k" if sys.platform == "darwin" else "ctrl-shift-k")
        self.ok("wait", "--text", "Recent sessions")
        self.ok("fill", "text:Search sessions", name)
        self.ok("wait", "--text", "New session named")
        self.assertIn(f'New session named \\"{name}\\"', self.ok("snapshot", "-i"), "the row names the typed name")
        self.ok("click", "id:quick-switcher-new")
        self.ok("wait", "--gone", SWITCHER_FOOTER)
        self.ok("wait", "--text", LAUNCHER_SUMMARY)
        self.ok("wait", "--text", name)
        self.ok("press", "escape")
        self.ok("wait", "--gone", LAUNCHER_SUMMARY)
        self.assertEqual(set(listed_titles()), before, "neither New started a session")
        self.assertEqual(last_selected(), origin, "and the open session is unchanged")

    # ---- The session list's Replace of a YOLO launch (54), on a session of its own ----------------------------------

    def test_54_a_yolo_replace_asks_in_its_row_and_the_one_off_answer_replaces(self) -> None:
        """Every host asks before YOLO launches until the user says otherwise, and the helm refuses a YOLO launch's
        plain Replace there with a marker the app answers with the launcher's YOLO question, drawn in the row the
        Replace was asked from; the one-off answer sends the Replace again with consent. That path crosses the helm's
        refusal marker, the operation store, the launcher's question and the row, which the in-process tests cover only
        against the helm client's fake. Specified, on a fake-agent command launch this step creates asserted YOLO (with
        consent, under a title of its own): the menu's Replace and Confirm replace bring up the question inside the
        source's row (its `alertdialog` nested under the row's `listitem` in the accessibility tree), and once "Start
        YOLO session anyway" is clicked the helm lists a session under the same title with a new id and not the old
        one, and the local host still asks before YOLO launches (the one-off answer changes no setting). The helm's
        listing is the oracle, as in step 34."""
        hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
        local = [host for host in hosts if host["kind"] == "local"]
        self.assertEqual(len(local), 1, f"premise: the helm lists one local host: {hosts}")
        self.assertFalse(local[0]["yolo_without_asking"], "premise: the local host asks before YOLO launches")
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        title = f"YOLO replace {uuid.uuid4().hex[:8]}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures)
        body["title"] = title
        body["command"]["yolo"] = True
        body["confirm_yolo"] = True
        source = gui_agent.helm_request(RUN, "POST", "/api/sessions", body)["id"]
        self.ok("wait", "--text", title)

        self.ok("click", f"text:Session actions for {title}")
        self.ok("wait", "--text", MENU_DESCRIPTION)
        self.ok("click", "id:session-menu-replace")
        self.ok("wait", "--text", "Confirm replace")
        self.ok("click", "text:Confirm replace")
        self.ok("wait", "--text", YOLO_ONE_OFF)
        self.assertTrue(
            nested_under(self.ok("snapshot", "-i"), f'listitem "{title}"', 'alertdialog "Confirm YOLO launch"'),
            "the question is drawn inside the source's row",
        )
        self.assertIn(source, listed_titles(), "premise: the refused Replace kept the source")
        self.ok("click", f"text:{YOLO_ONE_OFF}")

        def replacement() -> bool:
            titles = listed_titles()
            fresh = [id for id, listed in titles.items() if listed == title and id != source]
            return source not in titles and len(fresh) == 1

        eventually(replacement, f"{title} listed under a new id and not under its old one")
        self.ok("wait", "--gone", YOLO_ONE_OFF)
        hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
        self.assertFalse(
            [host for host in hosts if host["kind"] == "local"][0]["yolo_without_asking"],
            "the one-off answer left the local host asking",
        )

    # ---- The other-dialogs area's Welcome card (52, 53) --------------------------------------------------------------

    def test_52_welcome_reopens_from_the_help_menu_and_escape_returns_to_the_button(self) -> None:
        """SPEC.md's Welcome item in the Help menu shows the Welcome card again; reopening it does not rewrite the
        shared seen preference, and closing it returns focus to the "?" button. The step seeds and touches no session
        and follows neither of the card's links (Read Get started would open a browser, Send feedback now the feedback
        dialog). Specified: with the card not open, choosing Welcome from the menu closes the menu and opens the card
        with focus on Got it; Escape closes it with focus on the "?" button, and the helm's `welcome_seen` is what it
        was before the step. The footer's note is the oracle for open and closed, since only the open card draws it."""
        self.assertNotIn(WELCOME_NOTE, self.ok("snapshot"), "premise: the Welcome card is not open")
        seen = gui_agent.helm_request(RUN, "GET", "/api/preferences").get("welcome_seen")
        self.ok("click", "id:app-help-toggle")
        self.ok("wait", "--text", HELP_MENU_SUMMARY)
        self.ok("click", "id:help-menu-welcome")
        self.ok("wait", "--gone", HELP_MENU_SUMMARY)
        self.ok("wait", "--text", WELCOME_NOTE)
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Got it" \[focused\]', "focus starts on Got it")
        self.ok("press", "escape")
        self.ok("wait", "--gone", WELCOME_NOTE)
        self.assertRegex(self.ok("snapshot", "-i"), r'button "Help" \[focused\]', "focus is back on the ? button")
        self.assertEqual(
            gui_agent.helm_request(RUN, "GET", "/api/preferences").get("welcome_seen"),
            seen,
            "reopening the card wrote nothing",
        )

    def test_53_a_window_start_opens_the_card_only_when_the_run_asks_for_it(self) -> None:
        """Every bridge run's helm is fresh, so `gui-agent.py start` turns the window's start dialogs off unless given
        `--start-dialogs` (`docs/gui-agent.md`); this step is the end-to-end proof of both halves, which is what lets
        every other step and the screenshot batch rely on a default start showing no card. The default half is this
        smoke's own app: had its start opened the card, the card would still be open or some close would have marked
        it seen (each step's `setUp` presses Escape, which dismisses it), so the card's absence together with
        `welcome_seen` still unset shows it never opened. The other half needs a window start of its own, so this step
        starts a second app run of its own (no sessions, the binaries the smoke's start built) in a run directory beside
        the smoke's, and stops it whatever happens. Specified: the smoke's app shows no card and its helm has no
        `welcome_seen`; a run started with `--start-dialogs` shows the card by itself, without any input, and its
        helm's `welcome_seen` is unset, since only a close of the card writes it."""
        assert RUN is not None
        self.assertNotIn(WELCOME_NOTE, self.ok("snapshot"), "the default start opened no card")
        self.assertIsNone(
            gui_agent.helm_request(RUN, "GET", "/api/preferences").get("welcome_seen"),
            "nothing marked the card seen, so the default start never opened it",
        )
        other = RUN.parent / "start-dialogs"

        def other_run(*args: str, timeout: float | None = STEP_TIMEOUT_SECONDS) -> subprocess.CompletedProcess:
            return subprocess.run(
                [sys.executable, str(AGENT), str(other), *args], capture_output=True, text=True, timeout=timeout
            )

        stopped: subprocess.CompletedProcess | None = None
        try:
            # Inside the `try`, so an interrupt during `start` (Ctrl-C, or the recorder's SIGTERM, which `main` turns
            # into KeyboardInterrupt) still stops whatever it had started. No timeout for `start` and `stop`, as for the
            # smoke's own: `start` may wait on a build lock and `stop` must always finish its cleanup.
            started = other_run("start", "--sessions", "0", "--start-dialogs", "--no-build", timeout=None)
            self.assertEqual(started.returncode, 0, f"the second run did not start:\n{started.stdout}{started.stderr}")
            shown = other_run("wait", "--text", WELCOME_NOTE)
            self.assertEqual(shown.returncode, 0, f"no card at the asked-for start:\n{shown.stdout}{shown.stderr}")
            self.assertIsNone(
                gui_agent.helm_request(other, "GET", "/api/preferences").get("welcome_seen"),
                "opening the card wrote nothing",
            )
        finally:
            # Only a run that got as far as writing `run.json` has anything to stop, as `main` checks `RUN` exists; a
            # `start` that failed before then keeps its own error rather than `stop`'s. Nothing is asserted here, so
            # an interrupt or an earlier failure is never replaced by this one.
            if (other / "run.json").exists():
                stopped = other_run("stop", timeout=None)
                if stopped.returncode != 0:
                    print(f"gui-e2e-smoke: the second run's `stop` failed; it is kept at {other}", file=sys.stderr)
        self.assertIsNotNone(stopped, "the second run wrote no run.json, so it never started")
        assert stopped is not None
        self.assertEqual(stopped.returncode, 0, f"the second run did not stop:\n{stopped.stdout}{stopped.stderr}")

    # ---- The settings-and-hosts area's host uninstall (61) ----------------------------------------------------------

    def test_61_a_fake_hosts_uninstall_is_refused_then_runs_and_leaves_a_notice(self) -> None:
        """Uninstall is the only supported way to remove Farhelm from a remote host (SPEC.md, "Supported host setup"),
        and only the real helm shows that the menu's click goes through its plan, the confirmation and its one-use
        plan to a run that forgets the host, and that its refusals reach the window. A fake host is never connected,
        so the helm refuses its uninstall as not connected until the scripted backend reports the unit file already
        gone (`unit_removed`, as after a run that failed past removing it), when the helm plans the rest offline.
        This step registers a fake host of its own through the helm's API (a scripted setup that completes) and,
        with Hosts shown: chooses Uninstall from its menu, which opens the host refusal dialog naming the host as not
        connected, and Close closes it; then, with the unit reported gone and the backend holding its steps, chooses
        Uninstall again, which opens the confirmation with the helm's plan, and Uninstall there starts the run, which
        the row shows as "Uninstalling: 0 of ..." with its steps; released, the run makes the helm forget the host, the
        panel shows the removal notice naming it, and the notice's dismiss control hides it. The backend's script is
        emptied, a host left behind is removed and Hosts is hidden, whatever happened; the snapshot's names of the
        dialogs, the status and the notice are the oracles."""
        destination = f"uninstall-{uuid.uuid4().hex[:8]}.example"
        target = f"ssh:{destination}"
        gui_agent.script_hosts(RUN, {})
        probe = gui_agent.helm_request(
            RUN, "POST", "/api/hosts/probe", {"target": {"kind": "ssh", "destination": destination}, "new_entry": {}}
        )
        self.assertEqual(probe["result"], "provisionable", probe)
        host = gui_agent.helm_request(RUN, "POST", "/api/hosts/provision", {"probe_id": probe["probe_id"]})["host_id"]
        refused = "Farhelm refused this action"
        plan = f"Farhelm will uninstall from {destination}"
        notice = f"Farhelm was removed from {destination}; its data remains in"

        def listed() -> bool:
            """Whether the helm still lists the fake host."""
            return any(row["id"] == host for row in gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"])

        def run_view() -> dict:
            """The host's retained run as the helm serves it."""
            return gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning")

        def showing(*texts: str):
            """A check for `eventually`: the snapshot, once it names every one of `texts`."""

            def check() -> str | None:
                shot = self.ok("snapshot")
                return shot if all(text in shot for text in texts) else None

            return check

        try:
            eventually(lambda: run_view().get("status") == "completed", f"{destination}'s setup completing")
            if HOSTS_PANEL not in self.ok("snapshot"):
                self.ok("click", "id:hosts-panel-toggle")
            eventually(lambda: HOSTS_PANEL in self.ok("snapshot"), "the Hosts panel showing")

            self.ok("click", f"id:host-menu-toggle-{host}")
            self.ok("click", "id:host-menu-uninstall")
            shot = eventually(showing(refused, "Uninstall Farhelm"), "the refusal dialog opening")
            self.assertIn("isn't connected", shot, "the helm's reason shows")
            self.ok("click", "id:host-refusal-done")
            self.ok("wait", "--gone", refused)
            self.assertTrue(listed(), "the refusal changed nothing")

            gui_agent.script_hosts(RUN, {"targets": {target: {"unit_removed": True, "hold_actions": True}}})
            self.ok("click", f"id:host-menu-toggle-{host}")
            self.ok("click", "id:host-menu-uninstall")
            eventually(showing(plan), "the confirmation showing the helm's plan")
            self.ok("click", "id:prompt-confirm")
            self.ok("wait", "--gone", plan)
            eventually(showing("Uninstalling: 0 of", "Uninstall provisioning Running"), "the row showing the run")

            gui_agent.script_hosts(RUN, {"targets": {target: {"unit_removed": True}}})
            eventually(lambda: not listed(), "the helm forgetting the uninstalled host")
            eventually(showing(notice), "the removal notice naming the host")
            self.ok("click", "id:uninstall-notice-dismiss")
            eventually(lambda: notice not in self.ok("snapshot"), "the notice going")
        finally:
            failing = sys.exc_info()[0] is not None
            try:
                gui_agent.script_hosts(RUN, {})
                if listed():
                    eventually(lambda: run_view().get("status") != "running", f"{destination}'s run ending")
                    gui_agent.helm_request(RUN, "DELETE", f"/api/hosts/{host}")
                snapshot = self.ok("snapshot")
                if refused in snapshot or plan in snapshot:
                    agent("press", "escape")
                if HOSTS_PANEL in self.ok("snapshot"):
                    self.ok("click", "id:hosts-panel-toggle")
            except (AssertionError, urllib.error.HTTPError) as error:
                if not failing:
                    raise
                print(f"gui-e2e-smoke: step 61 left host {host} ({destination}) behind: {error}", file=sys.stderr)

    # ---- The approvals area's steps (60, 62 to 64), on sessions and hosts of their own ------------------------------
    #
    # These port the old app's spawn acceptance cases (and the agent host add's) to the desktop app: every request an
    # agent makes there now waits on a card in the window, and only the user's answer carries it out. Steps 60, 62 and 63
    # each seed a session of their own running the spawning fake agent, whose `spawn <folder>` line runs the real
    # `farhelm spawn` inside it; each child is titled after the fresh folder it starts in, so the window's row and the
    # helm's listing can be told apart from every other step's. Step 64 asks from a plain shell session instead, since
    # no fake agent script asks to add a host.

    def test_60_an_agent_request_waits_on_its_card_and_allow_carries_it_out(self) -> None:
        """An agent's request that needs approval waits on a card in the window, and the user's answer is what carries
        it out: the whole point of the approvals area, and what the desktop app refused at once before its cards
        existed. It crosses every layer: the fake agent runs the real `farhelm spawn` inside its session, the
        supervisor relays the request, the helm holds it and pushes it to the window, the window's approval region
        draws its card, and the card's Allow goes back through the helm's answer route. Specified: in a session of
        this step's own running the spawning fake agent, `spawn <folder>` makes the helm list one waiting request and
        the window draw its card ("Agent request"); Allow on that card ends the request, the agent reports the child's
        id, and the child's row, titled after its folder, appears in the window's list while nothing else is done to
        it (the old app's "children appear without refreshing the observer")."""
        title = self.open_spawner("Approval spawner")
        folder = spawn_folder("allowed")
        request = self.spawn_waits_on_a_card(title, folder)
        self.ok("wait", "--text", "Agent request")
        self.answer(request, "allow")
        # The Allow button went with its card, and focus recovery sent focus back to the spawner's terminal, which
        # `terminal-text` reads; a click on its row makes that explicit rather than relying on recovery.
        self.ok("click", f"text:{title}")
        child = re.search(r"SPAWNED:(\S+)", self.ok("terminal-text", "--wait", "SPAWNED:"))
        self.assertIsNotNone(child, "the agent reports the child's id")
        self.ok("wait", f"text:{folder.name}")
        # By title, not by the reported id: the reference `farhelm spawn` prints is not spelled the way the helm's
        # listing keys its sessions (a lookup by it found nothing on a run where the child was listed).
        self.assertIn(folder.name, listed_titles().values(), "the helm lists the child")

    def test_62_a_host_only_grant_lets_the_next_same_host_spawn_through_without_a_card(self) -> None:
        """"Always allow on <host> only" is the narrow grant SPEC.md offers for an agent acting on its own host's
        sessions, and its whole use is that the next such request needs no card; a grant that turned on the wider
        `farhelm` commands setting, or one that still asked, would be wrong either way. Specified: on a local host that
        asks for both, a spawn's card offers the host-only answer; choosing it ends the request, starts the child and
        turns on the host's "Manage sessions on this host without asking" alone; the next spawn from the same session
        (the fake agent's `spawn-parented` form, so its reply marker differs from the first's) then starts its child
        without the helm ever listing a waiting request, and both children's rows appear in the window. The grant is
        turned off again afterwards, so later steps find the host asking as before."""
        title = self.open_spawner("Host grant spawner")
        local = local_host()
        self.addCleanup(
            gui_agent.helm_request,
            RUN,
            "POST",
            f"/api/hosts/{local['id']}/own-sessions-without-asking",
            {"own_sessions_without_asking": False},
        )
        first = spawn_folder("granted-first")
        request = self.spawn_waits_on_a_card(title, first)
        self.assertTrue(request["own_host_session_action"], "premise: the helm offers the host-only grant here")
        self.answer(request, "always-allow-own-host")
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "SPAWNED:")
        self.ok("wait", f"text:{first.name}")
        granted = local_host()
        self.assertTrue(granted["own_sessions_without_asking"], "the answer turns the host-only grant on")
        self.assertFalse(granted["commands_without_asking"], "and leaves the wider commands setting off")

        second = spawn_folder("granted-second")
        self.ok("type", "id:terminal-grid", f"spawn-parented {second}")
        self.ok("press", "enter")

        def started_without_a_card() -> bool:
            """Whether the second child is listed yet, failing at once if a request waited on the way."""
            self.assertEqual(waiting_approvals(), [], "the granted spawn waited for an answer")
            return second.name in listed_titles().values()

        eventually(started_without_a_card, "the granted spawn starting its child")
        self.ok("terminal-text", "--wait", "SPAWNED-PARENTED:")
        self.ok("wait", f"text:{second.name}")

    def test_63_a_spawn_denied_over_an_open_dialog_creates_nothing_and_tells_the_agent(self) -> None:
        """The user's no is what stops the agent, and a card stays answerable while a dialog the user opened is up
        (SPEC.md, "Keyboard focus": approval cards stay usable over modals), without disturbing that dialog. A denied
        spawn that still started a child, or left the agent without an answer, would make the card decorative; a card
        the quick switcher's veil covered, or a Deny that closed the switcher, would break the other half. Specified:
        with a spawn's card up, the quick switcher's chord from the spawner's terminal opens the switcher; Deny on the
        card ends the request while the switcher stays open; Escape then closes it; the agent's `farhelm spawn` fails
        saying the user declined, and no session is listed under the child's folder name."""
        title = self.open_spawner("Denied spawner")
        folder = spawn_folder("denied")
        request = self.spawn_waits_on_a_card(title, folder)
        self.ok("press", "cmd-k" if sys.platform == "darwin" else "ctrl-shift-k")
        self.ok("wait", "--text", "Recent sessions")
        self.answer(request, "deny")
        self.assertIn(SWITCHER_FOOTER, self.ok("snapshot"), "the switcher stays open through the answer")
        self.ok("press", "escape")
        self.ok("wait", "--gone", SWITCHER_FOOTER)
        self.ok("click", f"text:{title}")
        screen = self.ok("terminal-text", "--wait", "SPAWN-ERROR:")
        # A long refusal can wrap in a narrow terminal, and the text the bridge reads breaks lines where it wraps.
        self.assertIn("declined", screen.replace("\n", ""), "the agent is told the user declined")
        self.assertNotIn(folder.name, listed_titles().values(), "a denied spawn starts no child")

    def test_64_an_agents_host_add_goes_through_the_add_host_dialog(self) -> None:
        """An agent may ask to add a host, but only the user's Add in the ordinary Add host dialog approves it, with
        whatever destination and alias the dialog then holds, and an agent's add always asks before setting a host up,
        whatever the user's preference says (SPEC.md, "Agent-spawned sessions"). Specified: from a shell session of
        this step's own, `farhelm agent host add <destination> --alias <alias>` makes the helm list one waiting request
        for a host add; the card's Allow opens the Add host dialog, and the request still waits (opening is not
        approval); the dialog's Add ends the request and, with "Set up new hosts without asking" turned on for this
        step, still asks "Set up host · <destination>"; Cancel there adds nothing; and the agent's command reports its
        request accepted for the destination the dialog held, the agent's own. That the dialog shows the agent's
        destination and alias is the in-process tests' (`approvals/host_add_tests.rs`). The fake destination never
        reaches real ssh: the app's scripted backend (`start --scripted-hosts`) answers its probe. The preference is
        put back afterwards, and a host the step left registered is removed."""
        stamp = uuid.uuid4().hex[:8]
        destination = f"agent-add-{stamp}.example"
        alias = f"alias-{stamp}"
        preferences = gui_agent.helm_request(RUN, "GET", "/api/preferences")
        gui_agent.helm_request(RUN, "PUT", "/api/preferences", {"skip_host_setup_confirmation": True})
        self.addCleanup(
            gui_agent.helm_request,
            RUN,
            "PUT",
            "/api/preferences",
            {"skip_host_setup_confirmation": preferences.get("skip_host_setup_confirmation")},
        )
        self.addCleanup(remove_host, destination)
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        title = f"Host adder {stamp}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures)
        body["title"] = title
        body["command"]["command"] = "sh"
        gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", f"text:{title}")
        self.ok("click", f"text:{title}")
        # The shell's answer, not the typed line, carries the marker: the typed line holds `$((40+2))` instead.
        self.ok("type", "id:terminal-grid", "echo shell-ready-$((40+2))")
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", "shell-ready-42")
        self.assertEqual(waiting_approvals(), [], "premise: no request waits yet")

        self.ok("type", "id:terminal-grid", f"farhelm agent host add {destination} --alias {alias}")
        self.ok("press", "enter")
        request = self.one_waiting_request("the host add request waiting for approval")
        self.assertEqual(request["action"]["kind"], "host_add")

        def dialog_open() -> bool:
            """Whether the card's Allow opened the Add host dialog, pressing it again while the answer bar's pause
            lasts. Each press gets a moment for the dialog to show before the next, since a press after it opened would
            only draw the card's "Another dialog is open" refusal."""
            agent("click", f"id:approval-{request['id']}-answer-allow")
            return agent("wait", "--text", ADD_HOST_SUMMARY, "--timeout", "1500").returncode == 0

        try:
            eventually(dialog_open, "the card's Allow opening the Add host dialog")
        except AssertionError:
            # Whether the request still waits tells a press the card kept refusing from a request that went away.
            raise AssertionError(
                f"the card's Allow never opened the Add host dialog; requests waiting now: {waiting_approvals()}"
            ) from None
        self.assertEqual([r["id"] for r in waiting_approvals()], [request["id"]], "opening the dialog approves nothing")

        self.ok("click", "id:add-host-submit")
        question = f"Set up host · {destination}"
        eventually(lambda: question in self.ok("snapshot"), "the agent's add asking before setup")
        self.assertEqual(waiting_approvals(), [], "the dialog's Add ended the request")
        self.ok("click", "id:prompt-cancel")
        self.ok("wait", "--gone", question)
        hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
        self.assertNotIn(destination, [h.get("destination") for h in hosts], "Cancel at setup adds no host")
        self.ok("click", f"text:{title}")
        # The reply names the destination the dialog's Add carried, the one its field held: the window cannot show
        # that apart from the card behind it, which shows the agent's destination too. A long reply wraps, and the
        # text the bridge reads breaks lines where it wraps.
        reply = self.ok("terminal-text", "--wait", "accepted;").replace("\n", "")
        self.assertIn(destination, reply, "the agent is told its add was accepted for the destination the dialog held")

    # ---- Helpers for the approvals area's steps ---------------------------------------------------------------------

    def open_spawner(self, prefix: str) -> str:
        """Seed and open a session of the calling step's own running the spawning fake agent, under a fresh title
        starting with `prefix`, and return that title once its terminal shows the agent ready.

        Checks the premise every spawn step rests on, that no request waits and the local host asks for both `farhelm`
        commands and session actions (otherwise no card would come). The spawner's own launch is a command Farhelm did
        not compose, so an agent may copy it only where YOLO launches need no asking: this turns that on for the local
        host until the step ends, then puts back what it was, and waives no approval. The session's body is the
        harness's own `spawner` kind, so this step and the screenshot scene run the same fake agent."""
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        title = f"{prefix} {uuid.uuid4().hex[:8]}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures, "spawner")
        body["title"] = title
        gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", f"text:{title}")
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        self.assertEqual(waiting_approvals(), [], "premise: no request waits yet")
        hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
        self.assertTrue(
            all(not h["commands_without_asking"] and not h["own_sessions_without_asking"] for h in hosts),
            "premise: no host lets `farhelm` commands or session actions through without asking",
        )
        local = local_host()
        gui_agent.helm_request(
            RUN, "POST", f"/api/hosts/{local['id']}/yolo-without-asking", {"yolo_without_asking": True}
        )
        self.addCleanup(
            gui_agent.helm_request,
            RUN,
            "POST",
            f"/api/hosts/{local['id']}/yolo-without-asking",
            {"yolo_without_asking": local["yolo_without_asking"]},
        )
        return title

    def spawn_waits_on_a_card(self, title: str, folder: Path) -> dict:
        """Type `spawn <folder>` into the open spawner titled `title` and return the one request the helm then lists
        as waiting. A failure says what the agent printed instead, which tells a refusal from a spawn that went through
        without asking."""
        self.ok("type", "id:terminal-grid", f"spawn {folder}")
        self.ok("press", "enter")
        return self.one_waiting_request("the spawn request waiting for approval")

    def one_waiting_request(self, what: str) -> dict:
        """The one request the helm lists as waiting, once it lists any; a failure names `what` and shows the focused
        terminal's text, where the asking command reports a refusal."""
        try:
            waiting = eventually(waiting_approvals, what)
        except AssertionError:
            screen = agent("terminal-text")
            raise AssertionError(
                f"{what} did not happen; the terminal shows:\n{screen.stdout}{screen.stderr}"
            ) from None
        self.assertEqual(len(waiting), 1, "one request waits")
        return waiting[0]

    def answer(self, request: dict, answer: str) -> None:
        """Press the card's `answer` button (its id's last part: `allow`, `deny`, `always-allow-own-host`) until the
        helm no longer lists `request` as waiting. The answer bar ignores presses for 700 ms after a card appears, so
        that a click aimed at one request cannot answer the next; the first presses can land in that pause."""
        button = f"id:approval-{request['id']}-answer-{answer}"
        last_click: list[subprocess.CompletedProcess] = []

        def answered() -> bool:
            if any(r["id"] == request["id"] for r in waiting_approvals()):
                last_click[:] = [agent("click", button)]
                return False
            return True

        try:
            eventually(answered, f"{answer} ending the request")
        except AssertionError:
            clicked = last_click[0] if last_click else None
            detail = f"{clicked.stdout}{clicked.stderr}" if clicked else "no click was made"
            raise AssertionError(f"{answer} did not end the request; the last click said:\n{detail}") from None

    # ---- The other-dialogs area's Keyboard shortcuts (65), on a session of its own ------------------------------

    def test_65_the_shortcuts_chord_from_a_terminal_opens_the_overview_and_escape_returns(self) -> None:
        """The Keyboard shortcuts chord is SPEC.md's one shortcut matched by the Slash key's position, taken by the
        app's keystroke interceptor before a focused terminal sees it, and closing the overview hands focus back to the
        terminal. Specified, in a basic fake-agent session this step creates under a title of its own: with its terminal
        focused and showing the echo of a stamped line, the platform's chord opens the overview with focus on its
        Close; Escape closes it, and the terminal has focus again (`terminal-text` reads only a focused terminal) and
        takes a second stamped line, which comes back as the agent's echo of exactly that line."""
        stamp = uuid.uuid4().hex[:8]
        fixtures = json.loads((RUN / "run.json").read_text())["binaries"]["fixtures"]
        title = f"Shortcuts origin {stamp}"
        body = gui_agent.seed_body(0, gui_agent.SESSION_CWD, fixtures)
        body["title"] = title
        gui_agent.helm_request(RUN, "POST", "/api/sessions", body)
        self.ok("wait", "--text", title)
        self.ok("click", f"text:{title}")
        self.ok("terminal-text", "--wait", "FAKE-AGENT READY")
        before = f"before-{stamp}"
        self.ok("type", "--keys", before)
        self.ok("press", "enter")
        self.ok("terminal-text", "--wait", f"echo:{before}")
        self.assertNotIn(SHORTCUTS_ROW, self.ok("snapshot"), "premise: the overview is not open")
        self.ok("press", "cmd-/" if sys.platform == "darwin" else "ctrl-shift-/")
        self.ok("wait", "--text", SHORTCUTS_ROW)
        self.assertRegex(
            self.ok("snapshot", "-i"), r'button "Close keyboard shortcuts" \[focused\]', "focus starts on Close"
        )
        self.ok("press", "escape")
        self.ok("wait", "--gone", SHORTCUTS_ROW)
        after = f"after-{stamp}"
        self.ok("type", "--keys", after)
        self.ok("press", "enter")
        screen = self.ok("terminal-text", "--wait", f"echo:{after}")
        # `terminal-text` prints the screen as one JSON string, its lines joined by escaped newlines.
        lines = [line.strip() for line in json.loads(screen).splitlines()]
        self.assertIn(f"echo:{after}", lines, "the echo is of exactly the typed line, nothing before it")


def waiting_approvals() -> list[dict]:
    """The requests the helm holds for the user's answer, oldest first (`GET /api/approvals`)."""
    return gui_agent.helm_request(RUN, "GET", "/api/approvals")["approvals"]


def local_host() -> dict:
    """The helm's row for the local host, with its permission settings as they are now."""
    return next(h for h in gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"] if h["kind"] == "local")


def spawn_folder(name: str) -> Path:
    """A new, empty folder for a spawned child to start in, inside this run's directory and named `name` plus a
    per-call stamp. A child is titled after its folder, so the stamp makes its row and listing entry this call's alone;
    the run directory goes with the run."""
    assert RUN is not None
    folder = RUN / "work" / f"{name}-{uuid.uuid4().hex[:8]}"
    folder.mkdir(parents=True)
    return folder


def remove_host(destination: str) -> None:
    """Remove the host registered under `destination`, if any, once no run holds it (the helm refuses a removal while
    one does): cleanup for a step that may have added one."""
    def registered() -> int | None:
        rows = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
        return next((row["id"] for row in rows if row.get("destination") == destination), None)

    host = registered()
    if host is None:
        return
    eventually(
        lambda: gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning").get("status") != "running",
        f"{destination}'s run ending before its host is removed",
    )
    gui_agent.helm_request(RUN, "DELETE", f"/api/hosts/{host}")


def nested_under(snapshot: str, outer: str, inner: str) -> bool:
    """Whether a `snapshot -i` tree shows a line containing `inner` nested under one containing `outer`: some line
    before it, less indented than it, with no line in between indented at most as far as that ancestor (which would
    close it)."""
    lines = snapshot.splitlines()
    for at, line in enumerate(lines):
        if inner not in line:
            continue
        depth = len(line) - len(line.lstrip())
        for above in reversed(lines[:at]):
            above_depth = len(above) - len(above.lstrip())
            if above_depth < depth:
                if outer in above:
                    return True
                depth = above_depth
    return False


def trash_close_focused(snapshot: str) -> bool:
    """Whether a `snapshot -i` tree shows focus on a Close button inside the Trash dialog: a focused `button "Close"`
    nested under the `dialog "Trash"` line, deeper than it, before the tree comes back to the dialog's depth. Matching
    the button alone would also accept the title bar's own Close, which Linux draws in the window."""
    lines = snapshot.splitlines()
    for at, line in enumerate(lines):
        if 'dialog "Trash"' not in line:
            continue
        depth = len(line) - len(line.lstrip())
        for inner in lines[at + 1 :]:
            if len(inner) - len(inner.lstrip()) <= depth:
                break
            if re.search(r'button "Close" \[focused\]', inner):
                return True
    return False

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


def step_refusals(source: str) -> list[str]:
    """Why the steps written in `source` would not all run, each once and in the order their numbers give, as one line
    per problem; empty when every step is in place. `main` prints these and refuses to start.

    Each area adds its steps to the one `Smoke` class, so merges and rebases keep bringing several areas' steps
    together, and three shapes they can leave behind lose or reorder a step without a word:

    - A function named `test_...` anywhere but directly in the module-level `Smoke` class is never collected. A jj
      automatic merge once put a module-level helper function between two areas' steps, so steps 52 and 53, which
      followed it, became unreachable code inside that function, which every run from then on skipped with nothing
      saying so. Reported in line order.
    - Two steps with the same name: Python keeps only the last definition, so the first never runs.
    - Two steps with the same number: two areas took the number independently, and their steps run in whatever order
      the rest of their names sort in. Refusing it makes the merge that brings them together surface the collision.

    The name check in `main` cannot see the first two, since `getTestCaseNames` only lists what the class ends up with,
    and it never compares numbers, so it misses the third.
    The source is parsed, not imported, so a stray step is found however deep it sits (in a function, at module level,
    or in another class) and before anything starts.

    A `test_` method under an `if` or `try` in the class body counts as stray too, although unittest would run it: no
    step needs that shape, and accepting it would make this check about what Python happens to bind rather than about
    where steps are written."""
    tree = ast.parse(source)
    steps = [
        member
        for node in tree.body
        if isinstance(node, ast.ClassDef) and node.name == "Smoke"
        for member in node.body
        if isinstance(member, (ast.FunctionDef, ast.AsyncFunctionDef)) and member.name.startswith("test_")
    ]
    placed = {id(step) for step in steps}
    stray = sorted(
        (node.lineno, node.name)
        for node in ast.walk(tree)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        and node.name.startswith("test_")
        and id(node) not in placed
    )
    refusals = [f"{name} (line {line}) is not a method of Smoke, so it would never run" for line, name in stray]
    # Grouped in class-body order (dicts keep insertion order), so each clash is reported where it first appears.
    by_name: dict[str, list[ast.FunctionDef | ast.AsyncFunctionDef]] = {}
    by_number: dict[str, list[ast.FunctionDef | ast.AsyncFunctionDef]] = {}
    for step in steps:
        by_name.setdefault(step.name, []).append(step)
        if numbered := re.match(r"test_(\d+)_", step.name):
            by_number.setdefault(numbered[1], []).append(step)
    for name, same in by_name.items():
        if len(same) > 1:
            lines = ", ".join(str(step.lineno) for step in same)
            refusals.append(f"{name} is defined in Smoke more than once (lines {lines}), so only the last would run")
    # A name defined twice is the clash above, not a number clash; it still lists every definition when another step
    # shares its number.
    for number, same in by_number.items():
        if len({step.name for step in same}) > 1:
            uses = ", ".join(f"{step.name} (line {step.lineno})" for step in same)
            refusals.append(f"step {number} is used by more than one method of Smoke: {uses}")
    return refusals


def main(argv: list[str]) -> int:
    """Start the app, run the steps, stop the app; exit 0 only when every step passed and the run stopped cleanly."""
    global RUN
    # First, before even the arguments: a step outside `Smoke`, or two sharing a name, would be skipped without a word,
    # and two sharing a number would run in an order nobody chose (`step_refusals` says how), so the run refuses rather
    # than passing with fewer steps than the file holds or with them out of order.
    refusals = step_refusals(Path(__file__).read_text(encoding="utf-8"))
    for refusal in refusals:
        print(f"gui-e2e-smoke: {refusal}", file=sys.stderr)
    if refusals:
        return 1
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
        # `--scripted-hosts` fakes every command the helm runs on a host (setup, Update and uninstall on SSH hosts)
        # for step 40 and the host steps after it. The session
        # steps never ask the helm to run one, and sessions themselves reach the supervisor as usual.
        start = ["start", "--sessions", "3", "--scripted-hosts", *(["--no-build"] if options.no_build else [])]
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
