"""Prove workflow safety and evidence gates without borrowing real GUI outcomes.

The fake Tart substrate has independently observable identities, states and
mutation calls. Tiny image fixtures prove format/linkage gates only; they are
never evidence that Farhelm itself rendered or that a conversation occurred.
"""

import base64
from collections import namedtuple
import io
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import unittest
import uuid
from unittest import mock

from brick_test import (REPO, UPGRADE_FLOOR, Run, Source, artifact, checks, metadata, plan, priority_entry, smoke,
                        tier, version)
from checkpoint_contract import REMOTE_MAC_PASS
from control import Control, Refused, atomic_json
from machine_profile import Profile
from test_machine_profile import fixture


PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j9AAAAABJRU5ErkJggg==')


class FakeSource:
    """Describe fictional release capabilities without accessing Git or the site."""

    def __init__(self, supported=()):
        self.supported = set(supported)

    def tags(self):
        """Mix eligible stable tags with an RC and an excluded pre-contract release."""
        return ['v0.22.0', 'v0.23.0', 'v0.24.0', 'v0.25.0-rc.1', 'v0.25.0', 'unrelated']

    def commit(self, tag):
        """Give each source tag a distinct observable identity."""
        return 'fixture-source-' + tag

    def override(self, tag):
        """The capability is explicitly supplied by this fixture, not a historical claim."""
        return tag in self.supported


class PlanningTests(unittest.TestCase):
    """Pin candidate selection and the full supported-old-version coverage."""

    def test_upgrade_floor_agrees_with_the_installer_and_helm(self):
        """A floor raise must keep release coverage aligned with the shipped update gates.

The coordinator stays usable without building Rust, so its literal is checked
against the shared source constant rather than loading a compiled helper.
"""
        source = (REPO / 'crates/farhelm-proto/src/version.rs').read_text()
        floors = re.findall(r'^pub\s+const\s+UPGRADE_FLOOR\s*:\s*&str\s*=\s*"([^"]+)"\s*;',
                            source, re.MULTILINE)
        self.assertEqual(len(floors), 1, 'expected one shared Rust upgrade-floor constant')
        self.assertEqual(UPGRADE_FLOOR, 'v' + floors[0])

    def test_the_supplied_floor_bounds_the_previous_stable_pairs(self):
        """Raising the floor removes unsupported sources without weakening the remaining pairs.

Use fictional fixed floors: this policy test must survive a future production
raise, and a floor beyond every previous stable must fail before guest setup.
"""
        result = plan('v0.25.0', 'v0.24.0', FakeSource(), floor='v0.24.0')
        self.assertEqual([v['id'] for v in result['passes']],
                         ['p0-fresh', 'p0-updater-upgrade', 'p0-installer-upgrade',
                          REMOTE_MAC_PASS, 'fresh', 'upgrade-v0.24.0'])
        self.assertTrue(all(v['previous'] == 'v0.24.0' for v in result['passes']))
        with self.assertRaisesRegex(Refused, 'from v0.25.0 onward'):
            plan('v0.25.0', 'v0.24.0', FakeSource(), floor='v0.25.0')

    def test_previous_stable_leads_and_older_supported_pairs_remain_required(self):
        """Testing only the latest old stable must not silently waive the older upgrade contracts within the cap.

Discovery is planned before the installer pair, because the installer pair
goes on to P1 and P2 while its candidate still runs. The plan carries the
layout marker that lets this coordinator continue it.
"""
        result = plan('v0.25.0', 'v0.24.0', FakeSource(), floor='v0.23.0')
        self.assertEqual([v['id'] for v in result['passes']],
                         ['p0-fresh', 'p0-updater-upgrade', 'p0-installer-upgrade',
                          REMOTE_MAC_PASS, 'fresh', 'upgrade-v0.24.0', 'upgrade-v0.23.0'])
        self.assertEqual(result['p0_sequence'], [v['id'] for v in result['passes'][:3]])
        self.assertEqual(result['passes'][5]['update_path'], 'manual')
        self.assertEqual(result['priority_layout'], 2)

    def test_bonus_upgrades_cover_at_most_the_three_newest_previous_stables(self):
        """Historical coverage stops at three sources however far back the upgrade floor reaches.

The maintainer capped it: each older source costs a whole pair. The newest
stable still leads P0 and the bonus list, and RCs never count as sources.
"""
        class ManyStables(FakeSource):
            """Offer five eligible previous stables and an RC between them."""
            def tags(self):
                """List more stables than the cap, out of order, to show the plan sorts them."""
                return ['v0.20.0', 'v0.24.0', 'v0.21.0', 'v0.23.0-rc.1', 'v0.23.0', 'v0.22.0', 'v0.25.0']
        result = plan('v0.25.0', 'v0.24.0', ManyStables(), floor='v0.20.0')
        self.assertEqual([v['id'] for v in result['passes'] if v['id'].startswith('upgrade-')],
                         ['upgrade-v0.24.0', 'upgrade-v0.23.0', 'upgrade-v0.22.0'])
        self.assertTrue(all(v['previous'] == 'v0.24.0' for v in result['passes'] if v['priority'] == 'P0'))

    def test_prerelease_plans_no_updater_pair(self):
        """The updater never offers an RC or dev build, so cloning a pair only to skip its check is waste.

The installer pair still runs, and carries the supervisor checks either way.
"""
        for candidate in ('v0.25.0-rc.1', 'v0.25.0-dev.1'):
            with self.subTest(candidate=candidate):
                result = plan(candidate, 'v0.24.0', FakeSource(['v0.24.0']), floor='v0.23.0')
                self.assertEqual(result['p0_sequence'], ['p0-fresh', 'p0-installer-upgrade'])
                self.assertNotIn('p0-updater-upgrade', [v['id'] for v in result['passes']])

    def test_installer_pair_upgrades_supervisors_before_agents_then_runs_p1_and_p2(self):
        """The pair finishes continuity before the two P2 checks.

Each supervisor's upgrade is its own P0 checkpoint ahead of the new agents, so
a flaky agent turn cannot obscure whether a supervisor came up. The old
sessions have to start before the upgrade, so that P1 checkpoint precedes the
relaunch; the updater pair checks discovery and relaunch only.
"""
        installer = checks('p0-installer-upgrade')
        self.assertEqual([(key, spec['priority']) for key, spec in installer.items()],
                         [('artifact-verification', 'P0'), ('old-fixture', 'P0'), ('old-sessions', 'P1'),
                          ('update-relaunch', 'P0'), ('local-supervisor-upgrade', 'P0'),
                          ('remote-supervisor-upgrade', 'P0'), ('local-supervisor-agent', 'P0'),
                          ('remote-supervisor-agent', 'P0'), ('local-old-session', 'P1'),
                          ('remote-old-session', 'P1'), ('remote-host-remove', 'P1'),
                          ('remote-host-uninstall', 'P1'), ('p2-feedback-send', 'P2'), ('p2-announcements', 'P2')])
        self.assertEqual(installer['p2-feedback-send'], {'gui': True, 'command': False, 'continuity': False,
                                                         'priority': 'P2'})
        self.assertEqual(installer['p2-announcements'], {'gui': True, 'command': True, 'continuity': False,
                                                          'priority': 'P2'})
        self.assertTrue(all(spec['command'] for key, spec in installer.items()
                            if key not in ('artifact-verification', 'p2-feedback-send')))
        self.assertEqual(list(checks('p0-updater-upgrade')),
                         ['artifact-verification', 'old-fixture', 'update-discovery', 'update-relaunch'])
        self.assertEqual({spec['priority'] for spec in checks('p0-updater-upgrade').values()}, {'P0'})

    def test_fresh_pass_checks_window_drag_after_its_sessions(self):
        """The title-bar check needs a selected session and must not cost the bonus session coverage.

Only the full fresh pass declares `window-drag`, as a GUI-only check that comes
after the session checks and before the addendum: the main pane's top row
exists only with a session open, and a failed check stops the pass. P0's
fresh fixture keeps its four checkpoints.
"""
        self.assertEqual(list(checks('fresh')),
                         ['artifact-verification', 'fixture-clean', 'install', 'app-start', 'remote-provision',
                          'codex-conversation', 'remote-terminal', 'window-drag', 'addendum'])
        self.assertEqual(checks('fresh')['window-drag'], {'gui': True, 'command': False, 'continuity': False})
        self.assertNotIn('window-drag', checks('p0-fresh'))
        self.assertNotIn('window-drag', checks('upgrade-v0.24.0'))

    def test_override_uses_old_source_and_never_selects_an_rc(self):
        """A candidate's support cannot make an unsupported old app use its override."""
        source = FakeSource(['v0.24.0'])
        stable = plan('v0.25.0', 'v0.24.0', source, floor='v0.23.0')
        self.assertEqual(stable['passes'][1]['update_path'], 'override')
        self.assertEqual(stable['passes'][2]['update_path'], 'manual')
        by_id = {entry['id']: entry for entry in stable['passes']}
        self.assertEqual(by_id['upgrade-v0.24.0']['update_path'], 'override')
        self.assertEqual(by_id['upgrade-v0.23.0']['update_path'], 'manual')
        rc = plan('v0.25.0-rc.1', 'v0.24.0', source, floor='v0.23.0')
        self.assertTrue(all(v['update_path'] == 'manual' for v in rc['passes'] if v['update_path'] is not None))

    def test_promoted_candidate_is_an_ordinary_rehearsal(self):
        """After-publication testing still uses older versions, never candidate-to-itself."""
        result = plan('v0.24.0', 'v0.24.0', FakeSource(), floor='v0.23.0')
        self.assertTrue(all(v['previous'] == 'v0.23.0' for v in result['passes']))
        self.assertEqual(result['passes'][1]['update_path'], 'ordinary')
        self.assertEqual(result['passes'][2]['update_path'], 'manual')

    def test_downgrades_missing_old_source_and_unsafe_tags_are_refused(self):
        """Invalid gate inputs must fail before any VM can be created."""
        with self.assertRaises(Refused):
            plan('v0.23.0', 'v0.24.0', FakeSource(), floor='v0.23.0')
        class MissingStable(FakeSource):
            """Omit the promoted release even though an older eligible tag exists."""
            def tags(self):
                """Expose the incomplete source prerequisite independently of planning."""
                return ['v0.23.0']
        with self.assertRaisesRegex(Refused, 'promoted stable is missing'):
            plan('v0.25.0', 'v0.24.0', MissingStable(), floor='v0.23.0')
        for value in ('--help', '../release', 'v01.2.3', 'v1.2.3-rc.01'):
            with self.subTest(tag=value), self.assertRaises(Refused):
                version(value)
        self.assertLess(version('v1.2.3-rc.2'), version('v1.2.3-rc.10'))
        self.assertLess(version('v1.2.3-rc.10'), version('v1.2.3'))

    def test_availability_receipt_does_not_claim_signature_verification(self):
        """Unverified public metadata cannot satisfy the old-ring verification milestone."""
        import hashlib
        installer = b'fixture installer bytes'
        inputs = {'install.sh': installer,
                  'SHA256SUMS': (hashlib.sha256(installer).hexdigest() + '  install.sh\n').encode(),
                  'SHA256SUMS.minisig': b'trusted comment: farhelm v0.25.0\n'}
        def fetch(url):
            """Return only the known candidate paths, without a network request."""
            self.assertTrue(url.startswith('https://get.farhelm.io/v0.25.0/'))
            return inputs[url.rsplit('/', 1)[1]]
        _, receipt = metadata('v0.25.0', fetch)
        self.assertEqual(receipt['signature_verification'], 'pending')
        inputs['install.sh'] = b'different bytes'
        with self.assertRaises(Refused):
            metadata('v0.25.0', fetch)


