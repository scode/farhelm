#!/usr/bin/env python3
"""Fetch and publish the GPUI app's golden reference pictures, which live outside this repository.

NOTE: This is the only thing that writes the store's ref. Do not push to it by hand.

The golden test (`crates/gui/farhelm-gui/src/goldens/`) compares every scene with a reference PNG. Those PNGs used to
be committed under `crates/gui/farhelm-gui/tests/goldens/<os>/`, and in three days they grew main's history by about
80 MB, so the maintainer decided on 2026-11-16 to keep them in a separate private repository instead. This is the one
place that history is told; other docs point here. Main keeps only the manifest,
`crates/gui/farhelm-gui/tests/goldens.txt`: one line per picture, `<platform>/<scene>--<appearance>.png <git blob id>`.
One line per picture is deliberate. Two PRs that change different scenes touch different lines and merge cleanly, and
two that change the same scene conflict on that scene's line, which is the conflict they should have.

Here's the TLDR of the store. It is the ref `refs/gui-goldens/keep` (`REF`) on the storage repository (`REMOTE`). Each
publish pushes a root "snapshot" commit holding only the pictures the store did not have yet, and moves the ref to a
"keeper" commit whose parents are the previous keeper and that snapshot. The ref only ever grows, so every blob any
manifest ever named stays reachable, and git stores a picture that comes back unchanged only once. The ref is outside
`refs/heads/`, so nothing that clones or fetches branches pulls it, and this script never creates a branch or a tag.
The shape follows `scripts/publish-docs-shots.sh`, minus its retention window: an old commit of main must still find
its references.

    gui-goldens.py fetch [--dir DIR] [--manifest FILE] [--cache DIR]
    gui-goldens.py publish [--captures DIR]
    gui-goldens.py --self-test

`fetch` writes every picture the manifest names to `DIR/<platform>/<scene>--<appearance>.png` (DIR defaults to
`target/gui-goldens/refs`), plus a `.gui-goldens-fetch` marker in DIR, checks each picture's blob id, and prints DIR;
`FARHELM_GOLDENS_DIR` points at DIR itself, not at its platform directory. It keeps a cache repository (default
`target/gui-goldens/cache.git`) and goes to the network only when the cache lacks a blob the manifest names, so it is
incremental, and offline when nothing changed. A blob the store does not hold fails the fetch, naming every such
picture, and leaves no directory behind to be mistaken for references. The defaults are under the checkout's own
`target/`, also when `CARGO_TARGET_DIR` points elsewhere, so checkouts that share a build directory never share
references.

`fetch --manifest FILE --cache DIR --dir DIR` fetches for another commit than the checkout's, given its manifest as a
file (`git show <commit>:crates/gui/farhelm-gui/tests/goldens.txt`). With all three given, nothing depends on where
the script itself lives, so a copy extracted from any commit (`git show <commit>:scripts/gui-goldens.py`) works from a
temporary directory outside any checkout; that is how a machine with access to the storage fetches for a tree it does
not have checked out, which may even predate this script.

NOTE: A known cost, left for later: a cold cache fetches the keeper's whole history, every picture ever published, not
just the ones the manifest names. That was about 40 MB at the switch, but the store only grows (the committed PNGs
reached 159 MB of versions in three days), so a fresh checkout's first run will get slower. A partial clone
(`--filter=blob:none`, fetching the manifest's blobs on demand) or a cache shared between checkouts would fix it.

`publish` takes a directory of captures written by `scripts/check-gui-goldens.py --update` (default
`target/gui-goldens/captures`): `<platform>/<file>`, a `<platform>.complete` marker the golden test writes once every
scene of that platform is captured, and `manifest.sha256`, the digest of the manifest the captures were taken against.
It pushes every capture the store lacks, then rewrites the manifest's lines for each captured platform: changed
pictures get their new id, new scenes are added, and scenes with no capture are dropped (other platforms' lines are
left alone). Because it replaces a platform's lines wholesale, it refuses captures taken against another manifest: a
rebase over a PR that published other scenes in between would otherwise write their old pictures back and drop their
new scenes. The push carries a lease on the keeper it read and is retried when another publish won the race. The
manifest is written only after the push succeeded, so it never names a picture the store lacks. Pushing what the store
lacks, rather than what differs from the manifest, is what lets the very first publish (an empty store and a manifest
that already lists every picture) work, and lets a publish whose manifest write was lost simply run again.

Both use plain `git` with whatever credentials the machine running it has (on a maintainer's machine, `gh auth
setup-git` is enough). Git never prompts on the terminal for HTTPS credentials: a missing credential fails the command.
The functions take the remote as a parameter only so that `--self-test` can run them against a bare repository in a
temporary directory, with no network; the command line always uses `REMOTE`.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Callable

REPO = Path(__file__).resolve().parent.parent
MANIFEST = REPO / "crates/gui/farhelm-gui/tests/goldens.txt"
REMOTE = "https://github.com/scode/farhelm-gpui-reference-storage.git"
REF = "refs/gui-goldens/keep"
WORK = REPO / "target/gui-goldens"
CACHE = WORK / "cache.git"
FETCHED = WORK / "refs"
CAPTURES = WORK / "captures"

# A directory `fetch` wrote holds this file. `fetch` replaces only such a directory (or an empty one), so a mistyped
# `--dir` cannot wipe something else.
FETCH_MARK = ".gui-goldens-fetch"
# Beside the captures: the SHA-256 of the manifest they were taken against (`record_manifest`).
MANIFEST_DIGEST = "manifest.sha256"
# A manifest path: the platform as Rust's `std::env::consts::OS` names it, then the file the golden test writes.
# Strict on purpose, since `fetch` turns it into a path under DIR.
NAME = re.compile(r"^([a-z0-9]+)/([a-z0-9][a-z0-9-]*--(?:light|dark)\.png)$")
BLOB = re.compile(r"^[0-9a-f]{40}$")
PUSH_ATTEMPTS = 5
# Variables that would point git at another repository than the cache (a hook or a wrapper may have set them).
LOCATING = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES")
# Commits in the store carry a fixed identity rather than whoever published, so no personal name or address ends up
# there, and a machine with no git identity configured can still publish.
IDENTITY = {
    "GIT_AUTHOR_NAME": "Farhelm GUI goldens",
    "GIT_AUTHOR_EMAIL": "gui-goldens@invalid",
    "GIT_COMMITTER_NAME": "Farhelm GUI goldens",
    "GIT_COMMITTER_EMAIL": "gui-goldens@invalid",
}

HEADER = f"""\
# The GPUI app's golden reference pictures, one line per picture: `<platform>/<scene>--<appearance>.png <git blob id>`.
# The pictures are not in this repository: they are blobs reachable from {REF} in
# {REMOTE}, a private repository.
# scripts/gui-goldens.py fetches them, publishes new ones and writes this file; do not edit the lines by hand.
"""


class Refused(Exception):
    """A refusal with a message for the operator; `main` prints it and exits 1."""


# --- manifest -------------------------------------------------------------------------------------------------------


def blob_id(data: bytes) -> str:
    """The id git gives `data` as a blob (SHA-1 object format), computed without a repository."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def read_manifest(path: Path) -> dict[str, str]:
    """The manifest's entries, path to blob id. Comments and blank lines are skipped; a malformed or repeated path is
    refused rather than guessed at, since a bad merge is the likeliest way to get one."""
    entries: dict[str, str] = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2 or not NAME.match(parts[0]) or not BLOB.match(parts[1]):
            raise Refused(f"{path}:{number}: not `<platform>/<scene>--<appearance>.png <blob id>`: {line!r}")
        if parts[0] in entries:
            raise Refused(f"{path}:{number}: {parts[0]} is listed twice")
        entries[parts[0]] = parts[1]
    return entries


