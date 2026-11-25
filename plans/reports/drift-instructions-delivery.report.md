## What this was about

Farhelm tells supported agents which command handles `$farhelm`. Disabling the hint should still let Farhelm capture the
agent's conversation for Restart. The drift suite already checked conversation capture and Restart. It now also checks
whether the hint reaches the model on installed agent CLIs.

The new `instructions_delivery` category asks the same question in two fresh sessions, one with the hint enabled and one
with it disabled. The enabled reply must name `farhelm agent instructions`. The disabled reply must say no command was
given. Both sessions must produce an accepted conversation report and a Resume offer.

## Things you should know

The live check reads the agent's terminal reply. The prompt asks the model to use its existing instructions, without
calling tools, and spells out the reply markers so a prompt echo cannot supply either complete answer. The matcher
accepts wrapping and quoting around the answer. It still checks the full reply payload.

The full no-upgrade run at `859cad7a` had the following results.

| Harness     | Installed version | Both settings                                                                                                  | Recorder run                           |
| ----------- | ----------------- | -------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| Claude Code | 2.1.332           | Passed                                                                                                         | `d1b84771-2f82-4433-9d82-6cd6e92e6d71` |
| Codex       | 0.172.0           | Passed                                                                                                         | `d1f7dade-d9c1-4b24-b3fd-bc1cd684fb98` |
| Goose       | 1.60.2            | Off passed. On printed the correct command with a period after the closing marker, which the matcher rejected. | `94beb6fc-b33b-4ccc-9b2f-5787ab9b0d8d` |
| Muse Code   | 1.4.25-R9361.1    | Passed                                                                                                         | `0bf8b6f6-e2f3-4197-99b6-9ea0f01949e8` |
| OMP         | 19.1.1            | Passed                                                                                                         | `ec5dd496-dc45-4ba9-8bdb-2f48df185bb6` |
| Pi          | 1.6.0             | Passed                                                                                                         | `9794660b-21db-4358-b387-8af374432890` |

The matcher now accepts one final period after the closing marker. Goose 1.60.2 passed both settings with that change.

## Open questions and possible follow-ups

No harness follow-up was deferred.

## PRs

- [#4804](https://github.com/scode/farhelm/pull/4804/changes) adds the opt-in category and removes the covered TODO
  entry.

## Checks run, reused and skipped

- At `b103b79c`, `cargo nextest run -p farhelm --test e2e -E 'test(/^real_agent_capture::drift::/)'` passed all 25
  ordinary drift tests, including the inventory and reply matcher. Recorder run `c95bb628-c9b5-4c0a-b6fb-ccb1e652918f`.
- At `b103b79c`, `python3 -B scripts/run-harness-drift.py --no-upgrade --category instructions_delivery --harness goose`
  passed both Goose launches (recorder run `f24231e7-3620-4949-9823-d922b6639734`).
- Reused the five live passes from
  `python3 -B scripts/run-harness-drift.py --no-upgrade --category instructions_delivery` at `859cad7a`. The delta
  accepts one period after the closing marker. It leaves the input, expected reply, startup settings and
  conversation-capture requirements intact. The same revision listed all six cases with
  `python3 -B scripts/run-harness-drift.py --list --category instructions_delivery` and passed
  `python3 -B scripts/test-run-harness-drift.py` (18 tests, recorder run `1769a415-6d98-4103-9a07-44b6433046ff`). The
  registration and runner have the same contents in the draft.
- `cargo clippy -p farhelm --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check TODO.md docs/harness-drift-tests.md`, `python -B scripts/check-test-sleeps.py` and `git diff --check`
  passed at `b103b79c`.
- Skipped the workspace and release batteries. The diff changes the opt-in test and its test-only helpers. The scoped
  drift checks exercise the affected code.

## Review gate

A fresh-context Sol high reviewer checked the source and cleared the final change. The review found that partial replies
and wrapped prompts could confuse the first matcher. The final reply markers require a complete answer and keep the
prompt echo out of the match. A scope review supported reusing the existing conversation-capture waiter. A cold reader
reviewed the commit title.

### Landing

#### What landed with it

Two plans landed this round: this one, through [#4804](https://github.com/scode/farhelm/pull/4804/changes), then
triage-spec-text-nov24, which changes the specs, docs and comments for a batch of triage decisions and does not touch
the drift tests. Since this plan was based, main gained the previous rounds' drift categories, among them the capture
timing category, which had added its own variant of the drift tests' shared launch helper (to start a supervisor with
hooks off) where this plan added one to start a supervisor with its startup instructions on or off. The rebase
conflicted there, and I merged the two into one helper that takes the supervisor's settings, with the two variants as
thin wrappers, so each category gets exactly what it had. The drift docs and the list of categories only needed both
sides kept.

#### Fixes made while landing

None beyond that conflict resolution.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. This round landed two
plans, drift-instructions-delivery and triage-spec-text-nov24. Both reports' checks ran before their stacks were rebased
onto the drift and triage changes that landed meanwhile, so the landing ran its checks on the two plans stacked together
on main, with the rebase's conflict resolutions in them (`140e156e`):

- `cargo nextest run --locked -p farhelm --test e2e` filtered to `real_agent_capture::drift::` and `hook_identity::`, on
  the test worker (recorder run `ea45d013-8e11-4428-86d3-84882969acf9`): all 67 passed, with no runtime skip. These are
  the drift tests that need no vendor CLI and every hook identity test, which includes the two that check the rewritten
  conversation notices.
- `cargo nextest run --locked -p farhelm-supervisor --lib` filtered to `core::vendor::`, `claude_pin` and
  `notifications::`, on the test worker (recorder run `34e4e046-fe01-45e1-b5b3-85f228cd1827`): all 17 passed. The notice
  texts live in those modules.
- `python3 -B scripts/run-harness-drift.py --list --category all`, on the test worker: it listed every category's cases,
  the six new instructions delivery cases included, starting no agent.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 releasing/generate-workflow.py --check` (the release workflow's source file changed), the website build
  (`bun run build` in `website`), `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check`,
  `python3 scripts/check-agents-md.py` (no errors; one warning, about an old citation in the triage archive, already on
  main) and `python3 -B scripts/test-run-harness-drift.py` (recorder run `1d1aca37-53b3-415f-9350-5c808cf57091`, all 18
  passed), locally: passed.

Reused: the executor's live run of the instructions delivery category against the installed agents, recorded in its
report; the rebase changed how the category reaches the shared launch helper, not what it launches.

Skipped: the live drift cases themselves, since they spend vendor turns, need the vendor logins and are opt-in by
design; the rest of the workspace Rust suite and the desktop checks, since the only behavior change in either plan is
the text of two notices, which the tests above cover; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing. The checks the report lists ran before the rebase; the ones above ran after it.

#### Things you should know that the report does not say

- **The category's coverage check lists its six agents by name**, as the plan asked. The other categories derive theirs
  from what each agent supports, so an agent that later starts getting the `$farhelm` hint would not make this one fail
  until someone adds it.
