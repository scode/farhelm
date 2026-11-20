# A later Tensorlake pause no longer inherits Farhelm's old suspend time

Written against main at 0055c6beb on 2026-11-19. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

This plan covers one TODO.md entry and one triage outcome, which are the same work.

The TODO.md entry (bucket "Plan decisions to revisit"), verbatim:

> **Revisit taking the Tensorlake pause attribution fix out of gui-ux-backend-fixes.** Decided on the maintainer's
> behalf by the landing monitor on 2026-11-19, when gui-ux-backend-fixes blocked twice on that one outcome (a later
> Tensorlake pause can show the earlier "Suspended from Farhelm" label and time in Hosts): the plan delivers its six
> other backend fixes without it, and the outcome stays unexecuted in the triage ledger to be scheduled as a plan of its
> own. A first answer had allowed one draft of about 200 lines; the second attempt reached 265 lines and still had a
> race (a Suspend clicked while Farhelm records that the sandbox ran can reuse the first pause's time). The alternative
> was holding the six fixes for one larger pause draft.

The triage outcome is `TRIAGE_OUTCOMES.md`, heading `old-suspension-stamp-labels-later-external-pause.md` (`fix code`),
with its feedback file `review_feedback_queue/old-suspension-stamp-labels-later-external-pause.md`. Execute it per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes", with the size gate this plan's Decisions
set in place of the ledger's.

The bug: after a Farhelm Suspend, a resume outside Farhelm, and a later Tensorlake idle suspension, the host's details
and the sandbox session's card say Farhelm suspended it, at the old time. SPEC.md "Topology" (Tensorlake hosts): "Its
details say since when, and whether Tensorlake suspended it on its own."

### Where it stands (from planning, at main 0055c6beb)

The Farhelm suspend time lives in two places: the helm database column `hosts.tensorlake_suspended_at`
(`crates/farhelm-helm/src/store.rs`, written only by `set_tensorlake_suspended_at`; exposed provider-neutrally as
`ProviderHandle::suspended_at()` in `crates/farhelm-helm/src/environment/mod.rs`), and the in-memory
`HostState::Suspended { suspended_by_farhelm_at, .. }` an actor publishes (`crates/farhelm-helm/src/manager.rs`).

- Writers: the lifecycle record closure in `crates/farhelm-helm/src/tensorlake/lifecycle.rs` (Suspend writes `now` when
  Tensorlake changed state, otherwise reuses `binding.suspended_at`, read after the provisioning lock but before the
  connection hold; Resume clears it), and the actor's `AttemptOutcome::Suspended` arm in `manager.rs`, which publishes
  `carried.or(row.suspended_at())`. Nothing else clears the stamp.
- Readers: the actor's carried value and its row reload, the Hosts projection's fallback to the registry row in
  `crates/farhelm-helm/src/hosts.rs` (`state_view`; the rows are read before the snapshots), and the two GUI wordings
  (`crates/gui/farhelm-gui/src/hosts/identity/phase.rs` and `crates/gui/farhelm-gui/src/environment_ui.rs`).
- Concurrency: Suspend holds the connection (`hold_connection`, which publishes the shown state, mints a new
  manager-wide incarnation and waits for the actor to park); Retry and holds arrive as nudges that drop an in-flight
  attempt at an await point; store writes run on a blocking thread and complete even when their future is dropped. The
  incarnation also moves on every client install or withdrawal, which is why both earlier trials, which keyed a clear to
  it, kept racing.

The earlier executor's two trials and their reviews are in its working log, `farhelm-plan-gui-ux-backend-fixes-log.md`
in the parent directory of the checkouts (outside the repository; read the pause entries), and in
`plans/queue/gui-ux-backend-fixes.md` "## Decisions" while that file is on main. Trial 1 (95 lines) cleared on a
positive Running guarded by "same incarnation, no hold": Retry and the Hosts read brought the old time back, a withdrawn
actor's guard could erase a replacement's newer stamp, and the already-at-target Suspend wrote back the pre-hold stamp.
Trial 2 (265 lines) keyed a running marker to the incarnation, which Suspend's own hold bumps, so a Suspend finding the
sandbox already re-paused could still reuse the first pause's time; and a failed clear followed by a reconnect exposed
it again.

### The design: retire stamps by time, not by identity

A Farhelm stamp S means "Farhelm's Suspend finished at S". Running evidence observed at time R proves every stamp with S
< R belongs to a suspension that has ended. Farhelm's own Suspend takes its stamp after the hold has parked the actor,
so its S is later than any R the actor gathered before parking. Every write then commutes, and no clear needs an
identity check.

- **Rule:** a stamp counts as Farhelm's attribution for the current suspension only if `S >= R`, where R is the latest
  Running evidence (no R: the stamp counts). `>=`, so two events in the same millisecond never lose a fresh stamp.
- **In-memory R:** a new field on the actor's status (for example `running_seen_at`), max-merged and independent of the
  incarnation, so Retry, reconnects, client installs and holds leave it alone. Set it synchronously right after the
  provider's status call answers Running, before the next await, so a dropped attempt can only lose an observation it
  never made, and any observation made is visible before the actor acknowledges a hold. Carry it when an actor is
  replaced, outside `carry_connection_hold`'s "only when a hold exists" branch (or wherever the replacement is
  installed).
