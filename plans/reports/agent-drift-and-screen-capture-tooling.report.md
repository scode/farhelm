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

### Landing

#### What landed with it

Only this plan landed this round. Since its PRs were based, main gained the Welcome empty-pane change, a planning PR
that queued eight plans for triaged review outcomes, and a TODO entry; none of them touches hook reports, the drift
checks or the screen capture tool, and the rebase applied without conflicts.

Although both PRs are titled `test:`, #5022 also changes what the supervisor writes to a session's hook log: an `acked`
line now ends with a tab and `committed gen=<number>` when the report was saved for that launch, or `ignored` when it
left the saved conversation unchanged. Your plan decision "No release note needed" covers leaving it without a changelog
entry, so the title stays; the custom agent page below covers the part outside Farhelm that reads that line.

#### Fixes made while landing

Both inside [#5022](https://github.com/scode/farhelm/pull/5022/changes); the landing's independent review found no
blocker.

- **The custom agent contract page documented the old hook log line (first should-fix).** It told custom agent authors
  that a healthy report logs `acked conv-123 -`, so a check matching that whole line would fail against a working
  integration. The page's three examples now show the tab and `committed gen=<number>`, and its contract changelog has
  an entry saying what changed and how to match the line; the command line and flags are unchanged. The CLI shows this
  page as `farhelm custom-agent spec`.
- **A per-harness decision sat in shared code (second should-fix).** Which harness's acknowledgment carries the full
  conversation ID as `id=` (only Grok's) was decided in the hook report module; it now lives with the other per-harness
  reporting decisions, as AGENTS.md's code rules ask. It behaves the same.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. The checks below ran on
the plan's PRs rebased onto main as it stood at the start of the landing (`8cd1f7ef`), which applied without conflicts;
every file's change was exactly what the executor pushed until the two landing fixes inside #5022.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `9902ff4b-2ea9-40a8-8f78-35005a410f83`), before the fixes: all 8731 passed, with no runtime `SKIPPED` lines. That
  covers the supervisor's hook report and screen fixture tests, the CLI's hook command and the drift checks' own unit
  tests.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker, before the fixes:
  passed; the first two again after them.
- After the fixes, the supervisor's library tests, `cargo nextest run -p farhelm-supervisor --lib`, on the test worker
  (recorder run `15377036-0207-4fcd-816b-388609e05a81`): all 1645 passed, including the hook report tests that read the
  moved Grok ID.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `bd09b025-5f5c-4522-a249-3e4448a0b1a6`), before the fixes: of its 49 steps, 48 passed and one was skipped by design on
  Linux (paste), including the step whose conversation is reported through the hook.
- The screen capture tool's own tests, `python3 -B scripts/test-capture-agent-screens.py`, locally (recorder run
  `1ffe145e-7547-4e97-ad8b-f5339b9f2fa1`): all 16 passed.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally, before and after the
  fixes: passed.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: this plan's files pass; it fails
  only on over-long lines in the triage outcomes archive, which main has had since an earlier triage record (#4978)
  landed. No PR fixes them yet; a formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused across the fixes: the workspace, desktop clippy and smoke runs, since the fixes move one function without
changing it and edit a docs page. The executor's runs were focused selections on an older base, and everything they
covered ran again above.

Skipped: the live drift tests against real agents and a screen re-capture, which spend vendor turns and need an explicit
request (`docs/harness-drift-tests.md`); the desktop app's own tests, since nothing in it changed; a Mac run; and CI,
which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **The stricter drift checks have not yet run against real agents.** The Codex, Antigravity and Grok checks now need a
  saved report for the resumed launch, and Grok's stricter check applies to every Grok drift case, not only Restart. The
  plan kept live drift runs out of scope, so the first real drift run is what validates these checks; read a Grok
  failure there with this change in mind.
- **SPEC_impl.md under-describes when an acknowledgment is `ignored`.** It names a subordinate Stop and an identity that
  is already saved, but Grok callbacks other than session start, a Stop whose reporting process cannot be checked, an
  Antigravity Stop whose transcript does not verify, and a Codex report that is not resumable yet are also `ignored`. It
  also does not mention Grok's `id=`. The hook troubleshooting page has the user-facing version; a sentence in
  SPEC_impl.md would close the gap.
- **Smaller points the review noticed.** The log reader ignores the conversation ID on `write-failed` lines, and one
  test asserts that as if the line had none; the screen capture tool's recovery survives a stopped process, as
  documented, but not a power loss in the middle of a write, which stops the next run with an error rather than
  corrupting fixtures; the Grok-only `id=` field may be more than the check needs; and one loader error message reads as
  if the test held the version lock that the capture tool holds. None needs a follow-up on its own.
