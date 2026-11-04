# Recording review findings

This file covers what every finding goes through before it is written to the queue, how to run an agent review whose
findings feed the queue (including what each reviewer is told), how the independent check before dropping a finding is
run, and what a feedback file contains. It belongs to the "Recording a finding", "Commissioning an agent review that
feeds the queue", "Reviewer delegate prompt" and "Trailing code review" reading lists in
`review_feedback_queue/AGENTS.md`.

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
`review_feedback_queue/.agents/recording.md`, "Before recording a finding", give an independent agent with fresh
context, one that neither produced the finding nor proposed the drop, the finding, the proposed basis for dropping it,
and the spec, TODO, `BUGS.md`, queue, ledger, or filter text that basis relies on. Drop the finding only when that agent
agrees the drop is justified. If the agent disagrees, or its verdict is ambiguous, record the finding. The same applies
when it is unclear whether a finding belongs in the `highest` bucket: treat it as if it does. A wrongly dropped security
or data-loss finding is gone without anyone having looked at it, while a wrongly recorded one costs a few minutes of
triage. `review_feedback_queue/.agents/recording.md`, "Independent drop check", says how to run that check.

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
  subagents.
- Do not spawn nested helper agents. In the audited run, helpers' caveats and dropped candidates were not carried
  forward by the reviewers that spawned them.
- Flag a GUI handler that decides what a user-confirmed action acts on by reading live state when it runs (whichever
  prompt or dialog is open, the current list, a precondition recomputed at send time) instead of using what the render
  the user acted on showed. `SPEC_impl.md`, "GUI: Dioxus" (the paragraph "Answers bind to what was drawn"), states the
  rule and the mechanism every prompt answers through. Reviews kept finding such handlers one at a time.
- Before judging any behavior, find the `SPEC.md` and `SPEC_impl.md` sections that govern your scope, read them in full,
  and check every candidate against them. Behavior the spec explicitly accepts is not a bug.
- Do not search the queue, `FILTER.md`, `BUGS.md`, `TODO.md`, the plans or `TRIAGE_OUTCOMES.md` for coverage. A
  candidate you believe one of those covers is still reported as a finding, with what you know of the cover in
  `possible_cover`; matching against them happens once, when the findings are recorded. Set `proposed_drop`, with its
  basis, only when you think a finding should not be queued; the commissioner decides.
- End the reply with three parts:
  - the findings, each with the template's fields plus a suggested bucket, a confidence (confirmed, or likely with the
    unverified premise named), `proposed_drop` and `possible_cover`;
  - every candidate considered and set aside, with the reason (refuted by the code under review, with file and line; or
    explicitly accepted by the spec, naming the exact `SPEC.md` or `SPEC_impl.md` section) and whether it could be in
    the `highest` bucket. A candidate acceptable only because of some other cover is not set aside; report it as a
    finding;
  - the exact files and line ranges read, and what in scope went unread.

  A spec-accepted candidate that could be in the `highest` bucket is reported as a finding with `proposed_drop` citing
  the section, not only listed, so the independent drop check sees it.

  A review procedure that keeps these fields in a durable record during the review may say how that record replaces or
  feeds the final reply and these three parts, as long as every field above reaches the commissioner.

Then, when collecting the replies:

- Check each finding for coverage and filters as `review_feedback_queue/.agents/recording.md`, "Before recording a
  finding", describes, starting from its `possible_cover`. Reviewers check only the spec, so this is where the other
  coverage sources are searched, and findings already in the queue are expected here.
- Send every proposed drop of a finding that could be in the `highest` bucket to the independent drop check
  (`review_feedback_queue/.agents/recording.md`, "Independent drop check"), including one only listed among the
  candidates set aside instead of reported, and one that appears only in passing prose. In the audited run most
  reviewers mentioned such drops only in passing. Finding those drops is your job as commissioner, done when building
  the check's register; the check itself only judges them. "Too rare" or "by design" with no spec, TODO, `BUGS.md`,
  queue, ledger or filter citation is not coverage.
- Compare each reviewer's stated coverage with its scope, and tell the user which parts went unread rather than
  presenting a quiet area as clean.
- When writing a finding into a feedback file, keep the reviewer's confidence, suggested bucket, possible cover and
  caveats alongside the TLDR and details. Triage needs them to judge coverage, and in the audited run they were lost in
  transcription.

## Independent drop check

NOTE: This check is a consultation, not a review. It judges drops that have already been found and stated; it does not
search for drops or new findings, and it does not reread the reviewers' output.

The check has two steps: you state every drop in a register, then fresh agents judge the register.

### The drop register

Run the check after every other decision about which findings to keep, so that it sees all of them. A register entry is
any of these, for a finding that could be in the `highest` bucket (including one whose bucket is unclear):

- a drop as covered or filtered, whether a reviewer proposed it or you did while checking coverage, deduplicating or
  filtering;
- a finding a reviewer reported and later withdrew;
- a finding whose consequence a reviewer later lowered out of the `highest` range. Its entry asks whether the finding is
  outside `highest`, not whether to drop it.

Each entry holds who proposed it and how they labelled the finding, the finding's original and latest wording, the
proposed basis, the exact spec, TODO, `BUGS.md`, queue, ledger or filter text that basis cites, and source references at
the reviewed revisions.

When the findings come from an agent review, check the register against the reviewers' output before consulting anyone:
every finding with `proposed_drop` or a drop you proposed while checking coverage, and every withdrawn or lowered
finding has an entry. A drop that appears only in a reply's prose goes back to the reviewer that made it as a named
question, so that its entry carries that reviewer's own basis and citation. When that reviewer can no longer be asked,
record the finding. This completeness check stays with you; it is not handed to the agents that judge the register.

### The consultation

Give the register entries to one or more fresh agents, none of which produced or proposed any entry it receives:

- Put the entries and their cited text in the prompt. The agent may read source at the reviewed revisions and the cited
  coverage sources as they stand now to check a basis. Nothing else is required of it as input, and it does not have to
  show that it read anything beyond the prompt.
- Batch entries by subsystem, so one agent's reading of the surrounding code serves several verdicts. Keep a batch small
  enough that the agent can actually check each basis against source.
- Ask for one verdict per entry: AGREE with the entry as narrowly stated, DISAGREE, or AMBIGUOUS, naming the file and
  line or the cited text it checked and the exact scope of any agreement.
- Run it as one turn. Continue the same agent for a second turn only when a verdict is unclear or does not address the
  stated basis.
- A concern the agent notices that is not a register entry comes back to you as a lead for whoever reviewed that scope.
  The agent does not go looking for more. When nobody can take the lead, record it as a finding, keeping its
  uncertainty.

Act on an entry only on AGREE. An AGREE that names nothing it checked, and anything short of a clear verdict after the
second turn, counts as AMBIGUOUS. An AGREE narrower than the entry covers only the agreed part: record the rest and name
what covers the agreed part. On DISAGREE or AMBIGUOUS, record the finding, and keep a lowered finding treated as
`highest`.

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
