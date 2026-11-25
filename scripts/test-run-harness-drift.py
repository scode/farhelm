#!/usr/bin/env python3
"""Exercise the manual drift runner using stand-ins. No test starts a vendor CLI."""

import contextlib
import io
import json
from pathlib import Path
import re
import runpy
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

from agent_screen_host import OUTER_SESSION_MARKERS
import harness_upgrades

runner = runpy.run_path(str(Path(__file__).with_name("run-harness-drift.py")))
# runpy returns a copy of the globals. Patch the actual function namespace so main uses the stand-ins.
globals_of_runner = runner["main"].__globals__
DRIFT_SOURCE = Path(__file__).resolve().parents[1] / "crates/farhelm/tests/e2e/real_agent_capture/drift"


def no_upgrades(names, env, upgrade):
    """Stand in for the upgrade phase in runner tests that are about verdicts, so no vendor updater ever runs."""
    return {name: harness_upgrades.Upgrade("current", "1.0", "1.0") for name in names}


class DriftRunnerTests(unittest.TestCase):
    def test_build_and_discovery_use_the_single_cargo_artifact_and_only_ignored_drift_tests(self):
        """A custom target directory must work and unrelated ignored live tests must never run."""
        artifact = {
            "reason": "compiler-artifact",
            "target": {"name": "e2e", "kind": ["test"]},
            "executable": "/synthetic/custom/target/debug/deps/e2e-test",
        }
        listing = (
            "real_agent_capture::real_claude_session_reports_its_identity_across_clear: test\n"
            "real_agent_capture::drift::tests::a_drift_unit_test: test\n"
            "real_agent_capture::drift::base::drift_codex: test\n"
            "real_agent_capture::drift::base::drift_antigravity: test\n"
            "real_agent_capture::drift::model_selection::drift_codex: test\n\n"
            "5 tests, 0 benchmarks\n"
        )
        outputs = [
            subprocess.CompletedProcess([], 0, json.dumps(artifact)),
            subprocess.CompletedProcess([], 0, listing),
        ]
        env = {"CARGO_TARGET_DIR": "/synthetic/custom/target"}
        with patch.object(subprocess, "run", side_effect=outputs) as run:
            binary = runner["build_binary"](env)
            cases = runner["discover"](binary, env)
        self.assertEqual(list(cases), [("base", "antigravity"), ("base", "codex"), ("model_selection", "codex")])
        self.assertEqual(cases["base", "codex"], "real_agent_capture::drift::base::drift_codex")
        self.assertEqual(cases["model_selection", "codex"], "real_agent_capture::drift::model_selection::drift_codex")
        self.assertIn("--no-run", run.call_args_list[0].args[0])
        self.assertEqual(run.call_args_list[1].args[0], [str(binary), "--ignored", "--list"])
        self.assertEqual(run.call_count, 2)

    def test_child_environment_keeps_login_without_nested_agent_and_farhelm_state(self):
        """Keep credentials in child env while dropping every outer-session marker."""
        outer = dict.fromkeys((*OUTER_SESSION_MARKERS, "TMUX", "TMUX_PANE", "FARHELM_AGENT_ID"), "outer")
        auth = {"CODEX_HOME": "vendor-home", "OPENROUTER_API_KEY": "stand-in", "PATH": "bin"}
        environment = {**outer, **auth}
        self.assertEqual(runner["child_environment"](environment), auth)
        self.assertEqual(environment, {**outer, **auth})

    def test_missing_invalid_and_inconsistent_verdicts_fail(self):
        """A passing libtest or a stale success record alone cannot establish coverage."""
        read = runner["read_verdict"]
        with tempfile.TemporaryDirectory() as directory:
            verdict = Path(directory) / "verdict.json"
            self.assertEqual(read(verdict, 0).verdict, "failed")
            for payload in [[], {"verdict": "other", "detail": "text"}, {"verdict": "passed", "detail": ""}]:
                verdict.write_text(json.dumps(payload))
                self.assertEqual(read(verdict, 0).verdict, "failed")
            verdict.write_text(json.dumps({"verdict": "could_not_run", "detail": "no login"}))
            self.assertEqual(read(verdict, 0).verdict, "could_not_run")
            self.assertEqual(read(verdict, 1).verdict, "failed")

    def test_missing_verdict_keeps_the_recorders_known_failure_reason(self):
        """The recorder can know why a case ended before Rust could write its result."""
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            manifest = evidence / "manifest.json"
            verdict = evidence / "missing.json"
            manifest.write_text(json.dumps({"schema_version": 1, "outcome": "timed_out", "recorder": {"error": None}}))
            result = runner["read_verdict"](verdict, 124, evidence)
            self.assertEqual(result.verdict, "failed")
            self.assertIn("timed out", result.detail)
            manifest.write_text(json.dumps({
                "schema_version": 1, "outcome": "recorder-error", "recorder": {"error": "fixture refused"},
            }))
            result = runner["read_verdict"](verdict, 125, evidence)
            self.assertEqual(result.verdict, "failed")
            self.assertIn("fixture refused", result.detail)
            for version in [2, True, 1.0]:
                manifest.write_text(json.dumps({"schema_version": version, "outcome": "timed_out"}))
                result = runner["read_verdict"](verdict, 124, evidence)
                self.assertIn("without a readable drift verdict", result.detail)

    def test_every_selected_case_runs_and_only_an_explicit_skip_fails(self):
        """Missing prerequisites are distinguishable and do not prevent another harness from running."""
        cases = {
            ("base", "claude"): "case-claude",
            ("base", "codex"): "case-codex",
            ("base", "grok"): "case-grok",
            ("model_selection", "codex"): "case-model",
        }
        result = runner["Result"]
        for requested, verdicts, code in [
            ([], [result("could_not_run", "no login"), result("passed", "ok"), result("passed", "ok")], 0),
            ([], [result("failed", "no report"), result("passed", "ok"), result("could_not_run", "no hook")], 1),
            (["--harness", "grok", "--harness", "codex"], [result("could_not_run", "no hook"), result("passed", "ok")], 1),
        ]:
            called = []

            def recorded(binary, test, name, env):
                called.append(name)
                return verdicts[len(called) - 1]

            with (
                patch.dict(globals_of_runner, {
                    "build_binary": lambda env: Path("/synthetic/e2e"),
                    "discover": lambda binary, env: cases,
                    "recorded_case": recorded,
                    "prepare": no_upgrades,
                }),
                contextlib.redirect_stdout(io.StringIO()) as output,
            ):
                self.assertEqual(runner["main"](requested), code)
            self.assertEqual(called, ["base/grok", "base/codex"] if requested else ["base/claude", "base/codex", "base/grok"])
            self.assertIn("COULD NOT RUN", output.getvalue())
            self.assertIn("PASSED", output.getvalue())

    def test_list_discovers_but_never_launches(self):
        """Listing the suite must spend zero vendor turns."""
        def unexpected(*args):
            self.fail("listing must never run a case")

        with patch.dict(globals_of_runner, {
            "build_binary": lambda env: Path("/synthetic/e2e"),
            "discover": lambda binary, env: {("base", "claude"): "case-claude", ("model_selection", "codex"): "case-model"},
            "recorded_case": unexpected,
            "prepare": unexpected,
        }), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(runner["main"](["--list", "--category", "model_selection", "--harness", "codex"]), 0)
        self.assertEqual(output.getvalue().strip(), "model_selection/codex")

    def test_category_selection_deduplicates_upgrades_but_retains_every_verdict(self):
        """A pass in one category cannot overwrite the same harness's failure in another."""
        cases = {("base", "codex"): "base-case", ("model_selection", "codex"): "model-case"}
        for filters in [
            ["--category", "all"],
            ["--category", "base", "--category", "model_selection", "--category", "base", "--harness", "codex"],
        ]:
            calls = []

            def prepare(names, env, upgrade):
                calls.append(("upgrade", names))
                return no_upgrades(names, env, upgrade)

            def recorded(binary, test, name, env):
                calls.append(("case", test))
                if test == "base-case":
                    return runner["Result"]("failed", "base failure")
                return runner["Result"]("passed", "model success")

            with patch.dict(globals_of_runner, {
                "build_binary": lambda env: Path("/synthetic/e2e"),
                "discover": lambda binary, env: cases,
                "prepare": prepare,
                "recorded_case": recorded,
            }), contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(runner["main"](filters), 1)
            self.assertEqual(calls, [("upgrade", ["codex"]), ("case", "base-case"), ("case", "model-case")])
            self.assertRegex(output.getvalue(), r"FAILED +base +codex +1\.0 +base failure")
            self.assertRegex(output.getvalue(), r"PASSED +model_selection +codex +1\.0 +model success")

    def test_invalid_category_combinations_run_no_upgrades_or_cases(self):
        """Upgrade-only needs no build, and a category without a selected harness cannot report success."""
        def unexpected(*args, **kwargs):
            self.fail("the rejected selection must not start work")

        cases = {("base", "grok"): "base-grok", ("model_selection", "codex"): "model-codex"}
        for argv, diagnostic, builds in [
            (["--upgrade-only", "--category", "base"], "--category cannot be used with --upgrade-only", False),
            (["--list", "--upgrade-only"], "--list cannot be used with --upgrade-only", False),
            (["--category", "model_selection", "--harness", "grok"], "available harnesses: codex", True),
            (["--category", "missing"], "unknown category: missing", True),
        ]:
            with patch.dict(globals_of_runner, {
                "build_binary": (lambda env: Path("/synthetic/e2e")) if builds else unexpected,
                "discover": lambda binary, env: cases,
                "prepare": unexpected,
                "recorded_case": unexpected,
            }), contextlib.redirect_stderr(io.StringIO()) as errors, self.assertRaises(SystemExit) as exited:
                runner["main"](argv)
            self.assertEqual(exited.exception.code, 2)
            self.assertIn(diagnostic, errors.getvalue())

    def test_retains_the_structured_verdict_with_the_recorder_files(self):
        """Give the recorder time for both turns and retain the verdict beside its manifest."""
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            (evidence / "manifest.json").write_text("{}")

            class Recorder:
                """A recorder that announces its directory before the child's own output."""
                def __init__(self, command, **kwargs):
                    self.stderr = io.StringIO(f"test-run evidence: {evidence}\n")
                    self.command = command
                    verdict_file = Path(kwargs["env"]["FARHELM_DRIFT_VERDICT_FILE"])
                    if verdict_file.exists():
                        raise AssertionError("the caller's verdict must not be reused")
                    verdict_file.write_text(json.dumps({"verdict": "could_not_run", "detail": "login missing"}))

                def __enter__(self):
                    return self

                def __exit__(self, *_):
                    self.stderr.close()

                def wait(self):
                    return 0

            with patch.object(subprocess, "Popen", side_effect=Recorder) as spawn, contextlib.redirect_stderr(io.StringIO()):
                result = runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
            command = spawn.call_args.args[0]
            self.assertEqual(command[-7:], ["/synthetic/e2e", "--ignored", "--exact", "exact-case", "--test-threads", "1", "--nocapture"])
            self.assertEqual(command[command.index("--timeout") + 1], "720")
            self.assertIn("FARHELM_DRIFT_VERDICT_FILE", command)
            self.assertEqual(result.verdict, "could_not_run")
            self.assertEqual(result.evidence, evidence)
            self.assertEqual(json.loads((evidence / "drift-verdict.json").read_text())["detail"], "login missing")

    def test_runner_only_termination_reaches_the_recorder(self):
        """The recorder owns the test process group and must finish cleanup before the runner exits."""
        prior_handler = signal.getsignal(signal.SIGTERM)
        with patch.object(subprocess, "Popen") as spawn:
            recorder = spawn.return_value.__enter__.return_value

            def cancel(*_args, **_kwargs):
                signal.raise_signal(signal.SIGTERM)
                return spawn.return_value

            spawn.side_effect = cancel
            recorder.wait.return_value = 128 + signal.SIGTERM
            with self.assertRaises(SystemExit) as stopped:
                runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
        recorder.send_signal.assert_called_once_with(signal.SIGTERM)
        recorder.wait.assert_called_once()
        self.assertEqual(stopped.exception.code, 128 + signal.SIGTERM)
        self.assertEqual(signal.getsignal(signal.SIGTERM), prior_handler)

    def test_termination_during_a_failed_spawn_stops_the_suite(self):
        """A spawn error may fail one case, but a simultaneous cancellation ends the run."""
        def failed_spawn(*_args, **_kwargs):
            signal.raise_signal(signal.SIGTERM)
            raise OSError("fixture spawn failed")

        with patch.object(subprocess, "Popen", side_effect=failed_spawn):
            with self.assertRaises(SystemExit) as stopped:
                runner["recorded_case"](Path("/synthetic/e2e"), "exact-case", "grok", {"PATH": "bin"})
        self.assertEqual(stopped.exception.code, 128 + signal.SIGTERM)


