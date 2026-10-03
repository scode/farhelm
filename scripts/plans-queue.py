#!/usr/bin/env python3
"""Own the plans queue's format and move its states with compare-and-swap commits on main.

Several executors can work through `plans/` at once (plans/AGENTS.md). What keeps them from building the same plan
twice is that each plan's state lives on its line in `plans/queue/INDEX.md` on main, and every state change is a
commit made directly on main by this script, never by hand. A commit is built on top of the main the script just read
and published with a non-forced ref update, which GitHub refuses unless it is a fast-forward. So if anyone else moved
main in between, the update fails, the script reads main again and re-checks its precondition. That makes the commit
itself the lock: of two executors claiming the same plan at the same moment, exactly one commit lands, and the other
sees the line already claimed and exits 10.

NOTE: This script deliberately does very little. It knows the line grammar, the legal transitions, the invariants
between a state and the files it requires, and how to publish a commit safely. Everything that needs judgment (which
plan to pick, what a report says, whether a resumed plan is consistent) stays with the agents and the rules in
plans/AGENTS.md. It never touches the local checkout or jj state: it reads and writes the remote through `gh api`,
using the existing `gh` login, so it behaves the same from a colocated checkout or a jj workspace.

It only ever writes paths under `plans/queue/` and `plans/reports/`, and it never force-updates a ref.

Exit status:
  0   done (for `check` and `check-slug`: no violations)
  1   `check` found violations, or `check-slug` found the slug in history
  2   usage error
  3   error: a request failed, the result is ambiguous, or main stayed contended through every attempt
  10  the precondition does not hold (the line is not in the state the verb needs, or carries another claim id); the
      caller must re-decide rather than retry blindly

Run `scripts/test-plans-queue.py` after changing this file.
"""

from __future__ import annotations

import argparse
import base64
import dataclasses
import datetime
import hashlib
import json
import pathlib
import random
import re
import signal
import socket
import subprocess
import sys
import tempfile
import time
from typing import Callable, Iterable, Protocol

QUEUE_DIR = "plans/queue"
REPORTS_DIR = "plans/reports"
INDEX_PATH = f"{QUEUE_DIR}/INDEX.md"
WRITABLE_PREFIXES = (f"{QUEUE_DIR}/", f"{REPORTS_DIR}/")

EXIT_OK = 0
EXIT_VIOLATIONS = 1
EXIT_USAGE = 2
EXIT_ERROR = 3
EXIT_PRECONDITION = 10

SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9-]*$")
CLAIM_RE = re.compile(r"^[0-9a-f]{6}$")
OBJECT_ID_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
BRANCH_RE = re.compile(r"^[A-Za-z0-9._][A-Za-z0-9._-]*$")
REPO_RE = re.compile(r"^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$")

# One plan per physical line. INDEX.md is excluded from dprint so nothing ever rewraps it; a state change then edits
# exactly one line, which keeps its textual conflicts with planning PRs to that line.
LINE_RE = re.compile(r"^- \[(?P<state>[^\]]+)\] `(?P<slug>[^`]+)\.md` — (?P<rest>.+)$")
DEPS_RE = re.compile(r"^(?P<summary>.*?) \(after (?P<deps>`[^`]+\.md`(?:, `[^`]+\.md`)*)\)$")
DEP_ITEM_RE = re.compile(r"`([^`]+)\.md`")

SIMPLE_STATES = ("pending", "blocked", "in review", "approved")


class Usage(Exception):
    """A caller mistake: bad arguments or a request the verb can never satisfy. Exit 2."""


class Precondition(Exception):
    """The queue is not in the state the verb needs. Exit 10: the caller must look again and re-decide."""


class Failure(Exception):
    """Something went wrong that the caller cannot fix by re-deciding: a request error, an ambiguous result. Exit 3."""


# --- The index ------------------------------------------------------------------------------------------------------


@dataclasses.dataclass
class Entry:
    """One plan's line in INDEX.md.

    `state` is one of SIMPLE_STATES or "in-flight"; `claim` is the six-hex claim id and is set only when in flight.
    `deps` lists the slugs from a trailing `(after ...)` clause, in written order.
    """

    state: str
    slug: str
    summary: str
    deps: list[str]
    claim: str | None = None

    def state_text(self) -> str:
        return f"in-flight {self.claim}" if self.state == "in-flight" else self.state

    def render(self) -> str:
        line = f"- [{self.state_text()}] `{self.slug}.md` — {self.summary}"
        if self.deps:
            line += " (after " + ", ".join(f"`{d}.md`" for d in self.deps) + ")"
        return line


@dataclasses.dataclass
class Index:
    """INDEX.md split into its free-text header and its lines, in order.

    Everything before the first `- [` line is the header and is preserved byte for byte. After that, every non-blank
    line must be a plan line. A line that is not is reported by `parse_index` and kept verbatim, in place, as a string
    among the parsed entries, so that moving some other plan never silently deletes or reorders it: the problem stays
    visible until a person fixes it, and the line may be a plan someone mistyped.
    """

    header: str
    items: list[Entry | str]

    @property
    def entries(self) -> list[Entry]:
        return [item for item in self.items if isinstance(item, Entry)]

    def find(self, slug: str) -> Entry | None:
        for entry in self.entries:
            if entry.slug == slug:
                return entry
        return None

    def remove(self, slug: str) -> None:
        self.items = [item for item in self.items if not (isinstance(item, Entry) and item.slug == slug)]

    def render(self) -> str:
        lines = [item.render() if isinstance(item, Entry) else item for item in self.items]
        return self.header + "".join(line + "\n" for line in lines)


def parse_state(text: str) -> tuple[str, str | None]:
    """Split a bracketed state into (state, claim id). Raises ValueError for anything outside the grammar."""
    if text in SIMPLE_STATES:
        return text, None
    parts = text.split(" ")
    if len(parts) == 2 and parts[0] == "in-flight" and CLAIM_RE.match(parts[1]):
        return "in-flight", parts[1]
    raise ValueError(f"unknown state [{text}]")


