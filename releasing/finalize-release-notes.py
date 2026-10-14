#!/usr/bin/env python3
"""Put a stable release's approved notes on its GitHub release, in place of the draft the tag was built with.

A stable tag is pushed while the maintainer is still reviewing the release notes, so the tag's `CHANGELOG.md` section
opens with the draft notice and dist copies that draft into the GitHub release body. Once the approved wording has
landed on main, this script swaps it in. It touches only the notes block of the body, between dist's `## Release Notes`
heading and the first top-level `# ` heading of the download tables that follow, and leaves the release's name and
assets alone. `releasing/AGENTS.md`, "Cutting a stable release", step 3, is the procedure around it.

The swap is checked rather than trusted, because the body is public and a bad edit replaces it for everyone:

- The tagged section must carry the draft notice. A release that was never a draft is not this script's business, even
  when its section on main has since been corrected.
- The approved section (read from `--ref`, main by default) must no longer carry the draft notice, must pass the
  changelog format check, and must keep the tagged section's heading, since dist titled the release with that heading
  and this script does not rename it.
- The body's notes block must be exactly the tagged section, which is what dist put there. Anything else means someone
  edited the release by hand or dist changed its layout, and the script refuses rather than guess.
- A block that already holds the approved text is reported as finalized and left alone, so a re-run is harmless.

What this does NOT do: change `CHANGELOG.md` (the approved text lands on main by an ordinary PR first), handle
prereleases (they carry no curated notes of their own), or edit any release other than the one named.

Usage:
  finalize-release-notes.py --tag vX.Y.Z [--repo PATH] [--ref REF] [--dry-run]
  finalize-release-notes.py --self-test
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path

NOTES_HEADING = "## Release Notes"


def _load_checker():
    """Import `check-changelog.py` from beside this file, so both scripts read `CHANGELOG.md` with the same parser.

    The hyphenated file name rules out a plain import. The module is registered in `sys.modules` before it runs
    because its dataclasses look their own module up there while being defined.
    """
    path = Path(__file__).with_name("check-changelog.py")
    spec = importlib.util.spec_from_file_location("check_changelog", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


checker = _load_checker()
Problem = checker.Problem


def section(changelog_text: str, version: str, where: str):
    """The `## vX.Y.Z` section of a changelog text, or `Problem` naming `where` the text came from."""
    _preamble, releases = checker.parse_changelog(changelog_text)
    for release in releases:
        if release.version == version:
            return release
    raise Problem(f"{where} has no `## v{version}` section")


def notes_block(body: str) -> tuple[int, int]:
    """The `[start, end)` character span of the notes in a dist release body, before stripping.

    dist writes `## Release Notes`, a blank line, the section's body, and then a `# <package>` heading per package with
    its download tables. The span runs from just after the notes heading to the first top-level heading, or to the end
    of the body when there is none.
    """
    lines = body.splitlines(keepends=True)
    offset = 0
    start: int | None = None
    for line in lines:
        if start is None and line.rstrip("\n") == NOTES_HEADING:
            start = offset + len(line)
        elif start is not None and line.startswith("# "):
            return start, offset
        offset += len(line)
    if start is None:
        raise Problem(f"the release body has no `{NOTES_HEADING}` heading; dist's layout changed or it was edited by hand")
    return start, len(body)


def finalized_body(body: str, tagged, approved) -> str | None:
    """The release body with the approved notes swapped in, or `None` when the body already holds them.

    `tagged` and `approved` are the version's sections at the tag and on main. Raises `Problem` for every condition
    under which writing the body would be wrong; see the module docstring for the list.
    """
    # Without this, any stable release whose section on main later got a targeted correction would have its published
    # body rewritten too, which this script is not for: it only ever replaces a draft.
    if not checker.is_draft(tagged):
        raise Problem("the tagged section carries no draft notice; this release was never a draft, so there is nothing to finalize")
    if checker.is_draft(approved):
        raise Problem("the section on main still opens with the draft notice; land the approved text first")
    if approved.heading_text != tagged.heading_text:
        raise Problem(
            f"the heading changed from `{tagged.heading_text}` to `{approved.heading_text}`; dist named the release"
            " after the tagged heading, so keep it"
        )
    start, end = notes_block(body)
    current = body[start:end].strip()
    if current == approved.body_text():
        return None
    if current != tagged.body_text():
        raise Problem("the release body's notes are neither the tagged section nor the approved one; it was edited by hand?")
    return f"{body[:start]}\n{approved.body_text()}\n\n{body[end:]}"


def _run(repo: Path, *args: str) -> str:
    return subprocess.run(list(args), cwd=repo, check=True, capture_output=True, text=True).stdout


def finalize(repo: Path, tag: str, ref: str, dry_run: bool) -> int:
    """Read both sections and the live body, swap the notes, and confirm GitHub now serves the approved text.

    The read-back after the edit is the actual proof of success: `gh release edit` exiting zero says the request was
    accepted, not that the stored body is what this script meant to write.
    """
    match = checker.STABLE_TAG_RE.match(tag)
    if not match:
        raise Problem(f"`{tag}` is not a stable `vX.Y.Z` tag; prereleases carry no notes of their own to finalize")
    version = tag[1:]
    approved_text = _run(repo, "git", "show", f"{ref}:{checker.CHANGELOG_NAME}")
    format_problems = checker.check_changelog_format(approved_text)
    if format_problems:
        raise Problem(f"{checker.CHANGELOG_NAME} at {ref} fails the format check:\n  " + "\n  ".join(format_problems))
    tagged = section(_run(repo, "git", "show", f"{tag}:{checker.CHANGELOG_NAME}"), version, f"{checker.CHANGELOG_NAME} at {tag}")
    approved = section(approved_text, version, f"{checker.CHANGELOG_NAME} at {ref}")

    body = json.loads(_run(repo, "gh", "release", "view", tag, "--json", "body"))["body"]
    new_body = finalized_body(body, tagged, approved)
    if new_body is None:
        print(f"ok: the {tag} release already carries the approved notes from {ref}")
        return 0
    if dry_run:
        print(f"dry run: would replace the {tag} release notes with the section at {ref}:\n")
        print(approved.body_text())
        return 0

    with tempfile.NamedTemporaryFile("w", suffix=".md", encoding="utf-8") as notes:
        notes.write(new_body)
        notes.flush()
        _run(repo, "gh", "release", "edit", tag, "--notes-file", notes.name)
    written = json.loads(_run(repo, "gh", "release", "view", tag, "--json", "body"))["body"]
    if finalized_body(written, tagged, approved) is not None:
        raise Problem(f"the {tag} release body does not hold the approved notes after the edit; inspect it by hand")
    print(f"ok: the {tag} release now carries the approved notes from {ref}")
    return 0


# ---------------------------------------------------------------------------
# Self-test
# ---------------------------------------------------------------------------

DRAFT_CHANGELOG = f"""# Changelog

