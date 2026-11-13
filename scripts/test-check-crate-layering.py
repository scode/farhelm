"""Fixtures for check-crate-layering.py: synthetic `cargo metadata` graphs and manifests, no cargo run.

The layering rules keep GUI code from calling helm internals and keep GPUI and GPUI Kit inside the app crate; the lint
rule keeps every GUI crate from unwrapping. These cases make sure each forbidden shape fails with the rule that names
it, and that the shapes the rules allow (dev-dependencies, the app crate's own GPUI and Kit use) pass.
"""
import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name("check-crate-layering.py")
spec = importlib.util.spec_from_file_location("layering", SCRIPT)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)

ROOT = pathlib.Path("/repo")
DENY = '[lints.clippy]\nunwrap_used = "deny"\nexpect_used = "deny"\n'


def metadata(members, deps, root=ROOT):
    """A minimal `cargo metadata` document: `members` maps name -> manifest dir, `deps` maps name -> [(dep, kind)]."""
    names = set(members) | {d for edges in deps.values() for d, _ in edges}
    packages = [
        {"id": n, "name": n, "manifest_path": str(members.get(n, root / "registry" / n) / "Cargo.toml")} for n in names
    ]
    nodes = [
        {"id": n, "deps": [{"pkg": d, "dep_kinds": [{"kind": kind}]} for d, kind in deps.get(n, [])]} for n in names
    ]
    return {"packages": packages, "workspace_members": sorted(members), "resolve": {"nodes": nodes}}


class CrateLayeringTest(unittest.TestCase):
    """GUI crates must never reach the helm; only the GPUI app crates may reach GPUI or GPUI Kit."""

    def test_a_transitive_helm_dependency_fails_and_names_the_chain(self):
        """A GUI crate reaching the helm through an intermediate crate is a violation, reported with its chain."""
        meta = metadata(
            {"farhelm-gui": ROOT / "crates/gui/farhelm-gui", "shim": ROOT / "crates/shim"},
            {"farhelm-gui": [("shim", None)], "shim": [("farhelm-helm", None)]},
        )
        problems = checker.violations(meta, ROOT / "crates/gui")
        self.assertEqual(len(problems), 1)
        self.assertIn("farhelm-gui -> shim -> farhelm-helm", problems[0])
        self.assertIn("only over its API", problems[0])

    def test_dev_dependencies_do_not_count(self):
        """A test that stands up a helm through a dev-dependency is allowed."""
        meta = metadata({"farhelm-gui": ROOT / "crates/gui/farhelm-gui"}, {"farhelm-gui": [("farhelm-helm", "dev")]})
        self.assertEqual(checker.violations(meta, ROOT / "crates/gui"), [])

    def test_framework_free_gui_crate_reaching_gpui_fails(self):
        """A crate under crates/gui/ that is not a GPUI app crate must not depend on gpui-pre, even through a build dep."""
        meta = metadata(
            {"farhelm-grid-model": ROOT / "crates/gui/farhelm-grid-model"},
            {"farhelm-grid-model": [("gpui-pre", "build")]},
        )
        problems = checker.violations(meta, ROOT / "crates/gui")
        self.assertEqual(len(problems), 1)
        self.assertIn("farhelm-grid-model reaches gpui-pre", problems[0])
        self.assertIn("only the GPUI app crates may depend on GPUI or GPUI Kit", problems[0])

    def test_framework_free_gui_crate_reaching_kit_fails(self):
        """GPUI Kit stays behind the app crate's component layer: a framework-free crate may not depend on it."""
        meta = metadata(
            {"farhelm-input-mapping": ROOT / "crates/gui/farhelm-input-mapping"},
            {"farhelm-input-mapping": [("gpui-component", None)]},
        )
        problems = checker.violations(meta, ROOT / "crates/gui")
        self.assertEqual(len(problems), 1)
        self.assertIn("reaches gpui-component", problems[0])

    def test_gpui_app_crate_may_use_gpui_and_kit_and_crates_outside_gui_are_unchecked(self):
        """farhelm-gui depends on GPUI and Kit by design, and the helm itself is outside the rules' scope."""
        meta = metadata(
            {"farhelm-gui": ROOT / "crates/gui/farhelm-gui", "farhelm-helm": ROOT / "crates/farhelm-helm"},
            {
                "farhelm-gui": [("gpui-pre", None), ("gpui-component", None), ("gpui-base", None)],
                "farhelm-helm": [("farhelm-supervisor", None)],
            },
        )
        self.assertEqual(checker.violations(meta, ROOT / "crates/gui"), [])


class GuiLintTableTest(unittest.TestCase):
    """Every GUI crate denies unwrap_used and expect_used, so no new crate silently escapes the rule."""

    def check(self, manifests, workspace=None):
        """Write each manifest under a temporary crates/gui/<name>/ and return the lint check's problems."""
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            members = {}
            for name, text in manifests.items():
                crate = root / "crates/gui" / name
                crate.mkdir(parents=True)
                (crate / "Cargo.toml").write_text(text)
                members[name] = crate
            return checker.lint_violations(metadata(members, {}, root), root / "crates/gui", workspace or {})

    def test_a_crate_without_the_table_fails(self):
        """A GUI crate with no [lints.clippy] is reported, naming both lints."""
        problems = self.check({"farhelm-grid-model": '[package]\nname = "farhelm-grid-model"\n'})
        self.assertEqual(len(problems), 1)
        self.assertIn("farhelm-grid-model does not deny unwrap_used, expect_used", problems[0])

    def test_a_crate_that_only_warns_fails(self):
        """`warn` is not enough: the rule needs the lint to fail the build."""
        problems = self.check({"a": '[lints.clippy]\nunwrap_used = "deny"\nexpect_used = { level = "warn" }\n'})
        self.assertEqual(len(problems), 1)
        self.assertIn("does not deny expect_used", problems[0])

    def test_direct_and_inherited_denies_pass(self):
        """A crate's own deny table passes, and so does `[lints] workspace = true` over a workspace table that denies."""
        workspace = {"workspace": {"lints": {"clippy": {"unwrap_used": "deny", "expect_used": {"level": "deny"}}}}}
        self.assertEqual(self.check({"a": DENY, "b": "[lints]\nworkspace = true\n"}, workspace), [])


class CargoFailureTest(unittest.TestCase):
    """A cargo metadata failure must fail the check with cargo's own explanation, not a Python traceback."""

    def test_metadata_failure_reports_cargo_stderr(self):
        """With cargo replaced by a stand-in that fails, the script exits 1 and passes the stand-in's stderr through."""
        with tempfile.TemporaryDirectory() as directory:
            fake = pathlib.Path(directory) / "cargo"
            fake.write_text('#!/bin/sh\necho "error: the lock file needs to be updated" >&2\nexit 101\n')
            fake.chmod(0o755)
            result = subprocess.run(
                [sys.executable, "-B", str(SCRIPT)],
                capture_output=True,
                text=True,
                env={"PATH": f"{directory}:/usr/bin:/bin"},
            )
        self.assertEqual(result.returncode, 1)
        self.assertIn("cargo metadata failed (exit 101)", result.stderr)
        self.assertIn("the lock file needs to be updated", result.stderr)


if __name__ == "__main__":
    unittest.main()