def parse_index(text: str) -> tuple[Index, list[str]]:
    """Parse INDEX.md, returning the index and a list of grammar problems (empty when the file is well formed).

    A malformed plan line is reported and kept in place as a plain string, so callers can still reason about the
    well-formed lines while rendering keeps the malformed one. The mutating verbs refuse to publish a tree whose index
    has problems that the tree they started from did not already have; see `new_violations`.
    """
    lines = text.splitlines(keepends=True)
    header_end = len(lines)
    for i, line in enumerate(lines):
        if line.startswith("- ["):
            header_end = i
            break
    header = "".join(lines[:header_end])
    items: list[Entry | str] = []
    problems: list[str] = []
    keep = items.append

    for number, raw in enumerate(lines[header_end:], start=header_end + 1):
        line = raw.rstrip("\n")
        if not line.strip():
            continue
        match = LINE_RE.match(line)
        if not match:
            problems.append(f"INDEX.md line {number}: not a plan line: {line[:80]}")
            keep(line)
            continue
        try:
            state, claim = parse_state(match["state"])
        except ValueError as err:
            problems.append(f"INDEX.md line {number}: {err}")
            keep(line)
            continue
        slug = match["slug"]
        if not SLUG_RE.match(slug):
            problems.append(f"INDEX.md line {number}: bad slug {slug!r}")
            keep(line)
            continue
        summary, deps = match["rest"], []
        dep_match = DEPS_RE.match(match["rest"])
        if dep_match:
            summary = dep_match["summary"]
            deps = DEP_ITEM_RE.findall(dep_match["deps"])
        elif "(after" in match["rest"]:
            # Read as plain summary text, a mistyped clause would silently drop the dependency and let the plan start
            # before the work it was written on top of has landed.
            problems.append(f"INDEX.md line {number}: malformed (after ...) clause in {slug}")
            keep(line)
            continue
        items.append(Entry(state=state, slug=slug, summary=summary, deps=deps, claim=claim))
    return Index(header=header, items=items), problems


# --- Plan file sections -------------------------------------------------------------------------------------------
#
# The script edits plan files only by whole `## ` sections: it appends or removes `## Blocked`, and appends dated
# entries to `## Decisions`. Headings inside fenced code blocks are not headings, which matters because plan files
# quote shell snippets and Markdown.


_FENCE_RE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")


def _scan_fences(text: str) -> tuple[list[tuple[str, bool]], bool]:
    """Pair every line (with its newline) with whether it is inside fenced code; also say whether a fence is left open.

    Follows CommonMark closely enough for plan files: a fence opens with three or more backticks or tildes, and only a
    bare run of the same character at least as long closes it. So a quoted ```` ```bash ```` inside a four-backtick
    fence does not end it, and neither does an info-string line.
    """
    out: list[tuple[str, bool]] = []
    fence: str | None = None
    for line in text.splitlines(keepends=True):
        match = _FENCE_RE.match(line.rstrip("\n"))
        if fence is None and match and not (match[1][0] == "`" and "`" in match[2]):
            fence = match[1]
            out.append((line, True))
        elif (
            fence is not None
            and match
            and match[1][0] == fence[0]
            and len(match[1]) >= len(fence)
            and not match[2].strip()
        ):
            fence = None
            out.append((line, True))
        else:
            out.append((line, fence is not None))
    return out, fence is not None


def _section_spans(text: str) -> list[tuple[str, int, int]]:
    """Return (heading text, start offset, end offset) for every level-two section, ignoring fenced code."""
    spans: list[tuple[str, int, int]] = []
    offset = 0
    current: tuple[str, int] | None = None
    for line, fenced in _scan_fences(text)[0]:
        if not fenced and line.startswith("## "):
            if current is not None:
                spans.append((current[0], current[1], offset))
            current = (line[3:].strip(), offset)
        offset += len(line)
    if current is not None:
        spans.append((current[0], current[1], offset))
    return spans


def blocked_sections(text: str) -> list[tuple[int, int]]:
    return [(start, end) for heading, start, end in _section_spans(text) if heading == "Blocked"]


def remove_blocked(text: str) -> tuple[str, str]:
    """Remove the one `## Blocked` section; return (new text, the section's body without its heading)."""
    spans = blocked_sections(text)
    if len(spans) != 1:
        raise Precondition(f"expected exactly one ## Blocked section, found {len(spans)}")
    start, end = spans[0]
    section = text[start:end]
    body = section.split("\n", 1)[1] if "\n" in section else ""
    tail = text[end:]
    remaining = text[:start].rstrip("\n") + "\n" + ("\n" + tail if tail else "")
    return remaining, body.strip("\n")


def append_section(text: str, heading: str, body: str) -> str:
    return text.rstrip("\n") + f"\n\n## {heading}\n\n" + body.strip("\n") + "\n"


def append_decision(text: str, entry: str) -> str:
    """Append a dated entry to `## Decisions`, creating the section at the end of the file on first use.

    Decisions are append-only: an existing entry is never rewritten, because a later executor reads the whole history
    to know what the maintainer decided and in which order.
    """
    spans = [(start, end) for heading, start, end in _section_spans(text) if heading == "Decisions"]
    if not spans:
        return append_section(text, "Decisions", entry)
    if len(spans) > 1:
        raise Failure("plan file has more than one ## Decisions section")
    _, end = spans[0]
    head = text[:end].rstrip("\n") + "\n\n" + entry.strip("\n") + "\n"
    tail = text[end:]
    return head + ("\n" + tail if tail else "")


def splice_problems(text: str) -> list[str]:
    """Why text cannot be spliced into a plan file as a section body: headings of level one or two, or an open fence.

    Text the script splices into a plan file (a blocked question, a decision) must not contain them: a `## Options`
    heading inside a question would end the `## Blocked` section early, and answering it would then remove only half
    the question and leave the rest stranded in the plan body. An unclosed fence hides every later heading the same
    way, so the next `block` could not find its own section.
    """
    lines, open_fence = _scan_fences(text)
    found = [
        f"heading too high for a spliced section (use ### or deeper): {line.strip()}"
        for line, fenced in lines
        if not fenced and (line.startswith("# ") or line.startswith("## "))
    ]
    if open_fence:
        # An unclosed fence would swallow every heading after it once spliced, including a later ## Blocked.
        found.append("a code fence that is never closed")
    return found


def quote(body: str) -> str:
    return "\n".join(("> " + line) if line.strip() else ">" for line in body.strip("\n").splitlines())


# --- Content checks -----------------------------------------------------------------------------------------------

_HYGIENE_PATTERNS = ("/home/", "/Users/", "/tmp/", "/private/")


def hygiene_problems(text: str, hostnames: Iterable[str]) -> list[str]:
    """Find text that would leak local environment details into the public repository.

    This is a backstop, not the rule: plans/AGENTS.md asks for reports and questions without local paths or host
    names, and the cold readers check for it. The script refuses content that matches so a slip cannot land on main
    unreviewed (bookkeeping commits skip PR review). It cannot see a user name, because the repository's own owner
    name appears legitimately in URLs.
    """
    problems = []
    names = [h for h in hostnames if len(h) >= 4]
    for number, line in enumerate(text.splitlines(), start=1):
        for pattern in _HYGIENE_PATTERNS:
            if pattern in line:
                problems.append(f"line {number} contains {pattern!r}: {line.strip()[:100]}")
        for name in names:
            if re.search(rf"(?<![A-Za-z0-9_-]){re.escape(name)}(?![A-Za-z0-9_-])", line, re.IGNORECASE):
                problems.append(f"line {number} contains the host name: {line.strip()[:100]}")
    return problems


