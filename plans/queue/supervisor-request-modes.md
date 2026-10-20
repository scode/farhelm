# Supervisor: every request type declares how it runs

Written against main at 18e51c3e on 2026-10-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `tab-attach-no-wait.md`: the queue holds it back until that plan has landed on main. That plan
stops tab attach from stalling the read loop while its session is stopping, restarting or deleting, which this plan
would otherwise also have to fix, in the same handler. It is one of five structural refactor plans written together
(`supervisor-request-modes`, `helm-owned-by-default`, `helm-ingress-types`, `ui-peer-text-type`, `ui-session-actions`);
the others touch the helm and the UI, not the supervisor.

## The goal

This is a structural refactor, followed by fixes of the known requests that still stall the read loop. The refactor
changes no behavior; each fix is its own `fix:` PR. The purpose is to make one recurring class of supervisor bug hard to
reintroduce as agents keep adding request types, and to clear the instances of it that remain.

The supervisor serves each helm connection from one read loop. That loop awaits the request dispatcher (`handle_control`
in `crates/farhelm-supervisor/src/service/handlers.rs`) for every control message, so a handler that waits inside the
loop (on a lifecycle claim, an admission slot, a directory lock, a slow subprocess) stops terminal input and every other
request on that connection until it finishes. Each request arm chooses its own way of running: some run inline, some use
the admitted-spawn helper, some refuse when busy and then hand the work to a supervisor-owned task and wait for it,
Delete takes its slot inside its own task, Rename carries one permit through two phases, listing has its own limit.
Nothing forces the author of a new request type to make that choice deliberately, and nothing tests that the choice
keeps the loop moving. The "typing freezes on a busy host" chain was fixed one request type at a time (#1164, #1483,
#1486, #1490, #1493; #1490's message: restart "takes its slot on a path of its own, which that change did not reach"),
and the "server owns an accepted action" rule (SPEC_impl.md, "Who owns an accepted action") was missed per handler
(#1195, #1512).

Deliver four things:

1. **One owner for the connection's channel ids.** A connection keeps several connection-local maps keyed by channel id
   (relay, download, input, upload, and a set of channels in use for other purposes on main at 18e51c3e), and the "is
   this channel id already in use" check is repeated with different subsets of those maps at several sites in
   `service/connection.rs` and `service/handlers.rs`, with Attach relying on a check made elsewhere (`dddd9e9e` was one
   such gap). Put channel-id allocation and the in-use check behind one connection-local table type that every
   allocating request goes through, so the check cannot be partial. Same ids accepted and refused as today, in the same
   order relative to other checks; behavior-preserving. Do this first: the classification below has to know which
   requests touch channel state, and this table makes that explicit.
2. **An exhaustive execution-class declaration.** One function maps every `ControlMsg` variant to the execution class it
   uses today, written as a `match` with no wildcard arm, including an explicit "not a request" class for the variants
   that only ever travel the other way (replies, upcalls, hints). The class enum names the modes that actually exist;
   you derive that set from the code, not from this file. Adding a variant then fails to compile until its author picks
   a class, and the class is documented next to the enum with what each mode guarantees and why channel-allocating
   requests must stay inline (below). If the dispatcher's own `match` ends in a wildcard that only exists because
   `ControlMsg` carries both directions, replace the wildcard with the explicit reply variants so the dispatcher is
   exhaustive too.
3. **A conformance test that the read loop keeps moving.** For every variant whose class says it must not hold the loop,
   the test holds every admission slot and a lifecycle claim on the target session (and whatever other shared waits the
   class implies, including the per-key intent lock and the host-wide directory lock), sends that request, and proves
   the connection still processes a following frame (terminal input, or a cheap request) promptly. The test iterates the
   classification function, so a new variant classified as non-blocking is covered without anyone editing the test.
   While the fixes below are pending, the requests that still wait inside the loop are named exceptions in one list;
   each fix removes its entry, and when the stack is done the list is empty.
4. **Fixes for every request that still stalls the loop.** On main at 18e51c3e these are: session creation on both the
   full-authority and the session-authenticated paths, the keyed fresh-checkout reconciliation request the helm sends
   before such a create. Tab attach during a stop was a third; `tab-attach-no-wait.md` fixes it before this plan starts,
   and the conformance test must then cover attach as non-blocking. Your inventory may find more. Each gets its own
   `fix:` PR with a changelog fragment and a deterministic regression in the conformance test (see Decisions already
   made for the sources each one closes).

### Design principles

- The classification describes how each request runs; it does not move execution. Do not make the dispatcher perform
  admission or spawning on a handler's behalf: that reorders checks (Rename validates its title before taking a slot, so
  a bad title on a busy host answers "invalid request", not "unavailable"; Delete takes its fence before its slot) and
  changes replies. Handlers keep their own execution code.
- Requests whose effect later frames on the same connection depend on, in arrival order, stay inline. Attach, the
  begin-upload and begin-download requests, and anything else that allocates or claims a connection-local channel id or
  route must be processed before the next frame is read, or data, resize, detach and abort frames can race them
  (`125a15b9` and `dddd9e9e` show what that ordering protects). Attach also sends its reply through a writer permit it
  reserved before taking its locks, so its reply precedes any data frame from the forwarder it starts; keep that.
- Do not change who sends replies or when guards drop. Moving reply sending into the dispatcher was considered and
  rejected for this plan because it reorders replies against data frames.
- The connection-level pre-dispatch path (relay, download, stat-file handling before `handle_control` in
  `service/connection.rs`) and the separate dispatcher for session-authenticated connections are part of how requests
  run. Either cover their message kinds in the classification or record, with a reason, why they are out of its scope.
- Moving creation off the loop must keep what TODO.md's "Planned" entry lists: bounded handler admission, credential
  revalidation after every wait, keyed-create durability and replay, disconnect cleanup, and the per-key intent and
  host-wide directory locks. SPEC.md lets a create queue behind a Delete that holds the directory lock, so the fix waits
  off the loop; it must not refuse instead. The reconciliation request follows the same rule.
- Tab attach is different from creation: it allocates a channel that the frames after it on the same connection (data,
  resize, detach) refer to, so it can never simply move to a task. `tab-attach-no-wait.md` should already have removed
  its wait. If attach still waits inside the loop on the main you start from (for example because that plan's outcome
  was discarded over its size gate), fix it here in its own `fix:` PR, keeping per-channel ordering (for example, by
  registering the channel inline and moving only the lifecycle wait off the loop, with the channel's later frames
  handled consistently while that wait is pending), and remove its queue item if it is still open. Choose the design
  from the code and SPEC.md, and log it as a DECISION. If no design preserves both the ordering and the spec's attach
  semantics without new protocol, that is a block (see Fallbacks), not a reason to weaken either.

### You find the instances

The file and line references in this plan are illustrations from main at 18e51c3e, and other work keeps landing while
this plan waits in the queue. Do not treat them as a worklist. Your first step is an inventory, on the main you start
from: every `ControlMsg` variant, every place a control message is dispatched (both dispatchers and the connection's
pre-dispatch handling), and the execution mode each request actually uses today, read from the code. Record the
inventory in the working log, re-check it after every rebase onto a newer main, and summarize it in the report.

### Acceptance criteria

- Channel-id allocation and the in-use check go through one table type; no allocating request checks a subset of the
  maps by hand.
- Every `ControlMsg` variant is classified by one exhaustive function with no wildcard arm; each class is documented.
- The conformance test exercises every variant classified as not holding the loop, through the classification function,
  and fails when such a request stalls a following frame. At the end of the stack no request is an exception; the
  exceptions mechanism may be removed then if nothing uses it.
- Each stall fix keeps the guarantees listed under Design principles and is proven by a deterministic regression.
- Outside the `fix:` PRs: no behavior changes (same replies, same reply and frame order, same admission and refusal
  outcomes, same error messages). The no-functional-change reviewer confirms this for every PR, and confirms that each
  `fix:` PR is a clear fix of the targeted class.
- The creation fix removes TODO.md's "Planned" entry "Keep session creation off the connection read loop". The last code
  PR removes this plan's TODO.md entry (under "Code cleanup").
- Docstrings and comments explain the classification, each class's guarantee, and the ordering rule for inline requests,
  per the repository's documentation norms.

## Decisions already made

The maintainer's words when asking for this plan (2026-10-19): "Make sure that all plans explicitly call for a special
reviewer beyond the usual that ONLY focused on making sure there is no functional difference (the only exception is
CLEAR bugfixes that fix broken behavior that precisely fixes the kind of bug the refactor was intended to prevent in the
first place). Please make sure the plan contains the key design principles, but that the executors still must find all
the exact specific _instances_ where to apply them, since other work is landing while this is being planned and queued
etc. For this work, I want an astra high adversarial reviewer (both for the general adversarial review as well as the
extra no-functional-change reviewer)."

- **No behavior change, with one exception.** A behavior change is allowed only when it is a clear fix of broken
  behavior of exactly the class this refactor targets: a request that waits inside the read loop when its class says it
  must not, or an accepted mutation that a disconnect can leave half done. Such a fix goes in its own `fix:` PR, with a
  changelog fragment per root `AGENTS.md`, "Changelog fragments", and the no-functional-change reviewer must confirm it
  qualifies. Anything else you notice goes in the report's follow-ups.
- **Known stalls are fixed here.** When asked whether to leave them to their own TODO entry and queue item, the
  maintainer answered (2026-10-19): "unless there is a reason not to, tell me. are they special in any way other than it
  happens to have its own todo item? if not, then fix them here". Creation and the reconciliation request are not
  special beyond their size: they are exactly this class, and TODO.md's "Planned" entry (which this plan now covers, and
  which names `checkout-reconciliation-blocks-terminal-reader.md` and the `create-runs-inline-on-read-loop.md` triage
  outcome) lists what the fix must preserve. Tab attach is special only in its ordering constraint (Design principles),
  which bounds the design rather than excluding it; after this plan was written, its triage outcome was scheduled as
  `tab-attach-no-wait.md`, so this plan waits for that one instead of fixing attach twice. Any other stall your
  inventory finds is fixed the same way, each in its own `fix:` PR.
