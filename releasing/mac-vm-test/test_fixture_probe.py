"""Prove clean-fixture observations distinguish no-match from unavailable or dirty state."""

from pathlib import Path
import json
import subprocess
import tempfile
import unittest

from fixture_probe import ProbeError, paths, present, probe, require_clean_pair, merge_peer


class Commands:
    """Model positive manager readiness independently from a service-list no-match exit."""

    def __init__(self, no_matches=1, error=None, units='', processes='python3\n', launch='PID Status Label\n'):
        self.no_matches = no_matches
        self.error = error
        self.units = units
        self.processes = processes
        self.launch = launch
        self.calls = []

    def __call__(self, argv, **kwargs):
        """Return the actual command boundary's controlled bytes, never process environment changes."""
        self.calls.append(argv)
        if argv[0] == '/bin/ps':
            return subprocess.CompletedProcess(argv, 0, self.processes.encode(), b'')
        if argv[0] == '/bin/launchctl':
            return subprocess.CompletedProcess(argv, 0, self.launch.encode(), b'')
        if 'show' in argv:
            return subprocess.CompletedProcess(argv, 0, b'255\n', b'')
        if self.error is not None:
            return subprocess.CompletedProcess(argv, 1, b'', self.error.encode())
        code = 0 if self.units or 'list-units' in argv else self.no_matches
        return subprocess.CompletedProcess(argv, code, self.units.encode(), b'')


