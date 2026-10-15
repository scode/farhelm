# Spec: when Restart may resume the previous conversation

Written against main at 93092359 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where an outcome below tells you to change them.

This plan has no dependency on another plan.

## The goal

Restart resumes the agent's conversation from what Farhelm has recorded about it. Six review findings describe ways the
recorded conversation can lag behind the one the agent is really in (a report arriving during Restart, a transient
failure to record it, a reboot before it is applied, a missing Codex record, an upgrade done while Codex kept running, a
transcript read error), so that Restart resumes the previous conversation. The maintainer decided none of them is a code
defect in itself, and that SPEC.md should say why. Carry out these six triage outcomes, each recorded under its own
heading in `TRIAGE_OUTCOMES.md` (read every entry in full before starting):

1. `restart-drain-skips-newly-published-conversation.md` (`fix spec`; its Decision holds the maintainer's full words and
   its Completion criteria the spec change)
2. `restart-ignores-retryable-conversation-admission.md` (`fix spec`, covered by the change made in 1)
3. `reboot-discards-pending-new-conversation.md` (same)
4. `restart-withdrawal-discards-new-codex-selection.md` (same)
5. `codex-upgrade-replays-legacy-selection.md` (same)
6. `codex-transcript-error-discards-new-selection.md` (same)

### Acceptance criteria

- SPEC.md states the maintainer's three cases, in the maintainer's meaning:
  - Restart resuming the previous conversation in a brief window after the conversation was replaced (by clicking, by
    `/clear` in the agent, or similar) is acceptable.
  - A new conversation Farhelm never learns of, so that it stays undiscoverable long term, is a defect; the existing
    session-tracking notifications (SPEC.md, the notifications paragraph under Status) are its mitigation.
  - When Farhelm has a conversation recorded and tracking silently stops updating underneath it, continuing to resume
    that recorded conversation is an accepted consequence of whatever bug stopped the updates, not a defect in itself.
    Such bugs are still to be avoided.
- The two places that currently promise otherwise are reconciled with the new text rather than contradicting it: the
  sentence in Durability and resume that says a later Restart resumes "the conversation the agent was actually in", and
  the 2026-10-10 "Confirmed" paragraph under Lifecycle operations that limits wrong-conversation acceptance to a crash
  at one exact moment or two coinciding rare faults. Do not remove the protections that paragraph keeps (database
  corruption, bricking and similar lasting damage are never accepted).
- SPEC_impl.md is checked for text that contradicts the new rule; contradictions are fixed in the same PR, and nothing
  is added there that SPEC.md already says.
- Each of the six findings' feedback files and index entries is removed in its own PR, with its Execution field updated.
- No code changes.

## Requirement sources

**The user's decisions (2026-10-14):** the outcomes above, with the maintainer's words quoted in the first entry's
Decision. Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm. No-workhorse mode, which
`plans/AGENTS.md` requires. Grouping these six outcomes into one plan was agreed with the maintainer.

**Binding repository constraints:** root `AGENTS.md` "Execute triage outcomes" (one PR per outcome, ledger and queue
bookkeeping); SPEC.md "First-class harnesses", which names resuming the wrong conversation among what its allowance for
other harnesses does not cover. That does not literally contradict the new rule; add at most a cross-reference to it,
and leave the allowance as it is. `review_feedback_queue/FILTER.md` is not a product rule and is out of scope.

**Planner proposals:** where in SPEC.md the new text lives (Durability and resume is the natural home), the exact
wording, and the order of the stack.

## Outline

- PR 1 (`restart-drain-skips-newly-published-conversation.md`): the SPEC.md change, plus any SPEC_impl.md
  reconciliation, the queue removal and the ledger update.
- PRs 2 to 6, in the order listed above: each removes one feedback file and its index entry and updates that outcome's
  Execution field, noting that PR 1's spec change covers it. No other content.

### Validation

Documentation-only: per root `AGENTS.md` "Finishing work", run `dprint check` on the changed files and nothing else.

## Fallbacks

Agreed fallbacks: if a finding's described trigger turns out not to fit any of the three cases on closer reading, keep
its PR as the recorded `fix spec` outcome anyway (the ledger records the case it was assigned) and say so in the report.

Examples of departures that need the scope reassessment: any code change, or rewriting resume behavior beyond the three
cases.

Decisions to log: where the text went and its final wording, how the two contradicting passages were reconciled, what
was done (or not) in SPEC_impl.md.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-conversation-tracking-spec-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md`
(Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
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

- Follow root `AGENTS.md` "Execute triage outcomes": one reviewable commit, stable bookmark and draft PR per outcome, in
  a single linear stack in the order given under Outline above. Each PR carries its outcome's agreed change, the removal
  of its feedback file and `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution update. Record
  the jj change ID and bookmark in that Execution field before creating the PR, then add the PR's URL (with `/changes`
  appended, per the user's PR-link rule) to the same change and push it again; no separate bookkeeping PR. Replace the
  Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/triage-conversation-tracking-spec/<nn>-<short-name>` (for example
  `plan/triage-conversation-tracking-spec/01-restart-drain-skips`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits: type `docs` for a PR that changes SPEC.md or SPEC_impl.md, and
  `chore` for a PR that only removes feedback files and updates the ledger. No changelog fragment: none of these PRs has
  a user-facing effect.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

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
  `TRIAGE_OUTCOMES.md` entry.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

PRs in this stack that only remove feedback files and update the ledger, and change nothing else, may share one review
run that covers all of them together, instead of one run each.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above. If current code or specs invalidate a recorded triage decision, that is a block: root `AGENTS.md`
(Execute triage outcomes) returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless the
acceptance criteria above say otherwise, that together meets the acceptance criteria above, each PR passed its review
gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID, bookmark and PR. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
