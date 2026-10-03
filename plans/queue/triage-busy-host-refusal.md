# Execute the 2026-10-02 busy-host triage outcomes: refuse management requests instead of freezing the host

Written against main at 9e1dde03 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/queue/triage-busy-host-refusal.md` ``, exactly as root `AGENTS.md` section "Execute triage
outcomes" prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack,
in this order:

1. `list-admission-blocks-terminal-reader.md` (fix code)
2. `stop-admission-blocks-terminal-reader.md` (fix code)
3. `restart-admission-blocks-terminal-reader.md` (fix code)
4. `rename-admission-blocks-terminal-reader.md` (fix code)

The ledger lists Stop first, but the list goes first here on purpose: the shared refusal step lands in PR 2, and with
the list already off the management cap by then, no intermediate commit refuses a session-list request (which SPEC.md
forbids). The ledger says whichever admission item executes first introduces the shared step; that is PR 2.

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms proposed to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 4 draft PRs exist, stacked in the order above (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, as refined by the plan-time decisions below.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field.
- Each PR has passed the review gate.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "now use the planning system to plan execution of what we triaged", after a triage session on
2026-10-02 that recorded these outcomes (landed in #1475).

**The user's triage decisions (2026-10-02),** recorded in full in each ledger entry's Decision field:

- When a host's management capacity (the supervisor's eight request-handler slots) is exhausted, a further request is
  refused promptly with a "try again" error instead of waiting, "and ensure comments in code make it clear why".
- The session list is exempt from that cap rather than refused, with its own small limit.
- Delete keeps its current behavior: it already waits for its slot inside its own task, off the connection reader.

**The user's plan-time decisions (2026-10-02):**

- P1. Review gate: one fresh-context Opus 5.5 reviewer at high effort per PR, with the general charter below.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the per-item mechanisms below.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live
install is off-limits), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

## Per-item outline

Line numbers drift; find the code by name. Background, verified on main at 9e1dde03: the supervisor reads every frame of
a helm connection in one loop (`crates/farhelm-supervisor/src/service/connection.rs`) and awaits control handling
inline, so any wait inside a handler's dispatch freezes terminal input, resize, detach, list and upload-cancel frames
for every session on that connection. `HANDLER_ADMISSION_PERMITS` (8, `core.rs`) bounds request handlers. Three call
sites wait for a slot inside the reader: the shared `spawn_admitted` helper (`connection.rs`; used by list, directory
browse, repository search, tab open and tab close), and Stop's, Restart's and Rename's own `acquire_owned()` calls in
`handlers.rs`. Delete already spawns its task first and waits inside it. The waiting is deliberate today: the comments
on the semaphore and on `spawn_admitted` say it backpressures the sending connection, and the test
`spawn_admitted_acquires_the_permit_before_spawning_not_inside_the_task` enforces it. A long freeze escalates: the helm
drops the whole connection when a list request exceeds its 30-second `REFRESH_TIMEOUT`
(`crates/farhelm-helm/src/manager.rs`).

1. **The session list leaves the management cap.** `fix:`.
   - Proposal: `handle_list_sessions` spawns its tracked task immediately and waits for a separate small limit (two
     concurrent lists is a reasonable start; log the DECISION) inside that task, the way Delete waits for its slot. The
     wait is then off the reader. The number of waiting list tasks is bounded only by how many list requests the helm
     has outstanding on that connection, which is small but more than one: the periodic refresh, session detail reads,
     replace-with and the agent clone path each send their own (verified on main at 9e1dde03). Say that accurately in
     the comment; it does not call for a new cap. The status sampler already keeps its own limit apart from request
     admission (`ticker.rs`); follow that precedent and say so in the comment. Listing takes no lifecycle, intent or
     directory lock, so it needs no management slot for correctness; the limit only bounds its tmux subprocesses and
     capture sweep.
   - Test: hold all eight management slots with controlled lifecycle work, then send a list request and terminal input
     on the same connection; both must progress before the slots are released.
   - Changelog fragment: `kind: fixed`, written for someone running Farhelm (typing and the session list stay usable
     while many sessions are being stopped, restarted or deleted at once).
2. **The shared refusal step, applied to Stop and to `spawn_admitted`.** `fix:`.
   - Proposal: one helper that takes a management slot without waiting (`try_acquire_owned`) and otherwise replies with
     the existing `ErrorKind::Unavailable` and a message such as "this host is busy with other session operations; try
     again". The helm already maps `Unavailable` to HTTP 503 and the UI shows the error text on the action (Stop shows
     "stop: …" on the row). No new protocol error kind: a new variant would need a protocol version bump.
   - Replace the waiting acquire in Stop and inside `spawn_admitted`, which then covers directory browse, repository
     search, and tab open and close too. The refusal happens before any change, so retrying is safe.
   - Rewrite the semaphore's and `spawn_admitted`'s comments to say why the reader must never wait and why refusing is
     the chosen response (the user asked for this explicitly). Replace the test that enforces waiting with one where the
     slots are saturated, the request is refused with `Unavailable`, and terminal input to another session still reaches
     its pane.
   - Changelog fragment: `kind: changed` or `fixed` (log the DECISION): Stop, folder browsing, repository search and
     opening or closing tabs are refused with a "try again" message while a host is busy with many session operations,
     instead of freezing typing on that host.
3. **Restart uses the refusal step.** `fix:`. Replace Restart's own waiting acquire with the helper. Test: saturated
   slots, Restart refused, input to another session progresses. Make sure the UI's restart error and "restart with"
   error both show the message (`session_view.rs`). Changelog fragment as for PR 2, or `kind: none` if PR 2's entry
   already describes Restart (log the DECISION).
4. **Rename uses the refusal step.** `fix:`. Rename hands one permit through its database update and reply; keep that
   handoff with the permit now taken without waiting. Test as for PR 3. The existing end-to-end test
   `more_concurrent_renames_than_admission_slots_all_complete` fires 24 renames at once and expects every one to
   succeed; it encodes the waiting behavior this item removes, so rewrite it to require that every rename completes
   promptly, either succeeding or refused with `Unavailable`. Changelog fragment as for PR 3.

A consequence to document in the semaphore's comment, not to engineer around: waiters on the management slots are served
first come, first served, so while a batch of more than eight Deletes (which wait for their slots inside their own
tasks) drains, Stop, Restart, Rename, folder browsing, repository search and tab open and close are refused for the
whole batch, not just at its peak. The user accepted queueing for Delete and refusing for the rest; this is what that
combination does.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-busy-host-refusal-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/triage-busy-host-refusal/<nn>-<short-name>`.
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

