"""Translate single UI actions to the official native provider, with owned restarts.

The model still chooses controls from fresh native state. It cannot batch a
restart with a read of the disappearing process, write JavaScript, or launch an
app to hide a failed restart. All returned state/images remain provider output.
"""

import json
import math
import time

from window_geometry import image_metadata


METHODS = {'getAXState': (0, 1), 'getScreenshot': (0, 1), 'getAXStateAndScreenshot': (0, 1),
           'click': (1, 2), 'drag': (2, 2), 'scroll': (2, 3), 'selectText': (2, 3),
           'setValue': (2, 2), 'performSecondaryAction': (2, 2), 'paste': (1, 2),
           'pressKey': (1, 2)}


def wait_running(operations, expected, *, label, previous_pids=None, clock=time.monotonic, pause=time.sleep):
    """Establish process readiness after an owned launch or restart without another UI action."""
    deadline = clock() + 60
    while True:
        observed = operations.version()
        changed = previous_pids is None or (observed.get('desktop_pids')
                    and set(observed['desktop_pids']).isdisjoint(previous_pids))
        if observed['desktop_running'] and observed['running_versions'] == [expected] and changed:
            return observed
        if clock() >= deadline:
            operations.receipt(label + '-timeout', observed)
            raise RuntimeError('app did not return a running expected version before native binding')
        pause(0.25)


def tool_definition():
    """Expose documented native methods as data, never model-authored executable code."""
    return {'name': 'brick_ui', 'description': '''Perform ONE genuine guest-native UI operation.
Use target desktop/method getState/args [] for inventory and first-use native docs.
Use target finder or farhelm/method bind/args [] to bind that existing guest app.
Then use the documented app method name and a positional args array, e.g.
{"target":"farhelm","method":"click","args":[8]} or getAXState with
[{"disableDiffing":true}]. Each request performs one call; read fresh state after
input. No JavaScript or arbitrary app names. Use paste for exact text entry;
typeText is not exposed because partial text delivery was observed. Verify the
reactive field value before submitting. For the actual Restart to update
button first capture its visible menu, then use method restart with its center
coordinates as args [[x,y]], never ordinary click or an accessibility index.
That action clicks once, waits for the candidate process, and rebinds only after
observing it running. It never launches an absent Farhelm. Native images are
returned unchanged. An AX-plus-screenshot request can return only menu AX text:
that is no captured image. Dismiss the menu normally and request the actual
acceptance screenshot. Tool/permission failures stop the attempt.''',
        'inputSchema': {'type': 'object', 'properties': {
            'target': {'type': 'string', 'enum': ['desktop', 'finder', 'farhelm']},
            'method': {'type': 'string', 'enum': sorted(set(METHODS) | {'getState', 'bind', 'restart'})},
            'args': {'type': 'array', 'items': {}}},
            'required': ['target', 'method', 'args'], 'additionalProperties': False}}


