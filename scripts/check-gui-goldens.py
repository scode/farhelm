#!/usr/bin/env python3
"""Run the GPUI app's screenshot goldens under a renderer that gives the same pixels on every machine.

The goldens (`crates/gui/farhelm-gui/src/goldens/`) compare captures pixel for pixel, which only holds when every run
draws with the same rasterizer. On Linux that is Mesa's software Vulkan driver (lavapipe): this script points the
Vulkan and EGL loaders at Mesa's software drivers so a machine's real GPU, if it has one, is never picked. It also gives
the run a private, empty runtime directory, so the headless app cannot find a display server or a compositor's socket
from the session it was started in.

The reference pictures live outside the repository (`scripts/gui-goldens.py` says where and why). When
`FARHELM_GOLDENS_DIR` is already set, the run compares against that directory and fetches nothing: that is how a
machine without access to the storage repository runs the goldens, with references fetched elsewhere and copied over.
Otherwise this script runs `gui-goldens.py fetch` first, which needs read access to the storage repository, and points
the test at what it wrote.

The test is ignored in ordinary runs because it needs this environment; running it here is what un-ignores it. Pass
`--update` after an intended visual change: the run then compares nothing and writes every capture to
`target/gui-goldens/captures`, and `python3 scripts/gui-goldens.py publish` pushes the changed pictures and rewrites the
manifest's lines. The captures are tied to the manifest they were taken against, so publish before rebasing, or run
`--update` again after. Look at every picture its report names as changed or added before committing the manifest: the store
only grows, so a wrong picture that was pushed costs nothing, but a manifest line naming it makes it the reference.
Pass `--scene 'settings-*'` to compare or update only matching scenes. Repeat it to select the union of several
patterns. A selected update writes to `target/gui-goldens/selected-captures` for inspection. Publishing uses captures
from a full `--update` run.

Rendering uses up to four worker processes. Pass `--jobs 1` to run in one process.

The render has a wall-clock deadline (`--render-timeout`, 30 minutes by default). A renderer still running at the
deadline is killed, its partial output is printed with the appearance it was stuck on, and the run fails with status
124 (as `timeout(1)` does) after reporting the missing renders like any other failed ones. The harness's render budget
(`MAX_SCENE_RENDERS` in the goldens) already fails a fixture that redraws forever within seconds; the deadline is the
backstop for a hang that does not redraw.

Extra Cargo build options go before `--`. Only libtest output options (`--nocapture`, `--show-output`, `--quiet`)
go after it so a second test filter cannot quietly skip the render.

Compare mode keeps only the differing or unreferenced captures in the run's own directory under
`target/gui-goldens/`. Automatically fetched references live with that run so another comparison cannot replace
them mid-render. The test prints each retained capture's path beside its failure.
Differences, failed renders and missing appearances are reported in a portable review under `target/gui-goldens/changes/`.
"""

from __future__ import annotations

import argparse
import fnmatch
import importlib.util
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from contextlib import ExitStack, nullcontext
from functools import partial
from pathlib import Path
from typing import TypeVar

import golden_changes_page

REPO = Path(__file__).resolve().parent.parent
GUI_CRATE = REPO / "crates/gui/farhelm-gui"
SCENE_INVENTORY = "FARHELM_GOLDEN_SCENES="
Item = TypeVar("Item")
STOP_STATUSES = (128 + signal.SIGINT, 128 + signal.SIGTERM)
# Distinct from STOP_STATUSES on purpose: a timed-out render still gets its missing appearances reported and its changes
# page, which an operator's stop does not.
TIMEOUT_STATUS = 124

# A full-catalog Linux render took about 70 seconds per job with four jobs on 2026-11-25 (73, 71 and 71 s), so about
# five minutes in one process. There are no macOS or `--jobs 1` measurements. Thirty minutes leaves several times that
# for a single process on a slower or loaded machine, and still bounds a hang that the render budget misses to half an
# hour instead of the hours the two hangs that day ran for.
DEFAULT_RENDER_TIMEOUT = 30 * 60.

