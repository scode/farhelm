## What this was about

A second SSH connection-failure message could suggest a wrongly quoted command for starting a remote supervisor. The
maintainer chose to delete that message if real connections could no longer reach it. Execution confirmed that premise:
production SSH connections always provide process diagnostics and take the newer failure-reporting path before this
fallback.

The obsolete message, its host-kind switch and its four private tests are removed. Connections without process evidence
keep their original failure context. Real SSH failures retain the existing diagnosis and exact-or-descriptive start
advice, including the configured program and state directory. The corresponding TODO entry is removed.

## Things you should know

There is no user-visible change and no changelog fragment. The current production failure path already handles missing
supervisors; this removes unreachable advice rather than adding another command-quoting implementation. The typed
distinction between a clean pre-handshake close and an incomplete protocol frame remains in use.

Existing tests cover the system transport attaching diagnostics, exit 75 reporting a missing supervisor, exact displayed
commands or a descriptive fallback, and partial-frame failures retaining their protocol meaning. No additional test was
needed. A documentation pass removed obsolete annotation references and explains why a failure without process evidence
must not guess a start remedy.

## Open questions and possible follow-ups

None from this plan.

## The PR

- [#2706](https://github.com/scode/farhelm/pull/2706/changes): remove unreachable SSH handshake advice. Open draft;
  landing remains the monitor's job.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-helm --lib -E 'test(ssh::) | test(start_remedy) | test(ssh_natural_exit) | test(ssh_status) | test(transport::tests::) | test(long_remote_paths)'`,
  through the recorder with four slots, zero retries and required pinned tmux: 29 passed, no failures. Run
  `01940a3e-14f1-4e33-8965-1f4c7e187e1f`; 1,285 cases were outside the selection.
- `cargo nextest run -p farhelm-proto --lib -E 'test(handshake_)'`, through the recorder with four slots, zero retries
  and no tmux use: five passed, no failures. Run `b981c8fe-4765-4cbe-802c-ac147f88d78f`; 181 cases were outside the
  selection. Both reports are complete, retained output is untruncated, and neither output contains a runtime `SKIPPED`
  marker.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check` and `dprint check TODO.md`:
  passed. The isolated test-sleep checker inspected 315 delays with no missing rationale.

The recorded runs used main `36c8d7b0` plus this plan's working diff. Their functional coverage and Clippy result apply
to final PR commit `b91bbeef18128de9dd147d1e9af84cf23ec828f6`: only a comment example changed after the initial
snapshot, and no executable code changed. Main moved only through another plan's claim, whose full diff was inspected;
no rebase or repeated runtime check was needed.

Browser tests, the full workspace battery, release checks and live-install checks were skipped because this deletes a
production-unreachable branch and changes no UI, transport behavior or release machinery. No instruction file or
Markdown heading changed, so the agent-instruction checker was not needed.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort confirmed the deletion premise and preserved
live-path coverage. Its sole finding was a stale comment referring to the removed annotation. That comment was
corrected, and the same reviewer confirmed no remaining findings. The commit and PR wording also passed a fresh cold
read. There were no rejected findings, scope departures or weakened guarantees.
