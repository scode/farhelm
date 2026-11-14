#!/usr/bin/env python3
"""Generate, or check, the dependency license notices that `farhelm licenses` prints.

Every shipped Farhelm binary carries the license notices of the crates compiled into it (decision D-18): the MIT, BSD,
ISC, Apache and other licenses those crates are under all require their notice to travel with copies of the code.
`crates/farhelm/notices/dependencies.txt` holds them, checked in so a release needs no extra step, and `farhelm`
embeds it at build time. This script is the only way that file is written:

    python3 scripts/dependency-notices.py           # regenerate the file
    python3 scripts/dependency-notices.py --check   # fail when the committed file no longer matches the graph

How it is built. The pinned cargo-about (`scripts/install-pinned-cargo-about.py`) runs once per shipped binary
(`farhelm`, `farhelm-desktop`) with `crates/farhelm/notices/about.toml`: normal and build dependencies only, for every
release target, offline against the locked graph after `cargo fetch --locked`. Its JSON output is merged here rather
than rendered by a cargo-about template, because one template run covers one root crate and the file must cover both
binaries. Entries with the same license text are grouped, each naming the crates under it. Farhelm's own crates (under
`crates/`) are left out; vendored upstream code under `vendor/` is third-party and stays in.

Where a crate's package has no usable notice, cargo-about silently prints the SPDX template, with placeholders such as
`<year> <copyright holders>` where the copyright line belongs; `--fail` does not catch that. Two inputs close the gap:
`clarify` entries in `about.toml` for crates that ship a file cargo-about did not match, and `upstream.toml` with
checked-in upstream texts for crates whose package has none or splits its notice over several files. This script
then refuses to write, or to pass `--check`, when any MIT, BSD, ISC, Zlib or NCSA text still holds a placeholder or
has no copyright line, unless `upstream.toml` acknowledges that the crate's upstream publishes none, and when an
`upstream.toml` entry no longer matches the graph.
What else makes it fail: cargo-about's `--fail` (a license it cannot identify) and its `accepted` list (a license
outside `deny.toml`'s policy). Nothing local leaks into the file: no paths are printed, only crate names, versions and
license texts, so every checkout produces the same bytes.

The check is in the ordinary checks (CI's `deny` job, `.agents/validation.md`) and runs when `Cargo.lock`, a manifest,
`about.toml` or `upstream.toml` changes. `scripts/test-dependency-notices.py` covers the merge, filter, render, check
and copyright-guard logic offline.
"""

import argparse
import difflib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
import tomllib
import urllib.parse


REPO = Path(__file__).resolve().parent.parent
CONFIG = Path("crates/farhelm/notices/about.toml")
UPSTREAM = Path("crates/farhelm/notices/upstream.toml")
OUTPUT = Path("crates/farhelm/notices/dependencies.txt")
# The binaries a release ships. A binary added to a release adds its manifest here.
BINARIES = (Path("crates/farhelm/Cargo.toml"), Path("crates/farhelm-desktop/Cargo.toml"))
REGENERATE = "python3 scripts/dependency-notices.py"
RULE = "=" * 80
WIDTH = 120

HEADER = f"""\
Third-party license notices for Farhelm's farhelm and farhelm-desktop binaries

These are the license notices of the open-source crates compiled into those binaries, grouped by license text. Each
group names the crates it covers, then gives the license text as the crates ship it. Where cargo-about cannot print a
crate's notice as published (its package carries none, or the notice spans several files), the text was taken from the
crate's repository or package, as its "Upstream text:" lines cite; where upstream publishes no copyright line, one cited
sentence says so before the license terms.

The source code of each crate, at the version named, is available from crates.io (https://crates.io), and for the
crates vendored under `vendor/`, from Farhelm's own source repository. This includes the crates under the Mozilla
Public License 2.0, whose source must be made available to recipients of the binaries.

Generated from Cargo.lock by `{REGENERATE}`; do not edit by hand.
"""


def crate_path(crate_id):
    """The local directory of a path dependency named by a cargo package id, or None for a registry crate."""
    if not crate_id.startswith("path+file://"):
        return None
    # Package ids are URLs: a checkout path with a space or other reserved character arrives percent-encoded.
    return Path(urllib.parse.unquote(crate_id[len("path+file://"):].split("#", 1)[0]))


def is_own_crate(crate, repo):
    """Whether a crate is Farhelm's own (a workspace crate under `crates/`), which needs no third-party notice.

    Vendored upstream trees under `vendor/` are path dependencies too, but their code is third-party and keeps its
    notice, so only `crates/` counts as Farhelm's own.
    """
    path = crate_path(crate["id"])
    if path is None:
        return False
    try:
        relative = path.resolve().relative_to(repo.resolve())
    except ValueError:
        return False
    return relative.parts[:1] == ("crates",)


