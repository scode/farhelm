## What this was about

When an agent reports its conversation late or stops reporting it, Restart is unavailable and Farhelm cannot reopen that
conversation. A vendor update can also expose hook output in the terminal. The plan called for testing these boundaries
with installed CLIs, including separate runs with hooks off. The new `capture_timing_hooks` category covers the eight
agents that report conversations. The only production change is to the hook log.

## Things you should know

Claude and Goose were resumable before input. Codex, Muse, Grok and Antigravity lacked a Resume offer at the idle
composer and were resumable after submission. Pi and OMP reported pending identities at startup. Restart stayed
unavailable at the idle composer and became available after a turn, with a matching turn-end report. OMP's live file
included a 256-byte title record, which Farhelm's parser accepted.

Claude, Codex, Muse, Grok and Antigravity sent `Stop`. Grok also sent `UserPromptSubmit`. The hook log now names
selected successful events so each remains visible in diagnostics when callbacks replace the same report. The expected
hook output markers were absent from the retained terminal text, including Antigravity's empty JSON response. The
hooks-off launches replied, the supervisor sampled them as settled, and no callback or capture appeared during
observation. Goose received the saved conversation ID on Restart with its reporter still off. Codex still received
`--no-daemon`.

## Open questions and possible follow-ups

No open follow-up from the live checks. An earlier Antigravity run paused at a file-access prompt. The text prompt now
explicitly forbids tools, and both modes passed on rerun. The initial Grok, Pi and OMP check counted pending identities
before input. The revised check measures the actual Restart offer, and all three passed. The Goose fixture now declines
the vendor's offer to switch back to the previous disposable directory. The Stop unit test now supplies its log
destination.

## The PRs

