#!/usr/bin/env python3
"""The changelog's three mechanical checks: file format, fragment coverage, and what dist will publish.

`releasing/AGENTS.md` describes the changelog process; this script is the part of it that a machine can hold. It
exists because the curated `CHANGELOG.md` is read by cargo-dist at tag time and copied into the GitHub release, and a
file that looks right to a human can still be one dist parses into nothing or into the wrong section. Nothing here
writes to the repository; every mode reports and exits non-zero on a problem.

## The three modes

`format` lints `CHANGELOG.md` and the fragments under `releasing/changelog.d/` against the layout the process
stipulates: one `## vX.Y.Z - YYYY-MM-DD` heading per stable release, newest first; category headings from the fixed
emoji table in its fixed order; prose subsections only under Highlights; one-item bullets ending in a PR reference
everywhere else. The format is deliberately narrower than what dist accepts, so a violation here is a curation slip,
not a dist failure. It runs in the release gate on every tag.

`fragments` answers "did every user-facing change since the last stable release leave a changelog fragment". It walks
the commits since the merge base with the last stable tag, picks out the Conventional Commit types the process says
must have an entry (`feat`, `fix`, `perf`, `style`, `revert`, and any type marked `!`), and reports which of them
added a fragment, which are claimed by a fragment's `pr:` line, and which are missing. It is the release-time sweep's
input, not a gate: the maintainer decides what a missing entry means.

`announce` compares what cargo-dist computed for a tag (its manifest's `announcement_title` and
`announcement_changelog`) with what this file's own reading of `CHANGELOG.md` says the tag should get. A stable tag must
land on the newest section, byte for byte; a prerelease tag must land on nothing, or on the stable section for its
version when that already exists (dist's documented fallback, which the process tolerates). The manifest comes from a
file, from an environment variable (how the release gate passes the plan job's output), or from running `dist plan`.
This is the check that catches a heading dist cannot parse before the release exists.

## What this does NOT do

It does not generate `CHANGELOG.md` from fragments. Curation is a conversation, not a transform, and the fragments are
raw material for it. It does not know about RC or dev releases beyond letting them through `announce` untouched.

Usage:
  check-changelog.py format [--repo PATH]
  check-changelog.py fragments [--repo PATH] [--since TAG]
  check-changelog.py announce --tag TAG [--repo PATH] [--manifest PATH | --manifest-env NAME]
  check-changelog.py --self-test
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

CHANGELOG_NAME = "CHANGELOG.md"
FRAGMENTS_DIR = Path("releasing") / "changelog.d"
FRAGMENT_README = "README.md"

# The category table IS the format: names, emoji, and order are all fixed here and nowhere else. `releasing/AGENTS.md`
# repeats it in prose for humans; if the two ever disagree, this table is what the gate enforces.
CATEGORIES: tuple[tuple[str, str], ...] = (
    ("Highlights", "✨"),  # sparkles
    ("Breaking", "\U0001f4a5"),  # collision
    ("Added", "\U0001f680"),  # rocket
    ("Changed", "\U0001f504"),  # counterclockwise arrows
    ("Fixed", "\U0001f527"),  # wrench
    ("Removed", "\U0001f5d1️"),  # wastebasket with emoji presentation selector
)
CATEGORY_ORDER = {name: index for index, (name, _emoji) in enumerate(CATEGORIES)}
CATEGORY_HEADING = {f"### {emoji} {name}": name for name, emoji in CATEGORIES}

# Fragment kinds map onto the bullet categories; `none` records a deliberate "considered, nothing user-facing" so the
# coverage sweep can tell an omission from a decision.
FRAGMENT_KINDS = ("breaking", "added", "changed", "fixed", "removed", "none")

# Conventional Commit types whose commits must leave a fragment. Any type carrying `!` joins them regardless.
REQUIRED_TYPES = frozenset({"feat", "fix", "perf", "style", "revert"})

RELEASE_HEADING_RE = re.compile(r"^## v(\d+\.\d+\.\d+) - (\d{4}-\d{2}-\d{2})$")
STABLE_TAG_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")
ANY_TAG_RE = re.compile(r"^v(\d+\.\d+\.\d+)(-[0-9A-Za-z.]+)?$")
PR_REFERENCE_RE = re.compile(r"\(#\d+(, #\d+)*\)$")
SUBJECT_RE = re.compile(r"^(?P<type>[a-z]+)(\([^)]*\))?(?P<bang>!)?: ")
SUBJECT_PR_RE = re.compile(r"\(#(\d+)\)\s*$")


class Problem(Exception):
    """A check failed for a reason the operator has to read; the message is the whole report."""


# ---------------------------------------------------------------------------
# CHANGELOG.md parsing
#
# A small, strict reader of the one layout the process allows. It is NOT a
# reimplementation of parse-changelog (dist's parser); `announce` compares
# this reader's answer with dist's precisely because the two are independent.
# ---------------------------------------------------------------------------


@dataclass
class Release:
    """One `## vX.Y.Z - date` section: its heading text as dist will title the release, and its body lines."""

    version: str
    date: str
    heading_text: str
    line_number: int
    body: list[str] = field(default_factory=list)

    def body_text(self) -> str:
        return "\n".join(self.body).strip()


def parse_changelog(text: str) -> tuple[list[str], list[Release]]:
    """Split the file into preamble lines and releases, keyed by the `## ` headings.

    Every `## ` line is taken to be a release heading; validation of its shape happens in `check_changelog_format`, so
    this function collects rather than judges. Fenced code blocks are honoured so a `##` inside an example cannot start
    a section (dist's parser skips fences too).
    """
    preamble: list[str] = []
    releases: list[Release] = []
    in_fence = False
    for number, line in enumerate(text.splitlines(), start=1):
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
        if not in_fence and line.startswith("## "):
            match = RELEASE_HEADING_RE.match(line)
            version, date = (match.group(1), match.group(2)) if match else ("", "")
            releases.append(Release(version, date, line[3:], number))
            continue
        (releases[-1].body if releases else preamble).append(line)
    return preamble, releases


def _version_tuple(version: str) -> tuple[int, int, int]:
    major, minor, patch = version.split(".")
    return int(major), int(minor), int(patch)


def check_changelog_format(text: str) -> list[str]:
    """Return every format violation in `CHANGELOG.md`, as one line each; an empty list is a pass.

    The rules encode the layout `releasing/AGENTS.md` stipulates. They are stricter than dist needs so the file stays
    uniform across releases: dist only cares that a release heading starts with the version, but a reader scanning
    twenty releases cares that every one of them is shaped the same way.
    """
    problems: list[str] = []
    preamble, releases = parse_changelog(text)

    first_content = next((line for line in preamble if line.strip()), "")
    if first_content != "# Changelog":
        problems.append("the file must open with the heading `# Changelog`")
    if any(line.startswith("#") and not line.startswith("# Changelog") for line in preamble):
        problems.append("only prose may appear between `# Changelog` and the first release heading")
    if not releases:
        problems.append("no release sections found")

    previous: tuple[int, int, int] | None = None
    seen_versions: set[str] = set()
    for release in releases:
        where = f"line {release.line_number}"
        if not release.version:
            problems.append(
                f"{where}: release heading `## {release.heading_text}` must read `## vX.Y.Z - YYYY-MM-DD`"
            )
            continue
        try:
            _dt.date.fromisoformat(release.date)
        except ValueError:
            problems.append(f"{where}: `{release.date}` is not a calendar date")
        current = _version_tuple(release.version)
        if release.version in seen_versions:
            problems.append(f"{where}: v{release.version} appears twice")
        seen_versions.add(release.version)
        if previous is not None and current >= previous:
            problems.append(f"{where}: v{release.version} is not older than the release above it; newest goes first")
        previous = current
        problems.extend(f"{where} (v{release.version}): {item}" for item in _check_release_body(release.body))
    return problems


def _check_release_body(lines: list[str]) -> list[str]:
    """Validate one release's categories: fixed headings in fixed order, prose only under Highlights, bullets elsewhere."""
    problems: list[str] = []
    categories: list[tuple[str, list[str]]] = []
    stray: list[str] = []
    in_fence = False
    for line in lines:
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
        if not in_fence and line.startswith("### "):
            name = CATEGORY_HEADING.get(line)
            if name is None:
                allowed = ", ".join(f"`{heading}`" for heading in CATEGORY_HEADING)
                problems.append(f"unknown category heading `{line}`; the headings are {allowed}")
                categories.append(("?", []))
            else:
                categories.append((name, []))
            continue
        (categories[-1][1] if categories else stray).append(line)

    if any(line.strip() for line in stray):
        problems.append("text before the first category heading; a release holds only categories")
    if not categories:
        problems.append("a release needs at least one category")

    last_order = -1
    for name, body in categories:
        if name == "?":
            continue
        order = CATEGORY_ORDER[name]
        if order <= last_order:
            problems.append(f"category `{name}` is out of order or repeated; the order is Highlights, Breaking, Added, Changed, Fixed, Removed")
        last_order = order
        if not any(line.strip() for line in body):
            problems.append(f"category `{name}` is empty; omit a category with no entries")
            continue
        if name == "Highlights":
            problems.extend(_check_highlights(body))
        else:
            problems.extend(f"under `{name}`: {item}" for item in _check_bullets(body))
    return problems


def _check_highlights(lines: list[str]) -> list[str]:
    """Highlights hold `####` subsections with prose and nothing at the top level; bullets belong in the categories."""
    problems: list[str] = []
    seen_subsection = False
    for line in lines:
        if line.startswith("#### "):
            seen_subsection = True
            continue
        if not seen_subsection and line.strip():
            problems.append("Highlights must start with a `#### ` subsection; loose text before it is not allowed")
            break
    if not seen_subsection:
        problems.append("Highlights needs at least one `#### ` subsection")
    return problems


def _check_bullets(lines: list[str]) -> list[str]:
    """Every entry is one list item, one paragraph, ending in `(#N)` or `(#N, #M)`; no headings, no free prose.

    dprint wraps long bullets onto indented continuation lines, so an item is the `- ` line plus the indented lines
    after it, and the PR reference is checked on the item's joined text rather than on its last physical line.
    """
    problems: list[str] = []
    items: list[list[str]] = []
    for line in lines:
        if not line.strip():
            continue
        if line.startswith("- "):
            items.append([line[2:]])
        elif line.startswith("  ") and items:
            items[-1].append(line.strip())
        elif line.startswith("#"):
            problems.append(f"heading `{line}` is not allowed here; only Highlights has subsections")
        else:
            problems.append(f"free text `{line[:60]}` is not allowed here; every entry is a `- ` bullet")
    if not items and not problems:
        problems.append("no bullet entries")
    for item in items:
        joined = " ".join(item)
        if not PR_REFERENCE_RE.search(joined):
            problems.append(f"entry `{joined[:60]}…` must end with its PR reference, `(#N)` or `(#N, #M)`")
    return problems


# ---------------------------------------------------------------------------
# Fragments
# ---------------------------------------------------------------------------


@dataclass
class Fragment:
    """A parsed `releasing/changelog.d/*.md` file: its kind, the PRs it claims, and the prose body."""

    path: Path
    kind: str
    prs: list[int]
    body: str


def parse_fragment(path: Path, text: str) -> Fragment:
    """Parse one fragment, raising `Problem` for anything the format does not allow.

    The front matter is deliberately tiny: `kind:` says which category the entry lands in (or `none` for a considered
    omission), and an optional `pr:` claims one or more PR numbers so a fragment written after its change merged still
    covers it in the sweep. The body is free prose for curation.

    The front matter is a leading `---` block, YAML-style, rather than a `key: value` header followed by a divider.
    dprint formats these files with everything else, and a `---` line directly under text is a setext heading to
    Markdown, which dprint rewrites into `## kind: added`; a leading front matter block is the one shape it leaves alone.
    """
    lines = text.splitlines()
    if not lines or lines[0] != "---":
        raise Problem(f"{path}: must open with a `---` front matter line")
    try:
        divider = lines.index("---", 1)
    except ValueError as missing:
        raise Problem(f"{path}: no closing `---` line ending the front matter") from missing
    kind: str | None = None
    prs: list[int] = []
    for line in lines[1:divider]:
        if not line.strip():
            continue
        key, sep, value = line.partition(":")
        key, value = key.strip(), value.strip()
        if not sep:
            raise Problem(f"{path}: front matter line `{line}` is not `key: value`")
        if key == "kind":
            if value not in FRAGMENT_KINDS:
                raise Problem(f"{path}: kind `{value}` is not one of {', '.join(FRAGMENT_KINDS)}")
            kind = value
        elif key == "pr":
            for part in value.split(","):
                part = part.strip().lstrip("#")
                if not part.isdigit():
                    raise Problem(f"{path}: `pr:` takes PR numbers such as `123` or `123, 456`, not `{value}`")
                prs.append(int(part))
        else:
            raise Problem(f"{path}: unknown front matter key `{key}`; only `kind` and `pr` exist")
    if kind is None:
        raise Problem(f"{path}: missing `kind:` line")
    body = "\n".join(lines[divider + 1 :]).strip()
    if not body:
        raise Problem(f"{path}: the body is empty; for `kind: none` write why there is no entry")
    return Fragment(path, kind, prs, body)


def fragment_files(repo: Path) -> list[Path]:
    """The fragment files on disk, README excluded, in name order so reports are stable."""
    directory = repo / FRAGMENTS_DIR
    if not directory.is_dir():
        return []
    return sorted(path for path in directory.glob("*.md") if path.name != FRAGMENT_README)


def load_fragments(repo: Path) -> tuple[list[Fragment], list[str]]:
    fragments: list[Fragment] = []
    problems: list[str] = []
    for path in fragment_files(repo):
        try:
            fragments.append(parse_fragment(path.relative_to(repo), path.read_text(encoding="utf-8")))
        except Problem as why:
            problems.append(str(why))
    return fragments, problems


# ---------------------------------------------------------------------------
# Coverage sweep over git history
# ---------------------------------------------------------------------------


def _git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args], check=True, capture_output=True, text=True
    ).stdout


