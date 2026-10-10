### Since the last review

[PR #1776](https://github.com/scode/farhelm/pull/1776/changes) was amended; no PR was added or dropped. The maintainer
chose to repair the existing shutdown test without weakening its guarantee, and to block again if that needed
significant complexity. The repair uses pause and watcher-failure controls already present in this PR. It adds no
production behavior or new test mechanism.

The earlier landing fixes remain: Linux retries recursive registration when a report folder disappears during
registration, while persistent errors retain periodic fallback; the implementation specification and comments describe
busy-session skipping correctly. The PR remains a draft.

### What this was about

An agent could finish reporting its conversation while Farhelm waited for the supervisor's next two-second pass. File
events now wake conversation-report processing, including the first report in a new session directory. The regular pass
remains the backstop.

### Things you should know

Linux uses notify 8.2.0's inotify backend. macOS uses its default FSEvents backend, whose batching can still delay
pickup. Missing events or unavailable file watches leave the regular pass active. A persistent watcher error logs once
and disables immediate pickup until the next supervisor startup.

Reports retain the existing session ownership and conversation checks. They are processed one at a time; an event
arriving during processing remains due for another pass. A short recovery pause bounds repeated attempts during a
database or tmux outage. The regular status pass skips a session whose report processing is busy, while Restart waits
for pending reports before choosing the conversation to resume. Stopping the supervisor waits for a report pass already
underway to finish.

The old shutdown test paused the regular pass by holding report admission busy. That pass now intentionally skips the
busy session, so the test could no longer reach its setup and timed out. The repaired test disables the independent file
watcher, verifies that refusal occurred, and pauses the regular pass at its existing directory-listing boundary. It
proves shutdown remains pending and the report warning has not happened before release, then verifies the warning after
shutdown completes. This keeps the original completion guarantee.

### Open questions and possible follow-ups

No maintainer decision is outstanding. Native macOS execution remains untested. The FSEvents dependency and recursive
attachment compiled for Apple Silicon in an isolated check; the full supervisor cross-check stopped at bundled SQLite
because the executing Linux environment lacks a macOS C compiler.

### PRs

- [PR #1776](https://github.com/scode/farhelm/pull/1776/changes) — prompt report pickup and periodic fallback, with the
  repaired shutdown proof. Draft head `585f807ea432`, based on main `6bfbd9af8036`.

### Checks run, reused and skipped

Rust executions used the recorder, pinned nextest 0.9.143 and tmux 3.7c, four global slots and zero retries. No selected
runtime substrate skip appeared in the successful runs listed below.

Run this round:

- Exact shutdown reproduction, `test(=service::ticker::tests::shutdown_waits_out_a_report_refresh_already_in_flight)`,
  failed at its obsolete readiness premise in run `455417dd-8413-4b13-8acb-d98bc632cc38`. Its record remains retained.
  This was a deterministic fixture incompatibility, not a latent flake. The repaired exact test passed 1/1 in
  `b204fbe2-3f89-49f8-8038-75f66812b4b9`.
- `cargo nextest run -p farhelm-supervisor --lib` selecting `service::report_watch::`, `service::report_files::`,
  `service::capture::`, `shutdown_stops_the_ticker_and_waits_for_it`, and `the_task_ends_when_its_supervisor_is_dropped`
  passed 29/29 in `5f1abc1e-64cf-4fd1-8cff-1e5f6ad2a4fb`. This covers watcher registration and fallback, capture
  contention, and stop/drop behavior alongside the repaired proof.
- `cargo fmt --all -- --check`, `dprint check SPEC_impl.md TODO.md releasing/changelog.d/prompt-hook-reports.md`, and
  `python3 releasing/check-changelog.py format` passed. The isolated `python -B scripts/check-test-sleeps.py` found 281
  delays and zero missing reasons. These inspect the changed source and prose directly.

Reused from the first execution, whose pushed head was `8930ddd9c559`:

- Real hook first-directory and burst proofs,
  `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::watch_applies)'`, passed 2/2 in
  `a7f5c59c-a5d6-42f8-8ec2-07815e68a28d`. They use a ten-minute ticker and observe the initial empty scan before
  reporting, excluding a periodic pass or startup catchup as the explanation. The later monitor fix changes registration
  error retry; it preserves this normal event path, which also received fresh source review and the current watcher unit
  selection above.
- Neighboring `hook_identity::`, `codex_identity::`, and `restart_with_resume::` integration proofs passed 39/39 in
  `f4333797-c82d-46d5-886a-7f283b0400fa`. The report acceptance and Restart contracts they cover were preserved; current
  unit execution covers the changed scheduling and registration paths. These results do not validate unrelated newer
  upstream features.
- Three first-report repetitions passed 1/1 each: `66c85c15-0c92-4eb2-a8ae-9f86a1969a76`,
  `86dc0c73-e611-4571-adcf-be8e15dc875f`, and `5cf1658a-5d13-40eb-9dc6-e886c18d4615`. They show repeated ordinary Linux
  pickup, rather than forcing the original subscription race.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo check -p farhelm-desktop` passed for the first execution. The shipped watcher mechanisms and owner types
  remain; subsequent changes are registration retry and the contained test fixture repair, reviewed independently and
  compiled by the current supervisor selections. These older checks were not rerun and are not claimed as lint or
  desktop compilation of the current head.
- The isolated notify Apple Silicon compilation remains dependency-level evidence. Full supervisor compilation and macOS
  execution remain unavailable.

Rebase assessment inspected upstream supervisor lifecycle, ownership and filesystem work, the host and launcher changes,
and the corresponding specification changes. Trash workers and session directory admission do not change report-drain
locks; shutdown-expiry output quieting does not change cooperative ticker stop; host appearance and launcher
presentation do not change report scheduling. The later OMP change tightens top-process attribution before report
publication, preserving file processing and watcher behavior. The final rebase changed no report code or repaired
fixture. The current unit results therefore remain applicable without repetition solely for a new hash.

Full Rust and browser batteries, native desktop execution, doctests, installer, provisioning, release and hosted CI were
skipped: the remaining repair is test-only, and the targeted current proofs cover its shutdown and report-processing
contracts. No UI asset or executable documentation changed in this PR. Native macOS testing would add platform evidence,
but was unavailable.

An earlier development compilation, `cef274cb-ca05-428b-b2a3-9d5e5bf717fd`, failed before tests because a resource-proof
collection needed a type annotation; that was corrected and its failed record retained. An incomplete earlier
source-delay check was replaced by completed source-only checks, rather than treated as a pass.

### Review gate outcome

A fresh source reviewer configured as gpt-6.1-sol at high effort reviewed the whole current PR, retained landing
repairs, shutdown fixture, specifications and notify backend contracts. It reported no findings. The repaired test
preserves the maintainer's guarantee and passes the complexity gate. Source review did not execute tests; the executor
supplied the recorded runtime results above.

The commit and PR wording passed a fresh gpt-6.1-sol medium cold read after a rewrite clarified the report trigger and
macOS batching. Required inherited-model resume and report cold reads are separate from that source review.
Implementation and investigation stayed with the executing agent; delegation was limited to prescribed reviews. Native
tools expose requested reviewer configurations, but not independently reported runtime model identities or token
counters. Private review evidence remains in the executor's session record and working log.

### Landing

Landed on 2026-10-10 (UTC) as #1776, one squash commit on main, in the plan's second round. The first landing attempt
was blocked: the plan made the supervisor's periodic pass skip a session whose report is being taken in, and an older
test that proves a stop lets a pass already underway finish had used exactly that wait to pause the pass, so it could
never reach its setup. You chose to rework the test, with a test-only hold point if needed, under a complexity gate.

#### Fixes made while landing (first attempt, kept)

The first landing attempt fixed two problems that stay in what landed: a report folder removed while the supervisor was
refreshing its list of watched folders switched immediate pickup off for the rest of that supervisor's life (it now
retries, and still gives up on errors a retry cannot fix, such as the system's limit on watches); and SPEC_impl.md, two
code comments and the new file-watching library's declaration were corrected.

#### Review before merging

A separate reviewer read this round by reading the code only. The reworked test holds the periodic pass at its existing
report-listing point, after the pass's last stop check and before the warning it ends with, with the file watcher
disabled (and that refusal checked) so the periodic pass is the only thing that can do the work. A stop that cancelled
the pass instead of letting it finish would leave the warning unset, so the test still tells the two apart. No test seam
reaches the shipped binary, and nothing else changed since the earlier review. Two nits were left as they are: if a
future change made startup create the reports folder early, the test would hang until the runner's limit instead of
failing with a clear message; and the hold point's documentation still describes only its first users.

#### Checks

- Run now, on the plan rebased onto main: `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and the supervisor's unit tests in full through the recorder with
  pinned tmux 3.7c, four slots and no retries (run `d8fc910c`, 1083 of 1084). The one failure was a tmux output-filter
  test this plan does not touch, which waits a fixed window for a filtered pane's output to resume; it ran while the
  machine's load average was between 17 and 26 on 18 CPUs, and then passed ten times in a row on its own. A single
  failure under that load is not logged as a flake.
- Reused from the executor: its runs of the reworked test and of the real-hook selection from the first round; the
  landing changed nothing this round.

Nothing in the report above was made untrue by the landing.
