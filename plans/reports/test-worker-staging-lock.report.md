## What this was about

When two test requests targeted the same worker checkout, the second copied its tools and source before checking whether
the checkout was busy. It could overwrite what the first run was using and then copy the first run's logs as its own. A
standalone setup could also change shared prerequisites while a different checkout was running tests.

## Things you should know

Each request stages its own files. The worker installs them under the checkout lock, keeping file mtimes and the build
cache for incremental runs. The lock stays with the worker when SSH disconnects. Setup waits for other setup calls and
refuses while tests are running. The driver prints an invocation ID at startup. Use it with
`scripts/test-worker/test-worker.sh collect --out <directory> <invocation-id>` to retrieve a detached run's evidence
after it finishes. The worker retains 20 invocations per checkout. A local output directory carries one invocation stamp
and rejects concurrent reuse.

Stop keeps both locks through recorder shutdown. It also waits when the foreground command exits before another process
in its group. If the group still runs after the 35-second grace period, the worker escalates and retains whatever
evidence was written.

## Open questions and possible follow-ups

The local stand-in covers the driver, file transfer, locking and process-group behavior. An SSH daemon and setup's
package installation are outside that fixture.

## The PRs

- [#4575](https://github.com/scode/farhelm/pull/4575/changes) protects staged input, active runs and collected evidence.
  Staging and detachment both needed evidence keyed by the same invocation ID, so they share one PR.

## Checks

- Ran `python3 -B scripts/test-worker/test-test-worker.py -v` on `39ac2a87543c`. All eight local scenarios passed,
  including a busy refusal with byte-identical source and tools, SSH-client interruption, bootstrap exclusion, output
  ownership and recovery, retention, and checkout-scoped Stop. Recorder `470e906b-3b55-4591-9e00-fae9e4f85f86`.
- Ran the command below on `e6ce5ed7758d`. All three cases passed with no skips. Recorder
  `e7817ac8-3dc0-40e5-ac15-775fb21edccd`. The earlier suite still covers the other paths. The later change recognizes
  live sibling threads when the main thread has exited. The rebases preserved the tested worker scripts through draft
  head `18180f0638c5`.

  ```sh
  python3 -B scripts/test-worker/test-test-worker.py -v \
    WorkloadStatTest.test_zombie_main_thread_does_not_hide_its_live_threads \
    WorkerTest.test_stop_keeps_both_leases_through_recorder_teardown \
    WorkerTest.test_forced_stop_leaves_a_collectable_invocation
  ```
- ShellCheck, Python byte compilation, scoped Markdown formatting, whitespace and the instruction checker passed. The
  instruction checker reported five pre-existing citation warnings.
- Skipped Rust, GUI and live worker runs. This change is to maintenance tooling and its committed fixture uses a local
  SSH stand-in.

## Review gate

Independent GPT-6.2 Sol high review and delta checks confirmed all findings were resolved. A separate scope review
confirmed the Stop implementation uses the recorder's group-ownership technique while allowing a normal run to finish
with a deliberately detached child.

### Landing

#### What landed with it

The plan's PR, #4575, landed last in this round, after the other three plans, with a landing fix, #4584. Since the
plan's work was written, main had gained only TODO entries and the Tensorlake triage's last PRs, which do not touch the
test worker harness. This round's own worker runs went through the new harness and worked, the first of them after the
re-run of the worker's setup that a changed setup script triggers.

An independent reviewer found no blocker: locks are always taken in the same order, Stop and cleanup are sound in every
path traced, nothing new reaches the worker or gains credentials there, and the test fixture never touches the real
worker.

#### Fixes made while landing

In [#4584](https://github.com/scode/farhelm/pull/4584/changes), one line of the worker instructions: they still said the
driver checks the setup marker before each run, which the worker now does itself under its setup lock.

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

- **Setup will re-run back and forth for a while.** Until every checkout that uses the worker is on the new main, a run
  from an old checkout and one from a new checkout each find the other's setup marker and re-run setup, and setup runs
  from the old version ignore the new rule that setup cannot run during tests. This stops once every checkout that uses
  the worker is on the new main; until then, avoid running setup from an old checkout while tests run.
- Every request, a refused one included, now uploads the whole tracked tree rather than only changed files (tens of MB);
  neither the report nor the instructions say so.
- If the worker-side process that supervises a detached run dies, the checkout lock and the setup lock are released
  while its tests may still be running, so another request or setup can start on top of them.
- Setup triggered automatically no longer streams its progress, and if SSH disconnects during an install, the failure
  message is lost.
- The worker's disk filled during this round's first workspace run (two tests failed with "no space left on device");
  the harness only cleans its build directory when a run starts with less than 15 GB free, and this run started above
  that. A higher threshold or a cleanup after each run is a possible follow-up.
