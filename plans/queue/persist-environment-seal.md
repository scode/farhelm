# Remember a sealed environment across helm restarts

Written against main at 63d8ecd5 on 2026-11-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Keep a sealed environment's record across helm restarts.** Since environment-agent-operations (#3334 to #3364), the
> last Delete in a sandbox or container makes the Farhelm inside seal itself, and the helm then destroys the
> environment. The helm's record that an environment is sealed lives in memory only. If destroying it fails and the helm
> then restarts, the helm forgets the seal until it connects to the environment again: until then Hosts shows an
> ordinary sandbox or container with no sessions instead of "Deleting failed" with Retry. A running sandbox reconnects
> and is destroyed then. But when the helm stays down for more than about half an hour, Tensorlake's 30-minute idle
> timeout suspends the sandbox, Farhelm never resumes one on its own, and the sandbox sits in Hosts as an empty sandbox
> billing for snapshot storage until the user notices and resumes or deletes it. The fix is to persist the helm's seal
> record (environment and when the seal was seen) so a restarted helm keeps showing the failed deletion with Retry, and
> can finish the destruction of a suspended sealed sandbox without waiting for a connection. This is helm-side state
> about a session's end, which SPEC_impl.md forbids; the maintainer wants it anyway, so the change also writes that
> exception into SPEC_impl.md. A process inside an environment writing the seal itself is not a trust boundary the
> maintainer cares about (decided 2026-11-18). Recorded by the maintainer on 2026-11-18 from the plan's review.

### Where it stands (from planning, at main 63d8ecd5)

- The helm's seal record is in memory only: `ConnectionManager::sealed` (`crates/farhelm-helm/src/manager.rs`,
  `SealedHost { Pending, Failed(String) }`, with `note_sealed`, `sealed_attempt`, `sealed_outcome`, `sealed_failed`,
  `seal_is_actionable`); its doc says "Not durable: after a helm restart the next listing reports the seal again". While
  a host is in it, the manager reports the host as `HostState::Deleting { failure }`, and the app draws "being deleted",
  with the reason and Retry when the destruction failed, purely from that state.
- Every destruction attempt (the environment session's Delete answered "sealed", Retry, and the seal finisher started at
  helm startup, which retries pending seals every 10 s) goes through `finish_seal_locked`
  (`crates/farhelm-helm/src/sessions/sandbox.rs`) -> `hosts::finish_sealed_environment` -> `forget_host_locked`, which
  calls the provider's `destroy` and, on success, deletes the host row.
