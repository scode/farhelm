## What this was about

When automatic Claude trust is enabled for managed checkouts, deleting a checkout leaves a cleanup record if Farhelm
cannot remove its trust entry from Claude's settings. Startup retried those records before serving, waiting up to twelve
seconds for Claude's settings lock for each record in each of two passes. Several leftovers could exceed the desktop
app's supervisor startup deadline and prevent the app from opening.

The maintainer decided that startup cleanup should be best effort: try once without waiting, retain the record if the
lock is busy, log it, and show no user-facing notification. That behavior is implemented. Ordinary cleanup after Delete
still has its existing twelve-second contention budget.

## Things you should know

A busy lock can leave an old trust entry until a later supervisor start. Nothing schedules another attempt while that
supervisor stays running. A later unlocked start removes the trust key and clears its cleanup record. The existing
checks against removing trust from a reused checkout path, and ownership through cancelled filesystem work, remain in
place. The change meets the agreed complexity gate through the existing lock budget; it adds no record format or
background retry mechanism.

The regression uses three retired checkouts sharing a held settings lock. It exercises both construction and the second
reload before the supervisor serves, verifies that the settings bytes and cleanup records remain intact, and verifies
their removal on a later unlocked start. Each startup boundary has a five-second test deadline, below one ordinary
lock-wait budget; this is regression evidence, not a general startup-time guarantee. No installed Claude process or
desktop window was used.

## Open questions and possible follow-ups

None required for this plan. No scope expansion or fallback was needed.

## The PRs

- [#2201](https://github.com/scode/farhelm/pull/2201/changes): skip Claude lock waits at startup. Draft head
  `b3f4ddf11e263a2aaa42c8c9fa67e749495227ad`.

## Checks run, reused and skipped

- Run:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(claude_trust) | test(startup_trust) | test(agent_kind::claude::trust::)'`
  through the recorder, run `77a31481-cba1-4269-be49-981f39b9c74f`: all 13 selected tests passed. The 1,122 excluded
  tests were filter skips; there were no runtime early-return skips among the selected tests. Linux, pinned nextest
  0.9.148, pinned tmux 3.7c, four slots, no retries. This covers contention, retained-record recovery, cancellation
  ownership, ordinary post-Delete fencing, launch opt-in, and preservation of unrelated settings. No failed runs
  occurred.
- Run: `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`, both passed, for Rust formatting
  and compilation/lint coverage of changed signatures and callers.
- Run: `dprint check` on the changed Markdown, `python3 releasing/check-changelog.py format`,
  `python -B scripts/check-test-sleeps.py` with the pinned isolated parser environment, and
  `python3 scripts/check-agents-md.py`, all passed. The delay checker inspected 302 delays with no missing rationale.
  These check the changed prose, fragment, test timing annotations and removed feedback-file references.
- Reused after rebasing onto `7eed523f`: all runtime and source-review evidence above. The intervening changes were
  triage landing bookkeeping and the calm outage UI; the latter's final patch was unchanged from the diff inspected
  before picking this plan. It does not interact with startup trust cleanup. Both rebases were conflict-free and left
  this plan's patch unchanged apart from the PR URL in its ledger entry. The final Markdown check was repeated.
- Skipped: browser, desktop runtime, website builds and unrelated Rust suites, because the change is confined to
  supervisor trust cleanup and the focused tests exercise both pre-serving passes. No doctest behavior changed. No
  hosted CI was requested; PR creation starts none in this repository.

## The review gate's outcome

A fresh source reviewer requested as `gpt-6.1-sol` at high effort through `codex exec` read the full patch, unchanged
locking and ownership code, relevant specifications and the test-authoring checklist. It found no actionable issues and
confirmed the complexity gate. That reviewer ran no tests; the executor ran and inspected the checks above. A fresh
native `gpt-6.1-sol` medium cold reader checked the commit and PR wording. Implementation was not delegated.
