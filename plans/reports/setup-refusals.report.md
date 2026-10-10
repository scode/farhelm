## What this was about

Linux setup could write a service that could not start when the Farhelm program's path contained `$`. An edited service
of type `oneshot` could start programs from several installations, while Farhelm read only one start command to decide
which installation owned it. Uninstall's stale install-lock advice also used display formatting for shell arguments;
pasting advice for a path containing dollars or backticks could expand substitutions, and non-ASCII paths could select a
different pathname.

The maintainer decided to refuse dollar-bearing program paths and service types other than `simple`, and to quote
recovery commands safely. All three outcomes were implemented within the agreed complexity gate in one draft PR.

## Things you should know

Both local service renderers reject `$` in Farhelm's program path, with advice to install at a path without it. Remote
host add and update planning carry the same explanation before there is a plan to execute. State-directory arguments and
tmux environment values still support dollars.

The service-type check accepts an absent or empty type as the `simple` default, follows the last assignment, accepts
spaces around `=`, and reads only exact `[Service]` sections. It reaches standalone uninstall, local setup and setup's
legacy `--uninstall`, including dry runs. Refusal on the second service leaves both files intact and sends no mutating
commands to the service manager. Recovery advice asks for a simple unit with one start command, rather than suggesting
that changing the type alone repairs a multi-command service.

Execution found a mistaken planning assumption: local setup and its legacy removal checked only the ownership marker, so
changing the executable reader alone would not make them refuse another service type. The required source review
identified this and a whitespace bypass. Its independent scope assessment approved one shared pure type check at those
existing boundaries. Both issues are fixed; other command-recognition rules and the existing dollar read-back behavior
remain as agreed. Full systemd configuration and drop-in analysis remain outside this work.

Stale-lock commands now preserve literal UTF-8 paths, including spaces, embedded quotes, non-ASCII text, dollars and
backticks. A control-bearing or non-UTF-8 path gets display-only manual recovery advice. Interrupted installs with a
journal still direct the user to the installer for recovery.

All three feedback files and index entries were removed, all three ledger entries reference the same change and PR, and
the changelog fragment is included. No outcome was dropped or deferred. The already-landed specifications were
unchanged.

## Open questions and possible follow-ups

None. The tests use owned fixtures and an injected service manager; they do not demonstrate startup or removal through a
live systemd manager. The existing quoted-path formatter is used unchanged, and no native macOS run was needed for the
shared Unix recovery-advice branch.

## PRs

- [#1817 — reject unsupported service setups and quote lock recovery](https://github.com/scode/farhelm/pull/1817/changes)
  — one draft PR based on main; the executor did not mark it ready or merge it.

## Checks run, reused and skipped

- Initial focused recorded nextest run `0e3cf5b1-7d41-4c21-a111-6ed3c81db25a` passed 119/119 tests on the working diff
  over main `fdd03c9a`, with pinned nextest, four slots and zero retries. It covered unit rendering/readback, local
  setup, uninstall and two provisioning-plan checks. Its source fingerprint begins `8b0ac964`. No selected test emitted
  a runtime skip; selection exclusions are recorded separately. The recovery/uninstall-lock and provisioning-plan
  results are reused because the subsequent changes affect only service-type checking and setup's preflight.
- Final focused recorded nextest run `4accd076-f48e-448b-b793-1af3d49517f1` passed 79/79 unit and setup tests, including
  selected-service ownership, both missing local refusal paths, whitespace overrides/resets and Farhelm's own rendered
  units. It covers the final working diff over main `fdd03c9a`, fingerprint beginning `145e49a3`, with the same
  four-slot, zero-retry policy. No selected test emitted a runtime skip. Both runs retain complete JUnit reports and
  untruncated output. Their service manager is injected; these selections do not exercise a live tmux server.
- `cargo clippy -p farhelm -p farhelm-helm --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings` passed, covering both test-enabled code and the shipped CLI's
  configuration. `cargo fmt --all -- --check`, changed Markdown formatting and changelog format passed. The isolated
  interpreter's `python -B scripts/check-test-sleeps.py` inspected 282 delays with zero unannotated; no delay was added.
- Full workspace runtime, browser, desktop, installer acceptance and release suites were skipped: the changed behavior
  is covered by the focused renderer, reader, setup, planning and recovery-command checks, with source inspection of
  existing error propagation. There is no browser asset or UI change. Hosted CI was not dispatched; draft PR pushes do
  not start CI in this repository.
- Every main change from tested base `fdd03c9a` through `d8d43e77` was read: queue transitions and the dev-tooling
  report only. The careful rebase was conflict-free and preserved executable code, specs and fixtures, so both runtime
  runs and the source review remain applicable. The later ledger URL edit affects bookkeeping only and passed
  formatting.

## Review gate outcome

The required fresh gpt-6.1-sol/high source reviewer found the whitespace and local-preflight gaps, approved the bounded
shared check in its scope assessment, and then returned PASS on the final source. Its charter included the complete
test-authoring checklist and all three ledger contracts. The executor read both reports and checked the runtime evidence
separately; the reviewer did not run tests or verify live systemd behavior.

The second fresh gpt-6.1-sol/medium wording reader understood the problems and found no contradicted claims or
convention violations. Implementation remained local under no-workhorse mode. Actual native model attribution and usage
counters were unavailable; requested routes are recorded as requests. Private evidence remains under session UUID
`babea5b4-d5fc-4824-9a71-faa36bdd96dd` and the plan's working log beside the checkouts.