class SourceOverrideTests(unittest.TestCase):
    """Pin where the real Git reader looks for an old release's update override.

Planning tests use FakeSource, so they cannot catch the reader looking in the
wrong place. These build a throwaway repository with fictional release tags in
each source layout and run the actual `Source` against it. Git is told who the
committer is on its command line, so neither the test process's environment nor
the user's Git configuration has to supply an identity.
"""

    NEW = 'crates/farhelm-desktop/src/updater.rs'
    OLD = 'crates/farhelm-ui/src/desktop/updater.rs'
    SUPPORTED = 'const LATEST_OVERRIDE: &str = "FARHELM_DESKTOP_UPDATE_LATEST";\n'
    UNSUPPORTED = 'const ORIGIN: &str = "https://get.farhelm.io";\n'

    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root)
        self.git('init', '-q')

    def git(self, *args):
        """Run Git in the fixture repository with a fixed identity, no signing or hooks, and a bound.

Commit and tag signing are both turned off here because a machine's global configuration may turn them on, and a
signed tag would need a message and a key agent; the timeout keeps such a surprise from hanging the suite.
"""
        # Git hooks can export selectors for their own repository. The fixture's
        # Git children must discover the temporary repository from its directory.
        env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
        return subprocess.run(['git', '-C', str(self.root), '-c', 'user.name=Fixture',
                               '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false',
                               '-c', 'tag.gpgSign=false', '-c', 'core.hooksPath=/dev/null', *args],
                              check=True, capture_output=True, text=True, timeout=30, env=env).stdout

    def release(self, tag, files):
        """Tag a commit whose `crates/` tree holds exactly `files`, like a release cut from that layout."""
        shutil.rmtree(self.root / 'crates', ignore_errors=True)
        for path, text in files.items():
            (self.root / path).parent.mkdir(parents=True, exist_ok=True)
            (self.root / path).write_text(text)
        self.git('add', '-A')
        self.git('commit', '-q', '--allow-empty', '-m', tag)
        self.git('tag', tag)
        # The cases below depend on which files each tag holds, so check that
        # premise against Git itself rather than trusting the working tree.
        self.assertEqual(self.git('ls-tree', '-r', '--name-only', tag).splitlines(), sorted(files))

    def test_post_gpui_release_is_read_at_the_desktop_crate_path(self):
        """A GPUI-era previous stable must be planned from its own updater source.

Releases cut after the GPUI cutover keep the updater in farhelm-desktop. The
reader answers from that file, and when a tag somehow holds both paths the new
one wins: its marker-free text means no override even though the stale old
file carries the variable.
"""
        self.release('v0.30.0', {self.NEW: self.SUPPORTED})
        self.release('v0.31.0', {self.NEW: self.UNSUPPORTED, self.OLD: self.SUPPORTED})
        self.assertTrue(Source(self.root).override('v0.30.0'))
        self.assertFalse(Source(self.root).override('v0.31.0'))

    def test_pre_gpui_release_falls_back_to_the_ui_crate_path(self):
        """Older previous stables, back to the upgrade floor, keep their override decision.

A tag with only the webview-era path is read there, and the answer still
depends on whether that file names the override variable.
"""
        self.release('v0.23.0', {self.OLD: self.UNSUPPORTED})
        self.release('v0.24.0', {self.OLD: self.SUPPORTED})
        self.assertFalse(Source(self.root).override('v0.23.0'))
        self.assertTrue(Source(self.root).override('v0.24.0'))

    def test_release_without_updater_source_is_refused(self):
        """An unknown layout stops planning instead of quietly choosing the manual branch.

A tag with neither path is refused with a message naming the fix, and a tag the
checkout lacks keeps the ordinary missing-source refusal.
"""
        self.release('v0.25.0', {'crates/farhelm/src/main.rs': 'fn main() {}\n'})
        with self.assertRaisesRegex(Refused, 'no desktop updater source at a known path'):
            Source(self.root).override('v0.25.0')
        with self.assertRaisesRegex(Refused, 'release source lookup failed'):
            Source(self.root).override('v0.26.0')


class FakeTart:
    """Model Tart independently from workflow state and retain every mutation call."""

    def __init__(self, profile):
        self.profile = profile
        self.states = {}
        self.calls = []
        self.children = []
        self.account = 'guest'
        self.fail_clone = False
        self.fail_start_probe = False
        self.markers = {}
        home = Path(profile.values['tart_home'])
        for role in ('mac', 'linux'):
            base = profile.values[role + '_base']
            directory = home / 'vms' / base['name']
            directory.mkdir(parents=True)
            atomic_json(directory / 'config.json', {'macAddress': base['mac_address']})
            self.states[base['name']] = 'stopped'

    def runner(self, argv, **kwargs):
        """Make each modeled state transition visible through independent subsequent probes."""
        self.calls.append(argv)
        action = argv[1]
        result = b''
        if action == 'list':
            result = json.dumps([{'Name': name} for name in self.states]).encode()
        elif action == 'get':
            if self.fail_start_probe and self.states[argv[2]] == 'running':
                raise Refused('injected post-spawn probe failure')
            result = json.dumps({'State': self.states[argv[2]]}).encode()
        elif action == 'clone':
            if self.fail_clone:
                raise subprocess.CalledProcessError(1, argv)
            name = argv[3]
            directory = Path(self.profile.values['tart_home']) / 'vms' / name
            directory.mkdir()
            atomic_json(directory / 'config.json', {'macAddress': '02:00:00:00:00:10'})
            self.states[name] = 'stopped'
        elif action == 'exec':
            name = argv[3] if argv[2] == '-i' else argv[2]
            command = argv[4:] if argv[2] == '-i' else argv[3:]
            if '/sbin/shutdown' in command:
                self.states[name] = 'stopped'
            elif command == ['/usr/bin/id', '-un']:
                result = (self.account + '\n').encode()
            elif any('commands.txt' in item for item in command):
                self.markers[name] = b'fixture-smoke-ready\n'
            elif any('printf %s' in item for item in command):
                result = b'/fictional/guest/evidence'
        elif action == 'delete':
            name = argv[2]
            self.states.pop(name)
            shutil.rmtree(Path(self.profile.values['tart_home']) / 'vms' / name)
        return subprocess.CompletedProcess(argv, 0, result)

    def spawn(self, argv, **kwargs):
        """Retain an owned synthetic child whose liveness follows the modeled VM."""
        owner, name = self, argv[-1]
        owner.states[name] = 'running'
        class Child:
            """No OS process is spawned by this fixture; no real PID may be signaled."""
            pid = 1000 + len(owner.children)
            returncode = None

            def poll(self):
                """Report completion only after a separate shutdown changes VM state."""
                if owner.states.get(name) != 'running':
                    self.returncode = 0
                return self.returncode
        child = Child()
        owner.children.append(child)
        return child

    def factory(self, *args):
        """Use the real ownership/journal implementation with fake command and export substrates."""
        model = self
        class FixtureControl(Control):
            """Keep workflow calls on production fences while avoiding real Tart execution."""

            def start(self, role, headless=False):
                """Route spawning to synthetic children rather than an installed Tart binary."""
                return super().start(role, headless, spawner=model.spawn)

            def collect(self, role, guest_directory, label):
                """Retain a real tar fixture so smoke export validation reads actual bytes."""
                name = self.owned(role)
                path = self.directory / (label + '.tar')
                with tarfile.open(path, 'w') as archive:
                    content = model.markers[name]
                    info = tarfile.TarInfo('./commands.txt')
                    info.size = len(content)
                    archive.addfile(info, io.BytesIO(content))
        return FixtureControl(*args, runner=self.runner)

    def connect(self, control, mac, peer, user, route, *, peer_role='linux'):
        """Assert named owned peers, then record a fictional already-proved SSH handle."""
        if self.states[mac] != 'running' or self.states[peer] != 'running':
            raise AssertionError('both owned guest processes must be live before connection')
        if control.owned(peer_role) != peer:
            raise AssertionError('the connection must use the exact owned peer role')
        if user != self.account:
            raise AssertionError('account premise must already be checked')
        control.data['ssh'] = {'alias': 'fixture-peer', 'user': user}
        control.save()