def local_hostnames() -> list[str]:
    full = socket.gethostname()
    return sorted({full, full.split(".")[0]})


# --- Snapshots and invariants ---------------------------------------------------------------------------------------


@dataclasses.dataclass
class Snapshot:
    """The queue's files at one point: a local tree, or a commit on the remote.

    `files` maps every path under plans/queue/ and plans/reports/ to an opaque version token (the blob id for a remote
    snapshot, the content hash for a local one); `read` returns a file's text. Reads are lazy because a remote read is
    one API call per file and most checks need only the index.
    """

    files: dict[str, str]
    read: Callable[[str], str]


def plan_path(slug: str) -> str:
    return f"{QUEUE_DIR}/{slug}.md"


def report_path(slug: str) -> str:
    return f"{REPORTS_DIR}/{slug}.report.md"


def violations(snapshot: Snapshot, content_for: set[str] | None = None) -> list[str]:
    """List every invariant the snapshot breaks.

    `content_for` limits the content-dependent checks (the Blocked section) to the named plan slugs; None checks all.
    The mutating verbs pass only the plans they touch, since every other plan file was checked when it was written and
    reading them all would cost one request each.

    Invariants: the index parses; no duplicate slugs; every line has a plan file and every plan file has a line; a
    report exists for `in review` and `approved` plans and for no plan that is not listed; a plan has a `## Blocked`
    section exactly when it is blocked (never more than one); dependencies name neither the plan itself nor form a
    cycle among listed plans. A dependency on an unlisted slug is allowed: that is how a satisfied dependency looks.
    """
    if INDEX_PATH not in snapshot.files:
        return [f"{INDEX_PATH} is missing"]
    index, problems = parse_index(snapshot.read(INDEX_PATH))
    found = list(problems)
    seen: set[str] = set()
    for entry in index.entries:
        if entry.slug in seen:
            found.append(f"{entry.slug}: listed more than once")
        seen.add(entry.slug)
        if plan_path(entry.slug) not in snapshot.files:
            found.append(f"{entry.slug}: listed, but {plan_path(entry.slug)} does not exist")
        if entry.state in ("in review", "approved") and report_path(entry.slug) not in snapshot.files:
            found.append(f"{entry.slug}: [{entry.state}] needs {report_path(entry.slug)}")
        if entry.slug in entry.deps:
            found.append(f"{entry.slug}: depends on itself")
        if (content_for is None or entry.slug in content_for) and plan_path(entry.slug) in snapshot.files:
            count = len(blocked_sections(snapshot.read(plan_path(entry.slug))))
            if entry.state == "blocked" and count != 1:
                found.append(f"{entry.slug}: [blocked] needs exactly one ## Blocked section, found {count}")
            if entry.state != "blocked" and count:
                found.append(f"{entry.slug}: has a ## Blocked section but is [{entry.state}]")
    for path in snapshot.files:
        if path == INDEX_PATH:
            continue
        if path.startswith(f"{QUEUE_DIR}/"):
            name = path[len(QUEUE_DIR) + 1 :]
            if "/" in name or not name.endswith(".md"):
                found.append(f"{path}: unexpected file in {QUEUE_DIR}/")
            elif name[:-3] not in seen:
                found.append(f"{path}: plan file without an INDEX.md line")
        elif path.startswith(f"{REPORTS_DIR}/"):
            name = path[len(REPORTS_DIR) + 1 :]
            if "/" in name or not name.endswith(".report.md"):
                found.append(f"{path}: unexpected file in {REPORTS_DIR}/")
            elif name[: -len(".report.md")] not in seen:
                found.append(f"{path}: report for a plan that is not listed")
    found.extend(_cycles(index))
    return found


def _cycles(index: Index) -> list[str]:
    graph = {e.slug: [d for d in e.deps if index.find(d)] for e in index.entries}
    found = []
    state: dict[str, int] = {}

    def visit(node: str, path: list[str]) -> None:
        state[node] = 1
        for dep in graph.get(node, []):
            if state.get(dep) == 1:
                found.append("dependency cycle: " + " -> ".join(path[path.index(dep) :] + [dep]))
            elif dep not in state:
                visit(dep, path + [dep])
        state[node] = 2

    for slug in graph:
        if slug not in state:
            visit(slug, [slug])
    return found


def new_violations(before: Snapshot, after: Snapshot, touched: set[str]) -> tuple[list[str], list[str]]:
    """Split the violations of `after` into (new ones, ones `before` already had).

    The mutating verbs refuse only new violations. If a planning PR merged a malformed line, every executor would
    otherwise be stuck unable to block or deliver until someone fixed it; reporting it as a warning keeps the queue
    moving while still making the problem visible.
    """
    # Parse problems carry line numbers, which shift when a transition removes a line; compare without them.
    def key(problem: str) -> str:
        return re.sub(r"^INDEX\.md line \d+: ", "INDEX.md: ", problem)

    old = {key(v) for v in violations(before, touched)}
    new = violations(after, touched)
    return [v for v in new if key(v) not in old], [v for v in new if key(v) in old]


def base_violations(base: Snapshot, tree: Snapshot) -> list[str]:
    """What a planning PR must not do to the queue that `base` (main) already holds.

    A planning PR may add plan files with new `[pending]` lines, reorder lines, reword a summary, and revise the plan
    file of a plan that is pending or blocked on main. It must not change any line's state or dependencies, drop a
    line, add a line in any other state, or touch a report or the file of a plan in any other state. Without this a
    planning agent resolving a textual conflict by taking its own side could turn a claim back into `[pending]`, and a
    second executor would build the same plan.
    """
    # A base without a queue (before the queue existed) constrains nothing.
    base_index, _ = parse_index(base.read(INDEX_PATH)) if INDEX_PATH in base.files else (Index("", []), [])
    tree_index, _ = parse_index(tree.read(INDEX_PATH)) if INDEX_PATH in tree.files else (Index("", []), [])
    found = []
    for old in base_index.entries:
        new = tree_index.find(old.slug)
        if new is None:
            found.append(f"{old.slug}: line removed (only the queue script removes lines)")
            continue
        if (new.state, new.claim) != (old.state, old.claim):
            found.append(f"{old.slug}: state changed from [{old.state_text()}] (only the queue script moves states)")
        if new.deps != old.deps:
            found.append(f"{old.slug}: dependencies changed")
        if old.state not in ("pending", "blocked") and tree.files.get(plan_path(old.slug)) != base.files.get(
            plan_path(old.slug)
        ):
            if tree.read(plan_path(old.slug)) != base.read(plan_path(old.slug)):
                found.append(f"{old.slug}: plan file changed while [{old.state}] on main")
    for new in tree_index.entries:
        if base_index.find(new.slug) is None and new.state != "pending":
            found.append(f"{new.slug}: new line must be [pending]")
    report_paths = {p for p in set(base.files) | set(tree.files) if p.startswith(f"{REPORTS_DIR}/")}
    for path in sorted(report_paths):
        if path not in base.files or path not in tree.files or base.read(path) != tree.read(path):
            found.append(f"{path}: reports are written only by the queue script")
    return found