# Mesa's software drivers, at the paths Debian and Ubuntu install them to (`mesa-vulkan-drivers`, `libegl-mesa0`).
LAVAPIPE_ICD = Path("/usr/share/vulkan/icd.d/lvp_icd.json")
MESA_EGL_VENDOR = Path("/usr/share/glvnd/egl_vendor.d/50_mesa.json")


class ReviewInterrupted(Exception):
    """Carry an operator's cancellation through page staging without certifying an update."""

    def __init__(self, status: int):
        super().__init__(status)
        self.status = status


def load_goldens_tool():
    """`scripts/gui-goldens.py` as a module (its name is not importable), for its fetch and default paths."""
    spec = importlib.util.spec_from_file_location("gui_goldens", REPO / "scripts/gui-goldens.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def golden_env(
    base: dict[str, str], runtime_dir: str, references: Path | None, captures: Path
) -> dict[str, str]:
    """Give each child a software renderer, an isolated display environment, and this run's paths."""
    env = {key: value for key, value in base.items() if key not in ("WAYLAND_DISPLAY", "DISPLAY")}
    env["VK_ICD_FILENAMES"] = str(LAVAPIPE_ICD)
    env["__EGL_VENDOR_LIBRARY_FILENAMES"] = str(MESA_EGL_VENDOR)
    env["LIBGL_ALWAYS_SOFTWARE"] = "true"
    env["XDG_RUNTIME_DIR"] = runtime_dir
    env.pop("FARHELM_UPDATE_GOLDENS", None)
    env.pop("FARHELM_GOLDENS_DIR", None)
    env.pop("FARHELM_GOLDEN_CAPTURE_DIR", None)
    env.pop("FARHELM_GOLDEN_SCENES", None)
    if references is None:
        env["FARHELM_UPDATE_GOLDENS"] = str(captures)
    else:
        env["FARHELM_GOLDENS_DIR"] = str(references)
        env["FARHELM_GOLDEN_CAPTURE_DIR"] = str(captures)
    return env


def build_binary(base_env: dict[str, str], cargo_args: list[str]) -> Path:
    """Build once and locate the unit test executable in Cargo's artifact records, including custom targets."""
    command = [
        "cargo", "test", "--locked", "--no-run", "-p", "farhelm-gui", "--lib",
        "--message-format=json-render-diagnostics", *cargo_args,
    ]
    built = subprocess.run(command, cwd=REPO, env=base_env, text=True, stdout=subprocess.PIPE, check=True)
    binaries = set()
    for line in built.stdout.splitlines():
        if not line.startswith("{"):
            continue
        artifact = json.loads(line)
        if (artifact.get("reason") == "compiler-artifact"
            and artifact["target"]["name"] == "farhelm_gui"
            and "lib" in artifact["target"]["kind"]
            and artifact["profile"]["test"]
            and artifact["executable"]):
            binaries.add(Path(artifact["executable"]))
    if len(binaries) != 1:
        raise ValueError("Cargo did not report one farhelm-gui unit test executable")
    return binaries.pop()


def list_scenes(binary: Path, base_env: dict[str, str]) -> list[str]:
    """Ask this exact test binary for its ordered scene names without starting a headless app."""
    listing = subprocess.run(
        [str(binary), "--ignored", "--exact", "goldens::list_scenes", "--nocapture"],
        cwd=GUI_CRATE, env=base_env, text=True, stdout=subprocess.PIPE, check=True,
    )
    inventories = [line.removeprefix(SCENE_INVENTORY) for line in listing.stdout.splitlines()
                   if line.startswith(SCENE_INVENTORY)]
    if len(inventories) != 1:
        raise ValueError("the golden test did not report one scene inventory")
    names = json.loads(inventories[0])
    if (not isinstance(names, list) or not names
        or not all(isinstance(name, str) and re.fullmatch(r"[a-z0-9][a-z0-9-]*", name) for name in names)
        or len(set(names)) != len(names)):
        raise ValueError("the golden test reported an empty, invalid or duplicate scene inventory")
    return names


