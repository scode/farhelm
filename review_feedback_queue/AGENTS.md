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

Every finding goes through the checks below before it is written. Triage removes covered findings anyway (see
Lifecycle), so recording one only to have triage verify and delete it costs a human's time and buys nothing. The checks
move that work to recording time.

- Covered: do not record a finding that is fully covered by behavior SPEC.md or SPEC_impl.md explicitly accepts, by an
  item in TODO.md's `Planned` bucket, by an item already in this queue, or by a decision already recorded for the same
  finding in root `TRIAGE_OUTCOMES.md`. Apply the test triage applies: the trigger, the consequence, and the scope must
  all match. Sharing a subsystem or a keyword is not enough. When only part of a finding is covered, record the
  remainder and name what covers the rest. When a new finding adds substance to an existing queue item, extend that item
  rather than dropping the finding or writing a duplicate.
- Filtered: do not record a finding from an automated review of committed code that fully matches a filter in
  `FILTER.md`. The filters never apply to findings about a change still under review, or to a problem a person reported
  or asked to have fixed.

When it is unclear whether a finding is covered, it is not covered: record it and name the spec section or item that
might cover it, so triage makes the call. When a person asked to have the finding recorded, do not drop it silently;
tell them what covers it and let them decide.

NOTE: A finding whose consequence would put it in the `highest` bucket (security, or loss of user data, credentials,
processes, or other user-owned work; the root `AGENTS.md` defines the buckets) is never dropped on one agent's judgment.
Before dropping it for any of the reasons above, give an independent reviewer with fresh context, one that neither
produced the finding nor proposed the drop, the finding, the proposed basis for dropping it, and the spec, TODO, queue,
ledger, or filter text that basis relies on. Drop the finding only when that reviewer agrees the drop is justified. If
the reviewer disagrees, or its verdict is ambiguous, record the finding. The same applies when it is unclear whether a
finding belongs in the `highest` bucket: treat it as if it does. A wrongly dropped security or data-loss finding is gone
without anyone having looked at it, while a wrongly recorded one costs a few minutes of triage.

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