def local_snapshot(root: pathlib.Path) -> Snapshot:
    files: dict[str, str] = {}
    for directory in (QUEUE_DIR, REPORTS_DIR):
        base = root / directory
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            if path.is_file():
                rel = path.relative_to(root).as_posix()
                files[rel] = hashlib.sha256(path.read_bytes()).hexdigest()
    return Snapshot(files=files, read=lambda rel: (root / rel).read_text(encoding="utf-8"))


# --- GitHub ---------------------------------------------------------------------------------------------------------


class NotFastForward(Exception):
    """GitHub refused a non-forced ref update because the branch moved, or was moving, since it was read.

    GitHub reports this two ways. "Update is not a fast forward" is the plain case. Under truly concurrent updates of
    the same ref it often answers 422 "Reference cannot be updated" instead (seen in the smoke test against the real
    service, 2026-10-02, in most racing rounds). Both mean the same thing to the loop: re-read and try again.
    """


class GitHub(Protocol):
    """The handful of GitHub operations the script needs. The real one shells out to `gh api`; tests inject a fake."""

    def head(self, branch: str) -> str: ...

    def commit(self, sha: str) -> dict: ...

    def tree(self, sha: str, recursive: bool = False) -> list[dict]: ...

    def blob(self, sha: str) -> str: ...

    def create_tree(self, base_tree: str, entries: list[dict]) -> str: ...

    def create_commit(self, message: str, tree: str, parent: str) -> str: ...

    def update_ref(self, branch: str, sha: str) -> None: ...

    def is_ancestor_or_same(self, ancestor: str, descendant: str) -> bool: ...

    def open_pulls(self) -> list[dict]: ...

    def pull(self, number: int) -> dict: ...

    def merged_pulls(self, prefix: str) -> list[dict]: ...

    def path_history(self, branch: str, path: str, limit: int) -> list[dict]: ...


class GhApi:
    """GitHub through `gh api`, so the existing login is used and no token is ever handled here."""

    def __init__(self, repo: str, timeout: float = 60.0, gh: str = "gh"):
        self.repo = repo
        self.timeout = timeout
        self.gh = gh

    def _call(
        self,
        endpoint: str,
        method: str = "GET",
        body: dict | None = None,
        extra: list[str] | None = None,
        scoped: bool = True,
    ):
        path = f"repos/{self.repo}/{endpoint}" if scoped else endpoint
        argv = [self.gh, "api", "-H", "Accept: application/vnd.github+json", path]
        if method != "GET":
            argv += ["--method", method]
        if body is not None:
            argv += ["--input", "-"]
        argv += extra or []
        try:
            done = subprocess.run(
                argv,
                input=json.dumps(body) if body is not None else None,
                capture_output=True,
                text=True,
                timeout=self.timeout,
            )
        except subprocess.TimeoutExpired as err:
            raise Failure(f"gh api {endpoint} timed out after {self.timeout:.0f}s") from err
        except FileNotFoundError as err:
            raise Failure("gh is not on PATH") from err
        if done.returncode != 0:
            text = (done.stderr + "\n" + done.stdout).strip()
            lowered = text.lower()
            if "422" in text and ("fast forward" in lowered or "reference cannot be updated" in lowered):
                raise NotFastForward(text.splitlines()[0] if text else "")
            last = text.splitlines()[-1] if text else f"exit {done.returncode}"
            raise Failure(f"gh api {method} {endpoint}: {last[:300]}")
        return done.stdout

    def _json(self, *args, **kwargs):
        return json.loads(self._call(*args, **kwargs))

    def head(self, branch: str) -> str:
        # The singular `ref` endpoint matches exactly; the plural one prefix-matches and can answer with an array.
        return self._json(f"git/ref/heads/{branch}")["object"]["sha"]

    def commit(self, sha: str) -> dict:
        return self._json(f"git/commits/{sha}")

    def tree(self, sha: str, recursive: bool = False) -> list[dict]:
        data = self._json(f"git/trees/{sha}" + ("?recursive=1" if recursive else ""))
        if data.get("truncated"):
            raise Failure(f"tree listing {sha} truncated")
        return data["tree"]

    def blob(self, sha: str) -> str:
        data = self._json(f"git/blobs/{sha}")
        return base64.b64decode(data["content"]).decode("utf-8")

    def create_tree(self, base_tree: str, entries: list[dict]) -> str:
        return self._json("git/trees", "POST", {"base_tree": base_tree, "tree": entries})["sha"]

    def create_commit(self, message: str, tree: str, parent: str) -> str:
        return self._json("git/commits", "POST", {"message": message, "tree": tree, "parents": [parent]})["sha"]

    def update_ref(self, branch: str, sha: str) -> None:
        self._call(f"git/refs/heads/{branch}", "PATCH", {"sha": sha, "force": False})

    def is_ancestor_or_same(self, ancestor: str, descendant: str) -> bool:
        status = self._json(f"compare/{ancestor}...{descendant}")["status"]
        return status in ("identical", "ahead")

    def open_pulls(self) -> list[dict]:
        out = self._call(
            "pulls?state=open&per_page=100",
            extra=["--paginate", "--jq", ".[] | {number, ref: .head.ref, draft, url: .html_url}"],
        )
        return [json.loads(line) for line in out.splitlines() if line.strip()]

    def path_history(self, branch: str, path: str, limit: int) -> list[dict]:
        return self._json(f"commits?sha={branch}&path={path}&per_page={limit}")

    def pull(self, number: int) -> dict:
        data = self._json(f"pulls/{number}")
        return {"number": data["number"], "ref": data["head"]["ref"], "merged": bool(data.get("merged_at"))}

    def merged_pulls(self, prefix: str) -> list[dict]:
        # Search matches head branches by prefix but is approximate and can lag a fresh merge by a minute or so, so
        # this is only used for hints in `status`; `remove` verifies the PR numbers its caller names one by one.
        query = f"q=repo:{self.repo} is:pr is:merged head:{prefix}"
        data = json.loads(self._call("search/issues", extra=["-X", "GET", "-f", query], scoped=False))
        numbers = [item["number"] for item in data.get("items", [])]
        return [p for p in (self.pull(n) for n in numbers) if p["merged"] and p["ref"].startswith(prefix)]


