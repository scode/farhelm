## What this was about

The November 24 triage marked 50 review findings as discarded. They still appeared in the active feedback queue until
their decisions were executed. This plan removes those findings from the queue and archives all 50 original triage
decisions.

## Things you should know

Each archived entry carries the PR link, bookmark and change ID. The changes are confined to the review queue and its
ledgers.

## Open questions and possible follow-ups

None from this plan.

## PRs

- All 50 findings listed in the PR description were removed, and all 50 corresponding outcomes were archived in the same
  change. [PR #4798](https://github.com/scode/farhelm/pull/4798/changes)

## Checks run, reused and skipped

- Compared the 50 names in the plan's Outline against the removed feedback files, removed index bullets, removed
  active-ledger headings and new archive headings. Each set matched, and the index matched the remaining files.
  Historical archive bytes and all graduated fields outside Execution matched the originals. The 50 Execution fields
  contain the verified PR link.
- `dprint check -- TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and `git diff --check`
  passed. `python3 scripts/check-agents-md.py` reported zero errors and one pre-existing warning about a citation to a
  test checklist line in the archive.
- Ran these checks on the final candidate. Skipped runtime suites because the change only moves review records.

## Review

The independent review verified the 50-item scope and preservation of the original records. It flagged wording that
could confuse a group triage choice with the recorded outcome for an individual item. The final entries refer to the
individual outcome. The finding was addressed and the review gate cleared.

### Landing

#### What landed with it

Three plans landed this round, in this order: drift-capture-timing-hooks, drift-workspace-trust, then this one through
[#4798](https://github.com/scode/farhelm/pull/4798/changes). The other two change the live drift tests and the hook log,
and touch none of the review queue files this plan changes, so nothing interacted with it.

#### Fixes made while landing

None.

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

Nothing. The landing's own review checked the bookkeeping: 50 finding files removed from the review queue together with
exactly their 50 index lines, and 50 triage decisions moved intact to the end of the archive, each naming this PR, with
the archive's earlier content untouched.

#### Things you should know that the report does not say

- **One finding still in the queue points at a discarded fix.** It is about a terminal web link split across two screen
  rows, where Farhelm reads the link a second way by joining the rows, and that second reading has no test. It cites a
  fix this triage discarded and asks for two tests, one where the warning stays quiet and one where it fires; the one
  that fires tests exactly the discarded behavior, so it would fail. The finding is part of the pending
  triage-terminal-nov24 plan, whose executor should add only the quiet case. Its index line also still mentions the
  discarded finding.
