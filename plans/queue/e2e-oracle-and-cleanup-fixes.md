# Execute thirteen triage outcomes: end-to-end test oracles, fixture cleanup, and a Linux-only e2e suite

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out thirteen triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, in one draft PR. Their headings, which are
also the feedback file names under `review_feedback_queue/`:

- `short-writer-timeout-masks-shutdown-drain-regression.md` (`fix code`)
- `sink-healing-witness-consume-queued-output.md` (`fix code`)
- `restart-test-requires-screen-content-restart-correctly.md` (`fix code`)
- `environment-value-parsed-output-complete.md` (`fix code`)
- `persistence-assertion-rejects-legitimately-newer-activity-timestamp.md` (`fix code`)
- `readiness-failure-leave-systemd-probe-child-running.md` (`fix code`)
- `pipe-scan-assertion-failures-leave-fixture-processes-running.md` (`fix code`)
- `stop-escalation-fixtures-cleanup-own-whole-stubborn-tree.md` (`fix code`)
- `dispatch-overlap-fixture-has-same-incomplete-cleanup-armed.md` (`fix code`)
- `live-processes-reported-dead-macos.md` (`fix code`)
- `child-discovery-always-times-out-macos.md` (`other`, settled by the previous item's change)
- `unavailable-process-scan-certifies-no-marked-processes.md` (`other`, settled by the same change)
- `pid-cleanup-guard-inert-macos.md` (`other`, settled by the same change)

The planning PR that queued this plan set each of these outcomes' Execution field to "planned in
`plans/queue/e2e-oracle-and-cleanup-fixes.md`"; that is the state you start from, not `pending`.

All thirteen are defects in tests and test fixtures of the end-to-end binary that drives a real supervisor and real tmux
(`crates/farhelm/tests/e2e/`). Nothing changes for someone running Farhelm. They fall into three groups:

- Tests that can pass while the behavior they guard is broken, or fail while it is correct:
  - closing a connection to a peer that stopped reading is bounded by a drain window, but the test shortens a different
    timeout that can end the connection on its own;
  - a session's output sink that is killed comes back and the busy tab keeps being read, but the post-heal witness can
    be satisfied by output queued before the kill;
  - an rc-file edit reaches a restarted agent, but the test requires the first run's line in the new attachment, which a
    correct Restart clears from the visible screen;
  - the same test parses the environment value as soon as its prefix arrives, before the rest of the line;
  - a session's activity timestamp is persisted, but the test requires exact equality with a sampled value, which a
    later, legitimately newer write can overtake.
- Fixture cleanup that does not cover failure paths: the systemd probe child spawned without kill-on-drop, the pipe-scan
  test's `cat` and `sleep` with no cleanup, and the two stubborn-child fixtures whose PID guard owns one process (and is
  armed late in one of them).
- The end-to-end suite's `/proc` helpers, which on macOS report live processes as dead, time out discovering children,
  certify that no marked process survived without scanning, and leave the PID guard inert. The user chose to make the
  suite Linux-only rather than portable.

