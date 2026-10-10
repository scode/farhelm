## What this was about

A host could send hidden or direction-changing characters that disguised a session's status diagnostic or sidebar
folder. A malformed message or refused terminal attachment could also send terminal controls into the helm's logs. The
maintainer accepted these four triage findings as straightforward fixes and authorized one shared PR.

## Things you should know

All four fixes use the existing escaping behavior. Ordinary status wording and folder abbreviation stay the same. Empty
sidebar folders stay blank: the triage ledger explicitly required that, overriding the planner's proposal to show
`(empty)`. Empty or whitespace-only status diagnostics now have a visible placeholder, as the plan intended.

No new helper, formatter, protocol, or log-capture machinery was needed. No outcome tripped its complexity gate. The
four feedback entries were removed and their execution records link the shared PR.

## Open questions and possible follow-ups

None. The raw host name beside the folder was explicitly outside these findings. This work does not claim a broader
audit of all host text.

## PRs

- [#1797: fix: escape host text in session details and logs](https://github.com/scode/farhelm/pull/1797/changes) — one
  commit covering all four outcomes; left as a draft for the plans monitor.

## Checks run, reused and skipped

- Ran focused nextest through the recorder for the status tests, sidebar row tests, and helm's existing
  text-normalization test: 37 passed, 1,425 selected out, no failures or runtime early-return skips. Recorder run
  `545b2eb3-17ff-408c-abb4-5a648fffff1e`; pinned nextest 0.9.143 and tmux, four slots, zero retries. The two changed GUI
  regressions passed.
- Ran `cargo clippy -p farhelm-ui -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`, and `python3 releasing/check-changelog.py format`:
  passed.
- Ran `python -B scripts/check-test-sleeps.py` with the isolated pinned interpreter: 281 delays inspected, zero missing
  rationales.
- Reused the successful runtime and source-review evidence after rebasing from `c229b860` onto `ee306896`: all upstream
  changes were queue claims, with no source or spec interaction. The later test-helper docstring, feedback-index
  correction, and PR-link amendment do not alter tested behavior.
- Skipped browser and broader workspace runtime tests because the changes only route text through existing helpers. The
  sidebar regression observes actual rendered text mutations; pixel layout, clipping, and emitted log output were not
  tested. The two log sites have no new capture test, per the plan's scope; the existing escaping-and-bounding helper
  test passed.

## Review gate outcome

The required fresh source review, requested on gpt-6.1-sol at high effort, found no remaining correctness, design,
idiomaticity, or test-quality issues. It inspected all four sites, their helper contracts, the GUI tests, and
bookkeeping. A wording cold read passed. An incidental finding about wrapped feedback-index continuations was fixed and
the source reviewer verified the correction.

The reviewer performed source inspection; the executor verified the runtime and lint results. Native runtime model
identity and token counters were unavailable. The executor did not mark the PR ready or merge it.