- Check when live agent conversations become resumable. [#4797](https://github.com/scode/farhelm/pull/4797/changes)

## Checks run, reused and skipped

- Compiled the final rebased source and listed all eight cases with
  `python3 -B scripts/run-harness-drift.py --list --category capture_timing_hooks`. The final rebase kept both module
  registrations after main added independent launch-prompt coverage.
- Ran on `cd44c9d1`: format, Markdown, test-delay checker, `cargo clippy -p farhelm --all-targets -- -D warnings`, and
  45 recorded Rust tests selected by
  `test(/^real_agent_capture::drift::capture_timing_hooks::/) | test(/^hook::tests::/)` in the farhelm binary and e2e
  targets (`c3be6574-db34-474c-9b35-99b436d7c2e3`).
- The final prompt was tested with
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category capture_timing_hooks --harness antigravity --harness claude`.
  Antigravity 1.5.16 (`031d47a1-739b-4691-a423-83359324c6ac`) and Claude 2.1.332
  (`ccf3a387-6ca1-479f-bdbe-a012ce546fbe`) passed. Claude was rerun because main added CLI updater suppression for drift
  runs.
- Reused from `34a928e6`: 61 recorded Rust tests with `test(/^real_agent_capture::drift::/) | test(/^hook::tests::/)`
  (`8a7e6ac6-f7f0-4c9c-a8e3-15b6870bcef4`). The full live category ran without upgrades. Six earlier live passes apply
  to the same launch, capture and callback checks before the prompt gained its explicit no-tools instruction: Codex
  0.172.0 (`221daad4-f70a-4dab-97db-f5fc5b83fae9`), Goose 1.60.2 (`d800bc44-c929-4e25-a92c-4cac8c0a2715`), Grok 1.0.92
  (`3eea4ed3-6c6c-4c31-b535-a6844151eeab`), Muse 1.4.25 (`471329a0-f409-4ac8-8045-4c2da804ed4b`), OMP 19.1.1
  (`5bf3ec0e-2a4f-4c16-918b-a57c0a5610fe`), and Pi 1.6.0 (`5059f239-2263-491a-8e00-6c48ea43cc99`).
- Reused 18 passing Python runner tests from `3cee496b`, recorded with `python3 -B scripts/test-run-harness-drift.py`
  (`356e41a6-63c2-4921-b22f-8ec52105737e`). The runner did not change.
- Retained the earlier Goose directory confirmation (`ed60f4b0-9628-4870-9e9b-58b763d77bd6`), Grok, OMP and Pi idle
  checks (`92bd577c-6f18-4fb3-8a90-a77d89639f26`, `6c684f5a-f9c3-4c2c-8da9-d98061f39510`,
  `3cb838bf-91ab-4a5a-96a2-ed4837215b97`), Stop fixture log-path failure (`e6bee496-6bfe-448a-947c-890f85d229ac`), and
  Antigravity file-access prompt (`6a4bc51a-defc-490e-be0b-4146ef071e2f`).
- Skipped the full workspace suite and screenshots because the change is confined to these tests and hook diagnostics.
  The plan called for checking the installed CLI versions without upgrading them.

## Review gate

One independent `gpt-6.2-sol` high-effort source review. Its findings were addressed with fresh callback evidence tied
to the captured conversation, retained terminal text checks, a Goose Restart check against the actual process, a settled
hooks-off turn, and a width check on the same OMP bytes sent to the parser. The commit and PR wording passed a separate
cold read.

### Landing

#### What landed with it

Three plans landed this round, in this order: this one, through
[#4797](https://github.com/scode/farhelm/pull/4797/changes) and the landing fix
[#4806](https://github.com/scode/farhelm/pull/4806/changes); then drift-workspace-trust; then triage-bookkeeping-nov24,
which only moves review queue records. The two drift plans were written before the previous rounds' drift plans landed
and were rebased onto them; they overlapped only in TODO.md, where each plan's planned entry was removed and the rest
kept. This plan renamed a helper of the drift tests' private configuration copy that the workspace trust plan's new
tests still called by its old name, so the stacked tests did not compile; that was fixed in the workspace trust plan's
own change while landing.

#### Fixes made while landing

#4806 changes the drift tests, their docs, a comment and the hook's code without changing what the hook does:

- **The hook log change was undocumented.** This plan made the agent hook add a tab, `event=` and the name of the
  callback that wrote the report to its `written` lines in a session's hook log, when the agent named it (`Stop` at the
  end of a turn, Grok's `UserPromptSubmit`, Antigravity's `SessionStart`). It is the plan's only change to what Farhelm
  ships. The website's "Agent hook injection" page, which documents that log line by line, still gave the old shape; it
  now describes the field. A changelog fragment records the change, and the supervisor's comment that the line's shape
  has one definition now mentions the field.
- **Per-agent choices now name every agent.** The hook decided which agent's session start gets the field by naming
  Antigravity beside a catch-all branch, and the category picked its agent-specific checks by comparing names. The
  repository's code rules forbid both, because an agent added later silently gets the default. Each now lists every
  agent, with the same answer as before for each.
- **The drift docs paragraph moved.** In the drift tests' docs, this category's paragraph sat between the model
  selection paragraph and the Codex note that belongs to model selection, so that note read as part of capture timing.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. This round landed three
plans: drift-capture-timing-hooks, drift-workspace-trust and triage-bookkeeping-nov24. The two drift plans change the
opt-in live drift tests, and the first also changes one shipped thing, the line the agent hook writes to a session's
hook log. The third changes only the review queue's records. So the landing ran the hook's tests and every test that
reads the hook log, the drift tests that need no vendor CLI, and the checks that inspect the changed files, on the three
plans stacked together with their landing fixes.

On that stack after the first round of landing fixes:

- `cargo nextest run --locked -p farhelm --bin farhelm --test e2e` filtered to the hook's own tests (`hook::`) and every
  end-to-end module that reads a hook log (`real_agent_capture::`, `restart_with_resume::`, `session_rename::`,
  `codex_identity::`, `wrapper_launch::` and `hook_identity::`), on the test worker (recorder run
  `b46ee0ca-5eaf-4047-bf2a-906a29c4f60f`): all 151 passed, with no runtime skip.
- `cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(/^hook_report::/) | test(/report_files::/)'`, on the
  test worker (recorder run `fecd7225-24e2-463e-b784-ae1654b6106c`): all 13 passed.
- `python3 -B scripts/run-harness-drift.py --list --category all`, on the test worker: it listed every category's cases,
  the two new ones included, starting no agent.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` (the shipped binary's
  configuration), on the test worker: passed.
- `python3 -B scripts/test-run-harness-drift.py`, locally (recorder run `142456f4-bd75-4627-86da-551e939ee1a0`): all 18
  passed.
- The website build (`bun run build` in `website`, for the hook page), `python3 releasing/check-changelog.py format`,
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 scripts/check-agents-md.py` (no errors; one warning, about an old citation in the triage archive, already on
  main), locally: passed.

A second and third round of landing fixes then changed doc comments, the hook log paragraph on the website, the
changelog fragment and the drift docs, and added one drift unit test (the Claude backups exclusion). On the final stack:
`cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'` on the test worker
(recorder run `cca8783c-f6fd-4b61-82a7-aaedc25ea883`, all 30 passed, the new test included), both clippy runs on the
test worker, and locally the formatter, the sleep check, `dprint check`, `check-agents-md`, the changelog check and the
website build: passed. The hook's tests and hook-log readers above are reused from the first stack, since these passes
did not change what the hook does.

Reused: the executors' live runs of the two new categories against the installed agents, recorded in the reports. Both
plans were rebased onto a main that already turned off Claude's background updater, and both re-ran their Claude cases
live after that. The landing fixes change how the per-agent checks are written, not what any agent is launched with,
except that Claude's backups directory in the trust case's private copy now starts empty, which has not run live.

Skipped: the live drift cases themselves, since they spend vendor turns, need the vendor logins and are opt-in by
design; the rest of the workspace Rust suite and the desktop checks, since the only shipped change is the hook log line,
which the hook's tests and every hook-log reader above cover; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing. The landing's own review found the report accurate.

#### Things you should know that the report does not say

- **The hook log line changed for anyone who reads it.** A `written` line can now end with a tab and an `event=` field.
  Nothing in Farhelm reads that part of the line, but a script that splits the line by position sees one more field. The
  changelog fragment says so.
