## What this was about

Opening or reconnecting a terminal tab while its session stopped, restarted or deleted could pause typing and unrelated
requests for every session on that host for up to two seconds. The maintainer chose to fix this within about 100 changed
lines.

Tab attachment now refuses immediately when the session is busy stopping, restarting or deleting. Other requests can
continue, and the terminal retries using its existing delay. Accepted attachments still keep the session protected until
the tab is resolved and control is transferred.

## Things you should know

The fix, regression test and comments total 99 changed lines, within the gate. The existing retry delay starts at half a
second and grows to 30-second probes, so a quick refusal does not create a retry loop. No process grace period changed.
The queued feedback was removed and its execution recorded. The working log follows the `farhelm-plan-<slug>-log.md`
rule beside the checkouts.

## Open questions and possible follow-ups

None. The plan stayed within the agreed scope and size.

## The PRs

- [#2409 — keep tab attachment from delaying host input](https://github.com/scode/farhelm/pull/2409/changes), draft.

## Checks run, reused and skipped

Run:

- `python3 scripts/record-test-run.py --runner nextest --kind development --selection 'supervisor tab attach, lifecycle claims and tab resolution' --concurrency '4 nextest slots; retries 0' --timeout 600 --tmux required -- cargo nextest run -p farhelm-supervisor --lib -E 'test(tab) | test(lifecycle)'`
  with pinned nextest 0.9.148 and the repository's pinned tmux: 51 passed, 1,117 selected out, no failures or runtime
  substrate skips reported. Recorder run `911232da-8d9d-4581-af61-f25e5357e703`; complete source, report and output
  evidence, with cleanup complete. This includes the updated paused-clock regression: the held session claim produces a
  conflict without advancing time or installing an input route, and becomes available after release.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` passed for the changed crate's production and test
  targets.
- `cargo fmt --all -- --check`,
  `dprint check TRIAGE_OUTCOMES.md releasing/changelog.d/tab-attach-no-wait.md review_feedback_queue/INDEX.md`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py` passed.
- `python -B scripts/check-test-sleeps.py` using the isolated pinned parser environment passed: 304 delays inspected,
  none missing a reason.

Reused for the final PR: the runtime run covered base `3755dab8` plus the final product and test diff. Only queue,
fragment and execution bookkeeping changed afterwards, so its coverage still applies. No failed runtime attempt was
replaced or omitted.

Skipped: browser and UI runtime tests because neither terminal retry policy nor UI code changed; the refusal already
uses the existing delayed reconnect path, which the independent reviewer checked. Full workspace tests and workspace
Clippy were not needed for this supervisor-only admission change; the focused tests and crate lint cover its behavior
and configuration change.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in correctness, design, idiom,
test quality or the size gate, and checked the final bookkeeping. The PR message passed its cold read after clarifying
the explanation. A separate cold read found the report sufficient for a decision and free of public-repository hygiene
issues.
