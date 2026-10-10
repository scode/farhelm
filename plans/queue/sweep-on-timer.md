# The supervisor sweeps on its timer only, and stops re-reading settled sessions

Written against main at 363ba6b1 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. The `transcript-reads-on-need` plan landed just before this one was written
(PRs #1734, #1736, #1738) and changed the same capture pass, including the report-folder drain and the store's cached
statements. Read what it did (its PRs and SPEC_impl.md) before starting; do not redo that work.

## The goal

On an ordinary host with about ten live sessions, the supervisor spent a large share of a core on work whose answer had
not changed (`lore/2026-10-08-supervisor-idle-cpu.md` has the measurements). One multiplier on all of it: the
per-session sweep runs on the 2 s ticker and again at the start of every session-list request (and every session-info
request), and the helm lists on a 3 s backstop and again after every status-change hint (at most every 200 ms). That
came to about one full sweep every 1.3 s, overlapping and contending, enough that ticks overran. Inside the sweep,
stopped sessions whose outcome is settled re-read their launch status file, query checkout provenance and re-parse their
checkout-preparation JSON on every tick and every list, forever.

Make the ticker the only thing that sweeps, have list and info requests answer from the state it leaves, and stop
re-reading what cannot have changed.

Acceptance criteria:

- A session-list request and a session-info request run no capture pass (`capture_now`/`capture_pass`), apply no hook
  report files, resolve no notifications, read no launch status file or preparation state, and commit no session outcome
  or transition. They answer from stored and in-memory state (D1).
- A list still shows an agent exit, and tabs opened or closed, at once: it keeps its one pane-state query
  (`TmuxDriver::pane_states`, a single `list-panes -a`) and derives live status and tabs from it, as today (D1).
- The observation lists used to do moves to the tick, with the same pane-ownership rules `observe_entry` applies through
  `pane_evidence` today: an absent pane (including one whose id tmux recycled into another Farhelm session) and an owned
  dead pane are observed for exit and launch status; an unattributed (renamed) pane is observed for its launch status
  only, never for an exit. The tick's current filter (absent, or present under the entry's own name and dead) is
  narrower than that and must widen. The tests `a_list_still_reports_the_launch_sentinel_of_an_unattributed_pane` and
  `a_list_reads_a_pane_recycled_into_another_farhelm_session_as_gone` keep their guarantees, moved to the tick, not
  dropped.
- The pane-died wake (`reap_pass`) observes and commits the sessions whose agent pane just died: those whose recorded
  outcome is not yet terminal (running, stop requested) and whose own pane it finds dead. It does not re-observe every
  stopped session, whose agent panes stay dead on screen. So a newly exited agent, and a launch failure classified as
  error from its launch status file, is committed at the wake, not a tick later (D1). The tick remains the fallback (a
  killed window does not fire the hook). The wake hints before it commits and the commit hints again; a sub-second
  Exited-then-Error flicker in the helm is acceptable and needs no ordering lock.
- A supervisor that may not record (a handoff candidate, or one that could not read the boot id), or a commit that
  failed, must not make a launch failure show as Exited forever. Today each list re-reads the launch status file for
  that case. Keep the found error detail in memory on the session entry when it cannot be committed, and have the reply
  read it (D4).
- Restart keeps its decision-time capture pass (`capture_pass(true)`), and startup and reload keep theirs; nothing else
  that is not the ticker runs one.
- Hook-reported state (a conversation becoming resumable), launch-error classification other than at a pane death, and
  notification resolution may lag by up to about one tick; that is accepted (D1). A launch status file that cannot be
  read no longer fails the list request: the list shows what the pane shows, and the tick logs and retries. SPEC_impl.md
  and `list_all`'s doc say both.
- Settled stopped sessions stop re-reading (D2). The launch status file and the checkout-preparation state of a session
  are no longer read on later passes once a read found nothing, while the recorded outcome is terminal (an unannotated
  exit or an interruption) and the session's own pane was seen dead, or the interruption came from a boot change (the
  previous boot's launch shim cannot write any more). Never stop reading after a read error, after a found error that
  was not committed, or while the supervisor may not record. A pane that is merely absent within the same boot keeps
  being read, because `pane_states` can briefly report an empty server under load and an exit recorded then may precede
  the shim's failure; that sentinel read is one failed `open`, and the preparation read follows the same rule. A new
  launch generation (restart) and a supervisor restart read again. The error-row cleanup runs once per error rather than
  every pass.
