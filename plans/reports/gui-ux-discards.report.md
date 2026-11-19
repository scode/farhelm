## What this was about

The 2026-11-18 GUI UX triage left fifty approved discards in the review queue. The maintainer chose one PR to carry out
all fifty decisions. The PR removes those entries from the queue and records each decision in the archive.

## Things you should know

Two approved discards covered the same temporary allocation while formatting a launch failure. The review found no
measured slowdown when Farhelm displayed the failure.

[#3209](https://github.com/scode/farhelm/pull/3209/changes) lowered peak memory when restoring terminal screens and
strengthened the main regression test. The remaining concern was in a separate snapshot test. That test can miss a short
memory spike if the process has already reached a higher peak. Triage found no instance of a regression passing for that
reason and approved discarding the remaining concern.

## Open questions and possible follow-ups

None.

## Pull requests

- Carry out the fifty approved discards. [#4203](https://github.com/scode/farhelm/pull/4203/changes)

## Checks

- `dprint check TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and Git's whitespace check
  passed.
- Compared the sorted queue index with the feedback files. All 796 surviving entries matched.
- `python3 scripts/check-agents-md.py` passed with existing citation warnings.
- Skipped runtime tests because the change is review-queue bookkeeping.

## Review gate

The independent review confirmed that only the fifty approved entries moved and every other queue entry stayed in place.
A cold read checked the commit title.
