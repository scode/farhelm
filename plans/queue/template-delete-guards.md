# Template Delete keeps its promises

Written against main at 6b7bbcda on 2026-10-26. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute three triage outcomes recorded on 2026-10-26, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute
triage outcomes": one draft PR per outcome, in this plan's one stack. Their `TRIAGE_OUTCOMES.md` entries are the binding
statement of what to do; read each in full from `main@origin`, together with its feedback file under
`review_feedback_queue/`, before working on it:

1. `template-retry-installs-unseen-permanent-delete.md` (`fix code`). In Templates (Settings → Templates), after a
   rename's save succeeds and the follow-up preferences and list reads fail, the dialog keeps an enabled plain "delete"
   (with the Undo promise) beside Retry. Retry causes no re-render, because the dialog reads the list resource's loading
   state with `peek` and never subscribes. When the reload finishes, the next render installs a Permanent handler before
   the "delete permanently" label is painted, and a click in that one-frame gap permanently deletes a stored template
   that fails current save rules, with no warning and no Undo.
2. `template-delete-reads-new-selection.md` (`discard`). Only the queue removal and the ledger's execution record; no
   code or spec change.
3. `template-delete-lost-reply-loses-undo.md` (`fix code`). When a Delete commits on the helm but its reply is lost
   (timeout or transport error), the dialog shows a raw error, drops the snapshot, offers no Undo and does not reload,
   so the deleted template still shows as present.

What the code does today, from triage's read of main at 2977e6c9 (line numbers approximate):

- `crates/farhelm-ui/src/list/templates.rs`: `DeleteKind::of` (around 1528-1537) treats an unknown list as Restorable;
  the button and its tooltip around 2257-2290; the Delete handler around 1955-2054 reads the selection and list when it
  runs, captures the kind from the render (around 1744, 2028), installs Undo only on success, and on error (around 2054)
  only shows the message; `undo_after_delete` returns nothing for Permanent (around 1551-1555); the failed-reads path
  that restarts the list around 1863-1927.
- `Resource::pending()` uses `peek` in the pinned `dioxus-hooks` 0.7.10 (`use_resource.rs`, around 328-330), so reading
  it does not subscribe the component.
- `crates/farhelm-ui/src/api.rs`: `delete_template` (around 4245-4258) passes the request error through as a string, so
  timeouts and transport errors cannot be told from helm refusals; the request funnel and its deadlines are around
  1858-1890 and 2123-2151.
- The helm commits the deletion in a detached task and replies afterwards (`crates/farhelm-helm/src/templates.rs`,
  around 160-208); a second Delete of a missing template gets a not-found refusal.

### Acceptance criteria

- Outcome 1: a click on a Delete whose painted label was plain "delete" never deletes permanently without Undo,
  including when a newer render has changed the kind but not yet been painted. A test covers the failed-reload, Retry,
  kind-change sequence. SPEC.md (Templates) is the rule: "The label the user saw decides: a click on 'delete
  permanently' never offers undo, and a click on plain 'delete' always does, even if the template list changed in
  between." Choose one of these two shapes and log why; planning review found that both hold on the desktop app's event
  path:
  - Keep Delete disabled whenever the template list is unknown (failed or never loaded), not only while it reloads. This
    covers the Retry route the ledger names.
  - Render one Delete button per kind, both kept mounted with a fixed kind each, and switch between them with `hidden`,
    so a click follows the element that was painted, in both directions. This costs updating the `.templates-delete`
    selectors in `e2e/templates.spec.ts` (about eight).

  Two shapes that look natural do not hold, so do not use them: disabling the button only while the list reloads (a read
  that finishes within one frame still installs the Permanent handler under the old painted label), and refusing a click
  whose kind differs from what was last painted (that needs the general paint tracking Fallbacks rules out).