- Tensorlake's `destroy` is API-only and treats an already-gone sandbox as success, so a suspended sandbox can be
  destroyed without a connection (`tensorlake/hosts_tests.rs` covers a suspended sandbox's whole-sandbox Delete). The
  gap is only that a restarted helm has forgotten the seal and never dials a suspended sandbox, so nothing reports it
  again.
- The helm database is at schema 88 (`crates/farhelm-helm/src/store.rs`, `SCHEMA_VERSION`; fresh databases are created
  in final shape by one batch ending in a literal `PRAGMA user_version`; migrations are `if version == N` steps; a
  migrated-versus-fresh test compares them). SPEC_impl.md records each helm schema step in a sentence.
- SPEC_impl.md forbids this state today: "add no helm-side record of which sessions an environment holds" in
  "Environment ownership and refusal", "Nothing about it is stored on the helm: after a restart the seal arrives with
  the next listing" in "Environment Delete and the seal", and "the seal is the only finality state" further on.

### Acceptance criteria

1. Once the helm has attempted to destroy a sealed environment, a helm restart keeps showing it as being deleted: on
   startup every remembered seal loads as pending, the finisher tries to destroy it once, and a failure shows "Deleting
   failed" with Retry, exactly as before the restart.
2. A suspended sealed Tensorlake sandbox is destroyed on helm startup without any connection to it.
3. The remembered seal goes away with the host row: a successful destruction, a vanished environment (destroy counts it
   as gone) and the user deleting the host all clear it, in the same transaction as the row.
4. Only the host and the time its seal was first recorded are stored; no failure reason, nothing about sessions. A seal
   reported by an ordinary host is never acted on or stored (unchanged).
5. SPEC_impl.md states the exception the maintainer approved (the one helm-side record about an environment's end, why
   it exists, when it is written and cleared, and that a process inside an environment being able to cause a seal is not
   a trust boundary the maintainer cares about, decided 2026-11-18), amends the three sentences quoted above, and adds
   the helm schema step's sentence. SPEC.md's sentence that the helm finishes destroying a sealed sandbox after its own
   restart may gain "including one it can no longer reach, from its own record of the seal".

## Decisions already made

Settled with the maintainer (TODO entry, 2026-11-18) and on the 2026-11-19 planning pages:

- **The maintainer wants the helm-side state despite SPEC_impl.md's rule**, and the change writes the exception into
  SPEC_impl.md. The trust boundary question is settled (not one the maintainer cares about).
- **On startup, try destroying each remembered seal once; a failure waits for Retry.**
- **Release note:** "No release note needed." (no highlight; the fragment still describes the fix for the full list).
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.
- Stated on the planning page and not objected to: store host and time only; no UI change; ordinary hosts never
  persisted; a new helm schema version.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the picks above.

**Binding repository constraints:** SPEC_impl.md's environment sections (amended here as the maintainer asked); root
`AGENTS.md` "Code rules" on disposable-environment providers (no provider comparisons in shared code; read the module
docs of `crates/farhelm-helm/src/environment/mod.rs`); `.agents/careful-rebase.md` on schema steps;
`.agents/validation.md`; root `AGENTS.md` "Conventional Commits", "Changelog fragments", "Tests and validation";
`.agents/test-authoring.md`.

**Planner proposals** (verified against the code and reviewed by an independent planning reviewer):

- **One write, in `finish_seal_locked`, before each attempt** (Delete, Retry and the finisher all pass through it). That
  covers the case that matters (a failed destruction, usually of an unreachable environment that sends no further
  listing). It gives up only a helm crash between noticing a seal and attempting it, the same kind of gap SPEC.md
  already accepts for a seal the helm never saw.
- **A nullable column on `hosts`** (for example `environment_sealed_at`), following the precedent of
  `tensorlake_pending_create_at`: it goes with the row by construction and needs no change to how host rows are read. A
  separate table with `ON DELETE CASCADE` is the alternative, about the same size; a close call that is yours to make if
  the column turns out to touch more than expected (log it). Written with "keep the first time" semantics.
- **Load at manager start** (`ConnectionManager::start`, which has the store), before actors publish: every stored seal
  becomes `Pending`, filtered by the same rule as `seal_is_actionable` (or left to `finish_sealed_environment`'s own
  refusal). The finisher then acts on it at once.
- **Provider-neutral:** keyed by host; the only per-provider work stays in `EnvironmentProvider::destroy`.

## Outline

One PR: `plan/persist-environment-seal/01-remember-seal` (`fix:`): the schema step (fresh-schema batch, the 88 -> 89
step or the next free number, the constant and the literal), the store write and read, the write in
`finish_seal_locked`, the startup load, the doc comments that call the record "not durable", the SPEC_impl.md and
SPEC.md text, tests, the changelog fragment (`kind: fixed`), and the TODO.md entry removed. About 250-350 lines.

Tests (build on `agent_requests.rs`'s seal tests with `container_fleet()` and `RunningContainers`, the Tensorlake
`FakeApi`, and `rest_harness.rs`'s `FleetBuilder`, which exposes the store before `start()`, so a test can seed a
remembered seal and then start, a natural restart): the schema step and its first-time semantics; clearing with the host
row; a restart with a suspended sealed Tensorlake sandbox (destroyed through the API, never dialed); a restart with a
failing destroy (shown failed with Retry, no client; Retry succeeds once destruction does).

## Fallbacks

- If the startup load cannot run before actors publish without restructuring `ConnectionManager::start`, load right
  after and make sure a host is never shown as an ordinary host in between in a way a test can observe; log it.

Decisions to log: column or table, the schema number used, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-persist-environment-seal-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-persist-environment-seal-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
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
  `plan/persist-environment-seal/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it (a user-visible fix is `fixed`, not `none`, even when the release note answer is "No
  release note needed."). A later PR in this stack that amends work an earlier PR of this stack built edits that PR's
  fragment instead of adding a second one. Where what got built departs from a release note entry (a renamed control, a
  part left out, a limitation it does not mention), the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.
- The helm schema step conflicts only at the end of the migration ladder with another plan adding one: on a rebase onto
  a main that gained a helm schema version, renumber per `.agents/careful-rebase.md`.

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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

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
- whenever the same component has needed repeated corrective review rounds.

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
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
