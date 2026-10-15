## What this was about

When a host starts reporting a different Farhelm installation, the adopt prompt and host mismatch evidence show the
recorded and reported identities for comparison. A host supplying an unusual identity could make two different values
look identical: a space rendered as `<U+0020>` looked like that literal text, and an empty identity looked like the
literal value `(empty)`. This weakened the comparison before adoption; the adoption request still used the raw identity.

The maintainer chose a small code fix. Both displays now use an unambiguous spelling: ASCII letters, digits and hyphens
stay as they are, while everything else is escaped. Escape punctuation and the empty marker can no longer impersonate
another identity. Ordinary supervisor-generated UUIDs look exactly as before.

## Things you should know

Custom identities containing punctuation show more escapes. This changes their display only; stored values and adoption
requests remain unchanged. No protocol, database, authentication or host-admission rule changes.

The regression covers the two reported collision pairs, a punctuation/escape pair, and existing UUID and invisible-text
cases. Both affected surfaces already use the shared renderer, so neither needed new UI wiring.

## Open questions and possible follow-ups

None identified. No scope departure was needed.

## The PRs

- [#2083: distinguish installation identities in adopt prompts](https://github.com/scode/farhelm/pull/2083/changes) —
  one draft PR with the fix, regression, changelog fragment and completed triage bookkeeping.

## Checks run, reused and skipped

- Ran the focused peer-rendering tests through the pinned nextest recorder: 8 passed, 491 selected out, no failures; run
  `77e20d38-92ff-4bea-b27c-39f475dda2d0`. Selection: `cargo nextest run -p farhelm-ui --lib -E 'test(peer::tests::)'`,
  four global slots and zero retries, pinned tmux.
- Ran `cargo clippy --all-targets -- -D warnings`, final `cargo fmt --all -- --check`,
  `python3 releasing/check-changelog.py format`, targeted dprint on the triage ledger and feedback index, and the
  isolated-interpreter `python -B scripts/check-test-sleeps.py`; all passed. The sleep checker inspected 301 delays,
  with zero unexplained. An initial formatting check found one assertion layout, corrected with cargo fmt.
- Reused runtime and source-review evidence across the rebase and final ledger URL update. The entire incoming main diff
  was inspected: queue/report updates, the deliberate-local-setup specification and its feedback cleanup, and a batch of
  new triage decisions. None changes this renderer, its callers, tests or dependencies. The new security text preserves
  the remote-host boundary and does not relax identity comparison. The only later code-file edit corrected a test
  comment about which pairs collided before the fix; assertions and executable code stayed unchanged.
- Skipped broader runtime, browser, desktop and installer checks: the changed behavior is the shared pure renderer,
  exercised directly by the focused tests. Both displays retain their existing escaped-text and direction-isolation
  markup; there is no new interaction or lifecycle behavior needing browser coverage.

## Review gate's outcome

The required fresh-context gpt-6.1-sol review at high effort, through the external harness, found no actionable issues.
It covered the full patch, plan, triage decision, both callers and the full test-authoring checklist. It confirmed that
the encoding distinguishes inputs and keeps UUIDs unchanged. A separate wording cold read matched the intended failure
and found no contradiction. A separate documentation pass corrected the test comment described above. No unresolved
review finding remains.