def select_scenes(names: list[str], patterns: list[str]) -> list[str]:
    """Reject a typo in any pattern, even if another pattern matches, and keep catalog order across overlaps."""
    if not patterns:
        return names
    selected = set()
    for pattern in patterns:
        matches = [name for name in names if fnmatch.fnmatchcase(name, pattern)]
        if not matches:
            raise ValueError(f"no golden scene matches {pattern!r}")
        selected.update(matches)
    return [name for name in names if name in selected]


def complete_update(captures: Path, platform: str, names: list[str]) -> bool:
    """Certify only the full catalog. A test invocation that exits zero without drawing is still incomplete."""
    expected = {f"{name}--{appearance}.png" for name in names for appearance in ("light", "dark")}
    pictures = {image.name for image in (captures / platform).glob("*--*.png") if image.is_file()}
    if expected != pictures:
        print(f"incomplete golden update under {captures}: "
              f"{len(expected - pictures)} missing, {len(pictures - expected)} unexpected", file=sys.stderr)
        return False
    (captures / f"{platform}.complete").write_text(f"{len(pictures)}\n", encoding="utf-8")
    return True


def incomplete_appearances(outcomes: Path, captures: Path, names: list[str], update: bool) -> dict[str, str]:
    """Check each requested appearance against its receipt. A matching comparison needs no retained PNG."""
    problems = {}
    expected_statuses = {"rendered"} if update else {"matched", "different"}
    for name in names:
        for appearance in ("light", "dark"):
            stem = f"{name}--{appearance}"
            try:
                record = json.loads((outcomes / f"{stem}.json").read_text(encoding="utf-8"))
            except FileNotFoundError:
                problems[stem] = "Missing render"
                continue
            except (OSError, ValueError):
                record = None
            status = record.get("status") if isinstance(record, dict) else None
            if status == "failed" and isinstance(record.get("reason"), str):
                problems[stem] = f"Render failed: {record['reason']}"
            elif not isinstance(status, str) or status not in expected_statuses:
                problems[stem] = "Render failed: invalid outcome record"
            elif status != "matched" and not (captures / f"{stem}.png").is_file():
                problems[stem] = "Missing capture after render"
    return problems


