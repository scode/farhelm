# Accepted specification limits

## What this was about

Three review findings concerned limits the maintainer chose to accept. Farhelm's optional automatic trust for Claude
edits Claude's settings under its advisory lock; a stalled writer can overwrite another writer's update after stale-lock
takeover. An open Settings dialog can keep showing old trust choices after another GUI saves them. Very large trash
collections can exceed listing capacity and leave a host's trash unavailable in the app, including individual cleanup.

The maintainer chose specification changes for all three: state the accepted lock trade-off, make the stale-view rule
for several GUIs explicit, and declare the oversized trash scale out of scope. The three draft PRs implement those
decisions and remove their feedback entries. Each retained triage outcome names its change, bookmark and PR.

## Things you should know

These are documentation changes only. Automatic trust remains opt-in and best effort; both specifications now state the
stale-lock takeover risk beside Claude's unlocked settings save on exit. Claude's actual takeover timing and a realistic
stalled-writer interleaving remain unverified. No stronger locking guarantee is claimed.

The trust paragraph now distinguishes the trust hint above the terminal, which updates through invalidation, from
Settings, which reads current shared values when opened. In the specification's "One GUI at a time" section, an
already-open view may retain a change's older state until reopening or page reload when the change came from another
GUI. Explicit live-update requirements, session takeover and displacement rules, destructive prompts' preconditions, and
correct concurrent behavior alongside the CLI and agents still hold. This rule covers stale display alone, not failures
of those guarantees.

The trash specification now says it is built for collections well below the 10,000-archive cleanup limit with ordinary
path lengths. That count is not a promise that every smaller collection is listable: long paths can exhaust the reply
budget earlier. A host beyond the intended scale may be unlistable; this plan adds no pagination or recovery route.

The stack was rebased onto main `1d8a7519`. All intervening diffs were read: keyboard scrolling changes landed in
separate specification sections and UI regions, along with feedback cleanup and plan bookkeeping. They do not alter
these three limits. The rebase was clean and left all specification changes identical to those reviewed. The only
post-review edits were the required PR URLs in the triage ledger. No runtime recheck was warranted.

## Open questions and possible follow-ups

No decision or required follow-up remains for this plan. The vendor scheduling scenario and an oversized on-disk trash
collection were not reproduced; those remain evidence limits, not claims that this documentation fixes either behavior.

## PRs

- [#2193: accepted limits of editing Claude settings](https://github.com/scode/farhelm/pull/2193/changes).
- [#2195: stale views across GUIs](https://github.com/scode/farhelm/pull/2195/changes).
- [#2196: intended trash scale](https://github.com/scode/farhelm/pull/2196/changes).

## Checks run, reused and skipped

- Run now: `dprint check SPEC.md SPEC_impl.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`,
  `python3 scripts/check-agents-md.py`, and `git diff --check` passed after the rebase. The instruction-reference check
  covers the removed feedback files. Documentation and acceptance-criteria passes covered all touched files.
- Reused: no runtime results. The three source reviews still apply because the rebase left each reviewed patch unchanged
  apart from the required addition of its PR URL to the triage ledger.
- Skipped: Rust, JavaScript, browser, desktop, installer and other runtime tests, builds and CI. The plan changes only
  specification text and feedback bookkeeping, with no executable examples or generated artifacts.

## The review gate's outcome

Each PR passed a separate fresh-context gpt-6.1-sol high source review with no findings. The reviews checked the full
outcome decision and acceptance criteria, specification consistency, preservation of unrelated guarantees, feedback
cleanup and the documentation-only complexity gate. The executor read the findings and checked them against the diffs.
The reviewers ran no tests, and no runtime behavior was verified by these gates. Fresh gpt-6.1-sol medium cold readers
checked commit and PR wording; the first message was rewritten after its initial read. No implementation work was
delegated, and all three complexity gates were met without expanding scope.

### Landing

Landed in round 76, as #2193, #2195 and #2196, merged bottom-up after the round's other plan and unchanged from what the
report describes. All three change only specification text and review bookkeeping.

#### What landed since the stack was based, and whether it interacted

Two code changes reached main between this stack's base and its landing. One is a fix for typing with an input method in
WebKit-based browsers and the desktop app (#2194), which changes keyboard handling and adds its own SPEC_impl.md
section. The other is the round's other plan, the cancelled-download fix (#2197), which changes the helm's download
bookkeeping and its SPEC_impl.md paragraph. I read both against this stack, and a fresh-context reviewer did the same
independently. Neither found an interaction: nothing in this stack says anything about keyboard handling or downloads,
and neither change alters Claude settings edits, settings views or trash listing. #2197 also edited the triage ledger
and the review queue index, which every PR here edits too, but in separate entries. The landing rebase merged them
without conflict and left each PR's diff identical to what the executor delivered.

The reviewer also checked the new specification text against the code:

- The Claude settings statement (#2193) is accurate. Farhelm waits for Claude's settings lock but never takes over a
  stale one. Just before its final rename it gives up if it has held the lock too long or the lock was replaced, but a
  writer stalled after those checks and past Claude's stale-lock threshold can still overwrite a save made in that gap,
  in either direction.
- The stale-views rule (#2195) matches the code: the trust hint above the terminal re-reads on every update, while
  Settings reads once when opened.
- The trash scale statement (#2196) matches the 10,000-archive cleanup limit and the trash listing, which returns a
  host's whole trash in one size-limited reply, so "may be unlistable" past that scale is right.

#### Fixes made during the landing

None.

#### A wording note

SPEC.md now says Farhelm follows Claude's settings lock "the way Claude's own writers do". That is slightly loose:
Claude's own writers are understood to take over a stale lock (the report notes Claude's actual takeover behavior is
unverified), and Farhelm never does. The phrase comes from the principle recorded with the maintainer's triage decision,
so it was left as written. SPEC_impl.md's matching paragraph says Farhelm retries without ever taking over a lock, which
pins the behavior down. If SPEC.md should be exact, a one-clause follow-up would say Farhelm waits on Claude's lock but
never takes over a stale one.

#### Checks

- Run now, at the rebased tip: `dprint check` on SPEC.md, SPEC_impl.md, TRIAGE_OUTCOMES.md and the review queue index,
  and `python3 scripts/check-agents-md.py`, both clean.
- Reused: the executor's three source reviews, which still apply because the landing rebase left each PR's diff
  unchanged.
- Skipped: all runtime tests and builds, since the stack changes only specification text and bookkeeping.

Nothing in the executor's report became untrue at landing.
