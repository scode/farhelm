### What this was about

An agent could finish reporting its conversation while Farhelm waited for the supervisor's next two-second pass.
Conversation tracking now receives a file-event wake, including the first report in a newly created session directory.
The regular pass remains the backstop.

### Things you should know

The maintainer chose notify 8.x across Linux and macOS. Linux uses inotify; macOS uses the default FSEvents backend,
whose batching may add latency. Missing events or a failed watch still leave the regular pass active. A failed watch
logs once and stays disabled until the next supervisor startup.

Each report still has to belong to the running session and satisfy the existing conversation checks. Reports are
processed one at a time. A report arriving during another update remains due for a later pass, and a short pause
prevents repeated retries from consuming CPU continuously during a database or tmux outage. A slow report check can
delay other reports in that pass; the regular status-sampling task skips a session whose conversation update is busy
instead of waiting behind it. Restart still waits for pending reports to be processed before choosing which conversation
to resume.

Review found a Linux race: notification of a new session directory could arrive before the library had subscribed to its
files. Farhelm now confirms that the recursive watch is in place before listing those reports. If the watch fails during
registration, event-driven pickup ends and the regular periodic pass carries on. A shutdown request during registration
prevents another report update from starting. A report update already underway at shutdown finishes its database write
and matching in-memory state. The library's Linux thread releases its resources asynchronously after its watcher is
dropped; a focused test observes eventual release of that thread and its inotify descriptor.

### Open questions and possible follow-ups

No product decision is outstanding. The notify FSEvents dependency and recommended recursive attachment compiled for
Apple Silicon in an isolated check, but FSEvents was not exercised. The full supervisor cross-check stopped at bundled
SQLite because this Linux environment lacks a macOS C compiler. Native macOS validation remains the coverage gap.

### PRs

- [PR #1776](https://github.com/scode/farhelm/pull/1776/changes) — prompt conversation-report pickup, periodic fallback,
  lifecycle and hook proofs, implementation specification and removal of the covered TODO. One reviewed draft PR; not
  marked ready or merged.

### Checks run, reused and skipped

All Rust execution used the recorder, pinned nextest 0.9.143 and tmux 3.7c, four global slots and zero retries.
Successful selected tests printed no runtime substrate skips. Broader compilation and integration work ran in an owned
Linux container bounded to four CPUs and 12 GiB memory.

- Corrected supervisor selection:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(report_watch::) | test(report_files::) | test(periodic_refresh_skips_a_capture_claim)'`,
  run `ec126bc6-53b6-46eb-856a-78503d809ade`, 13 passed. After the final post-registration stop check,
  `test(report_watch::)` ran again as `dba5d567-3d31-4c60-aa7a-d0a01f49b1cb`, all five passed. The unchanged drain and
  capture-contention cases retain their earlier coverage.
- Real-hook selection: `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::watch_applies)'`, run
  `a7f5c59c-a5d6-42f8-8ec2-07815e68a28d`, both passed. The first-directory and burst cases use a ten-minute ticker and
  observe the initial empty watch scan before reporting, so a scheduled pass or startup catchup cannot explain
  acceptance.
- Neighboring identity and Restart selection:
  `cargo nextest run -p farhelm --test e2e -E 'test(hook_identity::) | test(codex_identity::) | test(restart_with_resume::)'`,
  run `f4333797-c82d-46d5-886a-7f283b0400fa`, 39 passed.
- Three separate repetitions of `test(=hook_identity::watch_applies_first_report_before_a_long_ticker_interval)` each
  passed: `66c85c15-0c92-4eb2-a8ae-9f86a1969a76`, `86dc0c73-e611-4571-adcf-be8e15dc875f` and
  `5cf1658a-5d13-40eb-9dc6-e886c18d4615`. These establish repeated ordinary backend pickup; they do not
  deterministically force the library's original subscription-gap schedule. The acknowledged-registration repair also
  received source review.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo check -p farhelm-desktop` passed. The binary lint checks the shipped configuration with test seams disabled;
  desktop compilation checks the supervisor's other shipped owner.
- `cargo fmt --all -- --check`, changed-file `dprint check` and `python3 releasing/check-changelog.py format` passed.
  The isolated `python -B scripts/check-test-sleeps.py` inspected 279 delays with zero missing rationales; reused after
  subsequent stop/premise checks and prose edits introduced no delays or test modules.
- The full Apple Silicon supervisor check was attempted and blocked at SQLite's C compilation. An isolated notify 8.2.0
  check compiled its FSEvents dependency, recommended recursive attachment and Send bound for `aarch64-apple-darwin`.
  This is dependency-level compilation evidence, not full supervisor compilation or macOS execution.

The runtime checks cover the working change based on `3eadb14b`. Main through the PR's base `194b4aac` changed only
queue claims, deliveries, reports and the blocked sound-plan question. Those diffs were inspected for interactions; none
changed report pickup or its callers, so the successful results were reused after the clean rebase. The PR head is
`8930ddd9c559`.

One development compile attempt, `cef274cb-ca05-428b-b2a3-9d5e5bf717fd`, failed before tests because the new
native-resource proof needed an explicit collection type. That was corrected; the failed record remains retained. An
earlier source-delay check ended incomplete, and completed checks supplied the result above. No runtime failure was
retried away.

Full Rust/browser batteries, browser integration, native desktop execution, doctests, installer, provisioning, release
and hosted CI were skipped. This change affects report scheduling and capture contention; the selected watcher, hook and
Restart proofs cover those contracts, while no UI assets, executable examples, installer or release procedure changed.
No live-install or deployment operation was performed.

### Review gate outcome

The mandated general reviewer was configured as gpt-6.1-sol at high effort and ran through the native agent tool. It
reviewed the scope reassessment and final source; all findings were addressed. The commit and PR wording cold read used
a native agent configured as gpt-6.1-sol at medium effort and passed. The execution tools expose those requested
configurations, but do not return independently reported runtime-model identity or token-usage counters.

The executing agent performed the implementation, investigation and validation itself. Delegation was limited to the
required source review and cold reads; no implementation worker or foreign-harness shell-out was used. Source-review
conclusions and the executor's runtime evidence are distinguished above.
