### What this was about

The terminal's “this drag did not copy” guidance could become a thin, clipped column when the window narrowed while it
was showing. It measured itself at its old position near the window edge. The maintainer delegated a small fix: measure
at the origin before placing the notice, using the same approach as ordinary tooltips.

### Things you should know

Only measurement placement changed. The stylesheet, timer, dismissal and announcement lifecycle are unchanged. The
regression fixture now models width shrinking at a stale position, instead of always reporting one constant size.

A prior executor completed the implementation, tests and source review, then returned the plan because its native
report-reviewer controls were unavailable. This executor verified that work, completed the missing native gates and
prepared the report. No new product change was needed on resume.

### Open questions and possible follow-ups

None. The fix met the recorded complexity gate. The regression uses a layout-dependent JavaScript fixture; no full-app
browser or live desktop resize was run for this plan.

### The PRs

- [#2111: keep terminal copy guidance readable on window resize](https://github.com/scode/farhelm/pull/2111/changes) —
  draft, based on main.

### Checks run, reused and skipped

- Reused the revised notice tests run before the controller fix: 15 passed and two failed, including a measured width of
  one pixel instead of 200. This deliberate failure confirms that the fixture detects stale-position wrapping.
- Reused `node --test` from `crates/farhelm-ui/js-tests` after the fix: all 249 tests passed, no skips. The retained
  output was inspected. It covers the controller and position-dependent fixture at the reviewed revision `4bae2420`; the
  final original PR head `4ea2c9d4` added only the PR link. The resume rebase preserves all product and test bytes.
- Reused the original targeted dprint and changelog checks. Ran the Git diff whitespace check and changelog format lint
  again on resume: passed. The original documentation pass covered the measurement invariant and fixture premise.
- Read every incoming main change from the original base `fe323ffb` to the resumed base `a3e2a045`: only queue tracking
  and reports, with no product, spec or validation-input interaction. Rebased the one-PR stack as required for
  resumption; no repeated runtime check was warranted.
- Skipped Rust and browser builds: the plan requires the position-dependent unit seam and JavaScript harness, and no
  Rust, CSS or event-lifecycle behavior changed. The sleep checker does not inspect this JavaScript unit harness, and no
  Rust or browser test source changed.

### The review gate's outcome

The required fresh-context gpt-6.1-sol high source reviewer found no issues. Its full artifact was read and checked
against the diff on resume; it was a static review and did not independently run tests. A separate gpt-6.1-sol medium
wording cold read passed after clarifying the terminal/window context in the title. A native independent resume check
confirmed the saved implementation and evidence; the report also receives its required native cold read before delivery.
No workhorse implementation was used.
