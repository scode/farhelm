# UI: one shared Replace/YOLO action controller

Written against main at 18e51c3e on 2026-10-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `ui-peer-text-type.md`, which itself runs after `host-entry-session-ids.md`: the queue holds it
back until `ui-peer-text-type.md` has landed on main. Both earlier plans rewrite the sidebar and session header code
this plan restructures. It also runs after these plans, each of which changes behavior in code this plan consolidates; a
behavior-preserving refactor must preserve their results, not race them:

- `replace-yolo-shared-question.md` makes the sidebar's and header's Replace controls share one YOLO question per
  session.
- `launcher-retry-identity.md` changes when every launcher, Replace included, keeps or drops a retry's identity after a
  refusal (the decision this plan's outcome enum encodes).
- `launcher-composer-drafts.md`, `launcher-enter-feedback.md` and `launch-sequences.md` change the session launcher
  (`list/create_form.rs`), whose manual operation-lock releases this plan converts.
- `select-newest-on-close.md` changes which session opens when the open one goes away, which the Replace and Delete
  flows trigger.

It is one of five structural refactor plans written together (`supervisor-request-modes`, `helm-owned-by-default`,
`helm-ingress-types`, `ui-peer-text-type`, `ui-session-actions`).

## The goal

This is a structural refactor: it changes no behavior, except for the clear fixes described below, each in its own PR.
Its purpose is to stop one UI flow from existing in several hand-maintained copies.

Replace exists in two copies: the sidebar's (`crates/farhelm-ui/src/list/view.rs`, around 2330-2440 on main at 18e51c3e)
and the session header's (`crates/farhelm-ui/src/session_view.rs`, around 1580-1670). Each keeps or mints the retry key,
builds the YOLO question, settles the attempt and handles errors; the pending-replace type is defined twice (`view.rs`
around 505, `session_view.rs` around 493); there are separate "take the YOLO answer" helpers in several components. The
refusal types the API layer returns (`api.rs`, around 2308-2400) carry independent yes/no flags (answered, YOLO
confirmation, stale), and each caller works out "keep the retry key?" from their combination. The same cancel race was
fixed in three places in one day (#1658, #1659, #1660); the YOLO consent flow has a chain of about a dozen fixes (#1384
through #2251, including #1394 and #1396, the same fix for the header and then the sidebar); the retry chain runs #1908
to #2206.

The UI's operation lock (`crates/farhelm-ui/src/ops.rs`) also offers manual `claim()` and `release()` beside a guard
whose drop clears the lock unconditionally. Today every manual release sits right before a return, so this is a latent
trap rather than a live bug: a manual release followed by an await would let a second operation claim the lock, and the
first guard's drop would then clear the second claim.

Deliver, in this order:

1. **A guard-only operation lock.** Remove the public manual `claim()` and `release()`; the guard is the only way to
   hold the lock. With manual release gone at most one guard exists at a time, so no generation stamp is needed. Where a
   caller released early today, release at the same point with an explicit `drop` of the guard, so release timing does
   not move.
2. **One Replace controller.** A plain (non-component) type that owns the retry-key slot, the pending YOLO question and
   the precondition guard for Replace, used by both the sidebar and the session header, with one pending-replace type
   and one YOLO-question type in place of the duplicated helpers. How the two surfaces share it follows whatever
   `replace-yolo-shared-question.md` put on main, so behavior is unchanged (see Decisions already made). Restart and
   Delete join only if your inventory finds the same flow duplicated there; log that as a DECISION either way.
3. **An outcome enum.** Where the refusal flags drive the retry-key decision, decode them once into an enum of the
   outcomes that actually occur (for example: not answered, refused, needs YOLO confirmation, stale), and make the
   controller decide from the enum.

### Design principles

- One copy of each rule. After this plan, a fix to the Replace flow, the YOLO question or the retry-key decision lands
  in one place.
- Behavior is unchanged: the same requests in the same order, the same retry-key keep-or-mint decisions, the same
  questions, focus handling and refresh triggers, the same timing of lock release.
- The controller is plain Rust holding the signals the component creates, so its decisions can be unit-tested without a
  browser.

### You find the instances

The file and line references in this plan are illustrations from main at 18e51c3e, and other work, including the two
plans this one runs after, keeps changing this code. Do not treat them as a worklist. Your first step is an inventory,
on the main you start from: every caller of the operation lock's manual API; every copy of the Replace flow; every type
and helper that holds or answers a YOLO question; every site that combines the refusal flags to decide about the retry
key; and whether Restart and Delete repeat the Replace flow. Record the inventory in the working log, re-check it after
every rebase, and summarize it in the report.

### Acceptance criteria

- The operation lock has no public manual claim or release; release timing is unchanged at every converted site.
- Replace's retry-key, YOLO-question and settle logic exists once, in the controller, used by both surfaces; the
  duplicated pending-replace and YOLO-answer types and helpers are gone.
- The retry-key decision is made from the outcome enum, not from flag combinations at call sites.
- Unit tests cover the controller's decisions and the enum's decoding.
- Apart from clear fixes in their own PRs: no behavior change, confirmed by the no-functional-change reviewer for every
  PR.
- The last code PR removes this plan's TODO.md entry (under "Code cleanup").
- Docstrings explain the controller's contract and what it owns, per the repository's documentation norms.

## Decisions already made

The maintainer's words when asking for this plan (2026-10-19): "Make sure that all plans explicitly call for a special
reviewer beyond the usual that ONLY focused on making sure there is no functional difference (the only exception is
CLEAR bugfixes that fix broken behavior that precisely fixes the kind of bug the refactor was intended to prevent in the
first place). Please make sure the plan contains the key design principles, but that the executors still must find all
the exact specific _instances_ where to apply them, since other work is landing while this is being planned and queued
etc. For this work, I want an astra high adversarial reviewer (both for the general adversarial review as well as the
extra no-functional-change reviewer)."

- **No behavior change, with one exception.** A behavior change is allowed only when it is a clear fix of broken
  behavior of exactly the class this refactor targets: copies of the same flow that disagree, or an operation lock left
  claimed or cleared wrongly. Such a fix goes in its own `fix:` PR, with a changelog fragment per root `AGENTS.md`,
  "Changelog fragments", and the no-functional-change reviewer must confirm it qualifies. Anything else you notice goes
  in the report's follow-ups.
- **The create form's unmount fix is its own PR.** Converting the session launcher (`list/create_form.rs`, which holds
  all 17 manual `release()` calls on main at 18e51c3e) to guard-only use means an unmounted form releases the lock
  instead of stranding it. That is a clear fix of the class and goes in its own `fix:` PR, separate from the
  behavior-preserving conversion.
- **The controller preserves today's sharing between surfaces.** The maintainer chose per-surface instances so this plan
  would not itself change whether the sidebar and header share a YOLO question. That question was then triaged and
  scheduled as `replace-yolo-shared-question.md`, which this plan runs after. Model whatever it put on main: if the two
  surfaces' questions are mutually exclusive, keep per-surface instances with that exclusion; if the question was
  hoisted to page level, the controller owns that one question. If that outcome was discarded over its size gate, keep
  per-surface instances with today's behavior and leave its queue item alone.
- **Retry identity follows `launcher-retry-identity.md`.** That plan changes which refusals retire a retry key, for
  Replace and every launcher. The outcome enum encodes the rules as they are on main after it, not as this file
  describes them.
- **Reviewers:** both per-PR reviews run on gpt-6-astra at high effort (see Review gate).
- **If gpt-6-astra is unreachable** when a review is due (CLI missing, quota, outage): block the plan per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. No substitute
  reviewer.
- No-workhorse mode, which `plans/.agents/planning.md` requires.

## Requirement sources

**The user's request and decisions:** above. The plan's origin is a 2026-10-19 whole-codebase structural assessment;
this plan was kept in the final five because the Replace/YOLO flow is the UI's largest source of fixes, with the
guard-only lock first so value arrives early and the risky part stays incremental.

**Binding repository constraints:** SPEC.md's Replace, Restart and YOLO-consent passages; SPEC_impl.md's GUI passages on
retry keys, the precondition guard and the operation lock; root `AGENTS.md`, "Tests and validation", "Conventional
Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md` for tests.

**Planner proposals:** the controller and enum shapes above. A smaller design that meets the same acceptance criteria is
fine, logged as a DECISION.

## Outline

A suggested stack; reshape it if the inventory says otherwise:

1. `refactor:` guard-only operation lock, converting callers outside the launcher with release timing unchanged.
2. `fix:` the launcher's conversion, releasing the lock when the form unmounts, with a fragment and a test.
3. `refactor:` the outcome enum, decoded once in the API layer, with callers converted.
4. `refactor:` the Replace controller, one surface at a time if that keeps the PRs reviewable, removing the duplicated
   types and helpers.
5. The TODO.md entry removal rides on the last code PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` (including the UI's web target), the `farhelm-ui` unit tests through
`python3 scripts/record-test-run.py`, and the browser specs covering Replace, Restart, YOLO confirmation and retry from
both the sidebar and the session header, narrowly selected.

## Fallbacks

Examples of departures that need the scope reassessment: sharing one controller instance across surfaces, an app-wide
session store, extending the controller to the session launcher's create flow, or a generation-stamped lock.

Decisions to log: whether Restart and Delete joined the controller and why, the outcome enum's variants and how each
flag combination maps to one, every converted lock site and how its release timing was preserved, every clear fix taken,
and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-ui-session-actions-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewers included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/ui-session-actions/<nn>-<short-name>`
  (for example `plan/ui-session-actions/01-guard-only-lock`).
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
