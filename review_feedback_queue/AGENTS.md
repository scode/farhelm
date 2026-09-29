# review_feedback_queue/ rules

This directory is the review feedback queue: findings from code reviews (agent or human) that are real enough to keep
but were not fixed in the review itself. One file per finding, plus an index.

## Files

- `INDEX.md` lists every feedback file with a one-line description of the problem. It must always match the actual files
  in this directory. Updating it is part of every change here, not a follow-up.
- `<mnemonic-name>.md` is one piece of feedback, named briefly after the problem (`disconnect-race.md`,
  `stale-cache-on-rename.md`), never a number or a date.
- `FILTER.md` holds the filters that keep low-value findings from automated review of committed code out of the queue
  and out of triage. It is not a feedback file and is not listed in `INDEX.md`. Its rules decide only what is worth a
  human's triage time; they are not product or coding rules, and nothing outside this directory's recording and triage
  work should apply them.

## Before recording a finding

Check each finding from an automated review of committed code against `FILTER.md`, and do not record one that fully
matches a filter there. The filters never apply to findings about a change still under review, or to a problem a person
reported or asked to have fixed.

## What goes in a feedback file

Every file has three things:

- Reviewed commit: the full hash of the commit that was reviewed — the code the feedback is about.
- TLDR: the impact in user-facing terms, readable by someone with no knowledge of the code. What goes wrong for the
  user, not which function is at fault.
- Details: everything a future agent needs to judge whether the feedback is correct and to fix it. Be specific: file
  paths with line numbers against the reviewed commit, what the code does wrong, and — when you know it — how to verify
  the problem and what a fix looks like.

New files follow this template:

```markdown
# <short title>

Reviewed commit: <full commit hash>

## TLDR

<what goes wrong for the user, in plain terms>

## Details

<agent-facing: exact code references, why it is wrong, how to verify, what a fix looks like>
```

## Lifecycle

Feedback leaves the queue the same way it arrived: explicitly. A change, commit, or PR that addresses an item must
remove the item's file (and its `INDEX.md` line) in the same commit, change, or PR — or narrow the file to what remains,
if it only partially addresses it. A fix that leaves its feedback item behind is incomplete.

The root `AGENTS.md` defines the separate "triage review feedback" and "execute triage outcomes" steps. Triage records
the user's decision in root `TRIAGE_OUTCOMES.md` without changing the feedback file or index. Execution applies that
decision and removes or narrows the item in its own draft PR. A `discard` outcome means only "not worth the human's time
at this time", not that the feedback is wrong; its execution removes the file and index entry without a spec or code
change. The outcome ledger lives outside this directory and is not a feedback file to add to `INDEX.md`.

Exception: when triage verifies that a finding is fully covered by an accepted specification rule, an existing `Planned`
item in TODO.md, or a filter in `FILTER.md`, record the basis in the ledger and remove the feedback file and index entry
immediately. These items must not remain in the queue to be skipped repeatedly. Completing this queue cleanup does not
mean the planned implementation is complete. A filter match likewise says nothing about whether the behavior is
acceptable in code; it only means the finding is not worth triage.