- Outcome 2: the feedback file and its index line are removed and the ledger records the execution. Nothing else.
- Outcome 3: on a transport failure or timeout of a template Delete, Templates says the delete may have happened and
  re-reads the list, so a deleted template shows as gone; no Undo is offered after a lost reply; helm refusals keep
  today's error. A test loses the reply of a committed delete. This is a deliberate stopgap: TODO.md's "Treat the
  GUI–helm connection as infallible" entry may later remove it, so keep it small and do not build general lost-reply
  machinery for it. `delete_template` already tells "no status received" from "the helm refused", so the boundary change
  is small. Settled for the executor: a sign-in (401) refusal and a wait that timed out before anything was sent may
  also land in the "may have happened" report, which is harmless; the rename path's internal delete keeps today's
  handling; only the template list is re-read, with no reconciliation of quick-launch marks or number keys.
- Each `fix code` PR is a `fix:` and carries a changelog fragment (root `AGENTS.md`, "Changelog fragments"); the
  `discard` PR is a `chore:` or `docs:` with none.

## Decisions already made

Settled with the maintainer on 2026-10-26:

- **The outcomes** are as recorded in `TRIAGE_OUTCOMES.md`; outcome 1 follows the maintainer's rule that a guard the
  spec names holds even for a click that lands between a render and its paint, while outside such guards a click in that
  gap is carried out as delivered (which is why outcome 2 is a discard).
- **One plan for the three outcomes**, because they touch the same Delete handler and would conflict as separate plans.
- **Outcome 3's shape**: "say the outcome is unknown and refresh", without Undo.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the three `TRIAGE_OUTCOMES.md` entries named above, and the decisions above.

**Binding repository constraints:** SPEC.md, Templates (the delete label rule and Undo), "Signing in again" (an action
"either completes and reports its outcome, or reports that its outcome is unknown") and the 2026-10-10 sub-second
allowance; SPEC_impl.md, "Answers bind to what was drawn"; root `AGENTS.md`, "Tests and validation", "Changelog
fragments", "Conventional Commits"; `.agents/test-authoring.md`; `review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Prefer the smallest fix for outcome 1 that holds on the desktop app's event path; a general change to the shared
  confirmation mechanism is out of scope (see Fallbacks).
- For outcome 3, distinguish transport failures from refusals at the `delete_template` boundary only, not across the
  whole API funnel.

## Outline

Three PRs, in ledger order:

1. `plan/template-delete-guards/01-retry-permanent-label`: outcome 1, with its test and fragment.
2. `plan/template-delete-guards/02-discard-selection-race`: outcome 2's queue removal and ledger record.
3. `plan/template-delete-guards/03-lost-delete-reply`: outcome 3, with its test and fragment.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy on `farhelm-ui`
(including `--features desktop` if you touch desktop-only code), the `farhelm-ui` unit tests for Templates through
`python3 scripts/record-test-run.py`, the Templates browser spec on Chromium and WebKit if you change its selectors or
the web behavior, and `python -B scripts/check-test-sleeps.py` when you change Rust or browser tests. PR 2 needs only
`dprint check`.

A browser spec cannot show outcome 1's gap: the web renderer applies screen updates within the render, so such a test
passes with or without the fix. Use the existing unit harness that re-renders and then delivers a click to an element
captured before the re-render (`crates/farhelm-ui/src/list/row.rs`, around 3562; `templates.rs` has tests of that kind
around 2577 and 3276), and show the test fails without the fix.

## Fallbacks

Examples of departures that need the scope reassessment: changing the shared `ConfirmSlot` mechanism or every
destructive handler, adding lost-reply reconciliation beyond template Delete, or a general paint barrier for the desktop
renderer.

Decisions to log: outcome 1's fix shape and why it holds on desktop, how a lost reply is recognized, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-template-delete-guards-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/template-delete-guards/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per triage outcome in the order the outline gives,
each meeting that outcome's completion criteria in `TRIAGE_OUTCOMES.md` (as refined by Decisions already made above),
each having passed its review gate, and each recording its execution in the outcome's Execution field (change id,
bookmark and PR URL) and removing its feedback file and `review_feedback_queue/INDEX.md` line, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
