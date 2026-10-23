# Recording review findings

This file covers what every finding goes through before it is written to the queue, how to run an agent review whose
findings feed the queue (including what each reviewer is told), and what a feedback file contains. It belongs to the
"Recording a finding", "Commissioning an agent review that feeds the queue", "Reviewer delegate prompt" and "Trailing
code review" reading lists in `review_feedback_queue/AGENTS.md`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Before recording a finding

Every finding goes through the checks in `review_feedback_queue/.agents/recording.md`, "Before recording a finding",
before it is written. Triage removes covered findings anyway (see `review_feedback_queue/AGENTS.md`, "Lifecycle"), so
recording one only to have triage verify and delete it costs a human's time and buys nothing. The checks move that work
to recording time.

- Covered: do not record a finding that is fully covered by behavior SPEC.md or SPEC_impl.md explicitly accepts, by an
  item in TODO.md's `Planned` bucket, by a plan in `plans/queue/` that has not landed and whose stated work removes the
  finding's cause, by a known bug root `BUGS.md` records as one we have decided to live with, by an item already in this
  queue, or by a decision already recorded for the same finding in root `TRIAGE_OUTCOMES.md`. Apply the test triage
  applies: the trigger, the consequence, and the scope must all match. Sharing a subsystem or a keyword is not enough.
  When only part of a finding is covered, record the remainder and name what covers the rest. When a new finding adds
  substance to an existing queue item, extend that item rather than dropping the finding or writing a duplicate.
- Filtered: do not record a finding from an automated review of committed code that fully matches a filter in
  `FILTER.md`. The filters never apply to findings about a change still under review, or to a problem a person reported
  or asked to have fixed.

When it is unclear whether a finding is covered, it is not covered: record it and name the spec section or item that
might cover it, so triage makes the call. When a person asked to have the finding recorded, do not drop it silently;
tell them what covers it and let them decide.

NOTE: A finding whose consequence would put it in the `highest` bucket (security, or loss of user data, credentials,
processes, or other user-owned work; `review_feedback_queue/AGENTS.md`, "Priority buckets", defines the buckets) is
never dropped on one agent's judgment. Before dropping it for any of the reasons in
`review_feedback_queue/.agents/recording.md`, "Before recording a finding", give an independent reviewer with fresh
context, one that neither produced the finding nor proposed the drop, the finding, the proposed basis for dropping it,
and the spec, TODO, `BUGS.md`, queue, ledger, or filter text that basis relies on. Drop the finding only when that
reviewer agrees the drop is justified. If the reviewer disagrees, or its verdict is ambiguous, record the finding. The
same applies when it is unclear whether a finding belongs in the `highest` bucket: treat it as if it does. A wrongly
dropped security or data-loss finding is gone without anyone having looked at it, while a wrongly recorded one costs a
few minutes of triage.

## Running an agent review that feeds this queue

These rules apply to any agent review whose findings are meant for this queue, whatever its scope or size. They come
from auditing the transcripts of a whole-codebase review swarm (2026-09-30): the reviewers mostly did sound work, but
coverage and results went missing in mechanical ways that none of them reported.

Tell each reviewer:

- Read code in chunks of at most about 400 lines per command. A larger tool output is replaced by a short preview and
  saved to a file, and a reviewer that takes the preview for the content has not read the code. In the audited run one
  reviewer called a module's schema migrations clean without ever having seen them. If a result says the output was
  saved to a file, open that file before relying on it.
- Create, modify or delete nothing in the checkout, helper scripts included; scratch goes in the agent scratch space
  `AGENTS.md`, "Agent scratch space", describes. One audit agent left a stray script in the repository root.
- Return findings in the final reply rather than in a report file. Harnesses commonly refuse report files written by
  subagents. For trailing reviews, also keep the private findings logs and progress artifacts `persistent-review`
  requires; those artifacts supplement the final reply and stay outside the checkout.
- Do not spawn nested helper agents. In the audited run, helpers' caveats and dropped candidates were not carried
  forward by the reviewers that spawned them.
- Flag a GUI handler that decides what a user-confirmed action acts on by reading live state when it runs (whichever
  prompt or dialog is open, the current list, a precondition recomputed at send time) instead of using what the render
  the user acted on showed. `SPEC_impl.md`, "GUI: Dioxus" (the paragraph "Answers bind to what was drawn"), states the
  rule and the mechanism every prompt answers through. Reviews kept finding such handlers one at a time.
- End the reply with three parts:
  - the findings, each with the template's fields plus a suggested bucket, a confidence (confirmed, or likely with the
    unverified premise named), `proposed_drop` and `possible_cover`;
  - every candidate considered and not reported, with the reason (refuted, with file and line; or covered, naming the
    exact spec section, `Planned` item, `BUGS.md` entry, queue file, ledger heading or filter) and whether it could be
    in the `highest` bucket;
  - the exact files and line ranges read, and what in scope went unread.

  A covered candidate that could be in the `highest` bucket is reported as a finding with `proposed_drop` filled in, not
  only listed, so the check in the NOTE under `review_feedback_queue/.agents/recording.md`, "Before recording a
  finding", can see it.

Then, when collecting the replies:

- Send every proposed drop of a finding that could be in the `highest` bucket to the independent reviewer the NOTE under
  `review_feedback_queue/.agents/recording.md`, "Before recording a finding", describes, including drops that appear
  only in the list of unreported candidates or in passing prose. In the audited run most reviewers mentioned such drops
  only in passing. "Too rare" or "by design" with no spec, TODO, `BUGS.md`, queue, ledger or filter citation is not
  coverage.
- Compare each reviewer's stated coverage with its scope, and tell the user which parts went unread rather than
  presenting a quiet area as clean.
- When writing a finding into a feedback file, keep the reviewer's confidence, suggested bucket, possible cover and
  caveats alongside the TLDR and details. Triage needs them to judge coverage, and in the audited run they were lost in
  transcription.

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
