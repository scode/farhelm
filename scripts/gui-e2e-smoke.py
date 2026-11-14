#!/usr/bin/env python3
"""End-to-end smoke test of the GPUI app: the real app, its embedded helm and a real supervisor with fake agents,
driven through the test-automation bridge (`scripts/gui-agent.py`, `docs/gui-agent.md`).

NOTE: This is a smoke test, not the end-to-end suite. It covers the first slice's path through the app once each
(the session list, a row's menu, Delete, Rename, and typing into the open session's terminal), so a break in the wiring
between the app, the helm and the supervisor shows up somewhere, plus the area steps added since (30, 31 and 33, the
terminal's: a large paste, on a Mac only, text an input method commits, and scrollback; 32, the sidebar's quick
switcher; 34, the session list's Stop and Replace from a row's menu; 35, the sidebar's host rail; 36, the row menu's
Mark read and Mark unread; 37, the sidebar's resize divider; 40, the settings-and-hosts area's scripted host
setup). The behavior itself is specified and tested in process (`crates/gui/farhelm-gui/src/session_list/`,
`session_pane.rs`, `terminal/attached/tests.rs`); nothing here checks layout or wording beyond the names it
clicks.

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

# The quick switcher's footer: visible text only the open switcher draws, so its going tells that the switcher closed.
SWITCHER_FOOTER = "↑↓ choose · Enter pick · Escape close"

# The host chip's accessible name while the local host is chosen: the snapshot shows it only while the chip is drawn.
LOCAL_CHIP = "Filtered to host Local; remove filter"

# The sidebar's divider holding focus, as `snapshot -i` lists it: its role (AccessKit's focusable separator), its name
# and its state.
DIVIDER_FOCUSED = re.compile(r'splitter "Sidebar width".*\[focused\]')

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
        self.ok("click", "text:Stop")
        eventually(lambda: gui_agent.listed_states(RUN).get(ids[stopped]) == "exited", f"{stopped} listed as exited")

        self.ok("click", f"text:Session actions for {replaced}")
        self.ok("wait", "--text", MENU_DESCRIPTION)
        self.ok("click", "text:Replace")
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
        adds stays registered (shown unreachable, since the helm dials it over real ssh), so a step that needs the local
        host to be the only one, as 35 does, runs before this one."""
        destination = f"scripted-{uuid.uuid4().hex[:8]}.example"
        probe = gui_agent.helm_request(
            RUN, "POST", "/api/hosts/probe", {"target": {"kind": "ssh", "destination": destination}, "new_entry": {}}
        )
        self.assertEqual(probe["result"], "provisionable", probe)
        accepted = gui_agent.helm_request(RUN, "POST", "/api/hosts/provision", {"probe_id": probe["probe_id"]})
        host, run_id = accepted["host_id"], accepted["run_id"]
        hosts = gui_agent.helm_request(RUN, "GET", "/api/hosts")["hosts"]
        listed = {row["id"]: row.get("destination") for row in hosts}
        self.assertEqual(listed.get(host), destination, "the confirmation registered a new entry for the destination")

        def settled() -> dict | None:
            """This run's progress once it has finished, either way. "completed" alone is also what the helm reports
            for a host with no run at all, so the run id is matched too."""
            view = gui_agent.helm_request(RUN, "GET", f"/api/hosts/{host}/provisioning")
            if view.get("run_id") == run_id and view.get("status") in ("completed", "failed"):
                return view
            return None

        view = eventually(settled, f"the scripted setup of {destination} finishing")
        self.assertEqual(view["status"], "completed", f"the setup failed: {view.get('message')}")
        log = (gui_agent.scripted_hosts_dir(RUN) / "events.jsonl").read_text()
        events = [json.loads(line) for line in log.splitlines()]
        steps = {event["event"] for event in events if event["target"] == f"ssh:{destination}"}
        self.assertIn("install-farhelm", steps, "the helm ran the setup's steps through the scripted backend")

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