- Every PR: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` (or clippy on `farhelm-supervisor`
  when that is all that changed), and focused nextest selections on `farhelm-supervisor`'s `connection` and `handlers`
  tests, including the new saturation tests, through the recorder with `--tmux required`.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.
- A PR that only changes Markdown: `dprint check` on the changed files, nothing else.

### Review gate

Before finishing every PR, use the active galaxy-brain skill to delegate a review of that PR's changes. The user demands
the reviewer: a fresh-context agent on Opus 5.5 at high effort, with the general charter below. No review swarm. The
prompt carries the full charter, because the reviewer has nothing else:

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

Before implementing a substantial departure from the outline above (a new protocol error kind, a queue for refused
requests, a change to Delete, a per-connection fairness scheme; these are examples, not a blacklist), and whenever the
same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the ledger entry, the user decisions above, this outline, the current diff and the proposed departure
(what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the session list's separate limit and where it is acquired, the helper's name and
message, whether any other call site waits for a slot inside the reader that the ledger did not name, each changelog
kind, and every review finding you decided not to follow. The user will ask for these later.

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
criteria as refined by the plan-time decisions, every PR passed the review gate, and each PR has updated its own
`TRIAGE_OUTCOMES.md` Execution field and removed its queue item. Open, not merged: merging happens only after the
maintainer has reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
