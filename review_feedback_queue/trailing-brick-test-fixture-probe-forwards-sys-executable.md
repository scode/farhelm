# fixture_probe.py sends the GUI Mac's resolved sys.executable to remote-mac instead of the configured Python path

Reviewed commit: 54635894d9c4cab31de631edf8fab2a6479c8a88

## TLDR

When the release brick test uses a separate macOS base image for the headless remote Mac peer, the clean-fixture probe
can fail to start over SSH even when the peer has Python installed at the documented path, because the GUI Mac sends its
own resolved Python binary path instead of the configured path.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/fixture_probe.py:229`.

In releasing/mac-vm-test/fixture_probe.py:218-231, peer_probe() chooses the remote interpreter for the remote-mac role
using sys.executable (line 229: `interpreter = sys.executable if peer_role == 'remote-mac' else '/usr/bin/python3'`). In
the brick-test workflow, guest_operations.py:638 launches fixture_probe.py with sys.executable, whereas
guest_operations.py:487 (ssh()) invokes the peer using the configured literal `self.task['settings']['python']` and
network.py:22,178 uses `/opt/homebrew/bin/python3`. On macOS, Homebrew builds Python as a framework under Cellar, so
invoking `/opt/homebrew/bin/python3` sets `sys.executable` to a version-specific resolved path (such as
`/opt/homebrew/opt/python@3.14/bin/python3.14` or the underlying Cellar framework binary), not
`/opt/homebrew/bin/python3`. Because machine_profile.py:156-159 and BRICK-TEST.md:287-290 explicitly support a separate
`remote_mac_base` whose documented requirement is `/opt/homebrew/bin/python3` for SSH setup and
`guest_tester.settings.python` for checks, any separate remote Mac image with a different Python minor/patch version or
installation layout behind that symlink will fail `fixture-clean` with file-not-found on the peer even though it
satisfies every documented prerequisite. test_fixture_probe.py:90-98 asserts
`[sys.executable, '-', '--role', 'remote-mac']`, locking in this coupling. How to verify: inspect fixture_probe.py:229,
guest_operations.py:487,638, network.py:22,178, and BRICK-TEST.md:287-290; on macOS with Homebrew Python, compare
`/opt/homebrew/bin/python3 -c 'import sys; print(sys.executable)'` against `/opt/homebrew/bin/python3`. Fix: add an
explicit optional `--peer-python` argument (or default remote-mac to `/opt/homebrew/bin/python3` and pass
`self.task['settings']['python']` from guest_operations.py:638) so the peer command uses the configured path rather than
the caller's resolved `sys.executable`.

Reviewer metadata (g09_brick_test_mac_peer_prep/f1): suggested bucket: other (harm is confined to maintainer
release-testing tooling and only triggers when `remote_mac_base` differs from `mac_base` in its resolved Python binary
path; no user data loss or security impact).; confidence: confirmed (supported by source analysis of
fixture_probe.py:229, guest_operations.py:487,638, machine_profile.py:156-159, BRICK-TEST.md:287-290, and standard macOS
Homebrew Python framework sys.executable resolution; not executed on a macOS host).; possible cover: none.

Additional source record at `4816d0041045376198306ad97840522f446c15b8`, `releasing/mac-vm-test/guest_operations.py:638`
(g10_brick_test_mac_uninstall_impl/f3):

`releasing/mac-vm-test/BRICK-TEST.md:287-290` and `releasing/mac-vm-test/BRING-UP.md:65-67` specify that a separate
`remote_mac_base` only needs `/opt/homebrew/bin/python3` for SSH setup and `guest_tester.settings.python` for the paired
fixture check and SSH probes. `Operations.ssh()` (`releasing/mac-vm-test/guest_operations.py:487`, updated in
`4816d004`) follows that contract by executing `shlex.quote(self.task['settings']['python'])` on `remote-mac`. However,
`Operations.perform('fixture_clean')` (`releasing/mac-vm-test/guest_operations.py:638`) spawns `fixture_probe.py` with
`sys.executable`, and `fixture_probe.peer_probe()` (`releasing/mac-vm-test/fixture_probe.py:229`, added in `54635894` /
`g09`) forwards its own `sys.executable` over SSH to the `remote-mac` peer:

```python
# guest_operations.py:638
self.command([sys.executable, str(Path(__file__).with_name('fixture_probe.py')), '--role', 'mac',
    '--ssh-alias', self.context['ssh']['alias'], '--output', str(path)], timeout=180)
# fixture_probe.py:229
interpreter = sys.executable if peer_role == 'remote-mac' else '/usr/bin/python3'
```

On macOS Homebrew Python framework builds, invoking `/opt/homebrew/bin/python3` resolves `sys.executable` to the
versioned Cellar/Framework executable path (e.g.,
`/opt/homebrew/Cellar/python@3.13/3.13.x/Frameworks/Python.framework/...`). Even if `guest_operations.py:638` invoked
`self.task['settings']['python']` directly, CPython on the Mac framework build still resolves `sys.executable` inside
`fixture_probe.py`. When `remote_mac_base` is a distinct image built with a different Homebrew Python minor or patch
version behind the same `/opt/homebrew/bin/python3` (`settings['python']`) symlink, SSH fails to execute `mac_base`'s
Cellar path on `remote-mac`, breaking `fixture-clean` before testing can proceed. (Cross-group boundary with
`g09_brick_test_mac_peer_prep`.)

How to verify: Inspect `guest_operations.py:487,638` alongside `fixture_probe.py:229`: `guest_operations.py:638` passes
neither `self.task['settings']['python']` nor a peer-interpreter flag, and `fixture_probe.py:229` hardcodes
`sys.executable` for `remote-mac`. On two macOS guests where `/opt/homebrew/bin/python3` points to different Cellar
versions, `peer_probe()` builds an SSH command targeting the primary guest's resolved Cellar path.

Fix: Add an explicit `--peer-python` CLI option to `releasing/mac-vm-test/fixture_probe.py` (defaulting to
`sys.executable` for standalone calls) so `peer_probe()` uses that un-resolved configured path for `remote-mac`, and
have `guest_operations.py:638` pass `['--peer-python', self.task['settings']['python']]` (when
`self.task['pass_id'] == REMOTE_MAC_PASS`, or unconditionally for Mac peers).

Reviewer metadata (g10_brick_test_mac_uninstall_impl/f3): suggested bucket: other (harm is confined to maintainer
release-test tooling when using a distinct `remote_mac_base` image with a different Homebrew Python Cellar path).;
confidence: confirmed (code path and documented contract verified directly from source at
4816d0041045376198306ad97840522f446c15b8 and 1b3fe49af86979aebc2719965682a17f8c6366ac; macOS Homebrew framework
`sys.executable` resolution is standard CPython/Homebrew behavior).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g09_brick_test_mac_peer_prep/f1`,
`g10_brick_test_mac_uninstall_impl/f3`.
