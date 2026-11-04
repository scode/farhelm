"""Inject command and provider failures at the managed UI/command boundary.

These tests establish ordering, durable handoffs and evidence ownership. The
fake native provider is deliberately not a Farhelm acceptance observation.
"""

import calendar
import contextlib
import hashlib
import io
import json
import os
import plistlib
from pathlib import Path
import select
import sqlite3
import subprocess
import sys
import tempfile
import time
import types
import unittest
from unittest import mock

from guest_operations import (Operations, installer_environment, parse_mac_processes, remote_host_snapshot,
                              run_fixed, select_hosts, serve, write_json)
from guest_tester import managed_result, native_args, prompt, TesterError
from checkpoint_contract import (available_operations, require_agent_terminal, require_old_session,
                                 required_operations, require_remote_host_effect, turn_timeout)
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

    def test_remote_menu_effects_distinguish_remove_from_uninstall(self):
        """An empty fixture or missing data cannot pass either menu check.

Remove must preserve the live supervisor and installed resources, whereas
Uninstall must remove them. Both retain the database, so swapped menu actions
or a destructive cleanup cannot be accepted from a model's success claim.
"""
        before = {'service': {'LoadState': 'loaded', 'ActiveState': 'active', 'SubState': 'running', 'MainPID': '71'},
                  'lib_present': True, 'unit_present': True, 'binary_sha256': 'binary', 'unit_sha256': 'unit',
                  'data_present': True, 'database_present': True}
        uninstalled = dict(before, service={'LoadState': 'not-found', 'ActiveState': 'inactive',
                                           'SubState': 'dead', 'MainPID': '0'}, lib_present=False,
                           unit_present=False, binary_sha256=None, unit_sha256=None)
        require_remote_host_effect('remote-host-remove', before, before)
        require_remote_host_effect('remote-host-uninstall', before, uninstalled)
        for check, after in (('remote-host-remove', uninstalled), ('remote-host-uninstall', before)):
            with self.subTest(swapped=check), self.assertRaises(RuntimeError):
                require_remote_host_effect(check, before, after)
        for check, after in (('remote-host-remove', before), ('remote-host-uninstall', uninstalled)):
            for key in ('data_present', 'database_present'):
                with self.subTest(check=check, lost=key), self.assertRaises(RuntimeError):
                    require_remote_host_effect(check, before, dict(after, **{key: False}))
            with self.subTest(empty_start=check), self.assertRaises(RuntimeError):
                require_remote_host_effect(check, uninstalled, after)
        for key, value in (('MainPID', '72'), ('ActiveState', 'inactive')):
            with self.subTest(changed_service=key), self.assertRaises(RuntimeError):
                require_remote_host_effect('remote-host-remove', before,
                                          dict(before, service=dict(before['service'], **{key: value})))

    def test_remote_snapshot_requires_manager_and_records_files_independently(self):
        """A disconnected user bus cannot masquerade as a successful uninstall.

Read real fixture files with the production probe; only systemctl is injected.
Missing units may return exit 1, but only a positive manager probe and explicit
not-found/inactive properties make that an acceptable absence observation.
"""
        home = self.root / 'remote-home'
        for path, data in (('.local/lib/farhelm/farhelm', b'binary'),
                           ('.config/systemd/user/farhelm-supervisor.service', b'unit'),
                           ('.local/state/farhelm/supervisor.db', b'database')):
            file = home / path
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(data)
        loaded = b'LoadState=loaded\nActiveState=active\nSubState=running\nMainPID=71\n'
        def reply(code, output):
            """Expose exactly the subprocess text mode used by the Linux probe."""
            return subprocess.CompletedProcess([], code, output.decode(), '')
        with mock.patch('guest_operations.subprocess.run', side_effect=[reply(0, b'255\n'), reply(0, loaded)]):
            snapshot = remote_host_snapshot(home)
        self.assertEqual(snapshot['binary_sha256'], hashlib.sha256(b'binary').hexdigest())
        self.assertEqual(snapshot['unit_sha256'], hashlib.sha256(b'unit').hexdigest())
        self.assertTrue(snapshot['database_present'])
        with mock.patch('guest_operations.subprocess.run', return_value=reply(1, b'')) as runner:
            with self.assertRaisesRegex(RuntimeError, 'user manager'):
                remote_host_snapshot(home)
            self.assertEqual(runner.call_count, 1)
        missing = b'LoadState=not-found\nActiveState=inactive\nSubState=dead\nMainPID=0\n'
        with mock.patch('guest_operations.subprocess.run', side_effect=[reply(0, b'255\n'), reply(1, missing)]):
            self.assertEqual(remote_host_snapshot(home)['service']['LoadState'], 'not-found')
        with mock.patch('guest_operations.subprocess.run', side_effect=[reply(0, b'255\n'), reply(1, loaded)]):
            with self.assertRaisesRegex(RuntimeError, 'service observation'):
                remote_host_snapshot(home)

    def test_remote_menu_snapshots_are_ordered_and_before_cannot_be_replaced(self):
        """A late or overwritten baseline must not hide an earlier destructive menu action."""
        self.value['check_id'] = 'remote-host-remove'
        operations = self.operations()
        with self.assertRaisesRegex(RuntimeError, 'required first'):
            operations.perform('remote_host_after')
        with mock.patch.object(operations, 'ssh', return_value={'stdout': '{}'}) as ssh:
            operations.dispatch('remote_host_before')
            operations.dispatch('remote_host_after')
            self.assertIn('def remote_host_snapshot', ssh.call_args.args[0])
            with self.assertRaisesRegex(RuntimeError, 'already retained'):
                operations.dispatch('remote_host_before')
        self.assertNotIn('remote_host_before', available_operations('remote-supervisor-agent', 'p0-updater-upgrade', 'override'))

    def test_remote_menu_pass_requires_independent_effect_receipts(self):
        """Native success text alone cannot waive the SSH before/after acceptance contract."""
        for check in ('remote-host-remove', 'remote-host-uninstall'):
            with self.subTest(check=check):
                self.value['check_id'] = check
                self.value['pass_id'] = 'p0-updater-upgrade'
                write_json(self.output / 'managed-operations.json', {
                    'completed': sorted(required_operations(check, 'p0-updater-upgrade', 'override') - {'remote_host_after'}),
                    'observations': {}, 'receipts': [], 'continuity': None}, replace=True)
                with self.assertRaisesRegex(TesterError, 'omitted operations: remote_host_after'):
                    managed_result(self.value, self.output, {'status': 'pass', 'detail': 'Native success claim'})
                self.assertIn('Remove' if check.endswith('remove') else 'Uninstall',
                              managed_instructions(check, 'p0-updater-upgrade', 'override'))

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

    def test_installer_entrypoints_distinguish_fresh_fixtures_from_updates(self):
        """The new installer's update-only refusal must not prevent fresh or old-fixture setup.

Run a small installer contract fixture through the actual fixed operation and
shell boundary. It refuses a checksum-file invocation without an existing app,
and requires that same input for a manual upgrade of an existing app.
"""
        cases = (
            ('p0-fresh', 'install', 'install_candidate', False),
            ('fresh', 'install', 'install_candidate', False),
            ('p0-installer-upgrade', 'old-fixture', 'install_previous', False),
            ('upgrade-v0.31.0', 'old-fixture', 'install_previous', False),
            ('p0-installer-upgrade', 'update-relaunch', 'install_candidate', True),
            ('upgrade-v0.31.0', 'update-relaunch', 'install_candidate', True),
        )
        for number, (pass_id, check, operation, update) in enumerate(cases):
            with self.subTest(pass_id=pass_id, check=check):
                directory = self.root / f'install-{number}'
                output = directory / 'attempt/output'
                (output / 'work').mkdir(parents=True)
                context = dict(self.value['context'], pass_directory=str(directory),
                               signed_inputs=str(directory / 'signed'), update_path='manual')
                value = dict(self.value, pass_id=pass_id, check_id=check, context=context)
                signed = directory / ('old-signed-inputs' if operation == 'install_previous' else 'signed')
                signed.mkdir()
                version = context['previous' if operation == 'install_previous' else 'candidate']
                script = signed / 'install.sh'
                script.write_text('''if [ -n "${FARHELM_INSTALL_SUMS_FILE:-}" ]; then
    test -d "$HOME/Applications/Farhelm.app" || exit 23
    test "$FARHELM_INSTALL_SUMS_FILE" = "$EXPECTED_SUMS" || exit 24
elif [ "$EXPECT_UPDATE" = 1 ]; then
    exit 25
fi
test "$FARHELM_VERSION" = "$EXPECTED_VERSION" || exit 26
printf 'installer contract satisfied\\n'
''')
                sums = signed / 'SHA256SUMS'
                sums.write_text('fictional checksums\n')
                write_json(signed / 'verification.json', {
                    'version': version, 'installer_sha256': hashlib.sha256(script.read_bytes()).hexdigest(),
                    'sha256sums_sha256': hashlib.sha256(sums.read_bytes()).hexdigest()})
                home = directory / 'home'
                if update:
                    (home / 'Applications/Farhelm.app').mkdir(parents=True)

                def runner(argv, **kwargs):
                    """Give only the child its fixture home and the installer contract's expected mode."""
                    kwargs['env'] = dict(kwargs['env'], HOME=str(home), EXPECT_UPDATE=str(int(update)),
                                         EXPECTED_SUMS=str(sums), EXPECTED_VERSION=version)
                    return subprocess.run(argv, **kwargs)

                operations = Operations(value, output, runner=runner, home=home)
                if operation == 'install_previous':
                    operations.completed.update(('fixture_clean', 'verify_previous'))
                with mock.patch.object(operations, 'version', return_value={'installed': version.removeprefix('v')}):
                    result = operations.dispatch(operation)
                self.assertEqual(result['observation']['installer']['stdout'], 'installer contract satisfied\n')
                self.assertIn(operation, operations.completed)

    def test_installer_mode_does_not_inherit_another_updaters_inputs(self):
        """Ambient updater state must not turn setup into update-only or bind it to an unrelated generation."""
        ambient = {'PATH': '/fictional/bin', 'FARHELM_VERSION': 'v0.1.0',
                   'FARHELM_INSTALL_SUMS_FILE': '/other/SHA256SUMS', 'FARHELM_INSTALL_EXPECT': 'other-generation'}
        original = dict(ambient)
        for update in (False, True):
            with self.subTest(update=update):
                result = installer_environment(ambient, 'v0.32.0-rc.2', Path('/verified/SHA256SUMS'), update=update)
                self.assertEqual(result['FARHELM_VERSION'], 'v0.32.0-rc.2')
                self.assertEqual(result['PATH'], '/fictional/bin')
                self.assertNotIn('FARHELM_INSTALL_EXPECT', result)
                if update:
                    self.assertEqual(result['FARHELM_INSTALL_SUMS_FILE'], '/verified/SHA256SUMS')
                else:
                    self.assertNotIn('FARHELM_INSTALL_SUMS_FILE', result)
                self.assertEqual(ambient, original)

    def test_fixed_check_runs_only_its_selected_operation_without_native_provider(self):
        """Command-only acceptance must not spend a model turn or acquire a UI action lane."""
        self.value.update(check_id='artifact-verification', gui=False)
        operations = mock.Mock()
        operations.dispatch.return_value = {'receipt': 'managed-001-verify-candidate.json'}
        with mock.patch('sys.stdout', new_callable=io.StringIO) as stream:
            run_fixed(self.value, self.output, factory=lambda task, output: operations)
        operations.dispatch.assert_called_once_with('verify_candidate')
        rows = [json.loads(row) for row in stream.getvalue().splitlines()]
        self.assertEqual([row['type'] for row in rows], ['fixed.operation.completed', 'fixed.execution.completed'])
        self.assertEqual(rows[0]['identity']['attempt'], self.value['attempt'])
        self.assertEqual(json.loads((self.output / 'final.txt').read_text())['status'], 'pass')

    def test_fixed_executor_refuses_gui_and_stops_after_an_operation_failure(self):
        """Neither an unsupported checkpoint nor a failed command may manufacture fixed completion."""
        with self.assertRaisesRegex(RuntimeError, 'requires the native tester'):
            run_fixed(self.value, self.output, factory=lambda *args: self.fail('GUI acquired fixed commands'))
        self.value.update(check_id='artifact-verification', gui=False)
        operations = mock.Mock()
        operations.dispatch.side_effect = RuntimeError('fixture command failed')
        with mock.patch('sys.stdout', new_callable=io.StringIO) as stream:
            with self.assertRaisesRegex(RuntimeError, 'fixture command failed'):
                run_fixed(self.value, self.output, factory=lambda task, output: operations)
        self.assertEqual(stream.getvalue(), '')
        self.assertFalse((self.output / 'final.txt').exists())

    def test_fixed_installation_cannot_pass_on_the_installers_exit_alone(self):
        """A successful installer must agree with the installed marker, bundle and CLI before app startup."""
        self.value.update(check_id='install', gui=False)
        versions = {'installed': '0.30.0', 'bundle': '0.30.0', 'cli': {'stdout': 'farhelm 0.30.0\n'}}
        path = self.output / 'managed-operations.json'
        for field in ('installed', 'bundle', 'cli'):
            with self.subTest(field=field):
                wrong = dict(versions, **{field: {'stdout': 'farhelm 0.29.0\n'} if field == 'cli' else '0.29.0'})
                write_json(path, {'completed': ['install_candidate'], 'receipts': ['fixture.json'],
                    'observations': {'install_candidate': {'versions': wrong}}, 'continuity': None}, replace=True)
                with self.assertRaisesRegex(TesterError, 'installation-version'):
                    managed_result(self.value, self.output, {'status': 'pass', 'detail': 'unverified exit0'})

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

    def test_p0_native_readiness_uses_a_normal_finder_window_before_images(self):
        """Menu dismissal is a readiness premise, not an assumed effect of Escape.

A Help menu left over the Desktop made the native provider refuse a capture
before any product action. Every P0 fixture must establish the same ordinary
window/search probe without adding bonus gesture prerequisites.
"""
        for pass_id in ('p0-fresh', 'p0-installer-upgrade', 'p0-updater-upgrade'):
            with self.subTest(pass_id=pass_id):
                text = managed_instructions('native-preflight', pass_id, None)
                self.assertIn('normal file window', text)
                self.assertIn('File > New\nFinder Window', text)
                self.assertIn('Cancel secondary action', text)
                self.assertIn('Escape alone is not proof', text)
                self.assertIn('click its Search control', text)
                self.assertIn('Do not open\nHelp or Go to Folder', text)
                self.assertIn('P0 does not require drag or double-click probes', text)

    def test_override_removal_accepts_exact_native_accessibility_version(self):
        """Native version text proves the channel even when a tooltip image is unavailable.

An unpromoted candidate and the public default must remain distinguishable.
Neither generic Up to date text nor an endpoint that now names the candidate
proves override removal. Keep this contract in both P0 and bonus upgrades.
"""
        for pass_id in ('p0-updater-upgrade', 'upgrade-v0.32.0'):
            with self.subTest(pass_id=pass_id):
                text = managed_instructions('update-relaunch', pass_id, 'override')
                self.assertIn('fresh native UI text naming\nthe exact public default', text)
                self.assertIn('accessibility text or its visible tooltip', text)
                self.assertIn('A tooltip image is optional', text)
                self.assertIn('An Up to date message alone does not distinguish', text)
                self.assertIn('indistinguishable\noverride-removal observation as a gap', text)
                self.assertIn('public_latest', required_operations('update-relaunch', pass_id, 'override'))
                self.assertIn('candidate acceptance image', text)

    def test_fast_discovery_keeps_durable_evidence_without_progress_images(self):
        """A completed updater must not fail because its progress UI vanished before capture.

The delivered contract still requires a native Restart offer and independent
logs/version observations. Both P0 discovery and combined bonus upgrades must
carry it; these assertions cover the contract, not native image meaning.
"""
        for path in ('ordinary', 'override'):
            for check, pass_id in (('update-discovery', 'p0-updater-upgrade'),
                                   ('update-relaunch', 'upgrade-v0.29.0')):
                with self.subTest(path=path, check=check):
                    text = managed_instructions(check, pass_id, path)
                    self.assertIn('Capture the completed native Restart\nto update offer', text)
                    self.assertIn('An installing-state screenshot is optional', text)
                    self.assertIn('Call update_logs and versions', text)

        self.value['check_id'] = 'update-discovery'
        self.value['context']['update_path'] = 'ordinary'
        write_json(self.output / 'managed-operations.json', {
            'completed': ['update_logs', 'versions'], 'receipts': [], 'continuity': None,
            'observations': {
                'update_logs': {'available': True, 'lines': ['updater: 0.30.0 is installed; a restart finishes the update']},
                'versions': {'installed': '0.30.0', 'bundle': '0.30.0',
                             'running_versions': ['0.29.0'], 'cli': {'stdout': 'farhelm 0.30.0\n'}}}})
        self.assertEqual(managed_result(self.value, self.output, {
            'status': 'pass', 'detail': 'Native completed Restart offer observed; installation already finished.'})['status'], 'pass')

    def test_discovery_requires_installed_candidate_despite_a_ui_success_claim(self):
        """A UI success claim cannot hide an incomplete installation.

The old process may remain alive until the next checkpoint, but the installed
record, bundle and CLI must independently agree on the candidate already.
"""
        self.value['check_id'] = 'update-discovery'
        self.value['context']['update_path'] = 'ordinary'
        versions = {'installed': '0.30.0', 'bundle': '0.30.0', 'cli': {'stdout': 'farhelm 0.30.0\n'}}
        for field in ('installed', 'bundle', 'cli'):
            with self.subTest(field=field):
                wrong = dict(versions, **{field: {'stdout': 'farhelm 0.29.0\n'} if field == 'cli' else '0.29.0'})
                write_json(self.output / 'managed-operations.json', {
                    'completed': ['update_logs', 'versions'], 'receipts': [], 'continuity': None,
                    'observations': {'versions': wrong, 'update_logs': {'available': True, 'lines': []}}}, replace=True)
                with self.assertRaisesRegex(TesterError, 'installation-version'):
                    managed_result(self.value, self.output, {'status': 'pass', 'detail': 'Unverified UI claim'})

    def test_remote_update_contract_accepts_direct_action_without_inventing_a_dialog(self):
        """A product's direct Update now path must satisfy the same upgrade contract.

P0 still needs a real remote agent response and independent service evidence;
bonus upgrades still need original-process continuity. A confirmation dialog
is conditional product UI, not an additional acceptance state to manufacture.
"""
        for check, pass_id in (('remote-supervisor-agent', 'p0-updater-upgrade'),
                               ('remote-update', 'upgrade-v0.29.0')):
            with self.subTest(check=check):
                text = managed_instructions(check, pass_id, 'override')
                self.assertIn('Update now when offered', text)
                self.assertIn('confirmation if offered', text)
                self.assertIn('do not require an extra', text.lower())
                self.assertIn('absence of a confirmation is not', text.lower())
                self.assertIn('repeat a completed update', text)
                self.assertIn('remote_version', required_operations(check, pass_id, 'override'))
        self.assertIn('actual\nanswer 42', managed_instructions('remote-supervisor-agent', 'p0-updater-upgrade', 'override'))
        self.assertIn('continuity_remote_after', required_operations('remote-update', 'upgrade-v0.29.0', 'override'))

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

    def test_reused_detached_supervisor_counts_only_when_its_record_matches(self):
        """A reopened app reuses its detached supervisor, which is no longer its child.

Without the record the bonus reopen could never establish a running version.
The record counts only while an app runs and only when the live process
matches all of it: pid, program and start second. A stale record whose pid now
belongs to a terminal host or a reporter of the same versioned binary, or to a
later process, must not name the app's supervisor.
"""
        operations = self.operations()
        app = operations.home / 'Applications/Farhelm.app'
        state = operations.home / '.local/state/farhelm'
        state.mkdir(parents=True)
        program = f'{app}/Contents/Versions/0.30.0/farhelm'
        started = 'Sat Nov  1 12:00:05 2026'
        tables = {'table': (f'101 1 {app}/Contents/MacOS/farhelm-desktop\n'
                            f'102 1 {program}\n'
                            f'103 1 {app}/Contents/Versions/0.29.0/farhelm\n')}
        def runner(argv, **kwargs):
            """Answer the process table and the recorded pid's start time, never touching host processes."""
            if argv[:2] == ['/bin/ps', '-p']:
                self.assertEqual((kwargs['env']['TZ'], kwargs['env']['LC_ALL']), ('UTC', 'C'))
                return subprocess.CompletedProcess(argv, 0, (started + '\n').encode(), b'')
            return subprocess.CompletedProcess(argv, 0, tables['table'].encode(), b'')
        operations.runner = runner
        self.assertEqual(operations.version()['running_versions'], [])
        # The operation asks `ps` for UTC, so the record's second is read back with timegm.
        second = calendar.timegm(time.strptime(started, '%a %b %d %H:%M:%S %Y'))
        record = {'pid': 102, 'start': second * 1_000_000 + 123, 'program': program}
        for changed, expected in (({}, ['0.30.0']),
                                  ({'start': (second + 1) * 1_000_000}, []),
                                  ({'program': f'{app}/Contents/Versions/0.29.0/farhelm'}, []),
                                  ({'pid': 103}, [])):
            with self.subTest(changed=changed):
                (state / 'desktop-supervisor.json').write_text(json.dumps(dict(record, **changed)))
                self.assertEqual(operations.version()['running_versions'], expected)
        (state / 'desktop-supervisor.json').write_text(json.dumps(record))
        tables['table'] = tables['table'].replace('101 1 ', '101 1 /Applications/Other.app/')
        self.assertEqual(operations.version()['running_versions'], [], 'no running app, no live app supervisor')


