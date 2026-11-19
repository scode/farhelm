# Session survival wording during the old-terminal update

## What this was about

Farhelm's usual update message says sessions keep running. The one-time migration that retires old tmux terminals ends
work still running in those terminals. Three review findings flagged the risk of leaving work open based on the usual
reassurance. The decision to keep the usual session-survival message was recorded in the commit that restored it.

## Things you should know

The maintainer chose to describe how the product works in general and record narrow exceptions where they apply. The
specification now states that principle under Documentation scope. In the same specification, the Old terminals bullet
under Terminal experience records the choice to use the usual survival wording during this migration, including the
installer message. The three related triage decisions are now in the archive with the link to their implementation.

## Open questions and possible follow-ups

None.

## PRs

- [#4145](https://github.com/scode/farhelm/pull/4145/changes) documents the wording decision and records the three
  completed triage outcomes.

## Checks

- `dprint check SPEC.md TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and
  `git diff --check` passed at `ccf054e60b9d`.
- `python3 scripts/check-agents-md.py` reported zero errors and one pre-existing warning about an archived citation to
  the “Bound polling time” heading.
- Used fresh source checks. Skipped runtime tests because this change only edits the specification and review
  bookkeeping.

## Review gate

A fresh gpt-6.2-sol reviewer at high effort passed the final draft with no findings. The reviewer checked both additions
to the specification and verified that the three archived entries preserved the original text except for their Execution
fields. A separate cold reader checked the commit and PR wording.
