# Ordinary brick-test cleanup deletes live guests even when lost P1 retry evidence invalidates the P1 verdict, and the retry-evidence unit test misses this gate

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

During a release brick test, if a retained provider-retry archive for a P1 check (such as old-session survival or
remote-host uninstall) goes missing or is modified on disk, the P1 verdict degrades to could not run, yet ordinary pair
cleanup still deletes the live test VMs instead of keeping them so the operator can recover or inspect the evidence. On
the P1-only remote Mac pair, bypassing the P0 verdict check in cleanup also skips `native_preflight` evidence
verification and `preparation_error` gating.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/brick_test.py:1864`.

- Introduced in 5fcaf735cab90dedf6dc20dd46478f9b0266c3a8 (still present at review tip
  1b3fe49af86979aebc2719965682a17f8c6366ac; Turn 2 re-verification confirmed and expanded the empty-P0-projection impact
  on `p1-remote-mac-uninstall`).
- In `releasing/mac-vm-test/brick_test.py` (`test_owned`, lines 969-993 at 5fcaf735c / 999-1023 at 1b3fe49af),
  pre-action HTTP 429 provider retries are enabled for every non-feedback checkpoint, including P1 checkpoints
  (`old-sessions`, `local-old-session`, `remote-old-session`, `remote-host-remove`, `remote-host-uninstall`, and all six
  checkpoints of `p1-remote-mac-uninstall`), and append records to `entry['provider_retries']` with `check_id` and
  `sha256` digests.
- In `Run.verdict()` (`releasing/mac-vm-test/brick_test.py:1658-1667` at 1b3fe49af; `1615-1624` at 5fcaf735c),
  `judged = set(entry['required']) | {'native-preflight'}`, so a missing or corrupted `provider_retries` archive for a
  P1 checkpoint invalidates `verdict(priority_entry(entry, 'P1'))` (and the corresponding named check in
  `report()['p1']['checks']`) to `'could not run'`.
- However, `Run.cleanup()` (`releasing/mac-vm-test/brick_test.py:1864-1873` at 1b3fe49af; `1815-1823` at 5fcaf735c)
  checks only `(not p0['required'] or self.verdict(p0) == 'pass')` (which inspects `provider_retries` only for P0
  checkpoints and `native-preflight`) and `all(self.result_evidence_retained(result) for result in results)` (which
  inspects only the final `entry['results'][key]` entries for P1/P2). It never rechecks `provider_retries` (nor
  `readiness_retries` / `checkpoint_retries`) belonging to P1/P2 checkpoints. Additionally, on `p1-remote-mac-uninstall`
  at 1b3fe49af (`p0['required']` is empty), `(not p0['required'] or self.verdict(p0) == 'pass')` short-circuits without
  checking `native_preflight`, `native-preflight` retry history, or `entry.get('preparation_error')` (if
  `p1-remote-mac-uninstall` fails preparation and scope is then narrowed to `p0-only`, ordinary `cleanup()` deletes the
  failed-preparation pair without `--discard-failed`).
- In `releasing/mac-vm-test/test_brick_test.py:1615-1632` at 1b3fe49af (`1551-1567` at 5fcaf735c),
  `test_lost_retry_evidence_invalidates_only_the_checks_it_belongs_to` populates `entry['provider_retries']` with a
  missing archive for `'old-sessions'`, confirms `report['p1']['verdict'] == 'could not run'`, and immediately mutates
  `check_id` to `'native-preflight'` and calls `cleanup('p0-installer-upgrade', discard_failed=True)` without checking
  that ordinary `cleanup('p0-installer-upgrade')` refuses while P1 retry evidence is missing.
- How to verify: in `test_lost_retry_evidence_invalidates_only_the_checks_it_belongs_to`, after recording
  `'p2-feedback-send'` as `'pass'` with `entry['provider_retries']` pointing to a missing archive for `'old-sessions'`,
  calling `self.run.cleanup('p0-installer-upgrade')` (without `discard_failed=True`) currently deletes the guests
  instead of raising `Refused('pass is incomplete or failed...')`.
- Fix: in `Run.cleanup()` (`releasing/mac-vm-test/brick_test.py:1864-1873`), require
  `not entry.get('preparation_error')`, require all retry-history entries (`provider_retries`, `readiness_retries`,
  `checkpoint_retries`) on the pass to satisfy `result_evidence_retained()`, and require intact `native_preflight`
  evidence when `gui_backend == 'guest-native'` even if `p0['required']` is empty, before allowing ordinary cleanup
  without `discard_failed=True`. Update `test_lost_retry_evidence_invalidates_only_the_checks_it_belongs_to` in
  `releasing/mac-vm-test/test_brick_test.py` to assert that ordinary `cleanup('p0-installer-upgrade')` refuses until the
  P1 retry archive is restored (or `--discard-failed` is passed).

Reviewer metadata (g06_brick_test_tests_b/l1): suggested bucket: other (harm is confined to maintainer release-test
tooling in `releasing/mac-vm-test/`; premature VM cleanup on corrupted retry evidence or failed P1 preparation does not
affect shipped product security or user data).; confidence: confirmed (established directly from source analysis at both
5fcaf735cab90dedf6dc20dd46478f9b0266c3a8 and 1b3fe49af86979aebc2719965682a17f8c6366ac; not executed).; possible cover:
none.

Additional source record at `5fcaf735cab90dedf6dc20dd46478f9b0266c3a8`, `releasing/mac-vm-test/brick_test.py:1817-1826`
(g03_brick_test_coord_p2/f1):

In `releasing/mac-vm-test/brick_test.py:1817-1826` (`Run.cleanup`), P0 pairs with P1/P2 checks compute `complete` by
checking `self.verdict(priority_entry(entry, 'P0')) == 'pass'` and `self.result_evidence_retained(result)` for each
recorded P1/P2 result in `entry['results']`. However, `Run.verdict` (`releasing/mac-vm-test/brick_test.py:1615-1618`)
filters `entry['provider_retries']` to `judged = set(entry['required']) | {'native-preflight'}`, so
`self.verdict(priority_entry(entry, 'P0'))` only checks `provider_retries` belonging to P0 checks and
`native-preflight`. Because `Run.test_owned` (`releasing/mac-vm-test/brick_test.py:967-990`) records `provider_retries`
for all non-P2 checks—including P1 checks such as `old-sessions`, `local-old-session`, `remote-old-session`,
`remote-host-remove`, and `remote-host-uninstall`—any missing or altered archive referenced by a P1 `provider_retries`
entry invalidates `self.verdict(priority_entry(entry, 'P1'))` to `'could not run'`, yet `cleanup()` still evaluates
`complete = True` and deletes the VMs without `--discard-failed`. (Note also cross-group interaction with commit
`4816d004` / `g10_brick_test_mac_uninstall_impl`: once `p1-remote-mac` was added with `priority == 'P1'` and an empty P0
projection, `(not p0['required'] or self.verdict(p0) == 'pass')` also skips checking `native_preflight` and
`provider_retries` evidence retention for that pass.) Verify by recording a passing P1 check with a `provider_retries`
entry whose tar archive is removed or mutated, then calling `cleanup('p0-installer-upgrade')` without
`discard_failed=True`: `report()['p1']['verdict']` is `'could not run'`, while `cleanup()` succeeds instead of raising
`Refused`. Fix by checking `self.result_evidence_retained(row)` for every `provider_retries` (and any
`readiness_retries`/`checkpoint_retries`) row belonging to `later` checkpoints in `cleanup()` (and, for P1-only passes
at tip, checking `native_preflight` retention too).

Reviewer metadata (g03_brick_test_coord_p2/f1): suggested bucket: other (harm is confined to maintainer release-testing
tooling; it does not affect shipped user software, user data, or release promotion, because `report()` still invalidates
the P1 verdict to 'could not run').; confidence: confirmed; possible cover: none.

Additional source record at `4816d0041045376198306ad97840522f446c15b8`, `releasing/mac-vm-test/brick_test.py:1864-1878`
(g03_brick_test_coord_p2/f5):

Building on `5fcaf735`'s `Run.cleanup()` structure, commit `4816d004` (`releasing/mac-vm-test/brick_test.py:1864-1874`
at tip `1b3fe49af86979aebc2719965682a17f8c6366ac`) extended the tiered cleanup branch to
`entry.get('priority') in ('P0', 'P1')` and changed the P0 gate to
`(not p0['required'] or self.verdict(p0) == 'pass') and not entry.get('automation_failure')`. For `p1-remote-mac`
(`priority == 'P1'`), `p0['required']` is empty, so `not p0['required']` short-circuits to `True` without ever calling
`self.verdict()`—bypassing `verdict()`'s check of `entry.get('preparation_error')` (line 1672 at tip). When
`prepare('p1-remote-mac')` fails (`brick_test.py:687-691`), it sets `entry['phase'] = 'failed'` and
`entry['preparation_error'] = type(error).__name__` with empty `results` and no `automation_failure`. If scope is then
narrowed to `p0-only` via `limit_scope('p0-only')` (`brick_test.py:1388-1406`, which allows scope changes regardless of
pass failure state), `self.scope_tiers()` becomes `('P0',)`. Calling `cleanup('p1-remote-mac')` without
`discard_failed=True` then evaluates: `(not p0['required'] or ...)` -> `True`; `not entry.get('automation_failure')` ->
`True`; `all(key in entry['results'] for key, priority in later if priority in ('P0',))` -> vacuously `True`; and both
`results` checks -> vacuously `True`. Because `self.pair('p1-remote-mac')` (`brick_test.py:602`) permits
`phase == 'failed'`, `cleanup()` shuts down and deletes the failed preparation's clones without `--discard-failed`, sets
`entry['phase'] = 'deleted'` (releasing the `require_no_failure()` fence at line 1435), and records a misleading
`working clones deleted after checked evidence` journal note. (At `5fcaf735` itself, every pass entering line 1817 had
`priority == 'P0'` and non-empty `p0['required']`, so `verdict(p0)` still caught `preparation_error`; the bug is
introduced by `4816d004`'s empty-`p0` short-circuit.) Verify by failing `prepare('p1-remote-mac')`, running
`limit_scope('p0-only')`, and calling `cleanup('p1-remote-mac')` without `discard_failed=True`: it deletes the clones
instead of raising `Refused`. Fix by checking `not entry.get('preparation_error')` and `entry['phase'] == 'ready'`
(along with `native_preflight` and retry-history retention per `f1`) in `cleanup()`'s tiered `complete` expression.

Reviewer metadata (g03_brick_test_coord_p2/f5): suggested bucket: other (harm is confined to maintainer release-testing
tooling; it bypasses the `--discard-failed` retention gate for failed-preparation disposable VMs under a mid-run scope
narrowing to `p0-only`, without creating any false pass or affecting shipped user software).; confidence: confirmed;
possible cover: none.

Additional source record at `4816d0041045376198306ad97840522f446c15b8`, `releasing/mac-vm-test/brick_test.py:1864`
(g10_brick_test_mac_uninstall_impl/f1):

In `releasing/mac-vm-test/brick_test.py:1864-1875`, `Run.cleanup()` was extended from `entry.get('priority') == 'P0'` to
`entry.get('priority') in ('P0', 'P1')` and guards the P0 verdict check with
`(not p0['required'] or self.verdict(p0) == 'pass')`. On `p1-remote-mac`, every checkpoint has priority `P1`, so
`p0['required']` is empty and `self.verdict(...)` is never called during cleanup. As a result, three pass-level checks
that `Run.verdict()` enforces (`releasing/mac-vm-test/brick_test.py:1658-1686`) are skipped on `p1-remote-mac`:

1. `native_preflight` presence and SHA-256 verification (lines 1672-1683): if a `guest-native` `p1-remote-mac` pass has
   missing or corrupted preflight evidence, `cleanup()` still treats `complete` as `True` and deletes both Mac clones
   without `--discard-failed`, while `report()` downgrades `p1.checks['remote-mac-uninstall']` to `could not run`.
2. `provider_retries` evidence retention (lines 1658-1661): if a `p1-remote-mac` checkpoint recovered from a provider
   429 and its retained retry archive is missing or corrupted, `cleanup()` does not recheck it before deleting the
   guests, whereas `verdict()` invalidates the pass to `could not run`.
3. `preparation_error` (line 1670): if `prepare('p1-remote-mac')` failed with `preparation_error` and the run scope was
   subsequently narrowed to `p0-only`,
   `all(key in entry['results'] for key, priority in later if priority in self.scope_tiers())` is vacuously `True`,
   `not entry.get('automation_failure')` is `True`, and `cleanup('p1-remote-mac')` deletes the failed pair without
   `--discard-failed`.

How to verify: Construct a `guest-native` run with `p1-remote-mac` having all six checkpoints passing, corrupt or remove
the `native_preflight` artifact or a `provider_retries` artifact, and call `run.cleanup('p1-remote-mac')`; it succeeds
without `discard_failed=True` even though `run.report()['p1']['checks']['remote-mac-uninstall'] == 'could not run'`.
Similarly, set `preparation_error` on `p1-remote-mac`, call `run.limit_scope('p0-only')`, and call
`run.cleanup('p1-remote-mac')` without `discard_failed=True`.

Fix: In `Run.cleanup()`, explicitly require `not entry.get('preparation_error')`, recheck `provider_retries` (and
`readiness_retries`/`checkpoint_retries`) evidence retention for the pass's in-scope results, and for `guest-native`
runs with any passing `P1` check on a P1-only pair (or whenever `native_preflight` is recorded or all checks passed)
recheck `native_preflight` evidence hashes before allowing non-discard cleanup.

Reviewer metadata (g10_brick_test_mac_uninstall_impl/f1): suggested bucket: other (harm is confined to maintainer
release-test tooling — premature cleanup of disposable test VMs when evidence is damaged or after a narrowed scope —
with no security impact or loss of user-owned work).; confidence: confirmed (verified directly from source at
4816d0041045376198306ad97840522f446c15b8 and 1b3fe49af86979aebc2719965682a17f8c6366ac).; possible cover: none.

Additional source record at `4816d0041045376198306ad97840522f446c15b8`,
`releasing/mac-vm-test/test_native_coordinator.py:449` (g11_brick_test_mac_uninstall_tests/f1):

In releasing/mac-vm-test/brick_test.py:1864-1874 (`Run.cleanup`), ordinary-cleanup completeness for tiered passes
(`priority in ('P0', 'P1')`) computes `p0 = priority_entry(entry, 'P0')` and
`(not p0['required'] or self.verdict(p0) == 'pass')`, then checks only `not entry.get('automation_failure')` and
evidence retention across `entry['results']` for checkpoints whose tier is in `self.scope_tiers()`. On P0 pairs
(`p0-fresh`, `p0-updater-upgrade`, `p0-installer-upgrade`), `p0['required']` is non-empty, so
`self.verdict(p0) == 'pass'` runs and enforces three pass-level gates (lines 1659-1683): (1)
`not entry.get('preparation_error')` (and non-empty completed results), (2) intact `entry['native_preflight']['sha256']`
when `gui_backend == 'guest-native'`, and (3) intact `provider_retries` / `readiness_retries` / `checkpoint_retries`
archives. On `p1-remote-mac` (`REMOTE_MAC_PASS`), every checkpoint has `priority == 'P1'`, so `p0['required']` is empty
and `not p0['required']` short-circuits past `self.verdict(p0)`. This creates two concrete bypasses:

1. Missing or corrupted `native_preflight` or retry archives on `p1-remote-mac`: `Run.verdict(entry)` (and
   `report()['p1']['checks']['remote-mac-uninstall']`) becomes `'could not run'`, yet ordinary
   `Run.cleanup('p1-remote-mac')` still succeeds without `--discard-failed` and deletes the VMs.
2. Failed or interrupted preparation (`entry['phase'] == 'failed'` with `entry['preparation_error']`, or `'preparing'`
   after both clones exist) followed by `Run.limit_scope('p0-only')`: because `'P1' not in self.scope_tiers()`, the
   `all(key in entry['results'] ... if priority in self.scope_tiers())` and `results` checks are vacuously `True`,
   `not entry.get('automation_failure')` is `True`, and `entry.get('preparation_error')` / `entry['phase'] != 'ready'`
   are never checked. Ordinary `Run.cleanup('p1-remote-mac')` therefore deletes failed-preparation clones without
   `--discard-failed` and sets `entry['phase'] = 'deleted'`, which also clears `preparation_error` from
   `Run.require_no_failure()` (`brick_test.py:1435`). In commit `4816d004`, neither
   `releasing/mac-vm-test/test_brick_test.py:854-920`
   (`test_remote_mac_pair_follows_installer_cleanup_and_counts_as_p1`, `test_remote_mac_pair_is_outside_p0_only_scope`)
   nor `releasing/mac-vm-test/test_native_coordinator.py:449-478`
   (`test_remote_mac_pass_imports_native_evidence_and_only_authenticates_the_gui_mac`) tests `cleanup(REMOTE_MAC_PASS)`
   with corrupted `native_preflight`/retry evidence or with `preparation_error` + `limit_scope('p0-only')`. (Cross-group
   note: root defect is in `brick_test.py:1864-1874` under `g10_brick_test_mac_uninstall_impl`, corroborated by
   `g06_brick_test_tests_b` and `g02_brick_test_coord_p1`; missing regression tests belong to
   `g11_brick_test_mac_uninstall_tests`.) How to verify: (a) In `test_native_coordinator.py`, after
   `self.drive_pair(REMOTE_MAC_PASS)`, corrupt the `native_preflight` tar (as lines 202-205 do for `p0-fresh`) and call
   `self.run.cleanup(REMOTE_MAC_PASS)`; currently cleanup succeeds while
   `report()['p1']['checks']['remote-mac-uninstall'] == 'could not run'`. (b) In `test_brick_test.py`, with
   `enable_p0(remote_mac=True)` and P0 pairs cleaned, trigger a `preparation_error` on `REMOTE_MAC_PASS`, call
   `self.run.limit_scope('p0-only')`, and call `self.run.cleanup(REMOTE_MAC_PASS)`; currently cleanup succeeds without
   `discard_failed=True`. Fix: In `Run.cleanup`, require `entry['phase'] == 'ready'`,
   `not entry.get('preparation_error')`, intact `native_preflight` (when `gui_backend == 'guest-native'` and any
   checkpoint ran or was required in scope), and intact retry archives for all tiered passes even when `p0['required']`
   is empty; add regression tests for both cases in `test_native_coordinator.py` and `test_brick_test.py`.

Reviewer metadata (g11_brick_test_mac_uninstall_tests/f1): suggested bucket: other (confined to maintainer
release-testing coordinator and its unit tests; no user security, data-loss, or shipped product impact, so not highest
or high).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g03_brick_test_coord_p2/f1`,
`g03_brick_test_coord_p2/f5`, `g06_brick_test_tests_b/l1`, `g10_brick_test_mac_uninstall_impl/f1`,
`g11_brick_test_mac_uninstall_tests/f1`.
