# A Stop during an agent's start says it was stopped, not that systemd failed

Written against main at 5331b984 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Fix the misleading message when Stop interrupts a starting agent.** A Stop pressed while the agent is still starting
> is recorded as a launch that never started, and its message wrongly blames the host's systemd user manager. The
> message should say what happened: the user stopped the session before its agent finished starting. Found while
> reviewing the stop-closes-tabs plan on 2026-11-08; the behavior predates that plan.

The maintainer picked this entry on the planning picker and, on the planning page, chose that such a session shows
"Exited — stopped before the agent finished starting".

How it works today, from a read of main at 5331b984 (line numbers approximate; correct them to the truth as you go):

- **The "never started" wording** lives in one supervisor function in
  `crates/farhelm-supervisor/src/service/launch_artifacts.rs` (around 147-201). It fires when the agent's terminal is
  dead, the exec shim never wrote its failure report, and the launch spec was never read. It picks one of three
  messages: the systemd scope wrapper exited (around 191-192, "the host's systemd user manager may have refused the
  scope"), the login shell exited before running Farhelm's launch command (around 196-198), or the older scoped-launch
  wording (around 180-182). Up to five lines of terminal output may be appended.
- **Its callers:** the periodic observer (`service/status.rs` around 853), Stop's branch for an agent that already ended
  (`service/handlers.rs` around 1035), and the reload after a supervisor restart (`service/core.rs` around 7312).
- **Stop while the agent is starting:** the terminal still looks alive, so Stop takes its normal path (`handlers.rs`
  around 963-974 into `stop_live_agent`, `service/sweep.rs` around 1947). It records the stop intent, durably and in
  memory (`sweep.rs` around 1965; `core.rs` around 18845-18866), kills the process tree (around 1976-1988, which can
  take seconds on a scoped launch), then records the stopped exit (around 1993).
- **The race:** if the observer runs between the kill and the recorded exit, it sees a dead terminal, an unread spec and
  the wrapper marker, and produces the systemd message. The store turns that into an Error even over the pending stop
  (`store.rs` around 463-474), and the stopped exit that arrives afterwards is ignored because an error is final (around
  490). The observer reads the recorded state (`status.rs` around 777-782) but uses it only to decide whether a read is
  worthwhile; that check deliberately lets a pending stop through (`launch_artifacts.rs` around 240-249). The reload
  path has the same gap: the never-started check (`core.rs` around 7312) runs before the stop-intent handling (around
  7413-7477). Restart and Replace go through the same stop path (`sweep.rs` around 1971), so their old launch can hit
  the same race. This timing is inferred from code order and was not reproduced while planning.
- **The checkout-preparation inference** comes right after the wrapper one, in the observer (`status.rs` around 873-881)
  and the reload (`core.rs` around 7327-7348). For a session cloned into a fresh GitHub checkout, preparation state is
  published as not started before the terminal spawns (`launch.rs` around 1568-1571), so once the wrapper inference is
  skipped, the same Stop would fall through to "checkout preparation did not finish before its terminal stopped"
  (`status.rs` around 747-753) and still commit Error. The classifier's own docs already say callers keep annotated Stop
  outcomes outside this inference (`launch_artifacts.rs` around 141).
- **Stop's ended-agent branch** (`handlers.rs` around 976-1053) runs the same wrapper inference when the pane is already
  dead, and its recorded state can be a pending stop when an earlier Stop recorded its intent and then failed its sweep.
- **The annotation** is a constant inside the store's pure `Transition::apply` (`store.rs` around 447,
  `farhelm_proto::STOP_ANNOTATION`). Three transitions write it: `ObservedExit` / `RediscoveredExit` from a pending stop
  (around 504-507), used by the observer (`status.rs` around 692-696) and both reload branches (`core.rs` around
  7413-7421 and 7455-7468), and `StopCompleted` (around 516-518) from `stop_live_agent` (`sweep.rs` around 1992-1995).
  `enrich` keeps whichever annotation committed first (`store.rs` around 437). A shim failure report blocks nothing once
  an annotated Exited is committed (around 480-487).
- **On the wire** the annotation is free text (`crates/farhelm-proto/src/lib.rs` around 1096-1109, rendered verbatim).
  The one exception is compact mode's ended icon, which matches the exact string "stopped by user"
  (`crates/farhelm-ui/src/list/row.rs` around 1198); SPEC.md (around 2365-2366) requires distinct stopped and exited
  icons there.
- **The launch spec as a signal:** only the shim unlinks the spec after reading it (an unlink failure is fatal there,
  `launch.rs` around 860-866 and 1153-1169), or the supervisor after a durable classification, or the startup sweep,
  which keeps the current generation. At every place a stopped exit is recorded the pane is already dead or killed, so
  the spec's presence is frozen. A present spec also covers a shim that read it but was killed before exec; "never
  started" is still true there.
- **What the user sees:** the session row badge and the session header show "Error — <detail>"
  (`crates/farhelm-ui/src/status.rs` around 194-197). A completed user Stop shows "Exited — stopped by user" (`store.rs`
  around 504-507).
- **Specs:** SPEC.md "Status" (around 2345-2351) says Error means "the agent process could not be started at all" and "A
  user-initiated stop yields exited with an annotation"; "Creation" (around 1268-1272) gives the same split.
  SPEC_impl.md (around 3079-3088) describes how the failed launch step is told apart and that "a completed user Stop
  keeps its annotation", and says nothing about a stop still in progress.