def available_cpus() -> int:
    """Use the assigned CPUs where the OS exposes affinity, since it can be narrower than the host's count."""
    return len(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else (os.cpu_count() or 1)


def worker_groups(items: list[Item], jobs: int) -> list[list[Item]]:
    """Split catalog-order work within the requested process budget and the CPUs assigned to this runner."""
    count = min(jobs, available_cpus(), len(items))
    return [items[index::count] for index in range(count)]


def stuck_appearance(outcomes: Path, group: list[str]) -> str | None:
    """Name the appearance a renderer was still drawing when it was killed, or None when every one has its receipt.

    The renderer draws its group in catalog order, light before dark, and writes an outcome receipt as each appearance
    finishes, so the first appearance without one is the one it was stuck on. This relies on that order in
    `render_batch` in the goldens; a renderer that drew out of order would make this name the wrong appearance.
    """
    for name in group:
        for appearance in ("light", "dark"):
            stem = f"{name}--{appearance}"
            if not (outcomes / f"{stem}.json").exists():
                return stem
    return None


def render_scenes(command: list[str], groups: list[list[str]], env: dict[str, str], lock: int | None,
                  timeout: float | None = None) -> int:
    """Pass exact names from the inventory to each rendering worker, and name what a timed-out worker was stuck on."""
    requests = [{"FARHELM_GOLDEN_SCENES": json.dumps(group)} for group in groups]
    outcomes = env.get("FARHELM_GOLDEN_OUTCOMES_DIR")

    def stuck(index: int) -> str:
        if outcomes is None:
            return "no outcome receipts to say where it was stuck"
        found = stuck_appearance(Path(outcomes), groups[index])
        return f"stuck on {found}" if found is not None else "stuck after its last appearance"

    return run_workers(command, requests, env, lock, "goldens", timeout=timeout, stuck=stuck)


def format_duration(seconds: float) -> str:
    """Write a deadline the way an operator passed it, in whole minutes where it is one."""
    if seconds >= 60 and seconds % 60 == 0:
        return f"{int(seconds // 60)}m"
    return f"{seconds:g}s"


def run_workers(command: list[str], requests: list[dict[str, str]], env: dict[str, str], lock: int | None,
                label: str, *, timeout: float | None = None, stuck=None) -> int:
    """Own every child through exit, cancellation or the deadline, with a separate output log for each concurrent job.

    `timeout` bounds the whole phase, in seconds from the first spawn, not each job. At the deadline every unfinished
    child is killed and the result is TIMEOUT_STATUS; on SIGINT or SIGTERM it is 128 plus the signal. Either way, the
    output of every job not reported yet is printed after the kill, since a job's log is otherwise shown only once it
    exits and a stuck run would end with nothing that says what it was doing. `stuck(index)` (zero-based) describes
    what a job unfinished at the deadline was doing.
    """
    pass_fds = () if lock is None else (lock,)
    noun = "renderer" if label == "goldens" else label
    with ExitStack() as logs:
        workers = []
        stopping = None
        timed_out = False
        # Jobs before this index have had their output printed; the rest are printed after a stop or timeout.
        reported = 0
        unfinished = set()
        status = 0

        def stop(signum, _frame):
            # A handler can interrupt spawn before registration or wait before returncode is saved. Let the main
            # flow finish those operations before it signals or reaps any child.
            nonlocal stopping
            stopping = signum

        for signum in (signal.SIGINT, signal.SIGTERM):
            logs.callback(signal.signal, signum, signal.signal(signum, stop))
        deadline = None if timeout is None else time.monotonic() + timeout
        try:
            for request in requests:
                if stopping is not None:
                    break
                child_env = {**env, **request}
                log = None
                if len(requests) > 1:
                    log = logs.enter_context(tempfile.TemporaryFile(mode="w+t", encoding="utf-8", errors="replace"))
                child = subprocess.Popen(
                    command, cwd=GUI_CRATE, env=child_env, stdout=log,
                    stderr=subprocess.STDOUT if log is not None else None, pass_fds=pass_fds,
                )
                workers.append((child, log))
            for index, (child, log) in enumerate(workers, 1):
                # A renderer can take minutes. A timed wait lets a cancellation or the deadline reach the single
                # cleanup path.
                while stopping is None:
                    remaining = None if deadline is None else deadline - time.monotonic()
                    try:
                        # Past the deadline this still polls once (a zero timeout), so a job that already exited is
                        # reported as it ended rather than as timed out.
                        result = child.wait(timeout=0.1 if remaining is None else max(0., min(0.1, remaining)))
                    except subprocess.TimeoutExpired:
                        if remaining is not None and remaining <= 0:
                            timed_out = True
                            break
                        continue
                    break
                if stopping is not None or timed_out:
                    break
                if log is not None:
                    print(f"\n[{label} job {index}/{len(workers)}]", flush=True)
                    log.seek(0)
                    shutil.copyfileobj(log, sys.stdout)
                reported = index
                if result < 0:
                    print(f"{noun} terminated by signal {-result}")
                status = status or result
        finally:
            # On a spawn failure, stop or timeout, no worker should keep writing partial captures or holding the lease.
            # Which ones were still running is decided before the kill, which makes every one of them exit.
            unfinished = {index for index, (child, _) in enumerate(workers, 1) if child.poll() is None}
            for child, _ in workers:
                if child.poll() is None:
                    child.kill()
            for child, _ in workers:
                child.wait()
        if stopping is not None or timed_out:
            for index, (child, log) in enumerate(workers[reported:], reported + 1):
                # A job that died of a signal before the runner killed anything is cut short too: a Ctrl-C at a
                # terminal reaches the renderers in the foreground process group as well as the runner.
                signalled = index not in unfinished and child.returncode < 0
                if log is not None:
                    partial = ", partial output" if index in unfinished or signalled else ""
                    print(f"\n[{label} job {index}/{len(workers)}{partial}]", flush=True)
                    log.seek(0)
                    shutil.copyfileobj(log, sys.stdout)
                if signalled:
                    print(f"{noun} terminated by signal {-child.returncode}", flush=True)
                if timed_out and index in unfinished:
                    detail = f": {stuck(index - 1)}" if stuck is not None else ""
                    print(f"{noun} job {index}/{len(workers)} exceeded {format_duration(timeout)}{detail}", flush=True)
    if stopping is not None:
        return 128 + stopping
    return TIMEOUT_STATUS if timed_out else status


def review_update(binary: Path, env: dict[str, str], jobs: int, lock: int | None, temp: Path,
                  pairs: list[tuple[Path, Path]], reviews: Path) -> None:
    """Use the same binary and lease as the render. Only pairs with references need native pixel work."""
    requests = []
    for index, group in enumerate(worker_groups(pairs, jobs)):
        request = temp / f"review-{index}.json"
        request.write_text(json.dumps({
            "directory": str(reviews),
            "pairs": [(str(reference), str(capture)) for reference, capture in group],
        }), encoding="utf-8")
        requests.append({"FARHELM_GOLDEN_DIFF_BATCH": str(request)})
    command = [str(binary), "--ignored", "--exact", "goldens::changes::review_pairs", "--nocapture"]
    try:
        status = run_workers(command, requests, env, lock, "golden review")
    except OSError as error:
        print(f"could not start golden review: {error}", file=sys.stderr)
    else:
        if status in STOP_STATUSES:
            raise ReviewInterrupted(status)
        if status != 0:
            print(f"golden review exited with status {status}", file=sys.stderr)


def run_suite(tool, base_env: dict[str, str], update: bool, patterns: list[str],
              cargo_args: list[str], test_args: list[str], jobs: int,
              render_timeout: float | None = DEFAULT_RENDER_TIMEOUT) -> int:
    """Run with fresh captures, keeping an incomplete update unpublishable and any comparison failures inspectable."""
    lease = tool.capture_lock(tool.CAPTURES) if update else nullcontext()
    with lease as lock:
        # Build and select first, so a failed build or a mistyped --scene preserves previous artifacts.
        binary = build_binary(base_env, cargo_args)
        names = select_scenes(list_scenes(binary, base_env), patterns)
        tool.WORK.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".baseline-", dir=tool.WORK) as baseline:
            manifest = Path(baseline) / "goldens.txt"
            shutil.copyfile(tool.MANIFEST, manifest)
            return run_locked(tool, base_env, binary, names, update, patterns, test_args, jobs, lock, manifest,
                              render_timeout)