class NativeUI:
    """Own binding and the process handoff while leaving visual judgment to the tester."""

    def __init__(self, operations, clock=time.monotonic, pause=time.sleep):
        self.operations = operations
        self.bound = set()
        self.restarted = False
        self.clock = clock
        self.pause = pause
        self.before_restart = None
        self.image_sequence = 0

    def prepare(self, message):
        """Compile one validated native call; JSON arguments cannot add executable statements."""
        parameters = message['params']['arguments']
        if set(parameters) != {'target', 'method', 'args'}:
            raise RuntimeError('brick_ui accepts only target, method and positional args')
        target, method, args = (parameters[key] for key in ('target', 'method', 'args'))
        if target not in ('desktop', 'finder', 'farhelm') or not isinstance(args, list):
            raise RuntimeError('unsupported native target or arguments')
        if len(json.dumps(args)) > 32768:
            raise RuntimeError('native arguments exceed their bound')
        variable = 'brickFinder' if target == 'finder' else 'brickFarhelm'
        if method == 'getState' and target == 'desktop' and not args:
            code = 'await cua.getState();'
        elif method == 'bind' and target != 'desktop' and not args:
            code = self.bind_code(target)
        elif method == 'restart' and target == 'farhelm' and len(args) == 1:
            point = args[0]
            if (not isinstance(point, list) or len(point) != 2 or any(
                    type(value) not in (int, float) or not math.isfinite(value) or value < 0 for value in point)):
                raise RuntimeError('restart requires coordinates from the visible native menu image')
            if self.operations.task['check_id'] != 'update-relaunch' or self.restarted:
                raise RuntimeError('native restart is not applicable or was already attempted')
            if target not in self.bound:
                raise RuntimeError('bind the running old app before selecting Restart')
            self.before_restart = self.operations.version()
            previous = self.operations.context['previous'].removeprefix('v')
            if (self.before_restart['running_versions'] != [previous]
                    or not self.before_restart.get('desktop_pids')):
                raise RuntimeError('the old process is absent before the updater restart')
            self.operations.receipt('restart-before', self.before_restart)
            self.restarted = True
            # The window disappears during this action. Use a point from its
            # native image, avoiding another AX element lookup at teardown.
            code = f'await {variable}.click({json.dumps(point)});'
        else:
            bounds = METHODS.get(method)
            if target == 'desktop' or target not in self.bound or bounds is None or not bounds[0] <= len(args) <= bounds[1]:
                raise RuntimeError('native method needs a bound app and its documented arguments')
            code = f'await {variable}.{method}({", ".join(json.dumps(value) for value in args)});'
        return dict(message, params={'name': 'js', 'arguments': {'code': code, 'title': f'{target}: {method}'}})

    def bind_code(self, target):
        """Refuse to use getApp as a launch fallback after a missing product restart."""
        if target == 'farhelm':
            if not self.operations.version()['desktop_running']:
                raise RuntimeError('Farhelm is absent; native binding cannot launch it')
            selector = str(self.operations.home / 'Applications/Farhelm.app')
            variable = 'brickFarhelm'
        else:
            selector, variable = 'com.apple.finder', 'brickFinder'
        return f'var {variable} = await cua.getApp({json.dumps(selector)});'

    def complete(self, parameters, response):
        """Bind geometry to actual image bytes, including during bonus fixture readiness.

The provider response stays unchanged. Some menu states return AX without an
image; that is an observation, not a failed tool. Clear the image association
so frame evidence cannot silently borrow a previous screenshot's calibration.
"""
        if parameters['method'] == 'bind':
            self.bound.add(parameters['target'])
        task = self.operations.task
        needs_geometry = task['check_id'] == 'window-drag' or (
            task['check_id'] == 'native-preflight' and not task['pass_id'].startswith('p0-'))
        if (needs_geometry and parameters['target'] != 'desktop'
                and parameters['method'] in ('getScreenshot', 'getAXStateAndScreenshot')):
            images = [block for block in response.get('result', {}).get('content', []) if block.get('type') == 'image']
            if not images:
                self.operations.latest_images[parameters['target']] = None
                return
            if len(images) != 1:
                raise RuntimeError('native app capture returned ambiguous images')
            self.image_sequence += 1
            self.operations.latest_images[parameters['target']] = image_metadata(images[0], self.image_sequence)

    def settle_restart(self, message):
        """Wait on the actual candidate process before asking the native provider for its UI.

The click is never retried. Reading AX state while the old process disappears
can time out inside the provider, so readiness is a command-side process oracle.
Rebinding happens only after observing an already-running candidate, preserving
the acceptance requirement that the updater itself brought the app back.
"""
        candidate = self.operations.context['candidate'].removeprefix('v')
        after = wait_running(self.operations, candidate, label='restart',
                             previous_pids=self.before_restart['desktop_pids'], clock=self.clock, pause=self.pause)
        self.operations.receipt('restart-transition', {'before': self.before_restart, 'after': after})
        return dict(message, params={'name': 'js', 'arguments': {
            'code': self.bind_code('farhelm'), 'title': 'Bind the observed running candidate after Restart'}})
