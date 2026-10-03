# Execute the 2026-10-02 crash and lock triage outcomes, plus two systemd queue cleanups

Written against main at 9e1dde03 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/queue/triage-crash-replies-and-locks.md` ``, exactly as root `AGENTS.md` section "Execute triage
outcomes" prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack,
in this order:

1. `delete-skips-scoped-tab-on-stale-verdict.md` (other: queue cleanup only)
2. `create-rollback-orphans-unconfirmed-scope.md` (other: queue cleanup only)
3. `stop-restart-panic-no-reply.md` (fix code)
4. `host-write-lock-split-on-actor-respawn.md` (fix code)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms proposed to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 4 draft PRs exist, stacked in the order above (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, as refined by the plan-time decisions below.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field.
- PRs 3 and 4 have passed the review gate.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "now use the planning system to plan execution of what we triaged", after a triage session on
2026-10-02 that recorded these outcomes (landed in #1475).

**The user's triage decisions (2026-10-02),** recorded in full in each ledger entry's Decision field:

- Items 1 and 2: on Linux a working systemd user manager is simply required, with no fallback. Both findings are folded
  into the TODO.md `Maybe later` entry "Require a systemd user manager on Linux, with no fallback", which already landed
  with the triage record and names both cases (including that a failed create must fail visibly and keep its session
  when its scope kill is unconfirmed). What remains here is removing the two feedback files and their index lines. No
  code, spec or TODO change.
- Items 3 and 4: "crash the whole thing whenever it's simpler, so long as it's truly an internal crash/bug. but if it's
  trivial to make something resilient without screwing up the UX, do that." Both were found trivial to make resilient,
  and for item 4 making the crash fatal would contradict SPEC_impl.md's deliberate retire-and-revive design for a
  panicked host worker.

**The user's plan-time decisions (2026-10-02):**

- P1. Review gate: one fresh-context Opus 5.5 reviewer at high effort for PRs 3 and 4, with the general charter below.
  PRs 1 and 2 are queue cleanup only and get no review.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the per-item mechanisms below.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live
install is off-limits), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

## Per-item outline

Line numbers drift; find the code by name.

1. **Queue cleanup.** `docs:`. Remove `review_feedback_queue/delete-skips-scoped-tab-on-stale-verdict.md` and its index
   line, and update its Execution field. Nothing else.
2. **Queue cleanup.** `docs:`. The same for `create-rollback-orphans-unconfirmed-scope.md`.
3. **A crashed Stop or Restart still answers.** `fix:`.
   - Today the waiter tasks for Stop and Restart in `crates/farhelm-supervisor/src/service/handlers.rs` only log a
     panicked work task and send no reply, while Delete's waiter sends `ErrorKind::Internal` ("the session delete task
     failed: …"). The helm puts no deadline on that reply, so the UI's Stop or Restart, or an agent's
     `farhelm agent stop/restart`, waits until the connection drops.
   - Mirror Delete: give each waiter the request id and a clone of the reply channel, and on a panicked join send an
     `Internal` error. Restart's message says the outcome is unknown, since the panic may have come after the old agent
     stopped or the new one launched.
   - Tests: a work task that panics produces the error reply, for each of Stop and Restart, using whatever seam the
     Delete panic test (if any) uses; if no seam exists and adding one is more than small, log the DECISION and cover it
     the cheapest honest way.
   - Changelog fragment: `kind: fixed` (Stop and Restart report an error instead of waiting forever if the host's
     supervisor hits an internal fault while carrying them out).
4. **The cache-write lock survives a host worker restart.** `fix:`.
   - Background: the finding's headline consequence (a host edited or removed mid-install after its worker restarted)
     was fixed after the review by #1165 and #1167, which added the per-host provisioning lock in a manager-level map
     keyed by host id (`provision_locks`, `crates/farhelm-helm/src/manager.rs`), with the test
     `the_provisioning_lock_survives_actor_replacement`. The cache-write lock is still created per actor in
     `spawn_actor` and looked up through the current handle in `host_write_lock`; a handle is replaced while its host
     still exists only after the actor panicked.
   - Move the cache-write lock into its own manager-level map keyed by host id, created on first use, as
     `provision_locks` does, so `spawn_actor` and `host_write_lock` share one lock per host across actor replacement. Do
     not merge it with the provisioning lock: #1165 split them so a long install never stalls session write-backs and
     refreshes, and the comment beside the provisioning lock says so. Create an entry in `spawn_actor`, so only hosts
     that have a worker get one, and remove it where the host itself is removed; no pruning beyond that.
   - Test it the way the provisioning lock is tested: a lock held across an actor replacement still excludes the new
     actor's writers.
   - Note in the PR description that the finding's text predates #1165 and #1167.
   - Changelog fragment: `kind: none` (only reachable after an internal crash of a host's connection worker) unless you
     find a user-visible effect; log the DECISION.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-crash-replies-and-locks-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and `TRIAGE_OUTCOMES.md` Execution fields on each PR) rather than starting over. If it does not exist, this is a fresh
start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/triage-crash-replies-and-locks/<nn>-<short-name>`.
- One outcome per commit, bookmark and draft PR, in the order listed in The goal. Never combine outcomes. Within this
  run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect, as each item below
  says. Every `fix:` or `feat:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID
  and bookmark before creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- PRs 1 and 2: Markdown only, so `dprint check` on the changed files.
- PR 3: `cargo fmt --all -- --check`, clippy on `farhelm-supervisor`, and focused nextest selections for the stop,
  restart and delete handler tests through the recorder with `--tmux required` where they use tmux.
- PR 4: `cargo fmt --all -- --check`, clippy on `farhelm-helm`, and focused nextest selections for the manager's lock
  and actor-replacement tests.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.
- A PR that only changes Markdown: `dprint check` on the changed files, nothing else.

### Review gate

Before finishing PRs 3 and 4, use the active galaxy-brain skill to delegate a review of that PR's changes. The user
demands the reviewer: a fresh-context agent on Opus 5.5 at high effort, with the general charter below. No review swarm.
The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md` entry, its item above, and
the plan-time decisions that apply to it. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Address what the reviewer finds before moving on;
where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a process-wide panic hook or `panic = "abort"`,
merging the two per-host locks, changing the retire-and-revive design, any systemd change; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the ledger entry, the user decisions above, this outline, the current
diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the panic-test seam for item 3, each changelog kind, and every review finding you
decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. If an item needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing
one plan, step 10). Because the PRs form one linear stack, later items sit on top of the blocked one: finish the PRs
before it, record the question, and close the plan as blocked rather than building past it. If current code or specs
have moved so that a recorded decision no longer applies, that is a question for the user too, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself.

## Done criterion

The plan is complete when all 4 draft PRs exist as one linear stack, each satisfies its ledger entry's Completion
criteria as refined by the plan-time decisions, PRs 3 and 4 passed the review gate, and each PR has updated its own
`TRIAGE_OUTCOMES.md` Execution field and removed its queue item. Open, not merged: merging happens only after the
maintainer has reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
