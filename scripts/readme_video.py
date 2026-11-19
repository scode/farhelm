#!/usr/bin/env python3
"""Record the staged demo with the same desktop bridge used by the screenshot runners."""

import argparse
from contextlib import ExitStack
from dataclasses import dataclass
import fcntl
import json
import math
import os
from pathlib import Path
import pwd
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

import docs_shots

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
SCENARIO = REPO / "docs/readme-video/scenario.json5"
SCENE_ACTION = "farhelm_agent::SetDemoScene"
# The software renderer's readback must finish before the next tick so the bridge can service input.
FPS = 1


@dataclass(frozen=True)
class Mark:
    """A frame already written during a beat, counted within its segment."""

    label: str
    frame: int


@dataclass(frozen=True)
class Segment:
    """A bridge recording and the wall time its frames represent, excluding cuts."""

    files: tuple[Path, ...]
    seconds: float
    marks: tuple[Mark, ...]


def bridge_record(run: Path, session: str, cmd: str, **body) -> dict:
    """Use the bridge's existing recording requests directly so stopping leaves PNGs without an intermediate encode."""
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(20)
        connection.connect(str(run / "agent" / session / "app.sock"))
        with connection.makefile("rwb") as stream:
            stream.write(json.dumps({"cmd": cmd, **body}).encode() + b"\n")
            stream.flush()
            reply = json.loads(stream.readline(64 * 1024))
    if "ok" not in reply:
        raise RuntimeError(f"{cmd}: {reply.get('error', {}).get('message', 'invalid bridge reply')}")
    return reply["ok"]


class Recording:
    """Only active segments have an open bridge recorder. Each has a fresh frame directory and timeline origin."""

    def __init__(self, gui, batch, bridge, window: dict, plan: dict, fps: int):
        self.gui, self.batch, self.bridge = gui, batch, bridge
        self.window, self.plan, self.fps = window, plan, fps
        self.segments: list[Segment] = []
        self.frames: Path | None = None
        self.marks: list[Mark] = []
        self.clip = (batch.LINUX_FRAME, batch.LINUX_FRAME + batch.LINUX_TITLE_BAR)
        self.size = gui.expected_capture_size(plan)
        self.raw_size = (self.size[0] + 2 * self.clip[0], self.size[1] + self.clip[0] + self.clip[1])
        if window["scale"] != 1 or (window["width"], window["height"]) != self.raw_size:
            raise RuntimeError(f"expected a decorated {self.raw_size[0]}x{self.raw_size[1]} window at scale 1")
        account = pwd.getpwuid(os.getuid())
        self.needles = gui.capture_needles(bridge.run, bridge.record, account.pw_name, account.pw_dir,
                                           gui.scenario_remote_hosts(plan), os.uname().nodename)

    def terminal_text(self) -> str:
        """The same focused-terminal probe used for screenshot readiness and privacy checks."""
        raw = self.bridge._call("get", "probe", "terminal_text")
        return "" if raw.strip() == "null" else self.gui.probe_text(raw)

    def check_privacy(self) -> dict:
        """Apply the screenshot leak guard to the drawn frame and terminal before using the capture."""
        tree = json.loads(self.bridge._call("tree"))
        found = self.gui.leaks(tree, self.terminal_text(), *self.needles)
        if found:
            raise RuntimeError("the staged video shows a machine detail:\n  " + "\n  ".join(found))
        return tree

    def _request(self, cmd: str, **data):
        return bridge_record(self.bridge.run, self.gui.SESSION, cmd, **data)

    def start(self, mark: str | None = None):
        """Start a numbered segment. A mark belongs to its first capture, while its cue is still in place."""
        if self.frames is not None:
            raise RuntimeError("a segment is already recording")
        self.check_privacy()
        self.frames = self.bridge.run / f"video-segment-{len(self.segments):02d}"
        self.marks = [Mark(mark, 0)] if mark else []
        self._request("record_start", window=self.window["window"], dir=str(self.frames), fps=self.fps)
        first = self.frames / "frame-00000.png"
        # The writer opens this file only after render_to_image has returned. Its contents can finish later.
        self.gui.poll("the recorder to capture its first frame",
                      lambda: (True if first.exists() else None, "the first frame is pending"), 10, 0.1)

    def mark(self, label: str, text: str):
        """Start a new segment on a verified cue so pending saves from the previous scene cannot become its still."""
        def drawn():
            tree = self.check_privacy()
            visible = " ".join(run["text"] for run in tree["texts"]
                               if run["visible"]["w"] > 0 and run["visible"]["h"] > 0)
            return (True if text in visible else None), "the cue text is pending"

        self.gui.poll(f"the {label.lower()} to paint", drawn, 10, 0.2)
        self.stop()
        self.start(mark=label)

    def stop(self):
        """Stop at a cut and wait for all frames the bridge promised before those PNGs can be used."""
        if self.frames is None:
            raise RuntimeError("no segment is recording")
        reply = self._request("record_stop")
        if reply["dir"] != str(self.frames) or reply["frames"] < 1 or not reply["seconds"] > 0:
            raise RuntimeError("bridge returned an empty or unrelated segment")
        files = tuple(self.frames / f"frame-{index:05d}.png" for index in range(reply["frames"]))

        def written():
            missing = next((path.name for path in files if not complete_png(path)), None)
            return (None, f"{missing} is not complete") if missing else (True, "all frames are complete")

        self.gui.poll("the stopped segment's PNGs to finish writing", written, 15, 0.1)
        if any(self.batch.png_size(path) != self.raw_size for path in files):
            raise RuntimeError("a recorded frame has the wrong size")
        self.check_privacy()
        self.segments.append(Segment(files, reply["seconds"], tuple(self.marks)))
        self.frames = None


