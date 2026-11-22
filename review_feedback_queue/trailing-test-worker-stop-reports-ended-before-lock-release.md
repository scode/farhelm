# Test-worker stop.sh reports an invocation ended before with-lock.sh releases the checkout lock

Reviewed commit: 022d0f010fe5fb0d5acd3e73e02b0569517ec1bc

## TLDR

Stopping a test-worker run can report that the run has ended while the remote checkout lock is still held, causing an
immediately following run on that checkout to fail as busy with an unknown holder.

## Details

**other.** Source inspection. Main reference `scripts/test-worker/remote/with-lock.sh:45-49`.

In `scripts/test-worker/remote/with-lock.sh:39-49`, the `finish` EXIT trap removes `$base/lock.holder` and
`$base/lock.id` (line 46) and then runs `rm -rf -- "$stage"` (line 48) without ever unlocking fd 9 (`$base/lock`). Fd 9
is only released by the kernel after `rm -rf -- "$stage"` finishes and the bash process exits (and if the run failed or
was stopped before line 142, `$stage/src` still holds a full copy of the tracked repository). Meanwhile,
`scripts/test-worker/remote/stop.sh:23-28` considers the invocation ended as soon as `cat "$base/lock.id"` no longer
equals `$id`, without verifying that `$base/lock` is unlocked or owned by a different non-empty invocation ID. As a
result, `stop.sh` can print `invocation $id ended` and exit 0 while fd 9 is still held; an immediate follow-up
`test-worker.sh` invocation then hits `flock -n 9` failure at `with-lock.sh:127-129` and exits 75 with
`in use by: unknown`. The same assumption appears in `scripts/test-worker/test-test-worker.py:203` (`finish_owned_jobs`)
and `scripts/test-worker/test-test-worker.py:275-280`, which wait for `lock.id` to disappear and immediately launch a
follow-up run expecting exit 0 (creating a latent flake under load). Fix direction: unlock fd 9 (`flock -u 9`) in
`with-lock.sh:finish` immediately after removing `lock.holder`/`lock.id` and before `rm -rf -- "$stage"` (stage
directories are uniquely named per invocation and remain guarded by fd 7 until exit), and in `stop.sh:24` treat the
invocation as ended only when `flock -n "$base/lock" true` succeeds or `lock.id` holds a different non-empty invocation
ID (and similarly check the lock in `test-test-worker.py`).

Reviewer metadata (g02-test-worker-isolation/f1): suggested bucket: other (cannot be in highest: confined to maintainer
test-worker tooling and causes spurious exit-75 busy refusals rather than security or user-data loss); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g02-test-worker-isolation/f1`.