def write_manifest(path: Path, entries: dict[str, str]) -> None:
    """Write the header and one sorted line per entry."""
    path.write_text(HEADER + "".join(f"{name} {entries[name]}\n" for name in sorted(entries)), encoding="utf-8")


def manifest_digest(manifest: Path) -> str:
    """The manifest's SHA-256, which ties a set of captures to the manifest they were taken against."""
    return hashlib.sha256(manifest.read_bytes()).hexdigest()


def record_manifest(captures: Path, manifest: Path) -> None:
    """Record `manifest`'s digest in `captures`, as `scripts/check-gui-goldens.py --update` does before its run."""
    (captures / MANIFEST_DIGEST).write_text(manifest_digest(manifest) + "\n", encoding="utf-8")


# --- the cache repository -------------------------------------------------------------------------------------------


def git(cache: Path, *args: str, stdin: bytes | None = None, check: bool = True) -> subprocess.CompletedProcess:
    """Run git against the bare repository `cache`, never against whatever checkout the caller stands in, with no
    terminal prompt for HTTPS credentials and stdin closed, so a credential nobody can type fails instead of hanging."""
    env = {k: v for k, v in os.environ.items() if k not in LOCATING}
    env.update(IDENTITY, GIT_TERMINAL_PROMPT="0")
    result = subprocess.run(["git", "--git-dir", str(cache), "-c", "commit.gpgsign=false", *args],
                            input=stdin, stdin=subprocess.DEVNULL if stdin is None else None,
                            capture_output=True, env=env)
    if check and result.returncode != 0:
        raise Refused(f"git {' '.join(args)} failed: {result.stderr.decode(errors='replace').strip()}")
    return result