class FixtureProbeTests(unittest.TestCase):
    """Keep expected absence deterministic without weakening required substrate evidence."""

    def inspect(self, commands, role='linux', found=None):
        """Supply explicit fictional paths and independent absence/lookup seams."""
        return probe(role, Path('/fixture/home'), {}, runner=commands,
                     present=lambda path: str(path) == found, which=lambda name: None)

    def test_empty_systemd_exit_one_is_clean_after_positive_manager_probe(self):
        """The real Ubuntu no-match exit must pass only after both managers actually answer."""
        commands = Commands(no_matches=1)
        observed = self.inspect(commands)
        self.assertTrue(observed['clean'])
        self.assertEqual(observed['services'], [])
        self.assertEqual(sum('show' in argv for argv in commands.calls), 2)
        self.assertEqual(sum('list-unit-files' in argv or 'list-units' in argv for argv in commands.calls), 4)

    def test_connection_failure_is_not_absence(self):
        """An empty stdout accompanied by diagnostics cannot masquerade as systemd's no-match result."""
        with self.assertRaises(ProbeError):
            self.inspect(Commands(error='Failed to connect to bus'))

    def test_other_empty_failure_exit_is_not_absence(self):
        """Missing-command and other nonzero statuses cannot inherit the exit-one exception."""
        with self.assertRaises(ProbeError):
            self.inspect(Commands(no_matches=127))

    def test_inactive_service_and_installed_path_are_dirty(self):
        """A stopped installation is still an upgrade fixture, never a fresh one."""
        self.assertFalse(self.inspect(Commands(units='farhelm-supervisor.service disabled\n'))['clean'])
        self.assertFalse(self.inspect(Commands(), found='/fixture/home/.local/lib/farhelm')['clean'])

    def test_process_names_ignore_evidence_directory_but_detect_product(self):
        """The helper's farhelm-brick-test path must not match its own unrelated Python process."""
        self.assertTrue(self.inspect(Commands(processes='/fixture/farhelm-brick-test/python3\n'))['clean'])
        self.assertFalse(self.inspect(Commands(processes='/fixture/bin/farhelm-desktop\n'))['clean'])

    def test_mac_launch_agent_is_dirty(self):
        """A legacy launch agent invalidates cleanliness even with no current process or binary."""
        self.assertFalse(self.inspect(Commands(launch='- 0 org.example.farhelm\n'), role='mac')['clean'])

    def test_removed_app_preferences_and_staging_are_residue(self):
        """Removing the bundle alone must not make a previously used or interrupted installation fresh."""
        self.assertFalse(self.inspect(Commands(), role='mac',
                         found='/fixture/home/Library/Preferences/org.scode.farhelm.desktop.plist')['clean'])
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            applications = home / 'Applications'
            applications.mkdir()
            (applications / '.farhelm-app-build.fixture').mkdir()
            observed = probe('mac', home, {}, runner=Commands(),
                             present=lambda path: Path(path).is_relative_to(home) and present(path),
                             which=lambda name: None)
            self.assertFalse(observed['clean'])

    def test_peer_completion_must_agree_with_its_observation(self):
        """A clean label cannot hide SSH failure, wrong peer identity or conflicting exit status."""
        good = {'format': 1, 'clean': True, 'observations': [self.inspect(Commands())]}
        def peer(value, code=0, stderr=b''):
            """Create independent transport and JSON observations without making a network connection."""
            return subprocess.CompletedProcess([], code, json.dumps(value).encode(), stderr)
        self.assertEqual(merge_peer(peer(good)), good['observations'])
        for code, stderr in ((255, b''), (1, b''), (0, b'warning')):
            with self.subTest(code=code, stderr=stderr), self.assertRaises(ProbeError):
                merge_peer(peer(good, code, stderr))
        good['observations'][0]['role'] = 'mac'
        with self.assertRaises(ProbeError):
            merge_peer(peer(good))

    def test_absolute_xdg_paths_are_checked_with_defaults(self):
        """Absolute overrides add relevant state/unit locations; relative inputs cannot redirect them."""
        home = Path('/fixture/home')
        absolute = paths(home, {'XDG_STATE_HOME': '/fixture/state', 'XDG_CONFIG_HOME': '/fixture/config'})
        self.assertIn(Path('/fixture/state/farhelm'), absolute)
        self.assertIn(home / '.local/state/farhelm', absolute)
        self.assertIn(Path('/fixture/config/systemd/user/farhelm-supervisor.service'), absolute)
        self.assertEqual(paths(home, {}), paths(home, {'XDG_STATE_HOME': 'relative', 'XDG_CONFIG_HOME': 'relative'}))

    def test_broken_symlink_counts_as_residue(self):
        """A missing symlink target must not hide an interrupted or foreign installation."""
        with tempfile.TemporaryDirectory() as directory:
            link = Path(directory) / 'farhelm'
            link.symlink_to(Path(directory) / 'absent-target')
            self.assertTrue(present(link))
            self.assertFalse(present(Path(directory) / 'absent-target'))

    def test_positive_manager_observation_precedes_no_match(self):
        """The no-match exception is unavailable when a required manager's own query fails."""
        commands = Commands()
        def failed_manager(argv, **kwargs):
            """Retain normal command behavior except the independent user-manager readiness query."""
            if 'show' in argv:
                return subprocess.CompletedProcess(argv, 1, b'', b'Failed to connect')
            return commands(argv, **kwargs)
        with self.assertRaises(ProbeError):
            probe('linux', Path('/fixture/home'), {}, runner=failed_manager,
                  present=lambda path: False, which=lambda name: None)
        self.assertFalse(any('list-units' in argv or 'list-unit-files' in argv for argv in commands.calls))

    def test_pair_receipt_requires_concrete_clean_observations(self):
        """A top-level clean label cannot hide dirty services, omitted peer checks or duplicate guest roles."""
        mac = self.inspect(Commands(), role='mac')
        linux = self.inspect(Commands())
        pair = {'format': 1, 'clean': True, 'observations': [mac, linux]}
        self.assertEqual(require_clean_pair(pair), pair)
        linux['services'] = ['farhelm-supervisor.service inactive']
        with self.assertRaises(ProbeError):
            require_clean_pair(pair)
        linux['services'] = []
        linux['commands'].pop()
        with self.assertRaises(ProbeError):
            require_clean_pair(pair)
        pair['observations'] = [mac, mac]
        with self.assertRaises(ProbeError):
            require_clean_pair(pair)

    def test_receipt_cannot_hide_omitted_paths_or_contradictory_commands(self):
        """Host revalidation must expose partial path coverage and product processes hidden by summary booleans."""
        for defect in ('path', 'process', 'launch-agent'):
            with self.subTest(defect=defect):
                mac = self.inspect(Commands(), role='mac')
                pair = {'format': 1, 'clean': True, 'observations': [mac, self.inspect(Commands())]}
                if defect == 'path':
                    mac['paths'].pop()
                elif defect == 'process':
                    mac['commands'][0]['stdout'] = '/fixture/bin/farhelm-desktop\n'
                else:
                    mac['commands'][1]['stdout'] = '- 0 org.example.farhelm\n'
                with self.assertRaises(ProbeError):
                    require_clean_pair(pair)


if __name__ == '__main__':
    unittest.main()