None of these was reproduced at runtime; all were confirmed by reading the code. Each ledger entry (Assessment,
Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read all thirteen
entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger entry wins
and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every changed test keeps its existing assertions unless its ledger entry says otherwise, and its docstring states what
  it now proves and why.
- On Linux the end-to-end binary still contains every test it contains today, under the same `module::test_name` names;
  on other platforms it reports visibly that it was skipped rather than running or passing anything.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- Every changed test passed in a recorded focused run on the pinned tmux, and the sleep check reports no unannotated
  delay.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11, recorded in `TRIAGE_OUTCOMES.md`, landed in #1919):** the review queue was
settled by class questions. For tests that can pass without exercising what they claim, or fail on correct code, the
user chose "Fix, skip hard ones": fix with a complexity gate, and if a fix needs a new test-only hook in product code,
record `discard` instead. For test setup that is not cleaned up when an earlier step fails, the user chose "Fix all,
with gate". For the Linux-only parts of the test harness, the user chose "Gate e2e to Linux": mark the end-to-end suite
(or those helpers' callers) Linux-only in one change so it never passes vacuously on a Mac.

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries". Asked how to group the outcomes, the user chose "8 plans as proposed"; this plan is the end-to-end Rust test
group. A planning review then moved two helm unit-test outcomes to `unit-test-oracle-fixes.md` and one browser-spec
outcome to `browser-oracle-and-fixture-fixes.md`, where they share files and test lanes. Asked about the PR shape, the
user chose "One PR per plan", so this plan's outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome
rule for these outcomes only. Review gate: "gpt-6.1-sol high", a single fresh-context agent with the general charter.
No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the test shapes in the outline below, checked against main at d3b28820. Handling a
complexity-gate trip by dropping that one outcome from the PR and shipping the rest follows the precedent of the earlier
grouped triage plans (#1901).

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Reproducing failures: narrow tests first; Sharing the machine with other
agents; Agent scratch space; the rule against tests that modify the test process's own environment variables),
`plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for
every test change, `.agents/narrow-tests.md` for the focused runs, `docs/test-run-evidence.md` for the recorder and the
pinned nextest and tmux setup, and `docs/test-sleep-check.md` for any delay a fix adds.

## Outline

Line numbers drift; find the code by name. Verified on main at d3b28820.

- Writer drain masked by the stall timeout (`short-writer-timeout-masks-shutdown-drain-regression.md`):
  `session_lifecycle.rs`, `writer_never_reading_peer_does_not_hang_connection_shutdown`, builds its harness with
  `writer_stall: Duration::from_secs(2)`. The drain bound it claims to pin is `WRITER_DRAIN_TIMEOUT` (5 s) in
  `crates/farhelm-supervisor/src/service/connection.rs` (`drain_writer`). With a 2 s stall timeout the writer can end on
  its own, so a regression back to an unconditional writer join could still pass inside the 30 s deadline. Make the
  stall timeout longer than the test's 30 s deadline, so that only the drain bound can end the connection in time. The
  test's own docstring says the stall timeout was what made the half-close reachable once the writer queue was bounded;
  the request count was then sized to fit the 4 KiB transport so the half-close is reachable either way. Confirm that
  claim with the longer stall (a hang here means it is false). Update the docstring. Gate: if the half-close cannot be
  reached without the short stall timeout, or without a seam into the drain constant, record `discard` (the ledger's
  rule); other complications go back to the user.
- Sink-healing witness (`sink-healing-witness-consume-queued-output.md`): `terminal_tabs.rs`,
  `a_killed_session_sink_comes_back_while_its_terminals_stay_attached`, clears `tab_seen` after healing but leaves
  frames already queued in `tab_rx`, then accepts the same `HEAL-FLOOD-` prefix. Require output produced after healing:
  for example, note the highest `HEAL-FLOOD-<n>` seen once the replacement sink is confirmed and the agent round trip
  done, drain what is already queued, and require a record well past it, or have the flood loop's successor print a
  fresh marker after the heal. Keep the premise assertion and the agent round trip. Gate: as the sibling entries
  (`discard` if a production hook is needed).
- Restart test that requires cleared content (`restart-test-requires-screen-content-restart-correctly.md`):
  `restart_with_resume.rs`, `an_rc_file_change_between_launches_reaches_the_relaunched_agent`. Before changing it, run
  it narrowly (one recorded repetition run, pinned tmux) and record whether it passes, fails or skips today; the
  reviewer's reading says it should time out on every run, yet it is not in `FLAKES.md` or `deflake/known-flakes.txt`.
  Then: the first marker is already checked before the restart (`observed != "first"` branch); after the reattach, wait
  for `ENV:FARHELM_RC_MARKER=second` on its own instead of `wait_for_after` anchored on the first line, and correct the
  comment that says the first line is "still in scrollback" (SPEC.md says Restart does not keep the previous run's
  visible screen; SPEC_impl.md's restart paragraph says the reused terminal keeps history and clears the grid). Waiting
  for the full `=second` string cannot match the first run's output. Gate: as the sibling entries.
- Environment value parsed early (`environment-value-parsed-output-complete.md`): the same test waits for the prefix
  `ENV:FARHELM_RC_MARKER=` and parses at once with `marker_value`, which takes whatever has arrived. The `env-echo`
  fixture (`crates/farhelm-fixtures/src/fake_agent.rs`, `env_echo`) prints that line and flushes before `basic()` prints
  `FAKE-AGENT READY`, so wait for `FAKE-AGENT READY` before parsing (the shape of the fixed
  `directory-witness-read-pwd-writes-it.md`). Keep the skip branch and its message. Gate: none expected beyond the
  ledger's.
- Activity persistence equality (`persistence-assertion-rejects-legitimately-newer-activity-timestamp.md`):
  `session_lifecycle.rs`, `wait_for_persisted_activity`, used by
  `a_session_dates_its_activity_to_creation_and_output_moves_it`, requires `stored == advanced`. Durable activity writes
  only move forward (SPEC_impl.md, activity persistence), so accept `stored >= advanced` and update the helper's
  docstring (it currently says the exact value is required) and the failure message.
- Systemd probe child (`readiness-failure-leave-systemd-probe-child-running.md`): `session_lifecycle.rs`,
  `a_systemd_255_sigkill_of_a_multithreaded_scope_member_still_tears_it_down`, spawns `systemd-run` without
  `kill_on_drop` and waits for readiness before arming `PidKillGuard`. Set `.kill_on_drop(true)` on the command before
  spawning (the scope runs the fixture in place, so killing the child handle kills the fixture), and keep the explicit
  waits on the success path. Gate: other complications go back to the user.
- Pipe-scan fixture (`pipe-scan-assertion-failures-leave-fixture-processes-running.md`): `terminal_tabs.rs`,
  `the_pipe_holder_scan_names_every_holder_of_a_live_pipe`, spawns `cat` and `sleep 60` with std `Command` and cleans
  them up only after the assertions. Arm a kill-and-reap guard right after each spawn (a small local drop guard owning
  the `std::process::Child` is enough; `MarkedDecoy` in `harness.rs` shows the pattern), keeping the duplicated writer
  alive for the measurement. Gate: other complications go back to the user.
- Stubborn-child fixtures (`stop-escalation-fixtures-cleanup-own-whole-stubborn-tree.md`,
  `dispatch-overlap-fixture-has-same-incomplete-cleanup-armed.md`): `session_lifecycle.rs`,
  `stop_kills_a_child_that_ignores_sigterm` and `cheap_request_completes_before_a_slow_spawned_handler_in_flight`, arm
  only `PidKillGuard::arm(child_pid)`, which owns one process, and the second arms it after its attach, readiness and
  `stubborn-ready` waits. Arm `MarkerCleanupGuard::new(session.id.clone())` (`harness.rs`) right after `create_session`
  in both, so every process carrying the session's marker is owned from then on (the pattern other tests already use,
  for example `terminal_tabs.rs`'s `a_killed_session_sink_comes_back_while_its_terminals_stay_attached`). Keeping the
  `PidKillGuard` as well is harmless. Gate: other complications go back to the user.
- Linux-only end-to-end suite (`live-processes-reported-dead-macos.md`, plus
  `child-discovery-always-times-out-macos.md`, `unavailable-process-scan-certifies-no-marked-processes.md` and
  `pid-cleanup-guard-inert-macos.md`): `harness.rs`'s `process_is_gone`, `children_of`, `marked_pids` and
  `proc_starttime` read `/proc` only. `crates/farhelm/tests/e2e/main.rs` declares every module without a platform gate.
  Gate the binary's contents to `target_os = "linux"` in `main.rs`, without renaming any test: test names are
  `module::test_name`, and exact names appear in `scripts/test-tmux-pinned-shutdown.sh`, `.config/nextest.toml` groups,
  `FLAKES.md` and `deflake/known-flakes.txt`, so wrapping the modules in a new parent module is not acceptable. A
  `#[cfg(target_os = "linux")]` on each `mod` line, the shared `cli_support` path module included (only the e2e harness
  uses it in this binary), plus, under `#[cfg(not(target_os = "linux"))]`, one test that prints a `SKIPPED:` line naming
  the reason is one acceptable shape; a crate-level `#![cfg]` would leave an empty binary with no visible skip. Add a
  sentence to the module docs saying why the suite is Linux-only (its process oracles read `/proc`; the user chose
  gating over porting). The x86_64 Linux release gate's pinned-shutdown subset and the local workspace nextest run are
  unaffected; no CI or release workflow runs this binary on macOS (the macOS jobs run the uninstall tests only), so the
  change stays contained. Gate: if the gate turns out to affect a CI or release selection beyond this file, bring it
  back to the user.
- Changelog: the PR is test-only, so its type is `test:` and, per root `AGENTS.md` (Releases and the changelog), it adds
  no changelog fragment. If a gate trip leaves only some outcomes, the type stays `test:`.

The complexity gate for all thirteen is a change confined to the named tests, their local helpers and fixtures, and
`main.rs`'s module declarations, plus docstrings. A change to product code, or a strengthened check exposing a real
product failure, is a gate trip.

### Validation

Follow root `AGENTS.md` "Finishing work" and "Reproducing failures: narrow tests first"; no full battery.

- First, before any change: the narrow run of
  `restart_with_resume::an_rc_file_change_between_launches_reaches_the_relaunched_agent` described in the outline,
  recorded with `--kind repetition`. Keep its run directory and log the result.
- `cargo fmt --all -- --check` and `cargo clippy -p farhelm --all-targets -- -D warnings` (covers the e2e binary).
- On Linux, list the end-to-end binary's tests before and after the gate (`cargo nextest list -p farhelm --test e2e`)
  and confirm the same set. To see the non-Linux branch compile and report its skip without a macOS machine, temporarily
  flip the cfg predicate locally, build and run that one skip test, and revert before committing; log that it ran. If
  you cannot do that cleanly, say so in the report.
- One focused nextest run of the e2e binary through `scripts/record-test-run.py` with `--runner nextest`,
  `--kind development`, `--tmux required`, the pinned nextest and tmux first on PATH (the guarded setup in
  `docs/test-run-evidence.md`, recipe shape in `.agents/narrow-tests.md`), `--concurrency '4 nextest slots; retries 0'`,
  and a selection naming exactly what runs, combined into one `-E` expression:
  `test(=session_lifecycle::writer_never_reading_peer_does_not_hang_connection_shutdown)`,
  `test(=terminal_tabs::a_killed_session_sink_comes_back_while_its_terminals_stay_attached)`,
  `test(=restart_with_resume::an_rc_file_change_between_launches_reaches_the_relaunched_agent)`,
  `test(=session_lifecycle::a_session_dates_its_activity_to_creation_and_output_moves_it)`,
  `test(=session_lifecycle::a_systemd_255_sigkill_of_a_multithreaded_scope_member_still_tears_it_down)`,
  `test(=terminal_tabs::the_pipe_holder_scan_names_every_holder_of_a_live_pipe)`,
  `test(=session_lifecycle::stop_kills_a_child_that_ignores_sigterm)`,
  `test(=session_lifecycle::cheap_request_completes_before_a_slow_spawned_handler_in_flight)`.
- Read the retained output for runtime `SKIPPED` lines, as root `AGENTS.md` requires: the rc-file test skips on a shell
  it does not know, the systemd test skips without a user manager, and the pipe-scan test skips without `/proc`. A skip
  of a changed test is not a pass; log it and say so in the report.
- A failure under another agent's load is an environment event (root `AGENTS.md`, Sharing the machine): keep the
  retained run, re-run narrowly, and do not log a flake from one observation.
- Optionally, a one-off local mutation check where it is cheap and decisive (for example, temporarily replacing
  `drain_writer`'s bound with an unconditional join and confirming the drain test now fails), reverted before
  committing; log whether it ran.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md`; it must report
  zero unannotated delays. Apply `.agents/test-authoring.md` to every changed test.
- `dprint check` on the changed Markdown (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`).

No browser, desktop, installer, JS-unit or doctest runs: nothing they cover changes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-e2e-oracle-and-cleanup-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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
  `plan/e2e-oracle-and-cleanup-fixes/01-e2e-test-fixes`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. This PR is
  test-only, so its type is `test:` and it adds no changelog fragment; if the change unexpectedly gains a user-visible
  effect, the type and a fragment under `releasing/changelog.d/` follow root `AGENTS.md` (Releases and the changelog),
  validated with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. The three `other` outcomes are removed in the same change as
  `live-processes-reported-dead-macos.md`. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. There are two kinds of trip, and the ledger entry says which
applies:

- For the test-oracle outcomes (the first five in the list), a fix that needs a new test-only hook in product code is
  discarded, per the user's "Fix, skip hard ones". Never rewrite the recorded Outcome. In the same PR, set that
  outcome's Execution field to "`complete`: gate tripped (<reason>); discarded per the recorded decision", followed by
  the jj change ID, bookmark and PR URL, and remove its feedback file and index line.
- Any other trip (a new mechanism, cross-component plumbing, a change well outside the files the outline names, a
  product or design decision, the Linux gate reaching beyond `main.rs`, or a strengthened test that exposes a real
  product failure) is brought back to the user: leave that outcome out of the PR, keep its feedback file and index line,
  and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what
  you found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the
  shape). If `live-processes-reported-dead-macos.md` trips, its three `other` siblings stay with it.

Either way, name the outcome in the plan's report, and the rest of the PR ships. Block per Unattended fallback only when
every outcome in this plan was dropped, since then there is no PR to deliver. Code that turned out to be already fixed
on main is not a gate trip: record the outcome as `discard` with "already fixed" and the fixing commit, per root
`AGENTS.md`, and remove its queue item.

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
the user's decisions. The PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Also ask the reviewer to check, for each changed test, that it would now fail on the regression
its docstring names and no longer fails on correct behavior: an unconditional writer join at connection shutdown; a busy
tab that stopped being read after the sink respawn; a relaunch that did not source the edited rc file, and a correct
relaunch that clears the visible screen; a split environment line; a newer persisted timestamp. Ask it to check that
every fixture process is owned for cleanup from the moment it exists, and that the Linux gate keeps every test name on
Linux and reports a visible skip elsewhere. Where you disagree with a finding, decide on the merits and log the
DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared harness helper beyond the guards named, edits outside the files the outline names beyond tests and
bookkeeping), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

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
alternatives considered: in particular what the narrow run of the restart test showed before any change, the stall
timeout chosen for the drain test, how the sink-healing witness establishes output after the heal, the shape of the
Linux gate and its visible skip, whether the cfg-flip check and any one-off mutation check ran, every runtime `SKIPPED`
seen, the Conventional Commit type, every outcome dropped by its complexity gate and why, and every review finding you
decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out (and discarded) outcomes removed.
Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
