## What this was about

The OMP documentation promised that Farhelm could follow conversations started through `npx`. On Debian, Ubuntu and
other Linux systems whose `sh` is dash, the agent runs but Farhelm cannot follow its conversation, leaving Restart
unavailable. The maintainer chose to correct the documentation without changing how launches are recognized.

The [OMP page](/docs/agents/omp/) now distinguishes running the installed `omp` command, `bun x` and the compiled binary
from this `npx` limitation. Its source comments and implementation notes distinguish argument-pattern tests from live
npm behavior. The corresponding documentation TODO entry is removed.

## Things you should know

This changes documentation and comments only. It does not make `npx` conversation tracking work. Launches elsewhere,
including macOS, remain explicitly untested. There is no changelog entry because this changes no released behavior.

## Open questions and possible follow-ups

The existing statement that a resident wrapper always leaves Restart greyed out could not be confirmed from the code. A
single native or shell wrapper at the terminal's top process may be accepted; extra intermediary processes are refused,
and a Node launcher is refused for a direct `omp` launch. The plan explicitly allowed retaining that sentence and
reporting this gap. A follow-up can verify the relevant wrapper launch shapes and narrow the statement if needed. No
live wrapper or macOS `npx` verification was performed here.

## The PRs

- [#3207: correct OMP npx support claims](https://github.com/scode/farhelm/pull/3207/changes) — one draft PR based on
  main.

## Checks run, reused and skipped

- Passed `dprint check SPEC_impl.md TODO.md website/src/content/docs/docs/agents/omp.md` and
  `cargo fmt --all -- --check`.
- Passed `bun install --frozen-lockfile` and `bun run build` in `website`: 36 pages built and all internal links valid.
  The build reported warnings for the empty i18n collection and missing 404 content, then completed successfully.
- These checks covered revision `1c2c43ede8c66805954ef222f8835078fc0456ac`; the draft commit
  `8e2dde49355d02d2236f5d39ca3c58861064e6aa` has the same file contents. No additional checks were reused from another
  run.
- Skipped runtime tests, clippy, browser tests and CI because only prose and comments changed. No test or fixture
  changed, so no test-sleep check was needed. No instruction file or heading changed, so no agent-reference check was
  needed. No preview server was started because this is an unattended plan.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort passed the change with no actionable findings. It
identified the retained wrapper statement's verification gap described above. The independent commit-title cold read
matched the intended documentation correction and found no contradiction or convention violation.

### Landing

Landed with one wording fix made at landing. The OMP docs page now says what Farhelm can and cannot follow when OMP is
launched through `npx`, and the source comments and implementation notes say the same.

- #3207: the docs page, comments and implementation notes.

#### What landed since the stack was based, and whether it interacted

The round also landed own-host-sessions-without-asking and snapshot-in-place-paint, and main gained six test-only PRs
(#3215 to #3220); none touches OMP. stop-start-message, below this plan in the round's stack, did not land; this plan
does not depend on it.

#### Fixes made at landing

- The new `npx` sentence had left the following sentence's "that command" with nothing to refer to; it now says "the OMP
  command".

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

The executor's report is otherwise still accurate, apart from the PR's draft state; it is merged.