def last_stable_tag(repo: Path) -> str:
    """The highest `vX.Y.Z` tag known locally. Prerelease tags never count; a missing tag is an error, not `v0.0.0`."""
    best: tuple[tuple[int, int, int], str] | None = None
    for tag in _git(repo, "tag", "--list", "v*").split():
        match = STABLE_TAG_RE.match(tag)
        if match and (best is None or tuple(map(int, match.groups())) > best[0]):
            best = (tuple(map(int, match.groups())), tag)  # type: ignore[assignment]
    if best is None:
        raise Problem("no stable `vX.Y.Z` tag is known locally; run `git fetch --tags` first")
    return best[1]


@dataclass
class CommitCoverage:
    sha: str
    subject: str
    required: bool
    pr: int | None
    added: list[str]
    deleted: list[str]
    claimed_by: list[str]

    @property
    def covered(self) -> bool:
        return bool(self.added or self.claimed_by or (self.subject.startswith("revert") and self.deleted))


def sweep(repo: Path, since: str | None = None) -> tuple[str, list[CommitCoverage], list[str]]:
    """Walk the commits since the last stable release and pair each required one with its fragment evidence.

    Stable releases are tagged on a release branch that is never merged, so "since the release" means since the merge
    base of that tag and HEAD, not since the tag itself. Returns the tag used, every commit in range (required or not,
    so a fragment added by a `docs:` commit is still explained), and the stale fragments: files on disk whose adding
    commit predates the range, which means a previous release's curation left them behind.
    """
    tag = since or last_stable_tag(repo)
    base = _git(repo, "merge-base", tag, "HEAD").strip()
    log = _git(repo, "log", "--no-merges", "--reverse", "--format=%H%x00%s", f"{base}..HEAD")
    fragments, _problems = load_fragments(repo)
    claims: dict[int, list[str]] = {}
    for fragment in fragments:
        for pr in fragment.prs:
            claims.setdefault(pr, []).append(str(fragment.path))

    commits: list[CommitCoverage] = []
    shas_in_range: set[str] = set()
    for record in filter(None, log.split("\n")):
        sha, subject = record.split("\x00", 1)
        shas_in_range.add(sha)
        match = SUBJECT_RE.match(subject)
        required = bool(match and (match.group("type") in REQUIRED_TYPES or match.group("bang")))
        pr_match = SUBJECT_PR_RE.search(subject)
        pr = int(pr_match.group(1)) if pr_match else None
        added, deleted = [], []
        status = _git(repo, "diff-tree", "--no-commit-id", "--name-status", "-r", "--root", sha, "--", str(FRAGMENTS_DIR))
        for line in filter(None, status.split("\n")):
            code, _tab, name = line.partition("\t")
            if Path(name).name == FRAGMENT_README:
                continue
            if code.startswith("A"):
                added.append(name)
            elif code.startswith("D"):
                deleted.append(name)
            elif code.startswith("R"):
                # A rename lists `old\tnew`; the new name is what exists now.
                added.append(name.split("\t")[-1])
        commits.append(CommitCoverage(sha, subject, required, pr, added, deleted, claims.get(pr or -1, [])))

    stale: list[str] = []
    for path in fragment_files(repo):
        relative = str(path.relative_to(repo))
        adding = _git(repo, "log", "--diff-filter=A", "--format=%H", "--", relative).split()
        if adding and adding[0] not in shas_in_range:
            stale.append(relative)
    return tag, commits, stale


