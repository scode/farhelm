#!/usr/bin/env python3
"""Exercise Farhelm's replay/live boundary against one installed tmux.

This is not a replacement for the Rust integration test. It is a small,
dependency-free compatibility probe meant to be mounted into containers with
the oldest supported tmux packages:

    python3 scripts/check-tmux-cutover.py

The probe keeps a pane busy with numbered records, attaches a control client
with `no-output`, and submits capture plus output-enable as one command group.
It checks the protocol properties the production parser depends on, including
the rule that a failed capture prevents the later enable command from running.
"""

from __future__ import annotations

import argparse
import dataclasses
import pathlib
import queue
import re
import signal
import subprocess
import tempfile
import threading
import time


NUMBER = re.compile(rb"CUTOVER-(\d{8})")


@dataclasses.dataclass
class BlockEvent:
    """One complete tmux command reply."""

    identity: tuple[int, int, int]
    ending: bytes
    body: list[bytes]


@dataclasses.dataclass
class LineEvent:
    """One asynchronous control-mode notification."""

    line: bytes



def unescape(payload: bytes) -> bytes:
    """Undo the octal escaping used by tmux `%output` notifications."""
    result = bytearray()
    offset = 0
    while offset < len(payload):
        if (
            payload[offset : offset + 1] == b"\\"
            and offset + 3 < len(payload)
            and payload[offset + 1 : offset + 2] in b"0123"
            and all(byte in b"01234567" for byte in payload[offset + 2 : offset + 4])
        ):
            result.append(int(payload[offset + 1 : offset + 4], 8))
            offset += 4
        else:
            result.append(payload[offset])
            offset += 1
    return bytes(result)


class ControlClient:
    """A raw control client whose reader preserves protocol line order."""

    def __init__(self, tmux: str, socket: pathlib.Path, config: pathlib.Path):
        self.process = subprocess.Popen(
            [
                tmux,
                "-S",
                str(socket),
                "-f",
                str(config),
                "-C",
                "attach",
                "-f",
                "no-output",
                "-t",
                "cutover",
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
        self.received: queue.Queue[bytes | None] = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self) -> None:
        assert self.process.stdout is not None
        for line in self.process.stdout:
            self.received.put(line)
        self.received.put(None)

    def send(self, command: str) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(command.encode() + b"\n")
        self.process.stdin.flush()

    def lines_for(self, seconds: float) -> list[bytes]:
        deadline = time.monotonic() + seconds
        lines: list[bytes] = []
        while time.monotonic() < deadline:
            try:
                line = self.received.get(timeout=deadline - time.monotonic())
            except queue.Empty:
                break
            if line is None:
                break
            lines.append(line)
        return lines

    def close(self) -> None:
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGKILL)
            self.process.wait()


def marker(line: bytes) -> tuple[bytes, tuple[int, int, int]] | None:
    """Parse the exact numeric shape of a control command marker."""
    fields = line.rstrip(b"\r\n").split(b" ")
    if len(fields) != 4 or fields[0] not in (b"%begin", b"%end", b"%error"):
        return None
    try:
        identity = (int(fields[1]), int(fields[2]), int(fields[3]))
    except ValueError:
        return None
    return fields[0], identity


def parse_events(lines: list[bytes]) -> list[BlockEvent | LineEvent]:
    """Separate command blocks from asynchronous notifications."""
    events: list[BlockEvent | LineEvent] = []
    block_id: tuple[int, int, int] | None = None
    body: list[bytes] = []
    for line in lines:
        parsed = marker(line)
        if block_id is None:
            if parsed is not None and parsed[0] == b"%begin":
                block_id = parsed[1]
                body = []
            else:
                events.append(LineEvent(line))
            continue
        if parsed is not None and parsed[1] == block_id and parsed[0] in (b"%end", b"%error"):
            events.append(BlockEvent(block_id, parsed[0], body))
            block_id = None
            body = []
        else:
            body.append(line)
    if block_id is not None:
        raise AssertionError(f"unterminated command block {block_id}")
    return events


