# Name the real refusal when a provisioned host will not attach

Written against main at cd8bd112 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

When setting up or updating a host from the hosts panel, the last step waits up to 30 seconds for the helm to connect to
the newly installed supervisor. Today it recognizes only success: if the supervisor answers but the helm refuses it
(another protocol version, a different or unverifiable identity, an identity another registered host holds), the step
waits out the full 30 seconds with the host busy and fails with "timed out", while the host's row already shows the real
reason. After this plan, the step stops as soon as the helm refuses the supervisor it just installed and fails with that
refusal and its remedy, while a refusal the host held before the step (often the very protocol skew an update fixes)
never stops it.

Acceptance criteria:

- The attach step fails at once, naming the state and its remedy, when the connection attempt that answers its own
  reconnect ends in `VersionSkew`, `IdentityMismatch`, `IdentityUnverified` or `Duplicate`.
- A refusal published by an attempt that started before the step's reconnect never stops the wait: updating a host the
  manager currently holds as skewed, once the new supervisor speaks the right protocol, completes.
- The skew message carries both protocol versions and builds; mismatch and duplicate reuse the remedies the update
  step's trust check already gives; unverified gets a remedy from the variant's own docs. Peer-supplied text (builds,
  identities) is escaped. The message is not labeled as host stderr.
- The success rule, the 30-second budget for every other outcome, and the "host actor disappeared" failure are
  unchanged. `Unreachable` and `Retired` get no new handling.
- Tests as in the outline. The review feedback file and its INDEX entry are removed, the `TRIAGE_OUTCOMES.md` entry's
  Execution field records the PR, and the TODO.md entry is removed.

## Requirement sources

