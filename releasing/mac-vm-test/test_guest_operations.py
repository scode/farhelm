"""Inject command and provider failures at the managed UI/command boundary.

These tests establish ordering, durable handoffs and evidence ownership. The
fake native provider is deliberately not a Farhelm acceptance observation.
"""

import io
import json
import plistlib
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest import mock

from guest_operations import Operations, serve, write_json
from guest_tester import managed_result, native_args, prompt, TesterError
from checkpoint_contract import available_operations, required_operations
from guest_workflow import managed_instructions
from test_guest_tester import task
from test_window_geometry import observation


class ManagedOperationsTests(unittest.TestCase):
    """Keep failure handling and file ownership independent of model cooperation."""

    def setUp(self):
        """Own separate pass and attempt paths without changing the process environment."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.pass_dir = self.root / 'pass'
        self.pass_dir.mkdir()
        self.value = task()
        self.value['context'] = {'managed_operations': True, 'pass_directory': str(self.pass_dir),
            'source_checkout': str(self.pass_dir / 'source'), 'signed_inputs': str(self.pass_dir / 'signed'),
            'previous': 'v0.29.0', 'candidate': 'v0.30.0', 'ssh': {'alias': 'fixture-peer'}}
        self.output = self.pass_dir / 'attempt/output'
        (self.output / 'work').mkdir(parents=True)

    def operations(self, value=None, output=None, runner=None):
        """Use explicit fixture paths and optionally inject the fixed command boundary."""
        kwargs = {} if runner is None else {'runner': runner}
        return Operations(value or self.value, output or self.output, home=self.root / 'guest-home', **kwargs)

    def test_failed_command_blocks_native_call_even_when_client_continues(self):
        """The old failure batched a failed receipt command and a restart; the second must never be forwarded."""
        self.value['check_id'] = 'native-preflight'
        calls = []
        def runner(argv, **kwargs):
            """Fail the first fixed prerequisite command before any dependent UI can run."""
            calls.append(argv)
            return subprocess.CompletedProcess(argv, 1, b'', b'fixture command failure')
        operations = self.operations(runner=runner)
        native = types.SimpleNamespace(stdin=io.StringIO(), stdout=io.StringIO())
        messages = [
            {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {
                'name': 'brick_action', 'arguments': {'operation': 'prerequisites'}}},
            {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/call', 'params': {
                'name': 'brick_ui', 'arguments': {'target': 'desktop', 'method': 'getState', 'args': []}}}]
        responses = []
        serve(self.value, self.output, native, [json.dumps(row) for row in messages], responses.append, operations)
        self.assertEqual(len(calls), 1)
        self.assertEqual(native.stdin.getvalue(), '')
        self.assertTrue(all(row['result']['isError'] for row in responses))
        self.assertIn('gate is closed', responses[1]['result']['content'][0]['text'])
        self.assertEqual(len(list((self.output / 'work').glob('managed-*-failed-command.json'))), 1)
        self.assertTrue((self.output / 'managed-failure.json').is_file())

    def test_native_error_closes_gate_before_following_fixed_operation(self):
        """A refused native action must prevent a later installer just as a failed command prevents a click."""
        operations = self.operations(runner=lambda *args, **kwargs: self.fail('command ran after native failure'))
        native = types.SimpleNamespace(stdin=io.StringIO(), stdout=io.StringIO(json.dumps({
            'jsonrpc': '2.0', 'id': 1, 'result': {'isError': True, 'content': [{'type': 'text', 'text': 'fixture refusal'}]}}) + '\n'))
        messages = [
            {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': 'brick_ui',
                'arguments': {'target': 'desktop', 'method': 'getState', 'args': []}}},
            {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/call', 'params': {
                'name': 'brick_action', 'arguments': {'operation': 'install_candidate'}}}]
        responses = []
        serve(self.value, self.output, native, map(json.dumps, messages), responses.append, operations)
        self.assertEqual(len(native.stdin.getvalue().splitlines()), 1)
        self.assertIn('gate is closed', responses[1]['result']['content'][0]['text'])

    def test_native_client_request_is_answered_without_a_second_action_lane(self):
        """CUA's roots request must not deadlock while queued UI work must still respect a later failure."""
        operations = self.operations()
        replies = [
            {'jsonrpc': '2.0', 'id': 'native-roots', 'method': 'roots/list'},
            {'jsonrpc': '2.0', 'id': 1, 'result': {'isError': True,
                'content': [{'type': 'text', 'text': 'controlled native refusal'}]}}]
        native = types.SimpleNamespace(stdin=io.StringIO(), stdout=io.StringIO(
            ''.join(json.dumps(row) + '\n' for row in replies)))
        first = {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call',
                 'params': {'name': 'brick_ui', 'arguments': {'target': 'desktop', 'method': 'getState', 'args': []}}}
        queued = dict(first, id=2)
        client = {'jsonrpc': '2.0', 'id': 'native-roots', 'result': {'roots': []}}
        responses = []
        serve(self.value, self.output, native, map(json.dumps, (first, queued, client)), responses.append, operations)
        sent = [json.loads(row) for row in native.stdin.getvalue().splitlines()]
        self.assertEqual(len(sent), 2)
        self.assertEqual(sent[0]['params']['name'], 'js')
        self.assertEqual(sent[0]['params']['arguments']['code'], 'await cua.getState();')
        self.assertEqual(sent[1], client)
        self.assertEqual(responses[0]['method'], 'roots/list')
        self.assertIn('gate is closed', responses[-1]['result']['content'][0]['text'])

    def test_next_checkpoint_reads_retained_summary_without_raw_log(self):
        """Deleting a temporary source log cannot strand the next step after a receipt was retained."""
        self.value['check_id'] = 'update-discovery'
        operations = self.operations()
        raw = self.pass_dir / 'launch.log'
        raw.write_text('updater: override is active\n')
        operations.state['launch_log'] = str(raw)
        operations.dispatch('update_logs')
        raw.unlink()
        next_output = self.pass_dir / 'next-attempt/output'
        (next_output / 'work').mkdir(parents=True)
        value = dict(self.value, check_id='update-relaunch')
        next_operations = self.operations(value, next_output)
        result = next_operations.dispatch('prior_receipts')
        self.assertEqual(result['observation'][0]['value']['lines'], ['updater: override is active'])
        self.assertEqual(len(next_operations.state['receipts']), 1)
        self.assertFalse(raw.exists())

    def test_changed_retained_receipt_refuses_instead_of_reconstructing_it(self):
        """Digest binding distinguishes durable evidence from a filename that merely still exists."""
        operations = self.operations()
        name = operations.receipt('fixture-observation', {'version': '0.29.0'})
        (self.output / 'work' / name).write_text('{"version":"0.30.0"}')
        value = dict(self.value, check_id='later')
        later = self.operations(value)
        with self.assertRaisesRegex(RuntimeError, 'changed or disappeared'):
            later.dispatch('prior_receipts')
        self.assertIsNotNone(later.failure)

    def test_installer_cannot_replace_real_update_discovery(self):
        """The fixed API must preserve the acceptance boundary even if the model requests a shortcut."""
        self.value['check_id'] = 'update-discovery'
        operations = self.operations(runner=lambda *args, **kwargs: self.fail('installer boundary was bypassed'))
        with self.assertRaisesRegex(RuntimeError, 'substitute'):
            operations.dispatch('install_candidate')

    def test_old_installer_waits_for_both_fixture_and_verified_inputs(self):
        """A clean Mac alone cannot justify overwriting an old or dirty Linux fixture later in setup."""
        self.value['check_id'] = 'old-fixture'
        operations = self.operations(runner=lambda *args, **kwargs: self.fail('installer ran before its premises'))
        with self.assertRaisesRegex(RuntimeError, 'clean and verified'):
            operations.dispatch('install_previous')
        self.assertIsNotNone(operations.failure)

    def test_mutation_cannot_be_replayed_while_observations_can_poll(self):
        """A lost-looking UI response must not cause a second launch that conceals the first outcome."""
        self.value['check_id'] = 'app-start'
        operations = self.operations(runner=lambda *args, **kwargs: self.fail('repeated launch reached subprocess'))
        operations.completed.add('launch')
        with self.assertRaisesRegex(RuntimeError, 'cannot be replayed'):
            operations.dispatch('launch')
        self.assertIsNotNone(operations.failure)

    def test_tool_does_not_accept_commands_or_paths(self):
        """Finite operation names must not become an arbitrary shell or file interface."""
        operations = self.operations()
        with self.assertRaisesRegex(RuntimeError, 'unknown'):
            operations.dispatch('sh -c anything')
        native = types.SimpleNamespace(stdin=io.StringIO(), stdout=io.StringIO())
        responses = []
        serve(self.value, self.output, native, [json.dumps({'jsonrpc': '2.0', 'id': 1,
            'method': 'tools/call', 'params': {'name': 'brick_action', 'arguments': {
                'operation': 'versions', 'path': '/outside'}}})], responses.append, operations)
        self.assertTrue(responses[0]['result']['isError'])

    def test_managed_launch_disables_shell_and_uses_same_native_runtime(self):
        """A prompt restriction alone would leave the original source of accidental scripts available."""
        arguments = native_args(self.value['settings'], self.output)
        self.assertIn('features.shell_tool=false', arguments)
        self.assertIn('features.apps=false', arguments)
        self.assertIn('approval_policy="never"', arguments)
        self.assertIn('mcp_servers.cua_repl.default_tools_approval_mode="approve"', arguments)
        self.assertIn('guest_operations.py', ' '.join(arguments))
        text = prompt(self.value)
        self.assertIn('Shell access is disabled', text)
        self.assertIn('do not guess earlier paths', text)
        self.assertNotIn('Write result.json', text)

    def test_passing_ui_prose_cannot_omit_independent_commands(self):
        """The model's pass is insufficient when no machine-owned version observation was made."""
        self.value['check_id'] = 'app-start'
        write_json(self.output / 'managed-operations.json', {
            'completed': ['launch'], 'receipts': [], 'observations': {}, 'continuity': None})
        with self.assertRaisesRegex(TesterError, 'versions'):
            managed_result(self.value, self.output, {'status': 'pass', 'detail': 'fixture UI claim'})

    def test_old_running_process_cannot_pass_candidate_relaunch(self):
        """Installed bytes alone do not prove that Restart actually returned the candidate process."""
        self.value['check_id'] = 'update-relaunch'
        self.value['context']['update_path'] = 'ordinary'
        record = {'completed': ['versions'], 'receipts': [], 'continuity': None,
            'observations': {'versions': {'installed': '0.30.0', 'bundle': '0.30.0',
                'desktop_running': True, 'running_versions': ['0.29.0']}}}
        write_json(self.output / 'managed-operations.json', record)
        with self.assertRaisesRegex(TesterError, 'running-version'):
            managed_result(self.value, self.output, {'status': 'pass', 'detail': 'fixture UI claim'})

    def test_receipt_polling_keeps_handoff_size_bounded(self):
        """Repeated readiness observations retain history without nesting or duplicating handoff facts."""
        operations = self.operations()
        for number in range(25):
            operations.receipt('poll', {'counter': number})
        self.assertEqual(len(operations.state['receipts']), 1)
        self.assertEqual(len(list((self.output / 'work').glob('managed-*-poll.json'))), 25)
        later_output = self.pass_dir / 'later/output'
        (later_output / 'work').mkdir(parents=True)
        later = self.operations(dict(self.value, check_id='later'), later_output)
        observation = later.dispatch('prior_receipts')['observation']
        self.assertEqual(len(observation), 1)
        self.assertEqual(observation[0]['value']['counter'], 24)

    def test_candidate_process_cannot_hide_a_wrong_cli_version(self):
        """Installation and relaunch receipts must agree on the published CLI as well as the app process."""
        self.value['check_id'] = 'update-relaunch'
        self.value['context']['update_path'] = 'ordinary'
        write_json(self.output / 'managed-operations.json', {
            'completed': ['versions'], 'receipts': [], 'continuity': None,
            'observations': {'versions': {'installed': '0.30.0', 'bundle': '0.30.0',
                'desktop_running': True, 'running_versions': ['0.30.0'], 'cli': {'stdout': 'farhelm 0.29.0\n'}}}})
        with self.assertRaisesRegex(TesterError, 'running-version'):
            managed_result(self.value, self.output, {'status': 'pass', 'detail': 'fixture UI claim'})

    def test_bonus_task_carries_only_its_assigned_acceptance_contract(self):
        """The tester must not browse files or choose between unrelated recipe stages to learn its task."""
        self.value.update(pass_id='upgrade-v0.29.0', check_id='preserved-state')
        self.value['instructions'] = managed_instructions('preserved-state', 'upgrade-v0.29.0')
        text = prompt(self.value)
        self.assertIn('original sessions, saved brick-template', text)
        self.assertIn('same original session/captured identities', text)
        self.assertIn('Shell access is disabled', text)
        self.assertNotIn('## Select the upgrade path', text)
        self.assertNotIn('This is an automated release brick test checking', text)

    def test_feedback_task_embeds_the_authorized_message_without_file_browsing(self):
        """A fixture file read through Finder is unnecessary UI work and a possible new failure boundary."""
        text = managed_instructions('p1-feedback-send', 'p0-updater-upgrade')
        self.assertIn('[brick-test] vX.Y.Z', text)
        self.assertIn('This is an automated release brick test checking that this release can send feedback;', text)
        self.assertIn('Use native paste', text)
        self.assertNotIn('Read RECIPE', text)

    def test_window_contract_pairs_ten_images_with_measurements_and_control_checks(self):
        """App-only pixels cannot prove movement, and control closes must not escape the stationary-frame check."""
        text = managed_instructions('window-drag', 'fresh')
        for obligation in ('EXACTLY ten', 'After EACH screenshot call window_frame',
                           'after EVERY control input/close', 'image_scale', 'finder_frame',
                           'Measure once more after that close'):
            self.assertIn(obligation, text)
        self.assertIn('window_frame', required_operations('window-drag', 'fresh', None))
        self.assertNotIn('window_frame', available_operations('app-start', 'fresh', None))

    def test_bonus_readiness_checks_gestures_without_expanding_p0(self):
        """A missing native gesture must be found before bonus installation, while P0 keeps its smaller prerequisites."""
        bonus = managed_instructions('native-preflight', 'fresh')
        self.assertIn('stop before Farhelm', bonus)
        self.assertIn('double-click', bonus)
        self.assertIn('finder_frame', required_operations('native-preflight', 'fresh', None))
        self.assertNotIn('finder_frame', available_operations('native-preflight', 'p0-fresh', None))

    def test_bonus_readiness_cannot_pass_on_command_success_without_motion(self):
        """A reported pass and successful frame queries still need a measured same-window move."""
        self.value.update(check_id='native-preflight', pass_id='fresh')
        write_json(self.output / 'managed-operations.json', {
            'completed': ['prerequisites', 'finder_frame'], 'receipts': [], 'continuity': None,
            'observations': {}})
        write_json(self.output / 'work/managed-window-frames.json', {'observations': []})
        with self.assertRaisesRegex(TesterError, 'actual window-drag delivery'):
            managed_result(self.value, self.output, {'status': 'pass', 'detail': 'fictional native success'})

    def test_conversation_pass_requires_a_captured_identity(self):
        """An actual answer alone must not satisfy the bonus promise that the conversation can later resume."""
        self.value['check_id'] = 'codex-conversation'
        record = {'completed': ['inspect_state'], 'receipts': [], 'continuity': None,
                  'observations': {'inspect_state': {'supervisor.db': {
                      'sessions': [{'id': 'fixture-session', 'captured_conversation': None}]}}}}
        path = self.output / 'managed-operations.json'
        write_json(path, record)
        with self.assertRaisesRegex(TesterError, 'conversation capture'):
            managed_result(self.value, self.output, {'status': 'pass', 'detail': 'fixture UI answer'})
        record['observations']['inspect_state']['supervisor.db']['sessions'][0]['captured_conversation'] = 'fixture-conversation'
        write_json(path, record, replace=True)
        self.assertEqual(managed_result(self.value, self.output, {'status': 'pass', 'detail': 'fixture UI answer'})['status'], 'pass')

    def test_manual_restart_has_no_override_obligations_or_endpoint_probe(self):
        """A successful installer restart must not be rejected for an unexercised override's removal."""
        text = managed_instructions('update-relaunch', 'p0-installer-upgrade', 'manual')
        self.assertIn('install_candidate while the old app runs', text)
        self.assertNotIn('override', text)
        self.assertNotIn('public_latest', text)
        self.assertEqual(required_operations('update-relaunch', 'p0-installer-upgrade', 'manual'),
                         {'install_candidate', 'versions'})
        available = available_operations('update-relaunch', 'p0-installer-upgrade', 'manual')
        self.assertNotIn('public_latest', available)
        self.assertNotIn('launch_override', available)
        self.assertNotIn('launch', available)
        self.value['check_id'] = 'update-relaunch'
        self.value['context']['update_path'] = 'manual'
        operations = self.operations(runner=lambda *args, **kwargs: self.fail('irrelevant endpoint request executed'))
        with self.assertRaisesRegex(RuntimeError, 'override-removal'):
            operations.dispatch('public_latest')
        self.assertIsNotNone(operations.failure)

    def test_discovery_paths_compile_only_their_own_startup_premises(self):
        """Ordinary updates need no override launch; override updates must retain that launch as evidence."""
        ordinary = managed_instructions('update-discovery', 'p0-updater-upgrade', 'ordinary')
        self.assertIn('actual Check for updates', ordinary)
        self.assertNotIn('override', ordinary)
        override = managed_instructions('update-discovery', 'p0-updater-upgrade', 'override')
        self.assertIn('launch_override', override)
        self.assertIn('startup override announcement', override)
        self.assertIn('launch_override', required_operations('update-discovery', 'p0-updater-upgrade', 'override'))
        self.assertNotIn('launch_override', required_operations('update-relaunch', 'p0-updater-upgrade', 'override'))
        self.assertIn('launch_override', required_operations('update-relaunch', 'upgrade-v0.29.0', 'override'))

    def test_native_failure_record_omits_image_payload(self):
        """The latch remains small even when a provider returns a screenshot beside its error."""
        operations = self.operations()
        reply = {'jsonrpc': '2.0', 'id': 1, 'result': {'isError': True, 'content': [
            {'type': 'text', 'text': 'controlled native refusal'},
            {'type': 'image', 'data': 'private-image-payload'}]}}
        native = types.SimpleNamespace(stdin=io.StringIO(), stdout=io.StringIO(json.dumps(reply) + '\n'))
        call = {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call',
                'params': {'name': 'brick_ui', 'arguments': {'target': 'desktop', 'method': 'getState', 'args': []}}}
        serve(self.value, self.output, native, [json.dumps(call)], lambda value: None, operations)
        retained = (self.output / 'managed-failure.json').read_text()
        self.assertIn('controlled native refusal', retained)
        self.assertNotIn('private-image-payload', retained)

    def test_wrong_pass_binding_refuses_before_any_guest_command(self):
        """A staging mistake must be detected before prerequisite probes or mutations begin."""
        self.value['context']['pass_directory'] = str(self.root / 'other-pass')
        with self.assertRaisesRegex(RuntimeError, 'executor-owned task path'):
            self.operations(runner=lambda *args, **kwargs: self.fail('command ran before binding check'))

    def test_background_reporters_do_not_identify_the_live_app_version(self):
        """Old session reporters survive updates; only the GUI's direct supervisor child identifies its active payload."""
        operations = self.operations()
        app = operations.home / 'Applications/Farhelm.app'
        (app / 'Contents/Versions').mkdir(parents=True)
        (app / 'Contents/Versions/installed').write_text('0.30.0')
        with (app / 'Contents/Info.plist').open('wb') as stream:
            plistlib.dump({'CFBundleShortVersionString': '0.30.0'}, stream)
        for live, reporter in (('0.29.0', '0.30.0'), ('0.30.0', '0.29.0')):
            with self.subTest(live=live):
                table = (f'101 1 {app}/Contents/MacOS/farhelm-desktop\n'
                         f'102 101 {app}/Contents/Versions/{live}/farhelm\n'
                         f'103 999 {app}/Contents/Versions/{reporter}/farhelm\n')
                def runner(argv, **kwargs):
                    """Expose the same paths with different ownership, without touching host process state."""
                    data = b'farhelm 0.30.0\n' if argv[-1] == '--version' else table.encode()
                    return subprocess.CompletedProcess(argv, 0, data, b'')
                operations.runner = runner
                measured = operations.version()
                self.assertEqual(measured['running_versions'], [live])
                self.assertEqual(measured['desktop_pids'], [101])

    def test_frame_history_is_complete_without_growing_export_references(self):
        """Ten native images leave room for one frame trace, not a separate exported file for every measurement."""
        self.value['check_id'] = 'window-drag'
        count = 0
        def runner(argv, **kwargs):
            """Return actual-shaped window metadata while making each observation distinguishable."""
            nonlocal count
            count += 1
            data = observation()
            data['windows'][0]['bounds']['X'] += count
            return subprocess.CompletedProcess(argv, 0, json.dumps(data).encode(), b'')
        operations = self.operations(runner=runner)
        with mock.patch.object(operations, 'version', return_value={
                'running_versions': ['0.30.0'], 'desktop_pids': [101]}):
            for number in range(1, 11):
                operations.latest_images['farhelm'] = {'width': 1600, 'height': 1200,
                    'sequence': number, 'sha256': 'fixture-image'}
                response = operations.dispatch('window_frame')['observation']
                self.assertNotIn('observations', response)
        retained = json.loads((self.output / 'work/managed-window-frames.json').read_text())
        self.assertEqual(len(retained['observations']), 10)
        self.assertEqual(retained['observations'][0]['observation']['frame']['x'], 113)
        self.assertEqual(retained['observations'][-1]['observation']['frame']['x'], 122)
        record = json.loads((self.output / 'managed-operations.json').read_text())
        self.assertEqual(len(record['receipts']), 3)
        self.assertIn('managed-window-frames.json', record['receipts'])


if __name__ == '__main__':
    unittest.main()