def complete_png(path: Path) -> bool:
    """The bridge closes its writer after this fixed PNG trailer. Missing or half-written frames stay pending."""
    try:
        with path.open("rb") as stream:
            stream.seek(-12, os.SEEK_END)
            return stream.read() == b"\x00\x00\x00\x00IEND\xaeB\x60\x82"
    except OSError:
        return False


def timeline(segments: list[Segment]):
    """Give each frame a share of its segment's actual duration. Time between segments is absent from the video."""
    if not segments:
        raise ValueError("the video has no segments")
    lines = ["ffconcat version 1.0"]
    marks = []
    at = 0.0
    for segment in segments:
        if not segment.files or not math.isfinite(segment.seconds) or not segment.seconds > 0:
            raise ValueError("invalid recording segment")
        duration = segment.seconds / len(segment.files)
        for path in segment.files:
            # ffconcat's single quotes use shell-style escaping, but no shell sees the manifest.
            lines += [concat_file(path), "option framerate 1000", f"duration {duration:.9f}"]
        for mark in segment.marks:
            marks.append((mark.label, at + duration * mark.frame, segment.files[mark.frame]))
        at += segment.seconds
    # The last repeat ends the final hold. FFmpeg uses the next packet's timestamp to derive each hold.
    lines += [concat_file(segments[-1].files[-1]), "option framerate 1000"]
    return "\n".join(lines) + "\n", marks, at


def concat_file(path: Path) -> str:
    return "file '" + str(path).replace("'", "'\\''") + "'"


def encode_command(manifest: Path, movie: Path, clip: tuple[int, int], size: tuple[int, int]) -> list[str]:
    """Retain each marked frame at its millisecond timestamp and apply the same crop as the review stills."""
    x, y = clip
    width, height = size
    return ["ffmpeg", "-nostdin", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", str(manifest),
            "-vf", f"crop={width}:{height}:{x}:{y}", "-fps_mode", "vfr", "-enc_time_base", "1:1000",
            "-c:v", "libx264", "-crf", "18", "-preset", "fast", "-pix_fmt", "yuv420p", "-movflags",
            "+faststart", str(movie)]


def output_paths(movie: Path) -> tuple[Path, Path]:
    return movie.with_suffix(".marks.json"), movie.with_name(movie.stem + "-stills")


def check_output(movie: Path, marks: Path, stills: Path):
    """Only replace the stills directory if the previous marks account for every file in it."""
    if any(path.is_symlink() for path in (movie, marks, stills)):
        raise RuntimeError("refusing a symlink as a video output")
    for path in (movie, marks):
        if path.exists() and not path.is_file():
            raise RuntimeError(f"expected a regular file at the video output: {path}")
    if not stills.exists():
        return
    try:
        expected = {row["still"] for row in json.loads(marks.read_text())}
        actual = {f"{stills.name}/{file.name}" for file in stills.iterdir() if file.is_file() and not file.is_symlink()}
        if expected and expected == actual and len(actual) == len(list(stills.iterdir())):
            return
    except (OSError, ValueError, TypeError, KeyError):
        pass
    raise RuntimeError(f"the existing stills directory is not accounted for by the marks file: {stills}")