class WorkflowTests(unittest.TestCase):
    """Demonstrate failure, interruption and evidence boundaries on isolated private state."""

    def setUp(self):
        """Create a valid unrelated machine profile and two independently stopped source bases."""
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        data = fixture(self.root)
        data['tart_home'] = 'tart'
        path = self.root / 'profile.json'
        path.write_text(json.dumps(data)); path.chmod(0o600)
        self.profile = Profile.load(path, home=self.root / 'home')
        self.model = FakeTart(self.profile)
        self.evidence = Path(self.profile.values['evidence_root'])
        self.evidence.mkdir()
        usage = namedtuple('Usage', 'free')
        self.free = 20 * 1024 ** 3
        self.run = Run(self.evidence / ('brick-' + str(uuid.uuid4())), self.profile,
                       factory=self.model.factory, disk_usage=lambda path: usage(self.free), connector=self.model.connect)
        self.plan = plan('v0.25.0', 'v0.24.0', FakeSource(), floor='v0.23.0')
        # These lifecycle/evidence regressions resume a historical manifest.
        # Keep that premise explicit so the new P0 scheduling fence cannot
        # mask the ownership or evidence failure each test exists to observe.
        self.plan.pop('p0_sequence')
        self.plan['passes'] = [v for v in self.plan['passes'] if v['priority'] == 'bonus']
        self.plan['sha256sums_sha256'] = 'fixture-checksum'
        self.run.create(self.plan)
        self.assertTrue(all(state == 'stopped' for state in self.model.states.values()))

    def evidence_file(self, name, value):
        """Write an owned bounded artifact and return only its run-relative reference."""
        p = self.run.directory / name
        p.write_bytes(value)
        return name

    def prepare(self, pass_id='fresh'):
        """Establish live owned guests before testing any result or cleanup behavior."""
        children = []
        with self.run.locked():
            self.run.prepare(pass_id, children)
        self.assertEqual(len(children), 2)
        self.assertTrue(all(child.poll() is None for child in children))
        return children

    def test_replaced_or_running_base_prevents_all_cloning(self):
        """Source identity and stopped-state premises must precede any working VM mutation."""
        base = self.profile.values['mac_base']
        self.model.states[base['name']] = 'running'
        with self.run.locked(), self.assertRaisesRegex(Refused, 'remain stopped'):
            self.run.prepare('fresh', [])
        self.assertFalse(any(c[1] == 'clone' for c in self.model.calls))
        self.model.states[base['name']] = 'stopped'
        p = Path(self.profile.values['tart_home']) / 'vms' / base['name'] / 'config.json'
        atomic_json(p, {'macAddress': '02:00:00:00:00:99'})
        with self.run.locked(), self.assertRaisesRegex(Refused, 'identity changed'):
            self.run.prepare('fresh', [])
        self.assertFalse(any(c[1] == 'clone' for c in self.model.calls))

    def test_resume_profile_edit_cannot_redirect_existing_run(self):
        """A changed journal or VM binding is a different machine, not a cleanup override."""
        values = dict(self.profile.values, journal=str(self.root / 'different.md'))
        other = Run(self.run.directory, Profile(self.profile.source, values))
        with self.assertRaisesRegex(Refused, 'mismatch'):
            with other.locked():
                pass
        self.assertEqual(self.model.calls, [])

    def test_creation_intent_and_result_share_an_event_even_on_failure(self):
        """An unmatched intent must mean interrupted work, not unrelated result IDs."""
        journal = Path(self.profile.values['journal'])
        headings = [line for line in journal.read_text().splitlines() if line.startswith('## ')]
        self.assertIn('| intent |', headings[0])
        self.assertIn('| result |', headings[1])
        self.assertEqual(headings[0].rsplit(' | ', 1)[1], headings[1].rsplit(' | ', 1)[1])
        with self.assertRaises(FileExistsError):
            self.run.create(self.plan)
        headings = [line for line in journal.read_text().splitlines() if line.startswith('## ')]
        self.assertEqual(headings[-2].rsplit(' | ', 1)[1], headings[-1].rsplit(' | ', 1)[1])
        self.assertIn('creation failed', journal.read_text())

    def test_changed_evidence_invalidates_accepted_observation_and_cleanup(self):
        """Reusing a path with different bytes cannot inherit the old pass attestation."""
        self.prepare()
        reference = self.evidence_file('observation.txt', b'original observation')
        with self.run.locked():
            entry = self.run.selected('fresh')
            self.run.record('fresh', 'fixture-clean', 'pass', [reference], 'fictional clean fixture')
            # Narrow this synthetic pass so the later refusal distinguishes a
            # changed artifact from unrelated, deliberately unperformed checks.
            entry['required'] = {'fixture-clean': entry['required']['fixture-clean']}
            self.run.save()
            self.assertEqual(self.run.verdict(entry), 'pass')
        self.evidence_file('observation.txt', b'replacement observation')
        with self.run.locked():
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('fresh')
        self.assertFalse(any(c[1] == 'delete' for c in self.model.calls))

    def test_budget_refuses_new_pair_without_mutation(self):
        """Observed space limits fence cloning even when the profile was valid at init."""
        self.free = 512 * 1024 ** 2
        with self.run.locked(), self.assertRaisesRegex(Refused, 'floor'):
            self.run.prepare('fresh', [])
        self.assertFalse(self.model.calls)

    def test_storage_volume_change_is_refused_before_cloning(self):
        """A new mount cannot turn the evidence-volume floor into the wrong-disk check."""
        observed = []
        def volume_id(path):
            """Expose distinct VM/evidence devices without changing real mounts."""
            observed.append(path)
            return 'vm-volume' if path.name == 'vms' else 'evidence-volume'
        self.run.volume_id = volume_id
        with self.run.locked(), self.assertRaisesRegex(Refused, 'share a filesystem'):
            self.run.prepare('fresh', [])
        self.assertIn(Path(self.profile.values['tart_home']) / 'vms', observed)
        self.assertIn(self.evidence, observed)
        self.assertEqual(self.model.calls, [])

    def test_failed_startup_transfers_live_child_to_workflow_supervision(self):
        """Preparation must retain a newly spawned child even when start never returns."""
        self.model.fail_start_probe = True
        children = []
        with self.run.locked(), self.assertRaisesRegex(Refused, 'post-spawn probe failure'):
            self.run.prepare('fresh', children)
        self.assertEqual(len(children), 1)
        self.assertEqual(children, self.model.children)
        self.assertTrue(all(child.poll() is None for child in children))
        with self.run.locked():
            self.assertEqual(self.run.selected('fresh')['phase'], 'failed')
            self.model.fail_start_probe = False
            self.run.cleanup('fresh', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_interrupted_preparation_is_not_replayed_or_claimed_clean(self):
        """A failed clone must retain partial ownership state for actual reconciliation."""
        self.model.fail_clone = True
        with self.run.locked(), self.assertRaises(subprocess.CalledProcessError):
            self.run.prepare('fresh', [])
        with self.run.locked():
            self.assertEqual(self.run.data['passes']['fresh']['phase'], 'failed')
            with self.assertRaises(Refused):
                self.run.prepare('fresh', [])
            with self.assertRaises(Refused):
                self.run.cleanup('fresh')
        self.assertEqual(len([c for c in self.model.calls if c[1] == 'clone']), 1)

    def test_wrong_guest_account_retains_failure_and_live_children(self):
        """A mismatched SSH account is a fixture failure before authorizing remote key setup."""
        self.model.account = 'differentguest'
        children = []
        with self.run.locked(), self.assertRaisesRegex(Refused, 'account differs'):
            self.run.prepare('fresh', children)
        self.assertEqual(len(children), 2)
        self.assertTrue(all(child.poll() is None for child in children))
        with self.run.locked():
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            self.run.cleanup('fresh', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_one_active_pair_fences_the_next_upgrade(self):
        """A standard workflow cannot start another Mac pair while the first is owned and live."""
        self.prepare()
        before = len([c for c in self.model.calls if c[1] == 'clone'])
        with self.run.locked(), self.assertRaisesRegex(Refused, 'existing working pair'):
            self.run.prepare('upgrade-v0.24.0', [])
        self.assertEqual(len([c for c in self.model.calls if c[1] == 'clone']), before)

    def test_gui_pass_requires_image_and_hybrid_checks_also_require_commands(self):
        """Text-only logs and screenshots alone cannot satisfy different recipe evidence contracts."""
        self.prepare()
        command = self.evidence_file('command.txt', b'fixture observation')
        image = self.evidence_file('frame.png', PNG)
        empty = self.evidence_file('empty.txt', b'')
        with self.run.locked():
            for check_id in ('app-start', 'remote-terminal'):
                for refs in ([command], [empty]):
                    with self.subTest(check=check_id, refs=refs), self.assertRaises(Refused):
                        self.run.record('fresh', check_id, 'pass', refs, 'Fixture evidence gate only.')
            for check_id in ('codex-conversation', 'remote-terminal'):
                with self.subTest(check=check_id), self.assertRaises(Refused):
                    self.run.record('fresh', check_id, 'pass', [image], 'Fixture evidence gate only.')
            self.run.record('fresh', 'app-start', 'pass', [image], 'Format/linkage fixture; no product GUI was tested.')
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            with self.assertRaises(Refused):
                self.run.cleanup('fresh')

    def test_manifest_without_window_drag_keeps_its_declared_checks(self):
        """A run planned before `window-drag` existed must not gain an obligation it never declared.

The manifest's saved required set, not the current check table, decides what
a pass must record, so an older fresh pass stays completable as planned.
"""
        with self.run.locked():
            del self.run.selected('fresh')['required']['window-drag']
            self.run.save()
        self.prepare()
        with self.run.locked():
            self.assertNotIn('window-drag', self.run.selected('fresh')['required'])
            with self.assertRaisesRegex(Refused, 'declared check'):
                self.run.record('fresh', 'window-drag', 'could-not-run', [], 'Not declared by this manifest.')

    def test_evidence_cannot_escape_or_be_overwritten_as_an_observation(self):
        """Private run references and append-only observations survive later mistakes."""
        self.prepare()
        outside = self.root / 'outside.txt'; outside.write_bytes(b'owned outside fixture')
        link = self.run.directory / 'escape'; link.symlink_to(outside)
        with self.assertRaises(Refused):
            artifact(self.run.directory, 'escape')
        image = self.evidence_file('frame.png', PNG)
        with self.run.locked():
            self.run.record('fresh', 'app-start', 'pass', [image], 'Fixture linkage only.')
            with self.assertRaises(Refused):
                self.run.record('fresh', 'app-start', 'fail', [], 'Cannot erase the earlier result.')

    def test_failed_or_skipped_checks_never_become_a_pass(self):
        """An explicit product failure and an unavailable fixture remain different verdicts."""
        self.prepare()
        with self.run.locked():
            self.run.record('fresh', 'app-start', 'could-not-run', [], 'Fixture has no GUI substrate.')
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'could not run')
            self.run.record('fresh', 'remote-terminal', 'fail', [], 'Synthetic failure scenario.')
            self.assertEqual(self.run.verdict(self.run.selected('fresh')), 'fail')

    def test_record_hashes_the_bytes_it_validated_including_receipts(self):
        """Replacing evidence after its first read cannot inherit the old attestation.

        Receipt references also appear in the ordinary evidence list. Each must
        be read once, checked for progress, and bound to that same byte string.
        """
        self.prepare('upgrade-v0.24.0')
        receipt = {'pid': 42, 'start_ticks': 100,
                   'boot_id': '00000000-0000-0000-0000-000000000001', 'counter': 5}
        before = json.dumps(receipt).encode()
        after = json.dumps(dict(receipt, counter=6)).encode()
        refs = [self.evidence_file('before.json', before), self.evidence_file('after.json', after)]
        original = dict(zip(refs, (before, after)))
        reads = []

        def replace_after_read(root, reference):
            """Return valid bytes, then leave different bytes for any accidental reread."""
            raw = artifact(root, reference)
            reads.append(reference)
            (root / reference).write_bytes(b'replaced evidence')
            return raw

        with self.run.locked(), mock.patch('brick_test.artifact', side_effect=replace_after_read):
            self.run.record('upgrade-v0.24.0', 'continuity-before', 'pass', refs,
                            'Synthetic bytes/progress contract only.', *refs)
        result = self.run.selected('upgrade-v0.24.0')['results']['continuity-before']
        self.assertCountEqual(reads, refs)
        self.assertEqual(result['sha256'], {ref: hashlib.sha256(raw).hexdigest()
                                           for ref, raw in original.items()})

    def test_continuity_rejects_replacement_stale_or_malformed_receipts(self):
        """A larger counter cannot substitute a replacement process or an untyped identity."""
        self.prepare('upgrade-v0.24.0')
        before = {'pid': 42, 'start_ticks': 100, 'boot_id': '00000000-0000-0000-0000-000000000001', 'counter': 5}
        a = self.evidence_file('before.json', json.dumps(before).encode())
        for delta in ({'pid': 43, 'counter': 6}, {'counter': 5}, {'counter': '6'}):
            b = self.evidence_file('after.json', json.dumps(dict(before, **delta)).encode())
            with self.run.locked(), self.assertRaises((Refused, RuntimeError)):
                self.run.record('upgrade-v0.24.0', 'continuity-before', 'pass', [a, b],
                                'Synthetic continuity fixture.', a, b)
        b = self.evidence_file('after.json', json.dumps(dict(before, counter=6)).encode())
        with self.run.locked():
            self.run.record('upgrade-v0.24.0', 'continuity-before', 'pass', [a, b],
                            'Original fixture identity advanced.', a, b)

    def test_verification_must_use_old_ring_and_match_candidate_checksum(self):
        """Trust from the candidate itself must never waive the old app's compatibility check."""
        self.prepare()
        value = {'version': 'v0.25.0', 'previous_trust_ring': 'v0.25.0',
                 'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
                 'trusted_comment': 'verified', 'installer_checksum': 'verified'}
        ref = self.evidence_file('verification.json', json.dumps(value).encode())
        with self.run.locked(), self.assertRaises(Refused):
            self.run.record('fresh', 'artifact-verification', 'pass', [ref], 'Verification fixture.')
        value['previous_trust_ring'] = 'v0.24.0'
        self.evidence_file('verification.json', json.dumps(value).encode())
        with self.run.locked():
            self.run.record('fresh', 'artifact-verification', 'pass', [ref], 'Old-ring receipt fixture accepted.')

    def test_continuity_milestones_reject_replacements_replay_and_wrong_order(self):
        """Internally valid pairs must still identify one advancing original workload."""
        self.prepare('upgrade-v0.24.0')
        image = self.evidence_file('frame.png', PNG)
        identity = {'pid': 42, 'start_ticks': 100, 'boot_id': '00000000-0000-0000-0000-000000000001'}
        def pair(label, low, high, **changes):
            """Create distinct synthetic receipts so rejected cases overwrite no observation."""
            return [self.evidence_file(label + '-before.json', json.dumps(dict(identity, counter=low, **changes)).encode()),
                    self.evidence_file(label + '-after.json', json.dumps(dict(identity, counter=high, **changes)).encode())]
        baseline = pair('baseline', 1, 2)
        remote = pair('remote', 2, 3)
        replacement = pair('replacement', 2, 3, pid=99)
        with self.run.locked():
            def record(check, refs):
                """Drive the production evidence gates with GUI and typed command receipts."""
                self.run.record('upgrade-v0.24.0', check, 'pass', [image, *refs],
                                'Synthetic continuity contract only.', *refs)
            with self.assertRaisesRegex(Refused, 'preceding continuity'):
                record('remote-update', remote)
            record('continuity-before', baseline)
            with self.assertRaisesRegex(Refused, 'original process'):
                record('remote-update', replacement)
            with self.assertRaisesRegex(Refused, 'replays'):
                record('remote-update', baseline)
            record('remote-update', remote)
            with self.assertRaisesRegex(Refused, 'replays'):
                record('reopen', remote)
            record('reopen', pair('reopen', 3, 4))

    def test_saved_verdict_rechecks_cross_milestone_identity_with_all_checks_present(self):
        """Legacy accepted replacements must not pass or authorize cleanup after upgrading the harness.

Start with a complete valid synthetic pass, then model the old coordinator's
otherwise well-formed replacement receipts. This excludes missing checks and
artifact corruption as competing causes of the new refusal.
"""
        self.prepare('upgrade-v0.24.0')
        image = self.evidence_file('frame.png', PNG)
        command = self.evidence_file('command.txt', b'fictional command evidence')
        verification = self.evidence_file('verification.json', json.dumps({
            'version': 'v0.25.0', 'previous_trust_ring': 'v0.24.0',
            'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
            'trusted_comment': 'verified', 'installer_checksum': 'verified'}).encode())
        identity = {'pid': 42, 'start_ticks': 100, 'boot_id': '00000000-0000-0000-0000-000000000001'}
        order = ['continuity-before', 'remote-update', 'reopen']
        with self.run.locked():
            self.run.select_update('upgrade-v0.24.0', 'v0.24.0')
            entry = self.run.selected('upgrade-v0.24.0')
            for check_id, spec in entry['required'].items():
                refs = [verification] if check_id == 'artifact-verification' else [image, command]
                before = after = None
                if spec['continuity']:
                    low = 1 + order.index(check_id)
                    before = self.evidence_file(check_id + '-before.json', json.dumps(dict(identity, counter=low)).encode())
                    after = self.evidence_file(check_id + '-after.json', json.dumps(dict(identity, counter=low + 1)).encode())
                    refs.extend([before, after])
                self.run.record('upgrade-v0.24.0', check_id, 'pass', refs,
                                'Complete synthetic acceptance fixture; no product behavior was tested.', before, after)
            self.assertEqual(len(entry['results']), 10)
            self.assertEqual(self.run.verdict(entry), 'pass')
            result = entry['results']['reopen']
            for role, counter in (('before', 3), ('after', 4)):
                ref = result['continuity'][role]
                self.evidence_file(ref, json.dumps(dict(identity, pid=99, counter=counter)).encode())
                result['sha256'][ref] = hashlib.sha256(artifact(self.run.directory, ref)).hexdigest()
            self.assertEqual(self.run.verdict(entry), 'could not run')
            self.run.save()
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('upgrade-v0.24.0')
        self.assertFalse(any(call[1] == 'delete' for call in self.model.calls))

    def test_latest_is_reobserved_before_install_and_promotion_is_reported(self):
        """Promotion during old-state seeding must change path and invalidate pre-promotion coverage."""
        self.prepare('upgrade-v0.24.0')
        image = self.evidence_file('frame.png', PNG)
        command = self.evidence_file('command.txt', b'fixture installed version')
        with self.run.locked():
            with self.assertRaises(Refused):
                self.run.record('upgrade-v0.24.0', 'update-relaunch', 'pass', [image, command], 'Fixture check.')
            self.run.select_update('upgrade-v0.24.0', 'v0.25.0')
            self.assertEqual(self.run.selected('upgrade-v0.24.0')['update_path'], 'ordinary')
            self.assertEqual(self.run.report()['promotion_coverage'], 'after-publication')

    def test_required_addendum_checks_cannot_replace_baseline(self):
        """Release-specific coverage adds obligations instead of renaming away missing checks."""
        with self.run.locked():
            baseline = set(self.run.selected('fresh')['required'])
            self.run.add_check('fresh', 'extra-new-feature', gui=True)
            self.assertTrue(baseline < set(self.run.selected('fresh')['required']))
            with self.assertRaises(Refused):
                self.run.add_check('fresh', 'app-start')

    def test_fixture_smoke_exports_stops_and_deletes_only_working_guests(self):
        """The short smoke validates orchestration without manufacturing a release verdict."""
        second = Run(self.evidence / ('brick-' + str(uuid.uuid4())), self.profile,
                     factory=self.model.factory, connector=self.model.connect)
        second.create(smoke=True)
        smoke(second)
        with second.locked():
            self.assertEqual(second.report()['verdict'], 'fixture smoke: pass')
            self.assertEqual(second.selected('smoke')['phase'], 'deleted')
        self.assertEqual(set(self.model.states), {'mac-base', 'linux-base'})
        self.assertTrue(all(state == 'stopped' for state in self.model.states.values()))
        self.assertTrue(all(child.poll() == 0 for child in self.model.children))

    def enable_p0(self, scope='full', supported=True, candidate='v0.25.0', *, remote_mac=False):
        """Create a new priority-aware run without rewriting the legacy fixture's history.

The modeled bases are independently stopped. No real release bytes, guest
authentication, GUI outcome or product verdict is borrowed by these tests.
`supported` says whether the fictional previous stable's app honors the update
override, which decides whether its updater pair can discover the candidate.
The existing installer-pair tests exercise layout-2 manifests saved before
the additional Mac pair was scheduled. Tests of the new pair request it.
"""
        self.assertTrue(all(state == 'stopped' for state in self.model.states.values()))
        self.run = Run(self.evidence / ('brick-' + str(uuid.uuid4())), self.profile,
                       factory=self.model.factory, connector=self.model.connect,
                       disk_usage=lambda path: namedtuple('Usage', 'free')(self.free))
        release = plan(candidate, 'v0.24.0', FakeSource(['v0.24.0'] if supported else []), floor='v0.23.0')
        if not remote_mac:
            release['passes'] = [entry for entry in release['passes'] if entry['id'] != REMOTE_MAC_PASS]
        release['sha256sums_sha256'] = 'fixture-checksum'
        self.run.create(release, scope=scope)

    def complete_p0(self, pass_id, through=None):
        """Exercise all real record and cleanup gates with explicitly synthetic observations.

Tiny PNGs prove evidence format only. The fake substrate's stopped/absent
state independently checks cleanup before the next fixture is permitted.
An explicit boundary leaves the pair live for an unsettled evidence-gate test.
Without a boundary, this helper records every in-scope checkpoint, P1 and P2
included, so cleanup cannot silently skip an obligation; out-of-scope tiers
are left unrecorded, as the scheduler leaves them.
"""
        children = self.prepare(pass_id)
        image = self.evidence_file(pass_id + '.png', PNG)
        command = self.evidence_file(pass_id + '.txt', b'synthetic version and agent evidence')
        receipt = self.evidence_file(pass_id + '.json', json.dumps({
            'version': self.run.data['plan']['candidate'], 'previous_trust_ring': 'v0.24.0',
            'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
            'trusted_comment': 'verified', 'installer_checksum': 'verified'}).encode())
        with self.run.locked():
            if pass_id != 'p0-fresh':
                self.run.select_update(pass_id, 'v0.24.0')
            entry = self.run.selected(pass_id)
            for check in entry['required']:
                if tier(entry, check) not in self.run.scope_tiers():
                    continue
                refs = [receipt] if check == 'artifact-verification' else [image, command]
                self.run.record(pass_id, check, 'pass', refs, 'Synthetic evidence contract, not a product observation.')
                if check == through:
                    return children
            self.assertEqual(self.run.verdict(priority_entry(entry, 'P0')), 'pass')
            self.run.cleanup(pass_id)
        self.assertTrue(all(child.poll() == 0 for child in children))
        self.assertEqual(set(self.model.states), {'mac-base', 'linux-base'})

    def complete_stable_p0(self):
        """Pass and release the fresh and updater pairs, leaving the installer pair for the test itself."""
        self.complete_p0('p0-fresh')
        self.complete_p0('p0-updater-upgrade')

    def record_announcements(self, image):
        """Complete the second P2 checkpoint with independent command evidence in a synthetic pair."""
        receipt = self.evidence_file('announcements-launch.json', b'{"fixture": "announcement launch"}')
        self.run.record('p0-installer-upgrade', 'p2-announcements', 'pass', [image, receipt],
                        'Synthetic bell, dialog and history.')

    def test_remote_mac_pair_follows_installer_cleanup_and_counts_as_p1(self):
        """A failed Mac uninstall preserves P0 and feedback, while leaving bonus coverage runnable."""
        self.enable_p0(remote_mac=True)
        self.complete_stable_p0()
        self.complete_p0('p0-installer-upgrade', through='remote-host-uninstall')
        image = self.evidence_file('remote-mac.png', PNG)
        command = self.evidence_file('remote-mac.txt', b'synthetic Mac checkpoint evidence')
        receipt = self.evidence_file('remote-mac-verification.json', json.dumps({
            'version': self.run.data['plan']['candidate'], 'previous_trust_ring': 'v0.24.0',
            'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
            'trusted_comment': 'verified', 'installer_checksum': 'verified'}).encode())
        with self.run.locked():
            self.assertIn('remote-mac-uninstall', self.run.report()['p1']['checks'])
            with self.assertRaisesRegex(Refused, 'clean up the installer pair'):
                self.run.prepare(REMOTE_MAC_PASS, [])
            self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'pass', [image], 'Synthetic feedback result.')
            self.record_announcements(image)
            self.run.cleanup('p0-installer-upgrade')
            with self.assertRaisesRegex(Refused, 'remote Mac fixture before bonus'):
                self.run.prepare('fresh', [])
        first = len(self.model.calls)
        children = self.prepare(REMOTE_MAC_PASS)
        calls = self.model.calls[first:]
        clones = [argv for argv in calls if argv[1] == 'clone']
        self.assertEqual([argv[2] for argv in clones], ['mac-base', 'mac-base'])
        self.assertTrue(clones[1][3].endswith('-remote-mac'))
        self.assertFalse(any('continuity.py' in part for argv in calls for part in argv))
        self.assertEqual(len(children), 2)
        with self.run.locked():
            entry = self.run.selected(REMOTE_MAC_PASS)
            self.assertEqual(list(entry['required']),
                             ['artifact-verification', 'fixture-clean', 'install', 'app-start',
                              'remote-provision', 'remote-host-uninstall'])
            self.assertEqual({tier(entry, check) for check in entry['required']}, {'P1'})
            with self.assertRaisesRegex(Refused, 'bonus passes, not P1'):
                self.run.add_check(REMOTE_MAC_PASS, 'extra-ui')
            for check in entry['required']:
                refs = [receipt] if check == 'artifact-verification' else [image, command]
                failed = check == 'remote-host-uninstall'
                self.run.record(REMOTE_MAC_PASS, check, 'fail' if failed else 'pass', refs, 'Synthetic Mac result.')
            self.run.cleanup(REMOTE_MAC_PASS)
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['checks']['remote-mac-uninstall'], 'fail')
            self.assertEqual(report['p2']['verdict'], 'pass')
        self.assertTrue(all(child.poll() == 0 for child in children))
        self.prepare('fresh')

    def test_remote_mac_pair_is_outside_p0_only_scope(self):
        """The scope fence should refuse P1 before any probe or clone mutation."""
        self.enable_p0(scope='p0-only', remote_mac=True)
        with self.run.locked(), self.assertRaisesRegex(Refused, 'remote Mac fixture is outside'):
            self.run.prepare(REMOTE_MAC_PASS, [])
        self.assertEqual(self.model.calls, [])

    def test_prerelease_reports_its_skipped_discovery_and_still_runs_p1_and_p2(self):
        """An RC or dev run explains the discovery it never planned, and the rest of its coverage still runs.

The formal P0 verdict stays incomplete, because discovery did not run, but the
summary says every runnable P0 check passed once they have. The installer pair
carries the supervisor checks, so they need no updater pair.
"""
        cases = (('v0.25.0-rc.1', 'v0.25.0-rc.1 is a release candidate.'),
                 ('v0.25.0-dev.1', 'v0.25.0-dev.1 is a development prerelease.'))
        for candidate, reason in cases:
            with self.subTest(candidate=candidate):
                self.enable_p0(candidate=candidate)
                with self.run.locked():
                    p0 = self.run.report()['p0']
                    discovery = p0['unavailable_checks']['update-discovery']
                    self.assertIn(reason, discovery)
                    self.assertIn('only discovers stable releases, so this run planned no updater pair', discovery)
                    self.assertIn('public update channel pointed to v0.24.0', discovery)
                    self.assertEqual(p0['checks']['click-to-upgrade'], 'skipped')
                    self.assertNotIn('summary', p0)
                self.complete_p0('p0-fresh')
                self.complete_p0('p0-installer-upgrade')
                with self.run.locked():
                    report = self.run.report()
                    self.assertTrue(self.run.p0_executable_complete())
                    self.assertEqual(report['p0']['verdict'], 'could not run')
                    self.assertEqual(report['p0']['summary'], 'All runnable P0 checks passed. ' + discovery)
                    self.assertEqual({value for key, value in report['p0']['checks'].items()
                                      if key != 'click-to-upgrade'}, {'pass'})
                    self.assertEqual(report['p1']['verdict'], 'pass')
                    self.assertEqual(report['p2']['verdict'], 'pass')
                    self.assertEqual(report['passes']['p0-installer-upgrade']['phase'], 'deleted')
                self.prepare('fresh')
                with self.run.locked():
                    self.run.cleanup('fresh', discard_failed=True)

    def test_p0_fixtures_run_in_order_before_bonus_and_each_answers_its_own_question(self):
        """Broader functionality and one successful upgrade path cannot mask another untested path.

Every P0 named result has its own checkpoints: the installer pair answers the
installer upgrade, each supervisor's upgrade and each new agent separately.
P1 and P2 report their own named results from the same pair.
"""
        self.enable_p0()
        with self.run.locked():
            with self.assertRaisesRegex(Refused, 'complete P0 fixtures'):
                self.run.prepare('fresh', [])
            for pass_id in ('p0-updater-upgrade', 'p0-installer-upgrade'):
                with self.assertRaisesRegex(Refused, 'planned order'):
                    self.run.prepare(pass_id, [])
        self.assertEqual(self.model.calls, [])
        self.complete_stable_p0()
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')
            self.assertEqual(self.run.selected('p0-updater-upgrade')['update_path'], 'override')
            with self.assertRaisesRegex(Refused, 'complete P0 fixtures'):
                self.run.prepare('fresh', [])
        self.complete_p0('p0-installer-upgrade')
        with self.run.locked():
            self.assertEqual(self.run.selected('p0-installer-upgrade')['update_path'], 'manual')
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertNotIn('unavailable_checks', report['p0'])
            self.assertEqual(list(report['p0']['checks']),
                             ['fresh-install-start', 'click-to-upgrade', 'installer-upgrade',
                              'local-supervisor-upgrade', 'remote-supervisor-upgrade', 'local-agent', 'remote-agent'])
            self.assertEqual(set(report['p0']['checks'].values()), {'pass'})
            self.assertEqual(report['p1'], {'verdict': 'pass', 'checks': {
                'local-old-session': 'pass', 'remote-old-session': 'pass',
                'remote-host-remove': 'pass', 'remote-host-uninstall': 'pass'}})
            self.assertEqual(report['p2'], {'verdict': 'pass', 'checks': {
                'feedback-send': 'pass', 'announcements': 'pass'}})
            self.assertEqual(report['verdict'], 'could not run')
        self.prepare('fresh')

    def test_a_failed_p0_pair_stops_its_own_checks_but_not_the_next_p0_pair(self):
        """A broken fresh install says nothing about upgrades, so the next P0 pair may still run.

Within the failed pair nothing else may be recorded, earlier checks included.
The next pair waits only for the failed one to be released, which needs the
maintainer's retention decision (`--discard-failed`); bonus work stays fenced.
"""
        for status, expected in (('fail', 'fail'), ('could-not-run', 'could not run')):
            with self.subTest(status=status):
                self.enable_p0()
                self.prepare('p0-fresh')
                with self.run.locked():
                    self.run.record('p0-fresh', 'app-start', status, [], 'Synthetic failed or unavailable GUI.')
                    self.assertEqual(self.run.report()['p0']['verdict'], expected)
                    with self.assertRaisesRegex(Refused, 'stop after the first'):
                        self.run.record('p0-fresh', 'fixture-clean', 'pass', [], 'Must not continue.')
                    with self.assertRaisesRegex(Refused, 'existing working pair'):
                        self.run.prepare('p0-updater-upgrade', [])
                    self.run.cleanup('p0-fresh', discard_failed=True)
                    with self.assertRaisesRegex(Refused, 'complete P0 fixtures'):
                        self.run.prepare('fresh', [])
                self.prepare('p0-updater-upgrade')
                with self.run.locked():
                    self.run.cleanup('p0-updater-upgrade', discard_failed=True)

    def test_p0_order_discovery_and_agent_evidence_are_real_boundaries(self):
        """Neither a relaunch nor a manual install can substitute updater discovery, and P0 cannot gain bonus checks."""
        self.enable_p0(supported=False)
        self.complete_p0('p0-fresh')
        self.prepare('p0-updater-upgrade')
        image = self.evidence_file('manual.png', PNG)
        command = self.evidence_file('manual.txt', b'synthetic command evidence')
        receipt = self.evidence_file('manual.json', json.dumps({
            'version': 'v0.25.0', 'previous_trust_ring': 'v0.24.0',
            'sha256sums_sha256': 'fixture-checksum', 'signature': 'verified',
            'trusted_comment': 'verified', 'installer_checksum': 'verified'}).encode())
        with self.run.locked():
            self.run.record('p0-updater-upgrade', 'artifact-verification', 'pass', [receipt], 'Synthetic trust receipt.')
            self.run.record('p0-updater-upgrade', 'old-fixture', 'pass', [image, command], 'Synthetic old fixture.')
            with self.assertRaisesRegex(Refused, 'preceding P0'):
                self.run.record('p0-updater-upgrade', 'update-relaunch', 'pass', [image, command], 'Out of order.')
            self.run.select_update('p0-updater-upgrade', 'v0.24.0')
            self.assertEqual(self.run.selected('p0-updater-upgrade')['update_path'], 'manual')
            with self.assertRaisesRegex(Refused, 'manual installer cannot prove'):
                self.run.record('p0-updater-upgrade', 'update-discovery', 'pass', [image, command], 'Manual is not discovery.')
            with self.assertRaisesRegex(Refused, 'bonus passes'):
                self.run.add_check('p0-updater-upgrade', 'extra-settings', gui=True)

    def test_each_supervisor_and_agent_check_needs_command_evidence_and_reports_separately(self):
        """A screenshot alone proves no upgrade or agent reply, and an earlier success cannot waive a later check.

Each supervisor's upgrade answers its own question before any agent runs on
it, so a passing upgrade with an unrun agent check shows exactly that.
"""
        # P1-only scope keeps the continuity checks in this regression without
        # inventing a feedback-send observation merely to clean its pair.
        self.enable_p0(scope='p1-only')
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='update-relaunch')
        self.assertTrue(all(child.poll() is None for child in children))
        image = self.evidence_file('agent.png', PNG)
        command = self.evidence_file('agent.txt', b'synthetic agent response receipt')
        with self.run.locked():
            for check in ('local-supervisor-upgrade', 'remote-supervisor-upgrade', 'local-supervisor-agent',
                          'remote-supervisor-agent', 'local-old-session', 'remote-old-session',
                          'remote-host-remove', 'remote-host-uninstall'):
                with self.assertRaisesRegex(Refused, 'command/identity evidence'):
                    self.run.record('p0-installer-upgrade', check, 'pass', [image], 'GUI alone is not execution evidence.')
                self.run.record('p0-installer-upgrade', check, 'pass', [image, command], 'Synthetic evidence format only.')
                report = self.run.report()
                if check == 'remote-supervisor-upgrade':
                    self.assertEqual(report['p0']['checks']['remote-supervisor-upgrade'], 'pass')
                    self.assertEqual(report['p0']['checks']['local-agent'], 'could not run')
                    self.assertEqual(report['p0']['verdict'], 'could not run')
                if check == 'remote-supervisor-agent':
                    self.assertEqual(report['p0']['verdict'], 'pass')
                    self.assertEqual(report['p1']['verdict'], 'could not run')
            self.assertEqual(self.run.report()['p1']['verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')

    def test_a_failed_old_session_stops_p1_but_keeps_p0_feedback_cleanup_and_bonus(self):
        """Losing a session is a continuity failure, not a bricked install, so it stops only the P1 chain.

The later P1 checks share the same sessions and stop with it. P0 keeps its
pass, the feedback send still runs, and the pair cleans up normally once the
failure's evidence is retained.
"""
        self.enable_p0()
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='remote-supervisor-agent')
        image = self.evidence_file('old-session.png', PNG)
        with self.run.locked():
            self.run.record('p0-installer-upgrade', 'local-old-session', 'fail', [image],
                            'Synthetic failed old-session observation.')
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'fail')
            self.assertEqual(report['p1']['checks']['local-old-session'], 'fail')
            self.assertEqual(report['p1']['checks']['remote-old-session'], 'could not run')
            skipped = self.run.selected('p0-installer-upgrade')['results']['remote-old-session']
            self.assertEqual(skipped['status'], 'skipped')
            self.assertIn('local-old-session stopped its chain', skipped['detail'])
            with self.assertRaisesRegex(Refused, 'already exists'):
                self.run.record('p0-installer-upgrade', 'remote-old-session', 'pass', [image, image],
                                'Must not continue.')
            self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'pass', [image],
                            'Synthetic feedback frames; no live send claimed.')
            self.record_announcements(image)
            self.assertEqual(self.run.report()['p2']['verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))
        bonus = self.prepare('fresh')
        with self.run.locked():
            self.run.cleanup('fresh', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in bonus))

    def test_unavailable_old_sessions_leave_p0_running_and_p1_unavailable(self):
        """Old sessions start on the previous release, so failing to start them says nothing about the candidate.

The P0 checks after `old-sessions` still run; every old-session and remote
menu check is then unavailable rather than failed.
"""
        self.enable_p0(scope='p1-only')
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='old-fixture')
        image = self.evidence_file('old-sessions.png', PNG)
        command = self.evidence_file('old-sessions.txt', b'synthetic version receipt')
        with self.run.locked():
            self.run.record('p0-installer-upgrade', 'old-sessions', 'could-not-run', [image],
                            'Synthetic: the previous release never captured its conversation.')
            for check in ('update-relaunch', 'local-supervisor-upgrade', 'remote-supervisor-upgrade',
                          'local-supervisor-agent', 'remote-supervisor-agent'):
                self.run.record('p0-installer-upgrade', check, 'pass', [image, command], 'Synthetic P0 evidence.')
            results = self.run.selected('p0-installer-upgrade')['results']
            for check in ('local-old-session', 'remote-old-session', 'remote-host-remove', 'remote-host-uninstall'):
                self.assertEqual(results[check]['status'], 'skipped')
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'pass')
            self.assertEqual(report['p1']['verdict'], 'could not run')
            self.assertEqual(report['p1']['checks']['local-old-session'], 'could not run')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_a_failed_discovery_does_not_hide_the_supervisor_upgrade(self):
        """The supervisor checks moved to the installer pair so a broken updater cannot hide a bricked supervisor.

After the failed updater pair is released, the installer pair runs P0 and
P1. P2 can never be dispatched, since P0 failed elsewhere, so that pair still
cleans up normally. The independent Mac check can run then. Bonus stays fenced.
"""
        self.enable_p0(supported=False, remote_mac=True)
        self.complete_p0('p0-fresh')
        self.prepare('p0-updater-upgrade')
        with self.run.locked():
            self.run.select_update('p0-updater-upgrade', 'v0.24.0')
            self.run.record('p0-updater-upgrade', 'update-discovery', 'fail',
                            ['p0-updater-upgrade/latest-at-selection.json'], 'Synthetic: the old app found nothing.')
            self.assertEqual(self.run.report()['p0']['checks']['click-to-upgrade'], 'fail')
            self.run.cleanup('p0-updater-upgrade', discard_failed=True)
        children = self.complete_p0('p0-installer-upgrade', through='remote-host-uninstall')
        image = self.evidence_file('feedback.png', PNG)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'fail')
            self.assertEqual(report['p0']['checks']['local-supervisor-upgrade'], 'pass')
            self.assertEqual(report['p0']['checks']['remote-agent'], 'pass')
            self.assertEqual(report['p1']['checks']['remote-host-uninstall'], 'pass')
            self.assertEqual(report['p1']['checks']['remote-mac-uninstall'], 'could not run')
            send = self.run.selected('p0-installer-upgrade')['results']['p2-feedback-send']
            self.assertEqual(send['status'], 'skipped')
            self.assertIn('p0-updater-upgrade did not pass', send['detail'])
            with self.assertRaisesRegex(Refused, 'already exists'):
                self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'pass', [image], 'Never dispatched.')
            self.run.cleanup('p0-installer-upgrade')
            self.run.require_priority(REMOTE_MAC_PASS)
            with self.assertRaisesRegex(Refused, 'complete P0 fixtures'):
                self.run.prepare('fresh', [])
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_p0_only_scope_and_changed_evidence_fence_bonus_cloning(self):
        """A requested P0 stop stays effective, and stale successful evidence never authorizes more work."""
        self.enable_p0(scope='p0-only')
        for pass_id in self.run.data['plan']['p0_sequence']:
            self.complete_p0(pass_id)
        calls = len(self.model.calls)
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['requested_verdict'], 'pass')
            self.assertEqual(report['verdict'], 'could not run')
            self.assertEqual(report['p1']['verdict'], 'could not run')
            self.assertEqual(self.run.selected('p0-installer-upgrade')['results'].keys() & {'old-sessions'}, set())
            with self.assertRaisesRegex(Refused, 'outside this run scope'):
                self.run.prepare('fresh', [])
            # Revalidate bytes even after successful cleanup: a corrupt export
            # invalidates the P0 answer without resurrecting any guest.
            (self.run.directory / 'p0-fresh.png').write_bytes(b'changed image')
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')
        self.assertEqual(len(self.model.calls), calls)

    def test_mid_session_stop_and_stale_evidence_refuse_new_bonus_work(self):
        """A later stop request persists, while corrupt evidence fences a full run without relying on P0-only scope."""
        self.enable_p0()
        for pass_id in self.run.data['plan']['p0_sequence']:
            self.complete_p0(pass_id)
        calls = len(self.model.calls)
        (self.run.directory / 'p0-fresh.png').write_bytes(b'changed image')
        with self.run.locked(), self.assertRaisesRegex(Refused, 'complete P0'):
            self.run.prepare('fresh', [])
        self.assertEqual(len(self.model.calls), calls)
        self.enable_p0()
        self.complete_p0('p0-fresh')
        with self.run.locked():
            self.run.limit_scope('p0-only')
        with self.run.locked():
            self.assertEqual(self.run.report()['scope'], 'p0-only')
            with self.assertRaisesRegex(Refused, 'outside this run scope'):
                self.run.prepare('fresh', [])

    def test_legacy_report_does_not_relabel_old_observations_as_p0(self):
        """Resuming historical manifests preserves their original coverage instead of inventing missing upgrade paths."""
        with self.run.locked():
            report = self.run.report()
            self.assertEqual(report['p0']['verdict'], 'not planned')
            self.assertEqual(report['p1']['verdict'], 'not planned')
            self.assertEqual(report['p2']['verdict'], 'not planned')

    def test_run_planned_under_the_earlier_layout_is_refused_on_load(self):
        """An in-flight run from before P0/P1/P2 must finish under the coordinator that started it.

Its checkpoints and fences follow the earlier contract, so every command that
loads it is refused before touching a guest, and the `report.json` its own
coordinator saved stays as it was written.
"""
        self.enable_p0()
        self.prepare('p0-fresh')
        with self.run.locked():
            self.run.data['plan'].pop('priority_layout')
            self.run.save()
        saved = self.run.directory / 'report.json'
        saved.write_text(json.dumps({'verdict': 'could not run', 'saved_by': 'the earlier coordinator'}))
        before = saved.read_bytes()
        calls = len(self.model.calls)
        with self.assertRaisesRegex(Refused, 'earlier P0/P1 layout'):
            with self.run.locked():
                self.fail('an earlier-layout run must not load')
        self.assertEqual(saved.read_bytes(), before)
        self.assertEqual(len(self.model.calls), calls)

    def ready_for_p2(self, scope='full'):
        """Leave the installer pair live after independently validated P0 and P1.

This is a synthetic coordinator boundary, not a real feedback submission.
The two preceding pairs have already passed their evidence and cleanup gates.
"""
        self.enable_p0(scope=scope)
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='remote-host-uninstall')
        self.assertTrue(all(child.poll() is None for child in children))
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
            self.assertEqual(self.run.report()['p1']['verdict'], 'pass')
            self.assertEqual(self.run.report()['p2']['verdict'], 'could not run')
            self.assertNotIn('p2-feedback-send', self.run.selected('p0-installer-upgrade')['results'])
        return children

    def test_p2_is_gui_only_and_required_before_last_pair_cleanup(self):
        """A P0 and P1 pass neither invents a feedback result nor permits skipping the in-scope send."""
        children = self.ready_for_p2(scope='p2-only')
        before = self.evidence_file('feedback-filled.png', PNG)
        after = self.evidence_file('feedback-closed.png', PNG)
        with self.run.locked():
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('p0-installer-upgrade')
            self.assertTrue(all(child.poll() is None for child in children))
            self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'pass', [before, after],
                            'Synthetic filled/closed frames; no live send or GUI result claimed.')
            self.assertEqual(self.run.report()['p2']['verdict'], 'could not run')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('p0-installer-upgrade')
            self.record_announcements(before)
            report = self.run.report()
            self.assertEqual(report['p2'], {'verdict': 'pass', 'checks': {
                'feedback-send': 'pass', 'announcements': 'pass'}})
            self.assertEqual(report['requested_verdict'], 'pass')
            self.assertEqual(report['verdict'], 'could not run')
            self.run.cleanup('p0-installer-upgrade')
            with self.assertRaisesRegex(Refused, 'outside this run scope'):
                self.run.prepare('fresh', [])
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_lost_p2_evidence_preserves_guests_until_recovered(self):
        """Every P2 status retains its source guests until its exported evidence is recovered.

A late stop after P0 (the scope narrowed once P2 is already recorded) must not
lift that protection either: the scope fences future work, not evidence.
"""
        cases = ((status, damage, narrowed)
                 for status in ('pass', 'fail', 'could-not-run')
                 for damage, narrowed in (('changed', False), ('missing', False), ('missing', True)))
        for status, damage, narrowed in cases:
            with self.subTest(status=status, damage=damage, narrowed=narrowed):
                children = self.ready_for_p2()
                image = self.evidence_file('p2-only.png', PNG)
                expected = 'could not run' if status == 'could-not-run' else status
                with self.run.locked():
                    self.run.record('p0-installer-upgrade', 'p2-feedback-send', status, [image],
                                    'Synthetic P2 image independent of every P0 artifact.')
                    self.record_announcements(image)
                    self.assertEqual(self.run.report()['p2']['verdict'], expected)
                    if narrowed:
                        self.run.limit_scope('p0-only')
                    path = self.run.directory / image
                    if damage == 'changed':
                        path.write_bytes(b'changed P2-only image')
                    else:
                        path.unlink()
                    report = self.run.report()
                    self.assertEqual(report['p0']['verdict'], 'pass')
                    self.assertEqual(report['p2']['verdict'], 'fail' if status == 'fail' else 'could not run')
                    calls = len(self.model.calls)
                    with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                        self.run.cleanup('p0-installer-upgrade')
                    self.assertEqual(len(self.model.calls), calls, 'refusal must precede shutdown or deletion')
                    self.assertTrue(all(child.poll() is None for child in children))
                    # Restoring the exact accepted bytes makes cleanup possible;
                    # a blanket P2 cleanup refusal would fail this half.
                    path.write_bytes(PNG)
                    self.assertEqual(self.run.report()['p2']['verdict'], expected)
                    self.run.cleanup('p0-installer-upgrade')
                self.assertTrue(all(child.poll() == 0 for child in children))

    def test_every_p2_result_requires_all_valid_p0_evidence_and_a_finished_p1_sequence(self):
        """Failed and unavailable sends must not bypass the ordering fence that protects passing records.

P2 needs every P0 check of the run, an earlier pair's included, with retained
evidence. P1's results are not its prerequisite, but the P1 sequence must be
over: a send recorded while a P1 check could still run would close that check
for good. An unavailable P1 check that stops the chain ends the sequence.
"""
        self.enable_p0()
        self.complete_stable_p0()
        self.complete_p0('p0-installer-upgrade', through='remote-supervisor-upgrade')
        image = self.evidence_file('p2.png', PNG)
        command = self.evidence_file('p2-agent.txt', b'synthetic agent receipt')
        with self.run.locked():
            for status in ('pass', 'fail', 'could-not-run'):
                with self.subTest(status=status), self.assertRaisesRegex(Refused, 'complete all P0'):
                    self.run.record('p0-installer-upgrade', 'p2-feedback-send', status, [image], 'Out of order.')
            for check in ('local-supervisor-agent', 'remote-supervisor-agent'):
                self.run.record('p0-installer-upgrade', check, 'pass', [image, command], 'Synthetic agent evidence.')
            self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
            # A previously accepted earlier fixture is still a prerequisite:
            # the last pair alone cannot authorize P2 after its bytes changed.
            (self.run.directory / 'p0-fresh.png').write_bytes(b'changed image')
            for status in ('pass', 'fail', 'could-not-run'):
                with self.subTest(invalidated=status), self.assertRaisesRegex(Refused, 'complete all P0'):
                    self.run.record('p0-installer-upgrade', 'p2-feedback-send', status, [image], 'Invalid predecessor.')
            (self.run.directory / 'p0-fresh.png').write_bytes(PNG)
            with self.assertRaisesRegex(Refused, 'finish the P1 sequence before P2: local-old-session'):
                self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'could-not-run', [image], 'Too early.')
            self.run.record('p0-installer-upgrade', 'local-old-session', 'could-not-run', [image],
                            'Synthetic unavailable old session; it stops the P1 chain.')
            self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'pass', [image], 'After P1 ended.')
            self.record_announcements(image)
            self.assertEqual(self.run.report()['p2']['verdict'], 'pass')
            self.assertEqual(self.run.report()['p1']['verdict'], 'could not run')

    def test_p0_and_p1_stops_refuse_later_tiers_but_clean_the_last_pair(self):
        """A stop after P0 records no continuity or feedback result, and a stop after P1 sends nothing."""
        self.enable_p0(scope='p0-only')
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='remote-supervisor-agent')
        image = self.evidence_file('scoped.png', PNG)
        with self.run.locked():
            for check, priority in (('local-old-session', 'P1'), ('p2-feedback-send', 'P2')):
                with self.subTest(check=check), self.assertRaisesRegex(Refused, priority + ' is outside'):
                    self.run.record('p0-installer-upgrade', check, 'fail', [image], 'Outside scope.')
            self.assertEqual(self.run.report()['requested_verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))
        children = self.ready_for_p2(scope='p1-only')
        with self.run.locked():
            for status in ('pass', 'fail', 'could-not-run'):
                with self.subTest(status=status), self.assertRaisesRegex(Refused, 'P2 is outside'):
                    self.run.record('p0-installer-upgrade', 'p2-feedback-send', status, [image], 'Outside scope.')
            self.assertEqual(self.run.report()['requested_verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_failed_p2_preserves_p0_and_p1_and_allows_cleanup_and_bonus_observations(self):
        """Feedback outages do not hold disposable guests or suppress independent bonus checks."""
        for status, expected in (('fail', 'fail'), ('could-not-run', 'could not run')):
            with self.subTest(status=status):
                children = self.ready_for_p2()
                image = self.evidence_file('failed-feedback.png', PNG)
                with self.run.locked():
                    self.run.record('p0-installer-upgrade', 'p2-feedback-send', status, [image],
                                    'Synthetic failed/unavailable feedback observation.')
                    self.record_announcements(image)
                    report = self.run.report()
                    self.assertEqual(report['p0']['verdict'], 'pass')
                    self.assertEqual(set(report['p0']['checks'].values()), {'pass'})
                    self.assertEqual(report['p1']['verdict'], 'pass')
                    self.assertEqual(report['p2']['verdict'], expected)
                    self.assertEqual(report['verdict'], expected)
                    self.run.cleanup('p0-installer-upgrade')
                    self.assertEqual(self.run.report()['p2']['verdict'], expected)
                self.assertTrue(all(child.poll() == 0 for child in children))
                bonus = self.prepare('fresh')
                with self.run.locked():
                    self.run.record('fresh', 'fixture-clean', 'pass', [image], 'Synthetic fresh fixture.')
                    self.assertEqual(self.run.selected('fresh')['results']['fixture-clean']['status'], 'pass')
                    self.run.cleanup('fresh', discard_failed=True)
                self.assertTrue(all(child.poll() == 0 for child in bonus))

    def test_mid_session_p2_stop_retains_each_result_and_refuses_bonus(self):
        """Stopping after P2 includes its outcome; a failed send cannot become a narrower requested pass."""
        for status, expected in (('pass', 'pass'), ('fail', 'fail'), ('could-not-run', 'could not run')):
            with self.subTest(status=status):
                children = self.ready_for_p2()
                image = self.evidence_file('p2-scope.png', PNG)
                with self.run.locked():
                    self.run.limit_scope('p2-only')
                    self.run.record('p0-installer-upgrade', 'p2-feedback-send', status, [image], 'Synthetic P2 result.')
                    self.record_announcements(image)
                    self.assertEqual(self.run.report()['scope'], 'p2-only')
                    self.assertEqual(self.run.report()['requested_verdict'], expected)
                    self.run.cleanup('p0-installer-upgrade')
                    with self.assertRaisesRegex(Refused, 'outside this run scope'):
                        self.run.prepare('fresh', [])
                self.assertTrue(all(child.poll() == 0 for child in children))

    def test_widening_after_the_upgrade_cannot_start_old_sessions_on_the_candidate(self):
        """A baseline skipped under a stop after P0 can never run once the pair's upgrade ran.

Its sessions would start on the candidate and compare it with itself. Under
the stop it stays unrecorded; widening records it and every P1 check behind
it as skipped, the send still runs, and the pair cleans up normally.
"""
        self.enable_p0(scope='p0-only')
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='remote-supervisor-agent')
        image = self.evidence_file('widened.png', PNG)
        command = self.evidence_file('widened.txt', b'synthetic version receipt')
        with self.run.locked():
            self.assertNotIn('old-sessions', self.run.selected('p0-installer-upgrade')['results'])
            self.run.continue_full()
            results = self.run.selected('p0-installer-upgrade')['results']
            self.assertIn('update-relaunch, later in the pair, already ran', results['old-sessions']['detail'])
            for check in ('old-sessions', 'local-old-session', 'remote-old-session', 'remote-host-remove',
                          'remote-host-uninstall'):
                self.assertEqual(results[check]['status'], 'skipped')
            with self.assertRaisesRegex(Refused, 'already exists'):
                self.run.record('p0-installer-upgrade', 'old-sessions', 'pass', [image, command], 'On the candidate.')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('p0-installer-upgrade')
            self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'pass', [image], 'Synthetic send frames.')
            self.record_announcements(image)
            report = self.run.report()
            self.assertEqual((report['p0']['verdict'], report['p1']['verdict'], report['p2']['verdict']),
                             ('pass', 'could not run', 'pass'))
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_widening_to_p1_after_the_upgrade_closes_the_whole_p1_chain_and_cleans_up(self):
        """Checks behind a baseline that can never run can never run either, so they cannot hold the pair.

Widening a stop after P0 to a stop after P1 once the upgrade ran leaves no P1
check runnable and no send in scope; the pair must clean up normally rather
than wait forever for a sequence that cannot happen.
"""
        self.enable_p0(scope='p0-only')
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='remote-supervisor-agent')
        with self.run.locked():
            self.run.limit_scope('p1-only')
            entry = self.run.selected('p0-installer-upgrade')
            for check in ('old-sessions', 'local-old-session', 'remote-old-session', 'remote-host-remove',
                          'remote-host-uninstall'):
                self.assertEqual(entry['results'][check]['status'], 'skipped')
            self.assertEqual(self.run.report()['p1']['verdict'], 'could not run')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_a_baseline_failure_on_the_previous_release_is_recorded_as_unavailable(self):
        """Nothing `old-sessions` sees can fail the candidate: it runs before any candidate code.

A fail reported there is kept as could not run, with the original
observation in its detail, so P1 is unavailable rather than failed.
"""
        self.enable_p0(scope='p1-only')
        self.complete_stable_p0()
        children = self.complete_p0('p0-installer-upgrade', through='old-fixture')
        image = self.evidence_file('baseline.png', PNG)
        with self.run.locked():
            self.run.record('p0-installer-upgrade', 'old-sessions', 'fail', [image], 'Old app broke.')
            result = self.run.selected('p0-installer-upgrade')['results']['old-sessions']
            self.assertEqual(result['status'], 'could-not-run')
            self.assertIn('Old app broke.', result['detail'])
            self.assertEqual(self.run.report()['p1']['verdict'], 'could not run')
            self.run.cleanup('p0-installer-upgrade', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_a_released_p0_pair_with_a_harness_failure_does_not_block_the_next_p0_pair(self):
        """A relaunch that never brought the candidate back ends as a harness failure; it must not hide the supervisors.

While the failed pair is live, nothing else may start. Once the maintainer
released it, the next P0 pair may, while bonus work stays fenced.
"""
        self.enable_p0()
        self.complete_p0('p0-fresh')
        self.prepare('p0-updater-upgrade')
        with self.run.locked():
            self.run.selected('p0-updater-upgrade')['automation_failure'] = {
                'check_id': 'update-relaunch', 'status': 'could-not-run', 'kind': 'harness-failure',
                'detail': 'Synthetic: the candidate never came back after Restart to update.'}
            self.run.save()
            with self.assertRaisesRegex(Refused, 'stop after the first'):
                self.run.require_priority('p0-installer-upgrade')
            self.run.cleanup('p0-updater-upgrade', discard_failed=True)
            self.run.require_priority('p0-installer-upgrade')
            with self.assertRaisesRegex(Refused, 'complete P0 fixtures'):
                self.run.require_priority('fresh')
            self.assertEqual(self.run.report()['p0']['checks']['click-to-upgrade'], 'could not run')
        # The installer pair then runs P0 and P1; its send can never run, so
        # it is recorded as skipped and the pair cleans up normally without a
        # retention decision.
        children = self.complete_p0('p0-installer-upgrade', through='remote-host-uninstall')
        with self.run.locked():
            send = self.run.selected('p0-installer-upgrade')['results']['p2-feedback-send']
            self.assertEqual(send['status'], 'skipped')
            self.assertIn('p0-updater-upgrade was released before its P0 checks finished', send['detail'])
            self.assertEqual(self.run.report()['p1']['verdict'], 'pass')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_a_released_incomplete_p0_pair_makes_the_send_unreachable_not_pending(self):
        """A P0 pair discarded with checks never recorded can never complete, so the installer pair need not wait on P2.

Discovery cannot run on a deleted fixture. Without a recorded failure the
installer pair's send stays undispatchable all the same, and its pair must
clean up normally rather than ask for a second retention decision.
"""
        self.enable_p0()
        self.complete_p0('p0-fresh')
        self.prepare('p0-updater-upgrade')
        with self.run.locked():
            self.run.cleanup('p0-updater-upgrade', discard_failed=True)
            self.assertEqual(self.run.selected('p0-updater-upgrade')['results'], {})
        children = self.complete_p0('p0-installer-upgrade', through='remote-host-uninstall')
        with self.run.locked():
            self.assertEqual(self.run.report()['p0']['checks']['click-to-upgrade'], 'could not run')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_a_failure_claim_from_the_previous_release_never_blocks_promotion(self):
        """An aborted `old-sessions` turn that claimed fail saw only the previous release, not the candidate."""
        self.enable_p0()
        with self.run.locked():
            self.run.data['gui_backend'] = 'guest-native'
            for check in ('old-sessions', 'old-fixture'):
                self.run.selected('p0-installer-upgrade')['tester'] = {
                    'phase': 'failed', 'check_id': check,
                    'observation': {'status': 'fail', 'detail': 'Synthetic aborted-turn claim.'}}
                report = self.run.report()
                with self.subTest(check=check):
                    self.assertEqual(report['promotion_blocked_pending_review'], check != 'old-sessions')

    def test_p2_is_unavailable_after_a_released_p0_automation_failure(self):
        """A complete set of product results does not erase a failed owning attempt."""
        self.enable_p0()
        self.complete_stable_p0()
        with self.run.locked():
            entry = self.run.selected('p0-updater-upgrade')
            self.assertEqual((entry['phase'], self.run.verdict(entry)), ('deleted', 'pass'))
            entry['automation_failure'] = {'check_id': 'update-relaunch', 'status': 'could-not-run'}
            self.run.save()
        children = self.complete_p0('p0-installer-upgrade', through='remote-host-uninstall')
        with self.run.locked():
            entry = self.run.selected('p0-installer-upgrade')
            self.assertEqual(entry['results']['p2-feedback-send']['status'], 'skipped')
            self.assertEqual(entry['results']['p2-announcements']['status'], 'skipped')
            self.run.cleanup('p0-installer-upgrade')
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_empty_evidence_cannot_settle_a_nonpass_for_cleanup(self):
        """A zero-byte diagnostic is no evidence; the guests behind the observation must stay."""
        children = self.ready_for_p2()
        for name, value in (('empty.log', b''), ('empty.tar', None)):
            if value is None:
                with tarfile.open(self.run.directory / name, 'w') as archive:
                    archive.addfile(tarfile.TarInfo('./diagnostic.log'), io.BytesIO(b''))
        with self.run.locked():
            self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'could-not-run',
                            [self.evidence_file('empty.log', b''), 'empty.tar::./diagnostic.log'],
                            'Hand-made unavailable send with only empty files.')
            with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                self.run.cleanup('p0-installer-upgrade')
            self.run.cleanup('p0-installer-upgrade', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in children))

    def test_a_running_tester_owns_its_pass_against_hand_made_records(self):
        """A manual record must not take the slot of a guest result on its way back, or stop a chain under it."""
        self.enable_p0()
        self.prepare('p0-fresh')
        image = self.evidence_file('manual.png', PNG)
        with self.run.locked():
            for phase in ('running', 'reconciliation-required'):
                with self.subTest(phase=phase):
                    self.run.selected('p0-fresh')['tester'] = {'phase': phase, 'check_id': 'app-start',
                                                               'termination_observed': True}
                    for check in ('app-start', 'fixture-clean'):
                        with self.assertRaisesRegex(Refused, 'guest tester owns this pass'):
                            self.run.record('p0-fresh', check, 'could-not-run', [image], 'Hand-made observation.')
            self.assertEqual(self.run.selected('p0-fresh')['results'], {})

    def test_cleanup_keeps_a_pair_whose_later_tier_lacks_evidence_or_hit_a_harness_failure(self):
        """Ordinary cleanup needs retained bytes behind a nonpass, and no unresolved automation failure.

A hand-made unavailable send with no evidence, or a send whose import failed
in the harness, may still need the live guests; only the maintainer's
retention decision (`--discard-failed`) releases them.
"""
        for case in ('no evidence', 'automation failure', 'missing P1 retry'):
            with self.subTest(case=case):
                children = self.ready_for_p2()
                with self.run.locked():
                    if case == 'no evidence':
                        self.run.record('p0-installer-upgrade', 'p2-feedback-send', 'could-not-run', [],
                                        'Hand-made unavailable send without evidence.')
                    elif case == 'automation failure':
                        self.run.selected('p0-installer-upgrade')['automation_failure'] = {
                            'check_id': 'p2-feedback-send', 'status': 'could-not-run', 'kind': 'harness-failure',
                            'detail': 'Synthetic import failure after Send.'}
                        self.run.limit_scope('p1-only')
                    else:
                        self.run.limit_scope('p0-only')
                        self.run.selected('p0-installer-upgrade')['provider_retries'] = [
                            {'check_id': 'old-sessions', 'sha256': {'missing-retry.tar::./x': 'f' * 64}}]
                        self.assertEqual(self.run.report()['p0']['verdict'], 'pass')
                    with self.assertRaisesRegex(Refused, 'incomplete or failed'):
                        self.run.cleanup('p0-installer-upgrade')
                    self.assertTrue(all(child.poll() is None for child in children))
                    self.run.cleanup('p0-installer-upgrade', discard_failed=True)
                self.assertTrue(all(child.poll() == 0 for child in children))

    def test_a_confirmed_failure_outranks_a_later_harness_failure_in_every_verdict(self):
        """A recorded lost session must still fail the run when the feedback turn later aborts in the harness."""
        self.enable_p0()
        self.complete_stable_p0()
        self.complete_p0('p0-installer-upgrade', through='remote-supervisor-agent')
        image = self.evidence_file('lost-session.png', PNG)
        with self.run.locked():
            self.run.record('p0-installer-upgrade', 'local-old-session', 'fail', [image], 'Synthetic lost session.')
            self.run.selected('p0-installer-upgrade')['automation_failure'] = {
                'check_id': 'p2-feedback-send', 'status': 'could-not-run', 'kind': 'harness-failure',
                'detail': 'Synthetic aborted feedback turn.'}
            report = self.run.report()
            self.assertEqual(report['p1']['verdict'], 'fail')
            self.assertEqual(report['p2']['verdict'], 'could not run')
            self.assertEqual(report['passes']['p0-installer-upgrade']['verdict'], 'fail')
            self.assertEqual((report['verdict'], report['requested_verdict']), ('fail', 'fail'))

    def test_lost_retry_evidence_invalidates_only_the_checks_it_belongs_to(self):
        """A missing archive from a P1 provider retry must not invalidate the pair's P0 checks or fence P2.

Retry history of the pass-wide native preflight still counts everywhere.
"""
        children = self.ready_for_p2()
        with self.run.locked():
            entry = self.run.selected('p0-installer-upgrade')
            entry['provider_retries'] = [{'check_id': 'old-sessions', 'sha256': {'missing-retry.tar::./x': 'f' * 64}}]
            self.run.save()
            report = self.run.report()
            self.assertEqual((report['p0']['verdict'], report['p1']['verdict']), ('pass', 'could not run'))
            self.assertIsNone(self.run.chain_block('p0-installer-upgrade', 'p2-feedback-send', 'pass'))
            entry['provider_retries'][0]['check_id'] = 'native-preflight'
            self.run.save()
            self.assertEqual(self.run.report()['p0']['verdict'], 'could not run')
            self.run.cleanup('p0-installer-upgrade', discard_failed=True)
        self.assertTrue(all(child.poll() == 0 for child in children))

if __name__ == '__main__':
    unittest.main()