def open_cache(cache: Path) -> None:
    """Create the cache repository if it does not exist yet."""
    if not (cache / "HEAD").exists():
        cache.mkdir(parents=True, exist_ok=True)
        git(cache, "init", "--bare", "-q")


def sync_keeper(cache: Path, remote: str) -> str | None:
    """Copy the remote's keeper into the cache and return its id, or None when the remote has none yet (before the
    first publish). The cache's ref then matches the remote's, absent included."""
    listed = git(cache, "ls-remote", remote, REF).stdout.decode().split()
    if not listed:
        git(cache, "update-ref", "-d", REF)
        return None
    git(cache, "fetch", "--no-tags", "-q", remote, f"+{REF}:{REF}")
    return git(cache, "rev-parse", REF).stdout.decode().strip()


def kept_blobs(cache: Path) -> set[str]:
    """Every object reachable from the cache's keeper: what the store held when the cache last saw it."""
    if git(cache, "rev-parse", "--verify", "-q", REF, check=False).returncode != 0:
        return set()
    return {line[:40] for line in git(cache, "rev-list", "--objects", REF).stdout.decode().splitlines()}


# --- fetch ----------------------------------------------------------------------------------------------------------


def fetch(manifest: Path, out: Path, remote: str, cache: Path) -> Path:
    """Write every picture `manifest` names to `out` and return it; see the module docs for the guarantees."""
    entries = read_manifest(manifest)
    if out.exists():
        if not out.is_dir():
            raise Refused(f"{out} is not a directory")
        if any(out.iterdir()) and not (out / FETCH_MARK).exists():
            raise Refused(f"{out} holds files fetch did not write; give fetch an empty or new directory")
        shutil.rmtree(out)
    open_cache(cache)
    wanted = sorted(set(entries.values()))
    if not set(wanted) <= kept_blobs(cache):
        sync_keeper(cache, remote)
    kept = kept_blobs(cache)
    missing = sorted(name for name, blob in entries.items() if blob not in kept)
    if missing:
        raise Refused(f"{remote} {REF} lacks {len(missing)} of the manifest's pictures (never published?): "
                      + ", ".join(missing))
    blobs = {blob: git(cache, "cat-file", "blob", blob).stdout for blob in wanted}
    # Every blob is checked before anything is written, so a failure leaves no directory at all.
    for blob, data in blobs.items():
        if blob_id(data) != blob:
            raise Refused(f"the cache's blob {blob} does not hash to its id; delete {cache}")
    for name, blob in entries.items():
        (out / name).parent.mkdir(parents=True, exist_ok=True)
        (out / name).write_bytes(blobs[blob])
    (out / FETCH_MARK).write_text(f"written by scripts/gui-goldens.py fetch from {manifest}\n", encoding="utf-8")
    return out


# --- publish --------------------------------------------------------------------------------------------------------


