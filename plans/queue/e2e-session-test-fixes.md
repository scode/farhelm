# Execute seven triage outcomes: end-to-end terminal and session tests that prove what they claim

Written against main at 3f430cc4 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out seven triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `cgroup-cleanup-masks-marker-discovery-regression.md`
- `replay-geometry-test-rejects-valid-live-output.md`
- `orphan-client-test-accepts-failed-inspection-proof-no.md`
- `partial-shell-result-parsing.md`
- `post-stall-liveness-witness-include-replay.md`
- `shutdown-tests-prove-session-existence-more-strongly.md`
- `hook-test-startup-delete-newly-created-launch-specification.md`

All seven are defects in the end-to-end tests that drive a real supervisor and real tmux (`crates/farhelm/tests/e2e/`).
Nothing changes for someone running Farhelm. Each test either can pass while the behavior it guards is broken, or can
fail while that behavior is correct:

- Stopping a session must kill a background daemon its agent started, even one that detached itself from the agent's
  process tree. On a host with systemd, the test for this passes through the session's systemd scope, so it would still
  pass if Farhelm's own portable discovery of such daemons were removed.
- Reattaching a terminal at a narrower size must replay the screen reflowed to that size. The test can fail on a correct
  supervisor when the fake agent's echo of the typed text arrives late, after the test has moved on.
- A supervisor that is killed outright must not leave stray tmux clients behind once its replacement starts. The test's
  check for a leaked terminal input pipe treats "could not look" as "found nothing", so it can pass without inspecting
  anything.
- A terminal tab must start in the session's working directory and see the user's shell configuration. Both tests can
  read the shell's answer before it has fully arrived and fail on correct output.
- A viewer that stops reading is detached after the stall timeout, and the agent must keep running. The test's proof
  that the agent is producing again can be satisfied by history replayed on reattach, so a pane that stayed stuck could
  pass.
- Stopping the supervisor in an orderly way (systemd stop, quitting the desktop app, or a terminal hangup) must leave
  every session's agent running. The test only checks that the tmux session still exists, which it does even when the
  agent died, because a dead agent's pane is kept.
- The agent-reported conversation identity tests start their supervisor and can create a session before the supervisor's
  startup cleanup has finished, which can delete the new session's launch file and fail the launch. Real clients cannot
  hit this: the supervisor accepts no connection until startup ends.

