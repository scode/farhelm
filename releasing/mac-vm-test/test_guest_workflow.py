"""Pin setup consent and credential destinations without using real operator data."""

import json
from pathlib import Path
import tempfile
import subprocess
import os
import sys
import types
import unittest

from control import Refused
from brick_test import Source
from guest_workflow import (copy_credentials, instructions, managed_instructions, require_setup, source_url,
                            task_for)
from test_guest_tester import task


class CheckpointInstructionsTests(unittest.TestCase):
    """Inspect the actual guest envelopes where update-path actions are assigned.

Bonus upgrades combine discovery and relaunch in one checkpoint. P0 gives
discovery its own checkpoint, so duplicating that action can invalidate the
independent updater check. These assertions cover instructions, not live UI.
"""

    def envelope(self, pass_id, check_id, update_path, managed=False):
        """Supply fictional run context to the production task builder without a VM."""
        run = types.SimpleNamespace(
            selected=lambda name: {'required': {check_id: {'gui': True}}},
            data={'run_id': 'fixture-run'},
            profile=types.SimpleNamespace(values={'guest_tester': {'settings': task()['settings']}}))
        return task_for(run, pass_id, check_id,
                        {'pass_directory': '/fixture/pass', 'update_path': update_path, 'managed_operations': managed})

    def test_bonus_discovery_precedes_restart_for_native_update_paths(self):
        """Disabled auto updates cannot create a Restart offer until the tester checks and installs."""
        for update_path in ('ordinary', 'override'):
            with self.subTest(path=update_path):
                text = self.envelope('upgrade-v0.24.0', 'update-relaunch', update_path)['instructions']
                self.assertIn('FARHELM_DESKTOP_UPDATE_LATEST=candidate', text)
                self.assertLess(text.index("Click the old app's actual Check for updates"),
                                text.index("wait on the old app's real Restart to update offer"))
                self.assertIn('Require discovery and installation of candidate', text)

    def test_manual_and_p0_relaunch_keep_their_existing_action_boundaries(self):
        """Manual baselines use the installer, while P0 must not repeat its separate discovery."""
        for pass_id, update_path in (('upgrade-v0.24.0', 'manual'),
                                    ('p0-installer-upgrade', 'manual'),
                                    ('p0-updater-upgrade', 'ordinary'),
                                    ('p0-updater-upgrade', 'override')):
            with self.subTest(pass_id=pass_id, path=update_path):
                text = self.envelope(pass_id, 'update-relaunch', update_path)['instructions']
                self.assertIn('For manual path only, run the verified candidate installer', text)
                self.assertNotIn("Click the old app's actual Check for updates", text)
        discovery = self.envelope('p0-updater-upgrade', 'update-discovery', 'ordinary')['instructions']
        self.assertIn("Click the old app's actual Check for updates", discovery)

    def test_session_launches_use_the_same_model_as_the_guest_tester(self):
        """A model change must carry through the CLI envelope and both releases' native launch instructions."""
        for check_id in ('old-sessions', 'local-supervisor-agent', 'remote-supervisor-agent',
                         'codex-conversation', 'seed-state'):
            for managed in (False, True):
                with self.subTest(check=check_id, managed=managed):
                    envelope = self.envelope('p0-installer-upgrade', check_id, 'manual', managed=managed)
                    model = envelope['settings']['model']
                    self.assertIn('exact model ' + model, envelope['instructions'])
                    self.assertIn('custom model ID', envelope['instructions'])
                    self.assertNotIn('gpt-6.1-sol', envelope['instructions'])

    def test_manual_installer_waits_for_native_restart_readiness(self):
        """The installer can finish before the old app replaces its Check for updates control."""
        text = self.envelope('p0-installer-upgrade', 'update-relaunch', 'manual', managed=True)['instructions']
        self.assertLess(text.index('Call install_candidate'), text.index('keep observing'))
        self.assertLess(text.index('keep observing'), text.index('clicking the readout itself'))
        self.assertLess(text.index('Capture the visible menu'), text.index('brick_ui method restart'))

    def test_p0_relaunches_leave_the_local_supervisor_to_its_own_checkpoint(self):
        """Neither P0 relaunch may wait on, or Update, the supervisor the installer pair judges separately.

Bonus upgrades still wait for the connected candidate supervisor.
"""
        for pass_id, path in (('p0-installer-upgrade', 'manual'), ('p0-updater-upgrade', 'ordinary')):
            with self.subTest(pass_id=pass_id):
                text = self.envelope(pass_id, 'update-relaunch', path, managed=True)['instructions']
                self.assertIn("checked by P0's local-supervisor-upgrade checkpoint (on the installer pair): "
                              'do not wait for it or use Update here', ' '.join(text.split()))
                self.assertNotIn('wait for the connected candidate', text)
        text = self.envelope('upgrade-v0.24.0', 'update-relaunch', 'manual', managed=True)['instructions']
        self.assertIn('wait for the connected candidate\nlocal supervisor', text)

    # The startup checkpoints, split by which release the tester faces when
    # the app opens. app-start is the candidate's first open after a fresh
    # install (P0, the Mac P1 pair and the bonus fresh pass). update-relaunch
    # is the candidate after Restart to update, on every update path the P0
    # and bonus upgrade passes take. old-fixture is the previous release's
    # first open on every upgrade pass.
    CANDIDATE_STARTS = (('p0-fresh', 'app-start', None), ('p1-remote-mac', 'app-start', None),
                        ('fresh', 'app-start', None), ('p0-installer-upgrade', 'update-relaunch', 'manual'),
                        ('p0-updater-upgrade', 'update-relaunch', 'ordinary'),
                        ('p0-updater-upgrade', 'update-relaunch', 'override'),
                        ('upgrade-v0.24.0', 'update-relaunch', 'manual'),
                        ('upgrade-v0.24.0', 'update-relaunch', 'ordinary'),
                        ('upgrade-v0.24.0', 'update-relaunch', 'override'))
    BASELINE_STARTS = (('p0-installer-upgrade', 'old-fixture', 'manual'),
                       ('p0-updater-upgrade', 'old-fixture', 'ordinary'),
                       ('upgrade-v0.24.0', 'old-fixture', 'manual'))

    def test_only_a_previous_release_start_may_dismiss_a_welcome_card(self):
        """Candidate starts never mention a Welcome card; a previous release's start dismisses one only if it appears.

PRs #5012 and #5018 replaced the startup Welcome card, dismissed with Got it,
with a Welcome view inline in the empty main pane, so a candidate has no card
to dismiss and a tester told to dismiss one is sent after a control that is not
there. A previous release used as the upgrade baseline may predate the card,
show it, or postdate it, so its dismissal must stay conditional. Both envelopes
are checked, since each carries its own copy of the startup text. These
assertions cover the delivered instructions, not live product behavior.
"""
        for managed in (False, True):
            for pass_id, check_id, update_path in self.CANDIDATE_STARTS:
                with self.subTest(pass_id=pass_id, check=check_id, path=update_path, managed=managed):
                    text = self.envelope(pass_id, check_id, update_path, managed=managed)['instructions'].lower()
                    self.assertNotIn('welcome', text)
                    self.assertNotIn('got it', text)
            for pass_id, check_id, update_path in self.BASELINE_STARTS:
                with self.subTest(pass_id=pass_id, check=check_id, path=update_path, managed=managed):
                    text = ' '.join(
                        self.envelope(pass_id, check_id, update_path, managed=managed)['instructions'].split()).lower()
                    conditional = ('dismiss welcome with got it if present' if managed else
                                   'if the ordinary welcome card appears, dismiss it with got it')
                    self.assertEqual(text.count(conditional), 1)
                    # Every remaining mention of the card must be that
                    # conditional one; an unconditional dismissal beside it
                    # would still fail on a baseline without the card.
                    self.assertNotIn('got it', text.replace(conditional, ''))

    def test_every_plain_startup_keeps_the_tester_off_send_feedback(self):
        """The plain envelope's startup checkpoints all keep the tester from sending feedback before P2.

The live feedback send is P2's own authorized checkpoint and cannot be
recalled, while the candidate's inline Welcome view, like the older card,
offers Send feedback. Removing the card dismissal from the candidate starts
must not take this guard with it. Only the plain envelope carries this
sentence: the managed one, which live runs use, limits feedback on old-fixture
alone ("No session or feedback yet", pinned by OldSessionInstructionsTests),
and has no such guard on the candidate starts.
"""
        for pass_id, check_id, update_path in self.CANDIDATE_STARTS + self.BASELINE_STARTS:
            with self.subTest(pass_id=pass_id, check=check_id, path=update_path):
                text = self.envelope(pass_id, check_id, update_path)['instructions']
                self.assertEqual(text.count('Do not select Send feedback now or begin P2 early.'), 1)

    def test_remote_agent_prompt_is_entered_in_the_visible_terminal(self):
        """Keep the tester away from the launcher field that may be offscreen after remote Update."""
        text = self.envelope('p0-installer-upgrade', 'remote-supervisor-agent', 'manual', managed=True)['instructions']
        self.assertIn("Leave the launcher's optional Initial prompt blank", text)
        self.assertIn('prompt in the visible agent terminal', text)
        self.assertLess(text.index('Launch first'), text.index('Send: What is 17 + 25?'))


