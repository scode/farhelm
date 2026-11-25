## What this was about

An agent can stop at a trust prompt even when Farhelm is configured to pre-trust the workspace. Older drift checks could
answer the question and still pass. The maintainer asked for checks that leave the dialogs unanswered, cover both
explicit choices on Codex and Pi, and put Claude's config in a private copy for Farhelm's trust edit.

The new `workspace_trust` category launches fresh directories in the installed CLIs and waits for the composer without
approving anything. Codex's explicit untrusted launch must show restricted folder access. Pi must load an inert project
extension when approved and explicitly say that it ignored project resources when approval is disabled.

## Things you should know

Claude and Codex passed on the final revision. The earlier five-agent run supplied passing evidence for Antigravity,
Muse and Pi. Antigravity and Muse opened their composers without a trust dialog. Codex accepted the expected
workspace-specific argument in both cases, reached its composer when trusted and offered “Open restricted” for the
explicit untrusted launch. Pi loaded the extension only when approved and displayed its ignored-resources warning for
the other launch. The focused rerun covered Claude and Codex after the patch moved Pi's approval-question check earlier.

Claude's focused run reached the composer, found the new workspace's trust entry in the private config and verified the
real config's bytes were unchanged. The fixture preserves the vendor login, copies its trust files into the test
directory and directs the launch there before Farhelm resolves and edits the file.

## Open questions and possible follow-ups

The first full run found the expected private Claude trust entry but failed its byte comparison on the real config
(`2804bb90-4132-4303-9e31-77d9baaa3f00`). A separate check did not find the failed test's workspace entry in the real
config. The cause of the change is unknown. The focused rerun passed the same strict comparison, and the original
failure remains retained.

## PRs

- [#4794](https://github.com/scode/farhelm/pull/4794/changes) adds the workspace-trust category and the isolated Claude
  trust fixture.

## Checks run, reused and skipped

- Through the recorder, `cargo nextest run --locked -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'`
  passed 24 tests at `ec497613` (`744f3eb0-d67c-46c6-9289-54b987d2c401`).
- The recorder captured 18 passing Python runner checks at `36e4eea2` (`e993305a-72c3-43f7-a78a-6772247aba6e`) using
  `python3 -B scripts/test-run-harness-drift.py`. No subsequent change altered its runner or the five registrations.
  `python3 -B scripts/run-harness-drift.py --list --category workspace_trust` discovered all five at `4fb381ea`.
- The recorder captured two passing production launch-shim checks with disposable Claude configs at `e05bd3f9`
  (`f968f11c-b402-4bfd-b7cd-c2a32fa5c67b`), using
  `cargo nextest run --locked -p farhelm-supervisor --lib -E 'test(/plain_claude_launch_/)'`. The launch shim did not
  change.
- `python3 -B scripts/run-harness-drift.py --no-upgrade --category workspace_trust` ran all five on `4fb381ea`. The
  subsequent run of that command with `--harness claude --harness codex` passed on `ec497613`. Antigravity, Muse and Pi
  reuse the earlier evidence. Antigravity and Muse keep the same composer and question cues. Pi showed the same cues and
  passed the extension and refusal checks. The final patch checks its approval question earlier.

| CLI         | Installed version | Passing recorder run                   |
| ----------- | ----------------- | -------------------------------------- |
| Antigravity | 1.5.16            | `c3a6511c-485b-4c41-944e-e83e729d1ecb` |
| Claude      | 2.1.332           | `4b5bb20b-d6e4-41b9-88bf-3ffb96fc58df` |
| Codex       | 0.172.0           | `1a78a125-b36a-4d94-8ef6-fa7f580b8074` |
| Muse        | 1.4.25-R9361.1    | `8fd70e15-9db0-4983-90cc-4aa90572de53` |
| Pi          | 1.6.0             | `14b074cc-b9a1-43db-ab15-333574dc153c` |

- `cargo clippy --locked -p farhelm --all-targets -- -D warnings` and `cargo fmt --all -- --check` passed on `ec497613`,
  the draft's tree. Markdown, test-delay and instruction link checks passed on `4fb381ea`. The later edits only move
  startup checks and reuse the shared polling pace. Instruction checks reported one pre-existing archive heading
  warning.
- Vendor upgrades, model turns, the full workspace suite and desktop tests were skipped. The trust behavior is
  observable at startup, and the plan changes only opt-in drift checks and their TODO entry.

## Review gate

The independent Sol review caught paths that could have redirected Claude's trust edit or copied a protected file back
through an alias. These were repaired and verified with disposable files before launching Claude. It also identified
Codex's early composer cue and Pi's missing refusal evidence. The final review examined the updated behavior and found
no remaining code findings. The scope review and draft-title cold read passed.

### Landing

#### What landed with it

Three plans landed this round, in this order: drift-capture-timing-hooks; then this one, through
[#4794](https://github.com/scode/farhelm/pull/4794/changes) and the landing fix
[#4807](https://github.com/scode/farhelm/pull/4807/changes); then triage-bookkeeping-nov24, which only moves review
queue records. The capture timing plan renamed a helper of the drift tests' private configuration copy that this plan's
new tests still called by its old name, so the stacked tests did not compile; the two calls were changed to the new
helper, which makes the same checks, in this plan's own change while landing. Both drift plans were rebased onto the
previous rounds' drift plans and overlapped only in TODO.md, where each plan's planned entry was removed.

#### Fixes made while landing

#4807 changes only the drift tests and their docs:

- **The Claude case wrote into your real Claude backups.** The case starts Claude in a private copy of your home, so
  that the trust Farhelm records lands in copies of Claude's configuration that are discarded afterwards. Claude's
  `backups` directory beside that configuration was still a link to your real one, so Claude wrote snapshots of the
  private configuration, with the test directory marked trusted, into your real backups, and could prune your real ones
  there. It now starts empty in the copy and is discarded with it, and a new test checks that. The executor's live runs
  of the Claude case before this fix ran with the link, so your real Claude backups may hold snapshots from them that
  mark test directories as trusted, and may have lost old snapshots to Claude's pruning; the landing did not look at
  your real files.
- **Per-agent choices now name every agent.** The category chose which agents also get a launch with trust explicitly
  off, and how each shows the trust took effect, by comparing names, which the repository's code rules forbid because an
  agent added later would silently skip the checks. Each now lists every agent.
- **The drift docs now describe the category and Claude's private copy.** Neither was described before.

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

- **The Claude case will likely fail at random on a machine where you also use Claude.** It requires your real
  `~/.claude.json` to be byte for byte the same before and after the case. A Claude session of your own running at the
  same time can rewrite that file while the case runs (the review saw it rewritten every few minutes on this machine),
  which then fails the case with a message saying your real configuration changed while it ran, though the test did not
  change it. That is very likely the unexplained failure the report describes from the first full run. The plan asked
  for that exact comparison, so it is unchanged; a looser check (comparing only the test directory's trust entry, say)
  would avoid it. No TODO entry records this yet.