None of these was reproduced at runtime; all were confirmed by reading the code. Each ledger entry (Assessment,
Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read the seven
entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger entry wins
and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every changed test keeps its existing assertions unless its ledger entry says otherwise, and its docstring states what
  it now proves and why.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- Every changed test passed in a recorded focused run on the pinned tmux, and the sleep check reports no unannotated
  delay.
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
Asked how to group the 31 outcomes, the user chose seven plans with one PR each; this plan is the end-to-end terminal
and session tests group, so its outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these
outcomes only. Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also
a planner proposal: it follows the precedent of earlier grouped triage plans and is how this plan honors the user's
"bounce back to me".

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Reproducing failures: narrow tests first; Sharing the machine with other
agents; Agent scratch space; the rule against tests that modify the test process's own environment variables),
`plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for
the test changes, `.agents/narrow-tests.md` for the focused runs, `docs/test-run-evidence.md` for the recorder and the
pinned nextest and tmux setup, and `docs/test-sleep-check.md` for any delay a fix adds.

## Outline

Line numbers drift; find the code by name. Verified on main at 3f430cc4. All seven fixes are in the e2e test binary
(`crates/farhelm/tests/e2e/`, test names `module::test_name`); no product code changes. Two files carry two outcomes
each (`session_lifecycle.rs`, `terminal_tabs.rs`), but the edits are in separate tests and do not interact.

- Marker discovery masked by scopes (`cgroup-cleanup-masks-marker-discovery-regression.md`): `session_lifecycle.rs`,
  `stop_kills_a_reparented_marked_daemon`, builds its harness with `harness()`, whose default seams use the real systemd
  scope manager, so on a systemd host Stop's scope kill reaches the reparented daemon whether or not the
  environment-marker sweep works. Its two siblings right below it
  (`stop_kills_a_reparented_daemon_with_no_live_pane_to_walk_from`,
  `stop_kills_an_unmarked_child_of_a_reparented_daemon_via_closure_seeding`) already show the fix: build the harness
  with
  `harness_with_seams(SupervisorTimeouts::default(), SupervisorSeams { scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()), ..SupervisorSeams::default() })`,
  assert `launch_scope_of(&h, &session.id).await` is `None` after the create, and keep both `wait_until_pid_gone`
  checks. Add a sentence to the docstring saying scopes are off so that only the marker mechanism can reach the daemon.
  The scoped path stays covered by `a_scope_launched_stop_kills_through_the_cgroup_and_still_runs_the_sweep`. Gate: if
  the test fails once scopes are off, that is a real marker-discovery regression in the product, not a test fix; that is
  a gate trip (record what failed).
- Replay geometry (`replay-geometry-test-rejects-valid-live-output.md`): `session_lifecycle.rs`,
  `attach_replay_uses_the_requested_geometry`, waits for the bare payload before detaching, which the terminal's own
  echo of the typed line satisfies; a late fake-agent echo then lands after the reattach and trips the
  contiguous-payload check. Change the pre-detach wait to the agent's colored echo, `format!("echo:\x1b[36m{payload}")`
  (the `basic` fixture writes `echo:\x1b[36m<line>\x1b[0m`, as the test's own barrier wait already relies on). That is
  the whole ledger requirement: once the echo is awaited, no late payload bytes can follow the reattach. Keep the
  contiguous-payload assertion as it is, the 40-column reattach, the barrier, and the assertion message. Gate: anything
  beyond this test's waits and assertion.
- Leaked-writer check that accepts a failed inspection (`orphan-client-test-accepts-failed-inspection-proof-no.md`):
  `terminal_tabs.rs`, `a_killed_supervisor_leaves_no_orphaned_sink_client`, after `observe_protocol_cleanup_grace`,
  loops over `roster_pids(&control_client_roster(&sock).await)` and asserts `!scan.contains(" mode WRITE:")` on
  `stdin_pipe_holder_scan(pid)`. Two false passes: `control_client_roster` turns a failed, unspawnable or timed-out
  `list-clients` into text, which `roster_pids` parses as no pids; and `stdin_pipe_holder_scan_at` returns a report
  whose "scan INCOMPLETE", "/proc scan unavailable" or "stdin not inspectable" lines the assertion ignores. Fix both
  without changing the diagnostic callers (`wait_for_attached_client_roster`, `orphaned_client_report`, the reap
  waiters): give the roster query a `Result`-returning core with `control_client_roster` as the existing text wrapper
  over it (or, equally acceptable, check at the call site that every roster line starts with `pid=`, since every error
  text starts with "list-clients"), and have the scan also return a small structured verdict next to its report text,
  with `stdin_pipe_holder_scan` and `stdin_pipe_holder_scan_at` keeping their text for the report paths. A two-way
  verdict (conclusive, or inconclusive with the reason) is enough; matching on report wording is not, because "could not
  look" and "client exited" print the same "stdin not inspectable" line today. The leak check then panics with the
  report when the roster query failed or a scan was inconclusive, and passes when the scan is conclusive. A scanned
  client counts as gone (conclusive) only when `proc_root` itself is readable and `proc_root/<pid>` is absent; any other
  `fd/0` failure, a missing proc root included, is inconclusive, so a host without /proc fails loudly rather than
  passing having inspected nothing. A holder that vanishes mid-scan is skipped: the process-dir, fd-dir and fd-link
  `NotFound` cases already are, but a failed `fdinfo/<fd>` read is currently pushed onto the gaps whatever its error
  kind, and the scanned client holds its own stdin as fd 0, so a client exiting between that readlink and the fdinfo
  read would fail the test. Treat `NotFound` there like the other `NotFound` arms, as the helper's docstring already
  promises. A stdin that is not a pipe is conclusive: nothing can hold its write end. Extend the existing
  fabricated-proc tests (`the_pipe_holder_scan_reports_blind_spots_instead_of_swallowing_them`, plus a missing-client
  case, a missing-proc-root case and a vanished-fdinfo case) to pin the verdicts, so the rare-path code is exercised on
  every run. If the tmux server itself died (BUGS.md, abrupt supervisor death), the roster now fails loudly here instead
  of later; that is the intended effect, and the message should say what the roster returned. Gate: reshaping beyond
  these two helpers and this one call site, or finding that scans on an ordinary host are routinely inconclusive (which
  would make the test fail every run).
- Partial shell results (`partial-shell-result-parsing.md`): `terminal_tabs.rs`,
  `a_tab_runs_a_shell_in_the_sessions_working_directory` and the nested `tab_marker_value` in
  `an_rc_file_change_between_two_tab_opens_reaches_the_second_tab`, call `run_in_shell` with only the opening marker
  (`CWD[`, `ENV[`) and then parse up to `]`, so output split across terminal chunks parses as a truncated path or value.
  In the environment test a truncated value can also send the test down its SKIPPED branch on an unrecognized shell.
  Make each printf emit a closing marker the echoed command cannot contain, assembled from `%s` arguments the same way
  the opening marker is (for example `printf 'CW%s[%s]%s\n' D "$PWD" DWC` printing `CWD[<path>]DWC`, and the same shape
  for `ENV`), pass `]DWC` (and `]VNE`) to `run_in_shell` as the wait, and parse the text between `CWD[` and `]DWC`. The
  bare `DWC` would not do: the terminal echoes the typed command, which contains ` DWC` literally, while the echo has
  `]%s` where the output has `]DWC`. Output bytes arrive in order, so seeing the closing marker means the whole frame
  arrived. Both tests keep their assertions. Gate: changes to `run_in_shell` or `wait_for_shell` beyond a doc tweak.
- Post-stall witness (`post-stall-liveness-witness-include-replay.md`): `terminal_backpressure.rs`,
  `a_pause_past_the_stall_timeout_detaches_and_leaves_the_session_healthy`, computes `last_before_detach` from bytes the
  paused viewer received, then waits for `CUTOVER-{last_before_detach + 50}` in a buffer seeded with the reattach's
  replay. The counter keeps writing every few milliseconds while the viewer is stalled, so the replay itself can already
  hold that record. Take the highest record in the reattach's replay (`counter_records(&replay)`, falling back to
  `last_before_detach` when the replay has none) and require a record 50 past that, which can only arrive after the
  replay; the sibling `a_forced_tmux_pause_is_recovered_through_the_real_attachment` measures past its replay the same
  way. Update the comment above the witness. The rest of the test is unchanged. Gate: if the stronger witness fails, the
  pane really stays wedged after a stall detach; that is a product finding and a gate trip.
- Shutdown oracle (`shutdown-tests-prove-session-existence-more-strongly.md`): `supervisor_stop.rs`,
  `stop_attached_supervisor`, shared by `sigterm_closes_output_clients_in_order_and_exits_cleanly`,
  `a_closed_desktop_tether_closes_output_clients_in_order` and
  `a_hangup_to_the_supervisors_group_closes_output_clients_in_order`, ends with only `tmux has-session`, which holds for
  a dead agent because panes are kept on exit. After the existing checks, prove the agent still answers: send a unique
  line with `tmux send-keys` to the bare `fh-<id>` target (the `wait_for_agent_ready` docs explain why the `=` prefix
  must not be used on a pane target) and poll plain `capture-pane -p` (no `-e`, so the color codes are stripped and
  `echo:<line>` matches as plain text) for the `basic` fixture's echo of it within an explicit deadline. A reply is new
  progress, which implies a live pane; do not add a `#{pane_pid}` or `process_is_gone` check (the pane's root process is
  the login shell or launch shim, not shown to be the agent, and it would add the first /proc dependency to the file).
  Reading `#{pane_dead}` only to enrich the timeout message is fine. That round trip is the short bounded re-check the
  ledger anticipates for a slightly late death; its poll interval needs a `// sleep-ok:` reason per
  `docs/test-sleep-check.md` (a polling interval inside an explicit deadline). Keep every existing assertion, and update
  the helper's and the three tests' docstrings to say the agent's liveness, not just the session's existence, is
  checked. Gate: if the agent really dies in any variant, that is a product finding and a gate trip.
