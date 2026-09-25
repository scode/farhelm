# Filesystems without no-replace rename strand the archive step

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With a checkout root on e.g. an NFS home directory, deleting a checkout's last session fails and that session can then
never be deleted or restarted.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F6 / COR-RENAME-UNSUPPORTED`, tagged **possible**. Anchors and title:
`working_copies.rs:1199-1202`, `working_copies.rs:1314-1322`, `working_copies.rs:1374-1379`, `service/core.rs:9801`,
`service/core.rs:7263-7285` — On filesystems without no-replace rename, the last Delete of a checkout leaves it stuck
`archive_pending`

To archive a managed checkout safely, Delete first records the destination and marks the row `archive_pending`. Only
then does it call a rename that refuses to overwrite: `renameat2(RENAME_NOREPLACE)` on Linux,
`renameatx_np(RENAME_EXCL)` on macOS.

Some filesystems do not support that kind of rename, and the kernel rejects the call without moving anything. From
kernel source knowledge (not reproduced here), NFS and CIFS clients on Linux return `EINVAL`, some FUSE mounts behave
the same way, and macOS returns `ENOTSUP`. `rename_noreplace` maps only `EEXIST`/`ENOTEMPTY` to "name taken", and
everything else propagates as an error. The code cannot tell "this can never work here" apart from "the move may have
happened".

The row therefore stays `archive_pending`. Every Delete retry and every startup recovery calls
`reconcile_archive_with_effects`, which retries the same rename and gets the same error. While the row is pending,
`restart_session` (core.rs 9801) refuses to restart the session, and `refuse_pending_archive` (core.rs 7263-7285)
refuses new sessions in that directory. A refusal that changed nothing on disk becomes a permanent stuck state.
`EACCES`/`EPERM` on the archive directory end up in the same state, but fixing the permissions recovers from that.

Open premise: SPEC.md does not say whether checkout roots on such filesystems are supported. The "Healthy local
filesystems" acceptance covers I/O errors and hangs, not operations a filesystem does not support.

Suggested change: handle rename errors that prove the source never moved (`EINVAL`, `ENOSYS`, `ENOTSUP`, `EACCES`,
`EPERM`, `EXDEV`) separately. Re-check the source identity, roll the row back to `allocated` with the destination
cleared, and report a Delete failure that names the filesystem limitation. Alternatively, declare such filesystems
unsupported: probe for no-replace rename support when a root is admitted, and document the limit in SPEC_impl.md.