def install_output(movie: Path, marks: Path, stills: Path, staged: Path, backup: Path):
    """Serialize installs in the output directory and restore the old set if a rename fails."""
    def restore(source: Path, destination: Path):
        if source.exists():
            source.replace(destination)

    directory = os.open(movie.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        fcntl.flock(directory, fcntl.LOCK_EX)
        check_output(movie, marks, stills)
        with ExitStack() as rollback:
            for path in (stills, marks, movie):
                if path.exists():
                    saved = backup / path.name
                    rollback.callback(restore, saved, path)
                    path.replace(saved)
                source = staged / path.name
                rollback.callback(restore, path, source)
                source.replace(path)
            rollback.pop_all()
    finally:
        os.close(directory)


def write_output(recording: Recording, movie: Path):
    """Verify the cropped still for each mark and put finished output beside the old capture."""
    marks, stills = output_paths(movie)
    check_output(movie, marks, stills)
    manifest, positions, _ = timeline(recording.segments)
    movie.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".demo-video-", dir=movie.parent) as directory:
        work = Path(directory)
        listed = work / "frames.ffconcat"
        listed.write_text(manifest)
        staged, backup = work / "new", work / "old"
        backup.mkdir()
        staged_stills = staged / stills.name
        staged_stills.mkdir(parents=True)
        rows = []
        for number, (label, at, source) in enumerate(positions, 1):
            name = f"{number:02d}-{re.sub(r'[^a-z0-9]+', '-', label.lower()).strip('-')}.png"
            target = staged_stills / name
            shutil.copyfile(source, target)
            problem = recording.batch.crop_frame(target, recording.batch.LINUX_FRAME_MARGIN,
                                                  recording.batch.LINUX_FRAME_BORDER, recording.batch.LINUX_TITLE_BAR)
            if problem or recording.batch.png_size(target) != recording.size:
                raise RuntimeError(f"mark {label!r} cannot be cropped: {problem or 'wrong size'}")
            rows.append({"label": label, "at": round(at, 3), "still": f"{stills.name}/{name}"})
        staged_movie = staged / movie.name
        subprocess.run(encode_command(listed, staged_movie, recording.clip, recording.size), check=True)
        if not staged_movie.stat().st_size:
            raise RuntimeError("ffmpeg wrote an empty MP4")
        staged_marks = staged / marks.name
        staged_marks.write_text(json.dumps(rows, indent=2) + "\n")
        install_output(movie, marks, stills, staged, backup)
    for artifact in (movie, marks, stills):
        print(artifact)