def remote_snapshot(gh: GitHub, commit_sha: str, cache: dict[str, str]) -> tuple[Snapshot, str]:
    """Snapshot the queue at a commit; also return the commit's root tree id (the base for a new tree).

    `cache` maps blob ids to text across attempts of one invocation, since blobs are immutable.
    """
    root_tree = gh.commit(commit_sha)["tree"]["sha"]
    files: dict[str, str] = {}
    plans = [e for e in gh.tree(root_tree) if e["path"] == "plans" and e["type"] == "tree"]
    if plans:
        for item in gh.tree(plans[0]["sha"], recursive=True):
            full = f"plans/{item['path']}"
            if item["type"] == "blob" and full.startswith(WRITABLE_PREFIXES):
                files[full] = item["sha"]

    def read(path: str) -> str:
        sha = files[path]
        if sha not in cache:
            cache[sha] = gh.blob(sha)
        return cache[sha]

    return Snapshot(files=files, read=read), root_tree


# --- Formatting -----------------------------------------------------------------------------------------------------


Formatter = Callable[[str, str, str], str]


def dprint_formatter(text: str, path: str, config: str) -> str:
    """Format Markdown with dprint using main's own dprint.json (passed as text), not the local checkout's.

    A local config can lag main's, and a file formatted with the wrong one would fail `dprint check` on main. dprint
    runs in a scratch directory so it cannot discover any other config.
    """
    with tempfile.TemporaryDirectory(prefix="plans-queue-") as scratch:
        config_path = pathlib.Path(scratch) / "dprint.json"
        config_path.write_text(config, encoding="utf-8")
        try:
            done = subprocess.run(
                ["dprint", "fmt", "--stdin", path, "--config", str(config_path)],
                input=text,
                capture_output=True,
                text=True,
                cwd=scratch,
                timeout=120,
            )
        except FileNotFoundError as err:
            raise Failure("dprint is not on PATH; it is needed to format plan files and reports") from err
        except subprocess.TimeoutExpired as err:
            raise Failure(f"dprint timed out formatting {path}") from err
    if done.returncode != 0:
        raise Failure(f"dprint could not format {path}: {(done.stderr or done.stdout).strip()[:300]}")
    return done.stdout


# --- Transitions ----------------------------------------------------------------------------------------------------


@dataclasses.dataclass
class Change:
    """What one transition does to the queue: new file contents (None deletes) and the commit message."""

    writes: dict[str, str | None]
    message: str
    touched: set[str]


@dataclasses.dataclass
class Request:
    """A parsed mutating verb with its inputs already read from local files."""

    verb: str
    slug: str
    claim: str | None = None
    text: str | None = None
    dependents: str | None = None
    merged: list[int] = dataclasses.field(default_factory=list)
    today: str = ""


def _entry(index: Index, slug: str) -> Entry:
    entry = index.find(slug)
    if entry is None:
        raise Precondition(f"{slug} is not listed in {INDEX_PATH}")
    return entry


def _require(entry: Entry, states: tuple[str, ...], claim: str | None = None) -> None:
    if entry.state not in states:
        raise Precondition(f"lost: {entry.slug} is [{entry.state_text()}], not {' or '.join(states)}")
    if claim is not None and entry.claim != claim:
        raise Precondition(f"lost: {entry.slug} carries claim {entry.claim}, not {claim}")


def plan_transition(req: Request, snap: Snapshot, gh: GitHub) -> Change:
    """Compute what `req` changes, given the queue at the commit being built on.

    Raises Precondition when the queue does not allow it now (exit 10) and Usage when it never could. `gh` is consulted
    only by `remove` and `abandon`, which must know about the plan's PRs.
    """
    index, _ = parse_index(snap.read(INDEX_PATH))
    entry = _entry(index, req.slug)
    writes: dict[str, str | None] = {}
    plan = plan_path(req.slug)
    touched = {req.slug}

    if req.verb == "claim":
        _require(entry, ("pending",))
        waiting = [d for d in entry.deps if index.find(d) is not None]
        if waiting:
            raise Precondition(f"{req.slug} waits on {', '.join(waiting)}, which have not landed")
        entry.state, entry.claim = "in-flight", req.claim
        message = f"chore(plans): claim {req.slug}\n\nClaim {req.claim}.\n"
    elif req.verb == "unclaim":
        _require(entry, ("in-flight",), req.claim)
        entry.state, entry.claim = "pending", None
        message = f"chore(plans): give back {req.slug}\n\nClaim {req.claim}.\n"
    elif req.verb == "release":
        _require(entry, ("in-flight",))
        message = f"chore(plans): release {req.slug}\n\nThe maintainer released claim {entry.claim}.\n"
        entry.state, entry.claim = "pending", None
    elif req.verb == "block":
        _require(entry, ("in-flight",), req.claim)
        assert req.text is not None
        body = f"Blocked on {req.today} (claim {req.claim}).\n\n{req.text.strip()}"
        writes[plan] = append_section(snap.read(plan), "Blocked", body)
        entry.state, entry.claim = "blocked", None
        message = f"chore(plans): block {req.slug}\n\nClaim {req.claim}.\n"
    elif req.verb == "deliver":
        _require(entry, ("in-flight",), req.claim)
        assert req.text is not None
        writes[report_path(req.slug)] = req.text
        entry.state, entry.claim = "in review", None
        message = f"chore(plans): deliver the report for {req.slug}\n\nClaim {req.claim}.\n"
    elif req.verb == "answer":
        _require(entry, ("blocked",))
        assert req.text is not None
        remaining, question = remove_blocked(snap.read(plan))
        decision = (
            f"### {req.today}: answer to a blocked question\n\nThe question, as the executor put it:\n\n"
            f"{quote(question)}\n\nThe maintainer's answer:\n\n{req.text.strip()}"
        )
        writes[plan] = append_decision(remaining, decision)
        entry.state = "pending"
        message = f"chore(plans): answer {req.slug}\n"
    elif req.verb == "follow-up":
        _require(entry, ("in review",))
        assert req.text is not None
        decision = f"### {req.today}: follow-up after review\n\n{req.text.strip()}"
        writes[plan] = append_decision(snap.read(plan), decision)
        entry.state = "pending"
        message = f"chore(plans): request a follow-up on {req.slug}\n"
    elif req.verb == "approve":
        _require(entry, ("in review",))
        entry.state = "approved"
        message = f"chore(plans): approve {req.slug}\n"
    elif req.verb in ("remove", "abandon"):
        _require(entry, ("approved",) if req.verb == "remove" else ("pending", "blocked", "in review", "approved"))
        open_prs = [p for p in gh.open_pulls() if p["ref"].startswith(f"plan/{req.slug}/")]
        if open_prs:
            numbers = ", ".join(f"#{p['number']}" for p in open_prs)
            raise Precondition(f"{req.slug} still has open PRs ({numbers}); land or close them first")
        if req.verb == "remove":
            # "No open PRs" alone would also describe an interrupted abandon or PRs the maintainer closed unmerged,
            # and removing the plan then would record work as landed that never reached main.
            if not req.merged:
                raise Usage("remove needs --merged with the numbers of the plan's PRs that landed")
            for number in req.merged:
                pull = gh.pull(number)
                if not pull["ref"].startswith(f"plan/{req.slug}/"):
                    raise Usage(f"#{number} is not one of {req.slug}'s PRs (head {pull['ref']})")
                if not pull["merged"]:
                    raise Precondition(f"#{number} has not merged")
        dependents = [e for e in index.entries if req.slug in e.deps]
        if dependents and req.verb == "abandon":
            touched |= _settle_dependents(req, snap, dependents, writes)
        index.remove(req.slug)
        writes[plan] = None
        if report_path(req.slug) in snap.files:
            writes[report_path(req.slug)] = None
        if req.verb == "remove":
            message = f"chore(plans): remove the landed plan {req.slug}\n"
        else:
            message = f"chore(plans): abandon {req.slug}\n"
    else:
        raise Usage(f"unknown verb {req.verb}")

    writes[INDEX_PATH] = index.render()
    return Change(writes=writes, message=message, touched=touched)


