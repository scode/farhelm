# review_feedback_queue/ rules

This directory is the review feedback queue: findings from code reviews (agent or human) that are real enough to keep
but were not fixed in the review itself. One file per finding, plus an index.

## Files

- `INDEX.md` lists every feedback file with a one-line description of the problem. It must always match the actual files
  in this directory. Updating it is part of every change here, not a follow-up.
- `<mnemonic-name>.md` is one piece of feedback, named briefly after the problem (`disconnect-race.md`,
  `stale-cache-on-rename.md`), never a number or a date.

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
