# review_feedback_queue/ rules

This directory is the review feedback queue: findings from code reviews (agent or human) that are real enough to keep
but were not fixed in the review itself. One file per finding, plus an index.

This file holds the rules that apply to all work here, and a reading list for each flow; the procedures are in
`review_feedback_queue/.agents/`. Before acting in any flow named in `review_feedback_queue/AGENTS.md`, "Flows and
reading lists", read every file in its reading list in full. After a compaction or resume, re-read every file in your
role's reading list before your next action.

## Files

- `INDEX.md` lists every feedback file with a one-line description of the problem. It must always match the actual files
  in this directory. Updating it is part of every change here, not a follow-up.
- `<mnemonic-name>.md` is one piece of feedback, named briefly after the problem (`disconnect-race.md`,
  `stale-cache-on-rename.md`), never a number or a date.
- `FILTER.md` holds the filters that keep low-value findings from automated review of committed code out of the queue
  and out of triage. It is not a feedback file and is not listed in `INDEX.md`. Its rules decide only what is worth a
  human's triage time; they are not product or coding rules, and nothing outside this directory's recording and triage
  work should apply them.
- `TRAILING_REVIEW.md` records the last commit on `main` whose trailing review completed. It is review bookkeeping, not
  a feedback file, and is not listed in `INDEX.md`.
- `.agents/` holds the procedures the reading lists name. It is not feedback and is not listed in `INDEX.md`.

## Lifecycle

Feedback leaves the queue the same way it arrived: explicitly. A change, commit, or PR that addresses an item must
remove the item's file (and its `INDEX.md` line) in the same commit, change, or PR — or narrow the file to what remains,
if it only partially addresses it. A fix that leaves its feedback item behind is incomplete.

`review_feedback_queue/.agents/triage.md` and `review_feedback_queue/.agents/execute-outcomes.md` define the separate
"triage review feedback" and "execute triage outcomes" steps. Triage records the user's decision in root
`TRIAGE_OUTCOMES.md` without changing the feedback file or index. Execution applies that decision and removes or narrows
the item in its own draft PR. A `discard` outcome means only "not worth the human's time at this time", not that the
feedback is wrong; its execution removes the file and index entry without a spec or code change. The outcome ledger
lives outside this directory and is not a feedback file to add to `INDEX.md`.

Exception: when triage verifies that a finding is fully covered by an accepted specification rule, an existing `Planned`
item in TODO.md, a known bug recorded in root `BUGS.md`, or a filter in `FILTER.md`, record the basis in the ledger and
remove the feedback file and index entry immediately. These items must not remain in the queue to be skipped repeatedly.
Completing this queue cleanup does not mean the planned implementation is complete. A filter match likewise says nothing
about whether the behavior is acceptable in code; it only means the finding is not worth triage.

## Priority buckets

Every queued finding also has a triage priority bucket, recorded in `review_feedback_queue/INDEX.md`:

- `highest`: security-impacting bugs or bugs that can lose user data, credentials, processes, or other user-owned work.
- `high`: bugs that materially degrade the user experience, either through severe rare failures or moderate common ones.
- `other`: everything else, including correctness, diagnostics, cleanup, and convenience issues without those impacts.

Some consequences look like `highest` but are not (maintainer, 2026-10-14). Losing something the user can recreate in a
moment, such as unsaved form input or the clipboard's contents, is not losing user-owned work. Harm confined to
maintainer tooling (tests, release procedures, the docs build) stays out of `highest` unless it is catastrophic. A
documentation error is `highest` only when the statement could lead a reader into a security or data-loss mistake.

Assign the bucket from the user-visible consequence, not from implementation difficulty or reviewer confidence. A rare
security or data-loss consequence remains `highest`; uncertainty about whether a consequence crosses that boundary must
be surfaced to the user before finalizing the bucket. Keep the index grouped by bucket, with the highest bucket first,
and preserve index order within each bucket. Explain the chosen bucket when presenting an item for triage, and revisit
it if the user supplies a different severity judgment.

## Triage and execution are separate

This is a persistent triage flow, not an execution flow. Once the session is working through review feedback, remain in
triage mode across turns until the user explicitly starts a separate execution step. Every short command or agreement —
including “fix this”, “fix code”, “update the spec”, “let’s do that”, “agreed”, and “yes” — is a triage decision to
record, not permission to edit code, specifications, TODO entries, or queue files. The human must not repeat that this
is “just triage” or otherwise police the boundary on every item.

Execution requires a separate, unmistakable command that names the action, such as “execute the recorded triage
outcomes”, “implement the approved fixes now”, or “start the execution phase”. Contextual agreement with a
recommendation never starts execution. When execution is explicitly started, apply only the recorded scope and stop for
the documented complexity or scope gate rather than silently expanding it.

## Flows and reading lists

Coverage sources, named in several lists here: `SPEC.md`, `SPEC_impl.md`, the `Planned` bucket of `TODO.md`, `BUGS.md`,
`review_feedback_queue/INDEX.md` and the queue files it lists, and `TRIAGE_OUTCOMES.md`. These are searched for the
finding at hand, with the matching sections read in full; they are not read end to end.

### Recording a finding ("record review feedback", "add this to the review queue", "queue this finding")

This also covers rejecting or dropping a finding, recording findings from any review (agent or human, of a change under
review or of committed code), and extending an existing queue item. Before writing, extending or dropping any finding,
read these in full:

- `review_feedback_queue/AGENTS.md`
- `review_feedback_queue/.agents/recording.md`
- `review_feedback_queue/INDEX.md`
- `review_feedback_queue/FILTER.md`
- the coverage sources, for each finding

Fence: a finding that could be in the `highest` bucket is never dropped without an independent fresh-context reviewer
agreeing; `INDEX.md` changes in the same change as the feedback files.

