#!/usr/bin/env python3
"""Keep the agent instructions loadable: the root file within Codex's budget, every pointer and citation resolvable.

Codex injects only the first 32 KiB of the root `AGENTS.md` (its `project_doc_max_bytes` default) and drops the rest
without a marker. The root therefore holds the rules every task needs plus a map, and everything else lives in files
agents read on demand: nested `AGENTS.md` files and `.agents/` directories. That shape fails silently in three ways,
and this script checks for each of them:

- The root grows past the budget, so its tail (whatever was added last) vanishes for Codex again. The limit here is
  28 KiB, which leaves room under Codex's 32 KiB for whatever Codex adds around it. The root must also end with the
  `# End of AGENTS.md` sentinel, which is how an agent recognizes a truncated copy.
- A pointer names a file that does not exist, or a citation names a heading that was renamed, so an agent following
  the instructions reaches nothing. Checked: backticked paths that exist, start with a tracked top-level entry, or are
  unmistakably files (a `.md` path with a directory, or an uppercase stem like `SPEC.md` or `SPEC_impl.md`); Markdown
  links, resolved relative to the citing file as a renderer does; `` `file`, "Heading" `` citations in any tracked text file; and the
  older comma-less root citations (root `AGENTS.md` "Heading"), which must name a current root heading or one listed
  in `.agents/old-sections.md`. Other backtick spans (commands, runtime files like `~/.farhelm-ntfy-topic`, templates
  with `<placeholders>`) are not paths.
- A detail file becomes unreachable: nothing that an agent starting from the root would read points at it. The
  old-section map is a lookup table, so its mentions do not count as a route.

It also refuses a root `AGENTS.override.md` (Codex would load it instead of `AGENTS.md`) and any inventoried `CLAUDE.md`
or `CLAUDE.local.md` (Claude Code's default loader skips `AGENTS.md` when either exists in the working directory or a
parent). Ignored files and files outside the repository inventory are not checked.

What it does not check, though the plan for the split proposed some of it: whether a reading list is complete
(closure over what its files cite), how many hops a file is from the root, whether every index entry carries a fence,
whether a detail file that asks for a human answer points at `AGENTS.md`, "Unattended runs", whether a pointer's
trigger is the right one, and whether a condensed rule kept its exceptions. The reading lists are free-form prose in
several shapes, so a mechanical closure check would mostly check the parser; these were audited by hand and by review
when the split was made.

`lore/` is not read at all (`lore/AGENTS.md` says no tooling processes its contents). Broken citations in other
historical records (`TRIAGE_OUTCOMES.md`, plan reports) and in every queued plan are warnings, never failures: those
files must not be rewritten to follow a rename, and a queue command can move a landed plan back to pending without
anyone editing it.

Run from anywhere; `--self-test` runs the fixtures and touches nothing outside a temporary directory.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT_LIMIT = 28 * 1024
SENTINEL = "# End of AGENTS.md"

# The start of a citation in the normalized form: `path`, "Heading". The path must end in .md. Where the heading ends
# is decided by `cited_heading`, because headings may themselves contain quotes (`Picking: "pick a plan to execute"`).
CITATION = re.compile(r'`([A-Za-z0-9_./-]+\.md)`,\s+"')
BACKTICK = re.compile(r"`([^`\s]+)`")
HEADING = re.compile(r"^#{1,6} (.+?)\s*$", re.M)
# A quoted heading name followed by "above" or "below" is a relative reference to a section of the same file; after a
# move it can silently point at nothing, which is the failure this catches.
RELATIVE = re.compile(r'"([^"\n]{3,80})"\s+(above|below)\b')
# Root citations in the older, comma-less forms. Only the root file is meant: a path before `AGENTS.md` (as in
# `plans/AGENTS.md`) makes it some other file, which the lookbehind excludes.
OLD_ROOT_CITATION = re.compile(r'(?<![/\w])`?AGENTS\.md`?(?:\'s)?\s+(?:section\s+)?"([^"\n]{3,80})"')
HISTORICAL_PREFIXES = ("lore/", "plans/reports/")
HISTORICAL_FILES = {"TRIAGE_OUTCOMES.md", "plans/REPORTS.md", "plans/queue/INDEX.md"}


def old_section_names(text: str) -> set[str]:
    """The old root headings listed in the first column of `.agents/old-sections.md`'s table."""
    names: set[str] = set()
    for line in text.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) >= 2 and cells[0] and not set(cells[0]) <= set("- "):
            names.update(n.strip() for n in cells[0].split(","))
    return names


