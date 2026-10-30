# Supervisor: log each permanent trash deletion and spare removal with its timing

Written against main at 5831410e on 2026-10-30. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Log trash deletions.** The supervisor logs nothing when it permanently deletes a checkout from the trash. Log each
> deletion, including how long it took. Deleting small checkouts was seen taking many seconds each, and these logs are
> what would show where that time goes.

The maintainer also asked, on the planning page, for spare-checkout removals to be logged the same way, because they go
through the same file-by-file removal code.

How it works today, from a read of main at 5831410e (line numbers approximate):

- The trash dialog sends one checkout per request (SPEC_impl.md, "Archived-checkout trash", around 2936). Each host
  deletes one at a time; hosts run in parallel. The 15-minute deadline lives in the GUI only.
- Supervisor, `crates/farhelm-supervisor/src/service/handlers.rs` (around 2658-2679) routes `DeleteCheckoutTrash` to
  `delete_checkout_trash` in `crates/farhelm-supervisor/src/service/core.rs` (around 5178-5196). That waits for the
  host's directory admission (`working_copy_operations.lock_owned()`, shared with launches, restarts and session
  Deletes), then on the blocking pool reads `store::session_directory_paths(&db.lock())` and calls
  `checkout_trash::delete`.
- `delete_with` in `crates/farhelm-supervisor/src/checkout_trash.rs` (around 388-491) reads every working-copy row once
  per request (`all_working_copies(&db.lock())`), then for each id: the record state check, `member_count`,
  `open_archive` (root identity, the legacy `farhelm-archived-working-copies` migration, which also takes `db.lock()`,
  `openat2`, identity verification), the session guard (`canonicalize` and a comparison with session folders), the
  removal (`remove_opened_tree_with`, around 198-279, a post-order traversal with up to three rescans per directory),
  and `forget_retired`. Every outcome becomes `reply.removed` or `reply.issues`. Nothing on this path logs, in the
  supervisor or the helm.
- Spare removal: `discard_child` in `crates/farhelm-supervisor/src/spare_checkouts.rs` (around 596-609) opens and
  verifies the spare, renames it to a private `.<repo>:trash-<uuid>` name, reopens and verifies it, and calls
  `remove_opened_tree`. Its two callers are in `sweep_namespace` (around 537 and 554). It logs nothing either.
- Several steps wait on the supervisor's single shared database connection (`Db` in
  `crates/farhelm-supervisor/src/db.rs`), which is held through every write. A separate plan, `sqlite-wal.md`, makes
  those writes cheaper; these logs are what would show whether the database is where the time goes.
- Logging is `tracing` to stderr with a default `info` filter (`crates/farhelm/src/main.rs` around 2474-2494); on a
  Linux host that is the supervisor unit's journal. The only duration log today is the slow claim wait in core.rs
  (around 15713-15724: `started = Instant::now()`, `waited_ms = waited.as_millis()`). The move into the trash at session
  Delete already logs paths (`crates/farhelm-supervisor/src/service/teardown.rs` around 1075-1116). Tests capture log
  lines with the `farhelm-testtrace` crate (for example `teardown.rs` around 4043-4053).

### Acceptance criteria

- Every checkout the supervisor is asked to permanently delete from the trash produces exactly one `info` log line when
  its handling finishes, whatever the outcome: removed, record dropped without touching the folder, refused, or failed
  partway. The line carries the checkout id, the archived folder's path, the outcome, the total time in milliseconds,
  the time in each step, and how many files and folders were removed (partial counts on a failure).
- The steps, timed separately: the wait for the host's directory admission; the database reads (the session folder paths
  and the working-copy rows, each of which waits on the shared connection); the record checks (state and membership);
  opening and verifying the folder, including the legacy-trash migration and the session guard; the removal; and the
  record update. Steps measured once per request (the admission wait and the database reads) are repeated on each
  checkout's line, with a comment saying so; the GUI sends one checkout per request, so in practice they belong to that
  one checkout. No separate per-request line.
- Every spare-checkout removal through `discard_child` produces one `info` line when it finishes, with the repository,
  the spare's original name (not the private `:trash-<uuid>` name), the path of the location it sits in, the outcome,
  the total time, the removal time, and the files and folders removed.
- Counting adds no extra filesystem work: an increment beside each existing unlink, and a folder counted only when its
  final `rmdir` succeeds, so rescans do not double-count. Pass a counter by mutable reference rather than returning
  counts, so a failure still reports what was removed.
- Behavior is otherwise unchanged: the same replies, the same issues, the same admission, no new protocol field, nothing
  in the GUI.
- A unit test asserts the trash success line through `farhelm-testtrace` (outcome, id, path and counts; not exact
  timings), and at most one more covers a failure or refusal line. A focused test of the spare line if it is cheap with
  the existing spare fixtures.
- SPEC_impl.md's "Archived-checkout trash" gets a sentence saying each deletion is logged with its step timings, and the
  spare section the same for spare removals, if SPEC_impl.md describes them.
- A changelog fragment per root `AGENTS.md`, "Changelog fragments", when the PR's type calls for one, and the last code
  PR removes the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-10-30, on the planning page:

- **Per-step times and counts, one line when each deletion finishes.** No start line, and not total time only.
- **Every deletion at `info`.** Not `warn`, and not only slow deletions.
- **The checkout id and the path** are both on the line.
- **Spare-checkout removals are logged the same way.**
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above and the decisions above.

**Binding repository constraints:** SPEC.md's trash rules (around 1225-1271); SPEC_impl.md's "Archived-checkout trash"
(around 2871-2950) and "Logging" (around 4608-4648); SPEC_impl.md's rules that logged session ids are log-safe (around
122-130) and that peer-supplied text is Debug-escaped (around 1986-1991); root `AGENTS.md`, "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The step list above, which an independent planning review checked against the code. It added the database reads as
  their own step, because they wait on the shared connection and are a prime suspect for the slowness.
- Field names in the style of the existing `waited_ms` (`total_ms`, `admission_ms`, and so on), and `path` formatted the
  way teardown's move-to-trash line formats it.
- The trash line is emitted from where the per-id outcome is known (`delete_with` or its caller), with the per-request
  timings passed in, rather than by a new wrapper type.
- The spare line lives inside `discard_child`, so both of its callers are covered; `sweep_namespace` passes the
  location's path in.

## Outline

One PR:

1. `plan/log-trash-deletions/01-log-deletions`: the step timings and counters, the trash line and the spare line, the
   tests, the SPEC_impl.md sentences, the changelog fragment and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-supervisor`, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares
if tests change, and, through `python3 scripts/record-test-run.py`, the `checkout_trash` and `spare_checkouts` unit
tests in `farhelm-supervisor` plus anything else whose signatures the counter changes.

## Fallbacks

Examples of departures that need the scope reassessment: a new protocol field or GUI display of the timings, logging in
the helm, a per-request log line, changing how deletions are admitted or batched, or logging other removals beyond trash
deletions and spare removals.

Decisions to log: the final step list and field names, where each line is emitted, how the counter is threaded through
the traversal, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-log-trash-deletions-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/log-trash-deletions/<nn>-<short-name>`,
  as the outline names them.
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
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
