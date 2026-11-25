#!/usr/bin/env python3
"""The changelog's mechanical checks (file format, fragment coverage, what dist will publish) and plans' release notes.

`releasing/.agents/changelog.md` describes the changelog process; this script is the part of it that a machine can
hold. It exists because the curated `CHANGELOG.md` is read by cargo-dist at tag time and copied into the GitHub release,
and a file that looks right to a human can still be one dist parses into nothing or into the wrong section. Nothing here
writes to the repository; every mode reports and exits non-zero on a problem.

## The modes

`format` lints `CHANGELOG.md` and the fragments under `releasing/changelog.d/` against the layouts the process has used:
one `## vX.Y.Z - YYYY-MM-DD` heading per stable release, newest first; recognized category headings in their prescribed
order; and entries with the required PR references. Current sections use one-paragraph bullets in every category, and
end with the folded full list of every change (a `<details>` block with its own `####` categories) below the
highlighted ones, which must cite every PR the highlights cite. Older curated sections retain their historical layouts,
such as a fold of only the changes not highlighted, Misc fixes, or Highlights subsections. The newest
section, and only that one, may open with the fixed draft notice while its wording is still in review. The checker is
deliberately narrower than what dist accepts, so a violation here is a curation slip, not a dist failure. It runs in the
release gate on every tag.

`fragments` answers "did every user-facing change since the last stable release leave a changelog fragment". It walks
the commits since the merge base with the last stable tag, picks out the Conventional Commit types the process says
must have an entry (`feat`, `fix`, `perf`, `style`, `revert`, and any type marked `!`), and reports which of them
added or edited a fragment, which are claimed by a fragment's `pr:` line, and which are missing. It is the release-time
sweep's input, not a gate: the maintainer decides what a missing entry means.

`plans` lists every plan (`plans/AGENTS.md`) whose landing was recorded on main since that same merge base, with the PRs
that landed it and the `## Release note` section its plan file held when it landed: an entry, one of the two fixed
answers that are not one, or nothing (a plan written before plans carried the section). It reads history rather than
the working tree because approving a landed plan deletes its file, usually long before the release. Like `fragments`,
it is curation input, not a gate.

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
  check-changelog.py plans [--repo PATH] [--since TAG]
  check-changelog.py plan-note FILE...
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

# The category tables ARE the format: names, emoji, and order are all fixed here and nowhere else.
# `releasing/.agents/changelog.md`, "Format", repeats them in prose for humans; if the two ever disagree, these tables
# are what the gate enforces.
#
# A current section has two tiers. Its `###` categories are the highlights: the entries the maintainer settled at
# planning time, wrote at release time, or promoted from the agent's proposals. Below them, folded away, comes the full
# list the agent writes of every change users could notice, highlighted or not (`FOLD_*` below). Fixes worth
# highlighting is the highlights' fix category; the fold lists every fix, those included, under its Fixed.
CATEGORIES: tuple[tuple[str, str], ...] = (
    ("Breaking", "\U0001f4a5"),  # collision
    ("Added", "\U0001f680"),  # rocket
    ("Changed", "\U0001f504"),  # counterclockwise arrows
    ("Fixes worth highlighting", "\U0001f527"),  # wrench
    ("Removed", "\U0001f5d1️"),  # wastebasket with emoji presentation selector
)

# The full list sits at the very end of a section, inside an HTML `<details>` block whose first two lines are exactly
# these. A `<details>` block in `CHANGELOG.md` itself, rather than a fold the website adds, is what makes the list start
# folded in the GitHub release body too, since dist copies the section verbatim and GitHub renders the element. The
# blank line after the summary and before `FOLD_CLOSE` is what lets GitHub and MDX read the Markdown inside as Markdown.
# The categories inside are `####` headings, so that the parser's `###` categories end where the fold begins.
FOLD_OPEN = "<details>"
FOLD_SUMMARY = "<summary>All changes</summary>"
FOLD_CLOSE = "</details>"

# The fold started out as the list of only the changes the highlights left out, titled to match. The maintainer made it
# the list of every change on 2026-11-21, so that removing, rewording or merging highlights during review never changes
# what the full list holds and can never drop a change from the notes. The sections curated before that keep their
# title and their narrower contents; `website/scripts/release-notes.mjs` reads the same split to count each release's
# changes for the desktop app.
LEGACY_FOLD_SUMMARY = "<summary>All other changes</summary>"
LEGACY_FOLD_SUMMARY_VERSIONS = frozenset({"0.42.0", "0.43.0"})


def fold_summary_for(version: str) -> str:
    """The exact summary line a fold-era section of this version must use."""
    return LEGACY_FOLD_SUMMARY if version in LEGACY_FOLD_SUMMARY_VERSIONS else FOLD_SUMMARY


def fold_lists_every_change(version: str) -> bool:
    """Whether this version's fold is the complete list, so it must exist and cite every highlighted PR."""
    return _version_tuple(version) >= FIRST_FOLD_VERSION and version not in LEGACY_FOLD_SUMMARY_VERSIONS
FOLD_CATEGORIES: tuple[tuple[str, str], ...] = (
    ("Breaking", "\U0001f4a5"),
    ("Added", "\U0001f680"),
    ("Changed", "\U0001f504"),
    ("Fixed", "\U0001f527"),
    ("Removed", "\U0001f5d1️"),
)

# The first version curated with the fold. Sections before it keep whichever earlier table `categories_for` gives them,
# and may not carry a fold.
FIRST_FOLD_VERSION = (0, 42, 0)

# GitHub renders raw HTML inside Markdown, so an entry that mentions `</details>` in plain text would close the fold
# early on the GitHub release (the website escapes it, so the two would silently disagree). In a fold-era section the
# frame's tags may appear only as the frame itself, or inside a code span. The code-span pattern is the same heuristic
# the website generator uses, which is enough for the entries this file holds.
FRAME_TAG_RE = re.compile(r"</?\s*(details|summary)\b", re.IGNORECASE)
CODE_SPAN_RE = re.compile(r"`+[^`]*`+")

# From the fix split until the fold replaced it, the long tail of fixes went under a Misc fixes category at the end of
# the section, listed in full for transparency. Sections from v0.26.0 through v0.41.0 use this table.
MISC_FIXES_CATEGORIES: tuple[tuple[str, str], ...] = (
    *CATEGORIES,
    ("Misc fixes", "\U0001fa79"),  # adhesive bandage
)

# Misc fixes opens with exactly this paragraph, then its bullets. The fixed wording is what tells a reader of the GitHub
# release or the docs website why a release lists fixes they would not care about; a curator rewording it per release
# would only add a second thing to review.
MISC_FIXES_PREAMBLE = (
    '*Fixes that would normally be summed up as a single "various bug fixes" line. In the interest of transparency, the'
    " complete list is included.*"
)

# A stable section opens with exactly this line while the maintainer is still reviewing its wording. The tag is pushed
# with the draft so the release builds during that review, and dist copies the line into the GitHub release along with
# everything else, so anyone reading the release page or a watcher's notification before finalization knows the text
# will change. Finalization (`releasing/finalize-release-notes.py`) replaces the whole section, notice included. Only
# the newest section may carry it: a release cut on top of an unfinished one would bury the draft where nobody looks.
DRAFT_NOTICE = "**DRAFT release notes - not yet finalized**"

# Historical sections used these categories before the current format. They remain accepted so changing the release
# process never forces a rewrite of approved or in-flight release notes. New sections must follow `CATEGORIES`; these
# compatibility tables are not a recommendation for new curation.
#
# Highlights was a prose category above Breaking. These sections were already curated under it; a new release version
# must not silently revive it, so add a version here only when an in-flight section was approved before a format change.
LEGACY_HIGHLIGHTS: tuple[str, str] = ("Highlights", "\u2728")  # sparkles
LEGACY_HIGHLIGHTS_VERSIONS = frozenset({"0.12.0", "0.13.0"})
# Before the fix split, every fix went under one Fixed category in place of the two fix categories. Sections up to and
# including this version use it, and only those may.
PRE_SPLIT_CATEGORIES: tuple[tuple[str, str], ...] = (
    ("Breaking", "\U0001f4a5"),
    ("Added", "\U0001f680"),
    ("Changed", "\U0001f504"),
    ("Fixed", "\U0001f527"),
    ("Removed", "\U0001f5d1️"),
)
LAST_PRE_SPLIT_VERSION = (0, 25, 0)