def run_locked(tool, base_env: dict[str, str], binary: Path, names: list[str], update: bool, patterns: list[str],
               test_args: list[str], jobs: int, lock: int | None, manifest: Path,
               render_timeout: float | None = DEFAULT_RENDER_TIMEOUT) -> int:
    """Render against the pinned manifest. The caller holds the update lease through review and certification.

    A render that runs past `render_timeout` is not a stop: it goes on to the missing-render report and the changes
    page, so the appearances it never drew are listed like failed ones, and its status stays TIMEOUT_STATUS.
    """
    references = None
    if update:
        # Preserve the full update and its publish marker when inspecting a subset of scenes.
        captures = tool.CAPTURES.with_name("selected-captures") if patterns else tool.CAPTURES
        # Drop the old marker and pictures before drawing. The digest ties this output to the manifest in this run.
        try:
            shutil.rmtree(captures)
        except FileNotFoundError:
            pass
        captures.mkdir(parents=True)
        tool.record_manifest(captures, manifest)
        outcomes = manifest.parent / "outcomes"
    else:
        # A custom Cargo target can be shared by many checkouts. Both comparisons and their automatically fetched
        # references belong to this run in the checkout, so another invocation cannot replace either mid-render.
        run_dir = Path(tempfile.mkdtemp(prefix="compare-", dir=tool.WORK))
        captures = run_dir / "captures"
        # Matching appearances leave no PNG. Keep their receipts with the run even when a worker is interrupted.
        outcomes = run_dir / "outcomes"
        if given := base_env.get("FARHELM_GOLDENS_DIR"):
            references = Path(given).resolve()
        else:
            references = tool.fetch(manifest, run_dir / "refs", tool.REMOTE, tool.CACHE)
    outcomes.mkdir()
    with tempfile.TemporaryDirectory(prefix="fh-goldens.") as runtime_dir:
        os.chmod(runtime_dir, 0o700)
        command = [str(binary), "--ignored", "--exact", "goldens::component_goldens", *test_args]
        env = golden_env(base_env, runtime_dir, references, captures)
        env["FARHELM_GOLDEN_OUTCOMES_DIR"] = str(outcomes)
        groups = worker_groups(names, jobs)
        status = render_scenes(command, groups, env, lock, render_timeout)
    if status in STOP_STATUSES:
        # A cancelled render leaves its captures for inspection. Do not fetch new references or launch more jobs.
        return status
    if status < 0:
        status = 128 - status
    platform = {"darwin": "macos", "win32": "windows"}.get(sys.platform, sys.platform)
    incomplete = incomplete_appearances(outcomes, captures / platform, names, update)
    if incomplete:
        print(f"incomplete golden render under {captures}: {len(incomplete)} appearances missing or failed",
              file=sys.stderr)
        status = status or 1
    # Keep review work under the same capture lease, but give it no role in certifying publishable captures.
    review_batch = partial(review_update, binary, base_env, jobs, lock)
    try:
        page = golden_changes_page.report(tool, captures, platform, references, base_env, review_batch, manifest,
                                          incomplete)
    except ReviewInterrupted as interrupted:
        return interrupted.status
    if not update:
        if status == 0:
            shutil.rmtree(run_dir)
    else:
        status = finish_update(captures, platform, names, patterns, status)
    if page is not None:
        print(f"golden changes page: {page}")
    return status


