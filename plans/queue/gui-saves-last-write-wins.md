# GUI template and number-key saves are last-write-wins, except after a failed save

Written against main at a2848cf2 on 2026-10-21. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Settle a contradiction in SPEC.md the maintainer's way: GUI saves of templates and number-key bindings stay
last-write-wins against agent and command-line edits, and that is now written down as accepted, with one exception the
code must fix: after a failed number-key save, the GUI's next save must not write its stale whole map over agent changes
it has since re-read.

Carry out these 2 triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md`, in this order. Read
every entry in full first.

1. `template-preservation-promise-omits-last-write-wins.md` (`fix spec`): its own PR, carrying the spec exception both
   outcomes share.
2. `shortcut-save-overwrites-intervening-agent-binding.md` (`fix spec+code`): its own PR. Its spec part is the exception
   PR 1 writes, so this PR carries the code fix, any SPEC_impl.md sentence describing the changed overlay, the test and
   the changelog fragment. Size gate in its entry: about 100 lines; over it, block and bring it back.

The maintainer's words for both: "Spec, plus fix the failed-save case", and "to be clear spec means that this (excluding
the fail case) is accepted."

## Outline

Two PRs: `docs` then `fix`. Facts from the planning session's reading of main (verify them; line numbers drift):

- PR 1: SPEC.md's "A GUI write must not undo a change an agent or the command line made while that view was open"
  (around 3320) gets the explicit exception. SPEC.md's Concepts entry for launch templates (around 62–64) already says
  template edits are last-write-wins with no optimistic-concurrency check wanted, the number-key bullet (around 797)
  says bindings are a whole-map preference replaced on write, and SPEC_impl.md (around 5305) says the GUI's template and
  preference writes keep their last-write-wins behavior; make the three agree. The exception says GUI and agent edits of
  templates and number-key bindings are last-write-wins, with the GUI re-reading bindings before it saves, and that the
  failed-save case is excluded. The agent's own template write (SPEC.md around 2716) stays the one exception the other
  way; do not touch it.
- PR 2: in `crates/farhelm-ui/src/api.rs`, a failed write leaves the bindings slot unacknowledged (`finished`), so
  `dirty()` keeps returning the whole local map, and `template_bindings_after_read` overlays it on the next fresh read;
  `crates/farhelm-ui/src/list/templates.rs` (around 1318–1349) then re-reads, overlays, changes one key and queues the
  whole map, which the helm replaces without a precondition. The stale map has three consumers, and the fix covers all
  of them: the refresh overlay the key edit uses (`template_bindings_after_read`); the replay after signing back in
  (`seed_with_local_changes` and `begin_replay`, `api.rs` around 1318–1361), which writes the stale map over the fresh
  read with no user action and is the likeliest route, since an expired credential is the likeliest cause of a failed
  save; and the rename repoint path (`write_template_binding`), whose failures also keep their intent dirty. The
  planning review's suggested shape: remember a failed change as the delta between the map that was sent and the read it
  was computed from, so successive failed edits accumulate, and reapply that delta in every consumer. Any equivalent
  that keeps agent changes made before the read is fine, logged as a DECISION. The replay path counts toward the size
  gate. The race between the fresh read and the write is accepted and stays. The earlier decision
  `stale-template-dialog-undoes-agent-shortcut.md` deliberately kept the whole-map overlay; this outcome narrows it for
  the failed-save case only. SPEC_impl.md's overlay helper description (around 1123 and 1163–1166) changes only where it
  would become wrong.

### Validation

PR 1: `dprint check` on the changed files. PR 2: unit tests beside the existing bindings tests in `api.rs` (around
6494–6570, `number_binding_delete_retires_only_the_deleted_name` and
`number_bindings_refresh_and_clear_through_the_shared_queue`) showing that after a failed save, both the next key edit
and the sign-in replay keep a newer agent assignment from the fresh read while reapplying the user's own change; run the
narrowest `farhelm-ui` Rust selection through the recorder (`.agents/narrow-tests.md`). The browser spec
`e2e/tests/templates.spec.ts` covers clean interactions; run it only if the change touches the interaction path it
exercises.

## Fallbacks

Examples of departures that need the scope reassessment: a helm API change (per-key or conditional updates), or touching
template-body saves rather than bindings.

Decisions to log: the spec wording, the shape of the failed-save fix, and its measured size against the gate.

## Gates, acceptance and requirement sources

### Size gates

Each outcome's gate, where it has one, is in its `TRIAGE_OUTCOMES.md` entry's Completion criteria and is binding: about
100 lines of diff (code, tests and docs, not counting `TRIAGE_OUTCOMES.md`, the queue removal or the changelog
fragment). The maintainer prefers size gates to time estimates. Each entry also says what happens over the gate, and
that differs by outcome: some say to block and bring it back to the maintainer (then block per
`plans/.agents/executing.md`, "Executing one plan", step 10, after building and pushing the plan's other outcomes, with
a question that describes the larger fix, its measured size, and asks whether to build it or discard the outcome),
others say to discard the outcome instead (record Execution as complete with the gate tripped, the measured size and the
reason, remove the feedback file and index line, and change no code). Follow whichever the entry says. Measure and log
each outcome's size. The entry's described fix is the triaging session's proposal; a different fix within the same gate
that meets the same completion criteria is fine, logged as a DECISION.

### Acceptance criteria

- Each outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry, including any test it names.
- Each outcome's PR removes its feedback file and `review_feedback_queue/INDEX.md` entry and updates its Execution
  field.
- A `fix` PR carries a changelog fragment per `AGENTS.md`, "Changelog fragments" (`kind: fixed` unless the change has
  nothing user-facing, then `kind: none` with the reason); `docs`, `test`, `chore` and bookkeeping PRs carry none.

## Requirement sources

**The user's decisions (2026-10-21):** each outcome's Decision field records the maintainer's answer and its scope, most
of them from the theme questions of that day's triage of the highest-priority bucket, quoted in each entry. While
scheduling, the maintainer agreed to the 13 pending outcomes of that triage being scheduled as 6 plans with the grouping
in `plans/queue/INDEX.md`, including the grouped PRs this file names, and chose the review gate below: a single
fresh-context Opus 5.5 agent at high effort with the general charter, no swarm. No-workhorse mode, which
`plans/.agents/planning.md` requires. Six other outcomes of the same triage were folded into the TODO.md entry "Bind GUI
answers to what the user saw" and are not this plan's work.

**Binding repository constraints:** the rule each entry's Assessment cites;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits", "Code rules" and "Tests and validation", with `.agents/validation.md`;
`.agents/test-authoring.md` for any test change; and anything this file's outline names as binding for the plan.

**Planner proposals:** the PR order and the outline above, including the notes a fresh planning review added. A planner
proposal is not a requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-gui-saves-last-write-wins-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one reviewable commit, stable
  bookmark and draft PR per outcome (unless this file groups some outcomes into one PR, which the maintainer agreed for
  those cases only), in a single linear stack in the order given under The goal above. Each PR carries its outcome's
  agreed change, the removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its
  `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field before creating
  the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/gui-saves-last-write-wins/<nn>-<short-name>` (for example `plan/gui-saves-last-write-wins/01-short-name`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment in the same commit; `test`
  for a change only to tests or their fixtures; `docs` for documentation and specification text only; `chore` for
  maintainer tooling and queue bookkeeping. Only a fragment-requiring type adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TRIAGE_OUTCOMES.md`, `review_feedback_queue/`, `TODO.md` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high