class UpgradePhaseTests(unittest.TestCase):
    """The runner brings each harness to its latest release before testing it, and says so when it could not."""

    def test_every_drift_case_has_a_recipe_for_the_binary_it_launches(self):
        """A new drift case without a recipe would quietly test a stale CLI; a renamed binary would upgrade nothing."""
        source = (DRIFT_SOURCE / "mod.rs").read_text()
        for name, recipe in harness_upgrades.RECIPES.items():
            self.assertIn(f'"{recipe.binary}"', source, name)
        categories = {
            path.name: re.findall(r"^\s+drift_([a-z][a-z0-9_]*) =>", path.read_text(), re.MULTILINE)
            for path in DRIFT_SOURCE.glob("*.rs")
        }
        self.assertEqual(sorted(categories["base.rs"]), sorted(harness_upgrades.RECIPES))
        for category, registered in categories.items():
            self.assertLessEqual(set(registered), harness_upgrades.RECIPES.keys(), category)

    def stand_in(self, directory, body):
        """Write an executable `fake` CLI whose `--version` prints the file `version` beside it."""
        path = Path(directory) / "fake"
        path.write_text(
            "#!/bin/sh\n"
            f'here="{directory}"\n'
            'if [ "$1" = --version ]; then echo "$QUIET" >> "$here/quiet"; cat "$here/version"; exit 0; fi\n'
            f"{body}\n"
        )
        path.chmod(0o755)
        (Path(directory) / "version").write_text("fake 1.0\n")
        return {"PATH": f"{directory}:/usr/bin:/bin"}

    def test_upgrade_reports_versions_and_names_the_updaters_reason_for_failing(self):
        """Upgraded, already current and failed are told apart by the updater's exit and the versions around it."""
        recipe = harness_upgrades.Recipe("fake", ("fake", "update"))
        with tempfile.TemporaryDirectory() as directory:
            env = self.stand_in(directory, f'echo "fake 2.0" > "{directory}/version"; echo done')
            self.assertEqual(harness_upgrades.upgrade(recipe, env), harness_upgrades.Upgrade("upgraded", "fake 1.0", "fake 2.0"))
            self.assertEqual(harness_upgrades.upgrade(recipe, env).status, "current")
        with tempfile.TemporaryDirectory() as directory:
            env = self.stand_in(directory, "echo checking; echo 'Error: [unauthenticated]'; exit 1")
            result = harness_upgrades.upgrade(recipe, env)
            self.assertEqual(result, harness_upgrades.Upgrade("failed", "fake 1.0", "fake 1.0", "Error: [unauthenticated]"))
        self.assertEqual(harness_upgrades.upgrade(recipe, {"PATH": "/nonexistent"}).status, "not_installed")

    def test_updaters_get_no_terminal_input_and_the_quiet_environment_reaches_every_probe(self):
        """An updater that asks a question must fail, not hang, and a launcher must not update during a probe."""
        recipe = harness_upgrades.Recipe(
            "fake", ("fake", "update"), quiet_env={"QUIET": "yes"}, upgrade_env={"QUIET": "upgrading"},
        )
        with tempfile.TemporaryDirectory() as directory:
            env = self.stand_in(directory, 'echo "$QUIET" > "$here/updater"; read answer || { echo no input; exit 1; }')
            result = harness_upgrades.upgrade(recipe, env)
            self.assertEqual((result.status, result.detail), ("failed", "no input"))
            self.assertEqual((Path(directory) / "updater").read_text(), "upgrading\n")
            self.assertEqual((Path(directory) / "quiet").read_text(), "yes\nyes\n")

    def test_the_installer_runs_only_after_the_updater_fails(self):
        """Cursor's and goose's updaters broke while their installers worked; a working updater is never bypassed."""
        recipe = harness_upgrades.Recipe(
            "fake", ("fake", "update"), installer_url="https://example.invalid/install", installer_env={"CONFIGURE": "false"},
        )
        installs = []

        def installer(url, env):
            installs.append((url, env.get("CONFIGURE")))
            return True, ""

        with tempfile.TemporaryDirectory() as directory, patch.object(harness_upgrades, "run_installer", installer):
            env = self.stand_in(directory, "exit 0")
            self.assertEqual(harness_upgrades.upgrade(recipe, env).status, "current")
            self.assertEqual(installs, [])
            env = self.stand_in(directory, "echo updater broke; exit 1")
            result = harness_upgrades.upgrade(recipe, env)
            self.assertEqual(result.status, "current")
            self.assertIn("updater broke", harness_upgrades.describe("fake", result))
            self.assertEqual(installs, [("https://example.invalid/install", "false")])

    def test_installer_redirects_require_https_at_each_hop(self):
        """Refuse a non-TLS redirect before a downloader can connect to its target."""
        source = urllib.request.Request("https://example.invalid/install")
        handler = harness_upgrades.InstallerRedirects()
        target = "https://download.invalid/install"
        self.assertEqual(handler.redirect_request(source, None, 302, "Found", {}, target).full_url, target)
        for scheme in ("http", "ftp"):
            with self.subTest(scheme=scheme), self.assertRaisesRegex(urllib.error.URLError, "must use HTTPS"):
                handler.redirect_request(source, None, 302, "Found", {}, f"{scheme}://download.invalid/install")

    def test_installer_checks_the_final_url_before_reading_bytes(self):
        """A response outside HTTPS must never supply bytes to the fallback shell."""
        with patch.object(urllib.request, "build_opener") as build, patch.object(harness_upgrades, "run_updater") as run:
            response = build.return_value.open.return_value.__enter__.return_value
            response.geturl.return_value = "http://download.invalid/install"
            ok, detail = harness_upgrades.run_installer("https://example.invalid/install", {})
            self.assertFalse(ok)
            self.assertIn("must use HTTPS", detail)
            self.assertIsInstance(build.call_args.args[0], harness_upgrades.InstallerRedirects)
            response.read.assert_not_called()
            run.assert_not_called()

    def run_main(self, argv, upgrade_result=None):
        """Run the runner's main against stand-ins, returning its exit code, output and what it upgraded or probed."""
        calls = []
        upgrade_result = upgrade_result or harness_upgrades.Upgrade("current", "1.0", "1.0")

        def upgrade(recipe, env):
            calls.append(("upgrade", recipe.binary))
            return upgrade_result

        def probe(recipe, env):
            calls.append(("probe", recipe.binary))
            return harness_upgrades.Upgrade("skipped", "1.0", "1.0")

        with (
            patch.object(harness_upgrades, "upgrade", upgrade),
            patch.object(harness_upgrades, "probe", probe),
            patch.dict(globals_of_runner, {
                "build_binary": lambda env: calls.append(("build",)) or Path("/synthetic/e2e"),
                "discover": lambda binary, env: {("base", "claude"): "case-claude", ("base", "newcomer"): "case-newcomer"},
                "recorded_case": lambda binary, test, name, env: calls.append(("case", name)) or runner["Result"]("passed", "ok"),
            }),
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            code = runner["main"](argv)
        return code, output.getvalue(), calls

    def test_cases_run_after_their_upgrade_and_a_failed_upgrade_fails_the_run(self):
        """A pass on a CLI that could not be upgraded is not the evidence the suite exists to give."""
        code, output, calls = self.run_main(["--harness", "claude"])
        self.assertEqual((code, calls), (0, [("build",), ("upgrade", "claude"), ("case", "base/claude")]))
        self.assertRegex(output, r"PASSED +base +claude +1\.0")
        failed = harness_upgrades.Upgrade("failed", "1.0", "1.0", "offline")
        code, output, calls = self.run_main(["--harness", "claude"], failed)
        self.assertEqual((code, calls[-1]), (1, ("case", "base/claude")))
        self.assertIn("UPGRADE FAILED claude", output)
        self.assertIn("UPGRADE FAILED for claude", output)

    def test_a_case_without_a_recipe_is_reported_as_a_failed_upgrade(self):
        """Adding a harness to the suite must come with a decision about how it gets upgraded."""
        code, output, calls = self.run_main(["--harness", "newcomer"])
        self.assertEqual((code, calls), (1, [("build",), ("case", "base/newcomer")]))
        self.assertIn("no recipe in scripts/harness_upgrades.py", output)

    def test_no_upgrade_probes_versions_and_upgrade_only_builds_nothing(self):
        """Reproducing a failure needs the same version; upgrading alone needs neither the build nor a vendor turn."""
        code, _, calls = self.run_main(["--harness", "claude", "--no-upgrade"])
        self.assertEqual((code, calls), (0, [("build",), ("probe", "claude"), ("case", "base/claude")]))
        code, _, calls = self.run_main(["--upgrade-only", "--harness", "codex"])
        self.assertEqual((code, calls), (0, [("upgrade", "codex")]))
        code, _, calls = self.run_main(["--upgrade-only"])
        self.assertEqual(calls, [("upgrade", recipe.binary) for recipe in harness_upgrades.RECIPES.values()])


if __name__ == "__main__":
    unittest.main()
