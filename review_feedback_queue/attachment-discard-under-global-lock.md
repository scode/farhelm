# Delete removes attachments under the global attachments lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Deleting a session with large attachments briefly freezes terminal attach, resize and output flow control for every
other session on the host.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F13 / COR-DISCARD-UNDER-LOCK`, tagged **possible**. Anchors and title:
`service/teardown.rs:305-316`, `service/teardown.rs:692-694`, `service/teardown.rs:718`, `attachments.rs:365-373` —
Delete removes the session's attachments recursively while holding the supervisor-wide attachments lock

`attachments` is the supervisor's single mutex over the map of live terminal attachments, meaning which viewer is
attached to which session's terminal. Its doc lists the holders: attach and takeover, input, resize, output pause, stall
handling, connection teardown and delete.

Delete takes this lock once the process sweep is finished (teardown.rs 316) and holds it until line 718. Here,
"attachments" also means the files users uploaded to the session; those are parked in a quarantine directory before the
row is removed. After the final database transaction commits, Delete runs `discard_quarantined` (attachments.rs
365-373). That is a `remove_dir_all` over every file the session received, and it runs while the lock is still held.
Preparation cleanup and the hook-log unlink also run under the lock. SPEC_impl sets no cap on attachment size or count,
so the removal is unbounded. The teardown code's own comment says only fast steps belong under this lock.

While a large removal runs, attach, resize, input, output pause/resume and takeover for every other session on the host
wait. The lock is not needed for the removal. The parked directory has a unique name that nothing else touches, and
startup reconciliation already cleans up anything a failed removal leaves behind.

Open premise: this needs attachments large or numerous enough for the removal to take noticeable time.

Suggested change: send the detach notices while the guard is still held (that ordering is load-bearing and must stay),
then drop the guard. Run `discard_quarantined`, `cleanup_retired_preparation` and the hook-log unlink afterwards, or as
a detached best-effort task.