def read_captures(captures: Path, manifest: Path) -> dict[str, Path]:
    """The captured pictures by manifest path, refusing anything but complete platforms of well-named pictures taken
    against `manifest` as it is now.

    A platform counts only with its `<platform>.complete` marker, holding the number of pictures the golden test wrote,
    and only when that many are there: a run that stopped partway, or a directory copied in part, would otherwise drop
    every missing scene from the manifest."""
    if not captures.is_dir():
        raise Refused(f"{captures} does not exist; run scripts/check-gui-goldens.py --update first")
    digest = captures / MANIFEST_DIGEST
    if not digest.is_file():
        raise Refused(f"{captures} has no {MANIFEST_DIGEST}; run scripts/check-gui-goldens.py --update to make it")
    if digest.read_text(encoding="utf-8").strip() != manifest_digest(manifest):
        raise Refused(f"{manifest} changed since these captures were taken (a rebase?); run "
                      "scripts/check-gui-goldens.py --update again on the current tree, then publish")
    found: dict[str, Path] = {}
    for entry in sorted(captures.iterdir()):
        if entry == digest or (entry.is_file() and entry.suffix == ".complete" and (captures / entry.stem).is_dir()):
            continue
        if not entry.is_dir():
            raise Refused(f"{entry} is neither a platform's captures nor its .complete marker")
        marker = captures / f"{entry.name}.complete"
        if not marker.is_file():
            raise Refused(f"{entry} has no {marker.name}: its golden run did not finish")
        files = sorted(entry.iterdir())
        if marker.read_text(encoding="utf-8").strip() != str(len(files)):
            raise Refused(f"{entry} holds {len(files)} files where {marker.name} says {marker.read_text().strip()}")
        for file in files:
            name = f"{entry.name}/{file.name}"
            if not NAME.match(name) or not file.is_file():
                raise Refused(f"{file} is not a golden capture (`<scene>--<appearance>.png`)")
            found[name] = file
    if not found:
        raise Refused(f"{captures} holds no captures")
    return found


def mktree(cache: Path, lines: list[str]) -> str:
    """A tree object from `git ls-tree`-shaped lines (git sorts them)."""
    return git(cache, "mktree", stdin="".join(f"{line}\n" for line in lines).encode()).stdout.decode().strip()


def push_missing(cache: Path, remote: str, pictures: dict[str, str],
                 before_push: Callable[[int], None] | None) -> tuple[str | None, list[str]]:
    """Push every picture (manifest path to blob id) the store lacks as one snapshot, and return the new keeper and
    what it added; (None, []) when the store already holds them all. `before_push` is the self-test's seam for racing
    another publish."""
    last_error = ""
    for attempt in range(PUSH_ATTEMPTS):
        old = sync_keeper(cache, remote)
        kept = kept_blobs(cache)
        todo = sorted(name for name, blob in pictures.items() if blob not in kept)
        if not todo:
            return None, []
        platforms: dict[str, list[str]] = {}
        for name in todo:
            platform, file = name.split("/")
            platforms.setdefault(platform, []).append(f"100644 blob {pictures[name]}\t{file}")
        root = mktree(cache, [f"040000 tree {mktree(cache, files)}\t{p}" for p, files in platforms.items()])
        snapshot = git(cache, "commit-tree", root, "-m", "GUI golden references\n\n" + "\n".join(todo))
        snapshot_id = snapshot.stdout.decode().strip()
        parents = ["-p", old] if old else []
        keeper = git(cache, "commit-tree", mktree(cache, []), *parents, "-p", snapshot_id, "-m",
                     "Keep every GUI golden snapshot").stdout.decode().strip()
        if before_push:
            before_push(attempt)
        pushed = git(cache, "push", "-q", remote, f"{keeper}:{REF}", f"--force-with-lease={REF}:{old or ''}",
                     check=False)
        if pushed.returncode == 0:
            git(cache, "update-ref", REF, keeper)
            return keeper, todo
        # Most likely another publish moved the ref after `sync_keeper` read it; the next attempt starts from its
        # keeper, and drops whatever pictures it already pushed.
        last_error = pushed.stderr.decode(errors="replace").strip()
    raise Refused(f"the push to {REF} failed {PUSH_ATTEMPTS} times; the manifest is unchanged. Last: {last_error}")