def report_sweep(tag: str, commits: list[CommitCoverage], stale: list[str], problems: list[str]) -> int:
    print(f"commits since the merge base with {tag}: {len(commits)}")
    missing = 0
    for commit in commits:
        if not commit.required:
            if commit.added or commit.deleted:
                print(f"  note {commit.sha[:9]} {commit.subject}")
                for name in commit.added:
                    print(f"         adds {name}")
                for name in commit.deleted:
                    print(f"         removes {name}")
            continue
        if commit.covered:
            evidence = commit.added or commit.claimed_by or commit.deleted
            print(f"  ok   {commit.sha[:9]} {commit.subject}")
            for name in evidence:
                print(f"         {name}")
        else:
            missing += 1
            print(f"  MISSING {commit.sha[:9]} {commit.subject}")
    for name in stale:
        print(f"  STALE {name} predates {tag}; a previous curation left it behind")
    for problem in problems:
        print(f"  MALFORMED {problem}")
    if missing or stale or problems:
        print(f"{missing} required commit(s) without a fragment, {len(stale)} stale, {len(problems)} malformed")
        return 1
    print("every required commit since the last stable release has a fragment")
    return 0


# ---------------------------------------------------------------------------
# The dist announcement
# ---------------------------------------------------------------------------


def check_announce(tag: str, manifest: dict, changelog_text: str) -> list[str]:
    """Compare dist's computed announcement for `tag` with what this file's reading of `CHANGELOG.md` expects.

    Stable tag: the newest section must be exactly what dist found, title and body. Prerelease tag: dist must have found
    nothing, or the stable section for the same version, which is dist's own fallback and means the prerelease will
    carry those notes with its title rewritten. Anything else is a misread and fails.
    """
    match = ANY_TAG_RE.match(tag)
    if not match:
        return [f"tag `{tag}` is not `vX.Y.Z` or `vX.Y.Z-<prerelease>`"]
    version, prerelease = match.group(1), match.group(2)
    _preamble, releases = parse_changelog(changelog_text)
    by_version = {release.version: release for release in releases if release.version}
    title = manifest.get("announcement_title")
    body = manifest.get("announcement_changelog")
    problems: list[str] = []

    if manifest.get("announcement_is_prerelease") != bool(prerelease):
        problems.append(f"dist calls {tag} prerelease={manifest.get('announcement_is_prerelease')}, which disagrees with the tag")

    if not prerelease:
        expected = by_version.get(version)
        if expected is None:
            problems.append(f"{CHANGELOG_NAME} has no section `## v{version} - <date>`; a stable release needs one")
        elif releases[0] is not expected:
            problems.append(f"`## v{version}` is not the newest section in {CHANGELOG_NAME}; the release being cut must be first")
        if body is None:
            problems.append(f"dist found no changelog section for {tag}; the GitHub release would carry no notes")
        elif expected is not None:
            if title != expected.heading_text:
                problems.append(f"dist titles the release `{title}`, expected `{expected.heading_text}`")
            if (body or "").strip() != expected.body_text():
                problems.append("dist's copy of the section differs from the file's; check for a heading dist parsed differently")
        return problems

    if body is None:
        print(f"ok: {tag} is a prerelease and dist found no section for it; the release carries no notes")
        return problems
    fallback = by_version.get(version)
    if fallback is None:
        problems.append(f"dist found notes for prerelease {tag} but {CHANGELOG_NAME} has no v{version} section; dist picked something unexpected")
    elif (body or "").strip() != fallback.body_text():
        problems.append(f"dist's notes for {tag} are not the v{version} section")
    else:
        print(f"note: {tag} will carry the v{version} section under the title `{title}` (dist's stable-section fallback)")
    return problems