# v0.40.0 opens with the new editorial policy. Those bullets describe future notes and its Changed overview describes
# a migration that spanned hundreds of PRs. Citations are optional for those specific entries.
RELEASE_NOTES_POLICY_VERSION = "0.40.0"
RELEASE_NOTES_POLICY: tuple[str, str] = ("Changes to release notes going forward", "")


def categories_for(version: str) -> tuple[tuple[str, str], ...]:
    """The categories a release section of this version may use, in their required order.

    A version is held to the layout that was current when it was curated, which is what lets the format change without
    a rewrite of the sections already published under the old one. `version` must already be a valid `X.Y.Z`.
    """
    if _version_tuple(version) <= LAST_PRE_SPLIT_VERSION:
        categories = PRE_SPLIT_CATEGORIES
    elif _version_tuple(version) < FIRST_FOLD_VERSION:
        categories = MISC_FIXES_CATEGORIES
    else:
        categories = CATEGORIES
    if version in LEGACY_HIGHLIGHTS_VERSIONS:
        categories = (LEGACY_HIGHLIGHTS, *categories)
    if version == RELEASE_NOTES_POLICY_VERSION:
        categories = (RELEASE_NOTES_POLICY, *categories)
    return categories


# Fragment kinds map onto the bullet categories, with `fixed` covering both fix categories: which of the two a fix lands
# in is decided at curation, against the whole release, not by the PR author. `none` records a deliberate "considered,
# nothing user-facing" so the coverage sweep can tell an omission from a decision.
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

    The rules encode the layout `releasing/.agents/changelog.md`, "Format", stipulates. They are stricter than dist
    needs so the file stays uniform across releases: dist only cares that a release heading starts with the version,
    but a reader scanning twenty releases cares that every one of them is shaped the same way.
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
        may_be_draft = release is releases[0]
        # GitHub's HTML block that `</details>` starts runs until a blank line, so the next release's heading directly
        # below it would render as plain text when the file itself is viewed on GitHub.
        if release is not releases[-1] and release.body and release.body[-1] == FOLD_CLOSE:
            problems.append(f"{where} (v{release.version}): `{FOLD_CLOSE}` needs a blank line below it before the next release")
        problems.extend(
            f"{where} (v{release.version}): {item}"
            for item in _check_release_body(release.version, release.body, may_be_draft)
        )
    return problems


def is_draft(release: Release) -> bool:
    """Whether the section still opens with `DRAFT_NOTICE`, meaning the maintainer has not yet approved its wording."""
    first = next((line for line in release.body if line.strip()), "")
    return first == DRAFT_NOTICE


def _check_release_body(version: str, lines: list[str], may_be_draft: bool = False) -> list[str]:
    """Validate the version's category set and order, and the shape each category's body must have.

    `may_be_draft` admits `DRAFT_NOTICE` as the section's first line and nothing else before the first category. It is
    true only for the newest section; an older section carrying the notice is reported as unfinished rather than as
    stray text, so the message says what to do about it.
    """
    problems: list[str] = []
    first = next((index for index, line in enumerate(lines) if line.strip()), None)
    if first is not None and lines[first] == DRAFT_NOTICE:
        if not may_be_draft:
            problems.append("only the newest section may still be a draft; finalize this one before cutting another release")
        lines = lines[first + 1 :]
    if _version_tuple(version) >= FIRST_FOLD_VERSION:
        problems.extend(
            f"`{line[:60]}`: a `details` or `summary` tag outside code spans could open or close the fold on GitHub;"
            " put it in backticks"
            for line in lines
            if line not in (FOLD_OPEN, fold_summary_for(version), FOLD_CLOSE)
            and FRAME_TAG_RE.search(CODE_SPAN_RE.sub("", line))
        )
    lines, fold, fold_problems = _split_fold(version, lines)
    problems.extend(fold_problems)

    allowed = categories_for(version)
    categories, stray, heading_problems = _split_categories(lines, "### ", allowed)
    problems.extend(heading_problems)
    if any(line.strip() for line in stray):
        problems.append("text before the first category heading; a release holds only categories and the fold")
    # A release whose every change went into the full list has no highlights, and that is a legitimate release.
    if not categories and fold is None:
        problems.append("a release needs at least one category")
    problems.extend(_check_categories(version, categories, allowed))
    # The complete fold is what makes the highlights a pure selection: whatever the maintainer removes or merges there,
    # every change is still in the full list. A PR a highlight cites and the fold does not is the one way to lose that,
    # so it is checked here rather than left to curation.
    if fold_lists_every_change(version) and categories:
        if fold is None:
            problems.append("the folded full list lists every change, the highlighted ones included, so it cannot be omitted")
        else:
            missing = sorted(_cited_prs(lines) - _cited_prs(fold))
            if missing:
                numbers = ", ".join(f"#{number}" for number in missing)
                problems.append(f"the highlights cite {numbers}, which the folded full list of every change does not")

    if fold is not None:
        fold_categories, fold_stray, fold_heading_problems = _split_categories(fold, "#### ", FOLD_CATEGORIES)
        fold_problems = list(fold_heading_problems)
        if any(line.strip() for line in fold_stray):
            fold_problems.append("text between the summary and the first category heading")
        if not fold_categories:
            fold_problems.append("the fold needs at least one category; omit the fold when it would be empty")
        fold_problems.extend(_check_categories(version, fold_categories, FOLD_CATEGORIES, historical=False))
        problems.extend(f"in the folded full list: {item}" for item in fold_problems)
    return problems


def _split_fold(version: str, lines: list[str]) -> tuple[list[str], list[str] | None, list[str]]:
    """Separate a section's highlights from its folded full list, checking the fold's fixed frame.

    Returns the lines above the fold, the lines between the summary and `FOLD_CLOSE` (None when there is no fold), and
    the problems with the frame. The fold must be the last thing in the section, because the website and GitHub both
    show it as one closed block at the end, and anything after it would read as part of the highlights while sitting
    below the full list. A section older than `FIRST_FOLD_VERSION` carrying one is refused rather than accepted, so a
    new section cannot be given a fold under an old version number and an old one cannot grow one in a sweep.
    """
    start = next((index for index, line in enumerate(lines) if line == FOLD_OPEN), None)
    if start is None:
        return lines, None, []
    above, rest = lines[:start], lines[start + 1 :]
    if _version_tuple(version) < FIRST_FOLD_VERSION:
        return above, None, [f"`{FOLD_OPEN}`: only sections curated from v{'.'.join(map(str, FIRST_FOLD_VERSION))} on have the folded full list"]

    problems: list[str] = []
    if above and above[-1].strip():
        problems.append(f"`{FOLD_OPEN}` needs a blank line above it")
    content = [index for index, line in enumerate(rest) if line.strip()]
    if not content or rest[content[-1]] != FOLD_CLOSE:
        problems.append(f"the fold must end with `{FOLD_CLOSE}` on the section's last line")
        end = len(rest)
    else:
        end = content[-1]
        if rest[end - 1].strip():
            problems.append(f"`{FOLD_CLOSE}` needs a blank line above it, or GitHub shows the last entry as raw Markdown")
    body = rest[:end]
    if any(line in (FOLD_OPEN, FOLD_CLOSE) for line in body):
        problems.append("a section has at most one fold, and nothing inside it may open or close another")
    summary = fold_summary_for(version)
    if not body or body[0] != summary:
        problems.append(f"`{FOLD_OPEN}` must be followed directly by exactly `{summary}`")
        return above, body, problems
    if len(body) < 2 or body[1].strip():
        problems.append(f"`{summary}` needs a blank line below it, or GitHub shows the fold's Markdown as raw text")
    return above, body[1:], problems


