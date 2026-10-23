# Five supervisor fixes around managed checkouts, prepared checkouts and Delete

Written against main at c3345af7 on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where an outcome's completion criteria say which of their sentences to change.

This plan has no dependency on another plan.

## The goal

Five supervisor-side fixes that keep Farhelm from acting on the wrong folder or losing a surviving session's files: a
checkout with no recorded creation time is released, never archived or trusted; a stale create retry no longer takes
over a session the user restarted; a session that survives an interrupted Delete keeps its attachments; a duplicated
pane row counts as uncertain so startup cleanup keeps protecting a checkout being prepared; and taking a prepared
checkout falls back to a fresh clone when upstream was rewound or diverged.

Carry out these 5 triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md`, in this order (ledger
order, which `review_feedback_queue/.agents/execute-outcomes.md` asks for), one PR each. Read every entry in full first.
Each has a size gate of about 100 lines; over it, block and bring it back.

1. `no-birth-time-checkout-archives-reclone.md` (`fix spec+code`)
2. `create-retry-takeover-replaces-restarted-session.md` (`fix code`)
3. `interrupted-delete-loses-attachments-at-startup.md` (`fix code`)
4. `duplicate-pane-facts-can-unprotect-spare-claim.md` (`fix code`)
5. `spare-checkout-rewind-keeps-removed-history.md` (`fix spec+code`)

## Outline

Facts from the triage session's reading of main at b97bcfe4 (verify them; line numbers drift):

- PR 1: `crates/farhelm-supervisor/src/working_copies.rs`: `observe` leaves `birth_ns` None without `STATX_BTIME`
  (around 518-541); allocation stores it (around 1378-1407); `same_directory` (around 591-598) then compares device and
  inode only, and that reaches the archive move (around 1667), the trash delete via `verify_archived_handle` (around
  710; `crates/farhelm-supervisor/src/checkout_trash.rs`) and trust via `verify_identity_observed` (around 1552). A row
  recorded without a creation time whose folder still reports none is unconfirmed: Delete releases it with the existing
  release-without-archiving notice (SPEC.md "Confirmed 2026-09-28: archiving a checkout never blocks deleting its
  session"), the trash refuses its permanent delete, no automatic trust. A row whose folder reports a creation time now
  is a legacy row and keeps the lenient comparison (`inode-reuse-defeats-ownership-check.md` in the ledger). Update
  SPEC.md "Managed checkouts" (around 1093, "only where no creation time is available").
- PR 2: `restart_pending_launch` (`crates/farhelm-supervisor/src/store.rs` around 3749-3990) refuses only on pane,
  outcome and `conversation_source` (around 3858-3863). Also return `Launched` when `generation != 0`, matching
  `delete_untouched_retry` (around 5626-5633, PR #2570); `Launched` leads to settle-and-replay (`service/core.rs` around
  9689). Store test: Pending reservation, `begin_relaunch`, row Launching with empty pane, then `restart_pending_launch`
  returns `Launched`.
- PR 3: `reconcile_at_startup` (`crates/farhelm-supervisor/src/attachments.rs` around 440-462) discards the whole
  quarantine; parked directories are named `<session-id>-<uuid>` (around 330). For each whose session is in
  `known_sessions` and whose own attachments directory is absent, move it back; keep one that cannot be moved back;
  discard only those whose session no longer exists. Correct the module docs (around 299-314) and the teardown comment
  (`service/teardown.rs` around 648-650). Test with a surviving session.
- PR 4: `crates/farhelm-supervisor/src/tmux/control_codec.rs` (around 108-178) drops duplicated pane ids from the map;
  `service/status.rs` (around 151-152) then reads absence, and startup leaves the claim unprotected (`service/core.rs`
  around 6905-6947). Make a duplicate an uncertain observation, so the claims it could concern stay protected, as an
  ordinary query error already does (around 6880-6888). Unit test. The maintainer chose this over closing the item as
  covered by the private-server rule; keep it small.
- PR 5: `crates/farhelm-supervisor/src/spare_take.rs`: after the tracking-ref checks (around 101-120), run
  `git merge-base --is-ancestor HEAD <tracking>` through the existing query runner; anything but exit 0 (timeouts
  included) means a normal clone into the still-empty folder, which SPEC_impl.md already allows after a successful fetch
  (around 2469-2470). Test on the existing spare-take setup in `crates/farhelm-supervisor/src/launch.rs` tests (around
  5405, and 5667 for the divergence shape). One sentence each in SPEC.md "Managed checkouts" (around 1039-1054) and
  SPEC_impl.md, and correct the user docs sentence "A failed update stops the launch" in
  `website/src/content/docs/docs/using/start-a-session.mdx` (around 272), which a diverged branch no longer meets (read
  `website/AGENTS.md` first). Prepared checkouts are unreleased work: edit `releasing/changelog.d/spare-checkout.md`
  rather than adding a fragment, for this PR and for PR 4 if its change is visible to users.

### Validation

Each PR: the changed module's tests through the recorder with the pinned nextest and tmux setup
(`.agents/narrow-tests.md`); PR 5 also the spare-take launch tests. `cargo clippy --all-targets -- -D warnings` and
`cargo fmt --all -- --check` once at the tip. No full battery.

## Fallbacks

Each outcome's gate is binding: over about 100 lines, block and bring it back. Examples of departures that need the
scope reassessment: a Farhelm-private marker or other new ownership evidence for PR 1 (the ledger points at refusing
rather than building machinery), a durable delete-intent record for PR 3, or detecting tampering with the private tmux
server for PR 4.

Decisions to log: each PR's measured size; for PR 1 exactly which acts treat a creation-time-less row as unconfirmed;
for PR 3 what happens to a parked directory whose session exists but whose attachments directory is present.

## Gates, acceptance and requirement sources

### Size gates

Each outcome's gate, where it has one, is in its `TRIAGE_OUTCOMES.md` entry's Completion criteria and is binding: about
100 lines of diff, or the larger number the entry states (code, tests and docs, not counting `TRIAGE_OUTCOMES.md`, the
queue removal or the changelog fragment). The maintainer prefers size gates to time estimates. Over the gate, block and
bring it back to the maintainer (an entry that says "as above" means this): block per `plans/.agents/executing.md`,
"Executing one plan", step 10, after building and pushing the plan's other outcomes, with a question that describes the
larger fix, its measured size, and asks whether to build it or discard the outcome. Measure and log each outcome's size.
The entry's described fix is the triaging session's proposal; a different fix within the same gate that meets the same
completion criteria is fine, logged as a DECISION.

### Acceptance criteria

- Each outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry, including any test it names.
- Each outcome's PR removes its feedback file and `review_feedback_queue/INDEX.md` entry and updates its Execution
  field.
- A `fix` PR follows `AGENTS.md`, "Changelog fragments". A fix to work not yet in a stable release (check whether the
  change being fixed shipped in the latest stable tag) edits that work's existing fragment under
  `releasing/changelog.d/` instead of adding a second one; this file's outline names the fragment where the planning
  session found one. Otherwise it adds a fragment (`kind: fixed`, or `kind: none` with the reason when nothing is
  user-facing). `docs`, `test`, `chore` and bookkeeping PRs carry none.
- Any PR that changes Rust or browser tests or their helpers runs `python -B scripts/check-test-sleeps.py` with the
  isolated interpreter `docs/test-sleep-check.md` prepares (root `AGENTS.md`, "Tests and validation"); a Rust change
  also runs `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` at least once at the tip.

## Requirement sources

**The user's decisions (2026-10-22):** the maintainer decided each outcome on a triage page that day, after a read-only
verification of every finding against main; each outcome's Decision field records the choice and, where they gave one,
their words. Where a principle question decided several outcomes at once, the Decision field says which. While
scheduling, the maintainer agreed to that day's 40 pending outcomes being scheduled as ten plans grouped by area (this
is `checkout-ownership-oct22.md`) and chose the review gate below: a single fresh-context Opus 5.5 agent at high effort
with the general charter, no swarm. No-workhorse mode, which `plans/.agents/planning.md` requires.

**Binding repository constraints:** the rule each entry's Assessment cites;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits", "Code rules" and "Tests and validation", with `.agents/validation.md`;
`.agents/test-authoring.md` for any test change; and anything this file's outline names as binding for the plan.

**Planner proposals:** the PR order and the outline above, including the notes a fresh planning review added. A planner
proposal is not a requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-checkout-ownership-oct22-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

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
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
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

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one reviewable commit, stable
  bookmark and draft PR per outcome (unless this file groups some outcomes into one PR, which the maintainer agreed for
  those cases only), in a single linear stack in the order given under The goal above. Each PR carries its outcome's
  agreed change, the removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its
  `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field before creating
  the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/checkout-ownership-oct22/<nn>-<short-name>` (for example `plan/checkout-ownership-oct22/01-short-name`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment in the same commit; `test`
  for a change only to tests or their fixtures; `docs` for documentation and specification text only; `chore` for
  maintainer tooling and queue bookkeeping. Only a fragment-requiring type adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TRIAGE_OUTCOMES.md`, `review_feedback_queue/`, `TODO.md` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high
effort, launched as a native sub-agent where your harness serves that model and otherwise through
`$scode-harness-shellout` (that skill owns the launch mechanics). The prompt carries the full charter, because the
reviewer has nothing else:

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
  `TRIAGE_OUTCOMES.md` entry, including any size gate (ask the reviewer to flag anything beyond the gate).

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, whether each gated outcome met its gate (with
the measured diff size) and why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. If current code or specs invalidate a recorded triage decision (for example,
the finding turns out already fixed or wrong), that is a block: `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless this file
groups some into one PR (an outcome discarded under its gate still gets its bookkeeping PR; one whose gate says to bring
it back makes the plan block instead, after its other outcomes are pushed), that together meets the acceptance criteria
above, each PR passed its review gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID,
bookmark and PR. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
