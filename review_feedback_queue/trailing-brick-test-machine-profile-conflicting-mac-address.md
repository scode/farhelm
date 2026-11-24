# machine_profile.py accepts a remote_mac_base sharing mac_base's VM name with a conflicting MAC address

Reviewed commit: 54635894d9c4cab31de631edf8fab2a6479c8a88

## TLDR

A brick-test machine profile can name the same macOS base VM for both the GUI Mac and the headless remote Mac while
giving two conflicting hardware MAC addresses; profile loading accepts this contradiction, so the run only fails hours
later after all P0 pairs finish and the remote Mac pass starts.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/machine_profile.py:156-159`.

In releasing/mac-vm-test/machine_profile.py:150-159, `Profile.load()` explicitly permits `remote_mac_base` to reference
the same VM as `mac_base` (tested in test_machine_profile.py:91-93), while checking only
`values['remote_mac_base']['name'] == values['linux_base']['name']`. It does not check that when
`values['remote_mac_base']['name'] == values['mac_base']['name']`,
`values['remote_mac_base']['mac_address'] == values['mac_base']['mac_address']`. A single Tart VM directory has only one
`config.json` `macAddress`, so any profile with identical `name` and differing `mac_address` between `mac_base` and
`remote_mac_base` is statically impossible. Because `brick_test.py:648-651` checks `remote_mac_base`'s live MAC only
when preparing `p1-remote-mac` (which runs after all P0 pairs are cleaned up), a typo in `remote_mac_base.mac_address`
when duplicating `mac_base` slips past `Profile.load()` and fails late. How to verify: pass a profile with
`mac_base = {'name': 'mac-base', 'mac_address': '02:00:00:00:00:01'}` and
`remote_mac_base = {'name': 'mac-base', 'mac_address': '02:00:00:00:00:02'}` to `Profile.load()`; it succeeds instead of
raising `Refused`. Fix: in `machine_profile.py:156-159`, reject
`values['remote_mac_base']['name'] == values['mac_base']['name'] and values['remote_mac_base']['mac_address'] != values['mac_base']['mac_address']`
with `Refused`, and cover it in `test_machine_profile.py`.

Reviewer metadata (g09_brick_test_mac_peer_prep/f3): suggested bucket: other (harm is confined to maintainer
release-testing tooling; delayed config validation error with no security or user data-loss impact).; confidence:
confirmed (verified directly from source at releasing/mac-vm-test/machine_profile.py:150-159, brick_test.py:643-651, and
test_machine_profile.py:84-101).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g09_brick_test_mac_peer_prep/f3`.
