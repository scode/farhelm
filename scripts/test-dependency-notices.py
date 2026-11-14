#!/usr/bin/env python3
"""Exercise `dependency-notices.py`'s merge, filter, render and check logic offline.

The notices file is what makes the shipped binaries meet their dependencies' license terms, so its generator must not
silently drop a crate, leak a local path, or let a stale file pass the check. These tests stand in for cargo and
cargo-about with fixture reports (no network, no cargo-about binary) and pin those decisions: Farhelm's own crates are
left out while vendored upstream crates stay in, identical texts from the two binaries become one group, output is
stable and path-free, and `--check` fails on a stale or missing file while naming the regenerate command. They also pin
the copyright guard: cargo-about prints the SPDX template (`<year> <copyright holders>`) when it cannot match a crate's
own file, and silently, so the generator itself must refuse any MIT, BSD, ISC or Zlib text without a real copyright
line unless `upstream.toml` acknowledges that upstream publishes none.
"""

import importlib.util
import io
import json
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest


_path = Path(__file__).with_name("dependency-notices.py")
_spec = importlib.util.spec_from_file_location("dependency_notices", _path)
notices = importlib.util.module_from_spec(_spec)
exec(compile(_path.read_bytes(), str(_path), "exec"), notices.__dict__)

MIT = "Copyright (c) Someone\r\n\r\nPermission is hereby granted, free of charge   \r\n"
TEMPLATE = "MIT License\n\nCopyright (c) <year> <copyright holders>\n\nPermission is hereby granted, free of charge\n"
NO_HOLDER = (
    "Permission is hereby granted, free of charge, to any person.\n\n"
    "The above copyright notice and this permission notice\n"
)
REAL_REPO = Path(__file__).resolve().parent.parent


def crate(name, version, crate_id):
    """A `used_by` entry the way cargo-about reports it (only the fields the generator reads, plus a local path)."""
    return {"crate": {"name": name, "version": version, "id": crate_id}, "path": "/somewhere/local/LICENSE"}


def report(repo, *licenses):
    """A cargo-about JSON report holding the given (id, name, text, users) licenses."""
    return {
        "overview": [],
        "crates": [],
        "licenses": [
            {"id": lid, "name": name, "text": text, "source_path": "/home/someone/.cargo/x/LICENSE", "used_by": users}
            for lid, name, text, users in licenses
        ],
    }


