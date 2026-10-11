### What this was about

Five regression tests could miss the behavior they claimed to protect or fail during setup on macOS. The directory
browser's routing test did not check whether its simulated local host rejected an unexpected directory request. The
upload test could cancel before the simulated supervisor received the request to finalize the upload. The terminal test
could accept an incomplete escape sequence being flushed after its original idle deadline. Two supervisor tests depended
on Linux process inspection without isolating that dependency. The maintainer chose small test fixes, with any
substantial expansion returned for review.

### Things you should know

All five outcomes fit the agreed scope. The directory test now checks both hosts; the simulated supervisor waits for the
upload finalization request before triggering cancellation; and the terminal assertion rejects a flush after its
original idle deadline. Tab cleanup keeps portable coverage for two dead tabs and a live sibling, with detached-child
cleanup checked separately on Linux. The upload memory test runs only on Linux, where its process-memory observation is
available.

This changes test coverage only. The macOS check compiled the supervisor tests and rejected warnings; it did not run
them on macOS. The Linux-only detached-child and memory tests both ran successfully on Linux.

### Open questions and possible follow-ups

A separate create-session routing test still stops its simulated local host without checking whether that host rejected
an unexpected request. A follow-up should inspect whether the same coverage gap exists in
`crates/farhelm-helm/src/sessions_tests.rs`, test `a_create_defaults_to_the_local_host_and_honors_an_explicit_one`. The
shared two-second window used to observe an expected absence of activity remains tracked in
`review_feedback_queue/shared-silence-observers-have-same-premature-success-boundary.md`; changing it was outside this
plan. All five outcomes stayed within the agreed small-fix scope.

### The PRs

- [#1912](https://github.com/scode/farhelm/pull/1912/changes): all five test outcomes and their queue bookkeeping; still
  a draft, with nothing marked ready or merged by this executor.

### Checks run, reused and skipped

All fixed-code checks passed. Focused and repeated Rust executions used the recorder, pinned nextest, four slots and
zero retries; the tab-cleanup checks used pinned tmux. Other work was active during repetitions. No selected test
reported a runtime substrate skip; nextest's unselected tests are outside this coverage.

- `cargo nextest run -p farhelm-helm --lib -E ...`: directory routing and in-flight upload abort passed, recorder
  `49f75c8e-f52f-4bae-bb38-e0c9f6e8ccad`.
- `cargo nextest run -p farhelm-supervisor --lib -E ...`: idle flush, portable tab cleanup, Linux detached-child sweep
  and Linux upload-memory observation passed, recorder `a1f08bea-06bf-4b60-a668-40095264db60`.
- Thirty separately recorded idle-flush repetitions passed, from `1fe795cf-8153-4a02-b249-e5c2acb8f0e2` through
  `06fcd71e-fecb-456f-9b3e-90a4bdd88259`. Three paired helm repetitions passed: `ca8bbf77-b105-462d-b799-e1286b2698f2`,
  `4b7e2527-ec79-4ac1-9924-d6068a945371`, and `9b7d5ddb-d380-40cd-bda7-205bf3b3b4c6`. These check the paused-clock
  timing assertion and real-task races without expanding to unrelated suites.
- Three reverted mutation checks rejected their intended bugs: an additional local browse failed on the local peer's
  verdict after the remote reply passed (`ba441941-11c2-4a85-887b-ed99421d6b32`); removing the in-flight upload-abort
  watcher failed at the unchanged five-second response bound (`4d1ac7af-d5ec-43f7-a712-904d24639077`); renewing the idle
  deadline failed with a 90 ms flush (`a5b8e13d-2c2d-4960-82df-c15c3e653cdb`). An initial awaited-browse mutation hung
  because the fixture deliberately retains the connection after peer exit; it was stopped and retained separately
  (`4efad5e0-2a3b-40d5-8c53-503d36a2772d`), and supplies no successful mutation proof. All temporary product edits were
  restored by exact comparison before review and the repeated fixed-code runs.
- `cargo clippy --all-targets -- -D warnings` passed. The plan's
  `cargo clippy --locked -p farhelm-supervisor --tests --target aarch64-apple-darwin -- -D warnings` cross-check also
  passed, with its prescribed no-op C tools and separate target directory. These cover Linux warnings and macOS test
  compilation, respectively.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` with the isolated interpreter, and
  `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` passed. The sleep checker inspected 287 delays and
  found zero missing rationales.

Runtime and Clippy evidence covers the reviewed test diff based on `61386cb1`. The careful rebase onto `b3d61025`
preserved that five-file test diff byte for byte. Intervening installer refusals, UI license declarations and plan
bookkeeping changed no tested helper, dependency contract or specification, so their arrival required no runtime rerun.
The complete retained-run summary and per-attempt identities are recorded in
`farhelm-plan-helm-and-portability-test-fixes-log.md` beside the checkouts.

Workspace-wide runtime tests, browser, desktop, installer and doctest suites were skipped: these test-only edits have
focused execution and mutation evidence, with no uncovered product integration change. The shipped-binary-only Clippy
check adds no test-configuration coverage. Native macOS execution was not performed; the plan explicitly required the
cross-target compilation check instead.

### Review gate outcome

The required fresh-context source reviewer, requested on gpt-6.1-sol at high effort, reported no findings. Its report
checked the five test contracts, all mutation proofs, the platform split, the authoritative triage criteria and the full
test-authoring checklist. I read and accepted the complete report; no source correction was needed, and all five
outcomes stayed within the agreed scope. The independent wording cold read also accepted the title and motivation.
Implementation and validation stayed local as required; native model attribution and usage counters were unavailable.

### Landing

Landed on 2026-10-11 (UTC) as #1912. The rebase met the review queue index, where supervisor-lifecycle-test-fixes had
removed adjacent entries; both sets of removals were kept and the index still matches its files.

#### Review before merging

No blockers; every change in the product files sits inside their test modules. The browse, upload-abort, idle-flush,
upload-memory and tab-reaping tests now observe what their findings said they missed; the idle-flush test was traced
against the pinned Tokio and is deterministic under load. Note left as is: if a browse were sent only to the local host,
the test would hang until the runner's five-minute limit rather than fail with a clear message; a timeout around the
request would fix that.

#### Checks

These two plans, which change only tests, were landed together: helm-and-portability-test-fixes first,
browser-spec-fixes on top.

- Run now, on the two stacked: `cargo fmt --all -- --check`, `dprint check`, the test-delay checker (288 delays, none
  without a reason) and `cargo clippy --all-targets -- -D warnings`; the helm and supervisor unit tests through the
  recorder with pinned tmux 3.7c, four slots and no retries (run `b6805a2b`, 2143 of 2143); and on Chromium and WebKit
  with one worker and no retries the auth and spawn specs (run `c12ecee6`, 16 passed; the opt-in real-Claude spawn test
  skipped on both engines as designed) and the sidebar's long-folder-path test (run `b8ed84e9`, 2 passed).
- Not run: macOS; the real-Claude spawn test, which needs vendor credentials and is opt-in.
- Between those runs and the merges, main changed only plan bookkeeping.

Nothing in the report above was made untrue by the landing.