def tracked_files(root: pathlib.Path) -> list[str]:
    """Every tracked or new, non-ignored file, so files a change adds count before they are committed."""
    out = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard"],
        check=True, capture_output=True, text=True,
    ).stdout
    return sorted({line for line in out.splitlines() if line and (root / line).is_file()})


def is_instruction_file(path: str) -> bool:
    """`AGENTS.md` at any depth and anything under an `.agents/` directory."""
    parts = path.split("/")
    return parts[-1] == "AGENTS.md" or (".agents" in parts[:-1] and path.endswith(".md"))


def headings(text: str) -> set[str]:
    return {" ".join(h.split()) for h in HEADING.findall(strip_fences(text))}


def strip_fences(text: str) -> str:
    """Blanks fenced code blocks, so example headings and sample citations inside them are ignored."""
    return re.sub(r"^```.*?^```", lambda m: "\n" * m.group(0).count("\n"), text, flags=re.M | re.S)


def cited_heading(text: str, start: int, known: set[str]) -> str:
    """Returns the heading a citation names, given the offset just past its opening quote.

    Each later quote within reach is a candidate end. The first candidate that is a real heading wins, so a heading
    that contains quotes resolves; when none is, the shortest candidate is returned so the error names what the text
    most plausibly meant. Line breaks inside are folded to spaces because Markdown wraps prose.
    """
    window = text[start:start + 300]
    ends = [i for i, c in enumerate(window) if c == '"']
    candidates = [" ".join(window[:i].split()) for i in ends]
    for c in candidates:
        if c in known:
            return c
    return candidates[0] if candidates else " ".join(window.split("\n")[0].split())


def warn_only(path: str) -> bool:
    """Files whose broken citations are reported as warnings: historical records and every queued plan.

    Plans count whatever their state, because a queue command can move a landed plan back to pending without anyone
    editing it, and a citation that was tolerated a moment ago must not start failing CI on its own.
    """
    if path in HISTORICAL_FILES or path.startswith(HISTORICAL_PREFIXES):
        return True
    return path.startswith("plans/queue/") and path.endswith(".md")


def is_definite_path(ref: str) -> bool:
    """Recognize directory paths and uppercase file stems, including mixed-case underscore suffixes.

    These are checked even when nothing by that name exists, so a typo (`plnas/AGENTS.md`, `SPEC_impll.md`) fails
    rather than being mistaken for a command. Lowercase bare names (`cursor-launch.md`) are often examples, not paths.
    """
    if not ref.endswith(".md"):
        return False
    return "/" in ref or re.fullmatch(r"[A-Z0-9_]+(?:_[A-Za-z0-9_]+)?(\.[A-Za-z0-9_]+)?\.md", ref) is not None