Notable user-facing changes in each stable release.

## v1.1.0 - 2027-02-01

{checker.DRAFT_NOTICE}

### \U0001f680 Added

- A thing. (#21)
"""

APPROVED_CHANGELOG = """# Changelog

Notable user-facing changes in each stable release.

## v1.1.0 - 2027-02-01

### \U0001f680 Added

- A thing, worded the way the maintainer wants it. (#21)
"""

TABLES = "# farhelm 1.1.0\n\n## Download farhelm 1.1.0\n\n| File | Platform |\n"


def self_test() -> int:
    """Check the body swap against bodies shaped like dist's, since a wrong swap rewrites a public release page.

    The cases cover the swap itself, the re-run that must change nothing, and the refusals that do not need git or gh: a
    release that was never a draft, a section on main still marked as one, a renamed heading, a body whose notes match
    neither side, a prerelease tag, and a missing section. The format check of main's file and the read-back after the
    edit are not covered, since both need a real repository and release. Nothing here calls git or gh.
    """
    failures: list[str] = []

    def expect(condition: bool, what: str) -> None:
        print(f"  {'ok' if condition else 'FAILED'}: {what}")
        if not condition:
            failures.append(what)

    def refuses(body: str, tagged, approved) -> bool:
        try:
            finalized_body(body, tagged, approved)
        except Problem:
            return True
        return False

    tagged = section(DRAFT_CHANGELOG, "1.1.0", "draft")
    approved = section(APPROVED_CHANGELOG, "1.1.0", "approved")
    body = f"{NOTES_HEADING}\n\n{tagged.body_text()}\n\n{TABLES}"
    swapped = finalized_body(body, tagged, approved)
    expect(swapped == f"{NOTES_HEADING}\n\n{approved.body_text()}\n\n{TABLES}", "the draft notes are replaced and the tables kept")
    expect(swapped is not None and finalized_body(swapped, tagged, approved) is None, "a finalized body is left alone")
    expect(refuses(body, tagged, tagged), "a section still carrying the draft notice is refused")
    renamed = section(APPROVED_CHANGELOG.replace("2027-02-01", "2027-02-02"), "1.1.0", "renamed")
    expect(refuses(body, tagged, renamed), "a changed heading is refused")
    expect(refuses(body.replace("- A thing. (#21)", "- Edited by hand. (#21)"), tagged, approved), "hand-edited notes are refused")
    expect(refuses(TABLES, tagged, approved), "a body without the notes heading is refused")
    never_draft = section(APPROVED_CHANGELOG.replace("worded the way", "corrected the way"), "1.1.0", "never a draft")
    corrected_body = f"{NOTES_HEADING}\n\n{never_draft.body_text()}\n\n{TABLES}"
    expect(refuses(corrected_body, never_draft, approved), "a release whose tagged section was never a draft is refused")
    try:
        finalize(Path("/nonexistent"), "v1.1.0-rc.1", "origin/main", dry_run=True)
        expect(False, "a prerelease tag is refused before git or gh runs")
    except Problem:
        expect(True, "a prerelease tag is refused before git or gh runs")
    try:
        section(APPROVED_CHANGELOG, "1.2.0", "approved")
        expect(False, "a changelog without the version's section is refused")
    except Problem:
        expect(True, "a changelog without the version's section is refused")
    no_tables = f"{NOTES_HEADING}\n\n{tagged.body_text()}\n"
    expect(finalized_body(no_tables, tagged, approved) == f"{NOTES_HEADING}\n\n{approved.body_text()}\n\n", "a body with no tables after the notes is swapped")

    if failures:
        for line in failures:
            print(f"self-test FAILED: {line}", file=sys.stderr)
        return 1
    print("== self-test ok")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--repo", default=".", help="repository root (default: current directory)")
    parser.add_argument("--tag", help="the stable tag whose GitHub release to finalize, `vX.Y.Z`")
    parser.add_argument("--ref", default="origin/main", help="where the approved section lives (default: origin/main)")
    parser.add_argument("--dry-run", action="store_true", help="check everything and show the new notes; write nothing")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if not args.tag:
        parser.error("--tag is required unless --self-test is given")
    try:
        return finalize(Path(args.repo).resolve(), args.tag, args.ref, args.dry_run)
    except Problem as why:
        print(f"error: {why}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as failed:
        print(f"error: `{' '.join(failed.cmd)}` failed:\n{failed.stderr.strip()}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