class OldSessionInstructionsTests(unittest.TestCase):
    """Pin what P1's old-session turns are told, in both the managed and the plain envelope.

The receipts gate what the tester cannot fake, the survival of the old
session's terminal host included; these instructions carry what only the UI
can show: the Restart offer and the resumed agent remembering the earlier
turn. These assertions cover the delivered text, not live product behavior.
"""

    def test_only_old_sessions_starts_the_old_sessions(self):
        """Old sessions have their own P1 checkpoint, so no old fixture's P0 contract starts them.

Starting them happens on the previous release, before any candidate code, so
the tester is told a failure there is unavailable rather than a product fail.
"""
        for build in (managed_instructions, instructions):
            with self.subTest(builder=build.__name__):
                text = build('old-sessions', 'p0-installer-upgrade', 'manual')
                for phrase in ('ONE genuine Codex', 'ONE on the remote host', 'Remember the word bluebird',
                               'Restart\nonly after capture' if build is managed_instructions else 'captured in Farhelm',
                               'do not restart, stop or delete them', 'report could-not-run, never fail',
                               'Do not update yet'):
                    self.assertIn(phrase, text)
                for pass_id in ('p0-installer-upgrade', 'p0-updater-upgrade', 'upgrade-v0.24.0'):
                    self.assertNotIn('bluebird', build('old-fixture', pass_id, 'manual'))
        managed = managed_instructions('old-sessions', 'p0-installer-upgrade', 'manual')
        for operation in ('inspect_state', 'remote_inspect_state', 'terminal_hosts', 'remote_terminal_hosts'):
            self.assertIn(operation, managed)
        for pass_id in ('p0-installer-upgrade', 'p0-updater-upgrade', 'upgrade-v0.24.0'):
            self.assertIn('No session or feedback yet', managed_instructions('old-fixture', pass_id, 'manual'))

    def test_old_session_turns_compare_before_touching_and_ask_about_the_earlier_turn(self):
        """A host restart replaces the host, so the survival comparison must come first; any reply is not a resume.

The post-upgrade question's answer must differ from the 42 already on screen.
The agent restart must not be routed through the app updater's owned restart
transition, and the turn ends with the final sample the import requires, which
is also what checks the move onto a new terminal host. Neither envelope keeps
a path without a Restart or a terminal kind to read. A completed idle agent
restarts directly, without a working-agent confirmation.
"""
        for side, survival, final in (('local', 'host_survival', 'old_session_after'),
                                      ('remote', 'remote_host_survival', 'remote_old_session_after')):
            with self.subTest(side=side):
                text = managed_instructions(side + '-old-session', 'p0-installer-upgrade', 'manual')
                self.assertLess(text.index(f'call {survival} exactly'), text.index(f'Select the old {side} session'))
                restart = "use the session's ordinary Restart"
                self.assertLess(text.index('What is 6 times 8?'), text.index(restart))
                self.assertLess(text.index(restart), text.index('Which word did I ask you to remember'))
                self.assertLess(text.index('Which word did I ask you to remember'), text.index('Finish by calling ' + final))
                self.assertNotIn('stop after', text)
                self.assertNotIn('6 times 7', text)
                for phrase in ('only Clone or Replace', 'If survived is false',
                               'never brick_ui method restart', 'never supply it', 'earlier 17 + 25\nanswer',
                               'new answer 48', 'moved to one new terminal host with\nthe old one gone'):
                    self.assertIn(phrase, text)
                for phrase in ('confirmation only if offered', 'An idle agent\nrestarts directly',
                               'repeat a completed Restart to obtain a dialog'):
                    self.assertIn(phrase, text)
                plain = instructions(side + '-old-session', 'p0-installer-upgrade', 'manual')
                for envelope in (text, plain):
                    for phrase in ('band', 'details', 'terminal kind', 'branch', 'do not restart'):
                        self.assertNotIn(phrase, envelope)
                self.assertNotIn('brick_ui', plain)
                self.assertIn('Which word did I ask you to remember', plain)
                self.assertIn('new answer 48', plain)
                self.assertIn('confirmation only if offered', plain)
                # A survival comparison made after touching the session no longer
                # describes the upgrade: that is the tester's error, not a release failure.
                self.assertIn('touched the old session\nbefore this call: then report automation-failure', text)
                self.assertIn('automation-failure if you touched the old', plain)

    def test_agent_checks_sample_no_terminal_and_keep_the_remote_session_for_the_menu_checks(self):
        """The new-session checks decide nothing about the old session, so they take no terminal sample.

The remote one must still keep its session running: Remove must show it
preserved and Uninstall must end it, so a tester that tidied it away would
leave those checks without their premise.
"""
        for check, operation in (('local-supervisor-agent', 'terminal_hosts'),
                                 ('remote-supervisor-agent', 'remote_terminal_hosts')):
            with self.subTest(check=check):
                text = managed_instructions(check, 'p0-installer-upgrade', 'manual')
                self.assertNotIn('call ' + operation, text)
                self.assertIn('untouched', text)
                for envelope in (text, instructions(check, 'p0-installer-upgrade', 'manual')):
                    self.assertNotIn('terminal host', envelope)
                    self.assertEqual('Leave this new session running' in envelope, check.startswith('remote-'))