- Hook fixture startup race (`hook-test-startup-delete-newly-created-launch-specification.md`): `hook_identity.rs`,
  `ServeTask::spawn`, returns once `wait_for_supervisor_ready` connects, and in `Supervisor::serve`
  (`crates/farhelm-supervisor/src/service/core.rs`) the socket is bound before the startup snapshot and the awaited
  `sweep_launch_dir`, while connections are accepted only after startup ends. The test's own client talks over an
  in-memory pipe that bypasses that accept loop. After the existing readiness wait, still racing the serve task as the
  function does now, connect through `farhelm_supervisor::service::connect` and complete the protocol handshake with
  `SupervisorClient::start` under a bounded timeout, then drop that client. A completed handshake proves the accept loop
  runs, so startup reconciliation finished. Keep the change in `ServeTask::spawn`, not in the shared
  `wait_for_supervisor_ready`; its other callers (`codex_identity.rs`, `restart_with_resume.rs`, the ignored
  `real_agent_capture.rs` tests) gain the same barrier, which is harmless. Update the function's docstring with a third
  load-bearing ordering. Gate: a change to the shared readiness helper or to supervisor code.
- Changelog: the PR is test-only, so its type is `test:` and, per root `AGENTS.md` (Releases and the changelog), it adds
  no changelog fragment. If a gate trip leaves only some outcomes, the type stays `test:`.

The complexity gate for all seven is a change confined to the named tests and their local helpers, plus docstrings. A
change to product code, to a shared harness helper other than those named above, or a strengthened check exposing a real
product failure is a gate trip.

### Validation

Follow root `AGENTS.md` "Finishing work" and "Reproducing failures: narrow tests first"; no full battery.