- **The invariant that makes it work, which a correct-looking implementation can break:** (a) R is written with its own
  `send_modify` (or `send_if_modified`), not through the ordinary publish path, because that path drops every write
  while a hold is set; a Running answer polled between the hold being set and the park would otherwise be lost and the
  Suspend race would come back. (b) A durable clear is sent only after its R is in the current actor's memory. That is
  also why the clear cannot move into the Tensorlake provider's status call.
- **Durable compare-and-clear:** on Running evidence, clear the column only where the stored time is `< R`. It can be
  late, duplicated, dropped or run by a dead actor without ever erasing a newer Suspend's stamp; it is re-issued on
  every later Running observation that still sees an older stamp. Gate it in the actor on the existing provider-neutral
  accessor (`handle.suspended_at()` older than R), which is always empty for Podman, so it needs no new provider method
  and writes nothing on an ordinary probe; name the store method neutrally (for example
  `clear_suspend_attribution_before`), keeping to the module map in `environment/mod.rs`.
- **Apply the rule at every reader:** the actor's Suspended publication filters `carried.or(row stamp)` by the rule
  against its in-memory R; the Hosts projection drops its registry-row fallback and uses the snapshot only (the actor
  already loads the saved time on its first Suspended publication after a helm restart, and the lifecycle writes the row
  before publishing its confirmed state), and its `state_view` helper reduces to the snapshot conversion and can go; the
  already-at-target Suspend path, inside the record closure (after the hold has parked), reads R from the manager (a
  small accessor, or a field on the existing host status) and keeps the already-read `binding.suspended_at` only if it
  is `>= R`, otherwise `now`, written with the existing setter. No new atomic store statement is needed: the
  provisioning lock serializes the only other writer of the column, and the only write that can land between the read
  and the closure is the compare-and-clear, which removes only stamps older than an R no later than the one the closure
  reads.
- **No schema change.** The compare-and-clear makes "S < R" durable by deleting S, so R itself is never stored.

Accepted residual limits (to state in SPEC_impl.md): a resume and re-pause Farhelm never observes (the helm not running,
or both inside one 45-second re-probe while suspended) still shows the old time; a backwards wall-clock step can
misattribute, affecting wording only; a Running answer delivered late over the network (after a Suspend's confirming
read) can retire that fresh stamp, and a stamp taken in the same millisecond just before a Running observation is kept,
both affecting wording only; a durable clear that fails while storage is failing, followed by a helm restart and an
outside pause, shows the old time (memory masks it until the restart).

### Acceptance criteria

1. After a Farhelm Suspend (stamp S1), a resume outside Farhelm that Farhelm observes, and a later Tensorlake pause,
   both the Hosts details and the sandbox session's card attribute the pause to Tensorlake with its own noticed time,
   and the saved stamp is cleared.
2. None of these bring the old time back: Retry during reconnection, a Hosts read in progress, a stale row reload, a
   clear that lands late or fails (simulated by writing S1 back into the row), an actor replacement.
3. A Suspend pressed while Tensorlake has already paused the sandbox by itself, after Farhelm saw it running, issues no
   Tensorlake suspend call (already at target) and shows "Suspended from Farhelm" at the time of that Suspend, never S1.
4. A repeated Suspend on a sandbox Farhelm itself suspended keeps its original time; Farhelm's own Resume still clears
   it; a Suspend's stamp survives a helm restart as today.
5. SPEC_impl.md's text on the stored suspend time and on the Hosts list's use of it says how attribution is decided now
   and states the accepted residual limits. SPEC.md needs no change unless its wording contradicts the result.
6. The PR removes the feedback file and its `review_feedback_queue/INDEX.md` line, records the execution in the ledger
   entry and moves it to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (`review_feedback_queue/AGENTS.md`, "Lifecycle"), and
   removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning page:

- **Which fix:** the full fix above, in one change, allowing about 300 changed lines of code, tests and docs (not
  counting `TRIAGE_OUTCOMES.md`, the archive move, the queue removal, TODO.md or the changelog fragment). This replaces
  the ledger entry's 100-line size gate. Well past about 300 lines (say over 350), block rather than widening the fix.
- **Suspend on a sandbox Tensorlake had already paused by itself:** "Suspended from Farhelm" at the time of that
  Suspend, as today without a recorded time and as SPEC.md's "Farhelm's own completion time" says.
