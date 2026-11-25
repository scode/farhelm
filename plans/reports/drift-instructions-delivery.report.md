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
