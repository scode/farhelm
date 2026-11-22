# Test-worker status cannot see worker-wide bootstrap locks

Reviewed commit: 022d0f010fe5fb0d5acd3e73e02b0569517ec1bc

## TLDR

When a test-worker run times out waiting for a worker bootstrap, the error message tells the user to run `status`, but
`status` never reports an active standalone bootstrap or another checkout's auto-bootstrap.

## Details

**other.** Source inspection. Main reference `scripts/test-worker/remote/status.sh:5-7`.

In `scripts/test-worker/remote/with-lock.sh:75-77`, a run that waits 300s on `$worker/bootstrap.lock` (fd 8) exits 75
with `test-worker: timed out waiting for worker bootstrap; check status or try again`. However,
`scripts/test-worker/remote/status.sh:5-7` only checks `$base/lock` and `$base/lock.holder` for the caller's own
checkout. A standalone `bootstrap` (`with-lock.sh:121-124`) never acquires `$base/lock` or records any holder file, and
an auto-bootstrap triggered by a different checkout only writes that other checkout's `$other_base/lock.holder`.
Consequently, running `test-worker.sh status` from the timed-out checkout (or even from the checkout running a
standalone `bootstrap`) shows no active lock holder. Fix direction: record a worker-level bootstrap holder file (e.g.
`$worker/bootstrap.holder`) under `$worker/bootstrap.lock` while bootstrap runs (cleaned up on exit) and have
`status.sh` display it whenever `! flock -n "$worker/bootstrap.lock" true`.

Reviewer metadata (g02-test-worker-isolation/f5): suggested bucket: other (cannot be in highest: maintainer test-worker
diagnostic mismatch); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g02-test-worker-isolation/f5`.