- **A wake-and-repause Farhelm never saw:** accept it and say so in SPEC_impl.md. Do not use Tensorlake's `snapshot_id`
  or any other undocumented per-suspension marker.
- **The session card's "Suspended outside Farhelm" wording when no Farhelm time is recorded** (which may conflict with
  SPEC.md's "without inventing who suspended it"): out of this plan. Leave it; mention it under the report's possible
  follow-ups.
- **Release note:** "No release note needed." (no highlight; the fragment still describes the fix for the full list).
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the triage outcome's assessment and completion criteria
(with the size gate replaced as above), and the picks above.

**Binding repository constraints:** SPEC.md "Topology" (Tensorlake hosts: since when, and whether Tensorlake suspended
it on its own; an explicit Suspend shows Farhelm's own completion time); SPEC_impl.md on the stored suspend time and the
host list; the provider module map in `crates/farhelm-helm/src/environment/mod.rs` and root `AGENTS.md` "Code rules" (no
provider comparisons in shared code); `review_feedback_queue/AGENTS.md`, "Lifecycle", and
`review_feedback_queue/.agents/execute-outcomes.md`; root `AGENTS.md` "Conventional Commits", "Changelog fragments",
"Tests and validation"; `.agents/validation.md`; `.agents/test-authoring.md`.

**Planner proposals** (verified against the code and reviewed by an independent planning reviewer, which confirmed the
ordering claim: Suspend always holds before it stamps, the hold is set before the actor is nudged, the actor checks it
before every status call, and both clocks are the same wall-clock milliseconds): the design above, and these tests, all
deterministic and without timing gates:

1. A two-episode actor test (beside `a_suspended_sandbox_is_never_dialed_and_resumes_by_itself` in `manager.rs`, with
   `ScriptedEnvironments`): seed the row with S1, answer Running and connect, then answer Suspended and force a Retry;
   expect no Farhelm attribution, a fresh noticed time, and the row cleared. Then write S1 back into the row and Retry
   again; expect no attribution again.
2. The Suspend race through the fake Tensorlake API (`tensorlake/hosts_tests.rs`, `FakeApi` and `sandbox_helm`): a real
   Suspend (S1), flip the fake to Running outside Farhelm and wait for Connected, put S1 back on the row if the clear
   already ran, flip the fake to Suspended, then Suspend: no suspend call issued, and both the row and the confirmed
   state carry a time later than S1.
3. A store unit test of the compare-and-clear (removes a stamp older than R, keeps one `>= R`, no-op on a row without a
   stamp).
4. The Hosts projection test that asserts the row fallback (`hosts.rs`,
   `a_suspended_view_preserves_confirmation_and_recovers_attribution_from_the_row`) narrowed: drop the row assertions
   and keep the snapshot conversion's checks (observation time, diagnosis, confirmed attribution). Once the row no
   longer reaches the state, "an older row cannot invent the attribution" follows from the code's shape; no new
   projection harness.

Note: existing tests seed stamps near the epoch (10, 30). Any that observes Running before asserting such a stamp will
now see it retired, because R is real wall time; adjust their seeds rather than the rule.

## Outline

One PR: `plan/tensorlake-pause-attribution/01-retire-old-suspend-time` (`fix:`): the design, the tests, the SPEC_impl.md
text, the ledger update and archive move, the feedback file and index line removed, the TODO.md entry removed, and the
changelog fragment (`kind: fixed`).

## Fallbacks

- If the synchronous record of R cannot sit before the next await at the status call without restructuring `attempt`,
  record it at the first point that is synchronous with the Running answer and log why; the property that matters is
  that an observation is visible before the actor can acknowledge a hold.
- If recording R also on a fresh supervisor connection (proof the sandbox runs when the status call went unanswered)
  turns out necessary for an acceptance criterion, add it; otherwise leave it out and log the choice.
- If a test cannot reach a state without a timing gate, restructure the test, never add a sleep
  (`docs/test-sleep-check.md`).

Decisions to log: where R is recorded and how invariant (a) and (b) are kept, how R is carried across actor replacement,
the store statement, the final line count against the gate, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tensorlake-pause-attribution-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-tensorlake-pause-attribution-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/tensorlake-pause-attribution/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it (a user-visible fix is `fixed`, not `none`, even when the release note answer is "No
  release note needed."). Where what got built departs from a release note entry, the fragment's context paragraphs say
  how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The PR records the outcome's execution, archives its ledger entry, removes the feedback file and index line, and
  removes this plan's TODO.md entry, as acceptance criterion 6 says.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, design,
decisions and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md`
verbatim in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds. This one has a history: two earlier trials
  of this fix each needed another round. If review finds a race the design above does not close by construction, run the
  reassessment before patching it.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above within the size gate, each having passed its review gate. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
