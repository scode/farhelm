#!/usr/bin/env python3
"""Exercise the browser report adapter with public, synthetic JSON evidence."""
import hashlib
import json
import pathlib
import tempfile
import unittest
from types import SimpleNamespace

import test_run_playwright as adapter


def report(output, *, status="expected", expected="passed", results=None, version="1.62.0", errors=None):
    """Build a minimal pinned-runner report with one independently identified case per engine."""
    projects = [{
        "id": f"{engine}-x", "name": f"{engine}-x",
        "retries": 0, "repeatEach": 1, "outputDir": str(output),
    } for engine in ("chromium", "webkit")]
    specs = []
    for project in projects:
        actual = results if results is not None else [{"status": "passed", "retry": 0}]
        specs.append({
            "id": project["id"], "file": "fixture.spec.ts", "title": "fixture case",
            "tests": [{
                "projectId": project["id"], "expectedStatus": expected,
                "status": status, "results": actual,
            }],
        })
    return {
        "config": {
            "version": version, "workers": 1, "forbidOnly": True,
            "failOnFlakyTests": True, "shard": None, "projects": projects,
        },
        "suites": [{"specs": specs}],
        "errors": [] if errors is None else errors,
        "stats": {name: 2 if status == name else 0
                  for name in ("expected", "unexpected", "skipped", "flaky")},
    }


def policy_report(output):
    """Supply resolved engines separately, as Playwright's supplementary reporter does."""
    return {
        "schema_version": 1, "completed": True, "status": "passed",
        "workers": 1, "forbidOnly": True, "failOnFlakyTests": True,
        "projects": [{
            "name": f"{engine}-x", "engine": engine,
            "retries": 0, "repeatEach": 1, "outputDir": str(output),
        } for engine in ("chromium", "webkit")],
    }