- **The channel table is in scope**, as the first PR. The maintainer: "i dont see why leave it out from what you said,
  seems simple".
- **Reviewers:** both per-PR reviews run on gpt-6-astra at high effort (see Review gate).
- **If gpt-6-astra is unreachable** when a review is due (CLI missing, quota, outage): block the plan per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. No substitute
  reviewer.
- No-workhorse mode, which `plans/.agents/planning.md` requires.

## Requirement sources

**The user's request and decisions:** above. The plan's origin is a 2026-10-19 whole-codebase structural assessment
whose adversarial review ranked this first of five: the busy-host chain shows that one design decision needed several
PRs because each request type chooses its own mode, and a new request type still has no single place that forces the
choice.

**Binding repository constraints:** SPEC_impl.md, "Who owns an accepted action" and "Supervisor internals"; root
`AGENTS.md`, "Code rules" (no per-harness branching in shared code), "Tests and validation", "Conventional Commits",
"Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md` for the conformance test;
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` (the test must wait on conditions, not sleeps).

**Planner proposals:** the shape of the classification and test above. A smaller design that meets the same acceptance
criteria is fine, logged as a DECISION.

## Outline

A suggested stack; reshape it if the inventory says otherwise:

1. `refactor:` the connection's channel table.
2. `refactor:` the classification function, the class enum with its documentation, and (if applicable) the explicit
   reply arms replacing the dispatcher's wildcard. No handler moves.
3. `test:` the conformance test, iterating the classification, with the named exceptions list as it stands on main.
4. `fix:` creation off the read loop (both create paths), removing its exception and TODO.md's "Planned" entry.
5. `fix:` the fresh-checkout reconciliation request off the read loop (may fold into 4 if it shares the mechanism).
6. Only if attach still waits on the main you start from: `fix:` tab attach, per Design principles.
7. Any further `fix:` PRs for stalls the inventory found. The TODO.md "Code cleanup" entry removal rides on the last
   code PR.

Facts from the planning session's reading of main (verify them): the read loop is in `service/connection.rs` (around
619-700), the dispatcher `handle_control` around handlers.rs:2527 ends in `other => warn!(..)`, admission helpers
(`spawn_admitted`, `admit_or_refuse`) live in `connection.rs` around 1129-1180, Delete takes its slot in its own task
(handlers.rs around 1081-1095), Rename's two-phase permit around 2185, attach around 1233-1341, the
session-authenticated dispatcher around 2997.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the new conformance test and the supervisor service tests that cover
dispatch and admission, through `python3 scripts/record-test-run.py` with a narrow selection, and the test-sleep check.
Run the conformance test repeatedly under load before trusting it; a flaky enforcement test is worse than none.

## Fallbacks

Examples of departures that need the scope reassessment: moving a handler off the read loop other than as a stall fix
above, changing how replies are sent, a new dispatch framework or trait object per request, classifying by anything
other than the message variant, or a protocol change.

Agreed fallback for tab attach, if it is still this plan's to fix: if no design keeps per-channel ordering and SPEC.md's
attach semantics without a protocol change, push everything else (including the other fixes), leave attach as the one
named exception pointing at its queue item, and block with a question that describes the candidate designs and what each
would change.

Decisions to log: the channel table's shape, the set of classes and why, how the test holds the waits, how the
pre-dispatch path and the session-authenticated dispatcher are covered, the design of each stall fix and how it keeps
the listed guarantees, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-supervisor-request-modes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel (the inventory splits naturally), or
implementation split across agents. Whether to use them is your call. Concurrent agents that might otherwise conflict
can run in separate jj workspaces whose results you merge back; you still own the stack and every PR. A sub-agent whose
model is not available as a native sub-agent type, the reviewers included, is launched through
`$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) are exempt from these rules and run as
that file says.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/supervisor-request-modes/<nn>-<short-name>` (for example `plan/supervisor-request-modes/01-request-classes`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `refactor` for behavior-preserving restructuring, `test` for a change only to tests or their fixtures,
  `fix` (with a changelog fragment in the same commit) only for the clear-bugfix exception, `docs` for documentation
  only. A `fix` never shares a PR with refactoring.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch two reviews of its changes yourself and address what each finds before moving on. The
user demands exactly these reviewers, and no review swarm. Each is a single fresh-context agent on gpt-6-astra at high
effort, launched through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns
the launch mechanics). Run them independently; neither sees the other's findings. If gpt-6-astra cannot be reached,
block as Decisions already made says; never substitute another model.

**1. General adversarial review.** The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

**2. No-functional-change review.** The prompt carries this full charter:

> This PR is part of a structural refactor whose contract is that it changes no behavior. Your only question is whether
> this PR changes observable behavior compared with its parent commit. Ignore style, naming, design quality and test
> quality, except where a test change hides or accommodates a behavior change. Observable behavior includes: wire
> messages and their order; replies, error kinds, error messages and their precedence; HTTP statuses, headers and
> bodies; what is rendered, escaped, copied to the clipboard or put in URLs; which await, lock acquisition or check
> happens first; when a permit, guard or lock is released; whether work is cancelled when a client disconnects or a
> connection closes; database writes and their transaction boundaries; panics versus errors; logs that tests or
> operators rely on. For every difference, report the file, the quoted code before and after, a concrete scenario that
> behaves differently, and whether it is an unintended change to revert or a candidate for the exception. The exception
> applies only when all of these hold: the old behavior is clearly broken rather than a choice that SPEC.md,
> SPEC_impl.md or a recorded decision endorses; the new behavior fixes exactly the bug class this refactor exists to
> prevent, quoted below; and the PR is a `fix:` PR that contains only that fix and a changelog fragment. A behavior
> change inside a `refactor:` or `test:` PR is always a finding, even when it is a good fix. If you find no difference,
> say so and list what you checked. Report to the named file; edit nothing; touch no VCS state.

Each prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range of this one PR;
- a findings file in the scratch directory, one per reviewer;
- the acceptance criteria: this file's goal, design principles, Decisions already made and the outline; for the
  no-functional-change reviewer, quote the targeted bug class from Decisions already made.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the general
review's prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION; a no-functional-change finding you reject
needs the concrete reason the scenario cannot occur. Do not write a launch command here or in the log; the
harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular those named under Fallbacks above, every use of the clear-bugfix exception, and
every review finding you decided not to follow. The user will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed both reviews of its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
