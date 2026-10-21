# Helm trust boundary

## What this was about

The helm already checked the session facts and agent requests it receives from supervisors. Those checks still depended
on callers remembering them: storage accepted unchecked session rows, dispatch unwrapped fields that a separate
validator had checked, and any helm code could construct an error claiming a supervisor had chosen its classification.
The aim is to keep a future caller from skipping a check or changing what an error means.

Three refactors make those requirements part of the APIs. Session cache and history writes require an admitted row;
agent dispatch receives parsed required fields; supervisor error attribution is confined to the reply-decoding module.
The inventory covered six supervisor reply shapes carrying session rows, all 14 agent verbs, 80 error constructions, 14
dynamic error consumers in the helm and 47 in integration tests.

## Things you should know

The maintainer required this work to preserve current behavior. A separate bug-fix PR was permitted only for unchecked
supervisor data entering helm state, a missing required agent field getting past validation, or a wrongly attributed
error origin. The inventory found no such current production bug.

All three PRs preserve refusal text and precedence, request bytes and retry identities, ownership checks, cache merges,
transaction boundaries and error classification. Running-agent prompts remain disabled. A supervisor's Unauthorized
refusal still becomes HTTP 502, not the browser's signed-out response.

The session work extends the existing cache-row type. Its admission proves that session references are representable;
the existing ownership and reply-correlation checks remain separate. Error consumers retain the same classified error
type, so consumers that deliberately ignore origin keep their meaning. The plan's TODO entry is removed in the last PR.

## Open questions and possible follow-ups

A protocol comment for the disabled running-agent prompt path still describes empty text as legal, although the existing
helm rejects blank text. This helm-only refactor preserves that behavior and leaves the protocol comment for a separate
correction. There is no decision blocking this work.

## The PRs

- [#2492: checked session rows](https://github.com/scode/farhelm/pull/2492/changes)
- [#2494: parsed agent requests](https://github.com/scode/farhelm/pull/2494/changes)
- [#2495: restricted error attribution](https://github.com/scode/farhelm/pull/2495/changes)

## Checks run, reused and skipped

Final checks passed on the stack rebased onto main at `50cd9011`, with tip `69d6de57`:

- `cargo fmt --all -- --check` and `dprint check TODO.md`.
- `cargo clippy --all-targets -- -D warnings`, including the downstream Rust test callers and the newly landed UI types.

The reused `python -B scripts/check-test-sleeps.py` check ran on the source later committed as `a9dcb571`, using the
isolated pinned parser environment: 307 delays, none missing a rationale. The result remains applicable after the rebase
because the changed tests and their delays were unchanged.

Four focused helm runs through `python3 scripts/record-test-run.py --runner nextest` passed, using nextest 0.9.148, four
slots, zero retries and the pinned tmux substrate:

| Selection                                                           | Passed | Recorder run                           |
| ------------------------------------------------------------------- | -----: | -------------------------------------- |
| Session cache admission, list refusal, history and status retention |     20 | `9b343207-5378-4276-90cf-33341da81e29` |
| Identity-less refresh status retention                              |      1 | `50e4068b-f6c6-4da8-8d75-cf250181a985` |
| Parsed verbs, refusal precedence, dispatch and request digests      |     24 | `d956013b-5a83-47e8-b008-e0cb2502d1e8` |
| Error origin, HTTP classification and headers, upload refusals      |     15 | `40303f98-98ee-47b5-b0c6-255b77cb0afb` |

The first two runs captured main `8fd22481` plus dirty source. The 20-test run preceded a fixture-helper and formatting
cleanup; inspection established that production behavior and fixture values/assertions were unchanged. The one-test run
covered the final first-unit source later committed as `a47f81ae`. The other runs captured `d9aa9bc3` and `a9dcb571`.
All four have complete source capture, JUnit and retained output, with no runtime SKIPPED marker or cleanup limitation.

These runs remain the runtime evidence after the final rebase. The later refactors preserve the earlier covered
behavior, all 31 changed source files remained byte-identical through the rebase, and the landed UI and bookkeeping
changes introduced no new helm/protocol consumer or contract. The only merge conflict combined adjacent TODO deletions;
both were retained. Both reviews of each unit remain applicable for the same reason.

Earlier helm library/test-target compilation and per-unit helm lint checks passed during implementation. The final
workspace lint supersedes their compile coverage. A full Rust runtime suite, browser suite, desktop and installer checks
were skipped: the inventory, narrow contract tests and compiled callers cover the changed boundaries, with no wire, UI,
supervisor or installation behavior changed by this stack. CI was not dispatched.

## Review gate outcome

Each PR passed its two independent fresh gpt-6-astra/high reviews: general adversarial review and strict
no-functional-change review. All six reviews found no actionable issue or observable behavior difference on their first
round. No bug-fix exception or rejected code-review finding was needed. Commit and PR wording passed the required cold
reads.