class DependencyNoticesTests(unittest.TestCase):
    """Keep the generated dependency notices complete, stable and free of local detail."""

    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.repo = Path(self.scratch.name)
        (self.repo / "crates/farhelm/notices").mkdir(parents=True)
        self.registry = lambda name: crate(
            name, "1.0.0", f"registry+https://github.com/rust-lang/crates.io-index#{name}@1.0.0"
        )
        self.own = crate("farhelm-proto", "0.0.0", f"path+file://{self.repo}/crates/farhelm-proto#0.0.0")
        self.vendored = crate("vte", "0.15.0", f"path+file://{self.repo}/vendor/vte#0.15.0")

    def tearDown(self):
        self.scratch.cleanup()

    def fake_run(self, reports):
        """A `subprocess.run` stand-in: cargo fetch succeeds, cargo-about writes the next fixture report to `-o`."""
        queue = list(reports)
        calls = []

        def run(command, **_kwargs):
            calls.append(command)
            if command[:2] == ["cargo", "fetch"]:
                return subprocess.CompletedProcess(command, 0)
            output = Path(command[command.index("-o") + 1])
            output.write_text(json.dumps(queue.pop(0)))
            return subprocess.CompletedProcess(command, 0)

        return run, calls

    def test_own_crates_are_left_out_and_vendored_upstream_code_stays(self):
        """A text used only by Farhelm's own crates disappears; a vendored crate under vendor/ keeps its notice."""
        groups = notices.merge([report(
            self.repo,
            ("MIT", "MIT License", MIT, [self.own]),
            ("Apache-2.0", "Apache License 2.0", "apache text", [self.vendored, self.own]),
        )], self.repo)
        self.assertEqual(list(groups.values()), [{("vte", "0.15.0")}])

    def test_the_same_text_from_both_binaries_is_one_group(self):
        """Identical license texts merge across reports and across insignificant whitespace, uniting their crates."""
        first = report(self.repo, ("MIT", "MIT License", MIT, [self.registry("a")]))
        second = report(self.repo, ("MIT", "MIT License", MIT.replace("\r\n", "\n"), [self.registry("b")]))
        groups = notices.merge([first, second], self.repo)
        self.assertEqual(len(groups), 1)
        self.assertEqual(next(iter(groups.values())), {("a", "1.0.0"), ("b", "1.0.0")})

    def test_output_is_ordered_and_names_no_local_path(self):
        """The rendering does not depend on report order and prints names, versions and texts, never paths."""
        a = report(self.repo, ("MIT", "MIT License", MIT, [self.registry("zeta")]),
                   ("Apache-2.0", "Apache License 2.0", "apache", [self.registry("alpha")]))
        b = report(self.repo, ("Apache-2.0", "Apache License 2.0", "apache", [self.registry("alpha")]),
                   ("MIT", "MIT License", MIT, [self.registry("zeta")]))
        first = notices.render(notices.merge([a], self.repo))
        self.assertEqual(first, notices.render(notices.merge([b], self.repo)))
        self.assertLess(first.index("(Apache-2.0)"), first.index("(MIT)"))
        self.assertIn("Used by: zeta 1.0.0", first)
        self.assertNotIn("/home/", first)
        self.assertNotIn("/somewhere/", first)
        self.assertNotIn("\r", first)
        self.assertIn("Permission is hereby granted, free of charge\n", first)

    def test_check_fails_on_a_stale_or_missing_file_and_passes_on_a_current_one(self):
        """`--check` compares with a fresh generation, writes nothing, and its refusal names the regenerate command."""
        fixture = report(self.repo, ("MIT", "MIT License", MIT, [self.registry("a")]))
        path = self.repo / notices.OUTPUT
        cargo_about = ["--cargo-about", "/fixture/cargo-about"]
        for label, committed, expected in (("missing", None, 1), ("stale", "old notices\n", 1), ("current", "", 0)):
            with self.subTest(label):
                if committed is None:
                    path.unlink(missing_ok=True)
                elif label == "current":
                    path.write_text(notices.render(notices.merge([fixture, fixture], self.repo)))
                else:
                    path.write_text(committed)
                before = path.read_text() if path.exists() else None
                run, calls = self.fake_run([fixture, fixture])
                errors = io.StringIO()
                with redirect_stderr(errors), redirect_stdout(io.StringIO()):
                    status = notices.main(["--check", *cargo_about], repo=self.repo, run=run)
                self.assertEqual(status, expected)
                self.assertEqual(path.read_text() if path.exists() else None, before, "the check must write nothing")
                if expected:
                    self.assertIn(notices.REGENERATE, errors.getvalue())
                # One fetch, then one cargo-about run per shipped binary, offline and failing on unreadable licenses.
                self.assertEqual(calls[0], ["cargo", "fetch", "--locked"])
                self.assertEqual(len(calls), 1 + len(notices.BINARIES))
                for call in calls[1:]:
                    self.assertIn("--frozen", call)
                    self.assertIn("--fail", call)

    def write_upstream(self, body, files=()):
        """Write a fixture `upstream.toml` and its text files into the scratch repository."""
        (self.repo / notices.UPSTREAM).write_text(body, encoding="utf-8")
        for name, text in files:
            target = self.repo / notices.UPSTREAM.parent / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding="utf-8")

    def problems_for(self, *licenses):
        """`problems()` for one fixture report, with the scratch repository's `upstream.toml`."""
        overrides = notices.load_upstream(self.repo)
        return notices.problems(notices.merge([report(self.repo, *licenses)], self.repo, overrides), overrides)

    def test_a_template_or_holderless_text_fails_naming_its_crates(self):
        """The SPDX template, or an MIT text with no copyright line, must never ship; a real copyright line passes.

        Guards the silent fallback the gate found: cargo-about printed `Copyright (c) <year> <copyright holders>` for 21
        crates under `--fail` without a word.
        """
        cases = (("template", TEMPLATE, "SPDX template"), ("no holder", NO_HOLDER, "no copyright"))
        for label, text, expected in cases:
            with self.subTest(label):
                found = self.problems_for(("MIT", "MIT License", text, [self.registry("objc2")]))
                self.assertEqual(len(found), 1)
                self.assertIn("objc2 1.0.0", found[0])
                self.assertIn(expected, found[0])
        self.assertEqual(self.problems_for(("MIT", "MIT License", MIT, [self.registry("ok")])), [])
        # Apache-2.0 does not require a copyright line, so its canonical text is fine.
        self.assertEqual(self.problems_for(("Apache-2.0", "Apache License 2.0", "TERMS", [self.registry("a")])), [])
        # A shipped file with real notices may also carry template texts (aws-lc-sys does): it passes.
        mixed = MIT + "\nFull texts:\nCopyright (c) <year> <owner>.\n"
        self.assertEqual(self.problems_for(("BSD-3-Clause", "BSD", mixed, [self.registry("aws")])), [])
        # The license's own words never count as a copyright line, wherever a line wraps.
        wrapped = "Permission to use\ncopyright notice and this permission notice appear\nCOPYRIGHT HOLDERS BE LIABLE\n"
        self.assertEqual(len(self.problems_for(("ISC", "ISC", wrapped, [self.registry("isc")]))), 1)

    def test_ncsa_templates_and_unfilled_stand_ins_fail_while_real_holders_pass(self):
        """NCSA needs its copyright line like MIT, and an unfilled `YEAR AUTHOR` line is no holder either.

        Guards the second gate round's findings: NCSA (on the allowlist) was outside the guard, and a crate shipping
        `Copyright (c) YEAR AUTHOR` would have passed as if it named someone. Real lines that merely mention an AUTHORS
        file or "The Rust Project Developers" must still count.
        """
        ncsa = "Copyright (c) <Year> <Owner Organization Name>. All rights reserved.\nTERMS\n"
        found = self.problems_for(("NCSA", "NCSA", ncsa, [self.registry("llvm")]))
        self.assertEqual(len(found), 1)
        self.assertIn("SPDX template", found[0])
        unfilled = "Copyright (c) YEAR AUTHOR\n\nPermission is hereby granted\n"
        found = self.problems_for(("MIT", "MIT License", unfilled, [self.registry("blank")]))
        self.assertEqual(len(found), 1)
        self.assertIn("SPDX template", found[0])
        for real in (
            "Copyright (c) 2018, the respective contributors, as shown by the AUTHORS file.\n",
            "Copyright (c) 2014 The Rust Project Developers\n",
        ):
            with self.subTest(real):
                self.assertEqual(self.problems_for(("MIT", "MIT License", real, [self.registry("real")])), [])

    def test_the_gated_check_runs_only_when_a_dependency_input_changed(self):
        """The local and landing use skips cargo-about unless the lockfile, a manifest or a notices input changed.

        Guards the cost cut the orchestrator asked for: a run costs about three minutes of CPU, so an ordinary code
        change must not pay it, while a lockfile change must never skip the check.
        """
        for changed, expect_run in (("crates/farhelm-gui/src/lib.rs\n", False), ("Cargo.lock\n", True),
                                    ("crates/x/Cargo.toml\n", True), ("crates/farhelm/notices/upstream.toml\n", True)):
            with self.subTest(changed):
                calls = []

                def run(command, **_kwargs):
                    calls.append(command)
                    if command[:2] == ["git", "diff"]:
                        return subprocess.CompletedProcess(command, 0, stdout=changed)
                    if command[:2] == ["git", "ls-files"]:
                        return subprocess.CompletedProcess(command, 0, stdout="")
                    raise RuntimeError("stop after the gate")

                if expect_run:
                    with self.assertRaises(RuntimeError):
                        notices.main(["--check", "--only-if-inputs-changed-since", "main"], self.repo, run)
                else:
                    gated = ["--check", "--only-if-inputs-changed-since", "main"]
                    self.assertEqual(notices.main(gated, self.repo, run), 0)
                    self.assertFalse(any(c[0] != "git" for c in calls), "a skipped check must not run cargo-about")

    def test_cargo_about_runs_with_a_capped_thread_pool(self):
        """cargo-about saturated every core of the shared machine; its child process gets RAYON_NUM_THREADS=4."""
        envs = []

        def run(command, **kwargs):
            if command[:2] == ["cargo", "fetch"]:
                return subprocess.CompletedProcess(command, 0)
            envs.append(kwargs.get("env") or {})
            Path(command[command.index("-o") + 1]).write_text(json.dumps({"licenses": []}))
            return subprocess.CompletedProcess(command, 0)

        notices.cargo_about_reports(self.repo, Path("/bin/false"), run)
        self.assertTrue(envs)
        self.assertTrue(all(env.get("RAYON_NUM_THREADS") == "4" for env in envs))

    def test_an_extra_notice_ships_with_its_crate_and_goes_stale_without_it(self):
        """A license a crate compiles in but does not declare gets its own section, and only while the crate is linked.

        Guards the FreeType case: freetype-sys declares MIT but compiles in FreeType, whose license (FTL) requires a
        credit in the documentation of distributed binaries; cargo-about never reports it.
        """
        upstream = self.repo / "crates/farhelm/notices/upstream"
        upstream.mkdir()
        (upstream / "ft.txt").write_text("Portions of this software are copyright (c) 2023 The FreeType Project.\n")
        (self.repo / "crates/farhelm/notices/upstream.toml").write_text(
            '[[extra]]\nlicense = "FTL"\nname = "Freetype Project License"\n'
            'crates = ["freetype-sys 1.0.0"]\nfile = "upstream/ft.txt"\n'
        )
        overrides = notices.load_upstream(self.repo)
        extras = notices.load_extras(self.repo)
        linked = notices.merge(
            [report(self.repo, ("MIT", "MIT License", MIT, [self.registry("freetype-sys")]))], self.repo, overrides,
            extras,
        )
        self.assertIn("Freetype Project License (FTL)", notices.render(linked))
        self.assertEqual(notices.problems(linked, overrides, extras), [])
        gone = notices.merge(
            [report(self.repo, ("MIT", "MIT License", MIT, [self.registry("other")]))], self.repo, overrides, extras,
        )
        self.assertNotIn("FTL", notices.render(gone))
        found = notices.problems(gone, overrides, extras)
        self.assertEqual(len(found), 1)
        self.assertIn("freetype-sys 1.0.0", found[0])

    def test_an_upstream_text_replaces_the_template_for_exactly_its_crates(self):
        """`upstream.toml` puts the checked-in text in place of cargo-about's for the crates and license it names."""
        self.write_upstream(
            '[[text]]\nlicense = "MIT"\ncrates = ["objc2 1.0.0"]\nfile = "upstream/objc2.txt"\n',
            [("upstream/objc2.txt", "Upstream text: https://example.invalid/LICENSE\n\nCopyright (c) Real Holder\n")],
        )
        overrides = notices.load_upstream(self.repo)
        groups = notices.merge([report(
            self.repo, ("MIT", "MIT License", TEMPLATE, [self.registry("objc2"), self.registry("other")]),
        )], self.repo, overrides)
        by_text = {text: users for (_id, _name, text), users in groups.items()}
        self.assertEqual(by_text["Upstream text: https://example.invalid/LICENSE\n\nCopyright (c) Real Holder"],
                         {("objc2", "1.0.0")})
        self.assertEqual(by_text[notices.normalize(TEMPLATE)], {("other", "1.0.0")})
        found = notices.problems(groups, overrides)
        self.assertEqual(len(found), 1)
        self.assertIn("other 1.0.0", found[0])

    def test_acknowledging_no_copyright_line_needs_a_reason_and_still_rejects_placeholders(self):
        """An acknowledgement lets a holderless text through only with a reason; it never admits the template."""
        self.write_upstream('[[text]]\nlicense = "MIT"\ncrates = ["endi 1.0.0"]\nno-copyright-line = true\n'
                            'reason = "upstream LICENSE-MIT has none"\n')
        self.assertEqual(self.problems_for(("MIT", "MIT License", NO_HOLDER, [self.registry("endi")])), [])
        self.assertEqual(len(self.problems_for(("MIT", "MIT License", TEMPLATE, [self.registry("endi")]))), 1)
        self.write_upstream('[[text]]\nlicense = "MIT"\ncrates = ["endi 1.0.0"]\nno-copyright-line = true\n')
        with self.assertRaises(SystemExit):
            notices.load_upstream(self.repo)

    def test_an_upstream_entry_the_graph_no_longer_has_is_stale(self):
        """A version bump must not keep an old version's checked-in text silently: the stale entry fails."""
        self.write_upstream(
            '[[text]]\nlicense = "MIT"\ncrates = ["taffy 0.15.1"]\nfile = "upstream/taffy.txt"\n',
            [("upstream/taffy.txt", "Copyright (c) 2018 Visly Inc.\n")],
        )
        found = self.problems_for(("MIT", "MIT License", MIT, [self.registry("other")]))
        self.assertEqual(len(found), 1)
        self.assertIn("taffy 0.15.1", found[0])

    def test_generation_refuses_and_writes_nothing_when_a_text_lacks_its_copyright(self):
        """Both the writing run and `--check` exit 1 on a template text, and neither touches the committed file."""
        fixture = report(self.repo, ("MIT", "MIT License", TEMPLATE, [self.registry("a")]))
        for argv in (["--cargo-about", "/x"], ["--check", "--cargo-about", "/x"]):
            with self.subTest(argv[0]):
                run, _calls = self.fake_run([fixture, fixture])
                with redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()):
                    self.assertEqual(notices.main(argv, repo=self.repo, run=run), 1)
                self.assertFalse((self.repo / notices.OUTPUT).exists())

    def test_own_crates_are_recognized_under_a_path_that_needs_percent_encoding(self):
        """Cargo's package ids are URLs: a checkout path with a space must still mark Farhelm's own crates as own."""
        spaced = self.repo / "My Projects" / "farhelm"
        (spaced / "crates/farhelm-proto").mkdir(parents=True)
        encoded = str(spaced).replace(" ", "%20")
        own = crate("farhelm-proto", "0.0.0", f"path+file://{encoded}/crates/farhelm-proto#0.0.0")
        self.assertTrue(notices.is_own_crate(own["crate"], spaced))

    def test_the_llvm_exception_text_is_headed_as_what_it_is(self):
        """cargo-about files Apache-2.0 WITH LLVM-exception under plain Apache-2.0; the heading names the exception."""
        text = "Apache License\n--- LLVM Exceptions to the Apache 2.0 License ----\n"
        rendered = notices.render(notices.merge([report(
            self.repo, ("Apache-2.0", "Apache License 2.0", text, [self.registry("rustix")]),
        )], self.repo))
        self.assertIn("(Apache-2.0 WITH LLVM-exception)", rendered)

    def test_targets_and_binaries_match_what_a_release_builds(self):
        """A new release target or shipped binary must not leave its dependencies out while `--check` stays green.

        Reads the real repository (read-only): `about.toml`'s targets equal `dist-workspace.toml`'s, and `BINARIES` are
        exactly the workspace packages dist ships (those with a binary and not `dist = false`) plus dist's generic
        packages (`dist:` members).
        """
        about = tomllib.loads((REAL_REPO / notices.CONFIG).read_text(encoding="utf-8"))
        dist = tomllib.loads((REAL_REPO / "dist-workspace.toml").read_text(encoding="utf-8"))
        self.assertEqual(sorted(about["targets"]), sorted(dist["dist"]["targets"]))
        shipped = set()
        for member in dist["workspace"]["members"]:
            kind, _, where = member.partition(":")
            if kind == "dist":
                generic = tomllib.loads((REAL_REPO / where / "dist.toml").read_text(encoding="utf-8"))
                shipped.add(generic["package"]["name"])
        metadata = json.loads(subprocess.run(
            ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1"],
            cwd=REAL_REPO, check=True, capture_output=True, text=True,
        ).stdout)
        for package in metadata["packages"]:
            has_binary = any("bin" in target["kind"] for target in package["targets"])
            if has_binary and (package.get("metadata") or {}).get("dist", {}).get("dist", True) is not False:
                shipped.add(package["name"])
        names = {
            tomllib.loads((REAL_REPO / manifest).read_text(encoding="utf-8"))["package"]["name"]
            for manifest in notices.BINARIES
        }
        self.assertEqual(names, shipped)

    def test_generation_writes_the_rendered_file(self):
        """Without --check the file is rewritten from the reports."""
        fixture = report(self.repo, ("MIT", "MIT License", MIT, [self.registry("a")]))
        run, _calls = self.fake_run([fixture, fixture])
        with redirect_stdout(io.StringIO()):
            status = notices.main(["--cargo-about", "/fixture/cargo-about"], repo=self.repo, run=run)
        self.assertEqual(status, 0)
        self.assertEqual((self.repo / notices.OUTPUT).read_text(), notices.render(notices.merge([fixture], self.repo)))


if __name__ == "__main__":
    unittest.main()
