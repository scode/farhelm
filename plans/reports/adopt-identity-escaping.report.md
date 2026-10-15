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

### Landing

Landed as #2083, the round's only plan. While it landed, main gained a planning batch, two TODO entries and several
plans' claims and report deliveries; none touches how identities are displayed.

#### Review

An independent fresh-context review found one blocker, fixed below, and confirmed the rest: both identity comparisons
(the adopt prompt and the host mismatch, unverified and duplicate evidence) use the shared display; browser tests that
assert identities use only letters, digits and hyphens; and the PR's own bookkeeping is right.

#### Fix made while landing

The report said only custom identities with punctuation would look different. In fact the same display function also
renders the "site" row of the link approval card, whose job is to show the site a link opens. Passing only letters,
digits and hyphens turned every ordinary domain there into escapes, `example.com` into `example<U+002E>com`, on a
security prompt, with no test noticing. The display now passes printable ASCII through except `<` and the parentheses,
which is all the original collisions need: every `<` then starts an escape, and only the empty identity can render as
`(empty)`. Spaces, control characters and non-ASCII lookalikes are still escaped. A test now pins that `example.com`,
`127.0.0.1`, `[::1]` and a host with a port render unchanged, and the doc comment says why. The changelog fragment no
longer says "escape notation".

#### Left as is

- Pre-existing: a hosts test checks the adopt label with the general display rather than the identity display it
  actually uses.
- Pre-existing: the desktop app's refusal when its local supervisor reports another installation puts the raw identities
  in its message.

#### Checks

- Run now, on the rebased change with the fix in: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the
  UI crate's clippy; the peer-rendering and approval-card tests first, run `3ea58698-fd5b-4bd4-af95-15b015c255d0`: 25
  passed; the UI crate in full with the desktop feature, run `17c9b45a-27a2-448d-89c9-ab1c4d6847f6`: 650 passed; the
  destination-authority, m6-5-debts, provisioning and approval-layout specs on Chromium and WebKit, run
  `0bf445c0-ab78-4fab-b04e-7d73d090b475`: 152 passed.
- An earlier run of the same script on the original change was stopped once the review found the blocker; it is not
  counted.
