# Retain failed Restart scope cleanup across session deletion and supervisor restart

Written against main at 10973c17e on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Retain failed Restart cleanup when the session was deleted.** When Delete removes the session row while Restart is
> launching its replacement, `tear_down_relaunch_deleted_meanwhile` reports an unconfirmed replacement scope and keeps
> the replacement's host record. That record carries the socket and starter identity, while the deleted row held the
> scope selection. Startup can remove the record after finding the host gone even if its scope still holds a detached
> descendant. Preserve the unresolved scope selection across row removal and supervisor restart. Retry cleanup using the
> session and generation's derived scope name, and retire the obligation once the scope is confirmed gone. Cover a gone
> host with an unconfirmed scope that becomes checkable on a later start.

In product terms: on Linux, when a session is deleted in the narrow window while Restart is launching its replacement
and `systemd --user` is temporarily unreachable during teardown, Farhelm ends the replacement terminal host but cannot
confirm its systemd scope is gone. Today, the leftover launch record does not remember that the launch was scoped, so
when the supervisor later restarts after systemd recovers, startup sees the terminal host is already gone and drops the
record without ever killing the leftover systemd scope (and skips orphan cleanup completely if no sessions remain in the
database). Persist that unresolved scope cleanup in `supervisor.db`, retry it on supervisor startup using the derived
scope name, and retire it only once the scope is confirmed gone.

### Where it stands (from planning research and planning review at main 10973c17e; line numbers approximate)

- **`tear_down_relaunch_deleted_meanwhile`** (`crates/farhelm-supervisor/src/service/core/host_restart.rs`, about
  796-858): called at line 313 when `Transition::ConfirmRunning` returns `Ok(None)` because the session row disappeared
  while Restart launched its replacement. Currently takes
  `(&self, id: &str, scope: Option<&str>, new: &HostTerminal, old: Option<&HostTerminal>, artifacts: &LaunchArtifacts)`;
  its sole caller already has `generation: i64` in scope (about 203), so add `generation: i64` to its signature. Runs
  `reap_process_tree(..., ScopeUnits::recorded(scope), ..., ScopeKillFailure::Refuse)`, then `end_launch_host`. Today
  `artifacts.host()` is removed only if `end_launch_host` succeeds AND `tree_ended` is true; `LaunchHostRecord`
  (`host_launch.rs`, about 218-260) stores only the socket and starter identity, never whether the launch was scoped
  (`sessions.launch_scoped` was deleted with the row).
- **Startup reclaim (`crates/farhelm-supervisor/src/service/core/host_reclaim.rs` and `core.rs`, about 8119):**
  `serve()` runs startup cleanup after acquiring the exclusive state-directory lock and before `UnixListener::bind`
  (never inside `Supervisor::new_with_seams`, which runs before the lock is held). `reclaim_record` (about 704-721)
  deletes any unbacked `.host` record as soon as its socket is `Gone`, never checking a scope, and `discover_orphans`
  (about 504-510) returns `None` immediately when `stored.is_empty()` to protect live hosts against a wiped database.
  Keeping `pending_scope_cleanups` separate from `host_reclaim.rs` preserves those two opposite safety contracts.
- **Derived scope name (`crates/farhelm-supervisor/src/scope.rs`, about 168-170):**
  `crate::scope::unit_name(id, generation)` returns **`Option<String>`** (`Some("farhelm-<id>-<generation>.scope")` for
  valid UUID session IDs, `None` otherwise), so the database never stores or trusts an arbitrary systemd unit name.
- **Bounded startup reap helper (`crates/farhelm-supervisor/src/service/sweep.rs`, about 1484-1494):**
  `reap_process_tree_with_podman_deadline` already exists specifically for startup callers (`old_terminals.rs`, about
  313, 343) so optional Podman cleanup does not eat the whole startup budget.
- **Supervisor schema (`crates/farhelm-supervisor/src/store.rs`, about 93):** follows a versioned migration ladder with
  historical-schema fixture tests.

### Acceptance criteria

1. **Durable persistence in `supervisor.db`.**
   - Add a
     `pending_scope_cleanups(session_id TEXT NOT NULL, generation INTEGER NOT NULL, PRIMARY KEY (session_id, generation))`
     table in `crates/farhelm-supervisor/src/store.rs`, bumping `SCHEMA_VERSION` with the standard migration step,
     fresh-schema statement, and historical-schema migration test.
   - Pass `generation: i64` into `tear_down_relaunch_deleted_meanwhile`. Whenever `scope.is_some()` and
     `reap_process_tree` fails (`!tree_ended`), record `(session_id, generation)` in `pending_scope_cleanups`
     (idempotently) before returning. When `scope` is `None` (unscoped launches or macOS), record nothing. If recording
     into SQLite fails, log and include it in the teardown error.
   - Remove `artifacts.host()` when `end_launch_host` confirms the replacement host is gone AND
     `tree_ended || scope_cleanup_persisted` (so an unscoped `!tree_ended` failure or a failed SQLite insert still keeps
     `artifacts.host()`, while a durably recorded scope cleanup no longer leaves `artifacts.host()` for `host_reclaim`
     to chase).