def resolve(root: pathlib.Path, source: str, ref: str, top_level: set[str],
            basenames: frozenset[str] = frozenset()) -> pathlib.Path | None:
    """Resolves a backticked path against the repository root, then the citing file's directory.

    Returns None for spans that are not repository paths at all (commands, runtime files, templates with
    placeholders), so those are never reported as missing. A definite path (`is_definite_path`) that exists at
    neither base resolves to its root-relative location, which then fails the existence check.
    """
    ref = ref.rstrip("/").split("#")[0]
    if not ref or any(c in ref for c in "<>*{}$~ "):
        return None
    # Absolute paths, flags and dotfiles are not repository paths, except the two dot-directories that are.
    if ref.startswith(("/", "-", ".")) and not ref.startswith((".agents/", ".github/")):
        return None
    base_dir = (root / source).parent
    if (root / ref).exists():
        return root / ref
    if (base_dir / ref).exists():
        return base_dir / ref
    if "/" not in ref and (ref in basenames or ref in {"CLAUDE.md", "CLAUDE.local.md"}):
        # A bare name such as `INDEX.md` is written in the context of its own directory tree (`plans/queue/INDEX.md`
        # from `plans/.agents/`); any tracked file of that name satisfies it, and only a name matching no file at all
        # is a typo. The Claude instruction names are mentioned as files that must not exist.
        return None
    if ref.split("/")[0] in top_level or is_definite_path(ref):
        return root / ref
    return None


def resolve_link(root: pathlib.Path, source: str, target: str) -> pathlib.Path | None:
    """Resolves a Markdown link target the way a renderer does: relative to the citing file. Skips URLs and anchors."""
    target = target.split("#")[0]
    if not target or re.match(r"[a-z]+:", target) or target.startswith("/"):
        return None
    return ((root / source).parent / target).resolve()