def _cited_prs(lines: list[str]) -> set[int]:
    """The PR numbers cited by the trailing references of the bullet entries in these lines.

    An entry is its `- ` line plus any indented continuation lines, joined, the same way `_check_bullets` reads it: a
    reference wrapped across lines passes that check, so reading line by line here would miss its numbers.
    """
    items: list[str] = []
    for line in lines:
        if not line.strip():
            continue
        if line.startswith("- "):
            items.append(line[2:])
        elif line.startswith("  ") and items:
            items[-1] += " " + line.strip()
    cited: set[int] = set()
    for item in items:
        reference = PR_REFERENCE_RE.search(item)
        if reference:
            cited.update(int(number) for number in re.findall(r"#(\d+)", reference.group(0)))
    return cited


def _split_categories(
    lines: list[str], prefix: str, allowed: tuple[tuple[str, str], ...]
) -> tuple[list[tuple[str, list[str]]], list[str], list[str]]:
    """Group lines under the category headings at one level (`### ` or `#### `), against the given table.

    Returns the categories in file order, each as its name and body lines ("?" for an unknown heading, which is also
    reported), the lines before the first heading, and the problems with headings. Fenced code is honored so a heading
    inside an example does not start a category.
    """
    headings = {f"{prefix}{emoji} {name}" if emoji else f"{prefix}{name}": name for name, emoji in allowed}
    categories: list[tuple[str, list[str]]] = []
    stray: list[str] = []
    problems: list[str] = []
    in_fence = False
    for line in lines:
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
        if not in_fence and line.startswith(prefix):
            name = headings.get(line)
            if name is None:
                listed = ", ".join(f"`{heading}`" for heading in headings)
                problems.append(f"unknown category heading `{line}`; the headings are {listed}")
                categories.append(("?", []))
            else:
                categories.append((name, []))
            continue
        (categories[-1][1] if categories else stray).append(line)
    return categories, stray, problems


def _check_categories(
    version: str, categories: list[tuple[str, list[str]]], allowed: tuple[tuple[str, str], ...], historical: bool = True
) -> list[str]:
    """Check category order and each category's body; `historical` enables the shapes only old `###` sections use.

    The fold's categories pass `historical=False`: a fold exists only in sections newer than every historical exception,
    and its Fixed must not be read as the pre-split Fixed or its Changed as v0.40.0's.
    """
    problems: list[str] = []
    order_of = {name: index for index, (name, _emoji) in enumerate(allowed)}
    last_order = -1
    for name, body in categories:
        if name == "?":
            continue
        order = order_of[name]
        if order <= last_order:
            problems.append(
                f"category `{name}` is out of order or repeated; the order is {', '.join(n for n, _e in allowed)}"
            )
        last_order = order
        if not any(line.strip() for line in body):
            problems.append(f"category `{name}` is empty; omit a category with no entries")
            continue
        if historical and name == LEGACY_HIGHLIGHTS[0]:
            category_problems = _check_highlights(body)
        elif historical and name == "Misc fixes":
            category_problems = _check_misc_fixes(body)
        elif historical and name == RELEASE_NOTES_POLICY[0]:
            category_problems = _check_bullets(body, uncited_leading_entries=None)
        elif historical and version == RELEASE_NOTES_POLICY_VERSION and name == "Changed":
            category_problems = _check_bullets(body, uncited_leading_entries=1)
        else:
            category_problems = _check_bullets(body)
        problems.extend(f"under `{name}`: {item}" for item in category_problems)
    return problems


def _check_misc_fixes(lines: list[str]) -> list[str]:
    """Misc fixes is the fixed preamble paragraph, then ordinary bullets; nothing may precede the preamble."""
    content = [index for index, line in enumerate(lines) if line.strip()]
    if lines[content[0]] != MISC_FIXES_PREAMBLE:
        return [f"the category must open with exactly this paragraph: {MISC_FIXES_PREAMBLE}"]
    rest = lines[content[0] + 1 :]
    if not any(line.strip() for line in rest):
        return ["the preamble needs at least one bullet entry after it; omit the category when there is none"]
    return _check_bullets(rest)


def _check_highlights(lines: list[str]) -> list[str]:
    """Validate the prose subsection shape retained by historical release sections."""
    problems: list[str] = []
    seen_subsection = False
    subsection: list[str] = []

    def check_subsection() -> None:
        """Require prose and a trailing PR reference in each historical subsection."""
        content = [line.strip() for line in subsection if line.strip()]
        if not content:
            problems.append("Highlights subsections need prose")
            return
        if any(line.startswith(("-", "#", ">", "```")) or re.match(r"\d+\.\s", line) for line in content):
            problems.append("Highlights subsections may contain prose paragraphs only")
        prose = [line for line in content if not PR_REFERENCE_RE.fullmatch(line)]
        if not prose:
            problems.append("Highlights subsections need prose")
        if not PR_REFERENCE_RE.search(" ".join(content)):
            problems.append("Highlights subsections must end with a PR reference, `(#N)` or `(#N, #M)`")

    for line in lines:
        if line.startswith("#### "):
            if not line[5:].strip():
                problems.append("Highlights subsections need a title")
            if seen_subsection:
                check_subsection()
            subsection = []
            seen_subsection = True
            continue
        if not seen_subsection and line.strip():
            problems.append("Highlights must start with a `#### ` subsection; loose text before it is not allowed")
            break
        if seen_subsection:
            subsection.append(line)
    if not seen_subsection:
        problems.append("Highlights needs at least one `#### ` subsection")
    else:
        check_subsection()
    return problems