class PlaywrightReportTest(unittest.TestCase):
    """Reject incomplete evidence while preserving actual and expected outcomes."""

    def collect(self, data, policy=None, policy_mutation=None):
        """Publish synthetic fixed reports under a fresh private output directory."""
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            for project in data["config"]["projects"]:
                project["outputDir"] = str(root / adapter.ARTIFACTS)
            (root / adapter.REPORT).write_text(json.dumps(data))
            policy = policy if policy is not None else policy_report(root / adapter.ARTIFACTS)
            if policy_mutation:
                policy_mutation(policy)
            (root / adapter.POLICY).write_text(json.dumps(policy))
            return adapter.collect(root)

    def test_supplementary_policy_must_match_resolved_main_policy(self):
        """A second report cannot silently change the engine, budget, or output owner."""
        mutations = (
            lambda p: p.update(workers=True),
            lambda p: p.update(schema_version=True),
            lambda p: p.update(completed=False),
            lambda p: p.update(status=[]),
            lambda p: p.update(forbidOnly=False),
            lambda p: p.update(failOnFlakyTests=False),
            lambda p: p["projects"].append(p["projects"][0].copy()),
            lambda p: p["projects"][0].update(engine="firefox"),
            lambda p: p["projects"][0].update(name=[]),
            lambda p: p["projects"][0].update(retries=False),
            lambda p: p["projects"][0].update(repeatEach=2),
            lambda p: p["projects"][0].update(outputDir="foreign"),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                result = self.collect(report(pathlib.Path("unused")), policy_mutation=mutate)
                self.assertFalse(result["complete"], result)
                self.assertNotIn("counts", result)
        result = self.collect(report(pathlib.Path("unused")),
                              policy_mutation=lambda p: p["projects"][0].update(secret="private"))
        self.assertTrue(result["complete"], result)
        self.assertNotIn("private", json.dumps(result))

    def test_selection_keeps_only_file_and_grep_inputs(self):
        """Selection flags cannot override runner policy or terminate option validation."""
        self.assertEqual(adapter.selection_args(["npx", "playwright", "test", "x.spec.ts", "-g", "title"]), ["x.spec.ts", "-g", "title"])
        for value in (["npx", "playwright", "test", "--project=x"], ["npx", "playwright", "test", "--", "x"], ["npx", "playwright", "test", "-g"]):
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    adapter.selection_args(value)

    def test_valid_both_engine_report_counts_actual_and_outcomes(self):
        """Both engine buckets account for the actual results in a valid report."""
        with tempfile.TemporaryDirectory() as tmp:
            data = report(pathlib.Path(tmp) / adapter.ARTIFACTS)
            result = self.collect(data)
        self.assertTrue(result["complete"], result)
        self.assertEqual(result["counts"]["passed"], 2)
        self.assertEqual(set(result["engines"]), {"chromium", "webkit"})

    def test_missing_malformed_linked_and_policy_mismatch_stay_incomplete(self):
        """Missing, unparsable and linked reports cannot establish an empty denominator."""
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            self.assertFalse(adapter.collect(root)["complete"])
            (root / adapter.REPORT).write_text("{")
            (root / adapter.POLICY).write_text("{}")
            self.assertFalse(adapter.collect(root)["complete"])
            (root / adapter.REPORT).unlink()
            target = root / "real"
            target.write_text("{}")
            (root / adapter.REPORT).symlink_to(target)
            self.assertFalse(adapter.collect(root)["complete"])

    def test_no_result_is_not_an_actual_skip(self):
        """An unstarted case has no actual skip result even when its outcome says skipped."""
        with tempfile.TemporaryDirectory() as tmp:
            data = report(pathlib.Path(tmp) / adapter.ARTIFACTS, status="skipped", expected="skipped", results=[])
            result = self.collect(data)
        self.assertTrue(result["complete"], result)
        self.assertEqual(result["counts"]["not_run"], 2)
        self.assertEqual(result["counts"]["skipped"], 0)

    def test_rejects_duplicate_and_contradictory_data(self):
        """Duplicate project/spec identity and boolean totals cannot inflate coverage."""
        with tempfile.TemporaryDirectory() as tmp:
            data = report(pathlib.Path(tmp) / adapter.ARTIFACTS)
            data["suites"][0]["specs"][1]["id"] = data["suites"][0]["specs"][0]["id"]
            data["suites"][0]["specs"][1]["tests"][0]["projectId"] = "chromium-x"
            self.assertFalse(self.collect(data)["complete"])
            data = report(pathlib.Path(tmp) / adapter.ARTIFACTS)
            data["stats"]["expected"] = True
            self.assertFalse(self.collect(data)["complete"])

    def test_actual_outcomes_preserve_expected_failure_and_interruption(self):
        """Expected failure is executed coverage; interruption remains distinguishable from skip."""
        cases = (
            ("failed", "failed", "expected", "failed", 2),
            ("failed", "passed", "unexpected", "failed", 0),
            ("skipped", "skipped", "skipped", "skipped", 0),
            ("interrupted", "passed", "skipped", "interrupted", 0),
            ("timedOut", "passed", "unexpected", "timed_out", 0),
        )
        for actual, expected, outcome, counted, failures in cases:
            with self.subTest(actual=actual, expected=expected):
                data = report(pathlib.Path("unused"), status=outcome, expected=expected,
                              results=[{"status": actual, "retry": 0}])
                result = self.collect(data)
                self.assertTrue(result["complete"], result)
                self.assertEqual(result["counts"][counted], 2)
                self.assertEqual(result["counts"]["expected_failures"], failures)

    def test_synthetic_skips_are_unstarted_cases(self):
        """Setup and serial blocking must not masquerade as intentional test skips."""
        for expected in ("passed", "failed", "skipped"):
            with self.subTest(expected=expected):
                result = self.collect(report(pathlib.Path("unused"), status="skipped", expected=expected,
                                             results=[{"status": "skipped", "retry": 0}]))
                self.assertTrue(result["complete"], result)
                for bucket, size in ((result["counts"], 2), *[(b, 1) for b in result["engines"].values()]):
                    self.assertEqual(bucket["not_run"], size if expected != "skipped" else 0)
                    self.assertEqual(bucket["skipped"], size if expected == "skipped" else 0)
                    self.assertEqual(bucket["outcome_skipped"], size)

    def test_malformed_structure_and_values_never_escape_or_export_counts(self):
        """Compound values and missing arrays are incomplete, never crashes or empty passes."""
        mutations = (
            lambda d: d.update(suites=None),
            lambda d: d.pop("suites"),
            lambda d: d.update(errors=None),
            lambda d: d.update(errors=["not an error object"]),
            lambda d: d["suites"][0].update(specs=None),
            lambda d: d["suites"][0].update(suites={}),
            lambda d: d["suites"][0]["specs"][0].update(tests=None),
            lambda d: d["suites"][0]["specs"][0].update(title="x" * (adapter.TEXT_LIMIT + 1)),
            lambda d: d["config"].update(workers=True),
            lambda d: d["config"]["projects"][0].update(retries=False),
            lambda d: d["suites"][0]["specs"][0]["tests"][0].update(status=[]),
            lambda d: d["suites"][0]["specs"][0]["tests"][0].update(projectId={}),
            lambda d: d["suites"][0]["specs"][0]["tests"][0].update(results=[{}, {}]),
            lambda d: d["suites"][0]["specs"][0]["tests"][0]["results"][0].update(retry=False),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                data = report(pathlib.Path("unused"))
                mutate(data)
                result = self.collect(data)
                self.assertFalse(result["complete"], result)
                self.assertNotIn("counts", result)

    def test_empty_suites_and_specs_still_consume_the_traversal_budget(self):
        """A large empty hierarchy cannot bypass the cap by declaring no test results."""
        data = report(pathlib.Path("unused"))
        data["suites"] = [{}] * (adapter.NODE_LIMIT + 1)
        result = self.collect(data)
        self.assertEqual(result["reason"], "report node budget exceeded")
        data = report(pathlib.Path("unused"))
        nested = []
        for _ in range(130):
            nested = [{"suites": nested}]
        data["suites"] = nested
        self.assertEqual(self.collect(data)["reason"], "invalid suite nesting")

    def test_global_errors_are_retained_and_oversized_reports_are_incomplete(self):
        """Global errors count even when all cases pass; unreadable reports have no denominator."""
        result = self.collect(report(pathlib.Path("unused"), errors=[{"message": "fixture"}]))
        self.assertTrue(result["complete"], result)
        self.assertEqual(result["counts"]["global_errors"], 1)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / adapter.REPORT).write_bytes(b"x" * (adapter.REPORT_LIMIT + 1))
            result = adapter.collect(root)
            self.assertFalse(result["complete"])
            self.assertNotIn("counts", result)

    def test_success_requires_executed_cases_in_both_engines(self):
        """Exit zero cannot excuse global errors, unstarted engines or interrupted cases."""
        passed = self.collect(report(pathlib.Path("unused")))
        self.assertIsNone(adapter.success_problem(passed))
        expected_failure = self.collect(report(pathlib.Path("unused"), expected="failed",
                                              results=[{"status": "failed", "retry": 0}]))
        self.assertIsNone(adapter.success_problem(expected_failure))
        for data in (
            report(pathlib.Path("unused"), status="skipped", results=[]),
            report(pathlib.Path("unused"), status="skipped", results=[{"status": "interrupted", "retry": 0}]),
            report(pathlib.Path("unused"), errors=[{"message": "global failure"}]),
        ):
            self.assertIsNotNone(adapter.success_problem(self.collect(data)))
        one_engine = report(pathlib.Path("unused"))
        one_engine["suites"][0]["specs"].pop()
        one_engine["stats"]["expected"] = 1
        self.assertIsNotNone(adapter.success_problem(self.collect(one_engine)))
        failed = self.collect(report(pathlib.Path("unused")),
                              policy_mutation=lambda p: p.update(status="failed"))
        self.assertIsNotNone(adapter.success_problem(failed))
        self.assertIsNotNone(adapter.success_problem({"complete": False}))


class PlaywrightPreparationTest(unittest.TestCase):
    """Preparation verifies installed inputs without executing npx or changing process environment."""

    def test_pinned_probe_and_command_policy_with_private_child_environment(self):
        """Real CLI version spelling is accepted; mismatches and truncated probes refuse execution."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            e2e = root / "e2e"
            manifests = {}
            for name in ("playwright", "@playwright/test"):
                package = e2e / "node_modules" / name
                package.mkdir(parents=True)
                (package / "package.json").write_text(json.dumps({"version": adapter.VERSION}))
                manifests[f"node_modules/{name}"] = {"version": adapter.VERSION}
            lock = json.dumps({"packages": manifests}).encode()
            (e2e / "package-lock.json").write_bytes(lock)
            (e2e / "node_modules/playwright/cli.js").write_text("fixture CLI")
            (e2e / adapter.CONFIG).write_text("fixture config")
            (e2e / adapter.REPORTER).write_text("fixture reporter")
            node = root / "node"
            node.write_text("fixture executable, never invoked")
            node.chmod(0o700)
            original = {"PATH": str(root), "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD": "secret-value",
                        "PWTEST_SKIP_TEST_OUTPUT": "secret-value", "KEEP": "value"}
            controls = ("PWDEBUG", "PW_TEST_REPORTER", "PW_TEST_SOURCE_TRANSFORM",
                        "PW_TEST_SOURCE_TRANSFORM_SCOPE", "npm_config_pwdebug",
                        "npm_config_playwright_browsers_path", "npm_package_config_pwdebug",
                        "npm_package_config_playwright_browsers_path")
            original.update({name: "secret-value" for name in controls})
            versions = {"node": b"v24.16.0\n", "playwright": b"Version 1.62.0\n"}
            calls = []

            def probe(argv):
                """Expose bounded probe evidence without starting a process."""
                calls.append(argv)
                return SimpleNamespace(complete=True, stdout_sample_truncated=False,
                                       stdout_sample=versions["node" if len(argv) == 2 else "playwright"],
                                       evidence=lambda: {"fixture": True})

            child = original.copy()
            run_dir = root / "run"
            command, evidence = adapter.prepare(run_dir, root,
                ["npx", "playwright", "test", "a.spec.ts"], child, probe)
            self.assertEqual(command[:3], [str(node), str(e2e / "node_modules/playwright/cli.js"), "test"])
            self.assertEqual(len(calls), 2)
            for flag in ("--workers=1", "--retries=0", "--repeat-each=1", "--forbid-only",
                         "--fail-on-flaky-tests", "--max-failures=0", "--update-snapshots=none"):
                self.assertIn(flag, command)
            self.assertEqual(child["PLAYWRIGHT_JSON_OUTPUT_FILE"], str(run_dir / adapter.REPORT))
            self.assertEqual(child["KEEP"], "value")
            self.assertNotIn("PWTEST_SKIP_TEST_OUTPUT", child)
            self.assertIn("PWTEST_SKIP_TEST_OUTPUT", original)
            for name in controls:
                self.assertNotIn(name, child)
                self.assertIn(name, evidence["removed_environment_names"])
                self.assertEqual(original[name], "secret-value")
            self.assertNotIn("secret-value", json.dumps(evidence))
            self.assertEqual(evidence["hashes"]["package_lock"], hashlib.sha256(lock).hexdigest())
            for kind, invalid in (("node", b"vgarbage"), ("playwright", b"Version 1.61.0")):
                with self.subTest(kind=kind):
                    saved = versions[kind]
                    versions[kind] = invalid
                    with self.assertRaises(ValueError):
                        adapter.prepare(run_dir, root, ["npx", "playwright", "test"], original.copy(), probe)
                    versions[kind] = saved
            def truncated_probe(argv):
                """A successful process with truncated stdout does not establish its version."""
                result = probe(argv)
                result.stdout_sample_truncated = True
                return result

            with self.assertRaisesRegex(ValueError, "incomplete"):
                adapter.prepare(run_dir, root, ["npx", "playwright", "test"], original.copy(), truncated_probe)
            for name in ("playwright", "@playwright/test"):
                manifest = e2e / "node_modules" / name / "package.json"
                good = manifest.read_bytes()
                manifest.write_text(json.dumps({"version": "1.61.0"}))
                with self.assertRaisesRegex(ValueError, "version"):
                    adapter.prepare(run_dir, root, ["npx", "playwright", "test"], original.copy(), probe)
                manifest.write_bytes(good)
            (e2e / "package-lock.json").write_text(json.dumps({"packages": {}}))
            with self.assertRaisesRegex(ValueError, "lock"):
                adapter.prepare(run_dir, root, ["npx", "playwright", "test"], original.copy(), probe)


if __name__ == "__main__":
    unittest.main(verbosity=2)