def check(root: pathlib.Path, files: list[str]) -> tuple[list[str], list[str]]:
    """Returns (errors, warnings) for the repository at `root` with the given file list."""
    errors: list[str] = []
    warnings: list[str] = []
    top_level = {f.split("/")[0] for f in files}
    basenames = frozenset(f.split("/")[-1] for f in files)
    # lore/ is skipped entirely: `lore/AGENTS.md` says no tooling processes its contents (its own AGENTS.md is
    # instructions and is checked).
    texts = {f: (root / f).read_text(errors="replace") for f in files
             if f.endswith((".md", ".py", ".sh", ".rs", ".ts", ".yml", ".toml", ".mjs"))
             and (not f.startswith("lore/") or f == "lore/AGENTS.md") and f != "scripts/check-agents-md.py"}
    instruction = [f for f in files if is_instruction_file(f)]

    # The root budget and sentinel: the reason this script exists.
    root_md = root / "AGENTS.md"
    if not root_md.is_file():
        errors.append("AGENTS.md: missing")
    else:
        size = len(root_md.read_bytes())
        if size > ROOT_LIMIT:
            errors.append(f"AGENTS.md: {size} bytes, over the {ROOT_LIMIT}-byte budget")
        found = HEADING.findall(strip_fences(root_md.read_text()))
        if not found or f"# {found[-1]}" != SENTINEL or not texts.get("AGENTS.md", "").rstrip().endswith(SENTINEL):
            errors.append(f"AGENTS.md: must end with the heading '{SENTINEL}'")
    if (root / "AGENTS.override.md").exists():
        errors.append("AGENTS.override.md: Codex would load it instead of AGENTS.md; remove it")
    for f in files:
        name = f.split("/")[-1]
        if name in {"CLAUDE.md", "CLAUDE.local.md"}:
            errors.append(f"{f}: a {name} stops Claude Code from reading AGENTS.md by default; remove it")

    # Paths named in instruction files must exist, and reachability follows those same edges. Every path the root
    # names is checked, because its path table is what routes agents; elsewhere only `.md` targets are, since detail
    # files also name build outputs and generated files (`website/dist/`, a release's `vX.Y.Z.json`) that are
    # legitimately absent from a checkout.
    # The old-section map names every file that root text used to live in; it is a lookup table, not a reading route,
    # so its mentions do not make anything reachable.
    edges: dict[str, set[str]] = {}
    for f in instruction:
        text = strip_fences(texts[f])
        targets: set[str] = set()
        found = [(ref, resolve(root, f, ref, top_level, basenames)) for ref in BACKTICK.findall(text)]
        found += [(link, resolve_link(root, f, link)) for link in re.findall(r"\]\(([^)\s]+)\)", text)]
        for ref, target in found:
            if target is None:
                continue
            if f != "AGENTS.md" and not ref.split("#")[0].endswith(".md"):
                continue
            if not target.exists():
                errors.append(f"{f}: names `{ref}`, which does not exist")
                continue
            rel = target.relative_to(root).as_posix() if target.is_relative_to(root) else None
            if rel and rel.endswith(".md") and f != ".agents/old-sections.md":
                targets.add(rel)
        edges[f] = targets

    reachable = {"AGENTS.md"}
    frontier = ["AGENTS.md"]
    while frontier:
        for nxt in edges.get(frontier.pop(), ()):
            if nxt not in reachable:
                reachable.add(nxt)
                if nxt in edges:
                    frontier.append(nxt)
    for f in instruction:
        if "/.agents/" in f"/{f}" and f not in reachable:
            errors.append(f"{f}: no instruction file reachable from AGENTS.md points at it")

    # Citations anywhere must name an existing file and heading; relative heading references must stay in-file.
    heading_cache: dict[str, set[str]] = {}
    root_headings = headings(texts.get("AGENTS.md", ""))
    old_names = old_section_names(texts.get(".agents/old-sections.md", ""))
    for f, text in texts.items():
        historical = warn_only(f)
        body = strip_fences(text) if f.endswith(".md") else text
        # Older prose cites root sections without the comma: root `AGENTS.md` "Finishing work", AGENTS.md's "X". Such
        # a citation must name a current root heading or an old one the map in `.agents/old-sections.md` resolves.
        for m in OLD_ROOT_CITATION.finditer(body):
            heading = " ".join(m.group(1).split())
            if heading not in root_headings and heading not in old_names:
                (warnings if historical else errors).append(
                    f'{f}: cites root AGENTS.md "{heading}", which is neither a heading nor in .agents/old-sections.md')
        for m in CITATION.finditer(body):
            path = m.group(1)
            target = resolve(root, f, path, top_level)
            if target is None:
                continue
            problem = None
            if not target.is_file():
                problem = f"cites `{path}`, which does not exist"
            else:
                key = target.as_posix()
                if key not in heading_cache:
                    heading_cache[key] = headings(target.read_text(errors="replace"))
                heading = cited_heading(body, m.end(), heading_cache[key])
                if heading not in heading_cache[key]:
                    problem = f'cites `{path}`, "{heading}", which has no such heading'
            if problem:
                (warnings if historical else errors).append(f"{f}: {problem}")
        if f in instruction:
            own = headings(text)
            for m in RELATIVE.finditer(strip_fences(text)):
                if " ".join(m.group(1).split()) not in own and m.group(1)[0].isupper():
                    errors.append(f'{f}: "{m.group(1)}" {m.group(2)} names no heading in this file')
    return errors, warnings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true", help="run the fixtures and exit")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    root = pathlib.Path(__file__).resolve().parent.parent
    errors, warnings = check(root, tracked_files(root))
    for w in warnings:
        print(f"warning: {w}")
    for e in errors:
        print(f"error: {e}")
    size = len((root / "AGENTS.md").read_bytes()) if (root / "AGENTS.md").is_file() else 0
    print(f"check-agents-md: root AGENTS.md {size}/{ROOT_LIMIT} bytes; {len(errors)} errors, {len(warnings)} warnings")
    return 1 if errors else 0


# ---------------------------------------------------------------------------------------------------------------------
# Self-test fixtures. Each case builds a small tree, runs `check` on it, and asserts on the messages. The point is that
# every rule above fails on the input it exists for and stays quiet on a well-formed tree, so a later edit to a regex
# cannot quietly turn a check into a no-op.


def _tree(base: pathlib.Path, files: dict[str, str]) -> list[str]:
    """Writes the fixture files under `base` and returns the file list `check` would get from git."""
    for name, body in files.items():
        p = base / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body)
    return sorted(files)


GOOD_ROOT = "# Root\n\nRead `.agents/a.md` and `sub/AGENTS.md`.\n\n# End of AGENTS.md\n"