- **Tests that pin today's behavior:** `launch_artifacts.rs` tests around 1137-1234; `session_lifecycle.rs` around 6211
  (a planted wrapper failure expecting the systemd wording) and around 6388 (a Stop during an unscoped shell startup
  that keeps "stopped by user"); store rules around `store.rs` 7013-7086; `e2e/tests/terminal.spec.ts` around 1690-1694,
  whose comment accepts "it never started" for a Stop before the agent is ready.

### Acceptance criteria

- **A Stop during start is an Exited session**, annotated "stopped before the agent finished starting" (the UI shows
  "Exited — stopped before the agent finished starting"), whenever the user's stop was recorded before the launch was
  classified and the agent never read its launch spec. This holds through the observer path and through the reload after
  a supervisor restart, on scoped and unscoped launches, local and remote hosts, old tmux terminals and terminal hosts
  alike.
- **A Stop after the agent started** keeps today's "stopped by user".
- **One rule for every caller:** while the recorded state is a pending stop, no pane-based inference runs (not the
  wrapper, the login shell, nor the checkout-preparation inference), in the observer, in both reload branches and in
  Stop's ended-agent branch. This makes the outcome independent of whether the observer or Stop commits first.
- **Real failures still win while the stop is pending:** a failure report the exec shim wrote is direct evidence, and
  while the recorded state is still a pending stop it yields Error with its cause. Do not add a sentinel read to the
  stop path (once Stop's annotated Exited is committed, a later sentinel stays blocked, as today). A launch that had
  already failed before Stop was pressed, with no stop intent recorded, keeps its never-started cause.
- **Choosing the annotation:** every transition that writes a stop annotation carries the choice (whether the launch
  spec was still unread), at all four places: the observer, the reload's dead-pane and no-pane branches, and
  `stop_live_agent`. The first commit decides, as `enrich` already does. A failed or inconclusive spec read falls back
  to "stopped by user".
- **The compact icon:** the new annotation is a proto constant beside `STOP_ANNOTATION`, and compact mode shows the
  stopped icon for it too.
- **Older pages** show the new text verbatim (the annotation is free text) but the plain exited icon in compact mode,
  since they match only the old string. That is accepted under SPEC.md "Upgrade compatibility and client scale"
  (compatibility is decided per feature); log it as a DECISION. The badge's length cap may ellipsize the longer text
  where it applies; the detail line wraps. That is accepted too.
- **Restart, Replace, remote hosts and terminal hosts** share the changed supervisor code, so they are covered by the
  shared change; no separate reproduction is required.
- **Wording** in SPEC.md "Status" and "Creation" and SPEC_impl.md (around 3079-3088) states the rule, and the
  `terminal.spec.ts` comment around 1690 no longer accepts "it never started" for this case (a comment-only edit).
- **Tests:** deterministic unit tests are the proof of the race fix, since no seam exists between Stop's kill and its
  recorded exit: the observer (pending stop, dead terminal, unread spec, wrapper marker present) expecting the new
  annotation; the same with a checkout-preparation launch; both reload branches; Stop's ended-agent branch with a
  pending stop; a shim failure report under a pending stop yielding Error; the store transitions carrying the choice;
  and a UI test that compact mode shows the stopped icon for the new annotation. Tighten `session_lifecycle.rs` around
  6388 (today it checks only that some annotation exists) to the exact new annotation, and add a variant of it with a
  real scope manager instead of the disabled one, skipped where no user manager exists. Update existing tests only where
  they encode the old race outcome.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **The wording** is "stopped before the agent finished starting", shown as "Exited — stopped before the agent finished
  starting". The alternative offered was the plain "stopped by user".
- **Restart and Replace** get the same fix, since they share the stop path (stated on the planning page, not objected
  to).
- **Planner decisions from the planning review**, which the maintainer reviews in the planning PR: the pending-stop gate
  covers every pane-based inference, not only the wrapper and login shell (otherwise a Stop on a fresh-checkout session
  still ends in Error); the stop-annotating store transitions carry the annotation choice; and older pages showing the
  plain exited icon for the new text in compact mode is accepted.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md "Status" (around 2345-2351), "Creation" (around 1268-1272) and "Lifecycle
operations" (around 1605-1611); SPEC_impl.md around 3079-3088; SPEC.md "Upgrade compatibility and client scale" for the
new annotation; the per-harness rule in root `AGENTS.md`, "Code rules" (no harness comparisons at call sites); root
`AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Fix it where the classification happens: gate the pane-based inference on the recorded state at its three callers, so
  the ordinary exit records Stop's outcome. Do not move the stop intent or reorder Stop's steps.
- Carry the annotation choice as one input on the existing stop-annotating transitions (a flag, say) or one new variant
  that maps to the new annotation, whichever reads better in `store.rs`; this store-transition change is authorized by
  this plan. Do not add a new session state.

## Outline

One PR, `plan/stop-start-message/01-stop-during-start`: the supervisor change, the annotation, the tests, the spec and
comment wording, the changelog fragment and the TODO.md removal. Split the spec wording into a second PR only if it
reads better that way.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy, the
supervisor and UI unit tests touched, and, through `python3 scripts/record-test-run.py` with the pinned nextest and tmux
setup, the e2e selections for session lifecycle and the launch-artifact paths; `python -B scripts/check-test-sleeps.py`
with the interpreter `docs/test-sleep-check.md` prepares. The `terminal.spec.ts` change is a comment, so it needs no
browser run.

## Fallbacks

Examples of departures that need the scope reassessment: a new session state, a store change beyond carrying the
annotation choice, changing what Stop does or in which order, adding a sentinel read to the stop path, changing how a
real never-started cause is detected, or changing Delete.

Decisions to log: where the pending-stop check goes, how the annotation is chosen and carried, how older pages see it,
every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-stop-start-message-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/stop-start-message/<nn>-<short-name>`, as
  the outline names them.
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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