- Within observation and the capture pass, no other read repeats for a settled stopped session without a reason it could
  have changed; list each one you found and what you did with it in the log. The report-folder drain and statement
  caching are already done (above) and out of scope here.
- Every doc that says a list or reply observes, commits or refreshes is updated. SPEC_impl.md: the "Helm internals"
  bullet that says the capture sweep rides `ListSessions`, "Runtime state" ("Reply paths and the ticker both run this
  reconciliation"), the report-files paragraph ("reply paths", "or at the next reply"), the version-33 hint paragraph,
  and the paragraph saying `ListSessions` is the only reply computing a real liveness answer; SPEC_impl.md states the
  freshness rule of D1. Code docs: the ticker module doc, `TICKER_INTERVAL`'s doc, `capture.rs`, `report_files.rs`,
  `listing.rs`, `observe_entry`'s and `EntryObservation`'s docs, `read_launch_sentinel`'s,
  `sentinel_could_still_apply`'s, `dead_pane_exit_code`'s, `entry_info`'s `sentinel` parameter, `session_info_now`'s,
  the proto doc of `SESSIONS_CHANGED_MIN_GAP`, and the e2e harness's `wait_for_non_live_status` and `wait_for_listing`
  docs. Grep for `ListSessions`, `list pass`, `next list` and similar to find the rest.
- Tests: the e2e tests that relied on a list to commit outcomes, show launch errors, apply hook reports or resolve
  notifications drive the extracted observation step and a capture pass through a test seam (D5) before they list; tests
  that only check liveness, tabs or a surviving pane's exit code keep listing. New tests show that a list neither sweeps
  nor commits, that a newly dead agent pane is committed at the wake, that a settled stopped session is not re-read on
  later passes while a same-boot absent one is, and that an uncommitted launch error still shows as error.
- A best-effort before/after measurement (D6), recorded in the report.
- The last code PR removes the TODO.md entry "Sweep on the timer only, and stop re-deriving known state." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Sweep on the timer only, and stop re-deriving known state. The per-session sweep runs on the 2 s ticker and at the
start of every session-list and session-info request, including the lists the helm sends after every status-change hint:
about one full sweep every 1.3 s, overlapping and contending, enough that ticks overrun. The hook-only identity change
(#1540) removed the coalescing that used to skip a ticker sweep after a list's. Run the sweep from the timer only;
requests answer from the state it leaves. Within the sweep, stop re-reading anything that cannot have changed since it
was last read: in particular stopped sessions whose outcome is settled, which today re-read their launch status file,
query checkout provenance, and re-parse their checkout-preparation JSON on every ticker tick and every list, forever.
Settle how fresh a list must be, and keep pane liveness (`list-panes`) on the list if exit reporting needs to stay
immediate. Needs SPEC_impl.md edits where it says the capture sweep rides `ListSessions` on purpose. Details:
`lore/2026-10-08-supervisor-idle-cpu.md`."

**The user's decisions (2026-10-09):**

- D1. Freshness: lists answer from the state the last tick left. They keep the cheap pane-state query so exits and tab
  changes show at once, and the pane-died wake commits exits durably. Resume appearing after a hook report, launch-error
  status and notification resolution may lag up to about one tick. The maintainer picked this from the options
  presented; the rejected alternatives were dropping the pane query from lists too (an exit up to about 2 s late) and
  keeping hook-report application on lists. The planner reads D1 as accepting lag, not loss (the chosen option said
  these "may lag"): anything a list shows today must still show, at most a tick later.
- D3. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements; a
fresh-context planning review already shaped D2, D4, D5 and D6):

- D2. The stop-re-reading rule is an in-memory latch on the session entry (which a restart replaces, along with the
  generation), not a store column: a supervisor restart re-reads once per session, which is cheap and keeps the schema.
  If you find that every no-find read on the polling path is followed by the commit of a terminal outcome in the same
  pass, "read only while the recorded outcome is not terminal, plus once at reload" may need no latch at all, but only
  with the own-pane-seen-dead restriction above; log which you built.
- D4. An in-memory cell on the session entry for a found but uncommitted launch error, read by `entry_info`, replacing
  the list's re-read. It is the in-memory mirror of what each list used to recompute.
- D5. The test seam: extract the observation-and-commit step out of `sample_pass` (the wake needs it anyway) and extend
  the existing `reconcile_for_test` (`service/core.rs`, `cfg(test)` and `test-seams`) to run that step and then
  `capture_pass(true)`, rather than adding a second seam or a full-tick seam. A full tick would also sample screens,
  which would drift statuses to idle and move activity stamps in tests that poll; a real fast ticker in the harness
  would do the same, nondeterministically, on a shared CPU. `reconcile_for_test`'s existing users (`codex_identity`,
  `hook_identity`, `restart_with_resume`) gain a harmless observation step.
