# Execute seven triage outcomes: supervisor lifecycle test fixtures and oracles

Written against main at 3f430cc4 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out seven triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `stop-test-leave-fixture-permanently-stopped-failure.md`
- `restart-test-has-same-stopped-child-cleanup-gap.md`
- `hup-resistant-process-fixture-leaks-early-failure.md`
- `stop-intent-test-exercises-different-failure-path.md`
- `live-agent-restart-test-never-establishes-live-agent.md`
- `skip-tmux-test-passes-tmux-queried.md`
- `failed-spec-publication-test-observe-whether-tmux-started.md`

All seven are defects in the supervisor's own unit tests. Nothing changes for someone running Farhelm; what changes is
whether these tests clean up after themselves and whether they actually prove what they claim about the supervisor's
session lifecycle:

- Three tests leave processes behind when they fail early. Two tests about stopping and restarting a session while the
  client's connection drops start a helper process and freeze it with SIGSTOP; any failure before the final cleanup
  leaves a frozen process on the machine for good. A test about deleting a session whose terminal tab runs a program
  that ignores hangups starts a shell and a background sleep; any failure before its last loop leaves both running.
  Several agents share this machine, so leaked processes accumulate and add load that other agents' timing-sensitive
  tests then trip over.
- Four tests pass without exercising the behavior they are named for. The test that a stop which cannot record its
  intent kills nothing uses a session whose agent was never running, so it covers the dead-or-absent branch, not the
  running-agent stop path. The test that restarting a live agent keeps its terminal tabs never confirms the agent was
  live; with the test's dummy agent it takes the dead-agent path. The test that listing sessions skips the tmux pane
  query when no session has a terminal kills tmux in a way the query already tolerates, so it would pass even if the
  query ran. The test that a failed session-spec write never launches tmux checks the session table and the store but
  never looks at tmux.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the seven entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- The two SIGSTOP fixtures and the HUP-resistant fixture are owned by guards that kill and reap them on every exit path,
  and none signals a recycled pid: the SIGSTOP fixtures stay unreaped until the guard's own wait, and the HUP fixture's
  guard re-checks each process's identity (pid, start time, not a zombie) right before each signal, with the residual
  window documented as in `tmux.rs`. The success-path assertions of all three tests are unchanged.
- The running-agent stop path, the live-agent restart path, the skipped pane query, and the absence of a tmux pane after
  a failed spec write are each proven by an assertion that fails under a one-off mutation of the production code it
  guards. The mutations are run once, the result is logged, and they are never committed.
