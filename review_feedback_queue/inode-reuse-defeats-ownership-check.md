# Inode reuse defeats the checkout ownership check

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If you remove a Farhelm-created checkout folder and put something else at the same path, deleting the old session can
move your new folder into the archive.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F3 / COR-INODE-REUSE`, tagged **definite**. Anchors and title: `working_copies.rs:320-323`,
`working_copies.rs:1075-1089`, `working_copies.rs:1184-1194`, `working_copies.rs:1217-1228`,
`service/teardown.rs:585-596` — The `(dev, ino)` identity check cannot tell a recreated directory from the original, so
Delete can archive a folder Farhelm does not own

Farhelm's proof that it owns a managed checkout is the `(st_dev, st_ino)` pair it recorded when it created the
directory: the filesystem's device number and the directory's inode number. Every check before a destructive step
compares that pair with a fresh `stat`:

- `verify_identity`, which Delete uses to decide whether the last reference's checkout is still there to archive
- the source check inside `archive_move_with_effects`
- `verified_root` for the checkout root

The module docs promise that a recreated directory "is a different object and is never operated on". SPEC.md says "A
foreign object replacing the recorded path must remain untouched."

Inode numbers are reused, though. On ext4, a directory created right after another one was removed from the same place
often gets the same inode number. Two reviewers observed this, and I reproduced it in a scratch directory on this host's
ext4 filesystem: `mkdir bar-1; rm -rf bar-1; mkdir bar-1` returned the same inode both times.

So a user can remove a Farhelm-created checkout by hand and then clone or create something else at the same path. If the
new directory gets the old inode number, `verify_identity` reports `Matches`, and the next Delete of the old session
moves the user's new folder into the archive. The folder is moved, not deleted, so its contents survive under
`farhelm-archived-working-copies`. Still, it disappears from where the user put it, without consent, which is exactly
the case SPEC.md names. A checkout root that was removed and recreated passes `verified_root` in the same way.

Suggested change: store, next to device and inode, something that changes when an inode number is reused. Options are
the birth time (Linux `statx` with `STATX_BTIME`, macOS `st_birthtime`) or the inode generation number
(`FS_IOC_GETVERSION`). Treat a mismatch, or an unreadable value where one was recorded, as `DifferentObject` in
`verify_identity`, `archive_move_with_effects`, `reconcile_archive_with_effects` and `verified_root`. Rows recorded
before the change will not have the extra value, so the change needs a rule for them.