def self_test() -> int:
    """Prove each instruction rule rejects its bad tree and accepts its valid counterpart.

    All forbidden filenames live only in temporary fixtures, never the checkout.
    """
    cases = []

    def case(name: str, files: dict[str, str], expect_error: str | None, expect_warning: str | None = None):
        cases.append((name, files, expect_error, expect_warning))

    good = {
        "AGENTS.md": GOOD_ROOT,
        ".agents/a.md": '# A\n\nSee `sub/AGENTS.md`, "Flows".\n',
        "sub/AGENTS.md": "# Sub\n\n## Flows\n\nRead `sub/.agents/b.md`.\n",
        "sub/.agents/b.md": "# B\n",
    }
    case("well-formed tree passes", good, None)
    case("root over budget", {**good, "AGENTS.md": GOOD_ROOT.replace("# End", "x" * ROOT_LIMIT + "\n\n# End")},
         "over the")
    case("missing sentinel", {**good, "AGENTS.md": "# Root\n\nRead `.agents/a.md` and `sub/AGENTS.md`.\n"},
         "must end with")
    case("override file", {**good, "AGENTS.override.md": "x"}, "AGENTS.override.md")
    case("nested CLAUDE.md", {**good, "sub/CLAUDE.md": "x"}, "CLAUDE.md stops")
    case("root CLAUDE.local.md", {**good, "CLAUDE.local.md": "x"}, "CLAUDE.local.md stops")
    case("nested CLAUDE.local.md", {**good, "sub/CLAUDE.local.md": "x"}, "CLAUDE.local.md stops")
    case("prohibiting an absent Claude local file is not a missing pointer",
         {**good, ".agents/a.md": good[".agents/a.md"] + "Never add `CLAUDE.local.md`.\n"}, None)
    case("pointer to missing file", {**good, ".agents/a.md": "# A\n\nRead `sub/.agents/gone.md`.\n"},
         "does not exist")
    case("unreachable detail file", {**good, "sub/.agents/orphan.md": "# Orphan\n"}, "orphan.md: no instruction")
    case("detail files pointing only at each other are unreachable",
         {**good, "x/.agents/p.md": "`x/.agents/q.md`\n", "x/.agents/q.md": "`x/.agents/p.md`\n"},
         "p.md: no instruction")
    case("citation of missing heading", {**good, ".agents/a.md": '# A\n\nSee `sub/AGENTS.md`, "Gone".\n'},
         "no such heading")
    case("citation wrapped across lines resolves",
         {**good, "sub/AGENTS.md": "# Sub\n\n## Two words\n\nRead `sub/.agents/b.md`.\n",
          ".agents/a.md": '# A\n\nSee `sub/AGENTS.md`, "Two\nwords".\n'}, None)
    case("heading containing quotes resolves",
         {**good, "sub/AGENTS.md": '# Sub\n\n## Picking: "pick a plan", "next"\n\nRead `sub/.agents/b.md`.\n',
          ".agents/a.md": '# A\n\nSee `sub/AGENTS.md`, "Picking: "pick a plan", "next"".\n'}, None)
    case("missing build output named by a detail file is not an error",
         {**good, ".agents/a.md": "# A\n\nRemove `sub/dist/` and `sub/AGENTS.md`.\n"}, None)
    case("missing directory in the root path table is an error",
         {**good, "AGENTS.md": GOOD_ROOT.replace("`sub/AGENTS.md`", "`sub/AGENTS.md` and `sub/gone/`")},
         "does not exist")
    case("citation in code is checked", {**good, "tool.py": '# see `sub/AGENTS.md`, "Gone"\n'}, "tool.py: cites")
    case("historical citation only warns",
         {**good, "TRIAGE_OUTCOMES.md": '# Old\n\nSee `sub/AGENTS.md`, "Gone".\n'}, None, "TRIAGE_OUTCOMES.md: cites")
    case("a bare name matching some tracked file passes; one matching none fails",
         {**good, "sub/queue/INDEX.md": "x\n", ".agents/a.md": "# A\n\n`INDEX.md` and `INDXE.md`\n"}, "`INDXE.md`")
    case("queued plan citations only warn, whatever the plan's state",
         {**good, "plans/queue/INDEX.md": "- [pending] `p.md` — x\n",
          "plans/queue/p.md": '# P\n\nSee `sub/AGENTS.md`, "Gone".\n'}, None, "plans/queue/p.md: cites")
    case("lore is not read at all",
         {**good, "lore/old.md": '# Old\n\nSee `sub/AGENTS.md`, "Gone".\n'}, None)
    case("typo in a definite path is an error",
         {**good, ".agents/a.md": "# A\n\nRead `sbu/AGENTS.md`.\n"}, "names `sbu/AGENTS.md`")
    case("typo in an all-caps file name is an error",
         {**good, ".agents/a.md": "# A\n\nRead `SPECC.md`.\n"}, "names `SPECC.md`")
    case("typo in a mixed-case spec name is an error",
         {**good, ".agents/a.md": "# A\n\nRead `SPEC_impll.md`.\n"}, "names `SPEC_impll.md`")
    case("existing mixed-case specification resolves",
         {**good, "SPEC_impl.md": "# Implementation\n",
          ".agents/a.md": good[".agents/a.md"] + "Read `SPEC_impl.md`.\n"}, None)
    case("lowercase bare fragment names remain examples",
         {**good, ".agents/a.md": good[".agents/a.md"] + "Write `cursor-launch.md`.\n"}, None)
    case("markdown links resolve relative to the citing file",
         {**good, ".agents/a.md": "# A\n\nRead [b](.agents/b.md).\n", ".agents/b.md": "# B\n"}, "names `.agents/b.md`")
    case("the old-section map is not a reading route",
         {**good, ".agents/old-sections.md": "# Old\n\n`sub/.agents/c.md`\n", "sub/.agents/c.md": "# C\n",
          "AGENTS.md": GOOD_ROOT.replace("`.agents/a.md`", "`.agents/a.md`, `.agents/old-sections.md`")},
         "c.md: no instruction")
    case("old-form root citation of a vanished heading is an error",
         {**good, "tool.py": '# per root `AGENTS.md` "Finishing work"\n'}, 'cites root AGENTS.md "Finishing work"')
    case("old-form root citation resolved by the old-section map passes",
         {**good, ".agents/old-sections.md": "| Old | New |\n| --- | --- |\n| Finishing work | x |\n",
          "AGENTS.md": GOOD_ROOT.replace("`.agents/a.md`", "`.agents/a.md`, `.agents/old-sections.md`"),
          "tool.py": '# per root `AGENTS.md` "Finishing work"\n'}, None)
    case("relative reference to a moved heading",
         {**good, ".agents/a.md": '# A\n\nsee "Vercel deployments" below.\n'}, "names no heading")
    case("commands and runtime paths are not paths",
         {**good, ".agents/a.md": "# A\n\nRun `cargo test`, keep `~/.topic`, write `<slug>.md`.\n"}, None)
    case("examples in code fences are ignored",
         {**good, ".agents/a.md": '# A\n\n```\n`sub/.agents/none.md`, "Nope"\n# End of AGENTS.md\n```\n'}, None)

    failed = 0
    for name, files, expect_error, expect_warning in cases:
        with tempfile.TemporaryDirectory(prefix="check-agents-md-") as tmp:
            base = pathlib.Path(tmp)
            errors, warnings = check(base, _tree(base, files))
        ok = (any(expect_error in e for e in errors) if expect_error else not errors)
        ok = ok and (any(expect_warning in w for w in warnings) if expect_warning else True)
        if not ok:
            failed += 1
            print(f"FAIL {name}: errors={errors} warnings={warnings}")
    print(f"check-agents-md self-test: {len(cases) - failed}/{len(cases)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
