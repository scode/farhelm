# Planner tests miss refreshing an existing table's statistics

Reviewed commit: 4e01162e9954d73d0513898ecb16baa73984476b

## TLDR

The startup maintenance test can pass if Farhelm still creates missing SQLite statistics but stops refreshing stale
ones.

## Details

The fixture in `crates/farhelm-supervisor/src/db.rs:407` asserts that `sqlite_stat1` is absent before maintenance. It
establishes analysis of a previously unseen table and leaves a stale-statistics fixture untested.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `sqlite-maintenance/a10`.