- The existing dead-or-absent stop coverage and dead-agent restart coverage stay, under accurate names.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate. It is test-only, so it carries no changelog fragment.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-11):** the user asked for the whole review queue to be screened for findings
meeting all of these criteria: "it is CLEARLY a bug to be fixed. it's not ambiguous and doesn't need judgement that it
should be fixed given the nature of the product and its spec", "the fix seems reasonbly straight froward in the sense
that it won't add significant new complexity, require a re factor, etc.", and "its still relevant given current code vs
being stale due to code having changed"; and to triage that set as `fix code` "with an explicit complexity gate - it
must bounce back to me if during fixing it turns out to be more complicated and not fit the criteria". The decisions are
recorded in root `TRIAGE_OUTCOMES.md` (landed in #1896), one entry per feedback file, each with its own complexity gate
in its Completion criteria.

**The user's plan-time decisions (2026-10-11):** "let's use the planning system to schedule execution of the fixes".
Asked how to group the 31 outcomes, the user chose seven plans with one PR each; this plan is the supervisor lifecycle
tests group, so its outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these outcomes
only. Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also
a planner proposal: it follows the precedent of earlier grouped triage plans and is how this plan honors the user's
"bounce back to me".

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; Sharing the machine with other agents; the rule
against tests that modify the test process's own environment variables), `plans/AGENTS.md` (Executing one plan),
`review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for test changes, `docs/test-sleep-check.md`
for the sleep checker, and `docs/test-run-evidence.md` with `.agents/narrow-tests.md` for running the supervisor's unit
tests on the pinned nextest and tmux through the recorder.

## Outline

Line numbers drift; find the code by name. Verified on main at 3f430cc4. All changes are in tests and test-only helpers
of `crates/farhelm-supervisor`; no production code changes.

- Shared owned-child guard (serves the stop and restart outcomes). `crates/farhelm-supervisor/src/service/sweep.rs`, in
  the test `stopped_process_guard_resumes_a_stopped_owned_process`, defines a local `FixtureChild` wrapping
  `std::process::Child` whose Drop kills and waits; its doc notes that `Child` keeps the exit status after a manual
  reap, so Drop stays safe on the success path. The ledger asks only that the stop and restart tests share one guard,
  and both live in the `handlers.rs` test module. Default: a small kill-and-wait guard in that test module, used by both
  tests, with a doc carrying the same reap note; `sweep.rs` and `procs.rs` stay untouched. Optionally, if it is
  trivially clean, move the guard into the `#[cfg(test)]` module `sleeper` in `crates/farhelm-supervisor/src/procs.rs`
  as one test-only `pub(crate)` type and use it from the sweep test too; log the DECISION either way. Neither choice is
  a gate trip.
- Stop fixture: `crates/farhelm-supervisor/src/service/handlers.rs`, `stop_survives_connection_task_cancellation`. It
  spawns a raw sleeper, SIGSTOPs it and waits for the stop with `waitpid(WUNTRACED)`, then runs fallible setup (the
  supervisor, the session insert, the StopSession dispatch, a bounded wait for the claim, aborting the connection tasks,
  and a bounded poll for the process to go). Its only cleanup is the final `child.wait()`. Wrap the child in the shared
  guard right after spawn and before SIGSTOP; SIGKILL ends a stopped process. Keep the final `wait` and every assertion.
  The pid is never recycled under the guard because the test holds the unreaped child until the guard's own wait.
- Restart fixture: `restart_survives_connection_task_cancellation` in the same file has the same shape (with
  `Supervisor::new_with_seams`, a disabled scope manager and a RestartSession with `stop_if_running`). Same guard, same
  placement, assertions unchanged. The ledger asks for no mutation check for these two; optionally confirm once that a
  panic injected right after SIGSTOP leaves no stopped process, and log the result.
- HUP-resistant fixture: `crates/farhelm-supervisor/src/service/core.rs`,
  `delete_reaps_an_unmarked_process_under_a_tab_shell`. It opens a tab, probes the tab shell with `pane_process`, types
  an `exec env -i sh -c 'trap "" HUP; sleep 1000 & ...'` line that writes the sleep's pid to a pidfile, polls the
  pidfile, checks the processes with `procs::read_process`, deletes the session, and only in a final loop SIGKILLs
  survivors whose start time still matches. Add a guard armed with the tab shell's identity (pid and start time) as soon
  as the `pane_process` probe returns, before the line is typed; `exec` keeps the pid and start time. Record the sleep
  job's identity in the guard right after the test's own `read_process` of the pidfile pid, before any assertion on it
  (the parent check included). On drop, kill the job by identity if it was recorded, then, after re-checking the shell's
  identity, `killpg` the shell's process group: the pane process leads its own session and group, and a non-interactive
  `sh` keeps `&` jobs in its group, so the group signal also catches a job whose pid the test never learned. Every
  signal is preceded by a `read_process` re-read showing the same start time and a state that is not a zombie. The
  residual window between that re-read and the signal is accepted, following the precedent in
  `crates/farhelm-supervisor/src/tmux.rs`, which documents accepting it because pidfd is Linux-only; state this in the
  guard's doc. Reading the pidfile from inside the guard's drop, with a parent check, is an acceptable alternative; log
  the DECISION. The final survivors loop can fold into the guard, but the success-path assertion that delete reaped both
  processes stays as an assertion, not a cleanup. Gate: a fix that needs pidfd, a new process-tracking facility, or
  changes to `procs.rs` beyond test code.
- Stop intent: the existing `a_stop_that_cannot_record_its_intent_reports_instead_of_killing` in `core.rs` stores a row
  with a pane but an entry with no terminal, corrupts the row's `outcome_state` out of band with rusqlite, and asserts
  an Internal error containing "nothing was killed". Without a live pane, the StopSession handler in `handlers.rs` takes
  its dead-or-absent branch, whose failed classification write produces the same text. The running-agent path is
  `stop_live_agent` in `sweep.rs`: it records the stop intent first and returns `StopFailure::Unrecorded` ("recording
  the stop failed, so nothing was killed") before any sweep. The handler picks that branch purely from `pane_process`
  reporting a live pane on the entry's terminal, so no real launch is needed. Add a new test that reaches it with the
  existing test's shape (a stored row with a pane, `entry_with(...)`), but with a real pane created directly by
  `sup.tmux.create_session(...)` running `sleep 300` and the entry given `Some(Terminal { tmux_name, pane })`, as
  `reload_reconciles_a_stop_intent_against_the_pane_it_left_behind` in `core.rs` and `BusyHost::new` in `handlers.rs`
  already do. Premise: `pane_process` on that pane is owned and not dead, with its pid and start time recorded. Corrupt
  the row the same way, dispatch StopSession, assert the Internal "nothing was killed" error, and assert the same
  process (pid and start time) is still running. Only if that shape cannot reach the running-agent branch, fall back to
  a supervisor with a stub launch shim and the real create path (see the live-agent restart below), waiting for the
  launch to settle so no background write races the corruption. Rename the existing test to say it covers the
  dead-or-absent classification write failure, keep its assertions, and update its doc. Mutation check: in
  `stop_live_agent`, run the sweep before the intent write (or ignore the `Unrecorded` error); the new test must fail;
  restore. Gate: the corrupted row makes some other path fail first (for example status classification) so the
  running-agent path cannot be reached without new seams.
- Live-agent restart: `restart_reuses_live_agent_and_preserves_tabs` in `core.rs` uses `dummy_exe()`
  (`/nonexistent/farhelm`), so the agent never starts and `Supervisor::restart_session` takes its no-live-pane branch.
  Switch it to a supervisor with a stub launch shim, as in
  `unconfirmed_restart_is_refused_only_while_the_agent_is_working` (a `#!/bin/sh` script that `exec sleep 300`,
  `SupervisorSeams { launch_shell: Some("/bin/sh"), .. }`), assert the agent pane is live (owned, not dead) before the
  restart, call `restart_session(id, true, None, None)`, and keep the existing assertions that the tab list and the tab
  shell's pid are unchanged. Asserting that the old agent process is gone and a live agent is back is allowed but not
  required. `stop_if_running = true` keeps the restart clear of the working-agent consent refusal. Dead-agent coverage
  stays in `restart_replaces_only_the_dead_agent_window`; confirm it still covers what the old test effectively covered,
  and log it. Mutation check: make the live branch's stop sweep the whole session rather than only the agent; the tab
  shell dies and the test must fail; restore. Gate, as the ledger names it: a working launch in a supervisor unit test
  runs into status classification, consent or sweep markers, and reaching the live-agent restart path needs new fixture
  machinery.
- Skip tmux: `list_sessions_skips_pane_states_when_nothing_has_a_terminal` in `handlers.rs` kills the tmux server with
  `tmux -S <state>/tmux.sock kill-server` and asserts a session list. `TmuxDriver::pane_states` in `tmux.rs` treats the
  no-server refusals as an empty answer (`TmuxRefusal::means_no_panes`), so the listing would succeed even if the guard
  in `crates/farhelm-supervisor/src/service/listing.rs` (query pane states only when some entry has a terminal) were
  removed. Stimulus: after kill-server, remove the socket file (tolerating NotFound). tmux then reports "error
  connecting to <socket> (No such file or directory)", which `classify_tmux_refusal` maps to
  `TmuxRefusal::SocketMissing`, which is not tolerated, so the query errs. Assert `sup.tmux.pane_states().await` is an
  error as the premise, keep the listing assertion, and fix the doc's claim about what a query would return. Mutation:
  remove the guard in `listing.rs`; the test must fail; restore. Two stimuli to avoid: a directory at the socket path
  gets "no server running", which is tolerated (that is why `an_ambiguous_tmux_failure_keeps_the_launching_record` can
  use it for new-session but this test cannot), and removing the socket while the server is alive would leave the server
  unreachable by `StateDir`'s cleanup. Confirm the exact diagnostic on the pinned tmux; the classifier's own test,
  `refusal_classification_pins_every_recognized_diagnostic`, records the pinned texts. Gate: no stimulus produces a
  propagating error on the pinned tmux without a fake tmux binary or a new seam.
- Failed spec: `create_session_never_launches_tmux_after_a_failed_spec_publish` in `core.rs` makes `launch/` read-only,
  dispatches CreateSession, and asserts an error reply, no session entry and no stored row. Add an assertion that
  `sup.tmux.pane_states()` returns an empty map after the refused create. The server runs with `exit-empty off`, so an
  empty server answers with an empty list or a tolerated refusal. Mutation: in `Supervisor::spawn_agent` (where both the
  spec write and the tmux create live; the test's doc attributes them to `create_session`, so correct it), swap the spec
  publish and `self.tmux.create_session`; the pane survives the dummy agent's exit because of remain-on-exit, so the
  test must fail; restore. If a rollback path kills the window under the swapped order so the mutation passes, rethink
  the observation (for example, the server's session list) before treating it as a gate trip, and log it.
- Changelog: `test:`, so no fragment. A suggested title, to run through the cold read:
  `test: own supervisor lifecycle fixtures and sharpen their oracles`.

The complexity gate across these seven is a guard type and a handful of test edits. A production-code change, a new seam
in `Supervisor` or `TmuxDriver`, a fake tmux binary, pidfd, or new fixture machinery to make a launch work is a gate
trip for the outcome that needs it.

### Validation

Follow root `AGENTS.md` "Finishing work". The diff is test-only Rust in one crate:

- `cargo fmt --all -- --check`, and `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` (the shared guard
  is `#[cfg(test)]`, so the shipped binary's lint run adds nothing).
- A focused nextest selection through the recorder with the pinned nextest and tmux on PATH, as
  `.agents/narrow-tests.md` (Rust: recorded nextest selections) shows: `--runner nextest --kind development`, a
  selection naming the tests, `--concurrency '4 nextest slots; retries 0'`, `--tmux required`, and
  `cargo nextest run -p farhelm-supervisor --lib -E '<filter>'`. Select the seven changed tests, the new stop-intent
  test, the renamed dead-or-absent test, `restart_replaces_only_the_dead_agent_window`,
  `refusal_classification_pins_every_recognized_diagnostic`, plus
  `stopped_process_guard_resumes_a_stopped_owned_process` and the `sleeper` module's tests if the guard moved there.
- A short `--kind repetition` run of the tests that now start a live agent (the live-agent restart test, and the
  stop-intent test if it fell back to a real launch), since a working launch in a unit test is where timing could bite.
  Under load from other agents, treat a failure per root `AGENTS.md` (Sharing the machine) and keep the retained run.
- Each mutation check above as a focused recorded run against the mutated tree, logged with its run ID; restore and
  confirm the tree matches the commit before continuing.
- `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares; any new polling or
  settle wait carries a `sleep-ok` reason and a readiness oracle.
- `dprint check` on `TRIAGE_OUTCOMES.md` and `review_feedback_queue/INDEX.md`.

No browser, JS, desktop or installer checks apply. These tests are not Linux-gated, but this executor cannot run them on
macOS; log that as unverifiable on this host, not as a block.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-supervisor-lifecycle-test-fixes-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/supervisor-lifecycle-test-fixes/01-supervisor-lifecycle-tests`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. This PR changes
  only tests, so it is `test:` and needs no changelog fragment per root `AGENTS.md` (Releases and the changelog). If an
  outcome unexpectedly needs a production change, that is a gate trip, not a reason to change the type.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, a production-code change, or a product or
design decision), do not grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its
feedback file and index line, and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on
the user, followed by what you found and the realistic options with a recommendation (see `installer-startup-prune.md`
in the ledger for the shape). Name it among the open questions in the plan's report. The rest of the PR ships. The stop
and restart outcomes share one guard; drop only the outcome that trips, or both if the shared guard itself trips. Block
per Unattended fallback only when every outcome in this plan was dropped, since then there is no PR to deliver. Code
that turned out to be already fixed on main is not a gate trip: record the outcome as `discard` with "already fixed" and
the fixing commit, per root `AGENTS.md`, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. This PR changes tests and fixtures, so include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check: that every signal a guard sends is preceded by
an identity re-check (or targets an unreaped child), with the residual window documented, and that no guard can signal a
process the test did not start; that every guard is armed before the first fallible step after its process exists; that
the new stop-intent and live-agent tests assert their live-agent premise rather than assume it; that the skip-tmux
stimulus yields a propagating error rather than one `pane_states` tolerates; that each test would fail under the
mutation the outline names (supply the logged mutation results); and that the renamed dead-or-absent stop test's name
and doc match what it exercises. Where you disagree with a finding, decide on the merits and log the DECISION. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared formatter or protocol, edits outside the files the outline names beyond tests and bookkeeping), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where the shared owned-child guard lives and whether the sweep test's local guard
moved into it, the HUP-resistant guard's shape (recorded identity plus the process-group signal, or the pidfile read in
drop), the skip-tmux stimulus and the exact diagnostic the pinned tmux printed, how the stop-intent test reaches a live
pane (a direct pane, or the stub-shim fallback and how it waits for the launch to settle), the new name of the
dead-or-absent stop test, each mutation check with its result and run ID, the Conventional Commit type, every outcome
dropped by its complexity gate and why, and every review finding you decided not to follow. The user will ask for these
later.

### Unattended fallback

You never ask the maintainer anything in conversation while executing this plan: no question, not even an optional one,
and no offer of more work. Nobody is watching, so a question stalls the plan until someone happens to look. Every fork
is either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md` (Executing one plan, step 10).
Anything the maintainer should know, and any possible follow-up, goes into the plan's report.

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