**The user's request:** "let's plan the following, each in its own plan: ... name the real attach refusal ...", for the
TODO.md `Near term` entry, verbatim as of cd8bd112: "**Name the real refusal when a provisioned host will not attach.**
After setting up or updating a host, the last step waits for the helm to connect to the new supervisor and recognizes
only success. A supervisor that answers and refuses (another protocol version, a different or missing identity, another
registered host's identity) keeps it waiting the full 30 seconds, with the host busy, and the run fails as "timed out"
while the hosts panel already shows the real state; setting up or updating an existing entry whose machine was
reinstalled is the likely trigger. Deferred from triage execution 2026-10-02 because the fix is bigger than it looked:
the attach step must tell a refusal that answers its own reconnect from the one the host held before (often the very
skew an update fixes), and the connection manager publishes no evidence of which reconnect request an attempt answered.
Watching the host's status feed for `Connecting` and then a refusal was tried and reviewed; it misses coalesced updates,
can accept a refusal from an attempt already in flight, and goes blind when the nudge revives a stopped actor (it had
subscribed to the old actor's feed; polling the manager's status, as the step does today, follows a revived actor). A
design one reviewer proposed: the actor stamps each attempt with the nudge revision current when it starts and publishes
that stamp with any refusal, the fresh-window retry returns the revision its nudge set (a revived actor counting from
its own start), and the attach wait stops early on a refusal stamped at or after that ticket; it needs a deterministic
test of an attempt in flight across the nudge. Review item: `review_feedback_queue/attach-reports-generic-timeout.md`
(`TRIAGE_OUTCOMES.md` heading of the same name)."

**The user's decisions:**

- U1. Triage, 2026-10-01 (`TRIAGE_OUTCOMES.md`, heading `attach-reports-generic-timeout.md`): `fix code`, with a
  complexity gate, which tripped and deferred execution. Read that entry's completion criteria; they still apply.
- U2. 2026-10-02: asked why the distinction between an old and a fresh refusal is needed, and told that without it the
  wait would stop on the stale refusal at once on every update of an outdated host, the user said: "lets go with the
  proposal of having a sequence number to the connection". The proposal accepted with it: one counter shared by the
  whole connection manager rather than the reviewer's per-actor nudge revision, so a revived actor needs no special
  case.
- U3. The message reuses the existing wording (the version check's mismatch text with both versions and builds; the
  identity remedies the update step already uses) in the existing step-failure frame ("step N (label) failed: …; rerun
  provisioning to continue"). Proposed to the user, not objected to.
- U4. Review gate: a fresh-context Opus 5.5 reviewer at high effort. No-workhorse mode (see How to run).

**Binding repository constraints:**

- SPEC.md "Errors and diagnostics" (~1385): every failed operation shows an actionable error.
- SPEC_impl.md reconnect cadences and fresh windows (~2202-2226). No passage covers the attach wait; a sentence there is
  optional.
- `review_feedback_queue/AGENTS.md`: addressing a queued item removes it, and its index entry, in the same PR.
- Root `AGENTS.md`: a `fix` PR carries a changelog fragment.

**Planner choices (from the planning review), each a correctness requirement unless marked optional:**

- P1. One `AtomicU64` attempt counter on the connection manager (`crates/farhelm-helm/src/manager.rs`). `connect_phase`
  allocates one number per dial, immediately before the `select!` that races the attempt against the next nudge, and
  returns it with the `AttemptOutcome`. Allocating after the dial or at settlement would let a dial that began against
  the old supervisor carry a post-ticket number.
- P2. Only the four refusal publications (the `set_state` calls for skew, mismatch, unverified and duplicate) write the
  number, into a new field on `ActorStatus` beside the state, exposed on `HostStatus` from the same borrow. Not inside
  `HostState`: inside the variants it would leak into REST/UI serialization and make every 45-second re-probe of a
  skewed host wake every client. Name it distinctly (e.g. `refusal_attempt`); `Connecting { attempt }` already means the
  retry-ladder step.
- P3. `nudge_now` reads the ticket at entry, before deciding between the live and the revive branch, and
  `retry_now_with_fresh_window` returns it. Read after the nudge is sent, a genuine post-nudge attempt can get a number
  at or below the ticket; mismatch and duplicate then never publish again and the fix silently degrades to the timeout.
- P4. Pin one off-by-one convention and document it: `incarnation` stores the pre-increment value of `fetch_add(1)`;
  either stamp `fetch_add(1) + 1` and accept `stamp > ticket`, or stamp `fetch_add(1)` and accept `stamp >= ticket`.
- P5. The wait (`AttachSupervisor` arm in `crates/farhelm-helm/src/provisioning/service.rs`) keeps its success and
  disappearance rules and additionally stops on any of the four refusals whose stamp passes the ticket.
- P6. Message rendering: `BackendFailure::rendered()` prints its second argument as "host stderr" (today's timeout
  already reads "…: host stderr timed out"). Put the whole sentence in the first argument with an empty second one, and
  escape peer text with `peer_text` or the `{:?}` quoting `require_update_trusted` uses. For skew, build
  `farhelm_proto::io::VersionSkew` from the state's fields for its `Display` and append the state's own `remediation`;
  for mismatch and duplicate, reuse `require_update_trusted`'s remedy clauses without its "update is refused" framing.
- P7. The deterministic in-flight test needs a gate in the only dangerous window: after `take_settled_outcome` and
  before the refusal's `set_state` (a new slot in the existing `hold_at_gate` family). A gate at attempt start only
  exercises allocation order.
- P8. Optional, not required: an early stop on `Retired` (any `Retired` seen after the retry returns is current). Leave
  it out unless it is a one-arm addition.

Facts the planning review verified: frozen states (mismatch, duplicate) do publish again after the attach nudge, since
the nudge wakes them and the actor dials afresh; the revive branch works without special handling given P3, except the
rare branch where another caller already revived the actor, which falls back to today's timeout and is never a false
stop; `Unreachable` cannot appear within the 30-second budget because the fresh-window ladder outlasts it.

## Implementation outline

One PR, `fix:`. Line numbers drift; find code by name.

- manager.rs: P1-P4.
- service.rs: P5-P6.
- Tests:
  - Manager unit test with the P7 gate: hold a skew outcome in the gap, call `retry_now_with_fresh_window` and keep the
    ticket, release, assert the published stamp does not pass the ticket, then assert the next publish's stamp does.
    Plus a revive case.
  - The stale-refusal guard (new; `update_plans_and_executes_against_a_skewed_supervisor` does NOT hold a stale refusal:
    its manager state is connected and only the fake backend's probe is skewed). With the `rest_harness` fleet: script a
    skewed protocol, wait for `VersionSkew`, `fleet.edit` the script back to `PROTOCOL_VERSION`, run the update, assert
    it completes. Optionally the frozen analogue with `IdentityMismatch`. The test cadence re-probes hourly, so a
    published refusal stays until the nudge.
  - One table-driven provisioning test over the four refusals: the run fails with a message naming the refusal, not
    "timed out". No elapsed-time assertions.
- Bookkeeping: delete `review_feedback_queue/attach-reports-generic-timeout.md` and its `INDEX.md` line, set the
  `TRIAGE_OUTCOMES.md` entry's Execution field (change ID, bookmark, then PR URL in the same change), remove the TODO.md
  entry, changelog fragment `kind: fixed` (setting up or updating a host whose supervisor the helm refuses now fails at
  once with the reason, instead of after 30 seconds as "timed out").

### What not to build

No change to `HostState`'s variants or to what the hosts panel shows; no per-actor revision plumbing; no status-feed
subscription; no early stop on `Unreachable`; no change to retry cadences.

## Plan-specific notes

### Validation

Likely relevant: `cargo fmt --all -- --check`, both clippy runs, the helm crate's manager and provisioning tests through
the recorder (narrow selections first, `--tmux none` unless a selected test needs tmux),
`python3
releasing/check-changelog.py format`, `dprint check` on changed Markdown, and
`python -B scripts/check-test-sleeps.py` since tests change.

### Decisions to log

The counter's off-by-one convention, the field name, where exactly the number is allocated and returned, the wording of
each refusal message, whether `Retired` got an early stop, and any reviewer finding you declined.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-attach-refusal-reason-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
  `plan/attach-refusal-reason/<nn>-<short-name>`.
- One commit, bookmark and draft PR per PR in the implementation outline, as a linear stack in the order given. Err on
  the side of bite-sized PRs, but do not create churn: code added in one PR and deleted in a later PR of the same stack
  means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to all of this plan's open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits with the types the outline names. A `feat`, `fix`, `perf`,
  `style` or `revert` PR, or one whose type carries `!`, adds its changelog fragment under `releasing/changelog.d/` in
  the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`). Never edit `plans/` in a PR; this plan's state moves only through
  `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.

### Validation

Follow root `AGENTS.md` "Finishing work" for each PR: the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution, with the narrow-tests recipe in `.agents/narrow-tests.md` for
reproductions. When tests or their fixtures change, apply `.agents/test-authoring.md` and run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`. `dprint check` on changed Markdown. The
plan-specific notes above list the checks this work most likely needs; they are suggestions, not a checklist. Report
checks run, reused (with the covered revision) and skipped (with the reason) in the report.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot reach that
model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: this file's goal, the user's request and decisions, and the
planner choices. When the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Address what the reviewer finds before moving on, and log the DECISION where you decline a
finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (new persistent state, a new endpoint, protocol
message or subsystem, a compatibility layer, or anything the "What not to build" list names; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the ones the plan-specific notes above name, and every reviewer finding you
declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: push what is consistent, record the concrete tradeoff, continue independent authorized work, and
block per `plans/AGENTS.md` (Executing one plan, step 10) for what depends on it.

## Done criterion

The plan is complete when every PR in the implementation outline exists as an open draft, the stack satisfies the
acceptance criteria, every PR has passed the review gate, and the last code PR has removed the TODO.md entry this plan
covers. Open, not merged: merging happens only after the maintainer has reviewed this plan's report. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
