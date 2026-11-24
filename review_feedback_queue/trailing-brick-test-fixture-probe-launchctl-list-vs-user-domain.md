# fixture_probe.py uses legacy launchctl list instead of launchctl print user/<uid> for remote-mac (unverified premise)

Reviewed commit: 54635894d9c4cab31de631edf8fab2a6479c8a88

## TLDR

The release brick test's clean-fixture probe checks macOS launchd services using the legacy `launchctl list` command
rather than explicitly inspecting the `user/<uid>` launchd domain where Farhelm installs a remote Mac supervisor, so it
may miss a leftover loaded supervisor job (when its plist is absent) or fail to confirm `user/<uid>` domain
availability.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`releasing/mac-vm-test/fixture_probe.py:120-124`.

In releasing/mac-vm-test/fixture_probe.py:120-124 (and receipt revalidation at lines 176-178 and 194-196), both `mac`
and `remote-mac` roles check launchd exclusively by running `['/bin/launchctl', 'list']` and grepping stdout for
`'farhelm'`. By contrast, SPEC_impl.md:8084 and crates/farhelm-helm/src/provisioning/macos.rs:95,266-300 install and
inspect the remote Mac supervisor job (`io.farhelm.supervisor`, with `LimitLoadToSessionType=Background`) strictly in
the explicit `user/<uid>` domain via `launchctl print user/<uid>` and
`launchctl print user/<uid>/io.farhelm.supervisor`. Every other macOS launchd check added for this feature —
control.py:230, network.py:192, and guest_operations.py:258-274 — likewise queries `launchctl print user/<uid>` rather
than legacy `launchctl list`. Unlike Linux in fixture_probe.py:125-134 (which explicitly probes both `--user` and
`--system` managers), the macOS path neither proves `user/<uid>` domain availability nor targets `user/<uid>`
explicitly. Because `mac_base` defaults to `remote_mac_base`, a leftover loaded job in `user/<uid>` without its plist
(which SPEC.md:268 notes still counts as startable) could be missed if `launchctl list` in a GUI login (`gui/<uid>`) or
headless SSH session does not list `user/<uid>` Background jobs. How to verify: on a macOS 26 host or Tart VM, bootstrap
a `LimitLoadToSessionType=Background` plist into `user/$(id -u)` and compare `/bin/launchctl list` against
`/bin/launchctl print user/$(id -u)` from both a GUI terminal session and a headless SSH session. Fix: have
fixture_probe.py explicitly query `/bin/launchctl print user/<uid>` (or check `user/<uid>/io.farhelm.supervisor` in
addition to `launchctl list`) for macOS roles and enforce that observation in `require_clean_pair`.

Reviewer metadata (g09_brick_test_mac_peer_prep/f2): suggested bucket: other (harm is confined to maintainer
release-testing tooling; at worst a dirty base with an orphaned loaded `user/<uid>` job and no plist could pass
`fixture-clean`).; confidence: unverified after investigation; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g09_brick_test_mac_peer_prep/f2`.