def publish(captures: Path, manifest: Path, remote: str, cache: Path,
            before_push: Callable[[int], None] | None = None) -> str:
    """Push what the store lacks and rewrite the captured platforms' manifest lines; return a report for the
    operator."""
    entries = read_manifest(manifest)
    found = read_captures(captures, manifest)
    open_cache(cache)
    paths = list(found)
    # `--no-filters`: the bytes as they are, like `blob_id`, even where a global attributes file maps `*.png` to a
    # filter such as LFS, which would otherwise store and name a pointer instead of the picture.
    ids = git(cache, "hash-object", "-w", "--no-filters", "--stdin-paths",
              stdin="".join(f"{found[p]}\n" for p in paths).encode()).stdout.decode().split()
    pictures = dict(zip(paths, ids, strict=True))
    captured = {name.split("/")[0] for name in found}
    updated = {name: blob for name, blob in entries.items() if name.split("/")[0] not in captured}
    updated.update(pictures)
    keeper, pushed = push_missing(cache, remote, pictures, before_push)
    report = [f"pushed {len(pushed)} pictures to {REF} (keeper {keeper})" if keeper
              else "the store already holds every captured picture; pushed nothing"]
    changed = sorted(n for n in updated if n in entries and updated[n] != entries[n])
    added = sorted(set(updated) - set(entries))
    dropped = sorted(set(entries) - set(updated))
    if updated != entries:
        write_manifest(manifest, updated)
        report.append(f"rewrote {manifest}: {len(changed)} changed, {len(added)} added, {len(dropped)} dropped")
        report += [f"  {kind} {name}" for kind, names in (("changed", changed), ("added", added),
                                                           ("dropped", dropped)) for name in names]
    else:
        report.append(f"{manifest} already names every capture; unchanged")
    return "\n".join(report)


# --- command line ---------------------------------------------------------------------------------------------------


