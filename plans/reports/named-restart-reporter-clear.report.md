## What this was about

Restart with could select a different Pi or OMP conversation while leaving ordinary Restart aimed at the previous
conversation. This happened when Farhelm could not prepare the extension that reports the agent's current conversation:
the launch continued without reporting, so nothing replaced the saved old target. The maintainer chose to treat this
like conversation hooks being turned off.

Farhelm now checks that extension before deciding whether to retain the old target. A preparation failure clears it
through the existing relaunch transaction. If the relaunch definitively aborts before starting the replacement, the old
target is restored; a successful or ambiguous launch keeps it cleared. The product and implementation specifications,
changelog fragment and triage bookkeeping agree with that behavior.

## Things you should know

Restart with still proceeds when extension preparation fails. Ordinary Restart remains unavailable until a later
accepted report establishes a conversation. The extension is checked again at ordinary launch time; the plan's accepted
same-account file-tampering window between those checks remains.

The tests use owned tmux sessions and a launch stand-in. They establish the supervisor's capture, rollback and refusal
behavior, including a valid old saved conversation, healthy-extension retention and a deliberately mismatched extension.
They do not launch real Pi or OMP agents or prove vendor reporting. No schema change was needed.

## Open questions and possible follow-ups

None required for this outcome.

## PRs

- [#2742](https://github.com/scode/farhelm/pull/2742/changes) — clear stale resume when reporter setup fails; one draft
  PR, head `c53c7c30889b13f8eb47132bd37751d5b34ef085`, not marked ready or merged by this executor.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/named_(pi|omp)_restart_clears_capture_when_reporter_preparation_fails/)'`
  passed both new regression tests in recorder run `052b4522-da72-4a5b-84f0-8a37bf8d134d`.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/named_goose_restart_clears|named_restart_clears_capture_when_hooks|named_relaunch_clears_capture|pi_extension::tests/)'`
  passed all eight related tests in run `a6794385-abec-45e3-aa4e-5fa463e1d93a`.
- Both successful commands used nextest 0.9.148, pinned tmux 3.7c, four runner slots and zero retries. Their complete
  source evidence identifies base `6376dae9cbd3074506a51ca7dd725229d166c1f8` plus working-tree fingerprint
  `c95e281bd0acfc89437ea9df83dadc87792c6606ff089c5f03dbb1a8ab1f84ee`. Reports, output EOF and cleanup were complete. The
  1324 and 1318 skipped cases were outside the respective selections; neither run printed a runtime `SKIPPED` marker.
  These fixtures deliberately bypass systemd.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/named_(goose|pi|omp)_restart_clears|named_restart_clears_capture_when_hooks|named_relaunch_clears_capture|pi_extension::tests/)'`,
  run `0d12ff1f-8de7-4e04-8683-fa4e792a9ede`, failed two of ten tests because their fixture used a bare conversation ID
  where Pi and OMP require typed locators. The corrected fixture establishes a valid exact record and OMP ownership
  provenance before acting. That failed command remains retained and is not reused as passing evidence; the two
  successful commands above cover its complete selection.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` passed in recorder run
  `8e885fe8-8741-47c4-ba2e-ae0853f73e68`, with complete source evidence matching the runtime fingerprint above, full
  output and complete cleanup.
- `cargo fmt --all -- --check` passed for the final Rust source.
- `dprint check SPEC.md SPEC_impl.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md releasing/changelog.d/named-restart-reporter.md`
  passed. `dprint check TRIAGE_OUTCOMES.md` also passed on the published head after the PR-link amendment.
- `python3 releasing/check-changelog.py format` passed for the fragment.
- `python -B scripts/check-test-sleeps.py`, using the isolated interpreter prescribed by the repository, passed: 322
  calls inspected, no missing rationale.
- `python3 scripts/check-agents-md.py` passed with zero errors and two existing citation warnings unrelated to this
  outcome. It covers removal of the addressed feedback file.
- The successful commands above are reused individually for the published head. The patch was rebased from the tested
  base onto `660fb5b5`; the complete intervening main diff contained only another plan's delivery bookkeeping, and
  `jj interdiff` was empty. The static checks are reused on that same basis except the published-head ledger check
  explicitly noted above. Adding this PR's link to the execution ledger changed no executable input. The subsequent
  Steel UI landing changes neither the supervisor code nor the conversation-capture specification touched here, so no
  further runtime run or UI check was warranted.
- Browser, appshot, native desktop, installer and full-workspace runtime suites were skipped: this changes a bounded
  supervisor relaunch decision and its specifications, with no UI or installer change. The focused tests exercise the
  changed transaction and the existing asset contract.

## Review gate

The required gpt-6.1-sol/high reviewer found no issues in the implementation, specifications and final regression
fixture. The initial review missed the invalid fixture premise; the reviewer explicitly corrected that assessment after
the runtime failure and reviewed the strengthened fixture. The executor checked both findings artifacts against the
source. Commit and PR wording passed its fresh-context cold read.
