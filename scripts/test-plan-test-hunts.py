#!/usr/bin/env python3
"""Verify changed-test planning without compiling or running a test substrate."""
import importlib.util
import contextlib
import io
import json
import pathlib
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock


spec = importlib.util.spec_from_file_location("planner", pathlib.Path(__file__).with_name("plan-test-hunts.py"))
planner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(planner)


def packages():
    """Model a support library consumed by an application and an independent UI package."""
    return {
        "support": {"path": "crates/support", "manifest": {}, "dependents": {"app"}},
        "app": {"path": "crates/app", "manifest": {}, "dependents": set()},
        "ui": {"path": "crates/ui", "manifest": {}, "dependents": set()},
    }


class SelectionTest(unittest.TestCase):
    """Shared changes widen visibly, while unmapped inputs never become claimed coverage."""

    def plan(self, paths, *, missing=()):
        """Treat named fixture files as present unless the scenario marks them deleted."""
        return planner.build_plan(paths, packages(), lambda path: path not in missing, 2, 60)

    def test_shared_rust_dependencies_and_browser_helpers_widen(self):
        """A support change reaches consumer tests; browser helper changes reach both engines."""
        result = self.plan(["crates/support/src/lib.rs", "e2e/tests/helpers/term.ts"])
        rust = [c for c in result["commands"] if c["runner"] == "nextest"]
        self.assertEqual([c["argv"][c["argv"].index("-p") + 1] for c in rust], ["app", "support"])
        browser = [c for c in result["commands"] if c["runner"] == "playwright"]
        self.assertEqual(len(browser), 1)
        self.assertEqual(browser[0]["argv"][-1], ".")
        self.assertEqual(result["maximum_sequential_child_command_seconds"], 360)
        self.assertTrue(all(c["reasons"] for c in result["commands"]))

    def test_test_targets_do_not_guess_rust_module_names(self):
        """Changes to an e2e fixture select its complete target without a fabricated filter."""
        result = self.plan(["crates/app/tests/e2e/harness.rs", "crates/app/tests/e2e/session.rs"])
        self.assertEqual(len(result["commands"]), 1)
        command = result["commands"][0]["argv"]
        self.assertEqual(command[-4:], ["-p", "app", "--test", "e2e"])
        self.assertNotIn("-E", command)
        broadened = self.plan(["crates/app/tests/e2e/harness.rs", "crates/app/src/main.rs"])
        self.assertEqual(len([c for c in broadened["commands"] if c["runner"] == "nextest"]), 1)
        self.assertNotIn("--test", broadened["commands"][0]["argv"])

    def test_deleted_and_ambiguous_targets_widen_instead_of_selecting_dead_tests(self):
        """Deleted tests and shared support paths cannot produce a misleading narrow command."""
        for path in ("crates/app/tests/removed.rs", "crates/app/tests/helpers.rs"):
            result = self.plan([path], missing=(path,))
            self.assertEqual(result["commands"][0]["argv"][-2:], ["-p", "app"])
        result = self.plan(["e2e/tests/removed.spec.ts"], missing=("e2e/tests/removed.spec.ts",))
        self.assertEqual(result["commands"][0]["argv"][-1], ".")

    def test_browser_filenames_are_literal_regexes_and_unknown_inputs_remain_visible(self):
        """Punctuation in filenames must not broaden a selector or make its regex invalid."""
        path = "e2e/tests/odd[one].spec.ts"
        result = self.plan([path, "scripts/new-check.py", "docs/notes.md"])
        self.assertEqual(result["commands"][0]["argv"][-1], r"tests/odd\[one\]\.spec\.ts$")
        self.assertEqual(result["manual_review_paths"], ["scripts/new-check.py"])
        self.assertEqual(result["omitted_paths"][0]["path"], "docs/notes.md")
        self.assertFalse(result["executed"])

    def test_workspace_policy_changes_and_budget_overflow_are_explicit(self):
        """Runner and substrate changes widen every package; a nonfinite budget is refused."""
        result = self.plan([".config/nextest.toml", ".github/release/source-pins.env"])
        self.assertEqual(len(result["commands"]), 4)
        with self.assertRaises(ValueError):
            planner.build_plan(["Cargo.toml"], packages(), lambda path: True, 1000, 1e308)

    def test_cross_package_path_helper_keeps_both_known_consumers(self):
        """The macro compile-contract test imports a helper without a Cargo dependency edge."""
        members = {name: {"path": "crates/" + name, "manifest": {}, "dependents": set()}
                   for name in ("farhelm-testtrace", "farhelm-testtrace-macros")}
        result = planner.build_plan(["crates/farhelm-testtrace/tests/support/process.rs"],
                                    members, lambda path: True, 2, 60)
        self.assertEqual([c["argv"][-1] for c in result["commands"]],
                         ["farhelm-testtrace", "farhelm-testtrace-macros"])
        self.assertEqual(result["manual_review_paths"], [])
        ambiguous = self.plan(["crates/app/tests/support/process.rs"],
                              missing=("crates/app/tests/support/main.rs",))
        self.assertEqual(ambiguous["manual_review_paths"], ["crates/app/tests/support/process.rs"])

    def test_browser_partitions_preserve_all_selectors_within_both_limits(self):
        """Count and byte limits create visible extra batches rather than unusable commands."""
        cases = (
            [f"e2e/tests/spec{number:03}.spec.ts" for number in range(126)],
            ["e2e/tests/" + "nested/" * 200 + f"spec{number}.spec.ts" for number in range(20)],
        )
        for paths in cases:
            with self.subTest(path_count=len(paths)):
                result = self.plan(paths)
                self.assertGreater(len(result["commands"]), 1)
                actual = []
                for command in result["commands"]:
                    child = command["argv"][command["argv"].index("--") + 1:]
                    actual += planner.test_run_playwright.selection_args(child)
                    self.assertTrue(any("partition" in reason for reason in command["reasons"]))
                self.assertEqual(actual, [planner.re.escape(p.removeprefix("e2e/")) + "$" for p in sorted(paths)])
                self.assertEqual(result["maximum_sequential_child_command_seconds"], 120 * len(result["commands"]))