class WindowDragInstructionsTests(unittest.TestCase):
    """Pin the title-bar contract the guest tester receives for the full fresh pass.

The generic bonus text would leave the gestures, the frame observation and the
could-not-run boundary to the tester's reading of RECIPE. These assertions
cover the delivered instructions, not live window behavior.
"""

    def test_contract_names_every_gesture_and_the_input_capability_boundary(self):
        """An undelivered drag or double-click must read as could-not-run, not as a broken title bar.

The envelope must carry the explicit contract rather than the generic RECIPE
mapping: an unzoomed starting frame, drags and zoom/restore cycles on both
surfaces with the restore aimed at the zoomed window's own band, controls
that must neither drag nor stop working (a session action's confirmation and
the gear), a fixed ten-image schedule that fits the executor's retention and
captures the zoomed and dialog states, and the Finder comparison that
separates missing CUA input from a Farhelm failure.
"""
        text = CheckpointInstructionsTests().envelope('fresh', 'window-drag', None)['instructions']
        self.assertNotIn('Perform RECIPE.md checkpoint', text)
        for phrase in ("sidebar's top bar", "main pane's", 'find empty top-band background again', 'a control never drags',
                       'ordinary, unzoomed frame', 'Repeat step 3 from empty background of the main pane',
                       'confirmation panel to open', 'without deleting anything', 'exactly these ten',
                       'settings gear', "Finder window's title bar", 'could-not-run, not fail', 'do not create one',
                       'Window > Zoom', 'Press on the sidebar',
                       'Click the new-terminal button (+) after the tabs once', 'click the shell tab and require it', 'only the agent tab remains',
                       'icon view', 'unselected folder icon'):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, text)