def load_manifest(args: argparse.Namespace, repo: Path) -> dict:
    if args.manifest:
        return json.loads(Path(args.manifest).read_text(encoding="utf-8"))
    if args.manifest_env:
        raw = os.environ.get(args.manifest_env)
        if not raw:
            raise Problem(f"environment variable {args.manifest_env} is empty or unset")
        return json.loads(raw)
    result = subprocess.run(
        ["dist", "plan", "--tag", args.tag, "--output-format=json"],
        cwd=repo, capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise Problem(f"`dist plan --tag {args.tag}` failed:\n{result.stderr.strip()}")
    return json.loads(result.stdout)


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------

GOOD_CHANGELOG = """# Changelog

Notable user-facing changes in each stable release.

## v0.2.0 - 2026-02-01

### ✨ Highlights

#### A feature worth a paragraph

It does the thing. It does not do the other thing. (#12)

### \U0001f4a5 Breaking

- The old flag is gone. (#11)

### \U0001f680 Added

- The thing, with a long enough description that a formatter would wrap it onto a second physical line before
  the reference. (#12)

### \U0001f527 Fixed

- A fix. (#13, #14)

## v0.1.0 - 2026-01-01

### \U0001f680 Added

- Everything. (#1)
"""


def _run_git(repo: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-C", str(repo), *args], check=True, capture_output=True, text=True,
        env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"},
    )


def _commit(repo: Path, subject: str, files: dict[str, str | None]) -> None:
    """Write (or delete, for `None`) the given files and commit them under `subject`."""
    for name, content in files.items():
        path = repo / name
        if content is None:
            path.unlink()
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
    _run_git(repo, "add", "-A")
    _run_git(repo, "commit", "-q", "--allow-empty", "-m", subject)


def self_test() -> int:
    """Exercise all three modes on fixtures: the format rules, a synthetic history, and synthetic dist manifests.

    Why a self-test rather than a test file: the release gate calls this script on a fresh runner, and the other
    release scripts (`scripts/check-release-archive.py` and friends) already follow the `--self-test` convention, so
    the maintainer has one habit for all of them. The git fixture matters most: the sweep's "since the merge base"
    rule and the `pr:` claim rule are the two places a plausible-looking implementation goes wrong.
    """
    failures: list[str] = []

    def expect(condition: bool, what: str) -> None:
        print(f"  {'ok' if condition else 'FAILED'}: {what}")
        if not condition:
            failures.append(what)

    expect(check_changelog_format(GOOD_CHANGELOG) == [], "the reference changelog passes the format check")
    variants = {
        "release heading without a date": GOOD_CHANGELOG.replace("## v0.2.0 - 2026-02-01", "## v0.2.0"),
        "emoji before the version": GOOD_CHANGELOG.replace("## v0.2.0 - 2026-02-01", "## \U0001f680 v0.2.0 - 2026-02-01"),
        "an Unreleased section": GOOD_CHANGELOG.replace("## v0.2.0 - 2026-02-01", "## Unreleased"),
        "releases out of order": GOOD_CHANGELOG.replace("v0.1.0 - 2026-01-01", "v0.3.0 - 2026-01-01"),
        "a category without its emoji": GOOD_CHANGELOG.replace("### \U0001f527 Fixed", "### Fixed"),
        "categories out of order": GOOD_CHANGELOG.replace("### \U0001f4a5 Breaking\n\n- The old flag is gone. (#11)\n\n", "")
        .replace("### \U0001f527 Fixed", "### \U0001f4a5 Breaking\n\n- Late. (#11)\n\n### \U0001f527 Fixed"),
        "a bullet without a PR reference": GOOD_CHANGELOG.replace("- A fix. (#13, #14)", "- A fix."),
        "a subsection outside Highlights": GOOD_CHANGELOG.replace("- A fix. (#13, #14)", "#### A fix\n\nProse. (#13)"),
        "loose text in Highlights": GOOD_CHANGELOG.replace("#### A feature worth a paragraph\n\n", "Loose.\n\n#### A feature worth a paragraph\n\n"),
        "an empty category": GOOD_CHANGELOG.replace("- Everything. (#1)\n", ""),
        "a wrong top heading": GOOD_CHANGELOG.replace("# Changelog", "# Release notes"),
    }
    for what, text in variants.items():
        expect(check_changelog_format(text) != [], f"the format check rejects {what}")

    good_fragment = "---\nkind: added\npr: 12, 13\n---\n\nThe thing.\n"
    fragment = parse_fragment(Path("x.md"), good_fragment)
    expect(fragment.kind == "added" and fragment.prs == [12, 13], "a fragment's kind and pr claims parse")
    for what, text in {
        "a fragment without kind": "---\npr: 1\n---\nbody\n",
        "an unknown kind": "---\nkind: shiny\n---\nbody\n",
        "an unknown key": "---\nkind: added\nwhy: because\n---\nbody\n",
        "an empty body": "---\nkind: none\n---\n\n",
        "no closing divider": "---\nkind: added\nbody\n",
        "the divider shape dprint rewrites": "kind: added\n---\nbody\n",
    }.items():
        try:
            parse_fragment(Path("x.md"), text)
            expect(False, f"a fragment with {what} is rejected")
        except Problem:
            expect(True, f"a fragment with {what} is rejected")

    with tempfile.TemporaryDirectory() as work:
        repo = Path(work)
        _run_git(repo, "init", "-q", "-b", "main")
        fragments = str(FRAGMENTS_DIR)
        _commit(repo, "feat: first (#1)", {"a.txt": "a", f"{fragments}/{FRAGMENT_README}": "readme"})
        # The stable release lives on a branch off main, like the real ones.
        _run_git(repo, "checkout", "-q", "-b", "release-0.1.0")
        _commit(repo, "chore: release 0.1.0", {"v.txt": "0.1.0"})
        _run_git(repo, "tag", "v0.1.0")
        _run_git(repo, "checkout", "-q", "main")
        _commit(repo, "feat: with fragment (#2)", {"b.txt": "b", f"{fragments}/with-fragment.md": "---\nkind: added\n---\n\nB.\n"})
        _commit(repo, "fix: without fragment (#3)", {"c.txt": "c"})
        _commit(repo, "docs: not required (#4)", {"d.txt": "d"})
        _commit(repo, "refactor!: breaking by bang (#5)", {"e.txt": "e"})
        _commit(repo, "docs: claims an earlier pr (#6)", {f"{fragments}/late.md": "---\nkind: fixed\npr: 3\n---\n\nC.\n"})
        _commit(repo, "chore: release 0.2.0-rc.1 (#7)", {"v.txt": "0.2.0-rc.1"})
        _run_git(repo, "tag", "v0.2.0-rc.1")

        tag, commits, stale = sweep(repo)
        expect(tag == "v0.1.0", "the rc tag is ignored when finding the last stable release")
        by_pr = {commit.pr: commit for commit in commits}
        expect(1 not in by_pr, "commits before the release's merge base are outside the range")
        expect(by_pr[2].covered and by_pr[2].added == [f"{fragments}/with-fragment.md"], "a commit that adds a fragment is covered")
        expect(by_pr[3].covered and by_pr[3].claimed_by == [f"{fragments}/late.md"], "a later fragment's pr: claim covers an earlier commit")
        expect(not by_pr[4].required, "a docs commit is not required to have a fragment")
        expect(by_pr[5].required and not by_pr[5].covered, "a bang commit of a non-required type is required and reported missing")
        expect(stale == [], "fragments added inside the range are not stale")

        # Cut 0.2.0 for real: the curation commit consumes one fragment and forgets the other, then the release is tagged.
        _commit(repo, "docs: changelog for 0.2.0 (#8)", {f"{fragments}/with-fragment.md": None})
        _run_git(repo, "checkout", "-q", "-b", "release-0.2.0")
        _commit(repo, "chore: release 0.2.0", {"v.txt": "0.2.0"})
        _run_git(repo, "tag", "v0.2.0")
        _run_git(repo, "checkout", "-q", "main")
        _commit(repo, "feat: after the release (#9)", {"f.txt": "f"})
        tag, commits, stale = sweep(repo)
        expect(tag == "v0.2.0", "the newest stable tag wins")
        expect([commit.pr for commit in commits] == [9], "only commits after the new merge base are in range")
        expect(stale == [f"{fragments}/late.md"], "a fragment left behind by curation is reported stale")

    stable_manifest = {
        "announcement_is_prerelease": False,
        "announcement_title": "v0.2.0 - 2026-02-01",
        "announcement_changelog": GOOD_CHANGELOG.split("## v0.2.0 - 2026-02-01\n", 1)[1].split("## v0.1.0")[0],
    }
    expect(check_announce("v0.2.0", stable_manifest, GOOD_CHANGELOG) == [], "a stable tag matching the newest section passes")
    expect(check_announce("v0.2.0", {**stable_manifest, "announcement_changelog": None}, GOOD_CHANGELOG) != [], "a stable tag dist found nothing for fails")
    expect(check_announce("v0.2.0", {**stable_manifest, "announcement_title": "v0.2.0"}, GOOD_CHANGELOG) != [], "a title dist read differently fails")
    expect(check_announce("v0.2.0", {**stable_manifest, "announcement_changelog": "### other"}, GOOD_CHANGELOG) != [], "a body dist read differently fails")
    expect(check_announce("v0.1.0", {**stable_manifest, "announcement_title": "v0.1.0 - 2026-01-01", "announcement_changelog": "### \U0001f680 Added\n\n- Everything. (#1)"}, GOOD_CHANGELOG) != [], "a stable tag for a section that is not the newest fails")
    expect(check_announce("v0.3.0", {"announcement_is_prerelease": False, "announcement_title": "v0.3.0", "announcement_changelog": None}, GOOD_CHANGELOG) != [], "a stable tag with no section fails")
    empty = {"announcement_is_prerelease": True, "announcement_title": "v0.3.0-rc.1", "announcement_changelog": None}
    expect(check_announce("v0.3.0-rc.1", empty, GOOD_CHANGELOG) == [], "a prerelease with no notes passes")
    fallback = {**stable_manifest, "announcement_is_prerelease": True, "announcement_title": "v0.2.0-rc.2 - 2026-02-01"}
    expect(check_announce("v0.2.0-rc.2", fallback, GOOD_CHANGELOG) == [], "a prerelease carrying its stable section passes with a note")
    expect(check_announce("v0.3.0-rc.1", {**fallback, "announcement_is_prerelease": True}, GOOD_CHANGELOG) != [], "a prerelease carrying some other section fails")
    expect(check_announce("v0.2.0", {**stable_manifest, "announcement_is_prerelease": True}, GOOD_CHANGELOG) != [], "a prerelease flag disagreeing with the tag fails")

    if failures:
        for line in failures:
            print(f"self-test FAILED: {line}", file=sys.stderr)
        return 1
    print("== self-test ok")
    return 0


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--repo", default=".", help="repository root (default: current directory)")
    sub = parser.add_subparsers(dest="mode")
    sub.add_parser("format", help="lint CHANGELOG.md and the fragments")
    fragments = sub.add_parser("fragments", help="report fragment coverage since the last stable release")
    fragments.add_argument("--since", help="stable tag to sweep from (default: the highest vX.Y.Z tag)")
    announce = sub.add_parser("announce", help="compare dist's announcement for a tag with CHANGELOG.md")
    announce.add_argument("--tag", required=True)
    announce.add_argument("--manifest", help="a dist manifest JSON file")
    announce.add_argument("--manifest-env", help="environment variable holding the manifest JSON")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not args.mode:
        parser.error("choose a mode: format, fragments, announce, or --self-test")
    repo = Path(args.repo).resolve()
    try:
        if args.mode == "format":
            problems = check_changelog_format((repo / CHANGELOG_NAME).read_text(encoding="utf-8"))
            _fragments, fragment_problems = load_fragments(repo)
            problems.extend(fragment_problems)
            for problem in problems:
                print(f"format: {problem}", file=sys.stderr)
            if problems:
                return 1
            print(f"ok: {CHANGELOG_NAME} and {len(_fragments)} fragment(s) follow the stipulated format")
            return 0
        if args.mode == "fragments":
            _fragments, fragment_problems = load_fragments(repo)
            tag, commits, stale = sweep(repo, args.since)
            return report_sweep(tag, commits, stale, fragment_problems)
        manifest = load_manifest(args, repo)
        problems = check_announce(args.tag, manifest, (repo / CHANGELOG_NAME).read_text(encoding="utf-8"))
        for problem in problems:
            print(f"announce: {problem}", file=sys.stderr)
        if problems:
            return 1
        print(f"ok: dist's announcement for {args.tag} matches {CHANGELOG_NAME}")
        return 0
    except Problem as why:
        print(f"error: {why}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