- D6. The measurement is best-effort and does not gate completion. Use a supervisor started from temporary state and a
  synthetic client that issues `ListSessions` at the helm's cadence (3 s, plus bursts as hints would cause) with about
  ten sessions, never a helm, so nothing can touch the live install. Report the supervisor's CPU from `/proc/<pid>/stat`
  over a fixed window, and optionally counts of `list-panes` and launch-status opens from `strace -f -c` on your own
  child process. Add no product counters for it. Take the baseline on main (the test-seam PR changes no product code).
  If the machine is loaded by other agents, say so with the numbers, or skip with the reason logged.
- The PR slicing in Outline.

**What the planner found (at 363ba6b1, checked by a fresh-context review).**

- The ticker is `crates/farhelm-supervisor/src/service/ticker.rs`: `TICKER_INTERVAL` (2 s, overridable through
  `SupervisorSeams::ticker_interval`), `start_ticker`, `tick` (takes `sampling_admission`, runs `sample_pass`, then
  `capture_now`), and `sample_pass`: pane states, `publish_pane_deaths`, `reap_dead_tabs`, then `observe_entry` only for
  entries whose pane is absent or present under the entry's own tmux name and dead
  (`state.session_name == terminal.tmux_name && state.dead`), with `transition_many` and `mirror_committed_outcome`,
  cleanup for error rows, then a budgeted screen sample. `reap_pass` (the pane-died wake) hints, reads pane states,
  publishes deaths and reaps dead tabs; it deliberately does not observe or commit today, and its doc says why (the
  tick's other work there would let every exit pull the tick forward). Observing only the newly dead agent panes is
  narrower than that and is what D1 asks. `next_deadline` is anchored and skips whole missed intervals, so an
  overrunning tick pushes the next to the next 2 s boundary.
- `capture_now` → `capture_pass(false)` in `service/capture.rs`: `apply_report_files`, `refresh_report_only_captures`,
  the report liveness tripwire, `resolve_resumable_notifications`.
- `list_all` in `service/listing.rs` (from `handle_list_sessions` in `service/handlers.rs`, behind `list_admission`):
  `capture_now` (comment: "Refresh accepted reports before constructing offers"), `pane_states` (skipped when no entry
  has a terminal), `observe_entry` for every capped entry whatever its pane evidence, cleanup for settled errors,
  `transition_many`, `entry_info`, `project_checkout_metadata`, and the `last_listed` write, which Restart uses to
  describe the session before it began; keep it. `session_info_now` (RenameSession's reply) does the same in small.
  `restart_session` (`service/core.rs`) runs `capture_pass(true)` and reads preparation state itself; startup and
  `serve`'s reload run `capture_now`. A fresh-context review found no other dependency of Restart or of restart offers
  on the list path beyond the Resume lag D1 accepts.
- `session_status` (`service/status.rs`) derives a live or exited status straight from the pane-state map, so a list
  shows an exit without any commit. `entry_info`'s `sentinel` parameter exists so a reply shows error when the
  transition could not be committed (its doc cites "never let the reply itself regress to a stale Exited"); the tick
  keeps its own sentinel hits only in a local set today, which is why D4 is needed.
- The stopped-session reads are inside `observe_entry`, gated by
  `sentinel_could_still_apply(&recorded) && dead_or_absent` (an unattributed pane counts as "dead or absent" there too,
  though it is usually alive): `read_launch_sentinel` (via `spawn_blocking`), `wrapper_failure_detail`, and
  `interrupted_preparation_detail`, which calls `store.preparation_origin` and then `launch::read_preparation_state`, a
  synchronous `std::fs::read` plus serde on the async worker; a `Ready` state yields nothing and records nothing.
  `sentinel_could_still_apply` (`service/launch_artifacts.rs`) is false only for `Error` and an annotated `Exited`; its
  doc says a late-discovered sentinel must still supersede an unannotated `Exited` or `Interrupted`, citing PLAN_M3.md
  item 3. That wording is the code comment's; `lore/PLAN_M3.md` item 3 itself says "a current launch's sentinel outranks
  every inference", and the "addition 18" the code cites is a comment in `service/core.rs`, not in the lore file.
- The launch shim writes the launch status file and the preparation state synchronously before it exits (`launch.rs`,
  `record_launch_failure` and `abort_preparation`; preparation runs inside the shim before exec), so an owned pane seen
  dead makes a read after that final for the generation. An absent pane does not: `pane_states` folds "empty server",
  absent and mid-teardown diagnostics into an empty map (`is_definitively_empty`), and the e2e harness's
  `wait_for_listing` doc records a loaded machine catching a list that way. A related pre-existing issue, out of scope:
  the same transient empty map can make `interrupted_preparation_detail` record "preparation did not finish" as an error
  for a session still cloning; mention it in the report as a possible follow-up.
- Error-row cleanup (`cleanup_launch_artifacts`, `service/launch_artifacts.rs`) runs on every pass for already-`Error`
  rows from both the ticker and the list.
- Concurrency: per-session `capture_locks`, the `report_drain` mutex (`try_lock` for the ticker and lists),
  `sampling_admission` (one permit, tick and reap only), `list_admission` (two permits). No pass-level lock. The
  coalescing #1540 removed is visible in `git show 090a4766^:crates/farhelm-supervisor/src/service/capture.rs`
  (`CaptureReason::{Reply, Tick}`); with lists no longer sweeping it is not needed and must not come back.
- Tests: the default e2e harness (`crates/farhelm/tests/e2e/harness.rs`, `harness`/`harness_with_seams`) never runs
  `Supervisor::serve`, so no ticker runs and lists are the only thing that observes, commits, applies hook reports and
  resolves notifications there. Tests that will need the seam include `launch_sentinel_error_status.rs`,
  `boot_id_durable_outcome.rs`, `restart_under_concurrency.rs` and parts of `session_lifecycle.rs`; the `wait_for_*`
  helpers have about 48 uses and there are about 71 direct `list_sessions()` calls, most of which only need liveness and
  stay as they are. Harnesses that run `serve()` (for example `hook_harness` in `hook_identity.rs`) run the real ticker
  at 2 s; list-polls there may wait up to a tick longer within their budgets. Supervisor unit tests about list behavior:
  in `service/ticker.rs`, `a_list_still_reports_the_launch_sentinel_of_an_unattributed_pane` and
  `a_list_reads_a_pane_recycled_into_another_farhelm_session_as_gone` move to the tick and are kept;
  `a_list_that_first_records_an_exit_hints` becomes a wake or tick hint test;
  `a_list_never_records_an_exit_for_a_pane_renamed_out_of_band` becomes "a list commits nothing"
  (`a_moved_pane_never_produces_a_ticker_exit_transition` already pins the tick side); in `service/core.rs`,
  `list_error_cleanup_preserves_replay_and_read_only_artifacts` moves to wherever cleanup now runs.
- SPEC.md has no list-freshness number; its Status section says wrong status must be cosmetic only and never gate
  interaction, which this keeps.
- The helm (`crates/farhelm-helm/src/manager.rs`, `REFRESH_INTERVAL`, hint handling; `sessions.rs` `get_session`, which
  serves a session's detail with a whole `ListSessions`) needs no change: its lists become cheap.

**Binding repository constraints:** root `AGENTS.md`: Harness-specific code (read the map in
`crates/farhelm-supervisor/src/agent_kind/mod.rs` before touching anything per harness), Finishing work (targeted
validation, the test-sleep check, the recorder with nextest), Reproducing failures (narrow tests first), Releases and
the changelog (a fragment for `perf`), Testability (no tests that change the process environment), Sharing the machine
(the CPU and `systemd --user` are shared; timing-sensitive tests under another agent's load are environment events),
Agent scratch space, The live install is off-limits. SPEC.md's "Upgrade compatibility and client scale": no protocol
change is planned; an old helm with a new supervisor and the reverse must keep working. `.agents/test-authoring.md` for
any test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: extract the observation step; the test seam; tests that stop leaning on lists

`test:` or `refactor:`, no visible change and no changelog fragment. Extract `sample_pass`'s observation-and-commit
block into a function the tick, the wake and the seam can share, without changing what the tick observes yet. Extend
`reconcile_for_test` per D5. Move the e2e tests and helpers that need durable outcomes, launch errors, hook reports or
notification resolution to call the seam before they list, so they pass both before and after PR 2. Take the D6 baseline
on main if you do the measurement.

### PR 2: lists and info stop sweeping; the tick and the wake take over (D1, D4)

`perf:` with a changelog fragment (users may notice Resume appearing up to a couple of seconds after a hook fires; say
so plainly). Widen the tick's observation to `observe_entry`'s pane-evidence rules; make the wake observe the newly dead
agent panes; add D4's uncommitted-error cell; remove the capture pass, observation, transitions and cleanup from
`list_all` and `session_info_now`, keeping the pane-state query and `last_listed`. Move or rewrite the list-behavior
unit tests as What the planner found says, add tests for the new contract, and update SPEC_impl.md and the docs listed
in the acceptance criteria.

### PR 3: settled stopped sessions stop re-reading (D2)

`perf:` with a changelog fragment (`kind: none` if nothing is user-visible). The stop-re-reading rule for the launch
status file and the preparation state, cleanup once per error, and the bounded audit of observation and capture for
other repeated reads. Tests per the acceptance criteria, including a same-boot absent pane that keeps being read. Take
the D6 after measurement on the tip. Remove the TODO.md entry.

### Out of scope

The helm's refresh cadence and hint handling, `get_session`'s whole-list read, the protocol, screen sampling and its
budget, the report-folder drain and statement caching (done), the hook-report file watch (the separate "Pick up hook
report files immediately" TODO entry), the transient-empty-map preparation error noted above, SQLite settings, and the
allocator experiment.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the supervisor unit
tests and the affected e2e targets through `scripts/record-test-run.py` with nextest and `--tmux required` on the pinned
tmux; the test-sleep check; `dprint check`; and `python3 releasing/check-changelog.py format`. Because PR 2 changes when
state is committed for every session, a workspace nextest run at the tip is justified; a browser run is not needed
unless a UI-visible timing test fails.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-sweep-on-timer-log.md` in
the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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
  `plan/sweep-on-timer/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a store column or schema change for settled state, a
pass-level lock or coalescing scheme, a protocol change, moving hook-report application back onto lists, a new
background task, new product instrumentation for the measurement; these are examples, not a blacklist), and whenever the
same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the user's request and decisions above, this outline, the current diff and the proposed departure
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
alternatives considered: in particular how the stop-re-reading rule was built (latch or not, D2) and what evidence it
rests on, how the tick's observation was widened, how the wake picks the newly dead sessions, how D4's cell is set and
cleared, what each list-behavior test became, which other repeated reads were found and removed, the measurement method
and numbers or why it was skipped, the final PR slicing, and every review finding you decided not to follow. The user
will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Losing anything a list shows today (rather than showing it up to a tick later) is a weakened guarantee under D1; if you
cannot avoid it, that is such a block. Because the PRs form one linear stack, later PRs sit on top of a blocked one:
finish the PRs before it, record the question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.

## Decisions

### 2026-10-10: follow-up after review

The maintainer's follow-up, verbatim:

> send it back to fix case (1) with complexity gate. the mere fact that we need a database change is not a complexity
> problem (just in case that comes up in the executor), only the complexity of the resulting code in the end.

and, after discussing the other case: "yeah leave case 2, still send it back to fix case 1 with the complexity gate".

Agreed restatement:

- Case 1, to fix: a session whose agent exited on its own (an exited row without a stop annotation) before the host
  rebooted. Before the reboot the supervisor may have seen its pane dead and stopped re-reading its launch evidence, but
  that mark is held only in memory. After the reboot the new supervisor finds an exited row (not Interrupted, which only
  rows still running at the reboot get) and no pane to see, so it re-reads the launch failure file and the
  checkout-preparation evidence on every two-second tick for as long as the row exists. The plan's own reason for
  treating a reboot-interrupted launch as settled (the previous boot's launch cannot write any more) applies equally
  here, so such rows should settle too. The in-memory design was a planner proposal whose stated cost, "a supervisor
  restart re-reads once per session", does not hold for these rows.
- A database or schema change is allowed and is not by itself a complexity problem; judge complexity by the resulting
  code only. Possible shapes include recording per launch which boot it belongs to, persisting the settled mark, or
  settling finished rows when the supervisor first starts after a reboot; pick what keeps the code simplest while
  covering supervisor restarts within the same boot correctly.
- Complexity gate: if fixing case 1 needs significant code complexity, stop and block with the options instead of
  building it.
- Case 2 stays as it is: within one boot, a pane that is merely missing keeps being re-read, because tmux can briefly
  report no panes while the launch is still writing. It is rare and cheap; do not change it.
