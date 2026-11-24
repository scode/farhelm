# Linux remote-host-remove/uninstall start premise and test accept empty session_ids=[]

Reviewed commit: 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8

## TLDR

The release brick test's Linux remote host Remove and Uninstall checks accept a starting remote database that contains
zero sessions, so an empty or freshly recreated sessions table after Uninstall can pass the retained-data check instead
of failing the starting premise.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/test_guest_operations.py:87-96`.

In commit 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8, `require_remote_host_effect` in
`releasing/mac-vm-test/checkpoint_contract.py:210` (and `require_remote_host_start` at `checkpoint_contract.py:229` at
tip 1b3fe49af86979aebc2719965682a17f8c6366ac) checks `before['session_ids'] is not None` for Linux rather than requiring
a non-empty list (`bool(before['session_ids'])`), even though on `p0-installer-upgrade` both `remote-host-remove` and
`remote-host-uninstall` run after P0 `remote-supervisor-agent` (which leaves a genuine remote Codex session running in
`supervisor.db`). Correspondingly, `test_remote_menu_effects_distinguish_remove_from_uninstall` in
`releasing/mac-vm-test/test_guest_operations.py:87-96` at 5fcaf735 (`test_guest_operations.py:89-98` at tip 1b3fe49af)
only tests `session_ids=None` as an invalid starting premise and does not assert rejection of `session_ids=[]`. By
contrast, the later remote Mac uninstall commit 4816d0041 tightened `require_remote_host_start` to
`bool(before['session_ids'])` for `REMOTE_MAC_PASS`, enforced it eagerly at `remote_host_before`
(`guest_operations.py:739-742`), and tested `session_ids=[]` (`test_guest_operations.py:253-255,274-286` at tip), while
leaving the Linux branch accepting `[]`. As a result,
`require_remote_host_effect('remote-host-uninstall', dict(before, session_ids=[]), dict(uninstalled, session_ids=[]))`
passes without raising, weakening the retained-data invariant. Verify by inspecting `require_remote_host_start`
(`checkpoint_contract.py:229` at tip) and calling `require_remote_host_effect` with `session_ids=[]` on both `before`
and `after`. Fix by requiring `bool(before['session_ids'])` for Linux in `require_remote_host_start` (matching
`REMOTE_MAC_PASS`) and adding `('empty start sessions', dict(before, session_ids=[]))` to the premise-failure subtests
in `test_remote_menu_effects_distinguish_remove_from_uninstall`. (Cross-group note: production code lives in
`checkpoint_contract.py`, owned by `g04a_brick_test_guest_ops`; `REMOTE_MAC_PASS` alignment touches
`g11_brick_test_mac_uninstall_tests`.)

Reviewer metadata (g07_brick_test_tests_c/l1): suggested bucket: other (harm is confined to maintainer
release-verification tooling and is not catastrophic).; confidence: confirmed (established directly from source in
`checkpoint_contract.py`, `guest_operations.py`, `test_guest_operations.py`, `brick_test.py`, and `BRICK-TEST.md`).;
possible cover: none.

Additional source record at `5fcaf735cab90dedf6dc20dd46478f9b0266c3a8`,
`releasing/mac-vm-test/checkpoint_contract.py:211` (g04a_brick_test_guest_ops/f1):

In commit 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8, `require_remote_host_effect` in
`releasing/mac-vm-test/checkpoint_contract.py:210` checks `before['session_ids'] is None` (rather than requiring a
non-empty list) before comparing `after['session_ids'] != before['session_ids']` at line 216. At the review tip
`1b3fe49af86979aebc2719965682a17f8c6366ac`, this moved into `require_remote_host_start`
(`releasing/mac-vm-test/checkpoint_contract.py:227-231`): the remote Mac branch (`REMOTE_MAC_PASS`) explicitly requires
`retained_sample = bool(before['session_ids'])` and gates `remote_host_before` in
`releasing/mac-vm-test/guest_operations.py:739-742`, whereas the Linux branch still uses
`retained_sample = before['session_ids'] is not None` and does not gate `remote_host_before`.

On Linux (`p0-installer-upgrade`), both `remote-host-remove` and `remote-host-uninstall` run after `old-sessions`,
`remote-supervisor-agent`, and `remote-old-session` and are contracted to have two existing remote sessions whose data
is retained (`releasing/mac-vm-test/checkpoint_contract.py:385-388, 421-427`). If the UI model mistakenly clicks Delete
instead of Stop during `remote-host-uninstall` before calling `remote_host_before` (or if sessions are missing before
`remote-host-remove`), `remote_host_snapshot` returns `session_ids: []`, which satisfies `is not None`. The subsequent
check `after['session_ids'] != before['session_ids']` (`checkpoint_contract.py:251`) then passes vacuously even if
Uninstall emptied or replaced `supervisor.db` with an empty schema, defeating the stated purpose of `session_ids` in
`guest_operations.py:178-180`. In addition, because Linux does not call `require_remote_host_start` inside
`remote_host_before`, an invalid starting baseline does not close the action gate before the destructive Uninstall
click.

How to verify: Call
`require_remote_host_effect('remote-host-uninstall', dict(before, session_ids=[]), dict(uninstalled, session_ids=[]))`
in `test_guest_operations.py`; it currently returns without raising `RuntimeError`, whereas the equivalent
`REMOTE_MAC_PASS` call at `releasing/mac-vm-test/test_guest_operations.py:254` raises
`RuntimeError('remote menu check did not start from a provisioned running host with readable data')`.

Fix: Require `bool(before['session_ids'])` (or at least the expected non-empty sessions) for Linux in
`require_remote_host_start` (`checkpoint_contract.py:231`), run `require_remote_host_start` on `remote_host_before` for
Linux as well in `guest_operations.py:739-742` so the action gate closes before Remove/Uninstall on a broken fixture,
and add a Linux unit test covering `session_ids=[]` alongside `session_ids=None`.

Reviewer metadata (g04a_brick_test_guest_ops/f1): suggested bucket: other — harm is confined to release-testing
maintainer tooling (`review_feedback_queue/AGENTS.md`, "Priority buckets"); it can weaken a release-gate verification
check without directly harming end users or causing catastrophic maintainer-tooling damage.; confidence: confirmed;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g04a_brick_test_guest_ops/f1`,
`g07_brick_test_tests_c/l1`.