class DiscoveryTest(unittest.TestCase):
    """Read actual Git changes and bounded manifests without relying on a build tool."""

    def test_workspace_aliases_target_dependencies_and_cycles(self):
        """Reverse closure includes renamed inherited and target-specific dependency edges."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            manifests = {
                "Cargo.toml": '[workspace]\nmembers=["crates/a", "crates/b", "crates/c"]\n'
                              '[workspace.dependencies]\nrenamed={package="a",path="crates/a"}\n',
                "crates/a/Cargo.toml": '[package]\nname="a"\n[dev-dependencies]\nc={path="../c"}\n',
                "crates/b/Cargo.toml": '[package]\nname="b"\n[dependencies]\nrenamed.workspace=true\n',
                "crates/c/Cargo.toml": '[package]\nname="c"\n[target.\'cfg(unix)\'.build-dependencies]\nb={path="../b"}\n',
            }
            for name, data in manifests.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(data)
            found = planner.workspace(root, lambda path, limit: path.read_bytes())
            self.assertEqual(planner.dependent_closure({"a"}, found), {"a", "b", "c"})
            (root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/*"]\n')
            with self.assertRaises(ValueError):
                planner.workspace(root, lambda path, limit: path.read_bytes())

    def test_actual_git_discovery_keeps_deletions_staged_unstaged_and_untracked_paths(self):
        """Working changes supplement the chosen base, including both sides of a rename."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)

            def git(*args):
                """Run only local fixture VCS operations with an explicit disposable identity."""
                return subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                                       "-c", "commit.gpgsign=false", *args], cwd=root, check=True,
                                      capture_output=True, timeout=10)

            git("init", "-q")
            for name in ("deleted.rs", "staged.rs", "unstaged.rs"):
                (root / name).write_text("initial")
            git("add", ".")
            git("commit", "-qm", "fixture")
            (root / "deleted.rs").rename(root / "renamed.rs")
            (root / "staged.rs").write_text("changed")
            git("add", "staged.rs")
            (root / "unstaged.rs").write_text("changed")
            (root / "new file.rs").write_text("new")
            recorder = planner.test_hunt.load_recorder()
            actual_root, base, head, paths = planner.discover_changes(root, "HEAD", recorder)
            self.assertEqual(actual_root, root)
            self.assertEqual(base, head)
            self.assertEqual(paths, ["deleted.rs", "new file.rs", "renamed.rs", "staged.rs", "unstaged.rs"])
            with self.assertRaises(ValueError):
                planner.discover_changes(root, "--help", recorder)

    def test_partial_git_output_cannot_become_a_partial_plan(self):
        """Truncated or failed metadata observations are refused before path interpretation."""
        for complete, truncated in ((False, False), (True, True)):
            fake = SimpleNamespace(SignalIntent=lambda: None, bounded_probe=lambda *args, **kwargs:
                                   SimpleNamespace(complete=complete, stdout_sample_truncated=truncated,
                                                   stdout_sample=b"some.rs\0"))
            with self.assertRaises(ValueError):
                planner.git_output(fake, pathlib.Path("."), ["diff"])

    def test_cli_planning_does_not_create_bytecode_or_evidence(self):
        """A normal interpreter invocation leaves the measured fixture checkout unchanged."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            scripts = root / "scripts"
            scripts.mkdir()
            for name in ("plan-test-hunts.py", "test_hunt.py", "record-test-run.py", "test_run_nextest.py",
                         "test_run_playwright.py", "test_run_traces.py"):
                shutil.copyfile(pathlib.Path(__file__).with_name(name), scripts / name)
            (root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/app"]\n')
            package = root / "crates/app"
            package.mkdir(parents=True)
            (package / "Cargo.toml").write_text('[package]\nname="app"\n')
            for command in (["init", "-q"], ["add", "."], ["commit", "-qm", "fixture"]):
                subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                                "-c", "commit.gpgsign=false", *command], cwd=root, check=True,
                               capture_output=True, timeout=10)
            result = subprocess.run([sys.executable, str(scripts / "plan-test-hunts.py"),
                                     "--base", "HEAD", "--repeat", "2", "--timeout", "60"],
                                    cwd=root, capture_output=True, text=True, timeout=20)
            self.assertEqual(result.returncode, 0, result.stderr)
            plan = json.loads(result.stdout)
            self.assertEqual(plan["paths"], [])
            self.assertEqual(plan["commands"], [])
            self.assertEqual(list(root.rglob("__pycache__")), [])
            status = subprocess.run(["git", "status", "--porcelain"], cwd=root, capture_output=True,
                                    text=True, check=True, timeout=10)
            self.assertEqual(status.stdout, "")

    def test_cancellation_preserves_signal_status_and_restores_handlers(self):
        """A cancelled metadata probe must stop planning and leave no process-wide handler behind."""
        before = {number: signal.getsignal(number) for number in (signal.SIGINT, signal.SIGTERM)}

        def cancelled(*args, intent, **kwargs):
            """Model the bounded probe returning after processing shared cancellation."""
            intent.handle(signal.SIGTERM, None)
            raise ValueError("cancelled metadata")

        with mock.patch.object(planner, "discover_changes", cancelled), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(planner.main(["--base", "HEAD", "--repeat", "2", "--timeout", "60"]), 143)
        self.assertEqual({number: signal.getsignal(number) for number in before}, before)

    def test_cancellation_during_final_output_is_not_success(self):
        """The publication boundary still belongs to the temporary cancellation handler."""
        before = {number: signal.getsignal(number) for number in (signal.SIGINT, signal.SIGTERM)}
        encode = json.dumps

        def cancelling_encode(*args, **kwargs):
            """Deliver SIGTERM intent after the pre-output check but before output completes."""
            result = encode(*args, **kwargs)
            signal.getsignal(signal.SIGTERM)(signal.SIGTERM, None)
            return result

        discovered = (pathlib.Path.cwd(), "a" * 40, "a" * 40, [])
        with mock.patch.object(planner, "discover_changes", return_value=discovered), \
                mock.patch.object(planner, "workspace", return_value=packages()), \
                mock.patch.object(planner.json, "dumps", cancelling_encode), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(planner.main(["--base", "HEAD", "--repeat", "2", "--timeout", "60"]), 143)
        self.assertEqual({number: signal.getsignal(number) for number in before}, before)


if __name__ == "__main__":
    unittest.main()
