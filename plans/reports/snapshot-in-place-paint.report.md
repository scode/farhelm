## What this was about

Saving a terminal's screen and history for recovery briefly held two large buffers containing the same output. The
maintainer chose to reduce that peak for ordinary output while preserving the saved format. Some unusual screen states
cannot be recreated by replaying terminal output alone, such as a wide character with only one half erased; those cells
need separate saved corrections. Snapshots dominated by those cells remain the difficult case left for later, while the
ordinary text fixture uses almost entirely replayable output.

The encoder now reuses the output buffer for the final snapshot, shifting its contents in place to add the header.
Existing saved bytes stay compatible with running terminal hosts. The completed Near term TODO is removed, and the
requested Maybe later entry records the remaining cost of those cell corrections.

## Things you should know

On Linux, the ordinary-output test produced the same 3.55 MiB snapshot with peak growth of 3.7 MiB, down from 6.9 MiB.
That is about half the temporary memory in this fixture, not a reduction in the terminal's steady memory use. The new
assertion first failed against the unchanged encoder: 7,270,400 bytes of peak growth for a 3,726,399-byte blob. Its
measurement premise passed, so an earlier memory peak did not hide that growth.

A snapshot made mostly of cell corrections still needs their separate buffer: its 16 MiB blob raised the peak by 32.2
MiB, within the unchanged 36 MiB test allowance. The documented ceiling and size limit remain unchanged. Final snapshots
retain no spare allocation capacity. The changelog fragment uses `kind: none`, because this brief allocation improvement
changes no visible behavior.

## Open questions and possible follow-ups

Reducing the remaining peak for snapshots made mostly of cell corrections needs a separate design, now recorded under
Maybe later in TODO.md. The measurements here are Linux observations; they do not establish the same resident-memory
figures for another operating system or allocator.

## The PRs

