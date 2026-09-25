# An unstable device number makes checkout sessions undeletable

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a reboot on some filesystems, sessions created as fresh GitHub checkouts may refuse Delete and restart for good.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F5 / COR-DEV-UNSTABLE`, tagged **possible**. Anchors and title: `working_copies.rs:1222`,
`working_copies.rs:1075`, `working_copies.rs:1192`, `service/teardown.rs:624` — The ownership check treats the raw
device number as stable across reboots and remounts

The ownership proof described in F3 assumes that the device number (`st_dev`) of an unchanged directory never changes.
On some Linux setups it can. On btrfs subvolumes (Fedora's default layout puts `/home` on one), NFS, overlayfs, and some
device-mapper arrangements, the device number is assigned when the filesystem is mounted, and it can differ after a
reboot or remount.

When that happens, the checkout and its root still exist and are unchanged, but every identity comparison fails. Delete,
archive recovery and restart of any session with a managed checkout are all refused. The errors say the checkout "is no
longer the object its registry row captured", or report a root identity mismatch. Nothing ever re-records the identity,
so those sessions become permanently undeletable and unrestartable. F4 describes the root side of the same effect.

This was not reproduced. The open premise is how often device numbers actually change on users' machines.

Suggested change: stop treating `st_dev` as stable across boots. For example, store the boot id with the identity, and
after a boot or mount change compare the inode plus a filesystem id that survives a remount, such as `statfs`'s `f_fsid`
or the mount's UUID. At minimum, when only the device number differs, give a distinct diagnostic and a way to re-verify,
instead of a permanent refusal.
