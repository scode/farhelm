# Exiting stdio proxy mid-scan can stay in farhelm_pids without proxy classification and falsely fail Remove

Reviewed commit: a1261867e99f97c6fcf4f6abdf6c69342f501d93

## TLDR

During the brick test's Linux remote Remove check, if the closing SSH stdio proxy exits between two `/proc` reads while
taking the post-Remove snapshot, the probe keeps its PID in `farhelm_pids` without recognizing it as a stdio proxy. That
turns an expected connection teardown into a false product failure (`EffectFailure`).

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/guest_operations.py:210-224`.

In `remote_host_snapshot` (`releasing/mac-vm-test/guest_operations.py:210-224` at tip
`1b3fe49af86979aebc2719965682a17f8c6366ac`; lines 206-222 at `a1261867e99f97c6fcf4f6abdf6c69342f501d93`), the `/proc`
scan appends `pid` to `running` (`farhelm_pids`) immediately after `os.readlink(entry / 'exe')` matches `prefixes` (line
215), and only then opens and reads `entry / 'cmdline'` (lines 218-224) to classify `stdio_proxies`:

- If a `farhelm internal stdio` proxy exits between `os.readlink(entry / 'exe')` and `(entry / 'cmdline').open('rb')`,
  `open()` raises `FileNotFoundError`/`ProcessLookupError` (caught by `except OSError: continue`), or if `exit_mm`
  clears the process's `mm` before `read()`, `cmdline` returns `b''`.
- In either case, the exiting proxy remains in `after['farhelm_pids']` while being absent from
  `after['stdio_proxy_pids']`.
- In `checkpoint_contract.py:201-211`, `_resources_preserved_by_remove(before)` removes that proxy PID (because it was
  alive and classified in `before['stdio_proxy_pids']`), while `_resources_preserved_by_remove(after)` retains it in
  `after['farhelm_pids']`. `require_remote_host_effect` then raises
  `EffectFailure('Remove changed remote resources or the running supervisor')`, which `guest_tester.py:308-313` imports
  as a product failure (`status: 'fail'`).

Note on scope/likelihood: The race window between `readlink` and `open('cmdline')` is narrow (microseconds, and
`remote_host_after` runs after the GUI shows removal), so it requires proxy teardown to coincide with the scan. However,
treating `FileNotFoundError`/`ProcessLookupError` on `cmdline` as "still running and non-proxy" contradicts the
`readlink` comment on line 212 (`one that exited mid-scan`), and `test_guest_operations.py:152-159` masks this by
omitting `cmdline` on mock supervisor PID 71.

How to verify: In `test_guest_operations.py`, simulate PID 81 having a matching `exe` link and either a missing
`cmdline` (raising `FileNotFoundError`) or an empty `cmdline` (`b''`), then pass the resulting snapshot as `after` to
`require_remote_host_effect('remote-host-remove', connected, after)`; it raises `EffectFailure`.

Fix: On Linux `/proc`, if opening `cmdline` fails with `FileNotFoundError`/`ProcessLookupError` (or reading returns
`b''`, indicating a kernel thread or exiting/zombie process whose `mm` is gone), skip adding `pid` to `running`
(consistent with `readlink` skipping vanished processes), and update the mock supervisor in `test_guest_operations.py`
(PID 71) to have a realistic `cmdline` (`b'farhelm\0supervisor\0run\0'`). Alternatively/additionally, in
`require_remote_host_effect('remote-host-remove', before, after)`, any PID already proven to be in
`before['stdio_proxy_pids']` can be treated as a known proxy when checking teardown.

Reviewer metadata (g04a_brick_test_guest_ops/f2): suggested bucket: other — narrow race confined to release-testing
maintainer tooling (`review_feedback_queue/AGENTS.md`, "Priority buckets").; confidence: confirmed; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g04a_brick_test_guest_ops/f2`.
