# Checkout paths near PATH_MAX can't be archived

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With an extremely long checkout path, Delete fails forever.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F15 / COR-ARCHIVE-PATH-TOO-LONG`, tagged **possible**. Anchors and title:
`service/core.rs:4602-4608`, `service/core.rs:7432-7438` — Admitted checkout paths can be too long for the archive
destination

The preview (core.rs 4602-4608) and the create recheck (core.rs 7432-7438) accept a planned checkout path of up to 4096
bytes. The folder name itself is capped at 200 bytes, which leaves room for the archive suffix within the name. The full
path has no such margin.

Archiving turns `<root>/<name>` into `<root>/farhelm-archived-working-copies/<name>-<YYYYMMDDTHHMMSSZ>`, which is 49
bytes longer. After a name collision, a `-<32 hex>` UUID is appended as well, making it 82 bytes longer. A checkout
whose path is within that margin of the kernel's 4096-byte path limit fails the rename with `ENAMETOOLONG`. At that
point the row has already been marked `archive_pending`.

Recovery sees the unreadable destination, notes that the source still exists, and retries under a new name with a UUID
suffix, which is even longer. So Delete fails forever for that session, and restart is refused while the row is pending.

Open premise: realistic checkout roots are nowhere near this long, so this is a low-severity edge case.

Suggested change: reserve room for the archive suffix at admission, for example by rejecting planned paths within about
100 bytes of the path limit.