def _check_bullets(lines: list[str], *, uncited_leading_entries: int | None = 0) -> list[str]:
    """Validate paragraph-sized bullets, with PR references for entries about product changes.

    The file is stored one paragraph per line (GitHub renders a release body like a comment, newline as line break),
    but an item is still taken to be the `- ` line plus any indented lines after it, and the PR reference is checked
    on the joined text, so a hand-wrapped bullet is flagged for its reference, not misread as several entries.
    Historical sections can exempt their opening entries or pass None to exempt a whole category.
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
            problems.append(f"heading `{line}` is not allowed here; categories contain only bullets")
        else:
            problems.append(f"free text `{line[:60]}` is not allowed here; every entry is a `- ` bullet")
    if not items and not problems:
        problems.append("no bullet entries")
    for index, item in enumerate(items):
        joined = " ".join(item)
        if not joined.strip():
            problems.append("empty bullet entry")
        elif uncited_leading_entries is not None and index >= uncited_leading_entries and not PR_REFERENCE_RE.search(joined):
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
    A `---` line directly under text is a setext heading to Markdown, and the first draft of this format learned that
    from dprint rewriting `kind: added` over a divider into `## kind: added`. The fragments are excluded from dprint
    now for a different reason (see `_check_bullets`), but the leading block is the shape every Markdown tool treats
    as metadata, so it stays.
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
    modified: list[str]
    deleted: list[str]
    claimed_by: list[str]

    @property
    def covered(self) -> bool:
        """A commit is covered when it touched the fragments deliberately: added one, edited one (a follow-up to
        unreleased work amends the original's entry), deleted one as a revert, or is claimed by a fragment's `pr:`."""
        return bool(
            self.added or self.modified or self.claimed_by or (self.subject.startswith("revert") and self.deleted)
        )


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
        added, modified, deleted = [], [], []
        status = _git(repo, "diff-tree", "--no-commit-id", "--name-status", "-r", "--root", sha, "--", str(FRAGMENTS_DIR))
        for line in filter(None, status.split("\n")):
            code, _tab, name = line.partition("\t")
            # Count only files discovery can load. A rename's destination is
            # the candidate; arbitrary files beneath this directory are not notes.
            name = name.split("\t")[-1] if code.startswith("R") else name
            path = Path(name)
            if path.parent != FRAGMENTS_DIR or path.suffix != ".md" or path.name == FRAGMENT_README:
                continue
            if code.startswith("A"):
                added.append(name)
            elif code.startswith("M"):
                modified.append(name)
            elif code.startswith("D"):
                deleted.append(name)
            elif code.startswith("R"):
                added.append(name)
        commits.append(CommitCoverage(sha, subject, required, pr, added, modified, deleted, claims.get(pr or -1, [])))

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
            evidence = commit.added or commit.modified or commit.claimed_by or commit.deleted
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
# Plans' release notes, recovered from history
# ---------------------------------------------------------------------------
#
# A plan (`plans/AGENTS.md`) carries the maintainer's release note for its work in a `## Release note` section, settled
# while the plan was planned. The release curation uses it as the default entry for that work
# (`releasing/.agents/changelog.md`, "Plans' release notes"). The plan file itself is gone by then more often than not:
# approving a landed plan's report deletes the file, and that usually happens long before the next stable release. What
# survives is history. The queue script records every landing as its own commit on main, and at that commit the plan
# file is still there and `REPORTS.md` names the PRs that landed it. So this mode reads the plan as it stood when it
# landed, which is also the version the release should be checked against: its Decisions are complete for that round.

PLANS_QUEUE_DIR = Path("plans") / "queue"
PLANS_REPORTS = Path("plans") / "REPORTS.md"

# The subject `scripts/plans-queue.py` gives the commit that moves a plan from INDEX.md to REPORTS.md. That script is the
# only writer of these commits; if it ever rewords the subject, this pattern has to follow.
LANDING_SUBJECT_RE = re.compile(r"^chore\(plans\): record the landing of (?P<slug>[A-Za-z0-9][A-Za-z0-9-]*)$")

# A landed plan's line in REPORTS.md, a copy of `REPORT_LINE_RE` in `scripts/plans-queue.py`, which owns the grammar.
REPORT_LINE_RE = re.compile(
    r"^- \[`(?P<slug>[^`]+)`\]\(reports/(?P<link>[^)]+)\.report\.md\) landed (?P<date>\d{4}-\d{2}-\d{2}) "
    r"in (?P<prs>#\d+(?:, #\d+)*): (?P<summary>.+)$"
)

RELEASE_NOTE_HEADING = "Release note"

# The two answers that are not an entry. Their wording is fixed by the rule this mode serves,
# `releasing/.agents/changelog.md`, "Plans' release notes", so that this mode can tell them from a short entry.
RELEASE_NOTE_NONE = "No release note needed."
RELEASE_NOTE_DEFERRED = "No release note now; ask during the release."

_PLAN_FENCE_RE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")


def plan_release_notes(text: str) -> list[str]:
    """The body of every `## Release note` section in a plan file, each stripped, in file order.

    A well-formed plan has exactly one. More than one happens when a revision appends a new answer below an old one
    instead of replacing it, and reading only the first would silently keep the stale answer, so callers treat any count
    other than one as malformed. Headings inside fenced code are not headings: plan files quote Markdown and shell, and a
    quoted `## Release note` in an example must not be read as the plan's own. The fence rule is the queue script's
    (`_scan_fences` in `scripts/plans-queue.py`): a fence closes only on a bare run of its own character at least as long
    as the opening, so a ```` ``` ```` quoted inside a four-backtick fence does not end it. A section ends at the next
    level-two heading (the queue script appends `## Decisions` and `## Blocked` at the end of the file) or at the end of
    the file.
    """
    bodies: list[list[str]] = []
    current: list[str] | None = None
    fence: str | None = None
    for line in text.splitlines():
        match = _PLAN_FENCE_RE.match(line)
        if fence is None and match and not (match[1][0] == "`" and "`" in match[2]):
            fence = match[1]
        elif fence is not None and match and match[1][0] == fence[0] and len(match[1]) >= len(fence) and not match[2].strip():
            fence = None
        elif fence is None and line.startswith("## "):
            current = None
            if line[3:].strip() == RELEASE_NOTE_HEADING:
                current = []
                bodies.append(current)
            continue
        if current is not None:
            current.append(line)
    return ["\n".join(body).strip() for body in bodies]


def check_plan_note(text: str) -> list[str]:
    """What is wrong with a plan file's `## Release note`, for a planning PR to check before it lands.

    The planning flow writes every new or revised plan with exactly one non-empty section; a plan written before that
    rule has none, which is fine for history but not for a plan being written now, so a missing section is a problem
    here even though the `plans` mode accepts it.
    """
    notes = plan_release_notes(text)
    if not notes:
        return [f"no ## {RELEASE_NOTE_HEADING} section"]
    if len(notes) > 1:
        return [f"{len(notes)} ## {RELEASE_NOTE_HEADING} sections; a plan has exactly one"]
    if not notes[0]:
        return [f"the ## {RELEASE_NOTE_HEADING} section is empty"]
    return []


@dataclass
class PlanNote:
    """One plan whose landing was recorded since the last stable release, with its note as of its latest landing.

    A plan can land more than once in a release cycle: a follow-up puts it back on the queue and its next round lands
    again. `record` and `notes` come from the latest round, whose plan file carries every Decision the earlier ones did,
    and the PRs of every round in range are split by where they actually merged. That split is what decides which
    release the work belongs to, not the landing record: the monitor merges a plan's PRs first and records the landing
    afterwards, so a stable release cut in between ships PRs whose record falls in the next cycle (`prs_earlier` then
    holds every one of them). `earlier_round` says a round's record predates the range, so part of the work may already
    have shipped and the note may describe it again.
    """

    slug: str
    record: str
    dates: list[str]
    prs_in_range: list[int]
    prs_earlier: list[int]
    notes: list[str]
    decisions: bool
    earlier_round: bool

    @property
    def note(self) -> str | None:
        return self.notes[0] if len(self.notes) == 1 else None

    @property
    def status(self) -> str:
        if not self.notes:
            return "not recorded"
        if len(self.notes) > 1:
            return f"malformed ({len(self.notes)} sections)"
        if self.notes[0] == "":
            return "malformed (empty)"
        if self.notes[0] == RELEASE_NOTE_NONE:
            return "none needed"
        if self.notes[0] == RELEASE_NOTE_DEFERRED:
            return "ask during the release"
        return "entry"


def plan_notes(repo: Path, since: str | None = None) -> tuple[str, list[PlanNote], list[str]]:
    """Find every plan whose landing was recorded since the merge base with the last stable tag, and its release note.

    The range is the same as the fragment sweep's, for the same reason: a stable tag lives on a release branch that never
    merges. A PR counts as merged in range when a commit in range carries its `(#N)` subject suffix, which is how every
    squash merge on main is titled.

    What this does NOT find: a plan still listed in INDEX.md whose PRs are already on main (a landing in progress, or one
    that blocked partway through). It has no landing record yet, so its merged PRs are curated from their fragments; when
    it lands in a later cycle, this mode reports those PRs as merged before that cycle's range.

    Returns the tag used, the plans in order of their first landing in range, and problems: a landing record whose
    commit lacks the plan file, or does not add a REPORTS.md line for that plan linking its own report. Either means the
    history is not what the queue script writes.
    """
    tag = since or last_stable_tag(repo)
    try:
        base = _git(repo, "merge-base", tag, "HEAD").strip()
    except subprocess.CalledProcessError as failure:
        raise Problem(f"cannot find the merge base of {tag} and HEAD: {failure.stderr.strip()}") from failure

    def records(revisions: str) -> list[tuple[str, str]]:
        # Not limited to REPORTS.md's path: a landing commit that failed to touch REPORTS.md must still be seen, so that
        # it can be reported rather than silently skipped.
        log = _git(repo, "log", "--reverse", "--format=%H%x00%s", revisions)
        found = []
        for row in filter(None, log.split("\n")):
            sha, subject = row.split("\x00", 1)
            match = LANDING_SUBJECT_RE.match(subject)
            if match:
                found.append((sha, match.group("slug")))
        return found

    in_range_prs = {
        int(match.group(1))
        for subject in _git(repo, "log", "--format=%s", f"{base}..HEAD").split("\n")
        if (match := SUBJECT_PR_RE.search(subject))
    }
    earlier = {slug for _sha, slug in records(base)}

    def report_line(sha: str, slug: str) -> re.Match[str] | None:
        try:
            text = _git(repo, "show", f"{sha}:{PLANS_REPORTS}")
        except subprocess.CalledProcessError:
            return None
        return next((m for m in map(REPORT_LINE_RE.match, text.splitlines()) if m and m.group("slug") == slug), None)

    plans: dict[str, PlanNote] = {}
    problems: list[str] = []
    for sha, slug in records(f"{base}..HEAD"):
        line = report_line(sha, slug)
        before = report_line(f"{sha}^", slug)
        if line is None or line.group("link") != slug or (before is not None and before.group(0) == line.group(0)):
            problems.append(f"{sha[:9]} records the landing of {slug}, but does not add a REPORTS.md line linking its report")
            continue
        try:
            plan_text = _git(repo, "show", f"{sha}:{PLANS_QUEUE_DIR / (slug + '.md')}")
        except subprocess.CalledProcessError:
            problems.append(f"{sha[:9]} records the landing of {slug}, but the plan file is missing there")
            continue
        prs = [int(pr) for pr in re.findall(r"#(\d+)", line.group("prs"))]
        notes = plan_release_notes(plan_text)
        decisions = bool(re.search(r"^## Decisions\s*$", plan_text, re.MULTILINE))
        plan = plans.setdefault(slug, PlanNote(slug, sha, [], [], [], notes, decisions, slug in earlier))
        plan.record, plan.notes, plan.decisions = sha, notes, decisions
        plan.dates.append(line.group("date"))
        for pr in prs:
            bucket = plan.prs_in_range if pr in in_range_prs else plan.prs_earlier
            if pr not in plan.prs_in_range and pr not in plan.prs_earlier:
                bucket.append(pr)
    return tag, list(plans.values()), problems


def report_plan_notes(tag: str, plans: list[PlanNote], problems: list[str]) -> int:
    """Print each landed plan's note for the curating agent. Exit 1 for malformed history or a malformed note."""
    print(f"plans whose landing was recorded since the merge base with {tag}: {len(plans)}")
    malformed = 0
    for plan in plans:
        listed = ", ".join(f"#{pr}" for pr in plan.prs_in_range) or "no PR in this range"
        print(f"  plan {plan.slug} landed {', '.join(plan.dates)}: merged in this range {listed}")
        if plan.prs_earlier:
            print(f"    merged before this range: {', '.join(f'#{pr}' for pr in plan.prs_earlier)}")
        if not plan.prs_in_range:
            print("    every PR merged before this range: the work shipped earlier, and this note is not this release's")
        print(f"    plan file: git show {plan.record[:12]}:{PLANS_QUEUE_DIR}/{plan.slug}.md")
        print(f"    release note: {plan.status}")
        if plan.status == "entry":
            for row in (plan.note or "").splitlines():
                print(f"      {row}".rstrip())
        if plan.status.startswith("malformed"):
            malformed += 1
        if plan.decisions:
            print("    has Decisions: read them before acting on the note; one may change the note or the work")
        if plan.earlier_round:
            print(f"    an earlier round landed before {tag}: part of this work may already have shipped")
    for problem in problems:
        print(f"  MALFORMED {problem}")
    return 1 if problems or malformed else 0


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


# The reference changelog above predates the fix split (its versions sit below `LAST_PRE_SPLIT_VERSION`), so it
# exercises the rules every layout shares and the retained Fixed category. This one is the layout between the fix split
# and the fold, with Misc fixes at the end.
SPLIT_CHANGELOG = f"""# Changelog

Notable user-facing changes in each stable release.

## v0.31.0 - 2027-02-01

### \U0001f527 Fixes worth highlighting

- A fix most users would notice. (#21)

### \U0001f5d1️ Removed

- A setting nobody used. (#22)

### \U0001fa79 Misc fixes

{MISC_FIXES_PREAMBLE}

- A rare fix. (#23)
- Another rare fix. (#24, #25)

## v0.30.0 - 2027-01-01

### \U0001f680 Added

- Everything. (#1)
"""


# The current layout: highlights, then the folded full list of every change, highlighted ones included in the full
# list's own words, below a release in the Misc fixes layout that it follows.
FOLD_CHANGELOG = f"""# Changelog

Notable user-facing changes in each stable release.

## v0.44.0 - 2027-03-01

### \U0001f680 Added

- A feature the maintainer highlighted. (#31)

### \U0001f527 Fixes worth highlighting

- A fix most users would notice. (#32)

{FOLD_OPEN}
{FOLD_SUMMARY}

#### \U0001f4a5 Breaking

- A flag only scripts used is gone; drop it from yours. (#33)

#### \U0001f680 Added

- A feature, as the full list words it. (#31)

#### \U0001f504 Changed

- A small change. (#34)

#### \U0001f527 Fixed

- A fix most users would notice. (#32)
- A rare fix. (#35)
- Another rare fix. (#36, #37)

{FOLD_CLOSE}

## v0.41.0 - 2027-02-01

### \U0001fa79 Misc fixes

{MISC_FIXES_PREAMBLE}

- An older rare fix. (#23)
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
    """Exercise every mode on fixtures: the format rules, synthetic histories, and synthetic dist manifests.

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
        "a subsection within a category": GOOD_CHANGELOG.replace("- A fix. (#13, #14)", "#### A fix\n\nProse. (#13)"),
        "an empty category": GOOD_CHANGELOG.replace("- Everything. (#1)\n", ""),
        "a wrong top heading": GOOD_CHANGELOG.replace("# Changelog", "# Release notes"),
    }
    for what, text in variants.items():
        expect(check_changelog_format(text) != [], f"the format check rejects {what}")

    old_highlights = GOOD_CHANGELOG.replace("## v0.2.0", "## v0.13.0").replace("## v0.1.0", "## v0.12.0").replace(
        "### \U0001f4a5 Breaking", "### ✨ Highlights\n\n#### A feature\n\nProse. (#12)\n\n### \U0001f4a5 Breaking"
    )
    expect(check_changelog_format(old_highlights) == [], "the format check preserves historical Highlights sections")
    future_highlights = old_highlights.replace("## v0.13.0", "## v0.14.0", 1)
    expect(check_changelog_format(future_highlights) != [], "the format check rejects Highlights in new sections")
    reference_only = old_highlights.replace("Prose. (#12)", "(#12)")
    reference_only_problems = check_changelog_format(reference_only)
    expect(
        "Highlights subsections need prose" in " ".join(reference_only_problems),
        "the format check rejects a historical subsection without prose",
    )
    list_item = old_highlights.replace("Prose. (#12)", "- A list item. (#12)")
    expect(check_changelog_format(list_item) != [], "the format check rejects a list item in a historical subsection")
    nested_heading = old_highlights.replace("Prose. (#12)", "##### A nested heading\n\nProse. (#12)")
    expect(check_changelog_format(nested_heading) != [], "the format check rejects a nested heading in a historical subsection")
    empty_title = old_highlights.replace("#### A feature", "#### ")
    expect(check_changelog_format(empty_title) != [], "the format check rejects an untitled historical subsection")
    ordered_list = old_highlights.replace("Prose. (#12)", "1. A list item. (#12)")
    expect(check_changelog_format(ordered_list) != [], "the format check rejects an ordered list in a historical subsection")
    blockquote = old_highlights.replace("Prose. (#12)", "> A quote. (#12)")
    expect(check_changelog_format(blockquote) != [], "the format check rejects a blockquote in a historical subsection")
    fenced_code = old_highlights.replace("Prose. (#12)", "```\ncode (#12)\n```")
    expect(check_changelog_format(fenced_code) != [], "the format check rejects fenced code in a historical subsection")
    no_highlight_reference = old_highlights.replace("Prose. (#12)", "Prose.")
    expect(check_changelog_format(no_highlight_reference) != [], "the format check rejects a historical subsection without a PR reference")

    expect(check_changelog_format(SPLIT_CHANGELOG) == [], "the split-fixes changelog passes the format check")
    misc_heading = "### \U0001fa79 Misc fixes"
    split_variants = {
        "the retired Fixed category in a new section": SPLIT_CHANGELOG.replace(
            "### \U0001f527 Fixes worth highlighting", "### \U0001f527 Fixed"
        ),
        "Misc fixes without its preamble": SPLIT_CHANGELOG.replace(f"{MISC_FIXES_PREAMBLE}\n\n", ""),
        "a reworded preamble": SPLIT_CHANGELOG.replace(MISC_FIXES_PREAMBLE, MISC_FIXES_PREAMBLE.replace("complete", "full")),
        "a bullet before the preamble": SPLIT_CHANGELOG.replace(
            f"{misc_heading}\n\n", f"{misc_heading}\n\n- Early. (#20)\n\n"
        ),
        "a preamble with no bullets": SPLIT_CHANGELOG.replace("- A rare fix. (#23)\n- Another rare fix. (#24, #25)\n", ""),
        "a Misc fixes bullet without a PR reference": SPLIT_CHANGELOG.replace("- A rare fix. (#23)", "- A rare fix."),
        "Misc fixes above Removed": SPLIT_CHANGELOG.replace(
            "### \U0001f5d1️ Removed\n\n- A setting nobody used. (#22)\n\n", ""
        ).replace("- Another rare fix. (#24, #25)\n", "- Another rare fix. (#24, #25)\n\n### \U0001f5d1️ Removed\n\n- Late. (#22)\n"),
        "the split categories in a pre-split section": GOOD_CHANGELOG.replace(
            "### \U0001f527 Fixed", "### \U0001f527 Fixes worth highlighting"
        ),
    }
    for what, text in split_variants.items():
        expect(check_changelog_format(text) != [], f"the format check rejects {what}")

    # The editorial policy note is historical. Product entries and future versions keep their usual PR citation rules.
    policy = [f"### {RELEASE_NOTES_POLICY[0]}", "", "- Explain the editorial policy."]
    expect(_check_release_body("0.40.0", policy) == [], "v0.40.0 accepts its policy section without a PR reference")
    expect(_check_release_body("0.40.1", policy) != [], "later versions reject the historical policy section")
    migration_overview = ["", "### 🔄 Changed", "", "- Overview of the migration."]
    expect(
        _check_release_body("0.40.0", policy + migration_overview) == [],
        "v0.40.0 opens with the policy and can omit PR references on the migration overview",
    )
    expect(
        _check_release_body("0.40.0", migration_overview + policy) != [],
        "v0.40.0 requires the policy section before the product changes",
    )
    expect(
        any("must end with its PR reference" in problem
            for problem in _check_release_body("0.40.0", policy + migration_overview + ["- A product change."])),
        "v0.40.0 still requires PR references for product entries after the migration overview",
    )

    # The draft notice lets a stable tag go out before its wording is approved, so the format has to admit it, but only
    # in the one place the procedure puts it: the newest section's first line, verbatim.
    newest_heading = "## v0.31.0 - 2027-02-01\n\n"
    draft = SPLIT_CHANGELOG.replace(newest_heading, f"{newest_heading}{DRAFT_NOTICE}\n\n")
    expect(check_changelog_format(draft) == [], "the format check accepts the draft notice on the newest section")
    _preamble, draft_releases = parse_changelog(draft)
    expect(is_draft(draft_releases[0]) and not is_draft(draft_releases[1]), "is_draft tells a draft section from a finished one")
    older_heading = "## v0.30.0 - 2027-01-01\n\n"
    draft_variants = {
        "the draft notice on an older section": SPLIT_CHANGELOG.replace(older_heading, f"{older_heading}{DRAFT_NOTICE}\n\n"),
        "a reworded draft notice": SPLIT_CHANGELOG.replace(newest_heading, f"{newest_heading}**Draft notes**\n\n"),
        "text between the draft notice and the first category": draft.replace(
            f"{DRAFT_NOTICE}\n\n", f"{DRAFT_NOTICE}\n\nMore to come.\n\n"
        ),
        "the draft notice inside a category": SPLIT_CHANGELOG.replace(
            "- A fix most users would notice. (#21)", f"{DRAFT_NOTICE}\n\n- A fix most users would notice. (#21)"
        ),
    }
    for what, text in draft_variants.items():
        expect(check_changelog_format(text) != [], f"the format check rejects {what}")

    # The fold's frame is raw HTML that GitHub and the website's MDX only render as a closed block of Markdown when its
    # lines and blank lines are exactly so, and nothing else checks them before a release body is published.
    expect(check_changelog_format(FOLD_CHANGELOG) == [], "the folded full list passes the format check")
    fold_only = FOLD_CHANGELOG.replace(
        "### \U0001f680 Added\n\n- A feature the maintainer highlighted. (#31)\n\n"
        "### \U0001f527 Fixes worth highlighting\n\n- A fix most users would notice. (#32)\n\n",
        "",
    )
    expect(check_changelog_format(fold_only) == [], "a release with nothing highlighted, only the fold, passes")
    expect(
        check_changelog_format(FOLD_CHANGELOG.replace(f"\n{FOLD_OPEN}\n{FOLD_SUMMARY}", f"\n{FOLD_OPEN}\n{FOLD_SUMMARY}\n", 1)) == [],
        "extra blank lines inside the fold pass",
    )
    in_code = FOLD_CHANGELOG.replace("- A rare fix. (#35)", "- Exported pages now end with `</details>` correctly. (#35)")
    expect(check_changelog_format(in_code) == [], "a frame tag inside a code span passes")
    with_draft = FOLD_CHANGELOG.replace("## v0.44.0 - 2027-03-01\n\n", f"## v0.44.0 - 2027-03-01\n\n{DRAFT_NOTICE}\n\n")
    expect(check_changelog_format(with_draft) == [], "the draft notice works with the fold")
    fold_text = FOLD_CHANGELOG[FOLD_CHANGELOG.index(FOLD_OPEN) : FOLD_CHANGELOG.index(FOLD_CLOSE) + len(FOLD_CLOSE)]
    fold_variants = {
        "Misc fixes in a section after it was retired": FOLD_CHANGELOG.replace(
            f"{fold_text}\n", f"### \U0001fa79 Misc fixes\n\n{MISC_FIXES_PREAMBLE}\n\n- A rare fix. (#35)\n"
        ),
        "a fold in a section older than the fold": SPLIT_CHANGELOG.replace(
            "- Another rare fix. (#24, #25)\n", f"- Another rare fix. (#24, #25)\n\n{fold_text}\n"
        ),
        "a reworded summary": FOLD_CHANGELOG.replace(FOLD_SUMMARY, "<summary>Everything else</summary>"),
        "a missing summary": FOLD_CHANGELOG.replace(f"{FOLD_SUMMARY}\n", ""),
        "no blank line after the summary": FOLD_CHANGELOG.replace(f"{FOLD_SUMMARY}\n\n", f"{FOLD_SUMMARY}\n"),
        "no blank line before the close": FOLD_CHANGELOG.replace(f"(#36, #37)\n\n{FOLD_CLOSE}", f"(#36, #37)\n{FOLD_CLOSE}"),
        "no blank line before the fold": FOLD_CHANGELOG.replace(f"(#32)\n\n{FOLD_OPEN}", f"(#32)\n{FOLD_OPEN}"),
        "a fold that never closes": FOLD_CHANGELOG.replace(f"{FOLD_CLOSE}\n", ""),
        "text after the fold": FOLD_CHANGELOG.replace(f"{FOLD_CLOSE}\n", f"{FOLD_CLOSE}\n\n- Late. (#38)\n"),
        "a highlight category below the fold": FOLD_CHANGELOG.replace(
            f"{FOLD_CLOSE}\n", f"{FOLD_CLOSE}\n\n### \U0001f680 Added\n\n- Late. (#38)\n"
        ),
        "a second fold": FOLD_CHANGELOG.replace(f"{FOLD_CLOSE}\n", f"{FOLD_CLOSE}\n\n{fold_text}\n"),
        "fold categories out of order": FOLD_CHANGELOG.replace(
            "#### \U0001f4a5 Breaking\n\n- A flag only scripts used is gone; drop it from yours. (#33)\n\n", ""
        ).replace("#### \U0001f527 Fixed", "#### \U0001f4a5 Breaking\n\n- Late. (#33)\n\n#### \U0001f527 Fixed"),
        "a highlight category name inside the fold": FOLD_CHANGELOG.replace(
            "#### \U0001f527 Fixed", "#### \U0001f527 Fixes worth highlighting"
        ),
        "a `###` heading inside the fold": FOLD_CHANGELOG.replace("#### \U0001f504 Changed", "### \U0001f504 Changed"),
        "text before the fold's first category": FOLD_CHANGELOG.replace(
            f"{FOLD_SUMMARY}\n\n", f"{FOLD_SUMMARY}\n\nEverything else.\n\n"
        ),
        "an empty fold": FOLD_CHANGELOG.replace(
            fold_text, f"{FOLD_OPEN}\n{FOLD_SUMMARY}\n\n{FOLD_CLOSE}"
        ),
        "a fold bullet without a PR reference": FOLD_CHANGELOG.replace("- A rare fix. (#35)", "- A rare fix."),
        "an empty release": fold_only.replace(f"{fold_text}\n", ""),
        "a closing details tag in an entry's text": FOLD_CHANGELOG.replace(
            "- A rare fix. (#35)", "- Exported pages now end with </details> correctly. (#35)"
        ),
        "a summary tag in a highlight's text": FOLD_CHANGELOG.replace(
            "- A feature the maintainer highlighted. (#31)", "- Folded <summary> lines now render. (#31)"
        ),
        "no blank line between the fold and the next release": FOLD_CHANGELOG.replace(
            f"{FOLD_CLOSE}\n\n## v0.41.0", f"{FOLD_CLOSE}\n## v0.41.0"
        ),
    }
    for what, text in fold_variants.items():
        expect(check_changelog_format(text) != [], f"the format check rejects {what}")

    # The fold lists every change so the maintainer can remove or merge highlights without losing one. A highlight's PR
    # missing from the fold, or a fold left out under highlights, is the one way to break that, and only new sections are
    # held to it: v0.42.0 and v0.43.0 shipped a fold of only the changes not highlighted, under the older title.
    merged = FOLD_CHANGELOG.replace(
        "### \U0001f680 Added\n\n- A feature the maintainer highlighted. (#31)\n\n"
        "### \U0001f527 Fixes worth highlighting\n\n- A fix most users would notice. (#32)\n\n",
        "### \U0001f680 Added\n\n- Both changes, summed up as one highlight. (#31, #32)\n\n",
    )
    expect(check_changelog_format(merged) == [], "a merged highlight whose PRs the fold cites passes")
    wrapped = FOLD_CHANGELOG.replace("- Another rare fix. (#36, #37)", "- Another rare fix. (#36,\n  #37)").replace(
        "- A feature the maintainer highlighted. (#31)", "- A feature the maintainer highlighted. (#31, #37)"
    )
    expect(check_changelog_format(wrapped) == [], "a fold entry's wrapped reference counts toward the highlights' PRs")
    spaced = FOLD_CHANGELOG.replace("- A feature the maintainer highlighted. (#31)",
                                   "- A feature the maintainer highlighted. (#31)\n  ")
    expect(check_changelog_format(spaced) == [], "an indented blank line leaves the highlight's reference intact")
    complete_fold_variants = {
        "a highlight citing a PR the fold leaves out": FOLD_CHANGELOG.replace(
            "- A feature the maintainer highlighted. (#31)", "- A feature the maintainer highlighted. (#31, #39)"
        ),
        "highlights with no fold": FOLD_CHANGELOG.replace(f"{fold_text}\n\n", ""),
        "the older summary in a new section": FOLD_CHANGELOG.replace(FOLD_SUMMARY, LEGACY_FOLD_SUMMARY),
        "a highlight whose wrapped reference cites a PR the fold leaves out": FOLD_CHANGELOG.replace(
            "- A feature the maintainer highlighted. (#31)", "- A feature the maintainer highlighted. (#31,\n  #39)"
        ),
        "a highlight with a missing folded PR followed by an indented blank line": spaced.replace(
            "highlighted. (#31)", "highlighted. (#31, #39)"
        ),
    }
    for what, text in complete_fold_variants.items():
        expect(check_changelog_format(text) != [], f"the format check rejects {what}")
    legacy_fold = (
        FOLD_CHANGELOG.replace("## v0.44.0", "## v0.42.0")
        .replace(FOLD_SUMMARY, LEGACY_FOLD_SUMMARY)
        .replace("#### \U0001f680 Added\n\n- A feature, as the full list words it. (#31)\n\n", "")
        .replace("- A fix most users would notice. (#32)\n- A rare fix.", "- A rare fix.")
    )
    expect(check_changelog_format(legacy_fold) == [], "v0.42.0's fold of only the changes not highlighted passes")
    legacy_fold_text = legacy_fold[legacy_fold.index(FOLD_OPEN) : legacy_fold.index(FOLD_CLOSE) + len(FOLD_CLOSE)]
    legacy_no_fold = legacy_fold.replace(f"{legacy_fold_text}\n\n", "")
    expect(FOLD_OPEN not in legacy_no_fold.split("## v0.41.0")[0], "the v0.42.0 fixture without a fold really has none")
    expect(check_changelog_format(legacy_no_fold) == [], "v0.42.0 needs no fold under its highlights")
    expect(
        check_changelog_format(legacy_fold.replace(LEGACY_FOLD_SUMMARY, FOLD_SUMMARY)) != [],
        "the format check rejects the new summary on a section that shipped with the older one",
    )

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
        _commit(repo, "fix: amends unreleased work (#10)", {f"{fragments}/with-fragment.md": "---\nkind: added\n---\n\nB, amended.\n"})
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
        expect(by_pr[10].covered and by_pr[10].modified == [f"{fragments}/with-fragment.md"], "a follow-up that edits an existing fragment is covered")
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

    # Coverage must use discovery's file boundary, including destinations of
    # moves. Notes in nested folders or another format never reach curation.
    with tempfile.TemporaryDirectory() as work:
        repo = Path(work)
        _run_git(repo, "init", "-q", "-b", "main")
        fragments = str(FRAGMENTS_DIR)
        note = "---\nkind: fixed\n---\n\nA correction.\n"
        _commit(repo, "chore: baseline", {"baseline": "b"})
        _run_git(repo, "tag", "v0.1.0")
        for pr, name in enumerate(("ignored.txt", "extensionless", "nested/ignored.md", FRAGMENT_README), 20):
            _commit(repo, f"fix: invalid note (#{pr})", {f"{fragments}/{name}": note})
        _commit(repo, "fix: invalid edit (#24)", {f"{fragments}/ignored.txt": note + "Context.\n"})
        _commit(repo, "fix: move into discovery (#25)", {
            f"{fragments}/extensionless": None, f"{fragments}/recognized.md": note,
        })
        _commit(repo, "fix: move out of discovery (#26)", {
            f"{fragments}/recognized.md": None, f"{fragments}/nested/recognized.md": note,
        })
        _tag, commits, _stale = sweep(repo)
        by_pr = {commit.pr: commit for commit in commits}
        expect(all(not by_pr[pr].covered for pr in range(20, 25)), "non-fragments cannot satisfy added or modified coverage")
        expect(by_pr[25].covered, "a move into a top-level Markdown fragment supplies coverage")
        expect(not by_pr[26].covered, "a move to nested Markdown cannot supply coverage")

    # Plans' release notes come from history, not the working tree: the fixture approves (deletes) a plan after it
    # lands, as the review flow does, and the note must still be found. Every PR is a real squash commit, so the fixture
    # also covers PRs merged on the other side of the stable boundary from their landing record: one plan's PRs merged
    # before the tag and its record after, and one plan that landed a round before the tag and another after it.
    with tempfile.TemporaryDirectory() as work:
        repo = Path(work)
        _run_git(repo, "init", "-q", "-b", "main")
        queue, reports = str(PLANS_QUEUE_DIR), str(PLANS_REPORTS)
        landed_lines: dict[str, str] = {}

        def plan_file(*notes: str, extra: str = "") -> str:
            sections = "".join(f"## {RELEASE_NOTE_HEADING}\n\n{note}\n\n" for note in notes)
            return f"# A plan\n\n## The goal\n\n```markdown\n## {RELEASE_NOTE_HEADING}\n\nQuoted, not the note.\n```\n\n{sections}## Done criterion\n\nDone.\n{extra}"

        def merge(*prs: int) -> None:
            for pr in prs:
                _commit(repo, f"feat: plan work (#{pr})", {f"work-{pr}.txt": str(pr)})

        def land(slug: str, prs: list[int], text: str, link: str | None = None) -> None:
            listed = ", ".join(f"#{pr}" for pr in prs)
            landed_lines[slug] = f"- [`{slug}`](reports/{link or slug}.report.md) landed 2026-03-01 in {listed}: {slug}"
            _commit(repo, f"chore(plans): record the landing of {slug}", {
                f"{queue}/{slug}.md": text,
                reports: "# Plan reports to review\n\n" + "\n".join(landed_lines.values()) + "\n",
            })

        def approve(slug: str) -> None:
            del landed_lines[slug]
            _commit(repo, f"chore(plans): approve {slug}", {
                f"{queue}/{slug}.md": None,
                reports: "# Plan reports to review\n\n" + "\n".join(landed_lines.values()) + "\n",
            })

        _commit(repo, "chore: baseline", {"baseline": "b"})
        merge(1)
        land("shipped-before", [1], plan_file("The first round's note."))
        merge(2)
        _run_git(repo, "tag", "v0.1.0")
        land("recorded-late", [2], plan_file("Shipped before its record."))
        merge(10, 11)
        land("with-entry", [10, 11], plan_file("Sessions can be renamed.\n\nContext for the curator."))
        approve("with-entry")
        merge(12)
        land("none-needed", [12], plan_file(RELEASE_NOTE_NONE))
        merge(13)
        land("deferred", [13], plan_file(RELEASE_NOTE_DEFERRED, extra="\n## Blocked\n\nA question.\n"))
        merge(14)
        land("old-plan", [14], plan_file())
        merge(15)
        land("followed-up", [15], plan_file("The note as planned."))
        merge(16)
        land("followed-up", [15, 16], plan_file("The note as planned.", extra="\n## Decisions\n\nA follow-up.\n"))
        merge(17)
        land("shipped-before", [17], plan_file("The second round's note."))
        _commit(repo, "chore(plans): approve something that never landed", {"other": "o"})
        tag, plans, plan_problems = plan_notes(repo)
        by_slug = {plan.slug: plan for plan in plans}
        expect(tag == "v0.1.0" and plan_problems == [], "the plan notes read a well-formed history without problems")
        expect(list(by_slug) == ["recorded-late", "with-entry", "none-needed", "deferred", "old-plan", "followed-up",
                                 "shipped-before"],
               "every plan whose landing was recorded since the merge base is listed once, in landing order")
        expect(by_slug["with-entry"].note == "Sessions can be renamed.\n\nContext for the curator."
               and by_slug["with-entry"].prs_in_range == [10, 11],
               "an approved plan's note and PRs are read from its landing commit, past a quoted heading in a fence")
        expect([by_slug[s].status for s in ("none-needed", "deferred", "old-plan")]
               == ["none needed", "ask during the release", "not recorded"],
               "the two fixed answers, one followed by a Blocked section, and a plan without the section are told apart")
        expect(by_slug["followed-up"].prs_in_range == [15, 16] and by_slug["followed-up"].decisions
               and by_slug["followed-up"].note == "The note as planned.",
               "a plan landed twice collects both rounds' PRs once and reads its latest plan file up to its Decisions")
        expect(by_slug["recorded-late"].prs_in_range == [] and by_slug["recorded-late"].prs_earlier == [2],
               "a plan whose PRs merged before the tag but was recorded after it shows every PR as merged earlier")
        expect(by_slug["shipped-before"].earlier_round and by_slug["shipped-before"].prs_in_range == [17]
               and not by_slug["with-entry"].earlier_round,
               "a plan with a round before the stable tag is flagged, and only this cycle's PRs are listed")

        merge(18)
        land("two-notes", [18], plan_file(RELEASE_NOTE_NONE, "A newer entry appended below."))
        merge(19)
        land("empty-note", [19], plan_file(""))
        _tag, plans, _problems = plan_notes(repo)
        statuses = {plan.slug: plan.status for plan in plans}
        expect(statuses["two-notes"].startswith("malformed") and statuses["empty-note"].startswith("malformed"),
               "a plan with two Release note sections, or an empty one, is malformed rather than read as its first")
        expect(report_plan_notes(_tag, plans, _problems) == 1, "a malformed note fails the plans mode")

        merge(20)
        land("bad-link", [20], plan_file("A note."), link="someone-else")
        _commit(repo, "chore(plans): record the landing of no-report-line", {"stray": "s"})
        _tag, _plans, plan_problems = plan_notes(repo)
        expect(len(plan_problems) == 2 and "bad-link" in plan_problems[0] and "no-report-line" in plan_problems[1],
               "a landing record linking another plan's report, or adding no REPORTS.md line, is reported")
        try:
            plan_notes(repo, "v9.9.9")
            expect(False, "an unknown --since tag is reported as a problem")
        except Problem:
            expect(True, "an unknown --since tag is reported as a problem")

    expect(check_plan_note(plan_file("An entry.")) == [], "a plan with one non-empty Release note passes plan-note")
    expect(check_plan_note(plan_file()) != [] and check_plan_note(plan_file("")) != []
           and check_plan_note(plan_file("One.", "Two.")) != [],
           "plan-note rejects a missing, empty or repeated Release note section")

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
    plans = sub.add_parser("plans", help="list plans landed since the last stable release, with their release notes")
    plans.add_argument("--since", help="stable tag to look from (default: the highest vX.Y.Z tag)")
    plan_note = sub.add_parser("plan-note", help="check that plan files carry exactly one non-empty Release note")
    plan_note.add_argument("files", nargs="+", help="plan files to check")
    announce =sub.add_parser("announce", help="compare dist's announcement for a tag with CHANGELOG.md")
    announce.add_argument("--tag", required=True)
    announce.add_argument("--manifest", help="a dist manifest JSON file")
    announce.add_argument("--manifest-env", help="environment variable holding the manifest JSON")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not args.mode:
        parser.error("choose a mode: format, fragments, plans, plan-note, announce, or --self-test")
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
        if args.mode == "plan-note":
            bad = 0
            for name in args.files:
                for problem in check_plan_note(Path(name).read_text(encoding="utf-8")):
                    bad += 1
                    print(f"plan-note: {name}: {problem}", file=sys.stderr)
            if bad:
                return 1
            print(f"ok: {len(args.files)} plan file(s) carry one release note each")
            return 0
        if args.mode == "plans":
            tag, plans, plan_problems = plan_notes(repo, args.since)
            return report_plan_notes(tag, plans, plan_problems)
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