def normalize(text):
    """A license text with line endings unified, trailing spaces dropped and surrounding blank lines removed.

    Only whitespace that carries no meaning changes, so two crates shipping the same license file group together and the
    output is stable across checkouts.
    """
    lines = [line.rstrip() for line in text.replace("\r\n", "\n").replace("\r", "\n").split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines)


# Licenses whose terms require the copyright notice itself to be reproduced, so a text for them must carry a real
# copyright line, never the SPDX template's placeholder.
NEEDS_COPYRIGHT_LINE = {"MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "NCSA"}
# The placeholders SPDX templates (and REUSE-style LICENSES/ files copied from them) put where the holder belongs, in
# angle or square brackets: `<year>`, `<Owner Organization Name>` (NCSA), `[fullname]` and the like.
PLACEHOLDER = re.compile(
    r"<\s*(year|owner[^>]*|copyright holders?|name of author|organization[^>]*)\s*>"
    r"|\[(yyyy|year|fullname|name of copyright owner)\]",
    re.I,
)
# Bare, unfilled stand-ins a crate may ship on its copyright line instead of a holder (`Copyright (c) YEAR AUTHOR`):
# uppercase on purpose, so a real line naming "The Rust Project Developers" or "the AUTHORS file" still counts.
UNFILLED = re.compile(r"\b(YEAR|YYYY|AUTHOR|OWNER|COPYRIGHT HOLDER)\b(?!\s+file)")


def is_placeholder_line(line):
    """Whether a copyright line holds a template placeholder or an unfilled stand-in instead of a real holder."""
    return bool(PLACEHOLDER.search(line) or UNFILLED.search(line))
# A line that states a copyright, as opposed to the license's own words: case-sensitive, so a wrapped "copyright notice
# and this permission notice" or the warranty clause's "COPYRIGHT HOLDERS" never counts as one.
COPYRIGHT_LINE = re.compile(
    r"\W*(Copyright\b(?!\s+(notice|holders?|owners?|license|statement|and|law|protection))|\([cC]\)\s*\d|©)"
)


def has_copyright_line(text):
    """Whether a license text states at least one real copyright: a copyright line holding no template placeholder.

    A shipped file may carry full template texts after its real notices (aws-lc-sys does), so a placeholder alone does
    not condemn a text; a text whose only copyright lines are placeholders does.
    """
    return any(
        COPYRIGHT_LINE.match(line) and not is_placeholder_line(line) for line in text.split("\n")
    )


def load_upstream(repo):
    """`upstream.toml`'s entries as a dict keyed by (crate, version, license id).

    Each value is (text, acknowledged): the checked-in text, or None to keep cargo-about's (an entry that only
    acknowledges), and whether the entry acknowledges that upstream publishes no copyright line. An entry claiming that
    must say why, and only such an entry may leave out `file`.
    """
    path = repo / UPSTREAM
    if not path.exists():
        return {}
    overrides = {}
    for entry in tomllib.loads(path.read_text(encoding="utf-8")).get("text", []):
        acknowledged = bool(entry.get("no-copyright-line", False))
        label = entry.get("file") or ", ".join(entry["crates"])
        if acknowledged and not entry.get("reason", "").strip():
            raise SystemExit(f"dependency-notices: {UPSTREAM}: {label} sets no-copyright-line without a reason")
        if not acknowledged and "file" not in entry:
            raise SystemExit(f"dependency-notices: {UPSTREAM}: {label} has neither a file nor no-copyright-line")
        text = normalize((path.parent / entry["file"]).read_text(encoding="utf-8")) if "file" in entry else None
        for spec in entry["crates"]:
            name, version = spec.split(" ")
            overrides[(name, version, entry["license"])] = (text, acknowledged)
    return overrides


def load_extras(repo):
    """`upstream.toml`'s `[[extra]]` entries: notices for code a crate compiles in under a license it does not declare.

    Returns a list of (license id, license name, text, crates) with crates as (name, version) pairs. cargo-about only
    reports the licenses a crate declares, so a bundled C library under its own license (FreeType in freetype-sys) would
    otherwise ship without its notice.
    """
    path = repo / UPSTREAM
    if not path.exists():
        return []
    extras = []
    for entry in tomllib.loads(path.read_text(encoding="utf-8")).get("extra", []):
        text = normalize((path.parent / entry["file"]).read_text(encoding="utf-8"))
        crates = frozenset(tuple(spec.split(" ")) for spec in entry["crates"])
        extras.append((entry["license"], entry["name"], text, crates))
    return extras


def merge(reports, repo, overrides=None, extras=None):
    """Group every license text in cargo-about's reports with the third-party crates that use it.

    Returns a dict keyed by (license id, license name, normalized text) whose values are sets of (crate, version).
    A text used only by Farhelm's own crates disappears. Where `overrides` (from `upstream.toml`) holds a text for a
    crate and license, that crate is filed under the override's text instead of cargo-about's.
    """
    overrides = overrides or {}
    groups = {}
    for report in reports:
        for license in report["licenses"]:
            for use in license["used_by"]:
                if is_own_crate(use["crate"], repo):
                    continue
                user = (use["crate"]["name"], use["crate"]["version"])
                override = overrides.get((*user, license["id"]))
                text = override[0] if override and override[0] is not None else normalize(license["text"])
                groups.setdefault((license["id"], license["name"], text), set()).add(user)
    present = {user for users in groups.values() for user in users}
    for license_id, name, text, crates in extras or ():
        # An extra notice stands only while the graph still has its crates; `problems` reports a stale entry.
        if crates <= present:
            groups.setdefault((license_id, name, text), set()).update(crates)
    return groups


def problems(groups, overrides, extras=None):
    """Why the merged groups must not ship: placeholders, missing copyright lines, and stale `upstream.toml` entries.

    A text for a license in `NEEDS_COPYRIGHT_LINE` must hold no template placeholder, and must contain a copyright line
    unless every crate using it is acknowledged in `upstream.toml` as having none upstream. An `upstream.toml` entry for
    a crate, version or license the graph no longer has is stale: a version bump has to be checked again.
    """
    found = []
    acknowledged = {key for key, (_text, ack) in overrides.items() if ack}
    used = set()
    for (license_id, _name, text), users in groups.items():
        used.update((name, version, license_id) for name, version in users)
        if license_id not in NEEDS_COPYRIGHT_LINE:
            continue
        if has_copyright_line(text):
            continue
        crates = ", ".join(f"{name} {version}" for name, version in sorted(users))
        # The template is never acceptable, acknowledged or not: it prints a placeholder where a holder belongs.
        if any(COPYRIGHT_LINE.match(line) and is_placeholder_line(line) for line in text.split("\n")):
            found.append(f"{license_id} text for {crates} is the SPDX template, with placeholders; give the crate's own"
                         f" text ({CONFIG} clarify, or {UPSTREAM})")
        elif not all((n, v, license_id) in acknowledged for n, v in users):
            found.append(f"{license_id} text for {crates} has no copyright line; give the crate's own text ({CONFIG}"
                         f" clarify, or {UPSTREAM}), or acknowledge in {UPSTREAM} that upstream publishes none")
    present = {user for users in groups.values() for user in users}
    for license_id, _name, _text, crates in extras or ():
        if not crates <= present:
            missing = ", ".join(f"{name} {version}" for name, version in sorted(crates - present))
            found.append(f"{UPSTREAM} has a {license_id} extra notice for {missing}, which the graph no longer has;"
                         " check the current version and update the entry")
    for name, version, license_id in sorted(set(overrides) - used):
        found.append(f"{UPSTREAM} has a {license_id} text for {name} {version}, which the graph no longer has under"
                     " that license; check the current version's text and update the entry")
    return found


def render(groups):
    """The notices file: the header, then one section per license text, ordered by license id and first crate."""
    def order(item):
        (license_id, _name, text), users = item
        return (license_id, sorted(users)[0], text)

    sections = [HEADER]
    for (license_id, name, text), users in sorted(groups.items(), key=order):
        used_by = ", ".join(f"{crate} {version}" for crate, version in sorted(users))
        # cargo-about files the Apache text with LLVM's exception under plain Apache-2.0; name what the text is.
        if license_id == "Apache-2.0" and "LLVM Exceptions to the Apache 2.0 License" in text:
            name, license_id = "Apache License 2.0 with LLVM Exceptions", "Apache-2.0 WITH LLVM-exception"
        sections.append(
            "\n".join([
                RULE,
                f"{name} ({license_id})",
                textwrap.fill(f"Used by: {used_by}", WIDTH, break_on_hyphens=False, break_long_words=False),
                "",
                text,
            ])
            + "\n"
        )
    return "\n".join(sections)


# cargo-about scans license files on a rayon pool sized to every core by default, which saturated the shared machine;
# `RAYON_NUM_THREADS` (which cargo-about honors) caps it. Set for the cargo-about child only, never for this process.
CARGO_ABOUT_THREADS = "4"


def cargo_about_reports(repo, cargo_about, run=subprocess.run):
    """Run cargo-about once per shipped binary and return its parsed JSON reports.

    One run per binary rather than one `--workspace` run: the workspace also has members no shipped binary links (the
    lint demo, test fixtures), and `--workspace` would put their dependencies in the file; filtering them back out would
    need a dependency closure per release target, which is what cargo-about's per-binary run already computes.

    `run` is `subprocess.run`'s shape so tests can stand in for cargo and cargo-about without the process environment.
    """
    run(["cargo", "fetch", "--locked"], cwd=repo, check=True)
    reports = []
    with tempfile.TemporaryDirectory() as scratch:
        for index, manifest in enumerate(BINARIES):
            output = Path(scratch) / f"report-{index}.json"
            run(
                [
                    str(cargo_about), "generate", "--format", "json", "--frozen", "--fail",
                    "-c", str(repo / CONFIG), "-m", str(repo / manifest), "-o", str(output),
                ],
                cwd=repo, check=True, env={**os.environ, "RAYON_NUM_THREADS": CARGO_ABOUT_THREADS},
            )
            reports.append(json.loads(output.read_text(encoding="utf-8")))
    return reports


def install_cargo_about(repo, run=subprocess.run):
    """The pinned cargo-about executable, installed (or reused) by its installer."""
    result = run(
        [sys.executable, "-B", str(repo / "scripts/install-pinned-cargo-about.py")],
        cwd=repo, check=True, capture_output=True, text=True,
    )
    return Path(result.stdout.strip()) / "cargo-about"


def stale_report(expected, actual):
    """A short explanation of how the committed file differs from what the graph produces, or None when they match."""
    if expected == actual:
        return None
    diff = list(difflib.unified_diff(
        actual.splitlines(), expected.splitlines(), f"{OUTPUT} (committed)", f"{OUTPUT} (generated)", lineterm="",
        n=1,
    ))
    shown = "\n".join(diff[:40]) + ("\n..." if len(diff) > 40 else "")
    return f"{OUTPUT} is stale: regenerate it with `{REGENERATE}` and commit the result.\n{shown}"


def is_notice_input(path):
    """Whether a changed path can change the notices file: the lockfile, any manifest, or the notices' own inputs."""
    name = path.rsplit("/", 1)[-1]
    return (
        name in ("Cargo.lock", "Cargo.toml", "dist-workspace.toml", "deny.toml")
        or path.startswith("crates/farhelm/notices/")
        or path == "scripts/dependency-notices.py"
    )


def inputs_changed(repo, rev, run=subprocess.run):
    """Whether any notices input differs between `rev` and the working tree (committed or not)."""
    result = run(["git", "diff", "--name-only", rev], cwd=repo, check=True, capture_output=True, text=True)
    untracked = run(
        ["git", "ls-files", "--others", "--exclude-standard"], cwd=repo, check=True, capture_output=True, text=True,
    )
    paths = (result.stdout + untracked.stdout).split()
    return any(is_notice_input(path) for path in paths)


def main(argv=None, repo=REPO, run=subprocess.run):
    """Regenerate the notices file, or with --check compare it with a fresh generation."""
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="fail when the committed file is stale; write nothing")
    parser.add_argument(
        "--cargo-about", type=Path,
        help="an already verified cargo-about executable (CI installs it into its temporary directory first)",
    )
    parser.add_argument(
        "--only-if-inputs-changed-since", metavar="REV",
        help="skip (exit 0) unless Cargo.lock, a Cargo.toml or the notices inputs differ from REV: the local and"
        " landing use, since a run costs about three minutes of CPU",
    )
    args = parser.parse_args(argv)
    if args.only_if_inputs_changed_since and not inputs_changed(repo, args.only_if_inputs_changed_since, run):
        print(f"dependency-notices: no dependency input changed since {args.only_if_inputs_changed_since}; skipped")
        return 0
    cargo_about = args.cargo_about or install_cargo_about(repo, run)
    overrides = load_upstream(repo)
    extras = load_extras(repo)
    groups = merge(cargo_about_reports(repo, cargo_about, run), repo, overrides, extras)
    found = problems(groups, overrides, extras)
    if found:
        for problem in found:
            print(f"dependency-notices: {problem}", file=sys.stderr)
        return 1
    generated = render(groups)
    path = repo / OUTPUT
    if args.check:
        committed = path.read_text(encoding="utf-8") if path.exists() else ""
        problem = stale_report(generated, committed)
        if problem:
            print(f"dependency-notices: {problem}", file=sys.stderr)
            return 1
        print(f"dependency-notices: {OUTPUT} is current")
        return 0
    path.write_text(generated, encoding="utf-8")
    print(f"dependency-notices: wrote {OUTPUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
