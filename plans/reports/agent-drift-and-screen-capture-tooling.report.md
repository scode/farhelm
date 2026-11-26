## What this was about

The screen capture tool publishes a terminal screen and its title as two files. An interrupted write could leave files
from different captures, which the supervisor's tests could accept as a pair. The tool now restores the old pair after a
failed write, keeps the validated replacement, and recovers unfinished work before capturing that version again. The
fixture loader refuses interrupted publications and titles without a screen.

The Codex, Antigravity and Grok drift checks could accept a Restart after a new hook acknowledgment even when the
supervisor had kept the old saved conversation untouched. A change in a vendor CLI could then break conversation
reporting after Restart and still pass the check. The maintainer chose to require a write committed for the resumed
launch. The checks now verify the launch number, the original conversation ID and the saved resume command. The hook
troubleshooting page explains how to read the new outcomes.

## Things you should know

The hook log retains its existing outcome words and identity fields. The new acknowledgment suffix says whether the
supervisor saved the report. Grok's original report can contain both a conversation ID and a long transcript path. When
the log shortens that field, the suffix carries the complete Grok ID so the test can compare it to the conversation the
app will reopen.

The manual drift runner normally upgrades each selected vendor CLI and checks its version before running the drift
cases. Today, a child of a timed-out updater or fallback installer can finish replacing the CLI after the version probe.
A drift verdict can then name a different binary from the one the case ran. This delivery leaves that behavior in place
under the complexity rule and updates the TODO with the findings. Stopping the group as soon as the updater exits can
kill a legitimate worker. Waiting with a process-table scan can miss a successor forked during the scan. The reliable
options found in review need more process ownership machinery or a stricter Linux requirement.

## Open questions and possible follow-ups

The updater containment fix remains on TODO. The screen capture and Restart changes are ready for review.

## PRs

- [#5017](https://github.com/scode/farhelm/pull/5017/changes) makes screen and title publication recoverable and records
  the remaining updater issue in TODO.
- [#5022](https://github.com/scode/farhelm/pull/5022/changes) requires a committed hook report from the resumed launch
  in the drift checks.

## Checks run, reused and skipped

- Screen publication passed all 16 stand-in cases (`python3 -B scripts/test-capture-agent-screens.py`, run
  `c138018d-2edf-4236-a469-44b2736cf360`). All 10 supervisor screen fixture tests passed
  (`cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(screen_fixtures)'`, run
  `b8c448e8-d6e3-4405-a926-9285f20527e9` on `48836d70`). The final screen draft keeps the tested Rust loader and
  fixtures intact. The subsequent fixes affect Python publication and diagnostics.
- At `235fbc4e`, the final hook parser and admission selection passed all 32 supervisor tests
  (`cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(hook_report::tests) | test(report_files::tests) | test(claude_pin::)'`,
  run `9a47e27a-d1e7-4422-845d-b052fd4b87f1`). All 75 local drift, Codex and hook-identity integration tests passed
  (`cargo nextest run --locked -p farhelm --test e2e -E 'test(/^(real_agent_capture::drift|codex_identity|hook_identity)::/)'`,
  run `f7dadcba-8fce-4676-bd49-5bb37a6672a0`). The final formatting pass wrapped one assertion without changing what
  ran.
- `cargo fmt --all -- --check`, `dprint check` on changed Markdown, `git diff --check` and
  `python -B scripts/check-test-sleeps.py` passed. `python3 scripts/check-agents-md.py` reports an older archive
  citation warning. The broader workspace suite was skipped because the focused checks cover the changed admission,
  readers and fixture tools. Live vendor drift and screen recapture were out of scope.

## Review gate

The independent source reviews passed after corrections. The publication tests simulate a full-disk failure and an
unreadable journal and prove that recovery remains possible. The hook tests pin a same-identity ignored report and an
actual committed Codex callback. They also cover Grok's maximum-length ID and earlier log lines.
