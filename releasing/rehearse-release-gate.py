#!/usr/bin/env python3
"""Derive a throwaway workflow that runs the release gate in GitHub without cutting a release.

NOTE: The output is never committed to main. It goes on a throwaway branch under `rehearsal/release-gate/`, gets pushed
so GitHub runs it, and the branch is deleted afterwards. `.agents/run-all-tests.md` is the procedure that uses it.

The release's test gate (`.github/dist-build-setup.yml`, spliced by cargo-dist into every per-target build job of the
generated `.github/workflows/release.yml`) only ever runs on a tag push, and a tag push publishes a release. Several
failure classes only show up on GitHub's runners: the macOS and arm64 legs, the runner images' preinstalled tools, and
whatever Rust stable happens to be current there. This script is how to get that verdict for an arbitrary commit
without publishing anything.

Here's the TLDR of the rewrite, which emulates what dist's own `pr-run-mode = "upload"` would do:

- The trigger becomes a push to any branch under `rehearsal/release-gate/`, instead of a tag push.
- The plan job runs `dist plan` instead of `dist host --steps=create`, and reports that it is not publishing.
- The per-target build jobs (and with them the whole test gate) run whenever the plan has a build matrix. They still
  build the release artifacts and upload them as workflow artifacts, which is the release's packaging path exercised
  for free.
- Everything after the build jobs (the global artifacts, creating the GitHub release, signing, announcing) is dropped,
  which is what guarantees nothing is published. Workflow permissions drop to `contents: read` as a second fence.
- The two gate steps that need a real tag are replaced by the one half of them that does not. The tag-names-the-version
  assertion is dropped outright, and the changelog step keeps its `format` lint but not its `announce` comparison
  (which checks dist's announcement for a specific tag). A rehearsal therefore says nothing about either; a real tag
  build still checks both.

Every rewrite is an exact pattern match against the current `release.yml`, and a pattern that does not match exactly
once is a hard error rather than a silently skipped step. When dist regenerates `release.yml` into a shape this script
does not recognize, it refuses, and the patterns need updating against the new file. `--self-test` checks that the
current `release.yml` still converts and that the result keeps the properties above.

Usage, from the checkout root:

    python3 releasing/rehearse-release-gate.py            # writes .github/workflows/release-gate-rehearsal.yml
    python3 releasing/rehearse-release-gate.py --output PATH
    python3 releasing/rehearse-release-gate.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
DEFAULT_OUTPUT = ROOT / ".github" / "workflows" / "release-gate-rehearsal.yml"

# The branch glob the rehearsal triggers on. A per-run branch under it (rather than one fixed branch) lets two
# rehearsals from different checkouts coexist without one force-pushing over the other's run.
BRANCH_GLOB = "rehearsal/release-gate/**"

HEADER = """\
# GENERATED THROWAWAY: a release-gate rehearsal derived from release.yml by
# releasing/rehearse-release-gate.py. Never merge this file. It runs the
# per-target release build jobs, including the test gate spliced in from
# .github/dist-build-setup.yml, on a push to a rehearsal branch, and publishes
# nothing. See .agents/run-all-tests.md.
"""


class RewriteError(Exception):
    """`release.yml` no longer has the shape one of the rewrites expects."""


def rewrite(text: str) -> str:
    """Return the rehearsal workflow for one `release.yml` text, or raise RewriteError."""

    def sub(pattern: str, replacement: str) -> None:
        nonlocal text
        new, count = re.subn(pattern, replacement, text, flags=re.M | re.S)
        if count != 1:
            raise RewriteError(f"expected exactly one match, found {count}: {pattern}")
        text = new

    sub(r"^name: Release$", "name: Release gate rehearsal")
    sub(r'^permissions:\n  "contents": "write"\n', 'permissions:\n  "contents": "read"\n')
    sub(
        r"^on:\n  push:\n    tags:\n      - '\*\*\[0-9\]\+\.\[0-9\]\+\.\[0-9\]\+\*'\n",
        f"on:\n  push:\n    branches:\n      - '{BRANCH_GLOB}'\n",
    )
    # With no tag there is nothing to pass to `dist build --tag`, and an empty tag flag is what dist's own PR mode
    # uses: it builds whatever version the workspace declares.
    sub(
        r"^      tag: \$\{\{.*?\n      tag-flag: \$\{\{.*?\n      publishing: \$\{\{.*?\n",
        "      tag: ''\n      tag-flag: ''\n      publishing: 'false'\n",
    )
    sub(
        r"^          dist \$\{\{ \(!github.event.pull_request.*?--output-format=json > plan-dist-manifest.json\n",
        "          dist plan --output-format=json > plan-dist-manifest.json\n",
    )
    sub(
        r"^    if: \$\{\{ fromJson\(needs.plan.outputs.val\).ci.github.artifacts_matrix.include != null && .*?\}\}\n",
        "    if: ${{ fromJson(needs.plan.outputs.val).ci.github.artifacts_matrix.include != null }}\n",
    )
    sub(
        r'^      - name: "Assert the tag names this workspace\'s exact version"\n.*?(?=^      - name: "Assert the changelog)',
        "",
    )
    sub(
        r'^      - name: "Assert the changelog dist is about to publish"\n.*?(?=^      - name: )',
        '      - name: "Lint the changelog layout (the tag-independent half of the release assertion)"\n'
        "        run: python3 releasing/check-changelog.py format\n",
    )
    sub(r"^  build-global-artifacts:\n.*\Z", "")
    return HEADER + text.rstrip() + "\n"


def check_properties(workflow: str) -> list[str]:
    """Problems with a generated rehearsal, as text checks (no YAML parser is guaranteed to be installed)."""
    problems = []
    # Two-space keys are jobs only below `jobs:`; above it the same indentation holds `on:`'s event names.
    jobs_block = workflow.partition("\njobs:\n")[2]
    jobs = re.findall(r"^  ([a-z][a-z0-9-]*):\n", jobs_block, flags=re.M)
    if jobs != ["plan", "build-local-artifacts"]:
        problems.append(f"expected jobs [plan, build-local-artifacts], found {jobs}")
    if f"      - '{BRANCH_GLOB}'\n" not in workflow or "tags:" in workflow:
        problems.append("trigger is not a push to the rehearsal branch glob")
    if "dist host" in workflow:
        problems.append("still runs `dist host`, which creates a release")
    if "publishing: 'false'" not in workflow:
        problems.append("plan job does not report publishing: false")
    if '"contents": "read"' not in workflow:
        problems.append("workflow permissions are not contents: read")
    if "GITHUB_REF_NAME" in workflow or "github.ref_name" in workflow:
        problems.append("a step still depends on the tag name")
    if "check-changelog.py format" not in workflow:
        problems.append("the changelog format lint is missing")
    if "Run the test suite against the pinned tmux" not in workflow:
        problems.append("the release test gate's steps are missing from the build job")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT, help="where to write the workflow")
    group.add_argument("--self-test", action="store_true", help="convert the current release.yml and check the result")
    args = parser.parse_args()

    try:
        workflow = rewrite(RELEASE_WORKFLOW.read_text(encoding="utf-8"))
    except RewriteError as error:
        print(f"rehearse-release-gate: release.yml changed shape: {error}", file=sys.stderr)
        return 1
    problems = check_properties(workflow)
    if problems:
        for problem in problems:
            print(f"rehearse-release-gate: {problem}", file=sys.stderr)
        return 1

    if args.self_test:
        # Round-trip through a file too, so the write path is exercised and not just the string rewrite.
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "rehearsal.yml"
            path.write_text(workflow, encoding="utf-8")
            if check_properties(path.read_text(encoding="utf-8")):
                print("rehearse-release-gate: written workflow lost its properties", file=sys.stderr)
                return 1
        print("rehearse-release-gate: self-test passed")
        return 0

    args.output.write_text(workflow, encoding="utf-8")
    print(args.output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