class Cues:
    """Own the choreography's current overlay, leaving actual click and key dispatch to the bridge."""

    def __init__(self, recording: Recording):
        self.recording = recording
        self.bridge = recording.bridge
        self.state = {}
        self.serial = 0
        self.pointer = [1000, 540]

    def scene(self, **items):
        self.state.update(items)
        self.bridge.action(SCENE_ACTION, self.state)

    def next_key(self) -> int:
        self.serial += 1
        return self.serial

    def transition_ms(self, usual: int) -> int:
        """Allow the slowest recorder three samples across a fade or glide so the move shows up in its frames."""
        return max(usual, math.ceil(3000 / self.recording.fps))

    def content(self, box: dict):
        """Bridge geometry includes the frame and title bar. Shared callouts and Rust see the cropped content."""
        x, y = self.recording.clip
        return docs_shots.callouts.Rect(box["x"] - x, box["y"] - y, box["w"], box["h"])

    def annotations(self, *items, spotlight: bool = False, measured: dict | None = None):
        """Reuse the docs renderer's layout and send finished geometry, not placement instructions, to GPUI."""
        callouts = docs_shots.callouts
        self.bridge.forget_tree()
        targets = {item.target: self.content(measured[item.target] if measured and item.target in measured
                                            else self.bridge.box(item.target)) for item in items}
        shapes = []
        for item in callouts.layout(items, targets, *self.recording.size, theme=callouts.VIDEO_LAYOUT):
            r = item.bounds
            data = {"kind": item.kind, "rect": [r.x, r.y, r.w, r.h]}
            if item.kind == "ring":
                data["spotlight"] = spotlight
            else:
                control, wing1, wing2 = callouts.arrow(item.start, item.end, callouts.VIDEO_LAYOUT)
                data.update(lines=item.lines, arrow=[item.start, control, item.end, wing1, wing2])
            shapes.append(data)
        self.scene(annotations=shapes)

    def click(self, target: str):
        """Glide the drawn pointer while the real one stays parked, then press the real control and park again."""
        target = self.bridge.resolve(target)
        self.bridge.forget_tree()
        box = self.bridge.box(target)
        x, y = box["x"] + box["w"] / 2, box["y"] + box["h"] / 2
        where = [x - self.recording.clip[0], y - self.recording.clip[1]]
        duration = self.transition_ms(300)
        self.scene(pointer={"from": self.pointer, "to": where, "key": self.next_key(), "duration_ms": duration})
        hold(duration / 1000 + 0.15)
        click = json.loads(self.bridge._call("click", target, "--json"))
        self.park()
        if abs(click["x"] - x) > 1 or abs(click["y"] - y) > 1:
            raise RuntimeError(f"{target} moved while the pointer was gliding")
        self.pointer = where

    def park(self):
        """Reuse screenshot parking so a real hover cannot hold a terminal-link disclosure on screen."""
        entries = json.loads(self.bridge._call("snapshot", "-a", "--json", "--no-source"))["entries"]
        tree = json.loads(self.bridge._call("tree"))
        for target in self.recording.gui.park_targets(entries, tree):
            result = self.recording.gui.cli(self.bridge.run, self.bridge.record, ["hover", target], capture=True)
            if result.returncode == 0:
                return
        raise RuntimeError("the recorder could not park the pointer away from hover targets")

    def card(self, subtitle: str, *, leaving: bool = False):
        duration = self.transition_ms(350)
        self.scene(title={"subtitle": subtitle, "key": self.next_key(), "leaving": leaving, "duration_ms": duration})
        hold(duration / 1000 + 0.15)

    def wait_terminal(self, expected: str, timeout: float = 30):
        """The focused post-attach probe directly establishes readiness before the next terminal input."""
        def ready():
            current = self.recording.terminal_text()
            return (True if expected in current else None), "the focused terminal has not reached the replay line"

        self.recording.gui.poll(f"the focused terminal to show {expected!r}", ready, timeout, 0.2)

    def terminal_line(self, text: str) -> dict:
        """Join paint runs sharing a baseline. The terminal can paint one question in several runs."""
        def drawn():
            self.bridge.forget_tree()
            pane = self.bridge.box(f"id:{self.recording.gui.TERMINAL_ELEMENT}")
            tree = json.loads(self.bridge._call("tree"))
            rows = {}
            for run in tree["texts"]:
                box = run["visible"]
                if (box["w"] > 0 and box["h"] > 0 and pane["x"] <= box["x"] < pane["x"] + pane["w"]
                        and pane["y"] <= box["y"] < pane["y"] + pane["h"]):
                    rows.setdefault(round(box["y"]), []).append(run)
            matches = []
            for row in rows.values():
                ordered = sorted(row, key=lambda run: run["visible"]["x"])
                if text in "".join(run["text"] for run in ordered):
                    boxes = [run["visible"] for run in ordered if run["text"].strip()]
                    left = min(box["x"] for box in boxes)
                    top = min(box["y"] for box in boxes)
                    right = max(box["x"] + box["w"] for box in boxes)
                    bottom = max(box["y"] + box["h"] for box in boxes)
                    matches.append({"x": left, "y": top, "w": right - left, "h": bottom - top})
            return (matches[0] if len(matches) == 1 else None), f"{len(matches)} visible rows containing {text!r}"

        return self.recording.gui.poll("the question to paint in the terminal", drawn, 10, 0.25)


def hold(seconds: float):
    """Leave a cue or a transition on the recorded timeline long enough to read or see it."""
    time.sleep(seconds)


