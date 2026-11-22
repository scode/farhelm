"""Drive the real test-worker client and runner against a local SSH stand-in.

The fake worker has its own home and accepts one synthetic destination. Real
bootstrap and Rust/GUI runners are refused before their scripts can run.
"""

import fcntl
import hashlib
import os
from pathlib import Path
import re
import runpy
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import uuid


HARNESS = Path(__file__).resolve().parent
WAIT_SECONDS = 12
SSH = """#!/usr/bin/env python3
import os
from pathlib import Path
import sys

args = sys.argv[1:]
while args and args[0] in ("-o", "-l"):
    args = args[2:]
if not args or args[0] != "fixture-worker":
    sys.exit("fake ssh refuses every destination except fixture-worker")
home = Path(os.environ["FIXTURE_WORKER_HOME"])
command = " ".join(args[1:])
# Only the fixed launcher prefix needs inspecting. Bash handles its quoted payload verbatim.
words = command.split(maxsplit=5)
if not words or words[0] not in ("bash", "flock", "mkdir", "rm", "rsync", "test"):
    sys.exit("fake ssh refuses the worker command")
if words[0] == "rsync" and "/out/" in command and (home / "break-copy").exists():
    sys.exit("fixture broke evidence transport")
if words[0] == "bash" and words[1] not in ("-s", "-c"):
    script = home / words[1]
    if script.name != "with-lock.sh" or words[4] not in ("bootstrap", "run"):
        sys.exit("fake ssh refuses the real bootstrap or Rust/GUI runner")
    if not (script.parent / "bootstrap.sh").read_text().startswith("#!/bin/bash\\n# local stand-in"):
        sys.exit("fake ssh refuses to start with a real bootstrap script")
os.chdir(home)
os.execve("/bin/bash", ["/bin/bash", "-c", command], {**os.environ, "HOME": str(home)})
"""
FLOCK = """#!/bin/bash
# Show that an admission attempt actually found the held bootstrap lease.
case "$*" in
"8" | "-w 300 8")
  if [ -e "$HOME/hold-bootstrap" ] && [ ! -e "$HOME/release-bootstrap" ]; then
    if "$FIXTURE_FLOCK" -n 8; then
      "$FIXTURE_FLOCK" -u 8
    else
      printf '%s\\n' "$*" >>"$HOME/bootstrap-blocked"
    fi
  fi
  ;;
esac
# Model the transfer completing between the stale-directory scan and the lease attempt.
if [ "$1" = -n ] && [ -f "$HOME/refresh-stage" ] && [ "${2:-}" = "$(cat "$HOME/refresh-stage")/.lock" ]; then
  touch "$(cat "$HOME/refresh-stage")"
fi
exec "$FIXTURE_FLOCK" "$@"
"""
BOOTSTRAP = """#!/bin/bash
# local stand-in
set -uo pipefail
stage=$(dirname "$(dirname "$0")")
printf '%s\\n' "${stage##*/}" >>"$HOME/bootstrap-entered"
while [ -f "$HOME/hold-bootstrap" ] && [ ! -f "$HOME/release-bootstrap" ]; do
  sleep 0.02
done
sha256sum "$0" | cut -d' ' -f1 >"$HOME/farhelm-test-worker/bootstrapped"
"""
BLOCKING_RUN = """
printf '%s\n' "$$" >"$1"
while [ ! -e "$2" ]; do sleep 0.02; done
cat payload
"""


def digest(*roots):
    """Fingerprint the installed source, Git snapshot and tools, including their timestamps."""
    fingerprint = hashlib.sha256()
    for root in roots:
        paths = [root, *sorted(root.rglob("*"))] if root.is_dir() else [root]
        for path in paths:
            if path.is_file() and "target" not in path.relative_to(root).parts:
                fingerprint.update(os.fsencode(path))
                fingerprint.update(str(path.stat().st_mtime_ns).encode())
                fingerprint.update(path.read_bytes())
    return fingerprint.hexdigest()