- `cargo fmt --all -- --check` and `cargo clippy -p farhelm --all-targets -- -D warnings` (covers the e2e binary).
- One focused nextest run of the e2e binary through `scripts/record-test-run.py` with `--runner nextest`,
  `--kind development`, `--tmux required`, the pinned nextest and tmux first on PATH (the guarded setup in
  `docs/test-run-evidence.md`, recipe shape in `.agents/narrow-tests.md`), `--concurrency '4 nextest slots; retries 0'`,
  and a selection naming exactly what runs: the changed tests
  (`test(=session_lifecycle::stop_kills_a_reparented_marked_daemon)`,
  `test(=session_lifecycle::attach_replay_uses_the_requested_geometry)`,
  `test(=terminal_tabs::a_tab_runs_a_shell_in_the_sessions_working_directory)`,
  `test(=terminal_tabs::an_rc_file_change_between_two_tab_opens_reaches_the_second_tab)`,
  `test(=terminal_tabs::a_killed_supervisor_leaves_no_orphaned_sink_client)`, `test(the_pipe_holder_scan)`,
  `test(=terminal_backpressure::a_pause_past_the_stall_timeout_detaches_and_leaves_the_session_healthy)`), the
  `test(/^supervisor_stop::/)` module, and the modules whose fixtures go through `ServeTask::spawn`
  (`test(/^hook_identity::/)`, `test(/^codex_identity::/)`, `test(/^restart_with_resume::/)`), combined into one `-E`
  filter expression.
- Read the retained output for runtime `SKIPPED` lines, as root `AGENTS.md` requires: the rc-file tab test skips on a
  shell it does not know, and the pipe-scan tests skip without `/proc` or as root. A skip of a changed test is not a
  pass; log it and say so in the report. The marker-discovery test no longer needs systemd at all; the supervisor-stop
  tests run a real supervisor child that uses systemd scopes when available, which is fine either way.
- A failure under another agent's load is an environment event (root `AGENTS.md`, Sharing the machine): keep the
  retained run, re-run narrowly, and do not log a flake from one observation.
- Optionally, a one-off local mutation check where it is cheap and decisive (for example, temporarily disabling the
  marker sweep and confirming the marker-discovery test now fails), reverted before committing; log whether it ran.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md`; it must report
  zero unannotated delays. Apply `.agents/test-authoring.md` to every changed test.
- `dprint check` on the changed Markdown (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`).

No browser, desktop, installer, JS or doctest runs: nothing they cover changes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-e2e-session-test-fixes-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/e2e-session-test-fixes/01-e2e-session-tests`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. This PR is
  test-only, so its type is `test:` and it adds no changelog fragment; if the change unexpectedly gains a user-visible
  effect, the type and a fragment under `releasing/changelog.d/` follow root `AGENTS.md` (Releases and the changelog),
  validated with `python3 releasing/check-changelog.py format`.
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
cross-component plumbing, a change well outside the files the outline names, a product or design decision, or a
strengthened test that exposes a real product failure), do not grow it and do not block the whole plan for it. Leave
that outcome out of the PR, keep its feedback file and index line, and in the same PR set its `TRIAGE_OUTCOMES.md`
Execution field to `pending`, waiting on the user, followed by what you found and the realistic options with a
recommendation (see `installer-startup-prune.md` in the ledger for the shape). Name it among the open questions in the
plan's report. The rest of the PR ships. Block per Unattended fallback only when every outcome in this plan was dropped,
since then there is no PR to deliver. Code that turned out to be already fixed on main is not a gate trip: record the
outcome as `discard` with "already fixed" and the fixing commit, per root `AGENTS.md`, and remove its queue item.

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
its docstring names and no longer fails on correct behavior: marker discovery removed with scopes off; replay captured
before the attach-time resize; a leaked stdin writer hidden behind a failed roster query or an incomplete scan, while a
client exiting mid-scan still passes; a shell answer split across chunks; a pane still wedged after the stall detach; an
agent killed by an orderly supervisor stop; a launch file swept after the hook fixture returned. And ask it to check
that the leak check's diagnostic callers still get their text reports, and that every new delay carries a sound
`sleep-ok` reason. Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch
command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared harness helper, edits outside the files the outline names beyond tests and bookkeeping), and whenever
the same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with
this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff and the proposed
departure (what changed, why it is necessary, and which simpler alternative was ruled out):

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
alternatives considered: in particular the shape of the roster and scan verdicts and how a vanished client is told apart
from an unreadable one, the closing markers chosen for the shell probes, how the post-stall witness picks its baseline
when the replay has no records, the shutdown round trip's deadline and poll interval, the handshake timeout in the hook
fixture, whether any one-off mutation check ran, every runtime `SKIPPED` seen, the Conventional Commit type, every
outcome dropped by its complexity gate and why, and every review finding you decided not to follow. The user will ask
for these later.

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
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
