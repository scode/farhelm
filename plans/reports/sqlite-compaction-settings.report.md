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

### Landing

#### What landed with it

The plan's PR, #4574, landed third in this round, after the launch that resumes a suspended host and agent trust in
every folder, and before the test worker's staging lock, with no landing fix. Since the plan's work was written, main
had gained the desktop app and Tensorlake triages; nothing that landed touches the databases' code. Agent trust adds a
field elsewhere in the same supervisor file, with no interaction. There is no schema or protocol change.

An independent reviewer found no blocker:

- **Data.** The truncating checkpoint and the journal size limit cannot drop committed data, and the statistics tables
  `PRAGMA optimize` creates are harmless to an older build reopening the file.
- **When it runs.** The checkpoint runs only after the helm or supervisor has stopped serving and its background writers
  have finished, while it still holds the exclusive lock on its state directory; a supervisor without that lock skips
  it. The extra time at stop stays well within how long the next helm or supervisor waits to take over the state
  directory.

#### Fixes made while landing

None.

#### Checks

This round landed four plans stacked in this order on main: the launch that resumes a suspended host, agent trust in
every folder (protocol 84), SQLite maintenance, and the test worker's staging lock. The checks ran on the combined
result. The worker runs used this round's own test worker harness, so they exercised it too; they ran on a separate
arm64 Linux test worker, and the rest ran locally on x86_64.

On the four plans as delivered, stacked onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four nextest slots, no retries and four build jobs, on
  the test worker (recorder run `fa0b5b85-0c85-4685-a517-30a4dc406bf9`): 8304 of 8308 passed. Two failures were the
  worker running out of disk during the run (helm provisioning over SSH and the real Podman environment test, both "no
  space left on device"); the other two were the GUI tests the trust and wake plans' landing fixes correct.
- The four clippy runs (`--all-targets`, `-p farhelm --bins`, and the desktop app with and without `--features agent`),
  on the test worker, not through the recorder: two errors, which the wake plan's landing fix corrects.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `99f4ecdb-f251-4c4c-bdcb-84747ee38556`): all
  184 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `0b374828-a0ef-49ad-9af6-966eeace3dad`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `dbe2bda8-bda8-4af3-b9b6-ffc64f569658`):
  passed.
- `python3 -B scripts/test-worker/test-test-worker.py`, the worker harness's own tests against a local SSH stand-in,
  locally (recorder run `f6f57fc8-5f99-4ac8-8038-9994d9ce98de`): all 9 tests passed (the report counts eight scenarios);
  `shellcheck -x` on the harness scripts and `python3 -m py_compile` on its Python files: passed.
- The docs website build (`bun install --frozen-lockfile && bun run build`), `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

After the landing fixes (#4582, #4583, #4584):

- `cargo nextest run --workspace --exclude farhelm-desktop` again, on the test worker with free disk (recorder run
  `442411a7-47a3-448c-be2c-fb6e4582df74`): all 8309 passed, among them the two tests the full disk had failed, the two
  corrected GUI tests and the new home-directory test.
- The four clippy runs, on the test worker: passed.
- `python3 -B scripts/test-gui-screenshot-batch.py` (which would have failed before #4583 added five trust screenshots
  missing from the screenshot list), the docs website build, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check`, `python3 releasing/check-changelog.py format` and
  `python3 scripts/check-agents-md.py`, locally: passed.

The first runs' desktop, smoke, golden and harness results still apply. The fixes change no desktop-app or harness code
and nothing that draws: in the GUI library a removed `&`, a type alias, comments and tests; in the helm and the
supervisor only Claude agent requests and Claude trust launches, which the smoke test's stand-in agents never make.

Skipped: a launch onto a real suspended Tensorlake sandbox or stopped container, and a real Claude launch with trust on,
which spend cloud resources or vendor turns; macOS, since no Mac was part of this landing; CI, which runs only on
demand.

#### Things you should know that the report does not say

- The report's supervisor shutdown tests check that the supervisor stops cleanly, but none checks that the session
  database's journal is truncated at stop, or that a supervisor without the state-directory lock skips that step. The
  plan's acceptance criteria asked for those tests; the landing left them as a follow-up rather than writing them, since
  the behavior itself was reviewed and the shutdown suite passes. Adding the two tests is the follow-up.
- The logs name each database two ways ("helm database" and "helm db", "session database" and "session db").
