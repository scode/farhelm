"""Exercise source discovery and checker status using private, nonexecuted source fixtures."""
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('sleep_checker', pathlib.Path(__file__).with_name('check-test-sleeps.py'))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class SleepCheckerTest(unittest.TestCase):
    """A convention gate must distinguish violations, incomplete checks and genuinely clean input."""

    def test_conditional_module_paths_are_explicitly_incomplete(self):
        """A test-selected source must not disappear merely because its path requires cfg_attr evaluation."""
        files = {'crates/sample/src/lib.rs': b'#[cfg_attr(test, path = "test_impl.rs")] mod implementation;'}
        for present in (False, True):
            with self.subTest(present=present):
                if present:
                    files['crates/sample/src/test_impl.rs'] = b'fn helper() { sleep(d); }'
                with self.assertRaisesRegex(ValueError, 'conditional module path'):
                    checker.inventory(files, checker.SourceBudget())

    def test_raw_modules_and_nonroot_entrypoint_basenames(self):
        """Identifier spelling and familiar target basenames cannot change an ordinary module's directory."""
        for name in ('lib', 'main'):
            files = {
                'crates/sample/src/lib.rs': b'#[cfg(test)] mod r#tests;',
                'crates/sample/src/tests.rs': f'mod {name};'.encode(),
                f'crates/sample/src/tests/{name}.rs': b'mod r#helper;',
                f'crates/sample/src/tests/{name}/helper.rs': b'fn helper() { sleep(d); }',
            }
            self.assertEqual(len(checker.inventory(files, checker.SourceBudget())), 1)

    def test_linked_directories_cannot_report_a_clean_cli_inventory(self):
        """Both live and dangling directory links expose an inspection gap without following the target."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / 'crates').mkdir()
            (root / 'e2e/tests').mkdir(parents=True)
            target = root / 'fixture'
            target.mkdir()
            (target / 'example.ts').write_text('setTimeout(cb, 1);')
            link = root / 'e2e/tests/linked'
            for destination in (target, root / 'missing'):
                link.symlink_to(destination, target_is_directory=True)
                result = subprocess.run([sys.executable, '-B', checker.__file__, '--root', str(root)],
                                        capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn('uninspected symlink', result.stderr)
                link.unlink()

    def test_linked_e2e_parent_is_not_followed(self):
        """Starting inventory at e2e/tests must not skip the no-follow check for its fixed parent."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary) / 'checkout'
            (root / 'crates').mkdir(parents=True)
            outside = pathlib.Path(temporary) / 'outside'
            (outside / 'tests').mkdir(parents=True)
            (outside / 'tests/example.ts').write_text('// sleep-ok: observation\nsetTimeout(cb, 1);')
            (root / 'e2e').symlink_to(outside, target_is_directory=True)
            result = subprocess.run([sys.executable, '-B', checker.__file__, '--root', str(root), '--inventory'],
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn('incomplete check', result.stderr)
            self.assertEqual(result.stdout, '')

    def test_nested_tests_directory_is_not_a_crate_root(self):
        """External test helpers keep their module-relative child path even beneath a tests directory."""
        files = {
            'crates/sample/src/lib.rs': b'#[cfg(test)] mod tests;',
            'crates/sample/src/tests.rs': b'mod helpers;',
            'crates/sample/src/tests/helpers.rs': b'mod nested;',
            'crates/sample/src/tests/helpers/nested.rs': b'fn helper() { sleep(d); }',
        }
        self.assertEqual(len(checker.inventory(files, checker.SourceBudget())), 1)
        files['crates/sample/src/lib.rs'] = b'mod tests;'
        self.assertEqual(checker.inventory(files, checker.SourceBudget()), [])

    def test_binary_root_and_inner_test_scope_resolve_modules(self):
        """Cargo binary roots resolve sibling modules, and inner cfg still exposes missing test sources."""
        files = {
            'crates/sample/src/bin/tool.rs': b'#[cfg(test)] mod tests;',
            'crates/sample/src/bin/tests.rs': b'fn helper() { sleep(d); }',
        }
        self.assertEqual(len(checker.inventory(files, checker.SourceBudget())), 1)
        for directory in ('tests/scenarios', 'src/bin/tool'):
            files = {
                f'crates/sample/{directory}/main.rs': b'#[cfg(test)] mod helpers;',
                f'crates/sample/{directory}/helpers.rs': b'fn helper() { sleep(d); }',
            }
            self.assertEqual(len(checker.inventory(files, checker.SourceBudget())), 1)
        with self.assertRaisesRegex(ValueError, 'missing or ambiguous'):
            checker.inventory({'crates/sample/src/lib.rs': b'#![cfg(test)] mod missing;'}, checker.SourceBudget())

    def test_external_test_module_inherits_scope(self):
        """A path-attributed helper file remains checked even without its own test attribute."""
        files = {
            'crates/sample/src/lib.rs': b'#[cfg(test)] #[path = "helper.rs"] mod tests;',
            'crates/sample/src/helper.rs': b'fn helper() { std::thread::sleep(d); }',
            'crates/sample/src/production.rs': b'fn routine() { std::thread::sleep(d); }',
        }
        calls = checker.inventory(files, checker.SourceBudget())
        self.assertEqual([(item['path'], item['call']) for item in calls],
                         [('crates/sample/src/helper.rs', 'std::thread::sleep')])

    def test_missing_module_and_exhausted_time_are_incomplete(self):
        """A source gap or deadline cannot authorize a zero-violation result."""
        with self.assertRaisesRegex(ValueError, 'missing or ambiguous'):
            checker.inventory({'crates/sample/src/lib.rs': b'#[cfg(test)] mod missing;'}, checker.SourceBudget())
        with self.assertRaisesRegex(ValueError, 'elapsed-time budget'):
            checker.inventory({'crates/sample/src/lib.rs': b''}, checker.SourceBudget(seconds=0))

    def test_cli_distinguishes_unannotated_clean_and_invalid_sources(self):
        """An explicit inventory remains failing until all recognized delays have a real rationale."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            source = root / 'crates/sample/tests/example.rs'
            source.parent.mkdir(parents=True)
            (root / 'e2e/tests').mkdir(parents=True)
            for content, expected in (
                ('fn scenario() { sleep(d); }', 1),
                ('fn scenario() {\n // sleep-ok: observe the absence window\n sleep(d);\n}', 0),
                ('fn scenario( { sleep(d);', 2),
            ):
                source.write_text(content)
                result = subprocess.run([sys.executable, '-B', checker.__file__, '--root', str(root), '--inventory'],
                                        capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected != 2:
                    report = json.loads(result.stdout)
                    self.assertEqual(len(report['calls']), 1)
                    self.assertEqual(report['unannotated'], expected)
                else:
                    self.assertIn('incomplete check', result.stderr)

    def test_source_links_and_byte_limits_are_rejected(self):
        """Discovery neither follows a linked source nor reads beyond its aggregate byte allowance."""
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / 'crates').mkdir()
            (root / 'e2e/tests').mkdir(parents=True)
            target = root / 'outside.rs'
            target.write_text('fn helper() {}')
            linked = root / 'crates/linked.rs'
            linked.symlink_to(target)
            with self.assertRaisesRegex(ValueError, 'uninspected symlink'):
                checker.sources(root, checker.SourceBudget())
            linked.unlink()
            linked.write_text('fn helper() {}')
            with self.assertRaisesRegex(ValueError, 'byte budget'):
                checker.sources(root, checker.SourceBudget(byte_limit=1))


if __name__ == '__main__':
    unittest.main()