class WorkloadStatTest(unittest.TestCase):
    def test_zombie_main_thread_does_not_hide_its_live_threads(self):
        """Check the proc stat shape for a process whose main thread has called pthread_exit."""
        is_live_group_member = runpy.run_path(str(HARNESS / "remote/run-workload.py"))["is_live_group_member"]
        fields = ["Z", "1", "456", *["0"] * 14, "2"]
        self.assertTrue(is_live_group_member("123 (thread) name) " + " ".join(fields), 456))
        self.assertFalse(is_live_group_member("123 (thread) name) " + " ".join(fields), 457))
        fields[17] = "1"
        self.assertFalse(is_live_group_member("123 (thread) name) " + " ".join(fields), 456))


@unittest.skipUnless(
    all(shutil.which(name, path=os.defpath) for name in ("setsid", "flock", "rsync")), "Linux worker tools required",
)
class WorkerTest(unittest.TestCase):
    """Each test owns the checkouts, worker home, running commands, and their cleanup signals."""

    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory(prefix="fh-test-worker.")))
        self.home = self.root / "worker"
        self.home.mkdir()
        self.worker = self.home / "farhelm-test-worker"
        fake_bin = self.root / "bin"
        fake_bin.mkdir()
        self.executable(fake_bin / "ssh", SSH)
        self.executable(fake_bin / "flock", FLOCK)
        for name in ("sudo", "cargo", "rustup", "systemctl", "podman", "tailscale"):
            self.executable(fake_bin / name, '#!/bin/bash\nprintf "%s\\n" "$0" >>"$FIXTURE_WORKER_HOME/unsafe"\nexit 99\n')
        self.config = self.root / "test-worker.conf"
        self.config.write_text("destination=fixture-worker\n")
        self.env = {
            "PATH": f"{fake_bin}:{os.defpath}",
            "HOME": str(self.root),
            "FIXTURE_WORKER_HOME": str(self.home),
            "FIXTURE_FLOCK": shutil.which("flock", path=os.defpath),
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
        }
        self.release_paths = [self.home / "release-bootstrap"]
        self.children = []
        self.addCleanup(self.finish_owned_jobs)

    @staticmethod
    def executable(path, content):
        path.write_text(content)
        path.chmod(0o700)

    def fixture(self, parent, tree, payload):
        """Two different worktrees may map to the same checkout name on the worker."""
        checkout = self.root / parent / tree
        checkout.mkdir(parents=True)
        (checkout / "payload").write_text(payload + "\n")
        stable = checkout / "stable.txt"
        stable.write_text("incremental input\n")
        os.utime(stable, (1_700_000_002, 1_700_000_002))
        (checkout / ".gitignore").write_text("target/\n")
        git = ["git", "-C", str(checkout)]
        subprocess.run([*git, "init", "-q"], check=True, env=self.env, timeout=WAIT_SECONDS)
        subprocess.run([*git, "add", "."], check=True, env=self.env, timeout=WAIT_SECONDS)
        subprocess.run(
            [*git, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"],
            check=True, env=self.env, timeout=WAIT_SECONDS,
        )
        tools = shutil.copytree(HARNESS, checkout / "tools", ignore=shutil.ignore_patterns("__pycache__"))
        self.executable(tools / "remote" / "bootstrap.sh", BOOTSTRAP)
        return checkout

    def start(self, checkout, *args):
        argv = ["bash", str(checkout / "tools/test-worker.sh"), args[0], "--checkout", str(checkout),
                "--config", str(self.config), *map(str, args[1:])]
        log = self.root / f"driver-{len(self.children)}.log"
        with log.open("wb") as output:
            process = subprocess.Popen(
                argv, env=self.env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True,
            )
        self.children.append((process, log))
        return process, log

    def run_to_exit(self, checkout, *args, expected=0):
        return self.join(self.start(checkout, *args), expected)

    def join(self, job, expected, timeout=WAIT_SECONDS):
        process, log = job
        try:
            status = process.wait(timeout)
        except subprocess.TimeoutExpired:
            self.fail(f"driver is still running after {timeout}s: {log.read_text()}")
        output = log.read_text()
        self.assertEqual(status, expected, output)
        return output

    def finish_owned_jobs(self):
        """Release fixtures even after a failed assertion so a detached test cannot outlive its home."""
        for path in self.release_paths:
            path.touch(exist_ok=True)
        for process, _ in self.children:
            try:
                process.wait(2)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(WAIT_SECONDS)
        self.wait_for(lambda: not list(self.worker.glob("*/lock.id")), "owned worker runs to leave their tree locks")
        self.assertFalse((self.home / "unsafe").exists(), "the stand-in reached a host prerequisite command")

    def wait_for(self, predicate, description, job=None):
        """Wait for fixture-visible readiness, preserving diagnostics from the live driver on failure."""
        deadline = time.monotonic() + WAIT_SECONDS
        while not predicate():
            if job and job[0].poll() is not None:
                self.fail(f"driver exited waiting for {description}: {job[1].read_text()}")
            if time.monotonic() >= deadline:
                details = job[1].read_text() if job else ""
                self.fail(f"timed out waiting for {description}: {details}")
            time.sleep(0.01)  # Bound polling of the filesystem readiness markers.

    def invocation(self, output):
        match = re.search(r"test-worker: invocation ([0-9a-f-]+)", output)
        self.assertIsNotNone(match, output)
        return match[1]

    def blocked_run(self, checkout, name):
        ready = self.root / f"{name}.ready"
        release = self.root / f"{name}.release"
        self.release_paths.append(release)
        job = self.start(checkout, "run", "--out", self.root / f"{name}.out", "--",
                         "bash", "-c", BLOCKING_RUN, "--", ready, release)
        self.wait_for(lambda: ready.exists() and ready.read_text().strip().isdigit(), "runner marker", job)
        invocation = self.invocation(job[1].read_text())
        os.kill(int(ready.read_text()), 0)
        return job, invocation, release

    def test_busy_request_cannot_touch_detached_source_or_evidence(self):
        """Hash the admitted run against a changed rival, then collect it after the client disconnects."""
        first = self.fixture("first", "tree", "first payload")
        second = self.fixture("second", "tree", "other payload")
        with (second / "tools/remote/env.sh").open("a") as tool:
            tool.write("# changed tool in the rejected invocation\n")
        job, active, release = self.blocked_run(first, "owned")
        tree = self.worker / "tree"
        paths = (tree / "src", tree / "source.txt", tree / "synced.list", tree / "stage" / active / "tools")
        self.assertEqual((tree / "lock.id").read_text().strip(), active)
        self.assertEqual((tree / "src/payload").read_text(), "first payload\n")
        self.assertTrue((tree / "src/.git/HEAD").is_file())
        before = digest(*paths)
        owned_out = self.root / "owned.out"
        owned_stamp = (owned_out / "invocation.txt").read_text()
        reuse = self.run_to_exit(second, "run", "--out", owned_out, "--", "false", expected=75)
        self.assertIn("output directory", reuse)
        self.assertEqual((owned_out / "invocation.txt").read_text(), owned_stamp)

        os.killpg(job[0].pid, signal.SIGTERM)
        self.join(job, 143)
        self.assertIn(active, self.run_to_exit(first, "status"))

        out = self.root / "reused"
        out.mkdir()
        (out / "runs.tar.gz").write_text("old failure evidence")
        (out / "notes").write_text("user file")
        busy = self.run_to_exit(second, "run", "--out", out, "--", "false", expected=75)
        refused = self.invocation(busy)
        self.assertIn(active, busy)
        self.assertNotEqual(active, refused)
        self.assertEqual(digest(*paths), before)
        self.assertFalse((tree / "stage" / refused).exists())
        self.assertFalse((tree / "out" / refused).exists())
        self.assertEqual({p.name for p in out.iterdir()}, {"notes", "invocation.txt"})
        self.assertIn(f"invocation: {refused}", (out / "invocation.txt").read_text())

        pending = self.run_to_exit(first, "collect", "--out", out, active, expected=75)
        self.assertIn("still running", pending)
        self.assertFalse((out / "console.log").exists())
        release.touch()
        self.wait_for(lambda: (tree / "out" / active / "finished").exists(), "detached evidence to finish")
        self.wait_for(lambda: not (tree / "lock.id").exists(), "detached run to release the tree")
        mtime = (tree / "src/stable.txt").stat().st_mtime_ns
        self.assertEqual(mtime, (first / "stable.txt").stat().st_mtime_ns)

        # Admit another run before collection. Its output must never replace the original's evidence.
        later = self.run_to_exit(second, "run", "--out", self.root / "later", "--", "cat", "payload")
        self.assertIn("other payload", later)
        self.assertEqual((tree / "src/stable.txt").stat().st_mtime_ns, mtime)
        self.run_to_exit(first, "collect", "--out", out, active)
        self.assertEqual((out / "console.log").read_text(), "first payload\n")
        self.assertIn((tree / "out" / active / "source").read_text().strip(), (out / "invocation.txt").read_text())
        self.assertEqual((out / "notes").read_text(), "user file")

    def test_bootstrap_and_running_tests_have_distinct_worker_admission(self):
        """A run holds a shared lease across its work. Standalone bootstraps queue only with other setup."""
        first = self.fixture("first", "tree-a", "one")
        second = self.fixture("second", "tree-b", "two")
        job, active, release = self.blocked_run(first, "active")
        busy = self.run_to_exit(second, "bootstrap", expected=75)
        self.assertIn(f"tree-a run {active}", busy)
        self.assertIn("status or stop", busy)
        release.touch()
        self.join(job, 0)

        (self.home / "hold-bootstrap").touch()
        entered = self.home / "bootstrap-entered"
        count = len(entered.read_text().splitlines())
        boot_a = self.start(first, "bootstrap")
        self.wait_for(lambda: len(entered.read_text().splitlines()) == count + 1, "first held bootstrap", boot_a)
        boot_b = self.start(second, "bootstrap")
        blocked = self.home / "bootstrap-blocked"
        self.wait_for(lambda: blocked.is_file() and "8" in blocked.read_text().splitlines(),
                      "second bootstrap to find the held setup lock", boot_b)
        next_id = self.invocation(boot_b[1].read_text())
        self.assertNotIn(next_id, entered.read_text(), "bootstrap ran while another setup was held")
        (self.home / "release-bootstrap").touch()
        self.join(boot_a, 0)
        self.join(boot_b, 0)
        self.assertIn(next_id, entered.read_text())

        (self.home / "release-bootstrap").unlink()
        count = len(entered.read_text().splitlines())
        boot = self.start(first, "bootstrap")
        self.wait_for(lambda: len(entered.read_text().splitlines()) == count + 1, "exclusive setup to start", boot)
        waiting = self.start(second, "run", "--out", self.root / "after-bootstrap", "--", "cat", "payload")
        tree = self.worker / "tree-b"
        self.wait_for(lambda: "-w 300 8" in blocked.read_text().splitlines(),
                      "run to find the held setup lock", waiting)
        self.assertTrue((tree / "lock.holder").is_file())
        self.assertFalse((tree / "src/payload").exists(), "source installed before worker admission")
        (self.home / "release-bootstrap").touch()
        self.join(boot, 0)
        self.assertIn("two", self.join(waiting, 0))

    def test_prune_keeps_newest_and_respects_a_live_staging_lease(self):
        """Retention bounds old output while an upload with an old directory timestamp keeps its stage."""
        checkout = self.fixture("first", "tree", "new")
        tree = self.worker / "tree"
        old_ids = [str(uuid.uuid4()) for _ in range(20)]
        now = int(time.time())
        for index, invocation in enumerate(old_ids):
            result = tree / "out" / invocation
            result.mkdir(parents=True)
            (result / "finished").write_text("0\n")
            os.utime(result, (now - 100 + index, now - 100 + index))
        live = tree / "stage" / str(uuid.uuid4())
        abandoned = tree / "stage" / str(uuid.uuid4())
        refreshed = tree / "stage" / str(uuid.uuid4())
        for stage in (live, abandoned, refreshed):
            stage.mkdir(parents=True)
            (stage / ".lock").touch()
            os.utime(stage, (now - 180_000, now - 180_000))
        (self.home / "refresh-stage").write_text(str(refreshed))
        with (live / ".lock").open() as lease:
            fcntl.flock(lease, fcntl.LOCK_EX)
            job, newest, release = self.blocked_run(checkout, "recent")
            self.assertTrue(live.is_dir())
            self.assertTrue(refreshed.is_dir())
            self.assertGreater(refreshed.stat().st_mtime, now - 5)
            self.assertFalse(abandoned.exists())
            self.assertEqual({path.name for path in (tree / "out").iterdir()}, {*old_ids[1:], newest})
            release.touch()
            self.join(job, 0)
        out = self.root / "recent.out"
        gone = self.run_to_exit(checkout, "collect", "--out", out, old_ids[0], expected=1)
        self.assertIn("unknown or pruned invocation", gone)
        self.assertFalse((out / "summary.txt").exists())

    def test_a_descendant_cannot_keep_a_worker_lease_after_its_run(self):
        """A child that inherits descriptors must not keep setup locked after the command exits."""
        first = self.fixture("first", "tree-a", "one")
        second = self.fixture("second", "tree-b", "two")
        pid_file = self.root / "descendant"
        leak = (
            "import pathlib, subprocess, sys; "
            "p = subprocess.Popen(['sleep', '60'], close_fds=False, start_new_session=True, "
            "stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL); "
            "pathlib.Path(sys.argv[1]).write_text(str(p.pid))"
        )
        job = self.start(first, "run", "--out", self.root / "leak", "--", sys.executable, "-c", leak, pid_file)
        self.wait_for(lambda: pid_file.exists() and pid_file.read_text().isdigit(), "descendant PID", job)
        pid = int(pid_file.read_text())
        self.addCleanup(os.kill, pid, signal.SIGTERM)
        os.kill(pid, 0)
        self.join(job, 0)
        self.run_to_exit(second, "bootstrap")
        os.kill(pid, 0)

    def test_copy_failure_preserves_worker_exit_and_allows_collection(self):
        """The command result survives a transport failure, and collect can retrieve that run's evidence."""
        checkout = self.fixture("first", "tree", "fixture")
        out = self.root / "retry-copy"
        failure = self.home / "break-copy"
        failure.touch()
        result = self.run_to_exit(checkout, "run", "--out", out, "--",
                                  "bash", "-c", "echo retained; exit 7", expected=7)
        invocation = self.invocation(result)
        self.assertIn("could not copy evidence", result)
        self.assertIn("copy failed (command exit 7)", (out / "invocation.txt").read_text())
        self.assertFalse((out / "console.log").exists())
        failure.unlink()
        self.run_to_exit(checkout, "collect", "--out", out, invocation)
        self.assertEqual((out / "console.log").read_text(), "retained\n")

    def test_stop_releases_only_the_requested_checkout(self):
        """Two checkouts can run together, and Stop targets the process group recorded for its checkout."""
        first = self.fixture("first", "tree-a", "one")
        second = self.fixture("second", "tree-b", "two")
        stopped, _, _ = self.blocked_run(first, "to-stop")
        surviving, survivor, release = self.blocked_run(second, "to-keep")
        self.assertIn("ended", self.run_to_exit(first, "stop"))
        self.join(stopped, 143)
        self.assertIn(survivor, self.run_to_exit(second, "status"))
        self.assertIsNone(surviving[0].poll())
        release.touch()
        self.join(surviving, 0)

    def test_stop_keeps_both_leases_through_recorder_teardown(self):
        """A stopped recorder can still use the checkout until it has written its final evidence."""
        first = self.fixture("first", "tree-a", "original")
        second = self.fixture("second", "tree-b", "other tree")
        ready = self.root / "recorder-ready"
        ending = self.root / "recorder-ending"
        release = self.root / "release-recorder"
        self.release_paths.append(release)
        recorder = """
import pathlib, signal, sys, time
ready, ending, release = map(pathlib.Path, sys.argv[1:])
signal.signal(signal.SIGTERM, lambda *_: ending.touch())
ready.touch()
while not release.exists():
    time.sleep(0.01)
print(pathlib.Path("payload").read_text().strip(), flush=True)
"""
        out = self.root / "canceled"
        job = self.start(first, "run", "--out", out, "--", sys.executable, "-c", recorder, ready, ending, release)
        self.wait_for(ready.exists, "foreground recorder to install its signal handler", job)
        active = self.invocation(job[1].read_text())
        stopper = self.start(first, "stop")
        self.wait_for(ending.exists, "recorder to start stopping", stopper)
        busy = self.run_to_exit(first, "run", "--out", self.root / "busy", "--", "false", expected=75)
        self.assertIn(active, busy)
        busy_setup = self.run_to_exit(second, "bootstrap", expected=75)
        self.assertIn(active, busy_setup)
        collecting = self.run_to_exit(first, "collect", "--out", self.root / "early", active, expected=75)
        self.assertIn("still running", collecting)
        self.assertIsNone(stopper[0].poll())
        release.touch()
        self.join(stopper, 0)
        self.join(job, 143)
        self.assertEqual((out / "console.log").read_text(), "original\n")

    def test_forced_stop_leaves_a_collectable_invocation(self):
        """Stop escalates against a same-group child after its foreground shell has exited."""
        checkout = self.fixture("first", "tree", "owned")
        ready = self.root / "forced-ready"
        term_seen = self.root / "forced-term"
        child = """
import os, pathlib, signal, sys, time
ready, term_seen = map(pathlib.Path, sys.argv[1:])
signal.signal(signal.SIGTERM, lambda *_: term_seen.touch())
ready.write_text(str(os.getpid()))
while True:
    time.sleep(0.1)
"""
        # The foreground shell exits on TERM while its ordinary background child continues to use the tree.
        job = self.start(checkout, "run", "--out", self.root / "forced", "--",
                         "bash", "-c", '"$@" & wait', "--", sys.executable, "-c", child, ready, term_seen)
        self.wait_for(lambda: ready.exists() and ready.read_text().isdigit(), "same-group child to start", job)
        invocation = self.invocation(job[1].read_text())
        child_pid = int(ready.read_text())
        pgid = os.getpgid(child_pid)
        self.assertNotEqual(pgid, child_pid)
        child_fd = os.pidfd_open(child_pid)
        self.addCleanup(os.close, child_fd)

        def kill_if_alive():
            """Release only this owned child if an assertion interrupts the fixture."""
            try:
                signal.pidfd_send_signal(child_fd, signal.SIGKILL)
            except ProcessLookupError:
                pass

        self.addCleanup(kill_if_alive)
        stopper = self.start(checkout, "stop")
        self.wait_for(term_seen.exists, "child to acknowledge TERM and remain alive", stopper)

        def runner_exited():
            """Distinguish the leader's exit from the child's still-running teardown."""
            try:
                stat = Path(f"/proc/{pgid}/stat").read_text()
            except FileNotFoundError:
                return True
            return stat.rsplit(") ", 1)[1].split()[0] == "Z"

        self.wait_for(runner_exited, "workload leader to exit before the child", stopper)
        self.assertFalse(select.select([child_fd], [], [], 0)[0])
        tree = self.worker / "tree"
        result = tree / "out" / invocation
        busy = self.run_to_exit(checkout, "run", "--out", self.root / "refused", "--", "false", expected=75)
        self.assertIn(invocation, busy)
        self.assertFalse((result / "finished").exists())
        self.assertIn(invocation, self.join(stopper, 0, timeout=45))
        self.assertTrue(select.select([child_fd], [], [], 0)[0])
        self.join(job, 137)
        self.assertEqual((result / "finished").read_text(), "137\n")
        self.assertFalse((tree / "stage" / invocation).exists())
        self.run_to_exit(checkout, "collect", "--out", self.root / "final", invocation)


if __name__ == "__main__":
    unittest.main()
