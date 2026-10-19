"""Exercise native action construction and the process boundary around Restart.

The fake provider proves call ordering, never Farhelm behavior. Actual release
acceptance still needs its genuine provider event stream and inspected images.
"""

import io
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

from guest_operations import Operations, serve
from guest_ui import NativeUI
from test_guest_tester import task


def ui_call(identifier, target, method, args):
    """Build the structured client envelope without executable model text."""
    return {'jsonrpc': '2.0', 'id': identifier, 'method': 'tools/call',
            'params': {'name': 'brick_ui', 'arguments': {'target': target, 'method': method, 'args': args}}}


def version(value, running=True):
    """Distinguish the actual executable from candidate bytes already installed on disk."""
    return {'installed': '0.30.0', 'bundle': '0.30.0', 'desktop_running': running,
            'running_versions': [value] if running else [],
            'desktop_pids': [101 if value == '0.29.0' else 202] if running else []}


class NativeUITests(unittest.TestCase):
    """A restart crosses processes under harness control, never inside an arbitrary UI script."""

    def setUp(self):
        """Keep all receipts in an owned fictional pass with injected process observations."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / 'pass/attempt/output'
        (self.output / 'work').mkdir(parents=True)
        self.task = task()
        self.task.update(check_id='update-relaunch', pass_id='p0-installer-upgrade')
        self.task['context'] = {'pass_directory': str(self.root / 'pass'),
                                'candidate': 'v0.30.0', 'previous': 'v0.29.0'}
        self.operations = Operations(self.task, self.output, home=self.root / 'guest')

    def test_native_text_is_an_argument_and_cannot_batch_another_action(self):
        """Punctuation in entered text cannot become a second JavaScript statement or native input."""
        ui = NativeUI(self.operations)
        ui.bound.add('farhelm')
        text = 'hello"); await brickFarhelm.click(99); //'
        request = ui.prepare(ui_call(1, 'farhelm', 'paste', [text]))
        self.assertEqual(request['params']['arguments']['code'],
                         'await brickFarhelm.paste(' + json.dumps(text) + ');')
        with self.assertRaisesRegex(RuntimeError, 'documented arguments'):
            ui.prepare(ui_call(2, 'farhelm', 'typeText', [text]))
        with self.assertRaisesRegex(RuntimeError, 'documented arguments'):
            ui.prepare(ui_call(2, 'farhelm', 'eval', ['arbitrary code']))

    def test_absent_farhelm_cannot_be_launched_by_native_binding(self):
        """Binding after a failed restart must not turn an absent app into an apparent successful return."""
        ui = NativeUI(self.operations)
        with mock.patch.object(self.operations, 'version', return_value=version('0.29.0', running=False)):
            with self.assertRaisesRegex(RuntimeError, 'cannot launch'):
                ui.prepare(ui_call(1, 'farhelm', 'bind', []))
        self.assertEqual(ui.bound, set())

    def test_ax_only_menu_observation_cannot_reuse_an_earlier_image(self):
        """A successful menu observation may omit its image; retain it without borrowing stale calibration."""
        self.task.update(check_id='native-preflight', pass_id='fresh')
        ui = NativeUI(self.operations)
        self.operations.latest_images['finder'] = {'sequence': 1}
        response = {'result': {'content': [{'type': 'text', 'text': 'controlled menu AX state'}]}}
        ui.complete({'target': 'finder', 'method': 'getAXStateAndScreenshot', 'args': []}, response)
        self.assertIsNone(self.operations.latest_images['finder'])
        self.assertEqual(response['result']['content'][0]['text'], 'controlled menu AX state')

    def test_restart_waits_for_candidate_before_native_rebind(self):
        """A queued UI read cannot reach the disappearing old process between Restart and candidate readiness."""
        replies = [{'jsonrpc': '2.0', 'id': identifier, 'result': {'content': []}} for identifier in (1, 2, 2, 3)]
        native = types.SimpleNamespace(stdin=io.StringIO(), stdout=io.StringIO(
            ''.join(json.dumps(reply) + '\n' for reply in replies)))
        calls = [ui_call(1, 'farhelm', 'bind', []), ui_call(2, 'farhelm', 'restart', [[150, 80]]),
                 ui_call(3, 'farhelm', 'getAXState', [{'disableDiffing': True}])]
        responses = []
        observations = [version('0.29.0'), version('0.29.0'), version('0.30.0'), version('0.30.0')]
        with mock.patch.object(self.operations, 'version', side_effect=observations):
            serve(self.task, self.output, native, map(json.dumps, calls), responses.append, self.operations)
        self.assertFalse(any(row.get('result', {}).get('isError') for row in responses))
        codes = [json.loads(line)['params']['arguments']['code'] for line in native.stdin.getvalue().splitlines()]
        self.assertEqual(len(codes), 4)
        self.assertIn('cua.getApp', codes[0])
        self.assertEqual(codes[1], 'await brickFarhelm.click([150, 80]);')
        self.assertIn('cua.getApp', codes[2])
        self.assertIn('getAXState', codes[3])
        transition = json.loads(next((self.output / 'work').glob('*restart-transition.json')).read_text())
        self.assertEqual(transition['before']['running_versions'], ['0.29.0'])
        self.assertEqual(transition['after']['running_versions'], ['0.30.0'])

    def test_missing_candidate_refuses_without_native_rebind_or_second_click(self):
        """A restart timeout retains the last process observation and never repairs itself by launching the app."""
        ui = NativeUI(self.operations, clock=iter((0, 61)).__next__, pause=lambda value: self.fail('unbounded wait'))
        ui.bound.add('farhelm')
        call = ui_call(1, 'farhelm', 'restart', [[150, 80]])
        with mock.patch.object(self.operations, 'version', return_value=version('0.29.0')):
            ui.prepare(call)
            with self.assertRaisesRegex(RuntimeError, 'did not return'):
                ui.settle_restart(call)
            with self.assertRaisesRegex(RuntimeError, 'already attempted'):
                ui.prepare(call)
        self.assertEqual(len(list((self.output / 'work').glob('*restart-timeout.json'))), 1)

    def test_candidate_supervisor_cannot_hide_an_unchanged_gui_process(self):
        """Restart requires a new app process, not only candidate payloads or a newly started supervisor."""
        ui = NativeUI(self.operations, clock=iter((0, 61)).__next__, pause=lambda value: self.fail('unbounded wait'))
        ui.bound.add('farhelm')
        call = ui_call(1, 'farhelm', 'restart', [[150, 80]])
        unchanged = dict(version('0.30.0'), desktop_pids=[101])
        with mock.patch.object(self.operations, 'version', side_effect=[version('0.29.0'), unchanged]):
            ui.prepare(call)
            with self.assertRaisesRegex(RuntimeError, 'did not return'):
                ui.settle_restart(call)


if __name__ == '__main__':
    unittest.main()