def _settle_dependents(req: Request, snap: Snapshot, dependents: list[Entry], writes: dict) -> set[str]:
    """Decide what happens to plans that wait on a plan being abandoned.

    Removing the line would silently satisfy their `(after ...)` clause, and they would be built against a main that
    lacks the work they were written on top of. So abandoning refuses until the maintainer says which: `block` puts
    each dependent in front of them with a generated question, `drop-dep` keeps them pending without the dependency.
    """
    names = ", ".join(e.slug for e in dependents)
    if req.dependents is None:
        raise Usage(f"{names} depend on {req.slug}; pass --dependents block or --dependents drop-dep")
    touched = set()
    for dep in dependents:
        touched.add(dep.slug)
        if req.dependents == "drop-dep":
            dep.deps = [d for d in dep.deps if d != req.slug]
            continue
        if dep.state != "pending":
            raise Precondition(
                f"{dep.slug} depends on {req.slug} but is [{dep.state}]; settle it first or use drop-dep"
            )
        question = (
            f"Blocked on {req.today} because the plan it runs after, `{req.slug}.md`, was abandoned before landing. "
            f"This plan was written assuming that work would be on main. Decide whether it should proceed without it, "
            f"be revised, or be abandoned too."
        )
        writes[plan_path(dep.slug)] = append_section(snap.read(plan_path(dep.slug)), "Blocked", question)
        dep.state = "blocked"
        dep.deps = [d for d in dep.deps if d != req.slug]
    return touched


def apply(
    gh: GitHub,
    req: Request,
    branch: str,
    formatter: Formatter,
    *,
    attempts: int = 8,
    sleep: Callable[[float], None] = time.sleep,
    log: Callable[[str], None] = lambda s: print(s, file=sys.stderr),
) -> str:
    """Publish one transition as a commit on `branch`; return the new commit id.

    The loop is the compare-and-swap. Each attempt reads the branch head H, computes the change on top of H, and moves
    the branch to a commit whose only parent is H with a non-forced update. GitHub accepts that only while the branch
    is still at H, so a concurrent writer makes the update fail as not-a-fast-forward and the attempt starts over from
    the new head, where the precondition is checked again. Main is never force-updated.

    A failure other than not-a-fast-forward may still have landed (a response lost on the way back), so before
    reporting an error the head is re-read: if it is the new commit or descends from it, the transition happened.
    """
    cache: dict[str, str] = {}
    for attempt in range(1, attempts + 1):
        head = gh.head(branch)
        snap, root_tree = remote_snapshot(gh, head, cache)
        try:
            change = plan_transition(req, snap, gh)
        except Precondition:
            # A read just after someone else's write can in principle be stale. Look once more before telling the
            # caller it lost, since "lost" makes an executor drop work.
            sleep(2.0)
            head = gh.head(branch)
            snap, root_tree = remote_snapshot(gh, head, cache)
            change = plan_transition(req, snap, gh)
        config = _read_root_file(gh, root_tree, "dprint.json")
        for path, content in list(change.writes.items()):
            if content is not None and path != INDEX_PATH:
                change.writes[path] = formatter(content, path, config)
        for path in change.writes:
            if not path.startswith(WRITABLE_PREFIXES):
                raise Failure(f"refusing to write {path}: outside {QUEUE_DIR}/ and {REPORTS_DIR}/")
        after_files = dict(snap.files)
        for path, content in change.writes.items():
            if content is None:
                after_files.pop(path, None)
            else:
                after_files[path] = "new:" + hashlib.sha256(content.encode()).hexdigest()
        after = Snapshot(
            files=after_files,
            read=lambda p, w=change.writes, s=snap: w[p] if p in w and w[p] is not None else s.read(p),
        )
        new, old = new_violations(snap, after, change.touched)
        for problem in old:
            log(f"warning: already on {branch}: {problem}")
        if new:
            raise Failure("the change would break the queue: " + "; ".join(new))
        entries = []
        for path, content in sorted(change.writes.items()):
            if content is None:
                entries.append({"path": path, "mode": "100644", "type": "blob", "sha": None})
            else:
                entries.append({"path": path, "mode": "100644", "type": "blob", "content": content})
        tree = gh.create_tree(root_tree, entries)
        commit = gh.create_commit(change.message, tree, head)
        try:
            gh.update_ref(branch, commit)
            return commit
        except NotFastForward:
            # A refused update may still have landed when the refusal came from a concurrent update racing ours; if
            # the branch contains our commit, the next attempt would wrongly see our own change as someone else's.
            if gh.is_ancestor_or_same(commit, gh.head(branch)):
                return commit
            log(f"attempt {attempt}: {branch} moved; reading it again")
            sleep(random.uniform(0.5, 4.0))
            continue
        except Failure as err:
            try:
                if gh.is_ancestor_or_same(commit, gh.head(branch)):
                    return commit
            except Failure:
                pass
            raise Failure(f"updating {branch} failed and it does not contain the new commit: {err}") from err
    raise Failure(f"contended: {branch} kept moving through {attempts} attempts")


def _read_root_file(gh: GitHub, root_tree: str, name: str) -> str:
    for entry in gh.tree(root_tree):
        if entry["path"] == name and entry["type"] == "blob":
            return gh.blob(entry["sha"])
    raise Failure(f"{name} is missing on the branch")


# --- Read-only verbs ------------------------------------------------------------------------------------------------


