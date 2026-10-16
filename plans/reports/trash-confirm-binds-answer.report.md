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