effort, launched as a native sub-agent where your harness serves that model and otherwise through
`$scode-harness-shellout` (that skill owns the launch mechanics). The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry, including any size gate (ask the reviewer to flag anything beyond the gate).

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, whether each gated outcome met its gate (with
the measured diff size) and why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. If current code or specs invalidate a recorded triage decision (for example,
the finding turns out already fixed or wrong), that is a block: `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless this file
groups some into one PR (an outcome discarded under its gate still gets its bookkeeping PR; one whose gate says to bring
it back makes the plan block instead, after its other outcomes are pushed), that together meets the acceptance criteria
above, each PR passed its review gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID,
bookmark and PR. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Blocked

Blocked on 2026-10-22 (claim eac6f2).

### The failed-save fix for number-key shortcuts is over its size gate: land it or discard it?

#### What this is about

Number keys launch templates. In the Templates dialog each template can be given one of the keys 1–9 or 0, and an agent
(or the `farhelm` command line) can assign or move those keys too. When you change a key in the Templates dialog, the
app re-reads the current keys, applies your change, and saves the whole set back.

The 2026-10-21 triage found what happens when that save fails, most likely because your sign-in expired. The app keeps
the whole set it tried to save and treats it as the truth from then on. Your next key change, renaming a template that
has a key, or simply signing back in, then writes that old set back, undoing any key an agent moved or assigned in the
meantime. A later key press can then launch a template the agent had deliberately moved off that key.

You decided "Spec, plus fix the failed-save case": the spec should accept that GUI saves otherwise overwrite agent
edits, and the failed-save case should be fixed. The fix had a size gate of about 100 changed lines (code, tests and
docs), and "over it, block and bring it back". The triage estimated 60–120 lines and called it borderline.

#### What the plan did

The spec change is done and is draft PR #2562. SPEC.md now accepts that GUI saves of templates and number keys can
overwrite an agent's or the command line's edit, except after a failed number-key save.

The fix is built, tested and reviewed, and is held as draft PR #2573 on top of #2562. After a failed save, the app now
remembers only which key you chose for each template you changed, not the whole set. It applies just those choices to
every fresh read: when you next change a key, when you rename a template, when a number key launches, and in the
automatic retry after signing back in. Choices from several failed saves add up until one save goes through. Where your
change and an agent's touch the same key or template, yours wins, as it would have if the save had worked. Keys an agent
set that you did not touch survive.

Three things came up that the triage did not anticipate, all fixed in #2573:

- If the app cannot read the keys when you sign back in, the retry now waits for a real read. Before this, it would have
  saved your changes over an empty set, erasing every other key.
- A key you vacated by moving a template away is not cleared when an agent has since put another template on it.
- If a save fails, an agent then rebinds that key, and you change the key again, your second change is kept, not lost.

The first version tracked changes per key and needed two review rounds of fixes. A scope review then found a simpler
shape (tracking per template), which is what #2573 now contains. That review also checked for a smaller design that
still meets the spec and your wording, and found none under about 135 lines.

The fix measures 194 changed lines: about 90 of code, 35 of comments and implementation-spec text, and 65 of tests. The
tests cover the next key change, the sign-in retry, a failed sign-in read, an agent reusing a key you vacated, and
deleting a template. Most of the size over the estimate comes from the sign-in retry, which the estimate did not
include.

#### The question

Should #2573 land as built, or should this outcome be discarded?

- **Land it (recommended).** #2573 lands as is, after #2562. The failed-save case is then closed everywhere the old set
  could be written back. Cost: 194 lines in the code that saves the app's shared preferences, although the change only
  affects number keys. Recommended because every fix that keeps your criterion is over the gate anyway (none under about
  135 lines), and this one is already built and reviewed.
- **Discard it.** #2573 is dropped and the outcome is recorded as discarded over its gate. #2562 still lands, but its
  sentence excluding the failed-save case would have to be withdrawn, which makes the failed-save overwrite accepted
  like the rest.
- **A smaller fix that changes your criterion.** The app could simply forget a failed number-key save, so it never
  writes an old set back: about 30–40 lines. A change you made that failed to save would then be silently lost, which
  goes against your wording ("reapplies only the key change the user made") and #2562's spec text. A middle option (no
  retry of number keys after signing back in, about 110–125 lines, so still over the gate) leaves your change visible in
  this window only, while the helm, agents and other windows keep the old key.