def wake_check(gh: GitHub, baseline: str, ref: str) -> bool:
    """Whether `ref` holds anything an idle executor should wake for, compared with the baseline commit.

    The plans watcher (scripts/plans-watch.sh) calls this when the plans/ tree hash changes. Claims are the most
    frequent commits on main and can never give an idle executor work, so a difference made only of `[pending]` lines
    becoming `[in-flight …]`, or of one claim replacing another, is ignored. Everything else wakes: a new or removed
    line, any other state change (including a claim given back, which makes a plan pending again), changed
    dependencies, and any change to a plan file or report. This is a comparison with the baseline commit rather than a
    digest of one commit, because a claim and the release of a claim look the same to any function of a single state.
    """
    if not OBJECT_ID_RE.match(ref):
        ref = gh.head(ref)
    cache: dict[str, str] = {}
    before, _ = remote_snapshot(gh, baseline, cache)
    after, _ = remote_snapshot(gh, ref, cache)
    if {p: s for p, s in before.files.items() if p != INDEX_PATH} != {
        p: s for p, s in after.files.items() if p != INDEX_PATH
    }:
        return True
    if INDEX_PATH not in before.files or INDEX_PATH not in after.files:
        return (INDEX_PATH in before.files) != (INDEX_PATH in after.files)
    if before.files[INDEX_PATH] == after.files[INDEX_PATH]:
        return False
    old, old_problems = parse_index(before.read(INDEX_PATH))
    new, new_problems = parse_index(after.read(INDEX_PATH))
    if old_problems != new_problems or [e.slug for e in old.entries] != [e.slug for e in new.entries]:
        return True
    for a, b in zip(old.entries, new.entries):
        if (a.deps, a.summary) != (b.deps, b.summary):
            return True
        if a.state == b.state and a.claim == b.claim:
            continue
        if a.state in ("pending", "in-flight") and b.state == "in-flight":
            continue
        return True
    return False


def history(gh: GitHub, branch: str, slug: str, limit: int) -> list[str]:
    """One line per queue commit about `slug` among the last `limit` commits to INDEX.md: date, subject, claim id.

    This is how an executor that crashed mid-verb finds out whether its own `claim`, `block`, `deliver` or `unclaim`
    landed: every commit a claim holder makes carries `Claim <id>.` in its message.
    """
    out = []
    for item in gh.path_history(branch, INDEX_PATH, limit):
        message = item["commit"]["message"]
        subject = message.split("\n", 1)[0]
        if not subject.startswith("chore(plans): ") or not subject.endswith(f" {slug}"):
            continue
        claim = re.search(r"^Claim ([0-9a-f]{6})\.$", message, re.MULTILINE)
        out.append(f"{item['commit']['committer']['date']} {subject}" + (f" (claim {claim[1]})" if claim else ""))
    return out


def claim_age(gh: GitHub, branch: str, slug: str, claim: str) -> str | None:
    """The time the claim commit landed, from main's history of INDEX.md, or None if it is not among recent commits."""
    for item in gh.path_history(branch, INDEX_PATH, 100):
        message = item["commit"]["message"]
        if message.startswith(f"chore(plans): claim {slug}\n") and f"Claim {claim}." in message:
            return item["commit"]["committer"]["date"]
    return None


def status(gh: GitHub, branch: str, logs_dir: pathlib.Path, now: datetime.datetime) -> dict:
    """The queue as executors and review sessions start from it: one snapshot instead of each agent deriving its own.

    Also flags what a person should look at: a claim that is old and whose working log has gone quiet (possibly an
    executor that died), an approved plan with no open PRs (a landing that stopped before `remove`), and a pending plan
    that still has open PRs (unlanded work from an earlier round, which conflict judgments must count).
    """
    head = gh.head(branch)
    snap, _ = remote_snapshot(gh, head, {})
    index, problems = parse_index(snap.read(INDEX_PATH)) if INDEX_PATH in snap.files else (Index("", []), [])
    pulls = gh.open_pulls()
    plans = []
    for entry in index.entries:
        prs = sorted(
            ({"number": p["number"], "ref": p["ref"]} for p in pulls if p["ref"].startswith(f"plan/{entry.slug}/")),
            key=lambda p: p["ref"],
        )
        log = logs_dir / f"farhelm-plan-{entry.slug}-log.md"
        log_age = None
        if log.exists():
            log_age = (now.timestamp() - log.stat().st_mtime) / 3600
        item = {
            "slug": entry.slug,
            "state": entry.state,
            "claim": entry.claim,
            "deps_waiting": [d for d in entry.deps if index.find(d)],
            "open_prs": prs,
            "log_quiet_hours": None if log_age is None else round(log_age, 1),
            "flags": [],
        }
        if entry.state == "in-flight" and entry.claim:
            since = claim_age(gh, branch, entry.slug, entry.claim)
            item["claimed_at"] = since
            if since is None:
                item["flags"].append("claim commit not in recent history: an old claim; check whether it is abandoned")
            else:
                hours = (now - datetime.datetime.fromisoformat(since.replace("Z", "+00:00"))).total_seconds() / 3600
                if hours > 24 and (log_age is None or log_age > 6):
                    item["flags"].append("possibly abandoned: claimed over 24 h ago and its working log is quiet")
        if entry.state == "approved" and not prs:
            merged = gh.merged_pulls(f"plan/{entry.slug}/")
            if merged:
                numbers = ",".join(str(p["number"]) for p in merged)
                item["flags"].append(f"approved, no open PRs, merged {numbers}: finish landing with remove --merged")
            else:
                item["flags"].append(
                    "approved, no open PRs and none found merged (search can lag a minute): ask the maintainer"
                )
        if entry.state == "pending" and prs:
            item["flags"].append("unlanded work from an earlier round")
        plans.append(item)
    return {"head": head, "problems": problems, "plans": plans}


def render_status(data: dict) -> str:
    lines = [f"head: {data['head']}"]
    for problem in data["problems"]:
        lines.append(f"PROBLEM: {problem}")
    for plan in data["plans"]:
        state = plan["state"] + (f" {plan['claim']}" if plan["claim"] else "")
        parts = [f"[{state}] {plan['slug']}"]
        if plan["deps_waiting"]:
            parts.append("waits on " + ", ".join(plan["deps_waiting"]))
        if plan["open_prs"]:
            parts.append("PRs " + ", ".join(f"#{p['number']}" for p in plan["open_prs"]))
        if plan.get("claimed_at"):
            parts.append(f"claimed {plan['claimed_at']}")
        if plan["log_quiet_hours"] is not None:
            parts.append(f"log quiet {plan['log_quiet_hours']} h")
        lines.append(" | ".join(parts))
        for flag in plan["flags"]:
            lines.append(f"    ! {flag}")
    return "\n".join(lines)


# --- Command line ---------------------------------------------------------------------------------------------------


def _read_input(path: str | None, what: str) -> str:
    if path is None:
        raise Usage(f"{what} is required")
    try:
        text = pathlib.Path(path).read_text(encoding="utf-8")
    except OSError as err:
        raise Usage(f"cannot read {what} {path}: {err}") from err
    if not text.strip():
        raise Usage(f"{what} {path} is empty")
    return text