def output_numbers(lines: list[bytes]) -> list[int]:
    """Recover numbered records from `%output`, including split records."""
    stream = bytearray()
    for line in lines:
        if not line.startswith(b"%output "):
            continue
        fields = line.rstrip(b"\r\n").split(b" ", 2)
        if len(fields) == 3:
            stream.extend(unescape(fields[2]))
    return [int(match) for match in NUMBER.findall(stream)]


def run_trial(client: ControlClient, pane: str, history: int) -> None:
    """Require snapshot and live records to meet exactly once."""
    client.send(
        f"capture-pane -p -e -N -t {pane} -S -{history} ; "
        "refresh-client -f !no-output"
    )
    events = parse_events(client.lines_for(0.6))
    blocks = [event for event in events if isinstance(event, BlockEvent)]
    assert len(blocks) == 2, f"expected two command blocks, got {len(blocks)}"
    assert all(
        not line.startswith(b"%output ")
        for event in blocks
        for line in event.body
    ), "%output appeared inside a command block"

    snapshot = b"".join(blocks[0].body)
    snapshot_numbers = [int(match) for match in NUMBER.findall(snapshot)]
    assert snapshot_numbers, "capture contained no numbered records"
    cutover_index = events.index(blocks[-1])
    live_lines = [
        event.line
        for event in events[cutover_index + 1 :]
        if isinstance(event, LineEvent)
    ]
    live_numbers = output_numbers(live_lines)
    assert live_numbers, "enabling output produced no live records"
    expected_first = snapshot_numbers[-1] + 1
    assert live_numbers[0] == expected_first, (
        f"snapshot ended at {snapshot_numbers[-1]}, live began at {live_numbers[0]}"
    )
    assert live_numbers == list(range(live_numbers[0], live_numbers[-1] + 1)), (
        "live output contained a gap or duplicate"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tmux", default="tmux", help="tmux executable to probe")
    parser.add_argument("--trials", type=int, default=15)
    parser.add_argument("--history", type=int, default=2_000)
    args = parser.parse_args()

    version = subprocess.run(
        [args.tmux, "-V"], check=True, capture_output=True, text=True
    ).stdout.strip()
    with tempfile.TemporaryDirectory(prefix="farhelm-tmux-cutover-") as directory:
        root = pathlib.Path(directory)
        socket = root / "tmux.sock"
        config = root / "tmux.conf"
        config.write_text(
            "set -s exit-empty off\n"
            "set -g status off\n"
            "set -g history-limit 12000\n"
            "setw -g remain-on-exit on\n"
        )
        producer = (
            "i=0; while :; do printf 'CUTOVER-%08d\\n' \"$i\"; "
            "i=$((i+1)); sleep 0.002; done"
        )
        created = subprocess.run(
            [
                args.tmux,
                "-S",
                str(socket),
                "-f",
                str(config),
                "new-session",
                "-d",
                "-P",
                "-F",
                "#{pane_id}",
                "-s",
                "cutover",
                "-x",
                "80",
                "-y",
                "24",
                "sh",
                "-c",
                producer,
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        pane = created.stdout.strip()
        try:
            time.sleep(0.3)
            for trial in range(args.trials):
                client = ControlClient(args.tmux, socket, config)
                try:
                    handshake = client.lines_for(0.2)
                    assert not any(
                        line.startswith(b"%output ") for line in handshake
                    ), "no-output leaked or queued pane output"
                    run_trial(client, pane, args.history)
                finally:
                    client.close()

            failed = ControlClient(args.tmux, socket, config)
            try:
                failed.lines_for(0.2)
                failed.send(
                    "capture-pane -p -t %999999 ; refresh-client -f !no-output"
                )
                events = parse_events(failed.lines_for(0.3))
                blocks = [event for event in events if isinstance(event, BlockEvent)]
                assert len(blocks) == 1 and blocks[0].ending == b"%error", (
                    "failed capture did not stop the command group"
                )
                assert not any(
                    isinstance(event, LineEvent)
                    and event.line.startswith(b"%output ")
                    for event in events
                ), "refresh-client ran after the failed capture"
            finally:
                failed.close()
        finally:
            subprocess.run(
                [args.tmux, "-S", str(socket), "kill-server"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )

    print(f"{version}: {args.trials} cutover trials passed")


if __name__ == "__main__":
    main()