def finish_update(captures: Path, platform: str, names: list[str], patterns: list[str], status: int) -> int:
    """Decide whether these captures can be published independently of building the optional review artifact."""
    if status != 0:
        return status
    if patterns:
        print(f"\nselected captures written to {captures}. Run a full --update before publishing.")
        return 0
    if not complete_update(captures, platform, names):
        return 1
    print(f"\ncaptures written to {captures}. Publish them with\n  python3 scripts/gui-goldens.py publish\n"
          "and look at each picture it names as changed or added before committing the manifest.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0], allow_abbrev=False)
    parser.add_argument("--update", action="store_true", help="write this run's captures for gui-goldens.py publish")
    parser.add_argument("--scene", action="append", default=[], metavar="PATTERN",
                        help="match a scene name (repeatable, quote wildcards, selected updates cannot publish)")
    parser.add_argument("--jobs", type=int, default=4, metavar="N",
                        help="parallel render or review processes (default: 4, capped at available CPUs)")
    parser.add_argument("--render-timeout", type=float, default=DEFAULT_RENDER_TIMEOUT, metavar="SECONDS",
                        help="kill the renderers and fail if rendering takes longer than this "
                             f"(default: {DEFAULT_RENDER_TIMEOUT:g})")
    args, extras = parser.parse_known_args()
    if args.jobs < 1:
        parser.error("--jobs must be a positive integer")
    if not args.render_timeout > 0:
        parser.error("--render-timeout must be a positive number of seconds")
    separator = extras.index("--") if "--" in extras else len(extras)
    cargo_args, test_args = extras[:separator], extras[separator + 1:]
    for arg in test_args:
        if arg not in ("--nocapture", "--show-output", "--quiet"):
            parser.error(f"test argument {arg!r} is not an output option. Use --scene to select scenes.")
    if sys.platform.startswith("linux"):
        missing = [str(path) for path in (LAVAPIPE_ICD, MESA_EGL_VENDOR) if not path.exists()]
        if missing:
            print(f"the software renderer is not installed: {', '.join(missing)} missing", file=sys.stderr)
            return 1
    tool = load_goldens_tool()
    try:
        return run_suite(tool, dict(os.environ), args.update, args.scene, cargo_args, test_args, args.jobs,
                         args.render_timeout)
    except tool.Refused as refusal:
        print(f"gui-goldens: {refusal}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as error:
        return error.returncode
    except ValueError as error:
        print(f"gui-goldens: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
