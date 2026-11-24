# guest_ui.py still treats p1-remote-mac native-preflight as a bonus pass requiring geometry image parsing

Reviewed commit: 4816d0041045376198306ad97840522f446c15b8

## TLDR

In the release brick-test UI bridge, `p1-remote-mac` is still misclassified as a bonus pass during `native-preflight`
because `guest_ui.py` checks `not pass_id.startswith('p0-')` instead of `is_bonus_pass(pass_id)`. As a result,
`p1-remote-mac` preflight enforces bonus geometry image parsing (`image_metadata` and single-image restriction) on
Finder screenshots even though it never runs window-drag or `finder_frame` checks.

## Details

**other.** Source inspection. Main reference `releasing/mac-vm-test/guest_ui.py:150`.

Commit `4816d0041045376198306ad97840522f446c15b8` introduced `is_bonus_pass(pass_id)` in
`releasing/mac-vm-test/checkpoint_contract.py:24-26` (`not pass_id.startswith('p0-') and pass_id != REMOTE_MAC_PASS`)
and updated `guest_tester.py:371, 604`, `guest_workflow.py:427`, `guest_operations.py:596`, and
`checkpoint_contract.py:176` so that `p1-remote-mac` uses the non-bonus `native-preflight` path. However,
`releasing/mac-vm-test/guest_ui.py:149-150` (`NativeUI.complete`) was not updated and still reads:
`needs_geometry = task['check_id'] == 'window-drag' or (task['check_id'] == 'native-preflight' and not task['pass_id'].startswith('p0-'))`.
Because `'p1-remote-mac'` does not start with `'p0-'`, `needs_geometry` evaluates to `True` during `p1-remote-mac`'s
`native-preflight`. On every Finder `getScreenshot` or `getAXStateAndScreenshot` response, `guest_ui.py:151-160`
enforces `len(images) == 1` and runs `image_metadata()` (which raises `RuntimeError` and closes the action gate via
`guest_operations.py:1136-1139` on multi-image replies or unsupported image headers), storing metadata in
`operations.latest_images` that `p1-remote-mac` never consumes.

How to verify: Construct a `NativeUI` with `task = {'pass_id': 'p1-remote-mac', 'check_id': 'native-preflight'}` and
call `complete({'target': 'finder', 'method': 'getScreenshot', 'args': []}, response)` with two image blocks (or a
non-PNG/JPEG image); it raises `RuntimeError` instead of returning without geometry parsing like `p0-fresh`.

Fix: Import `is_bonus_pass` from `checkpoint_contract` in `releasing/mac-vm-test/guest_ui.py` and replace
`not task['pass_id'].startswith('p0-')` at line 150 with `is_bonus_pass(task['pass_id'])`.

Reviewer metadata (g04b_brick_test_guest_workflow/f3): suggested bucket: other (maintainer brick-test tooling
inconsistency; only triggers if native preflight on `p1-remote-mac` returns a response that fails `image_metadata` or
multi-image checks).; confidence: confirmed (verified directly at `guest_ui.py:149-160`,
`checkpoint_contract.py:21-26, 176`, `guest_tester.py:371, 604`, `guest_workflow.py:427`, and `guest_operations.py:596`
at `1b3fe49af86979aebc2719965682a17f8c6366ac`).; possible cover: none.

Additional source record at `4816d0041045376198306ad97840522f446c15b8`, `releasing/mac-vm-test/guest_ui.py:150`
(g10_brick_test_mac_uninstall_impl/f2):

Commit `4816d0041045376198306ad97840522f446c15b8` introduced `checkpoint_contract.is_bonus_pass(pass_id)`
(`releasing/mac-vm-test/checkpoint_contract.py:24-26`) so `p1-remote-mac` is excluded from bonus `native-preflight`
geometry requirements, and updated `checkpoint_contract.py:176`, `guest_operations.py:596`, `guest_tester.py:371,604`,
and `guest_workflow.py:427` from `not pass_id.startswith('p0-')` to `is_bonus_pass(...)`. It missed
`releasing/mac-vm-test/guest_ui.py:149-150` in `NativeUI.complete()`:

```python
needs_geometry = task['check_id'] == 'window-drag' or (
    task['check_id'] == 'native-preflight' and not task['pass_id'].startswith('p0-'))
```

Because `'p1-remote-mac'` does not start with `'p0-'`, `needs_geometry` evaluates to `True` on `p1-remote-mac` preflight
even though `p1-remote-mac` never runs `finder_frame` or bonus gesture checks. Every `finder` `getScreenshot` or
`getAXStateAndScreenshot` call therefore runs the `len(images) != 1` guard (`guest_ui.py:157`) and
`window_geometry.image_metadata()` (`guest_ui.py:159`, `window_geometry.py:14-44`), raising `RuntimeError` (which closes
the action gate in `guest_operations.py:serve()`) on any multi-image response or image failing `image_metadata`'s
dimension/header parser. (Also noted independently by `g04b_brick_test_guest_workflow`.)

How to verify: Construct a `NativeUI` with `task = {'pass_id': 'p1-remote-mac', 'check_id': 'native-preflight', ...}`
and call `complete({'target': 'finder', 'method': 'getScreenshot', 'args': []}, response)` with two image blocks or an
invalid image payload; `complete()` raises `RuntimeError` instead of ignoring geometry metadata as it does for
`'p0-fresh'`.

Fix: Import `is_bonus_pass` from `checkpoint_contract` in `releasing/mac-vm-test/guest_ui.py` and replace
`not task['pass_id'].startswith('p0-')` with `is_bonus_pass(task['pass_id'])` at line 150.

Reviewer metadata (g10_brick_test_mac_uninstall_impl/f2): suggested bucket: other (harm is confined to maintainer
release-test tooling — spurious preflight failure or unnecessary screenshot decoding on `p1-remote-mac` — with no
security impact or user data loss).; confidence: confirmed (verified directly from source at
4816d0041045376198306ad97840522f446c15b8 and 1b3fe49af86979aebc2719965682a17f8c6366ac).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g04b_brick_test_guest_workflow/f3`,
`g10_brick_test_mac_uninstall_impl/f2`.
