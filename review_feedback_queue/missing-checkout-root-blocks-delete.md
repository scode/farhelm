# A removed or recreated checkout root makes sessions undeletable

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After deleting or moving the folder configured as the checkout root, sessions that owned checkouts there can never be
deleted.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F4 / COR-ROOT-GONE-UNDELETABLE`, tagged **possible**. Anchors and title:
`service/teardown.rs:583-584`, `working_copies.rs:1217-1228`, `working_copies.rs:612-623`, `working_copies.rs:1442` — A
removed, recreated, moved-behind-a-symlink or unmounted checkout root makes its sessions permanently undeletable

Each managed checkout's registry row also records the `(dev, ino)` identity of the checkout root it lives under. Before
Delete settles the last reference to a checkout, `verified_root` requires that root to still exist at its recorded path
with that same identity. It requires this even when the checkout directory is already gone and there is nothing to move.
The same check runs in several places:

- teardown.rs 583-584, before the checkout itself is examined
- `retire_missing` in the final delete transaction
- recovery of `archive_pending` rows
- restart, through `recover_checkout_destination`

Any of these makes the check fail on every attempt:

- the user deleted the root (`ENOENT`)
- the root was recreated, so it has a new inode
- the root was replaced with a symlink to a new location
- the root was restored onto another disk
- the volume holding it is not mounted

The Delete fails, the session row stays, and restart is refused as well. An unmounted volume recovers once it is mounted
again; the other cases never do. Reviewers found no CLI or UI action that retires the row, so only hand-editing the
SQLite database gets rid of the session.

At one level this is deliberate. SPEC_impl.md ("Owned checkout admission and lifetime") says "Root identity is checked
even before accepting an apparently missing source." The reason is that an empty replacement root at the same path must
not be taken as proof the checkout is gone; the checkout may have moved elsewhere along with its original root.
SPEC.md's Lifecycle operations section, however, says that when a session's working directory has vanished, "delete
still works". Here there is no way out at all. When the root is truly absent, retiring the record loses nothing, because
nothing is moved or deleted.

The open question is for the maintainer: is the root-identity rule meant to override "delete still works" with no escape
hatch?

Suggested change:

- Keep refusing to move anything under a root that cannot be verified.
- When the root path is absent (`ENOENT`), retire the record as missing, with a warning that names both the root path
  and the checkout path.
- When a different object sits at the root path, either keep failing closed, or offer an explicit, logged, confirmed way
  to drop the record.
- Record the decision in SPEC.md/SPEC_impl.md.