def record_demo(recording: Recording):
    """The eight sample beats in intent.md. Cut both waits that have no action for the viewer to watch."""
    cue = Cues(recording)
    bridge, gui = cue.bridge, recording.gui
    title = "migrate auth tokens"
    row = f"id:session-row-{{session:{title}}}"
    item = f"id:session-item-{{session:{title}}}"
    status = "session-details-status"
    question = "Do you want to apply the migration to staging?"
    opening = "Every coding agent, on every machine, in one place."
    session_list = "Every session on every host, with live status."
    waiting = "This one is waiting on you."
    terminal = "The agent's real terminal, right in the app."
    running = "Status follows the agent: working again."
    typing = "Type to it like you would locally."
    closing = "Watch your agents. Answer them from anywhere."
    callout = docs_shots.Callout
    highlight = docs_shots.Highlight

    bridge.click(f"id:{gui.TERMINAL_ELEMENT}")
    cue.park()
    opened = next(session for session in recording.plan["sessions"] if session["open"])
    cue.wait_terminal(gui.last_transcript_line(opened["transcript"]))
    recording.start()
    cue.card(opening)
    recording.mark("Opening card", opening)
    hold(1.5)
    cue.card(opening, leaving=True)
    cue.scene(title=None)

    cue.annotations(highlight("id:session-list", 2),
                    callout("id:session-list", session_list, "right", dy=160))
    recording.mark("Session list callout", session_list)
    hold(2.3)
    cue.annotations()

    cue.annotations(highlight(item, 4), callout(item, waiting, "right"), spotlight=True)
    recording.mark("Waiting session spotlight", waiting)
    hold(2.2)
    cue.annotations()

    cue.click(row)
    cue.wait_terminal(question)
    question_box = cue.terminal_line(question)
    cue.annotations(callout("question", terminal, "right", dy=32),
                    measured={"question": question_box})
    recording.mark("Terminal callout", terminal)
    hold(2.3)
    cue.annotations()

    cue.click(f"id:{gui.TERMINAL_ELEMENT}")
    cue.wait_terminal(question)
    bridge.press("1")
    cue.wait_terminal("./scripts/migrate --env staging 0042")
    hold(1.0)
    recording.stop()

    key = bridge.session_key(title)
    gui.wait_for_statuses(lambda: gui.listed_states(bridge.run), {key: "running"}, timeout=30)

    def running_badge():
        tree = json.loads(bridge._call("tree"))
        badge = next((node.get("label") for node in tree["nodes"] if node.get("id") == status), None)
        return (True if badge == "Running" else None), f"the app's status mark is named {badge!r}"

    gui.poll("the open session to draw its Running mark", running_badge, 10, 0.25)
    recording.start()
    cue.annotations(callout(f"id:{status}", running, "top", dx=115))
    recording.mark("Status callout", running)
    hold(2.3)
    cue.annotations()
    recording.stop()

    cue.wait_terminal("Anything else before production?", timeout=40)
    recording.start()
    cue.scene(caption=typing)
    recording.mark("Typing caption", typing)
    hold(0.5)
    for word in ("also ", "update ", "the ", "rollout ", "doc"):
        bridge._call("type", word)
        hold(0.15)
    bridge.press("enter")
    cue.wait_terminal("Done. The rollout doc now covers staging and rollback.")
    hold(1.5)
    cue.scene(caption=None)

    cue.card(closing)
    recording.mark("End card", closing)
    hold(2.0)
    recording.stop()
    cue.scene(annotations=[], title=None, pointer=None, caption=None)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=REPO / "target/readme-video/readme-video.mp4")
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--fps", type=int, default=FPS, choices=range(1, 31), metavar="1-30")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("demo recording uses the Linux headless desktop")
    if args.output.suffix != ".mp4":
        parser.error("--output must end in .mp4")
    encoders = subprocess.run(["ffmpeg", "-hide_banner", "-encoders"], capture_output=True, text=True, check=True)
    if "libx264" not in encoders.stdout:
        parser.error("ffmpeg needs the libx264 encoder")
    if "\n" in str(args.output):
        parser.error("--output must be one line")
    check_output(args.output, *output_paths(args.output))
    gui = docs_shots.load(HERE / "gui-agent.py")
    batch = docs_shots.load(HERE / "gui-screenshot-batch.py")
    plan = gui.load_scenario(SCENARIO, pwd.getpwuid(os.getuid()).pw_name)
    width, height = gui.parse_size(gui.scenario_window_size(plan))
    run_root = Path(tempfile.mkdtemp(prefix="fh-demo.", dir="/tmp"))
    run = run_root / "run"
    start = ["start", "--scenario", str(SCENARIO), "--appearance", "light", "--size",
             f"{width + batch.MONITOR_MARGIN}x{max(height + batch.MONITOR_MARGIN, 1200)}", "--scale", "1"]
    if args.no_build:
        start.append("--no-build")

    def interrupted(signum, _):
        raise KeyboardInterrupt(f"signal {signum}")

    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, interrupted)
    success = False
    try:
        if gui.main([str(run), *start]):
            return 1
        record = json.loads((run / "run.json").read_text())
        bridge = docs_shots.Bridge(gui, run, record, plan)
        windows = json.loads(bridge._call("windows", "--json"))
        if len(windows) != 1:
            raise RuntimeError("the staged run must display one window")
        capture = Recording(gui, batch, bridge, windows[0], plan, args.fps)
        record_demo(capture)
        write_output(capture, args.output)
        success = True
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, KeyboardInterrupt) as error:
        print(f"readme-video: {error}", file=sys.stderr)
    finally:
        if (run / "run.json").exists():
            success = gui.main([str(run), "stop"]) == 0 and success
        if success:
            shutil.rmtree(run_root)
        else:
            print(f"readme-video: kept the run at {run_root}", file=sys.stderr)
    return int(not success)


if __name__ == "__main__":
    sys.exit(main())