### Commissioning an agent review that feeds the queue

Any agent review whose findings are meant for this queue, whatever its scope or size, including a trailing code review
and a review swarm whose results will be recorded here. Before writing the first reviewer's prompt or launching any
reviewer, read these in full:

- `review_feedback_queue/AGENTS.md`
- `review_feedback_queue/.agents/recording.md`
- `review_feedback_queue/INDEX.md`
- `review_feedback_queue/FILTER.md`
- `AGENTS.md`, "Agent scratch space" (root, already in context for agents started at the checkout root)

Phase files: on collecting the replies, the recording role's reading list in `review_feedback_queue/AGENTS.md`, "Flows
and reading lists" (the same files plus the coverage sources).

Fence: each reviewer gets the instructions in `review_feedback_queue/.agents/recording.md`, "Running an agent review
that feeds this queue", and the reviewer delegate prompt's reading list in `review_feedback_queue/AGENTS.md`, "Flows and
reading lists"; every proposed drop of a finding that could be in the `highest` bucket goes to an independent reviewer.

### Reviewer delegate prompt (the instructions each reviewer is given)

The commissioner's prompt to each reviewer carries the list in `review_feedback_queue/.agents/recording.md`, "Running an
agent review that feeds this queue", and tells the reviewer to read these in full before reading any code under review:

- `AGENTS.md`, in full, when the reviewer was not started at the checkout root (a delegate outside a checkout starts
  with nothing)
- `review_feedback_queue/AGENTS.md`, "Priority buckets" (for the suggested bucket)
- `review_feedback_queue/.agents/recording.md`, "What goes in a feedback file" (the template's fields) and "Before
  recording a finding" (what counts as coverage)
- `review_feedback_queue/FILTER.md`, when the review is an automated review of committed code
- the coverage sources, for each candidate it reports as covered

Fence: the reviewer creates, modifies or deletes nothing in the checkout, returns findings in its final reply, and
spawns no nested helper agents.

### Trailing code review

Triggers: "perform trailing code review", or similar wording around "trailing code review". Before fetching
`origin/main` to pin the review tip, read these in full:

- `review_feedback_queue/AGENTS.md`
- `review_feedback_queue/.agents/trailing-review.md`
- `review_feedback_queue/.agents/recording.md`
- `review_feedback_queue/INDEX.md`
- `review_feedback_queue/FILTER.md`
- `review_feedback_queue/TRAILING_REVIEW.md`
- the `jjstack` skill
- `AGENTS.md`, "Conventional Commits" (root, already in context for agents started at the checkout root)
- the coverage sources, for each finding

Fence: the request authorizes recording findings and updating the checkpoint only, not triage, fixes or execution; the
checkpoint advances only after every commit in the range was reviewed and every finding handled; the review-result PR
stays a draft and is never merged automatically.

### Triage ("triage review feedback")

Before presenting the first item, read these in full:

- `review_feedback_queue/AGENTS.md`
- `review_feedback_queue/.agents/triage.md`
- `review_feedback_queue/INDEX.md`
- `TRIAGE_OUTCOMES.md`, if it exists
- the `Planned` bucket of `TODO.md`
- `review_feedback_queue/FILTER.md`
- `BUGS.md`
- `AGENTS.md`, "Talking to the user" (root, already in context for agents started at the checkout root)
- for each item: its feedback file, the code paths it names, and the authoritative `SPEC.md` and `SPEC_impl.md` sections

Phase files: on deciding that an item needs a focused reproduction or check, read `.agents/validation.md`,
`.agents/test-evidence.md` and `.agents/narrow-tests.md` before running it.

Triage is a persistent mode and can run across many turns, so keep a working log at `farhelm-triage-log-<checkout>.md`
in the parent directory of the checkout, next to it and out of the repository, where `<checkout>` is the checkout
directory's name. It records the active mode (triage, or execution once a command that names it has started it), this
reading list, the item in progress, and the session's scratch directory. After any compaction or resume, read it first.
It is never committed.

Fence: during triage, “fix this”, “fix code”, “update the spec”, “let’s do that”, “agreed” and “yes” are decisions to
record in `TRIAGE_OUTCOMES.md`, never permission to edit code, specifications, TODO entries or queue files; execution
needs a separate command that names it.

### Executing triage outcomes ("execute triage outcomes", "execute the recorded triage outcomes", "implement the approved fixes now", "start the execution phase")

Before creating the first change, read these in full:

- `review_feedback_queue/AGENTS.md`
- `review_feedback_queue/.agents/execute-outcomes.md`
- `TRIAGE_OUTCOMES.md`
- `review_feedback_queue/INDEX.md`
- the `jjstack` skill
- `AGENTS.md`, "Tests and validation", "Changelog fragments" and "Conventional Commits" (root, already in context for
  agents started at the checkout root)
- `.agents/validation.md`
- `.agents/test-evidence.md`
- for each outcome: its feedback file and the `SPEC.md` and `SPEC_impl.md` sections it concerns

Phase files: on changing tests or their fixtures, read `.agents/test-authoring.md`; on a failing test, read
`.agents/narrow-tests.md`.

When execution starts in a session that kept a triage working log, record the change of mode and this reading list
there.

Fence: one draft PR per outcome in one linear stack; this flow never marks PRs ready, enables auto-merge, or merges;
outcomes marked `` planned in `plans/queue/<slug>.md` `` are executed only by running that plan; undecided items are not
triaged here.

### Scheduling outcomes through the planning system ("use the planning system to schedule these")

When the user explicitly asks for the planning system to schedule triage outcomes, read `plans/AGENTS.md` in full before
writing any plan. Fence: only outcomes whose Execution is `pending` can be scheduled.