def _parse_numbers(text: str | None) -> list[int]:
    if text is None:
        return []
    try:
        return [int(part.strip().lstrip("#")) for part in text.split(",") if part.strip()]
    except ValueError as err:
        raise Usage(f"--merged must be PR numbers separated by commas, got {text!r}") from err


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--repo", help="OWNER/NAME of the GitHub repository (required for every remote verb)")
    parser.add_argument("--branch", default="main", help="branch holding the queue (default main; tests only)")
    sub = parser.add_subparsers(dest="verb", required=True)

    st = sub.add_parser("status", help="print the queue with claims, open PRs and flags")
    st.add_argument("--json", action="store_true")

    ck = sub.add_parser("check", help="validate the queue's invariants")
    ck.add_argument("--tree", help="checkout root to check (default: the current directory)")
    ck.add_argument("--ref", help="check a remote commit or branch instead of a local tree")
    ck.add_argument("--base", help="also refuse what a planning PR may not change relative to this remote ref")

    cs = sub.add_parser("check-slug", help="refuse a slug that ever existed under plans/")
    cs.add_argument("slug")

    hs = sub.add_parser("history", help="list the recent queue commits for one plan, newest first")
    hs.add_argument("slug")
    hs.add_argument("--limit", type=int, default=100, help="how many INDEX.md commits to look through")

    wk = sub.add_parser("wake-check", help="print wake or ignore (the watcher's change filter)")
    wk.add_argument("--baseline", required=True)
    wk.add_argument("--ref", required=True)

    for verb, needs_claim, text_flag in (
        ("claim", True, None),
        ("unclaim", True, None),
        ("block", True, "--question"),
        ("deliver", True, "--report"),
        ("release", False, None),
        ("answer", False, "--decision"),
        ("follow-up", False, "--decision"),
        ("approve", False, None),
        ("remove", False, None),
        ("abandon", False, None),
    ):
        p = sub.add_parser(verb)
        p.add_argument("slug")
        if needs_claim:
            p.add_argument("--claim", required=True, help="six lowercase hex characters, chosen by the executor")
        if text_flag:
            p.add_argument(text_flag, dest="text_file", required=True, metavar="FILE")
        if verb == "abandon":
            p.add_argument("--dependents", choices=("block", "drop-dep"))
        if verb == "remove":
            p.add_argument(
                "--merged",
                required=True,
                help="comma-separated numbers of the plan's PRs that landed; each must be merged",
            )
    return parser


def _need_repo(args) -> str:
    if not args.repo or not REPO_RE.match(args.repo):
        raise Usage("--repo OWNER/NAME is required (a jj workspace has no .git for gh to infer it from)")
    if not BRANCH_RE.match(args.branch):
        raise Usage(f"--branch must be a plain branch name, got {args.branch!r}")
    return args.repo


def main(
    argv: list[str],
    gh_factory: Callable[[str], GitHub] = GhApi,
    formatter: Formatter = dprint_formatter,
    sleep: Callable[[float], None] = time.sleep,
) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.verb == "check" and not args.ref and not args.base:
            root = pathlib.Path(args.tree or ".")
            found = violations(local_snapshot(root))
            for problem in found:
                print(problem)
            return EXIT_VIOLATIONS if found else EXIT_OK

        repo = _need_repo(args)
        gh = gh_factory(repo)

        if args.verb == "check":
            if args.ref:
                ref = args.ref if OBJECT_ID_RE.match(args.ref) else gh.head(args.ref)
                snap, _ = remote_snapshot(gh, ref, {})
            else:
                snap = local_snapshot(pathlib.Path(args.tree or "."))
            found = violations(snap)
            if args.base:
                base = args.base if OBJECT_ID_RE.match(args.base) else gh.head(args.base)
                base_snap, _ = remote_snapshot(gh, base, {})
                found += base_violations(base_snap, snap)
            for problem in found:
                print(problem)
            return EXIT_VIOLATIONS if found else EXIT_OK
        if args.verb == "check-slug":
            if not SLUG_RE.match(args.slug):
                raise Usage(f"bad slug {args.slug!r}")
            paths = [f"plans/{args.slug}.md", plan_path(args.slug), report_path(args.slug)]
            used = [p for p in paths if gh.path_history(args.branch, p, 1)]
            for path in used:
                print(f"{args.slug}: {path} exists or existed on {args.branch}")
            return EXIT_VIOLATIONS if used else EXIT_OK
        if args.verb == "wake-check":
            if not OBJECT_ID_RE.match(args.baseline):
                raise Usage("--baseline must be a commit id")
            print("wake" if wake_check(gh, args.baseline, args.ref) else "ignore")
            return EXIT_OK
        if args.verb == "history":
            if not SLUG_RE.match(args.slug):
                raise Usage(f"bad slug {args.slug!r}")
            for line in history(gh, args.branch, args.slug, args.limit):
                print(line)
            return EXIT_OK
        if args.verb == "status":
            logs_dir = pathlib.Path(__file__).resolve().parents[2]
            data = status(gh, args.branch, logs_dir, datetime.datetime.now(datetime.timezone.utc))
            print(json.dumps(data, indent=2) if args.json else render_status(data))
            return EXIT_OK

        if not SLUG_RE.match(args.slug):
            raise Usage(f"bad slug {args.slug!r}")
        claim = getattr(args, "claim", None)
        if claim is not None and not CLAIM_RE.match(claim):
            raise Usage(f"--claim must be six lowercase hex characters, got {claim!r}")
        text = None
        if getattr(args, "text_file", None) is not None:
            text = _read_input(args.text_file, "the input file")
            problems = hygiene_problems(text, local_hostnames())
            if args.verb != "deliver":
                problems += splice_problems(text)
            if problems:
                raise Usage("the text cannot be committed as it is; fix and call again:\n  " + "\n  ".join(problems))
        req = Request(
            verb=args.verb,
            slug=args.slug,
            claim=claim,
            text=text,
            dependents=getattr(args, "dependents", None),
            merged=_parse_numbers(getattr(args, "merged", None)),
            today=datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        )
        commit = apply(gh, req, args.branch, formatter, sleep=sleep)
        print(commit)
        return EXIT_OK
    except Usage as err:
        print(f"usage: {err}", file=sys.stderr)
        return EXIT_USAGE
    except Precondition as err:
        print(str(err), file=sys.stderr)
        return EXIT_PRECONDITION
    except Failure as err:
        print(f"error: {err}", file=sys.stderr)
        return EXIT_ERROR
    except Exception as err:  # noqa: BLE001 - anything unexpected is "ambiguous", never a check result (exit 1)
        print(f"error: unexpected {type(err).__name__}: {err}", file=sys.stderr)
        return EXIT_ERROR


if __name__ == "__main__":
    # The plans watcher runs `wake-check` under a timeout and stops it with SIGTERM. Turning the signal into an exit
    # lets subprocess.run kill the `gh` request in flight, which Python's default handling would leave running.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    sys.exit(main(sys.argv[1:]))
