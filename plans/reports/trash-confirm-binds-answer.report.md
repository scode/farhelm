## What this was about

The trash dialog asks before permanently deleting a host's archived checkouts or all reachable hosts' archived
checkouts. Its confirming button previously deleted whichever selection was current when the answer ran. If an old
answer survived a change of selection, it could therefore authorize deletion of a different set than the one the user
saw.

The maintainer chose a small fix confined to the dialog. Each answer now carries the confirmation shown by its button.
An answer that no longer matches the pending confirmation does nothing; a matching answer deletes exactly its displayed
targets through the existing deletion guards.

## Things you should know

The missing check was established by reading the code. No browser or desktop event sequence delivering a stale answer
has been reproduced. This change closes that possible path without changing the helm, supervisor, or wire protocol. The
comparison includes the host or global choice, archive IDs, connection incarnation, and displayed totals.

## Open questions and possible follow-ups

None required for this fix. Browser delivery of the hypothetical old event remains unverified.

## The PRs

- [#2171: bind trash answers to their confirmation](https://github.com/scode/farhelm/pull/2171/changes) — the GUI fix,
  regression, changelog fragment, and completed review-feedback bookkeeping; draft, based on main.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, targeted `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and the isolated
  `python -B scripts/check-test-sleeps.py` passed. The sleep checker inspected 302 delays with none missing a rationale.
- Recorded `cargo nextest run -p farhelm-ui --lib -E 'test(list::trash::tests::)'` passed all four selected tests,
  including the new stale-answer regression; 500 tests were outside the selection. Recorder run
  `203d1e50-4cf3-4e3d-9384-9e94ac1ffd53`, four slots, zero retries, no tmux needed for these pure unit tests.
- `cargo check -p farhelm-ui --features desktop` passed, covering the desktop renderer's compilation separately.
- Runtime evidence was collected on `200233cc8577b6ef9c6b996481c72824a2b2bdd3` and reused after adding only the commit
  description, PR link, and rebasing over two queue-claim changes. Product source and test behavior are identical.
- Browser and wider runtime suites were skipped as the plan specifies: the regression checks the exact stale-answer
  decision directly, while a browser sequence delivering the replaced button's event has not been demonstrated. CI was
  not requested; opening this draft does not run CI automatically.

## Review gate outcome

A fresh gpt-6.1-sol reviewer at high effort found no correctness, design, idiomaticity, scope, or test-quality issues.
The full test-authoring checklist was supplied. A separate fresh gpt-6.1-sol reader at medium effort understood the
commit and PR wording without revision. The documentation pass covered the changed helper, callback, and regression.

### Landing

Landed on its own in round 71, as #2171, unchanged from what the report describes.

#### What landed since the PR was based, and whether it interacted

Three documentation changes reached main between this PR's base and its landing, all from the maintainer's plan review:
a clarification of the instructions for reviewing landed plans, two near-term TODO entries, and a SPEC.md sentence
saying that file-path detection in terminal output does not try to support file names containing whitespace. I read them
against this PR, and a fresh-context reviewer did the same independently. Neither found an interaction. The trash code
this PR changes is identical on main to the PR's base, and every earlier trash change, including the unchecked-entry
wording and totals fixes, was already in the base. The rebase was clean.

The reviewer also checked that the fix cannot refuse a legitimate answer. The pending confirmation is only ever replaced
whole, never edited in place, so an answer to an unchanged confirmation always matches. The single-checkout delete,
which asks no confirmation, is unaffected.

#### Fixes made during the landing

None. The trash has already shipped (it appears under v0.26.1 in the changelog), so recording this as a fix there is
right.

#### A possible follow-up

SPEC.md's rule that a destructive confirmation authorizes only what its prompt said (confirmed 2026-10-01) lists only
session actions. The trash section says its confirmations name the hosts, checkout count and size, but does not say the
answer applies only to what was named; this fix implements exactly that. Nothing contradicts, but one clause extending
the rule to permanent trash deletion would make the spec say what the code now does.

#### Checks

The executor ran the trash unit tests, including the new stale-answer regression, but no browser run. A comparison that
is too strict would refuse legitimate trash confirmations, which the unit tests did not exercise and a real dialog would
show. So that needed a browser run before landing.

- Run now, at the rebased tip, after fresh `cargo build` and web release builds: the trash dialog and checkout trash
  browser specs in full, on Chromium and WebKit, through the test recorder with the pinned tmux, one worker and no
  retries: `npx playwright test trash-dialog.spec.ts github-checkouts.spec.ts`. 50 of 50 passed with no skips, run
  `4e655722-0494-485f-bc38-77c45198701a`.
- Reused: the executor's recorded trash unit run `203d1e50-4cf3-4e3d-9384-9e94ac1ffd53`, its clippy, desktop compile,
  formatting, targeted `dprint`, changelog, agent-instruction and test-sleep checks. The rebase crossed only
  documentation, so the code they tested is what landed.
- Skipped: broader Rust, desktop and other browser runs, since nothing outside the trash dialog changed.

As the report says, the stale answer itself was found by reading the code and has not been reproduced in a browser.
Nothing in the executor's report became untrue at landing.
