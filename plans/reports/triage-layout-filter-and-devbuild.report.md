### What this was about

Three recorded review findings awaited the maintainer's agreed triage outcomes. With many trash hosts, the empty-trash
confirmation can open with its host list and permanent-loss explanation above the visible buttons. A long launch error
can similarly put Replace's warning that it kills the source session out of view. Both explanations remain reachable by
scrolling, and the actions require a click. The third finding concerns the Mac development-build helper resetting
another working copy when Git's repository-selecting environment variables are deliberately exported.

The maintainer chose a review filter for the two layout cases and discarded the development-build finding as confined to
maintainer tooling under deliberate environment setup. Each outcome has its own draft PR, removing its finding and index
entry and marking its triage execution complete.

### Things you should know

The filter requires unusually large content or an unusually small window, with the whole consequence limited to an
explanation being out of view but reachable by scrolling. Unreachable content and actions without the user's click
remain outside it. Like the existing filters, it governs recording and triage of already-landed findings; it does not
make the behavior acceptable during implementation or review.

This work changes review bookkeeping only. The dialogs and development-build helper retain their existing behavior; no
product code or specification changes, changelog fragments or release actions are involved.

### Open questions and possible follow-ups

None. Execution found no conflict with the recorded decisions and required no scope departure.

### The PRs

- [#2076](https://github.com/scode/farhelm/pull/2076/changes): the layout filter and trash-confirmation finding.
- [#2079](https://github.com/scode/farhelm/pull/2079/changes): the scrolled Replace-warning finding.
- [#2081](https://github.com/scode/farhelm/pull/2081/changes): the development-build environment finding.

### Checks run, reused and skipped

Ran `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md review_feedback_queue/FILTER.md` successfully,
including after the PR links were added to the ledger. Reviewed each removal's full diff and the combined stack for
scope and ledger/index consistency. No earlier validation was reused. Rust, JavaScript, browser, desktop, installer and
website runtime checks were skipped because these Markdown changes affect no executable examples or generated assets.

### Review gate's outcome

Fresh gpt-6.1-sol reviewers at high effort found no issues: one reviewed the filter and first removal, and a second
reviewed both removal-only PRs together, as the plan permits. Separate gpt-6.1-sol readers at medium effort checked each
commit message and PR wording; the accepted wording matches the decisions and diff. The executor verified the final PR
links and ledger entries. A separate native reader checked this report for standalone clarity and public-repo hygiene.

No unresolved findings or review coverage gaps remain for these bookkeeping changes. Native token counters were
unavailable; private orchestration evidence retains that accounting gap and identifies process launches whose events
were recorded retrospectively. Main advanced only through plan claims and another delivery report while this stack was
prepared; the full incoming diff had no interaction with it, so no rebase or additional runtime validation was needed
for delivery.

### Landing

Landed as #2076, #2079 and #2081, bottom-up, after trash-unchecked-wording (#2080) in the same round. Both plans removed
neighbouring entries from the review feedback index, and main had gained a batch of 50 triage decisions (#2082); every
index conflict was a pair of removals, and all were kept.

#### Review

An independent fresh-context review found nothing blocking. The filter follows the existing filters' shape, states the
agreed generalization without widening it, and does not override the independent check required before dropping a
highest-severity finding; its header still says filters never make behavior acceptable. Each PR removes only its own
finding and index entry and replaces only its own Execution field; nothing else in the repository referred to the three
removed findings.

#### Fix made while landing

Whether hidden text is "reachable by scrolling" was judged only for users who can scroll it. The trash confirmation puts
focus on its submit button under a pinned header, so a keyboard-only user can leave the scrolling area without scrolling
it. The filter's first exclusion now reads "content that cannot be reached at all, or that a keyboard-only or
screen-reader user cannot reach", which only narrows the filter.

#### Left as is

"Nothing is acted on without the user's own click" leaves keyboard confirmation (Enter on a focused button) outside the
filter, which errs on the safe side and keeps the maintainer's word.

#### Checks

- Run now: `dprint check` on the filter after the fix and on the index after each conflict resolution; no conflict
  markers on main after landing.
- Not run: anything else; only Markdown changed.
