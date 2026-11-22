# SQLite maintenance in the helm and supervisor

## What this was about

Farhelm uses SQLite to remember its hosts, cached sessions and preferences in the helm, and session state in each
supervisor. After a burst of writes, SQLite can retain the space in its write-ahead log (WAL) for later reuse. The plan
called for reclaiming space when SQLite can reuse the log and making database growth visible in Farhelm's logs.

## Things you should know

Shared read-write connections set a 6 MiB journal limit. SQLite can exceed it during a burst of writes or while a reader
holds an earlier view of the database. The limit applies when SQLite rewinds the log. When the helm or supervisor starts
with the exclusive lock on its state directory, it asks SQLite to refresh query planner statistics if due after schema
setup. It then logs database pages, free pages and WAL bytes.

The serving owner attempts to truncate the WAL on orderly stop after its background writers have finished. If another
connection still holds a snapshot, Farhelm logs that the WAL is busy and exits normally. The 250 ms limit applies to
waiting for SQLite locks. The whole checkpoint can take longer because writing and syncing pages depends on storage
latency. SQLite normally removes the WAL on the last clean close. The explicit checkpoint also covers a stop while an
idle connection remains open.

The plan accepted that ordinary macOS app quit ends the helm before this checkpoint usually runs. The journal limit
still applies during operation, and SQLite recovers the WAL on the next open. The plan uses the existing schema and
retention policy, with maintenance limited to the stores' ordinary read-write connections.

## Open questions and possible follow-ups

None.

## Pull request

- [#4574](https://github.com/scode/farhelm/pull/4574/changes) covers both databases and removes the completed TODO.

## Checks

- `cargo nextest run` on the final source selected both store modules, shared database tests, runtime ownership and
  startup cases, and the `supervisor_stop` suite from the end-to-end target. All 375 tests passed, including a reader
  holding a WAL snapshot, the helm's late write and five real supervisor shutdown cases. Recorder run
  `a695868e-07ad-4722-90be-933bc6711b90`.
- `cargo clippy -p farhelm-supervisor -p farhelm-helm --all-targets -- -D warnings` passed.
- A local C probe linked with bundled SQLite 3.53.2 and STAT4 timed the first `PRAGMA optimize=0x10002` on a synthetic
  cache with 50,000 records at the 500-session cap across 100 hosts. It took 38.2 ms in that run, independent of other
  startup work. Recorder run `28903139-3035-498d-ad81-a63c7b668f49`.
- `cargo fmt --all -- --check`, `dprint check SPEC_impl.md TODO.md`, `git diff --check` and
  `python -B scripts/check-test-sleeps.py` passed. All PR validation is from this run series.
- Focused local tests cover the database behavior and Linux process shutdown, so the full workspace run and hosted CI
  were skipped. This run used Linux, so the accepted macOS quit behavior was inspected in source.

## Review gate

The independent correctness review passed after corrections. The supervisor now checks it owns the state directory
before carrying the connection through shutdown, including when a stop races the refusal to serve. The owner measures
the WAL using SQLite's resolved filename, and both stores' tests verify the size event is emitted only by owning opens.
The executor verified each correction against the final diff and test results.