2. **Startup retry and retirement (startup only, inside `serve()` under the exclusive state-directory lock).**
   - During supervisor startup inside `serve()` (after acquiring the exclusive state-directory lock and before
     `UnixListener::bind`, NOT in `Supervisor::new_with_seams`), drain `pending_scope_cleanups`—**even when `sessions`
     is empty** (independent of `host_reclaim.rs`'s `stored.is_empty()` guard). Wrap the drain in a bounded deadline
     (e.g., `self.timeouts.orphan_reclaim` or a dedicated bounded budget) so a wedged systemd manager never stalls
     startup indefinitely.
   - For each `(session_id, generation)`, derive `let unit = crate::scope::unit_name(&session_id, generation);`. If
     `unit` is `None` (invalid non-UUID session ID), log a warning and retire the invalid row rather than panicking or
     looping forever. Otherwise pass `ScopeUnits::recorded(unit)` into `reap_process_tree_with_podman_deadline` (with
     `&SweepTarget::AgentOnly` and `ScopeKillFailure::Refuse`).
   - Retire (delete) the row from `pending_scope_cleanups` **only** when the reap returns `Ok(_)`. If it returns
     `Err(_)` or the startup budget expires before attempting it, keep the row and log a warning so a later supervisor
     start retries again.
3. **Tests, without sleeps (`.agents/test-authoring.md`):**
   - Schema migration unit test covering upgrade from the previous schema version and fresh-schema parity.
   - Store unit tests for inserting (idempotent), listing, and retiring `pending_scope_cleanups`.
   - Supervisor startup test (driving `serve()` or its extracted startup drain helper, mirroring `host_reclaim` tests)
     covering the exact scenario: a deleted session row during Restart where `end_launch_host` succeeds (host is gone)
     while `ScopeManager::fake_reprobing` is unusable during `tear_down_relaunch_deleted_meanwhile`, followed by
     restarting a new `Supervisor` on the same state directory:
     1. First with systemd still unusable → the obligation stays persisted (not dropped even though the host is gone and
        `sessions` has 0 rows).
     2. Then with systemd checkable and holding a live scope for `(session_id, generation)` → startup kills/confirms
        `farhelm-<id>-<generation>.scope` and retires the obligation (verified both when `sessions` is empty and with
        another unrelated session present).
4. **Docs / spec & bookkeeping:**
   - Update `SPEC_impl.md` "Cleanup on quit" (about 124-142, recording the maintainer-approved startup retry of
     `pending_scope_cleanups`) and any Restart teardown paragraph if it describes deleted-meanwhile cleanup.
   - Include a changelog fragment (`kind: fixed`) and remove the
     `Retain failed Restart cleanup when the session was deleted` entry from `TODO.md`.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Persistence:** Dedicated SQLite table (`pending_scope_cleanups`) in `supervisor.db` (chosen over a launch-directory
  sidecar file for atomicity and immunity to `host_reclaim.rs`'s `stored.is_empty()` guard).
- **Retry timing:** On supervisor startup only (no periodic background ticker).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- Supervisor database migration ladder and historical-schema fixtures in `crates/farhelm-supervisor/src/store.rs`.
- Never trust arbitrary stored unit names; always derive scope names from `(session_id, generation)` via
  `crate::scope::unit_name`.
- `SPEC_impl.md`, "Cleanup on quit" (approved startup sweeps).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and its "Instructions by path" for every path you touch.

**Planner proposals** (from research and the independent planning review):

- Keep everything in one concise PR: schema migration, `tear_down_relaunch_deleted_meanwhile` persistence, startup
  drain/retirement using `reap_process_tree_with_podman_deadline`, tests, `SPEC_impl.md` update, changelog fragment, and
  TODO removal.
- Keep `pending_scope_cleanups` separate from `host_reclaim.rs` because `host_reclaim` intentionally skips empty DBs
  while `pending_scope_cleanups` must drain even with 0 session rows.

## Outline

1. `plan/restart-deleted-cleanup/01-pending-scope-cleanups`: add `pending_scope_cleanups` migration and store methods,
   pass `generation` into `tear_down_relaunch_deleted_meanwhile` and record unresolved scope cleanups, drain and retire
   them inside `serve()` via `reap_process_tree_with_podman_deadline` (including with 0 session rows), add
   migration/store/restart tests, update `SPEC_impl.md`, add changelog fragment (`kind: fixed`), and remove the TODO.md
   entry. `fix: retain failed Restart scope cleanup across supervisor restarts`.

## Fallbacks

- If an existing test in `host_restart.rs` asserts that `artifacts.host()` remains on disk after a failed scoped
  teardown, update it to assert `pending_scope_cleanups` holds the durable obligation (and that `artifacts.host()` is
  kept only when the host itself could not be ended or DB persistence failed) and log a DECISION.
- A reviewer finding asking for periodic in-process retries or a launch-directory sidecar file is declined by decision;
  log it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-restart-deleted-cleanup-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-restart-deleted-cleanup-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes (`farhelm-supervisor` store/migration,
`host_restart`, `host_reclaim`, and `host_relaunches` tests, plus `cargo fmt --all -- --check` and
`cargo clippy --all-targets -- -D warnings`). Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Never take a
state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Retain failed Restart cleanup when the session
  was deleted"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. A supervisor schema version that another
  plan also took is renumbered at rebase (`.agents/careful-rebase.md`).
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
