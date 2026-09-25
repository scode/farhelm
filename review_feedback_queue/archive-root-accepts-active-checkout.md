# The archive move accepts an active checkout as its archive directory

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Same as F1.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F2 / COR-ARCHIVE-ROOT-IS-CHECKOUT`, tagged **definite**. Anchors and title:
`working_copies.rs:1231-1253`, `working_copies.rs:1162-1205`, `working_copies.rs:564`, `working_copies.rs:1407-1443`,
`working_copies.rs:1374-1379` — The archive move accepts an active managed checkout (or the source itself) as its
archive directory

This is the archive-side half of F1. When the last session of a managed checkout (a fresh clone Farhelm created and
tracks in its registry) is deleted, the checkout moves into `<root>/farhelm-archived-working-copies`. The helper that
prepares that directory, `ensure_archive_root`, accepts whatever is at that path as long as it is a real directory (not
a symlink) on the same filesystem. It never checks whether the directory is itself an active managed checkout, or even
the very directory being moved.

The guard for inconsistent registry evidence, `refuse_overlapping_archive`, compares only the source being moved with
the paths of other active rows. It never looks at the destination. The rename helper `rename_noreplace` maps only
`EEXIST`/`ENOTEMPTY` to a name collision. So when the source is the archive directory, the kernel's `EINVAL` (a
directory cannot be moved into its own subdirectory) comes back as a generic error.

By then the row has already been marked `archive_pending`, the journal state that means "a move may be in progress".
`archive_move_with_effects` writes that state after `ensure_archive_root` and immediately before the rename. From then
on, every Delete retry and every startup runs `reconcile_archive_with_effects`, which attempts the same rename and fails
the same way.

Fixing F1 only stops new colliding checkouts from being created. A database that already holds one needs this check to
stop Delete from moving other checkouts into it, and to give the colliding session a clear error instead of a permanent
`EINVAL`. It is also defense in depth against any future path that lets an active checkout land on the archive
directory.

Suggested change: after `ensure_archive_root`, refuse the move when the archive directory is equal to, contains, or lies
inside the path of any non-retired row, including the row being moved. In `archive_move_with_effects`, do this before
the journal is written, so the row stays `allocated` and nothing is marked pending. Apply the same check in
`reconcile_archive_with_effects`, and give the `source == archive_root` case its own clear error message.
