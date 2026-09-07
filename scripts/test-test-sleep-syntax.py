"""Check the sleep rule's syntax boundaries without compiling or executing fixture code."""
import unittest

from test_sleep_syntax import SyntaxBudget, external_modules, sleep_calls, sleep_details


class SleepSyntaxTest(unittest.TestCase):
    """Executable delays remain visible through macros and aliases; literal text stays inert."""

    def test_playwright_deadline_configuration_is_not_a_delay(self):
        """A test deadline sets a bound; it must not acquire sleep annotations or be moved into a wait helper."""
        source = b'test.setTimeout(60000); testInfo.setTimeout(5000); setTimeout(cb, 10); page.waitForTimeout(10);'
        self.assertEqual(sleep_calls(source, 'typescript'), [(1, 'setTimeout'), (1, 'page.waitForTimeout')])

    def test_transparent_wrappers_and_literal_member_names_preserve_calls(self):
        """Parentheses, raw identifiers and static computed properties cannot hide the same delay function."""
        rust = b'fn helper() { sleep(d); (std::thread::sleep)(d); std::thread::r#sleep(d); }'
        browser = b'''setTimeout(cb, 1); (setTimeout)(cb, 1); setTimeout!(cb, 1);
await page["waitForTimeout"](1); await page['waitForTimeout'](1);
const ignored = "page[waitForTimeout](1)";
'''
        self.assertEqual(len(sleep_calls(rust, 'rust')), 3)
        self.assertEqual(len(sleep_calls(browser, 'typescript')), 5)

    def test_macro_local_and_string_named_import_aliases(self):
        """An explicit use inside a macro branch is still source-visible, as is a quoted TS import name."""
        rust = b'''fn helper() { tokio::select! {
_ = async { use tokio::time::{sleep as pause}; pause(d).await; } => {}
} }'''
        browser = b'import { "setTimeout" as pause } from "node:timers"; pause(cb, 1);'
        self.assertEqual(sleep_calls(rust, 'rust'), [(2, 'pause')])
        self.assertEqual(sleep_calls(browser, 'typescript'), [(1, 'pause')])

    def test_inner_attributes_and_attributed_initializers_remain_test_scope(self):
        """Test scope applies to whole modules and cfg-qualified code beyond function declarations."""
        samples = [
            b'#![cfg(test)] fn helper() { std::thread::sleep(d); }',
            b'mod fixture { #![cfg(test)] fn helper() { std::thread::sleep(d); } }',
            b'#[cfg(test)] static FIXTURE: LazyLock<()> = LazyLock::new(|| { std::thread::sleep(d); });',
            b'fn helper() { #[cfg(test)] std::thread::sleep(d); }',
        ]
        for source in samples:
            with self.subTest(source=source):
                self.assertEqual(len(sleep_calls(source, 'rust', whole_file=False)), 1)
                production = source.replace(b'cfg(test)', b'cfg(not(test))')
                self.assertEqual(sleep_calls(production, 'rust', whole_file=False), [])

    def test_report_budget_bounds_repeated_rationale_before_retaining_results(self):
        """One shared long annotation cannot multiply into an unbounded serialized inventory."""
        source = b'fn fixture() { ' + b'sleep(d); ' * 100 + b'// sleep-ok: ' + b'x' * 1000 + b'\n}'
        with self.assertRaisesRegex(ValueError, 'report-byte budget'):
            sleep_details(source, 'rust', budget=SyntaxBudget(report_bytes=20000))
        with self.assertRaisesRegex(ValueError, 'delay-call budget'):
            sleep_details(b'fn fixture() { sleep(d); sleep(d); sleep(d); }', 'rust', budget=SyntaxBudget(calls=2))

    def test_processing_consults_cooperative_cancellation(self):
        """Cancellation is checked during syntax work rather than only before and after an entire file."""
        checks = 0

        def cancel_during_work():
            nonlocal checks
            checks += 1
            if checks == 200:
                raise ValueError('cancelled during syntax traversal')

        source = b'fn fixture() {\n' + b'// ordinary comment\nsleep(d);\n' * 2000 + b'}'
        with self.assertRaisesRegex(ValueError, 'cancelled during syntax traversal'):
            sleep_details(source, 'rust', budget=SyntaxBudget(check=cancel_during_work))
        self.assertEqual(checks, 200)

    def test_wrapped_macro_and_asserted_browser_callees(self):
        """Transparent source wrappers cannot hide a call, including inside unexpanded macro tokens."""
        for callee in ('sleep', '(sleep)', '((tokio::time::sleep))'):
            source = f'fn helper() {{ tokio::select! {{ _ = {callee}(d) => {{}} }} }}'.encode()
            self.assertEqual(len(sleep_calls(source, 'rust')), 1)
        for callee in ('setTimeout', '(setTimeout as typeof setTimeout)',
                       '(setTimeout satisfies typeof setTimeout)', '(<typeof setTimeout>setTimeout)'):
            self.assertEqual(len(sleep_calls(f'{callee}(cb, 1);'.encode(), 'typescript')), 1)
        self.assertEqual(sleep_calls(b'fn helper() { macro_name! { "(sleep)(d)" } }', 'rust'), [])

    def test_wrapped_deadline_receivers_are_still_configuration(self):
        """Transparent wrappers cannot turn Playwright deadline settings into artificial sleep violations."""
        source = b'''test.setTimeout(5); (test).setTimeout(5);
testInfo.setTimeout(5); (testInfo as TestInfo).setTimeout(5);
setTimeout(cb, 5); (setTimeout as typeof setTimeout)(cb, 5);'''
        self.assertEqual(len(sleep_calls(source, 'typescript')), 2)

    def test_explicit_alias_chains_ignore_declaration_order(self):
        """Reordering imports leaves the same source-written delay reachable, even with cyclic aliases."""
        first = b'use tokio::time::sleep as pause;'
        second = b'use pause as wait;'
        for declarations in (first + second, second + first):
            source = declarations + b'use wait as pause; fn helper() { wait(d); }'
            self.assertEqual(sleep_calls(source, 'rust'), [(1, 'wait')])

    def test_raw_variable_is_valid_rust_source(self):
        """The pinned grammar must accept ordinary raw identifiers used in maintained fixtures."""
        source = b'fn fixture() { let raw = "value"; let tail = &raw[1..]; consume(&raw); sleep(d); }'
        self.assertEqual(sleep_calls(source, 'rust'), [(1, 'sleep')])

    def test_rust_macro_alias_and_literals(self):
        """A select branch must not bypass the rule, and nested comments must not invent calls."""
        source = b'''use tokio::time::sleep as pause;
fn sample() {
    let ignored = r#"sleep(1)"#;
    /* nested /* sleep(2) */ comment */
    tokio::select! { _ = pause(d) => {}, _ = async { tokio::time::sleep(d).await } => {} }
}
'''
        self.assertEqual(sleep_calls(source, 'rust'), [(5, 'pause'), (5, 'sleep')])

    def test_browser_interpolation_alias_and_regex(self):
        """Template substitutions execute code even though surrounding template and regex text do not."""
        source = b'''import { setTimeout as pause } from 'node:timers/promises';
const literal = `waitForTimeout(1) ${await page.waitForTimeout(2)}`;
const regex = /setTimeout(3)/;
await pause(4);
'''
        self.assertEqual(sleep_calls(source, 'typescript'), [(2, 'page.waitForTimeout'), (4, 'pause')])

    def test_test_declarations_include_helpers_and_exclude_production(self):
        """Moving a call from a test into its test-only module helper must not remove coverage."""
        source = b'''fn production() { std::thread::sleep(d); }
#[cfg(test)]
mod tests { fn helper() { std::thread::sleep(d); } }
#[tokio::test]
async fn direct() { tokio::time::sleep(d).await; }
'''
        self.assertEqual(sleep_calls(source, 'rust', whole_file=False),
                         [(3, 'std::thread::sleep'), (5, 'tokio::time::sleep')])

    def test_compound_conditions_require_test_only_scope(self):
        """An alternative production platform is not test-only, unlike a conjunction requiring test."""
        source = b'''#[cfg(all(test, unix))]
fn test_only() { sleep(d); }
#[cfg(any(test, target_os = "macos"))]
fn also_production() { sleep(d); }
#[cfg(not(test))]
fn production_only() { sleep(d); }
#[cfg(any())]
fn disabled() { sleep(d); }
'''
        self.assertEqual(sleep_calls(source, 'rust', whole_file=False), [(2, 'sleep')])

    def test_annotation_must_be_explanatory_adjacent_comment(self):
        """A real rationale can cover a multiline call; empty annotations and string text cannot."""
        source = b'''fn sample() {
    // sleep-ok: observe absence for the required window
    tokio::time::sleep(d).await;
    let literal = "// sleep-ok: not a real comment";
    tokio::time::sleep(d).await;
    tokio::time::sleep(
        d); // sleep-ok: explicit multiline timing stimulus
    // sleep-ok:
    tokio::time::sleep(d).await;
    // sleep-ok: separated from the call

    sleep(d);
}
'''
        self.assertEqual([item['rationale'] for item in sleep_details(source, 'rust')],
                         ['observe absence for the required window', None,
                          'explicit multiline timing stimulus', None, None])

    def test_external_module_paths_preserve_declared_scope(self):
        """Sibling path attributes and nested modules must resolve to their actual source inventory keys."""
        source = b'''#[cfg(test)] #[path = "sessions_tests.rs"] mod tests;
#[cfg(test)] mod nested { mod helper; }
mod production;
'''
        self.assertEqual(external_modules(source, 'src/sessions.rs'), [
            (['src/sessions_tests.rs'], True),
            (['src/sessions/nested/helper.rs', 'src/sessions/nested/helper/mod.rs'], True),
            (['src/sessions/production.rs', 'src/sessions/production/mod.rs'], False)])
        self.assertEqual(external_modules(b'mod helper;', 'tests/integration.rs', crate_root=True),
                         [(['tests/helper.rs', 'tests/helper/mod.rs'], False)])

    def test_syntax_errors_and_unsupported_paths_are_visible(self):
        """An unparsed source or unsafe module reference cannot look like a file with no delays."""
        with self.assertRaises(ValueError):
            sleep_calls(b'fn broken( { sleep(d);', 'rust')
        with self.assertRaises(ValueError):
            external_modules(b'#[path = "/outside.rs"] mod tests;', 'src/lib.rs')


if __name__ == '__main__':
    unittest.main()