- [#3209: lower terminal snapshot peak memory](https://github.com/scode/farhelm/pull/3209/changes) — one draft
  performance PR based on main.

## Checks run, reused and skipped

- Passed `cargo nextest run -p farhelm-terminal-emulator` through the controlled recorder: 124 passed, 20 ignored; run
  `c4935403-9a71-44fa-ba7b-fa39d9487e12`. The Linux run used pinned nextest, four global slots, zero retries and
  recorder tmux mode `none` because no tmux is used. This includes the exact-limit allocation regression and unchanged
  golden-byte, round-trip and fuzz tests. No fixtures were rewritten.
- Passed all seven release memory tests, each in its own process, through generic recording: run
  `d7d89a94-64f5-473e-8db1-a2fddf77afe2`. Child command:
  `cargo nextest run -p farhelm-terminal-emulator --release --test memory --run-ignored only --no-capture --retries 0`.
  The Linux run used pinned nextest, serial execution imposed by no-capture, zero retries and recorder tmux mode `none`.
  Generic recording preserves output and exit status; these seven passes were read from the retained test output, not
  controlled-run metadata.
- Retained the intentional pre-change assertion failure as run `992c8612-e4a4-41a8-96eb-4ba0a7a958c1`. Child command:
  `cargo nextest run -p farhelm-terminal-emulator --release --test memory --run-ignored only -E 'test(=typical_output_at_full_scrollback)' --no-capture --test-threads 1`.
  It ran one test and failed the new peak assertion; it is evidence of the regression test's sensitivity, not a passing
  run. Nextest reported that no-capture already implies serial execution and ignores the redundant thread flag.
- Both passing runtime runs covered base `3831ab172c2bbea1473efa248a9f73ef97846f1e` with tracked-diff SHA256
  `2003ee749a0d108458563528940f675c9f79a5fd86937114dd585b161f73878e` and complete source identity. Subsequent edits
  changed only comments, memory documentation, TODO entries and the changelog fragment, so runtime coverage still
  applies.
- Passed `cargo fmt --all -- --check`, `cargo clippy -p farhelm-terminal-emulator --all-targets -- -D warnings`,
  `dprint check TODO.md releasing/changelog.d/snapshot-in-place-paint.md`,
  `python3 releasing/check-changelog.py format`, and `<isolated-python> -B scripts/check-test-sleeps.py`: 423 delays
  inspected, none missing a rationale. The isolated interpreter used the repository's pinned parser requirements.
- Skipped browser, installer and unrelated workspace tests because this changes only the emulator's snapshot allocation.
  Ignored full-size and private-recording experiments and timing benchmarks were not needed beyond the ordinary crate
  tests and the complete memory suite for this allocation change. No instruction file or Markdown heading changed, so no
  agent-reference check was needed. No CI run was dispatched.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort passed the source change with no actionable
findings, including the test-authoring checklist. Commit wording received an independent cold read.

### Landing

Landed as delivered, with no fixes at landing. Taking a terminal snapshot now needs about half the temporary memory it
did in the measured case, as the report describes.

- #3209: the snapshot change, its memory tests, a changelog entry, and the "Maybe later" TODO entry for the case it does
  not cover (snapshots made mostly of cell corrections, such as a wide character with only one half erased). The plan
  asked for that entry.

#### What landed since the stack was based, and whether it interacted

The round also landed own-host-sessions-without-asking and omp-npx-docs, and main gained six test-only PRs (#3215 to
#3220); none touches the terminal emulator. stop-start-message, below this plan in the round's stack, did not land; this
plan does not depend on it. Its TODO.md change met main's newer entries in the same bucket and kept both.

#### Checks

At the maintainer's request the browser specs and the Rust tests ran on Tensorlake sandboxes; the builds, lints and the
UI's tests ran on the development machine. They ran on the round's four plans stacked in queue order on main `ec2e4984`
(own-host-sessions-without-asking, stop-start-message, omp-npx-docs, snapshot-in-place-paint):

- Locally: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, the test delay check (423 delays, none
  without a reason), `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`
  (no errors), `cargo build`, the release web build and the docs website build: all passed. The UI crate's tests in its
  desktop build (`cargo nextest run -p farhelm-ui --features desktop`), run `bab8b05a-205f-451a-b862-62da7102602c`: all
  1109 passed.
- Browser specs on eight sandboxes, both engines (`approval-layout`, `remembered-permissions`, `agent-relay`,
  `yolo-guard`, `spawn`, `terminal-multihost`, `keyboard-scroll`, `settings`, `modal-layout`, `terminal`, `stop-tabs`,
  `terminal-restart`; `scripts/tensorlake/sandbox-tests.ts`, each shard through the recorder's generic mode with
  `npx playwright test --project=...`, three workers, no retries; the sandboxes run Node 24.16 against 24.20 locally).
  Shard runs `60e17368-0041-4586-87f1-20a5ecbf5e27`, `4b2c0b69-c923-47db-8551-624e3c172738` and
  `61086004-e0e6-4413-800f-9527624fd7b8` each had one failure; the other five shards passed. Two failures were the
  terminal spec's "stop and delete failures surface in the row's own error line" on both engines: main's #3194 added a
  hidden copy of a row's error line, and #3220, which landed during the round, fixes that test; no plan here touches it.
  The third was WebKit's "clones onto the other host with the source's own command" (agent relay), a 30-second wait for
  the clone; a rerun of that group, three times on both engines, run `8dc2d88b-61b1-413d-a4dc-8e74000ead8f`, passed all
  24. That one failure is kept as evidence, not recorded as a flake from one observation.
- Rust tests on ten sandboxes: a `cargo nextest archive` of the helm, protocol, supervisor, terminal emulator and
  farhelm crates (every unit and integration target, the Podman container tests included), split into ten hash
  partitions, each run through the recorder's generic mode with pinned nextest 0.9.148, four slots, no retries. About
  4,660 tests ran; four failed. Three were stop-start-message's: on these faster machines a test's Stop now often lands
  before the agent has started, which that plan correctly records as "stopped before the agent finished starting" while
  the tests still expect "stopped by user"; it went back to its executor to fix the tests and did not land this round.
  The fourth, `host_creates::managed_github_token_is_fresh_for_agents_tabs_and_restart` (shard run
  `06dce18d-2dc3-4ae3-bc7c-fcf350a9ecc4`), passed in a narrow local rerun, run `0574a2a8-1b27-4ba8-a72d-ffd21c8cc24c`;
  it is the first run of these tests on sandboxes, and one sandbox-only observation is not yet a flake.
- After the checks: stop-start-message was taken out of the stack, the review's wording fixes went in (documentation and
  a changelog entry), and the merges rebased the PRs onto main's six test-only PRs (#3215 to #3220). Nothing was rerun
  without stop-start-message: the tested tree included it, and the review found no code it shares with these plans, so
  the runs above still cover them. #3220's fix of the terminal spec was not run together with these plans. Main outside
  `plans/` matches the tested tree minus stop-start-message, plus those six PRs' spec files. `dprint check` and the
  changelog check passed again after the fixes; the docs website build ran before the documentation fixes and was not
  rerun.
- Not run: the rest of the browser suite, the other Rust crates, doctests and the native Mac app.

The emulator's release memory tests were not rerun at landing: the executor ran all seven, and nothing that landed since
touches the emulator. The executor's report is otherwise still accurate, apart from the PR's draft state; it is merged.
