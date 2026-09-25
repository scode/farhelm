# Checkout membership misses bind-mount aliases

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In a bind-mount setup, deleting one session can archive a checkout another listed session is still working in.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F11 / COR-BIND-MOUNT-MEMBERSHIP`, tagged **possible**. Anchors and title:
`working_copies.rs:655-681`, `working_copies.rs:1028-1047` — Checkout membership is path-text based, so a session
reaching a checkout through a bind mount does not keep it in place

A session counts as a user of a managed checkout, and so keeps it from being archived, only when its canonical working
directory is the checkout path or lies under it. The canonical directory is the `realpath` computed at create time. The
comparison is on path text, in `attach_existing_ancestors` for new sessions and `attach_retained_sessions` at
allocation.

`realpath` resolves symlinks but not bind mounts. A session whose working directory reaches the checkout through a
bind-mount alias is therefore not counted. When the last counted session is deleted, the checkout is archived while that
other session is still working in it. Its processes follow the renamed directory into the archive, and its recorded path
no longer points at anything.

Open premise: SPEC.md defines references over the managed directory "or its canonical subdirectories", so bind-mount
aliases may be out of scope, and such setups are rare. One reviewer explicitly declined to report this for that reason.

Suggested change: if bind mounts are in scope, also compare the `(dev, ino)` identity of the cwd and its ancestors when
attaching memberships. Otherwise, write the limitation down in SPEC_impl.md. The second option is documentation-only.
