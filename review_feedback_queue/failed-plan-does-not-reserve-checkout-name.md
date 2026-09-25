# A failed clone's plan does not reserve its folder name

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a failed fresh clone, a later clone can reuse the folder name; deleting that clone's session leaves its folder in
place, and deleting the old failed session later archives it while logging that it was left alone.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F8 / COR-PLANNED-NOT-RESERVED`, tagged **possible**. Anchors and title:
`service/core.rs:2091-2096`, `working_copies.rs:938-944`, `working_copies.rs:1028-1047`, `service/teardown.rs:487-505` —
A failed clone's unfinished plan does not reserve its folder name, so a later clone takes the folder and the failed
session silently co-owns it

A fresh-clone create first records a `planned` registry row for its target, say `<root>/bar-1`, and only then runs
`mkdir`. The plan can end up permanently unfinished in two ways:

- `mkdir` fails with an error other than "already exists", such as `EACCES` or `ENOSPC`. `allocate` classifies that as
  `Uncertain`, since the directory might have been created, and the create path keeps the session as a visible Error
  session together with its plan.
- The supervisor crashes between recording the plan and `mkdir`.

In both cases the session's recorded cwd is `<root>/bar-1`, but nothing treats that path as taken. There is no directory
for the occupancy scan to find, and `fresh_root_constraint_error` skips rows that have no recorded directory.

A later, unrelated create B can therefore preview and allocate the same `bar-1`. During that allocation,
`attach_retained_sessions` makes the old session A a member of B's checkout, because A's recorded cwd string equals B's
path. From then on:

- Deleting B's own session no longer archives B's checkout, because A still counts as a reference.
- A cannot recover. In the crash variant, retrying A is refused because the planned path is "occupied but its registry
  row recorded no directory identity". In both variants, restart of A is refused because its plan is unresolved.
- Deleting A handles A's old plan first, since memberships are processed in creation order. It logs that the directory
  at `bar-1` "has no established ownership and is left untouched", even though that directory is B's managed checkout.
  The same Delete then reaches B's row, finds A is its last member, and moves `bar-1` into the archive. The log line is
  false twice over: the directory is owned, and it was not left in place.

SPEC.md says ownership follows use. Here a session that never had a directory becomes an owner of someone else's
checkout, which changes when that checkout is archived, and the Delete diagnostic misleads whoever reads it.

Open premise: this needs a fresh clone left `planned` with no directory, either through a non-EEXIST `mkdir` failure or
through a crash in that window.

Suggested change:

- Treat an active `planned` row's recorded path (`canonical_root/original_basename`) as occupied, both in the name scan
  and in `fresh_root_constraint_error`.
- In `attach_retained_sessions`, skip sessions whose only link to the path is their own unfinished fresh plan.
- Emit the "left untouched" diagnostic only when no active row owns the path.