def arguments() -> argparse.ArgumentParser:
    """The command line's parser, apart from `main` so the self-test can check its wiring without the network."""
    parser = argparse.ArgumentParser(prog="gui-goldens.py", description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true", help="run the fixtures against a scratch bare repository")
    commands = parser.add_subparsers(dest="command")
    fetch_parser = commands.add_parser("fetch", help="write the manifest's pictures to a directory")
    fetch_parser.add_argument("--dir", type=Path, default=FETCHED, help=f"default {FETCHED.relative_to(REPO)}")
    fetch_parser.add_argument("--manifest", type=Path, default=MANIFEST,
                              help=f"the manifest to fetch for (default {MANIFEST.relative_to(REPO)})")
    fetch_parser.add_argument("--cache", type=Path, default=CACHE,
                              help=f"the cache repository (default {CACHE.relative_to(REPO)})")
    publish_parser = commands.add_parser("publish", help="push captures the store lacks and rewrite the manifest")
    publish_parser.add_argument("--captures", type=Path, default=CAPTURES, help=f"default {CAPTURES.relative_to(REPO)}")
    return parser


def main(argv: list[str]) -> int:
    """The command line: the checkout's own cache and the one storage repository; publish always rewrites the
    checkout's own manifest."""
    parser = arguments()
    args = parser.parse_args(argv)
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(SelfTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    try:
        if args.command == "fetch":
            print(fetch(args.manifest.resolve(), args.dir.resolve(), REMOTE, args.cache.resolve()))
        elif args.command == "publish":
            print(publish(args.captures.resolve(), MANIFEST, REMOTE, CACHE))
        else:
            parser.error("name a command: fetch or publish (or --self-test)")
    except Refused as refusal:
        print(f"gui-goldens: {refusal}", file=sys.stderr)
        return 1
    return 0


# --- self-test ------------------------------------------------------------------------------------------------------


class SelfTest(unittest.TestCase):
    """Fetch and publish against a bare repository standing in for the storage repository.

    What matters is what main can end up naming: the store must hold every blob a manifest written by `publish` names,
    `fetch` must hand back exactly those bytes or fail, and nothing but the one ref may ever appear in the store."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory(prefix="gui-goldens-test.")
        self.root = Path(self.tmp.name)
        self.remote = str(self.root / "store.git")
        subprocess.run(["git", "init", "--bare", "-q", self.remote], check=True)
        self.manifest = self.root / "goldens.txt"
        self.manifest.write_text(HEADER, encoding="utf-8")
        self.cache = self.root / "cache.git"
        self.fetch_cache = self.root / "fetch-cache.git"
        self.runs = 0

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def captures(self, pictures: dict[str, bytes], platform: str = "linux", manifest: Path | None = None) -> Path:
        """A fresh capture directory, laid out and marked as `check-gui-goldens.py --update` and the golden test leave
        it, taken against `manifest` (this test's own by default) as it is now."""
        self.runs += 1
        out = self.root / f"captures-{self.runs}"
        (out / platform).mkdir(parents=True)
        for name, data in pictures.items():
            (out / platform / name).write_bytes(data)
        (out / f"{platform}.complete").write_text(f"{len(pictures)}\n", encoding="utf-8")
        record_manifest(out, manifest or self.manifest)
        return out

    def publish(self, pictures: dict[str, bytes], **kw) -> str:
        """Publish `pictures` as one platform's complete capture to the test store, through this test's manifest."""
        return publish(self.captures(pictures, kw.pop("platform", "linux")), self.manifest, self.remote,
                       kw.pop("cache", self.cache), **kw)

    def fetch(self) -> dict[str, bytes]:
        """Fetch this test's manifest through a cache of its own, as another machine would, and read back every file."""
        out = fetch(self.manifest, self.root / "fetched", self.remote, self.fetch_cache)
        return {str(p.relative_to(out)): p.read_bytes() for p in out.rglob("*.png")}

    def store(self, *args: str) -> str:
        """Run a read-only git command against the test store and return its output."""
        return subprocess.run(["git", "--git-dir", self.remote, *args], check=True, capture_output=True,
                              text=True).stdout

    def test_a_publish_then_a_fetch_round_trips_every_picture_under_the_one_ref(self) -> None:
        """The basic contract: the manifest names exactly the captures, fetch returns their bytes, and the store holds
        one ref outside refs/heads/ (no branch or tag a clone would pull)."""
        self.publish({"a--light.png": png(1), "a--dark.png": png(2)})
        self.assertEqual(read_manifest(self.manifest), {"linux/a--light.png": blob_id(png(1)),
                                                        "linux/a--dark.png": blob_id(png(2))})
        self.assertTrue(self.manifest.read_text().startswith(HEADER))
        self.assertEqual(self.fetch(), {"linux/a--light.png": png(1), "linux/a--dark.png": png(2)})
        self.assertEqual(self.store("for-each-ref", "--format=%(refname)").split(), [REF])

    def test_a_second_publish_pushes_only_the_changed_picture_on_top_of_the_old_keeper(self) -> None:
        """The ref only grows: the new keeper's parents are the old keeper and a snapshot of just the changed picture,
        so the pictures an older main names stay reachable."""
        self.publish({"a--light.png": png(1), "b--light.png": png(2)})
        first = self.store("rev-parse", REF).strip()
        report = self.publish({"a--light.png": png(1), "b--light.png": png(3)})
        self.assertIn("1 changed, 0 added, 0 dropped", report)
        old_keeper, snapshot = self.store("rev-list", "--parents", "-n1", REF).split()[1:]
        self.assertEqual(old_keeper, first)
        self.assertEqual(self.store("ls-tree", "-r", "--name-only", snapshot).split(), ["linux/b--light.png"])
        self.assertEqual(self.fetch()["linux/b--light.png"], png(3))
        self.assertIn(blob_id(png(2)), self.store("rev-list", "--objects", REF))

    def test_unchanged_captures_push_nothing(self) -> None:
        """Running `--update` and `publish` again without a visual change must leave the store and its ref alone."""
        self.publish({"a--light.png": png(1)})
        keeper = self.store("rev-parse", REF)
        self.assertIn("pushed nothing", self.publish({"a--light.png": png(1)}))
        self.assertEqual(self.store("rev-parse", REF), keeper)

    def test_the_first_publish_pushes_pictures_the_manifest_already_names(self) -> None:
        """The switch's first publish: main's manifest lists every picture and the store is empty. Pushing only what
        differs from the manifest would push nothing and leave every line unfetchable."""
        write_manifest(self.manifest, {"linux/a--light.png": blob_id(png(1))})
        self.assertIn("pushed 1 pictures", self.publish({"a--light.png": png(1)}))
        self.assertEqual(self.fetch(), {"linux/a--light.png": png(1)})

    def test_the_manifest_gains_new_scenes_drops_gone_ones_and_keeps_other_platforms(self) -> None:
        """A renamed or deleted scene drops its line (the committed PNGs used to linger), and a Linux update never
        touches the lines another platform's run wrote."""
        self.publish({"mac--light.png": png(9)}, platform="macos")
        self.publish({"a--light.png": png(1), "gone--light.png": png(2)})
        report = self.publish({"a--light.png": png(1), "new--light.png": png(3)})
        self.assertIn("0 changed, 1 added, 1 dropped", report)
        self.assertEqual(sorted(read_manifest(self.manifest)),
                         ["linux/a--light.png", "linux/new--light.png", "macos/mac--light.png"])

    def test_captures_taken_against_another_manifest_are_refused(self) -> None:
        """Captures taken before a rebase over another PR's publish would write that PR's old pictures back and drop
        its new scenes, since a publish replaces a platform's lines wholesale; the manifest's digest catches it."""
        self.publish({"a--light.png": png(1)})
        stale = self.captures({"a--light.png": png(2)})
        self.publish({"a--light.png": png(1), "theirs--light.png": png(3)})
        with self.assertRaisesRegex(Refused, "changed since these captures were taken"):
            publish(stale, self.manifest, self.remote, self.cache)
        self.assertIn("linux/theirs--light.png", read_manifest(self.manifest))
        self.assertNotIn(blob_id(png(2)), self.store("rev-list", "--objects", REF))

    def test_a_publish_that_loses_the_race_retries_on_the_winners_keeper(self) -> None:
        """Two publishes racing: the loser's push is refused (by its lease, and as a non-fast-forward too), and its
        retry builds on the winner's keeper, so both snapshots stay reachable and neither erases the other."""
        self.publish({"a--light.png": png(1)})
        other = self.root / "other.txt"
        other.write_text(HEADER, encoding="utf-8")

        def race(attempt: int) -> None:
            if attempt == 0:
                publish(self.captures({"b--light.png": png(5)}, "macos", other), other, self.remote,
                        self.root / "other-cache.git")

        self.publish({"a--light.png": png(2)}, before_push=race)
        objects = self.store("rev-list", "--objects", REF)
        for data in (png(1), png(2), png(5)):
            self.assertIn(blob_id(data), objects)
        self.assertEqual(self.fetch(), {"linux/a--light.png": png(2)})

    def test_a_warm_cache_fetches_with_the_store_unreachable(self) -> None:
        """Machines without the storage's credentials run on directories fetched elsewhere, and a repeated fetch of an
        unchanged manifest must not need the network at all."""
        self.publish({"a--light.png": png(1)})
        self.fetch()
        shutil.rmtree(self.remote)
        self.assertEqual(self.fetch(), {"linux/a--light.png": png(1)})

    def test_a_cached_blob_that_does_not_hash_to_its_id_is_refused(self) -> None:
        """The fetched bytes are what the test compares against, so a corrupt cache must fail the fetch rather than
        hand over a wrong reference. Git does not verify a loose object's hash when reading it, which is the premise."""
        self.publish({"a--light.png": png(1)})
        self.fetch()
        good, bad = blob_id(png(1)), git(self.fetch_cache, "hash-object", "-w", "--stdin", stdin=png(2)).stdout

        def loose(blob: str) -> Path:
            return self.fetch_cache / "objects" / blob[:2] / blob[2:]

        self.assertTrue(loose(good).is_file(), "the premise: the fetched blob is a loose object")
        shutil.copyfile(loose(bad.decode().strip()), self.root / "swap")
        os.replace(self.root / "swap", loose(good))
        with self.assertRaisesRegex(Refused, "does not hash to its id"):
            self.fetch()
        self.assertFalse((self.root / "fetched").exists())

    def test_fetch_reads_the_manifest_it_is_given(self) -> None:
        """The coordination tooling fetches for the exact commit a worker runs by handing fetch that commit's manifest
        as a file: fetch must write that manifest's pictures, an older one's included, not the checkout's."""
        self.publish({"a--light.png": png(1)})
        older = self.root / "older.txt"
        shutil.copyfile(self.manifest, older)
        self.publish({"a--light.png": png(2)})
        out = fetch(older, self.root / "older-fetched", self.remote, self.fetch_cache)
        self.assertEqual((out / "linux/a--light.png").read_bytes(), png(1))
        defaults = arguments().parse_args(["fetch"])
        self.assertEqual((defaults.manifest, defaults.cache, defaults.dir), (MANIFEST, CACHE, FETCHED))

    def test_a_standalone_copy_fetches_with_manifest_cache_and_dir_given(self) -> None:
        """The coordination step runs a copy extracted from the worker's commit (`git show <sha>:scripts/...`) in a
        temporary directory, since the checkout it has may predate this script: with `--manifest`, `--cache` and
        `--dir` given, the copy must not need a checkout around it. Only the remote is redirected, through git's own
        `insteadOf` in the child's environment, so the copy runs unmodified and without the network."""
        self.publish({"a--light.png": png(1), "a--dark.png": png(2)})
        alone = Path(tempfile.mkdtemp(prefix="gui-goldens-alone.", dir=self.root))
        shutil.copyfile(__file__, alone / "gui-goldens.py")
        manifest = alone / "goldens.txt"
        shutil.copyfile(self.manifest, manifest)
        env = dict(os.environ, GIT_CONFIG_COUNT="1", GIT_CONFIG_KEY_0=f"url.{self.remote}.insteadOf",
                   GIT_CONFIG_VALUE_0=REMOTE)
        run = subprocess.run([sys.executable, "-I", str(alone / "gui-goldens.py"), "fetch", "--manifest", str(manifest),
                              "--cache", str(alone / "cache.git"), "--dir", str(alone / "refs")],
                             cwd=alone, env=env, capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(run.stdout.strip(), str(alone / "refs"))
        written = sorted(str(p.relative_to(alone / "refs")) for p in (alone / "refs").rglob("*") if p.is_file())
        self.assertEqual(written, [FETCH_MARK, "linux/a--dark.png", "linux/a--light.png"])
        self.assertEqual((alone / "refs/linux/a--light.png").read_bytes(), png(1))
        # The copy's idea of a checkout root is the directory above it; with all three options, nothing goes there.
        self.assertEqual(sorted(p.name for p in alone.iterdir()),
                         ["cache.git", "goldens.txt", "gui-goldens.py", "refs"])
        self.assertFalse((alone.parent / "target").exists())

    def test_a_blob_the_store_lacks_fails_the_fetch_and_leaves_no_directory(self) -> None:
        """A manifest naming an unpublished picture must fail loudly, never hand the test a partial directory."""
        self.publish({"a--light.png": png(1)})
        entries = read_manifest(self.manifest) | {"linux/b--light.png": blob_id(png(7))}
        write_manifest(self.manifest, entries)
        (self.root / "fetched").mkdir()
        (self.root / "fetched" / FETCH_MARK).write_text("")
        with self.assertRaisesRegex(Refused, "lacks 1 of the manifest's pictures.*linux/b--light.png"):
            self.fetch()
        self.assertFalse((self.root / "fetched").exists())

    def test_fetch_never_replaces_a_directory_it_did_not_write(self) -> None:
        """`fetch` empties its directory first, so a mistyped `--dir` must be refused, not wiped, and a regular file
        refused cleanly."""
        (self.root / "fetched").mkdir()
        (self.root / "fetched" / "keep-me").write_text("")
        with self.assertRaisesRegex(Refused, "did not write"):
            self.fetch()
        self.assertTrue((self.root / "fetched" / "keep-me").exists())
        shutil.rmtree(self.root / "fetched")
        (self.root / "fetched").write_text("")
        with self.assertRaisesRegex(Refused, "is not a directory"):
            self.fetch()

    def test_an_unfinished_capture_is_refused_before_anything_is_pushed(self) -> None:
        """Without its marker, or with fewer pictures than the marker counts, a capture would drop scenes."""
        out = self.captures({"a--light.png": png(1)})
        (out / "linux.complete").unlink()
        with self.assertRaisesRegex(Refused, "did not finish"):
            publish(out, self.manifest, self.remote, self.cache)
        out = self.captures({"a--light.png": png(1)})
        (out / "linux.complete").write_text("2\n")
        with self.assertRaisesRegex(Refused, "says 2"):
            publish(out, self.manifest, self.remote, self.cache)
        self.assertEqual(self.store("for-each-ref"), "")

    def test_a_malformed_or_repeated_manifest_line_is_refused(self) -> None:
        """A bad merge of the manifest must stop the run, never be read as a picture path (one that leaves the fetched
        directory, here) or as whichever of two lines came last."""
        for text, expected in ((f"linux/../x--light.png {'0' * 40}\n", "not `"),
                               (f"linux/a--light.png {'0' * 40}\nlinux/a--light.png {'1' * 40}\n", "listed twice")):
            self.manifest.write_text(text)
            with self.assertRaisesRegex(Refused, expected):
                read_manifest(self.manifest)


def png(seed: int) -> bytes:
    """Bytes that differ per seed and have the same length for every seed; nothing here decodes them."""
    return b"\x89PNG\r\n\x1a\n" + bytes([seed]) * 64


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