def mac_table(*rows):
    """Render fictional `ps -o pid=,ppid=,stat=,lstart=,args=` rows in the C locale's layout."""
    return ''.join(f'{pid:>5} {ppid:>5} Ss   Sat Nov  1 12:00:{second:02d} 2026 {args}\n'
                   for pid, ppid, second, args in rows)


class TerminalTransitionTests(unittest.TestCase):
    """Carry P0's old-session terminal facts from the old fixture to the later checkpoint turns.

Each checkpoint is a separate attempt with its own operations object, as in a
real run: only the pass's managed state and its digest-bound receipts connect
them. Process tables and session rows are fictional, except where a test
executes the real Linux `/proc` scan against processes it owns. Each fictional
host's child carries a private-looking argument, so every test also shows that
no command line other than a host's socket reaches a receipt.
"""

    def setUp(self):
        """Own one pass directory, a fictional guest home and a mutable modeled process table."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.pass_dir = self.root / 'pass'
        self.pass_dir.mkdir()
        self.home = self.root / 'guest-home'
        self.state = self.home / '.local/state/farhelm'
        self.state.mkdir(parents=True)
        self.hosts = str(self.state / 'hosts')
        self.table = ''
        self.attempts = 0
        self.context = {'managed_operations': True, 'pass_directory': str(self.pass_dir),
                        'source_checkout': str(self.pass_dir / 'source'), 'signed_inputs': str(self.pass_dir / 'signed'),
                        'previous': 'v0.29.0', 'candidate': 'v0.30.0', 'ssh': {'alias': 'fixture-peer'}}
        self.addCleanup(self.assert_no_private_argument)

    def assert_no_private_argument(self):
        """No retained record of any attempt may carry a child's or another process's command line."""
        for path in self.pass_dir.rglob('*'):
            if path.is_file():
                self.assertNotIn('private-marker', path.read_text(errors='replace'), str(path))

    def host(self, pid, second, child, name='a1'):
        """Model one terminal host and its live child; only the host's socket argument may be kept."""
        return [(pid, 1, second, f'/fixture/Versions/0.29.0/farhelm internal terminal-host --socket {self.hosts}/{name}'),
                (child, pid, second, '/fixture/codex --private-marker')]

    def sessions(self, generation, new=True, kind=None, new_kind=None):
        """Write the old session's supervisor row, and the new agent session's once it exists.

`kind` and `new_kind` model the stored terminal kind; None leaves the column
out, as in a schema from before it existed.
"""
        path = self.state / 'supervisor.db'
        path.unlink(missing_ok=True)
        columns = 'id TEXT, name TEXT, generation INTEGER, captured_conversation TEXT, token TEXT'
        with_kind = kind is not None or new_kind is not None
        with contextlib.closing(sqlite3.connect(path)) as connection, connection:
            connection.execute(f'CREATE TABLE sessions ({columns}{", terminal_kind TEXT" if with_kind else ""})')
            rows = [('session-1', 'old', generation, 'conversation-1', 'private-marker', kind)]
            if new:
                rows.append(('session-2', 'new', 1, None, 'private-marker', new_kind))
            for row in rows:
                connection.execute(f'INSERT INTO sessions VALUES ({",".join("?" * (6 if with_kind else 5))})',
                                   row if with_kind else row[:5])

    def attempt(self, check_id, pass_id='p0-updater-upgrade', **context):
        """Start one checkpoint's operations, reading the pass state earlier attempts left."""
        value = task()
        value.update(check_id=check_id, pass_id=pass_id, context=dict(self.context, **context))
        self.attempts += 1
        output = self.pass_dir / f'attempt-{self.attempts}' / 'output'
        (output / 'work').mkdir(parents=True)
        def runner(argv, **kwargs):
            """Serve the current modeled process table to the fixed `ps` command only."""
            if argv[0] != '/bin/ps':
                raise AssertionError('unexpected guest command: ' + argv[0])
            return subprocess.CompletedProcess(argv, 0, self.table.encode(), b'')
        return value, output, Operations(value, output, runner=runner, home=self.home)

    def old_fixture(self, *rows, kind=None):
        """Record the old fixture's local baseline: the captured session first, then the processes."""
        self.table = mac_table(*rows)
        self.sessions(generation=1, new=False, kind=kind)
        _, _, operations = self.attempt('old-fixture')
        operations.dispatch('inspect_state')
        return operations.dispatch('terminal_hosts')['observation']['hosts']

    def agent_check(self, *rows, new_kind=None):
        """Record the new local session's check: the session exists, then its terminal is sampled."""
        self.table = mac_table(*rows)
        self.sessions(generation=1, new_kind=new_kind)
        value, output, operations = self.attempt('local-supervisor-agent')
        operations.dispatch('terminal_hosts')
        return value, output, operations

    def old_session(self, before, after, generation, before_generation=1, kind=None, new_kind=None):
        """Run the old-session turn: survival on the pre-Restart table, the final sample on the later one."""
        value, output, operations = self.attempt('local-old-session')
        operations.dispatch('prior_receipts')
        self.table = mac_table(*before)
        self.sessions(before_generation, kind=kind, new_kind=new_kind)
        survival = operations.dispatch('host_survival')['observation']
        self.table = mac_table(*after)
        self.sessions(generation, kind=kind if generation == before_generation else 'host', new_kind=new_kind)
        operations.dispatch('old_session_after')
        return value, output, survival

    def accept(self, value, output):
        """Apply the import gate to a model's pass claim for this attempt."""
        return managed_result(value, output, {'status': 'pass', 'detail': 'fictional UI observation'})

    def test_host_branch_requires_the_same_host_child_and_a_restart_onto_a_new_host(self):
        """A replacement that merely restores the screen and resumes the conversation must not pass.

The old fixture's identities reach the later turn only through retained
receipts. A changed start time on the same pid, a new child, or another pid on
the same socket name is a replacement. After the Restart the old host must be
gone and exactly one new host present: a leaked old host, or a Restart back
into tmux, is refused.
"""
        old = self.host(200, 1, 201)
        self.assertEqual(len(self.old_fixture(*old)), 1)
        new = self.host(400, 5, 401, name='b2')
        self.agent_check(*old, *new)
        restarted = self.host(500, 9, 501, name='c3')
        value, output, survival = self.old_session([*old, *new], [*new, *restarted], generation=2)
        self.assertEqual((survival['branch'], survival['survived'], survival['candidate_creates_hosts'],
                          survival['restart_required'], survival['old_session']['id']),
                         ('host', True, True, True, 'session-1'))
        self.assertEqual(self.accept(value, output)['status'], 'pass')
        for label, replacement in (('restarted host', self.host(200, 7, 201)),
                                   ('new child', [old[0], (202, 200, 1, '/fixture/codex --private-marker')]),
                                   ('other pid, same socket', self.host(210, 7, 211))):
            with self.subTest(replacement=label):
                value, output, survival = self.old_session([*replacement, *new], [*new, *restarted], generation=2)
                self.assertIs(survival['survived'], False)
                with self.assertRaisesRegex(TesterError, 'did not survive'):
                    self.accept(value, output)
        for label, after, refusal in (('old host leaked', [*old, *new, *restarted], 'still running'),
                                      ('back to tmux', new, 'did not move')):
            with self.subTest(after=label):
                value, output, _ = self.old_session([*old, *new], after, generation=2)
                with self.assertRaisesRegex(TesterError, refusal):
                    self.accept(value, output)

    def test_tmux_branch_requires_survival_then_a_move_only_when_the_candidate_creates_hosts(self):
        """The band and move are conditional on the candidate's observed hosts, never on a release version.

The old tmux session must first keep its launch generation across the upgrade:
a candidate that silently relaunched it fails. With hosts created, a pass then
needs a later generation and one host that appeared since the survival sample.
With none created, no Restart is due and the generation must stay unchanged.
"""
        self.assertEqual(self.old_fixture(), [])
        new = self.host(400, 5, 401, name='b2')
        self.agent_check(*new)
        restarted = self.host(500, 9, 501, name='c3')
        cases = ((1, [*new, *restarted], 1, 'restart and resume'),
                 (1, new, 2, 'did not move'),
                 (2, [*new, *restarted], 3, 'did not survive'),
                 (1, [*new, *restarted], 2, None))
        for before_generation, after, generation, refusal in cases:
            with self.subTest(before=before_generation, generation=generation, hosts=len(after)):
                value, output, survival = self.old_session(new, after, generation, before_generation)
                self.assertEqual((survival['branch'], survival['restart_required']), ('tmux', True))
                if refusal:
                    with self.assertRaisesRegex(TesterError, refusal):
                        self.accept(value, output)
                else:
                    self.assertEqual(self.accept(value, output)['status'], 'pass')
        self.agent_check()
        for generation, refusal in ((1, None), (2, 'relaunched')):
            with self.subTest(no_hosts=generation):
                value, output, survival = self.old_session([], [], generation)
                self.assertEqual((survival['candidate_creates_hosts'], survival['restart_required']), (False, False))
                if refusal:
                    with self.assertRaisesRegex(TesterError, refusal):
                        self.accept(value, output)
                else:
                    self.assertEqual(self.accept(value, output)['status'], 'pass')

    def test_decisions_do_not_depend_on_when_the_tester_sampled(self):
        """Out-of-order calls must end in a refusal or an unavailable check, never a pass.

An agent-check sample taken before the new session's host started must not
make the candidate look host-less: the survival comparison samples hosts
itself. Samples that would stand for the wrong moment are refused outright.
"""
        self.table = ''
        self.sessions(generation=1, new=False)
        _, _, operations = self.attempt('old-fixture')
        with self.assertRaisesRegex(RuntimeError, 'only after this attempt observed'):
            operations.dispatch('terminal_hosts')
        self.old_fixture()
        self.sessions(generation=1, new=False)
        _, _, operations = self.attempt('local-supervisor-agent')
        with self.assertRaisesRegex(RuntimeError, 'only after the new agent session exists'):
            operations.dispatch('terminal_hosts')
        _, output, operations = self.agent_check()
        self.assertEqual(operations.observations['terminal_hosts']['new_hosts'], [])
        _, _, operations = self.attempt('local-old-session')
        with self.assertRaisesRegex(RuntimeError, 'before observing the old session afterwards'):
            operations.dispatch('old_session_after')
        new = self.host(400, 5, 401, name='b2')
        restarted = self.host(500, 9, 501, name='c3')
        value, output, survival = self.old_session(new, [*new, *restarted], generation=2)
        self.assertIs(survival['candidate_creates_hosts'], True)
        self.assertEqual(self.accept(value, output)['status'], 'pass')
        value, output, survival = self.old_session([*new, *restarted], [*new, *restarted], generation=2,
                                                   before_generation=2)
        self.assertIs(survival['survived'], False, 'a comparison made after the Restart cannot pass')
        self.sessions(generation=1, new=False)
        _, _, operations = self.attempt('local-old-session')
        with self.assertRaisesRegex(RuntimeError, 'new agent session from the agent check is gone'):
            operations.dispatch('host_survival')

    def test_stored_terminal_kind_cross_checks_the_sampled_processes(self):
        """Where the schema stores the kind, it must agree with the hosts the operations saw.

A new session stored as `host` with no new host process, an old session stored
as `host` that the old fixture saw in tmux, or a Restart whose row still says
`tmux`, all refuse. The new-session check's own sample is compared too.
"""
        self.old_fixture(kind='tmux')
        _, _, operations = self.agent_check(new_kind='host')
        with self.assertRaisesRegex(ValueError, 'terminal_kind'):
            require_agent_terminal(operations.observations['terminal_hosts'])
        new = self.host(400, 5, 401, name='b2')
        restarted = self.host(500, 9, 501, name='c3')
        value, output, survival = self.old_session([], [], 1, kind='tmux', new_kind='host')
        self.assertTrue(survival['kind_disagreements'])
        with self.assertRaisesRegex(TesterError, 'terminal_kind'):
            self.accept(value, output)
        value, output, survival = self.old_session(new, [*new, *restarted], 2, kind='host', new_kind='host')
        with self.assertRaisesRegex(TesterError, "old session stores terminal_kind 'host'"):
            self.accept(value, output)
        value, output, _ = self.old_session(new, [*new, *restarted], 2, kind='tmux', new_kind='host')
        self.assertEqual(self.accept(value, output)['status'], 'pass')
        # A new host appeared, but the restarted row still says tmux.
        _, _, operations = self.attempt('local-old-session')
        self.table = mac_table(*new)
        self.sessions(1, kind='tmux', new_kind='host')
        operations.dispatch('host_survival')
        self.table = mac_table(*new, *restarted)
        self.sessions(2, kind='tmux', new_kind='host')
        operations.dispatch('old_session_after')
        self.assertEqual(operations.observations['old_session_after']['session']['terminal_kind'], 'tmux')
        with self.assertRaisesRegex(ValueError, 'did not move'):
            require_old_session(operations.observations, 'local')

    def test_survival_is_compared_once_before_the_restart_replaces_the_host(self):
        """A second comparison after Restart would see the legitimate replacement and overwrite the one that counts."""
        old = self.host(200, 1, 201)
        self.old_fixture(*old)
        self.agent_check(*old)
        _, _, operations = self.attempt('local-old-session')
        self.table = mac_table(*old)
        operations.dispatch('host_survival')
        with self.assertRaisesRegex(RuntimeError, 'cannot be replayed'):
            operations.dispatch('host_survival')
        self.assertIsNotNone(operations.failure)

    def test_old_fixture_baseline_must_be_unambiguous_and_consistent_with_source(self):
        """Two hosts, a childless host, or a host the previous source cannot create would make survival meaningless."""
        old = self.host(200, 1, 201)
        cases = ((mac_table(*old, *self.host(210, 2, 211, name='z9')), {}),
                 (mac_table(old[0]), {}),
                 (mac_table(*old), {'previous_can_create_hosts': False}))
        self.sessions(generation=1, new=False)
        for table, context in cases:
            with self.subTest(context=context, rows=table.count('\n')):
                self.table = table
                _, _, operations = self.attempt('old-fixture', **context)
                operations.dispatch('inspect_state')
                with self.assertRaises(RuntimeError):
                    operations.dispatch('terminal_hosts')
                self.assertIsNotNone(operations.failure)
        self.table = mac_table(*old)
        _, _, operations = self.attempt('old-fixture', previous_can_create_hosts=True)
        operations.dispatch('inspect_state')
        self.assertEqual(len(operations.dispatch('terminal_hosts')['observation']['hosts']), 1)

    def test_old_fixture_pass_needs_one_captured_session_per_host(self):
        """Restart is only offered after capture, so an uncaptured old session cannot carry the later checks."""
        value = dict(task(), check_id='old-fixture', pass_id='p0-updater-upgrade', context=self.context)
        output = self.root / 'old-fixture-output'
        output.mkdir()
        completed = sorted(required_operations('old-fixture', 'p0-updater-upgrade', 'ordinary'))
        versions = {'installed': '0.29.0', 'bundle': '0.29.0', 'desktop_running': True,
                    'running_versions': ['0.29.0'], 'cli': {'stdout': 'farhelm 0.29.0\n'}}
        def record(remote_sessions):
            """Write the import record with the given remote session rows and one captured local row."""
            write_json(output / 'managed-operations.json', {
                'completed': completed, 'receipts': [], 'continuity': None, 'observations': {
                    'versions': versions,
                    'inspect_state': {'supervisor.db': {'sessions': [{'id': 's', 'captured_conversation': 'c'}]}},
                    'remote_inspect_state': {'supervisor.db': {'sessions': remote_sessions}}}}, replace=True)
        for remote in ([], [{'id': 'r', 'captured_conversation': None}],
                       [{'id': 'r', 'captured_conversation': 'c'}, {'id': 'x', 'captured_conversation': 'd'}]):
            with self.subTest(remote=remote):
                record(remote)
                with self.assertRaisesRegex(TesterError, 'old-session capture'):
                    managed_result(value, output, {'status': 'pass', 'detail': 'fictional'})
        record([{'id': 'r', 'captured_conversation': 'c'}])
        self.assertEqual(managed_result(value, output, {'status': 'pass', 'detail': 'fictional'})['status'], 'pass')

    def test_contract_confines_samples_to_their_checkpoints_and_floors_long_turns(self):
        """Only the updater fixture records old sessions, and only old-session turns compare and sample afterwards.

A comparison offered elsewhere could be run at the wrong moment; a baseline
demanded of the installer fixture would hold it to sessions it never starts.
The checkpoints that wait on genuine agent turns get a longer turn than the
default, never past the tester's maximum.
"""
        baseline = {'inspect_state', 'remote_inspect_state', 'terminal_hosts', 'remote_terminal_hosts'}
        self.assertLessEqual(baseline, required_operations('old-fixture', 'p0-updater-upgrade', 'ordinary'))
        self.assertFalse(baseline & required_operations('old-fixture', 'p0-installer-upgrade', 'manual'))
        self.assertFalse(baseline & required_operations('old-fixture', 'upgrade-v0.29.0', 'manual'))
        once = {'host_survival', 'remote_host_survival', 'old_session_after', 'remote_old_session_after'}
        for check in ('old-fixture', 'local-supervisor-agent', 'remote-supervisor-agent', 'reopen'):
            self.assertFalse(once & available_operations(check, 'p0-updater-upgrade', 'ordinary'))
        self.assertEqual(required_operations('local-old-session', 'p0-updater-upgrade', 'ordinary'),
                         {'prior_receipts', 'host_survival', 'old_session_after'})
        self.assertEqual(required_operations('remote-old-session', 'p0-updater-upgrade', 'ordinary'),
                         {'prior_receipts', 'remote_host_survival', 'remote_old_session_after'})
        self.assertFalse({'remote_host_survival', 'remote_old_session_after'}
                         & available_operations('local-old-session', 'p0-updater-upgrade', 'ordinary'))
        self.assertEqual(turn_timeout('old-fixture', 'p0-updater-upgrade', 1200), 2400)
        self.assertEqual(turn_timeout('old-fixture', 'p0-installer-upgrade', 1200), 1200)
        self.assertEqual(turn_timeout('local-old-session', 'p0-updater-upgrade', 3000), 3000)
        self.assertEqual(turn_timeout('old-fixture', 'p0-updater-upgrade', 3600), 3600)

    def test_mac_process_rows_keep_only_hosts_in_this_state_directory_and_their_children(self):
        """A socket elsewhere, a nested path or a zombie child is not the session's host; odd lines are counted.

A line that does not parse, such as a command line continued on a new line,
must not end the observation.
"""
        table = mac_table(*self.host(200, 1, 201),
                          (300, 1, 3, '/fixture/farhelm internal terminal-host --socket /tmp/other/hosts/a1'),
                          (301, 300, 3, '-zsh'),
                          (310, 1, 4, f'/fixture/farhelm --socket {self.hosts}/nested/a1'))
        table += '  202   200 Z    Sat Nov  1 12:00:01 2026 (zsh)\n'
        table += 'continued prompt line one two three four five six seven eight\n'
        table += 'x\n'
        rows, unparsed = parse_mac_processes(table)
        self.assertEqual(unparsed, 2)
        self.assertNotIn('private-marker', json.dumps(rows))
        self.assertEqual(select_hosts(rows, self.hosts),
                         [{'pid': 200, 'start': 'Sat Nov 1 12:00:01 2026', 'socket': self.hosts + '/a1',
                           'children': [{'pid': 201, 'start': 'Sat Nov 1 12:00:01 2026'}]}])

    def test_failed_process_scan_retains_no_command_line(self):
        """A failed `ps` can print other processes' arguments; only its status and safe diagnostics may be kept.

The gate still closes, and the failure receipt says how much output was
withheld, so the attempt stays diagnosable without the private table.
"""
        self.sessions(generation=1, new=False)
        value, output, operations = self.attempt('old-fixture')
        operations.dispatch('inspect_state')
        def failing(argv, **kwargs):
            """Return a partial table holding a private argument, with a failing status."""
            return subprocess.CompletedProcess(argv, 1, mac_table(*self.host(200, 1, 201)).encode(),
                                               b'ps: fixture failure\n')
        operations.runner = failing
        with self.assertRaisesRegex(RuntimeError, 'exited 1'):
            operations.dispatch('terminal_hosts')
        self.assertIsNotNone(operations.failure)
        retained = [json.loads(path.read_text()) for path in (output / 'work').glob('managed-*-failed-command.json')]
        self.assertEqual(len(retained), 1)
        self.assertEqual(retained[0]['returncode'], 1)
        self.assertIn('withheld', retained[0]['stdout'])
        self.assertIn('fixture failure', retained[0]['stderr'])

    def test_other_old_fixtures_sample_hosts_as_a_plain_diagnostic(self):
        """Only the updater pair's old fixture is a baseline; an unprompted sample elsewhere must not fail the attempt.

The installer fixture and bonus upgrades start no session at their old
fixture, so neither the capture premise nor the one-host rule applies there.
"""
        self.table = mac_table(*self.host(200, 1, 201), *self.host(210, 2, 211, name='z9'))
        for pass_id in ('p0-installer-upgrade', 'upgrade-v0.29.0'):
            with self.subTest(pass_id=pass_id):
                _, _, operations = self.attempt('old-fixture', pass_id=pass_id, previous_can_create_hosts=False)
                self.assertEqual(len(operations.dispatch('terminal_hosts')['observation']['hosts']), 2)
                self.assertIsNone(operations.failure)

    def start_host(self, name):
        """Start an owned stand-in terminal host with one child, and return it with the child's pid.

The host prints its child's pid only after that child has exec'd, so a scan
cannot see a fork still carrying the host's own arguments. Closing the host's
stdin ends both; cleanup tolerates a host the test already stopped.
"""
        code = ('import subprocess,sys\n'
                'c=subprocess.Popen([sys.executable,"-c","import sys;print(\'ready\',flush=True);sys.stdin.read()",'
                '"--private-marker"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)\n'
                'c.stdout.readline()\n'
                'print(c.pid,flush=True)\n'
                'sys.stdin.read()\n'
                'c.stdin.close()\n'
                'c.wait(timeout=10)\n')
        host = subprocess.Popen([sys.executable, '-c', code, '--socket', self.hosts + '/' + name],
                                stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop_host, host)
        if not select.select([host.stdout], [], [], 30)[0]:
            self.fail('owned fixture host did not report its child within 30 seconds')
        return host, int(host.stdout.readline())

    def stop_host(self, host):
        """End an owned stand-in host and its child through stdin, with a bounded wait."""
        if not host.stdin.closed:
            host.stdin.close()
        host.wait(timeout=10)
        if not host.stdout.closed:
            host.stdout.close()

    @unittest.skipUnless(sys.platform == 'linux', 'the remote scan reads Linux /proc')
    def test_remote_side_runs_end_to_end_on_real_linux_processes(self):
        """Drive the remote baseline, new-session sample, survival comparison and final sample over the modeled peer.

The modeled SSH transport runs the exact script texts locally against owned
processes and an explicit child-only HOME, so the remote operation names, the
remote session rows and the private-output path all compose as a real run
would use them. A real process whose last arguments are `--socket` and a path
in the state directory's hosts/ is found with its one real child and kernel
start identities; no child's command line leaves the modeled peer; the
session-row script drops columns outside the contract. A host-branch Restart
onto a new host, with the old host gone, passes the import's check.
"""
        old, child = self.start_host('r1')
        self.sessions(generation=3, new=False)
        transported = []
        def runner(argv, **kwargs):
            """Run the pinned-peer script text with this test's fictional home, never the harness environment."""
            self.assertEqual(argv[0], '/usr/bin/ssh')
            result = subprocess.run([sys.executable, '-'], input=kwargs['input'], capture_output=True,
                                    timeout=30, env={'HOME': str(self.home), 'PATH': os.defpath})
            transported.append(result.stdout)
            return result
        def attempt(check_id):
            """Start one remote checkpoint's operations on the shared pass state."""
            value = dict(task(), check_id=check_id, pass_id='p0-updater-upgrade', context=self.context)
            self.attempts += 1
            output = self.pass_dir / f'remote-{self.attempts}/output'
            (output / 'work').mkdir(parents=True)
            return Operations(value, output, runner=runner, home=self.root / 'mac-home')
        operations = attempt('old-fixture')
        rows = operations.dispatch('remote_inspect_state')['observation']['supervisor.db']['sessions']
        self.assertEqual(rows, [{'id': 'session-1', 'name': 'old', 'generation': 3,
                                 'captured_conversation': 'conversation-1'}])
        hosts = operations.dispatch('remote_terminal_hosts')['observation']['hosts']
        self.assertEqual([(entry['pid'], entry['socket']) for entry in hosts], [(old.pid, self.hosts + '/r1')])
        self.assertEqual([entry['pid'] for entry in hosts[0]['children']], [child])
        self.assertEqual(set(hosts[0]['start']), {'start_ticks', 'boot_id'})
        new, _ = self.start_host('r2')
        self.sessions(generation=3)
        operations = attempt('remote-supervisor-agent')
        sampled = operations.dispatch('remote_terminal_hosts')['observation']
        self.assertEqual([entry['pid'] for entry in sampled['new_hosts']], [new.pid])
        self.assertEqual([row['id'] for row in sampled['new_sessions']], ['session-2'])
        operations = attempt('remote-old-session')
        operations.dispatch('prior_receipts')
        survival = operations.dispatch('remote_host_survival')['observation']
        self.assertEqual((survival['branch'], survival['survived'], survival['candidate_creates_hosts']),
                         ('host', True, True))
        self.stop_host(old)
        restarted, _ = self.start_host('r3')
        self.sessions(generation=4)
        after = operations.dispatch('remote_old_session_after')['observation']
        self.assertEqual([entry['pid'] for entry in after['appeared_hosts']], [restarted.pid])
        self.assertFalse(after['old_host_present'])
        require_old_session(operations.observations, 'remote')
        self.assertTrue(all(b'private-marker' not in raw for raw in transported))


if __name__ == '__main__':
    unittest.main()
