#!/usr/bin/env python3
"""Exercise real nextest group cleanup in an isolated Linux worker fixture.

This compiles a dependency-free Rust test binary, then makes its selected test
and child ignore SIGTERM. The recorder must give nextest enough time to kill
that test group. Run on a sandbox when runner lifecycle or policy changes;
this is not an added per-PR or release stress gate.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time


RECORDER = pathlib.Path(__file__).with_name("record-test-run.py").resolve()
CONFIG = RECORDER.parent.parent / ".config" / "nextest.toml"
HOLDER = '''import json, os, pathlib, signal, subprocess, sys, time
root = pathlib.Path(__file__).parent
signal.signal(signal.SIGTERM, signal.SIG_IGN)
role = "child" if "--child" in sys.argv else "test"
child = None
if role == "test":
    child = subprocess.Popen([sys.executable, __file__, "--child"])
identity = {"pid": os.getpid(), "parent": os.getppid(), "group": os.getpgrp(),
            "start": pathlib.Path("/proc/self/stat").read_text().rsplit(")", 1)[1].split()[19]}
temporary = root / (role + ".tmp")
temporary.write_text(json.dumps(identity))
temporary.replace(root / (role + ".json"))
deadline = time.monotonic() + 45
while root.exists() and not (root / "stop").exists() and time.monotonic() < deadline:
    time.sleep(0.02)
# Publish before a voluntary exit: a delayed checker must not mistake the
# emergency lease for nextest's forced process-group cleanup.
(root / (role + ".voluntary")).touch()
if child is not None:
    child.wait(timeout=5)
'''
RUST = '''/// Keep both the selected test and its child alive until nextest kills their group.
/// Replacing this process preserves nextest's test PID while installing explicit signal behavior.
#[test]
fn holds_its_process_group() {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/hold.py")).exec();
    panic!("fixture exec failed: {error}");
}
'''


def same_live_process(identity: dict) -> bool:
    """Inspect an observed Linux process instance without acquiring signal authority."""

    try:
        fields = pathlib.Path(f"/proc/{identity['pid']}/stat").read_text().rsplit(")", 1)[1].split()
    except FileNotFoundError:
        return False
    return fields[19] == identity["start"] and fields[0] != "Z"


def run_setup(argv: list[str], cwd: pathlib.Path) -> None:
    """Bound fixture setup separately from the runner timeout being exercised."""

    subprocess.run(argv, cwd=cwd, check=True, timeout=60)


def assert_forced_cleanup(root: pathlib.Path, identities: list[dict]) -> None:
    """Accept dead instances only when neither holder used its voluntary fallback."""

    if any(same_live_process(identity) for identity in identities):
        raise AssertionError("nextest left a SIGTERM-resistant group member alive")
    if any((root / f"{role}.voluntary").exists() for role in ("test", "child")):
        raise AssertionError("a fixture exited voluntarily; this does not prove runner cleanup")


def main() -> None:
    """Retain the real runner receipt and fail if either group member survives cleanup."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=pathlib.Path, required=True)
    parser.add_argument("--trigger", choices=("recorder", "test"), default="recorder")
    args = parser.parse_args()
    if not pathlib.Path("/proc/self/stat").exists():
        parser.error("this cleanup fixture requires Linux process-instance observations")
    with tempfile.TemporaryDirectory(prefix="nextest-cleanup-") as directory:
        root = pathlib.Path(directory)
        (root / ".config").mkdir()
        shutil.copyfile(CONFIG, root / ".config" / "nextest.toml")
        if args.trigger == "test":
            # Exercise nextest's own deadline without waiting five minutes.
            # Only this fixture's execution period changes; signal grace and
            # group policy remain the actual maintained configuration.
            config_path = root / ".config" / "nextest.toml"
            config = config_path.read_text()
            original = 'period = "60s", terminate-after = 5'
            if config.count(original) != 1:
                raise AssertionError("fixture timeout adaptation needs updating for the current policy")
            config_path.write_text(config.replace(original, 'period = "1s", terminate-after = 1'))
        (root / "Cargo.toml").write_text(
            '[package]\nname = "farhelm"\nversion = "0.0.0"\nedition = "2024"\n'
            '[[test]]\nname = "e2e"\npath = "fixture.rs"\n', encoding="utf-8")
        # Match the real policy's package/binary selectors. This tiny crate
        # contains no product code; it exercises the same nextest group policy.
        (root / "fixture.rs").write_text(RUST, encoding="utf-8")
        (root / "hold.py").write_text(HOLDER, encoding="utf-8")
        run_setup(["git", "init", "--quiet"], root)
        run_setup(["git", "add", "."], root)
        run_setup(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                   "commit", "--quiet", "-m", "fixture"], root)
        # Build before starting the execution deadline: compiler scheduling
        # must not replace the intended live-test interruption premise.
        run_setup(["cargo", "test", "--no-run", "--offline"], root)
        command = [sys.executable, str(RECORDER), "--runner", "nextest", "--kind", "development",
                   "--selection", f"real nextest {args.trigger} timeout; SIGTERM-resistant test and child",
                   "--concurrency", "4 nextest slots; one fixture test", "--tmux", "none",
                   "--timeout", "5" if args.trigger == "recorder" else "30",
                   "--output-root", str(args.output_root.resolve()),
                   "--", "cargo", "nextest", "run", "--test", "e2e"]
        recorder = subprocess.Popen(command, cwd=root, start_new_session=True)
        identities = []
        try:
            deadline = time.monotonic() + 10
            while not all((root / f"{role}.json").exists() for role in ("test", "child")):
                if time.monotonic() >= deadline:
                    raise AssertionError("runner never established the live test/child premise")
                time.sleep(0.02)
            identities = [json.loads((root / f"{role}.json").read_text()) for role in ("test", "child")]
            if (identities[0]["group"] != identities[1]["group"]
                    or identities[0]["group"] != identities[0]["pid"]
                    or identities[0]["group"] == identities[0]["parent"]):
                raise AssertionError("fixture must occupy a nextest-owned group outside the recorder group")
            if not all(same_live_process(identity) for identity in identities):
                raise AssertionError("both fixture processes must be live before the timeout")
            status = recorder.wait(timeout=20)
            expected = 124 if args.trigger == "recorder" else 100
            if status != expected:
                raise AssertionError(f"expected {args.trigger} timeout status {expected}, got {status}")
            assert_forced_cleanup(root, identities)
            print("PASS: real nextest killed its test group before recorder cleanup ended", flush=True)
        finally:
            # A separate private lease releases survivors if the assertion or
            # runner fails. Remembered PIDs never authorize fallback signals.
            (root / "stop").touch()
            try:
                recorder.wait(timeout=10)
            except subprocess.TimeoutExpired:
                recorder.kill()
                recorder.wait(timeout=5)
            deadline = time.monotonic() + 5
            while any(same_live_process(identity) for identity in identities) and time.monotonic() < deadline:
                time.sleep(0.02)


if __name__ == "__main__":
    main()