class SetupTests(unittest.TestCase):
    """Require prepared native fixtures before any actual test can copy authentication."""

    def setUp(self):
        """Create fictional private setup records with independently checked file modes."""
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.config = {'settings': task()['settings'], 'auth_source': str(self.root / 'source.json'),
                       'authorization': str(self.root / 'consent.json'),
                       'fixture_receipt': str(self.root / 'fixture.json')}
        self.profile = types.SimpleNamespace(values={'guest_tester': self.config,
            'mac_base': {'name': 'mac-fixture', 'mac_address': '02:00:00:00:00:01'},
            'linux_base': {'name': 'linux-fixture', 'mac_address': '02:00:00:00:00:02'}})
        self.authorization = {'format': 1, 'approved': True, 'scope': 'disposable-test-clones',
                              'destinations': ['mac', 'linux'], 'auth_source': self.config['auth_source']}
        self.receipt = {'format': 1, 'bases': {role: self.profile.values[role + '_base']
                                             for role in ('mac', 'linux')},
                        'settings': self.config['settings'], 'checks': {key: True for key in (
                            'native-screenshot-click-type', 'native-after-reopen', 'clone-permission-carryover',
                            'auth-not-in-bases', 'farhelm-clean', 'linux-codex-ready')}}
        self.write('consent.json', self.authorization)
        self.write('fixture.json', self.receipt)
        self.write('source.json', {'fixture': 'fictional-login-bytes'})

    def write(self, name, value):
        """Write only owned fixture files, never ambient authentication or environment."""
        path = self.root / name
        path.write_text(json.dumps(value))
        path.chmod(0o600)

    def test_consent_requires_both_disposable_roles_and_exact_source(self):
        """Mac consent alone must not authorize a later remote-agent credential copy."""
        self.assertEqual(require_setup(self.profile), self.config)
        for changed in ({'destinations': ['mac']}, {'approved': False},
                        {'auth_source': '/fictional/other-login'}, {'scope': 'reusable-base'}):
            self.write('consent.json', dict(self.authorization, **changed))
            with self.subTest(changed=changed), self.assertRaisesRegex(Refused, 'standing consent'):
                require_setup(self.profile)

    def test_fixture_identity_runtime_and_missing_capability_refuse(self):
        """An old setup attestation cannot silently certify a replacement base or runtime."""
        for key in self.receipt['checks']:
            value = dict(self.receipt, checks=dict(self.receipt['checks'], **{key: False}))
            self.write('fixture.json', value)
            with self.subTest(capability=key), self.assertRaises(Refused):
                require_setup(self.profile)
        self.write('fixture.json', dict(self.receipt, settings=dict(self.config['settings'], desktop_app='/Other.app')))
        with self.assertRaises(Refused):
            require_setup(self.profile)
        self.write('fixture.json', self.receipt)
        self.profile.values['mac_base'] = dict(self.profile.values['mac_base'], name='replacement')
        with self.assertRaises(Refused):
            require_setup(self.profile)

    def test_configured_model_and_effort_can_change_without_replacing_the_native_receipt(self):
        """Model changes get live preflight on the same attested native runtime."""
        self.config['settings'] = dict(self.config['settings'], model='another-model', effort='medium')
        self.assertEqual(require_setup(self.profile)['settings'], self.config['settings'])

    def test_feedback_consent_is_separate_from_credential_destinations(self):
        """Credential injection cannot authorize an external feedback issue.

P0 and P1 remain runnable without feedback consent. Full and stop-after-P2
setup must refuse before starting unless the operator separately recorded that
exact live-send scope.
"""
        self.assertEqual(require_setup(self.profile), self.config)
        with self.assertRaisesRegex(Refused, 'live P2 feedback'):
            require_setup(self.profile, feedback=True)
        self.write('consent.json', dict(self.authorization, feedback_send_approved=True))
        self.assertEqual(require_setup(self.profile, feedback=True), self.config)
        self.write('consent.json', dict(self.authorization, feedback_send_approved='yes'))
        with self.assertRaisesRegex(Refused, 'live P2 feedback'):
            require_setup(self.profile, feedback=True)

    def test_credentials_use_stdin_and_owned_destinations_only(self):
        """Journaled argv must not carry bytes, even when both roles share one immutable read."""
        calls = []
        class Controller:
            """Expose exact owned names and capture the otherwise private stdin boundary."""
            def owned(self, role):
                """Return the fixture-owned name rather than a caller-supplied destination."""
                return 'owned-' + role
            def mutate(self, argv, detail, **kwargs):
                """Retain inputs separately so tests can detect argv or journal disclosure."""
                calls.append((argv, detail, kwargs))
        expected = (self.root / 'source.json').read_bytes()
        copy_credentials(Controller(), self.config)
        self.assertEqual([call[0][2] for call in calls], ['owned-mac', 'owned-linux'])
        for argv, detail, kwargs in calls:
            self.assertEqual(kwargs['input'], expected)
            self.assertNotIn('fictional-login-bytes', json.dumps(argv) + detail)
        (self.root / 'source.json').chmod(0o644)
        with self.assertRaisesRegex(Refused, 'private'):
            copy_credentials(Controller(), self.config)
        self.assertEqual(len(calls), 2)

    def test_records_are_private_and_legacy_profiles_cannot_start(self):
        """Historical cleanup support cannot become a host-UI fallback for new tests."""
        (self.root / 'consent.json').chmod(0o644)
        with self.assertRaisesRegex(Refused, 'private'):
            require_setup(self.profile)
        with self.assertRaisesRegex(Refused, 'format 2'):
            require_setup(types.SimpleNamespace(values={}))

    def test_real_credential_writer_refuses_overwrite_and_restricts_modes(self):
        """Execute the actual stdin writer in child homes without mutating the harness environment."""
        homes = {role: self.root / role for role in ('mac', 'linux')}
        for home in homes.values():
            home.mkdir(mode=0o700)
        class Controller:
            """Map fictional owned guests to explicit isolated subprocess homes."""
            def owned(self, role):
                """Keep role lookup independent from arbitrary caller file paths."""
                return role
            def mutate(self, argv, detail, **kwargs):
                """Run the unchanged production write script with a child-only HOME."""
                return subprocess.run([sys.executable, *argv[4:]], check=True,
                                      env={'HOME': str(homes[argv[2]]), 'PATH': os.defpath}, **kwargs)
        copy_credentials(Controller(), self.config)
        for home in homes.values():
            stored = home / '.codex/auth.json'
            self.assertEqual(stored.read_bytes(), (self.root / 'source.json').read_bytes())
            self.assertEqual(stored.stat().st_mode & 0o777, 0o600)
        with self.assertRaises(subprocess.CalledProcessError):
            copy_credentials(Controller(), self.config)
        for home in homes.values():
            self.assertEqual((home / '.codex/auth.json').read_bytes(), (self.root / 'source.json').read_bytes())

    def test_source_origin_does_not_export_git_authentication(self):
        """Only an ordinary public GitHub URL can be passed to the guest source checkout."""
        def source(url):
            """Supply source metadata without Git, network access or host credentials."""
            return types.SimpleNamespace(git=lambda *argv: url)
        self.assertEqual(source_url(source('git@github.com:fixture/project.git')),
                         'https://github.com/fixture/project.git')
        for url in ('https://token@github.com/fixture/project', 'file:///private/checkout',
                    'https://example.invalid/fixture/project', 'https://github.com/fixture/project?token=x'):
            with self.subTest(origin=url), self.assertRaises(Refused):
                source_url(source(url))

    def test_real_source_contract_supplies_text_origin(self):
        """The actual Git reader must compose with guest staging, not only a shaped mock."""
        checkout = self.root / 'checkout'
        # Both setup writes belong to the temporary checkout, even inside a Git hook.
        env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
        subprocess.run(['git', 'init', '-q', str(checkout)], check=True, capture_output=True, env=env)
        subprocess.run(['git', '-C', str(checkout), 'remote', 'add', 'origin',
                        'https://github.com/fixture/project.git'], check=True, capture_output=True, env=env)
        self.assertEqual(source_url(Source(checkout)), 'https://github.com/fixture/project.git')


if __name__ == '__main__':
    unittest.main()
