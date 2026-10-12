# Execute thirteen triage outcomes: supervisor, helm and test-sweep unit tests that prove what they claim

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out thirteen triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR.
Their headings, which are also the feedback file names under `review_feedback_queue/`:

- `ignored-transcript-feeder-write-errors-conceal-missing-input.md`
- `recreation-test-assumes-distinct-birth-timestamps.md`
- `no-directory-created-assertion-requires-directory-exist.md`
- `prompt-classification-test-finish-waiting-prompt-appears.md`
- `shutdown-race-test-establish-intended-interleaving.md`
- `logging-test-establish-refresh-completed.md`
- `macos-environment-test-accepts-failed-observations-success.md`
- `shared-silence-observers-have-same-premature-success-boundary.md`
- `replay-refusal-tests-do-directly-observe-absence-delete-plain-replace.md`
- `replay-refusal-tests-do-directly-observe-absence-delete-replace-with-overrides.md`
- `omp-child-report-fixtures-bypass-nested-runtime-check.md`
- `busy-pane-assertion-depends-another-panes-readiness.md`
- `cleanup-leaves-orphan-tmux-processes-kill-failure.md`

All thirteen are defects in in-crate tests (`farhelm-supervisor`, `farhelm-helm`) or in the test-state sweep crate
(`farhelm-teststate`) that every harness runs at startup. Nothing changes for someone running Farhelm. Each test either
can pass while the behavior it guards is broken, can fail while that behavior is correct, or leaves test-owned processes
behind:

- The terminal stream must drop a terminal query that tmux already answered. The test feeds the query through a helper
  that ignores write errors and then only checks that nothing came out, which an empty stream also satisfies.
- Re-cloning over a checkout Farhelm made must be seen as a different directory. The test assumes the recreated
  directory gets a different birth time, which a coarse filesystem clock tick can deny it, so it can fail on correct
  code.
- Archive reconciliation with the checkout gone must report it missing and invent no checkout. The test says "nothing is
  created" but can only pass if the empty archive folder was created.
- A real pane showing an approval prompt must classify as waiting. The test counts the pane as settled before the prompt
  has necessarily been drawn, so a slow shell makes it fail on correct code.
- A helm shutdown must win against a host-list reconciliation already in flight. The test's single yield does not force
  that overlap, so it can pass without exercising it.
- A connected host's reconnection log must not log a line per refresh. The test never waits for a refresh to complete,
  so a noisy logger could pass.
- On macOS, the test pinning Apple's withholding of a platform binary's environment passes even when every observation
  failed.
- Many helm "refused by itself, sent nothing to the host" tests share a silent fake supervisor whose two-second window
  starts at the handshake, not after the request under test, so on a slow machine they can pass without watching it.
- Replacing a session must never delete the source when the host's create reply names the source itself. The plain
  Replace and the Replace-with tests check the refusal, but their fake host stops reading right after its create reply,
  so a Delete sent afterwards would go unseen.
- Three OMP child-report tests build their competing reporter as an unrelated process tree, which an earlier check
  rejects, so they never reach the nested-runtime check they are named for.
- A continuously printing pane must stay running while a quiet neighbour goes idle. The test only waits for the quiet
  pane, so a briefly descheduled printer makes it fail on correct code.
- The startup sweep of dead test runs deletes a run's directory even when it skipped or failed to kill that run's tmux
  server, which removes the socket path the next sweep would need to retry, so a test-owned tmux server can survive
  forever.

None of these was reproduced at runtime; all were confirmed by reading the code. Each ledger entry (Assessment,
Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read the thirteen
entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger entry wins
and the disagreement is a DECISION to log (or a gate trip, per Complexity gate). The planning PR that queued this plan
set each outcome's Execution field to "planned in `plans/queue/unit-test-oracle-fixes.md`"; that, not `pending`, is the
expected starting state.

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every changed test keeps its existing assertions unless its ledger entry says otherwise, and its docstring states what
  it now proves and why.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- Every changed test that can run on Linux passed in a recorded focused run (on the pinned tmux where tmux is involved),
  and the sleep check reports no unannotated delay. The macOS-only test is validated as described under Validation.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11, recorded in root `TRIAGE_OUTCOMES.md`, landed in #1919):** the user settled
whole classes of findings with one answer each. For tests that "can pass without exercising what they claim, or can fail
on correct code" while the product works, the user chose "Fix, skip hard ones": fix them under a complexity gate, and
where a fix would need a new test-only hook in product code, discard it instead. For test setup that is not cleaned up
when an earlier step fails, the user chose "Fix all, with gate". Every one of this plan's ledger entries carries its own
gate in its Completion criteria.

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries". Asked how to group them, the user chose "8 plans as proposed"; this plan is the supervisor and helm unit-test
group. Asked about PR shape, the user chose "One PR per plan", so this plan's outcomes share one PR, overriding root
`AGENTS.md`'s one-PR-per-outcome rule for these outcomes only. Review gate: "gpt-6.1-sol high", a single fresh-context
agent with the general charter, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a gate trip by dropping that one outcome from the PR and shipping the rest is also a planner
proposal, following the precedent of earlier grouped triage plans.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Reproducing failures: narrow tests first; Sharing the machine with other
agents; Agent scratch space; the rule against tests that modify the test process's own environment variables),
`plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for
the test changes, `.agents/narrow-tests.md` for the focused runs, `docs/test-run-evidence.md` for the recorder and the
pinned nextest and tmux setup, and `docs/test-sleep-check.md` for any delay a fix adds.

## Outline

Line numbers drift; find the code by name. Verified on main at d3b28820. No product code changes except where an outcome
names its own test helper; the test-state sweep crate is test infrastructure, not product.

- Transcript feeder (`ignored-transcript-feeder-write-errors-conceal-missing-input.md`):
  `crates/farhelm-supervisor/src/tmux/stream.rs`, test module. `stream_over_transcript` spawns a task that runs
  `let _ = feeder_stdin.write_all(transcript).await;`. Make a failed write fail the test: `.expect(...)` inside the task
  and keep its `JoinHandle` reachable, or write the transcript before returning, whichever stays local to the helper; a
  panic in a detached task alone is not enough, since nothing would observe it. In
  `passthrough_queries_bypass_stripping_across_every_wrapper_split`, change the bare-query transcript to carry a
  sentinel after the query (for example `%output %0 \x1b[6n\n%output %0 after-query\n%exit\n`) and assert the stream
  yields exactly `OutputEvent::Bytes(b"after-query")` and then `None`, so an empty or missing input can no longer pass
  as "stripped". Check whether the query and the sentinel can be coalesced into one `Bytes` event by the stripper; if
  so, assert the collected bytes equal the sentinel instead. Update the test docstring. Gate: anything beyond this
  helper and this test.
- Recreated directory (`recreation-test-assumes-distinct-birth-timestamps.md`):
  `crates/farhelm-supervisor/src/working_copies.rs`, `a_recreated_checkout_directory_is_a_different_object`. After the
  recreate, read the new directory's birth time with the module's own `observe(path)` (the `Observed.birth_ns` field;
  Linux uses `statx`, macOS its own `observe`). If the inode was reused and the birth time equals the recorded
  `fresh.path_birth_ns`, the premise did not hold: print a `SKIPPED:` line naming it and return, as the test already
  does for a filesystem without birth times. Otherwise keep the `DifferentObject` assertion. Update the docstring to say
  which premises it checks. FLAKES.md already notes this exposure for the test; no new entry. Gate: anything beyond this
  test.
- No-directory assertion (`no-directory-created-assertion-requires-directory-exist.md`): same file,
  `reconcile_reports_source_missing_when_nothing_remains`. The contract the ledger settled: no checkout directory is
  invented, and an empty archive folder may exist (reconciliation creates it before it finds the source missing, and
  SPEC.md does not forbid that). Assert that `bar` does not exist again and that the archive folder is either absent or
  empty (the `entries` helper panics on an absent folder, so check existence first), and rewrite the docstring and the
  assertion message to state that contract. No production change. Gate: none expected beyond this test.
- Prompt classification (`prompt-classification-test-finish-waiting-prompt-appears.md`):
  `crates/farhelm-supervisor/src/service/ticker.rs`, `a_prompt_on_a_real_pane_classifies_waiting_through_the_sampler`.
  Before starting the ticker, wait (within the file's `TEST_DEADLINE`, polling at `TEST_TMUX_POLL_INTERVAL` with a
  `sleep-ok:` reason) until `sup.tmux.capture_pane_tail(...)` for each of the two sessions contains the dialog's last
  line, `Esc to cancel`, reading the session's terminal from the sessions map the way `wait_for_busy_pane_to_change`
  does. Only then start the ticker and wait for the quiet streaks; the existing Waiting and Idle assertions stay. A
  small local helper shared by both panes is fine. Gate: anything beyond this test and a local helper.
- Shutdown race (`shutdown-race-test-establish-intended-interleaving.md`): `crates/farhelm-helm/src/manager.rs`,
  `a_shutdown_beats_a_reconciliation_that_is_already_running`. `sync_registry` takes `self.reconcile`, then awaits
  `self.store.list_hosts()`, then checks `map.shut_down`. To force the overlap without a new production hook, use the
  existing `HelmStore::connection_for_test()` (`crates/farhelm-helm/src/store.rs`): hold the database with a `Db::call`
  whose closure blocks on a channel the test controls, spawn the reconcile, wait until it holds the reconcile lock (the
  test is inside the module, so `manager.reconcile.try_lock()` failing is observable; since the store read is the next
  await after taking that lock, a held lock plus a held database means the reconcile is at the read), run `shutdown()`,
  release the database, and await the reconcile. Keep every existing assertion. Rewrite the docstring, which wrongly
  calls `yield_now` the barrier. First confirm `Db::call` actually serializes on one connection (read
  `crates/farhelm-supervisor/src/db.rs`); if it does not, or holding the read needs anything new in product code, the
  outcome is discarded per its ledger entry (bookkeeping as Complexity gate says). Gate: anything else that does not fit
  a contained test change goes back to the user.
- Reconnection log (`logging-test-establish-refresh-completed.md`): same file,
  `the_reconnection_trail_names_the_host_and_its_current_destination`. Replace the single
  `advance(REFRESH_INTERVAL * 5)` with a loop that advances one `REFRESH_INTERVAL` at a time and, after each, waits for
  a completed refresh: either the host's published state reaching `Connected { last_refresh: RefreshHealth::Ok { .. } }`
  on the first pass and a further list request on later ones (the test transport in the same module records requests),
  or another observable completed publication, whichever is direct. Then retarget and check the log as now. Keep the
  destination and span assertions. Gate: anything beyond this test.
- macOS environment withholding (`macos-environment-test-accepts-failed-observations-success.md`):
  `crates/farhelm-supervisor/src/procs.rs`, `a_platform_binary_childs_environment_is_withheld_on_modern_macos`
  (`#[cfg(target_os = "macos")]`). Order matters: first poll `read_process_argv(pid)` until it returns the post-exec
  argv of `/bin/sleep` (argv[0] ending in `sleep`, then the `30` argument); only environment reads made after that point
  count, so a pre-exec environment read can never pair with a post-exec argv read. Count an observation only when such a
  later `read_environ(pid)` returns `Some`. Require at least one, check the child is still running (`try_wait` is
  `None`) at the end of the window before killing it, and fail with a clear message when the exec was never seen or no
  successful observation happened. Update the docstring. Gate: anything beyond this test.
- Shared silent supervisor (`shared-silence-observers-have-same-premature-success-boundary.md`):
  `crates/farhelm-helm/src/rest_harness.rs`, `silent_supervisor`, used by about a dozen tests in
  `crates/farhelm-helm/src/sessions_tests.rs` (about 15 callers). The window must cover the request under test instead
  of starting at the handshake. Dropping the harness is not a proven EOF: a held router keeps the `ConnectionManager`
  alive, and the spliced relay ends only on its kill broadcast or a helm-side EOF. Do not use `fleet.kill_connection`
  either, because its `select!` can drop frames still buffered. The helper's docstring explains it uses bounded silence
  rather than EOF on purpose, so pick one of the two smallest shapes and log it as a DECISION:
  - (a) the ledger's literal wording: the helper takes a oneshot receiver, and its two-second window starts when the
    caller fires it after the response under test has arrived;
  - (b) each caller calls `harness.manager.shutdown()` after its last request and before awaiting the peer task, an
    ordered close of the helm side, and the helper reads until that EOF and fails on any frame, with a generous overall
    deadline that fails loudly.

  Either way, a one-line mechanical edit per caller is within the gate. Confirm the spliced harness sends no background
  traffic (refreshes) to this peer; if it does, the helper ignores that traffic by kind. Update the helper's docstring.
  Gate: anything beyond the helper plus a one-line edit per caller (reshaping callers, a product hook) trips the gate,
  and under the user's "Fix, skip hard ones" decision the outcome is discarded (see Complexity gate).
- Replay refusals (`replay-refusal-tests-do-directly-observe-absence-delete-plain-replace.md` and
  `replay-refusal-tests-do-directly-observe-absence-delete-replace-with-overrides.md`): same file,
  `a_create_reply_that_replays_the_source_id_is_refused_before_any_delete` and
  `a_replace_with_create_reply_that_replays_the_source_id_is_refused_before_any_delete`. Each scripted peer answers the
  create with a reply that names the source id and then stops reading, so a later `DeleteSession` would go unseen. After
  the refusal response and the existing assertions, call `harness.manager.shutdown()` for an ordered helm-side close;
  the peer keeps reading after its create reply until that EOF and fails if any `DeleteSession` (or any other control
  that would act on the source) arrives. Keep the existing conflict, body and cache assertions. The two share one shape
  and can share a small local helper. If observing the absence needs a production hook, both outcomes are discarded per
  the gate.
- OMP child reports (`omp-child-report-fixtures-bypass-nested-runtime-check.md`):
  `crates/farhelm-supervisor/src/service/core.rs`, the OMP runtime fixture's `spawn_sibling` and its three callers
  (`omp_child_report_before_parent_establishes_nothing`, `omp_child_reports_after_parent_leave_the_binding`,
  `omp_concurrent_child_and_parent_reports_resolve_to_the_parent`). The competitor must be spawned under the pane, by
  the first runtime, so its ancestry passes the pane-anchor check (`report_files.rs`) and reaches the nested-runtime
  corridor check (`core/vendor/omp.rs`). The fixture writes its own shim files into scratch; extend the shim so the
  first runtime can start a nested runtime on request (for example when a trigger file appears), keep both trees in the
  fixture's owned process group, premise-assert that the competitor's parent chain reaches the pane pid, and keep the
  existing refusal assertions so they now fail if the nested-runtime rejection is removed. A missing Bun interpreter is
  still a `SKIPPED` early return. This is a harness that is not first-class (OMP), so keep the fix small: expect a few
  dozen lines of fixture (the shim's trigger, a spawn helper, the ancestry premise) and edits to three call sites. If it
  is heading well past that, stop early: that is a gate trip. Gate: a product-code hook means the outcome is discarded;
  anything else beyond a fixture-local change goes back to the user.
- Busy pane (`busy-pane-assertion-depends-another-panes-readiness.md`): `ticker.rs`,
  `a_busy_pane_classifies_running_and_a_quiet_one_decays_to_idle`. After the quiet pane's streak reaches the threshold,
  shut the ticker down, call the existing `wait_for_busy_pane_to_change(&sup, "busy")`, then drive one `sample_pass` so
  the busy pane's sample records that change, and only then classify both. That mirrors
  `a_busy_pane_stays_running_when_the_rotation_samples_it_rarely`. The quiet pane's extra sample keeps it idle. Update
  the docstring. Gate: anything beyond this test.
- Startup sweep (`cleanup-leaves-orphan-tmux-processes-kill-failure.md`): `crates/farhelm-teststate/src/lib.rs`, `reap`
  and `kill_tmux_server`. Have `kill_tmux_server` report whether the server is confirmed gone (the `kill-server` command
  exited, successfully or because nothing answers on the socket) or not (spawn failure, deadline, wait error), and have
  `reap` keep the directory, with a diagnostic, when any socket in it was skipped for budget or not confirmed gone, so
  the next sweep retries it. Update `TMUX_KILL_TOTAL_BUDGET`'s and `reap`'s docs, which currently promise a retry the
  code makes impossible. A permanently wedged socket is then retried on every sweep, bounded by the existing per-kill
  deadline and total budget; log that tradeoff as a DECISION. Add or extend a unit test in the crate for the
  kept-directory case if it fits the existing test style without new machinery. Gate: anything beyond these two
  functions, their docs and a test.
- Changelog: the PR is test-only, so its type is `test:` and, per root `AGENTS.md` (Releases and the changelog), it adds
  no changelog fragment. If a gate trip leaves only some outcomes, the type stays `test:`.

### Validation

Follow root `AGENTS.md` "Finishing work" and "Reproducing failures: narrow tests first"; no full battery.

- `cargo fmt --all -- --check`, and
  `cargo clippy -p farhelm-supervisor -p farhelm-helm -p farhelm-teststate --all-targets -- -D warnings`.
- One focused nextest run through `scripts/record-test-run.py` with `--runner nextest`, `--kind development`,
  `--tmux required`, the pinned nextest and tmux first on PATH (the guarded setup in `docs/test-run-evidence.md`, recipe
  shape in `.agents/narrow-tests.md`), `--concurrency '4 nextest slots; retries 0'`, and a selection naming exactly what
  runs, combined into one `-E` filter expression: in `farhelm-supervisor`, the changed tests by their full paths (the
  transcript, recreation, no-directory, prompt-classification and busy-pane tests, and the three OMP child-report
  tests); in `farhelm-helm`, the two manager tests, the two replay-refusal tests, and every test that calls
  `silent_supervisor` (take their paths from `cargo nextest list`, since the helper change affects all of them); and the
  whole `farhelm-teststate` package. Confirm the exact test paths with `cargo nextest list` before the run rather than
  guessing module names.
- Read the retained output for runtime `SKIPPED` lines, as root `AGENTS.md` requires: the recreation test skips without
  birth times or now when its premise fails, and the OMP tests skip without Bun. A skip of a changed test is not a pass;
  log it and say so in the report.
- The macOS-only test cannot run on a Linux executor. Check that it still compiles by reading it against the macOS
  `procs` functions it calls (both are `pub(crate)` with macOS implementations), and if a macOS cross-check is available
  (`cargo check --target aarch64-apple-darwin -p farhelm-supervisor --tests` with that target installed), run it. If the
  target is missing, or the cross-check fails on Linux for toolchain reasons (a missing SDK, linker or C toolchain for
  the target) rather than a type error in this test, that means "not available": log it as a DECISION and name it in the
  report and the review prompt, not a failure. Do not add a Linux variant of a macOS-only test.
- A failure under another agent's load is an environment event (root `AGENTS.md`, Sharing the machine): keep the
  retained run, re-run narrowly, and do not log a flake from one observation.
- Optionally, a one-off local mutation check where it is cheap and decisive (for example, removing the shutdown check in
  `sync_registry` and confirming the race test now fails, or removing query stripping and confirming the transcript test
  fails), reverted before committing; log whether it ran.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md`; it must report
  zero unannotated delays. Apply `.agents/test-authoring.md` to every changed test.
- `dprint check` on the changed Markdown (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`).

No browser, desktop, installer, JS or doctest runs: nothing they cover changes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-unit-test-oracle-fixes-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/unit-test-oracle-fixes/01-unit-test-oracles`.
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

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above. Two
kinds of trip, handled differently, because the user's decision was "Fix, skip hard ones":

- A fix that would need a new test-only hook, seam or accessor in product code, or (for the shared silent supervisor)
  more than the helper plus a one-line edit per caller: the outcome is discarded under the user's agreed fallback, which
  is not a question. Never rewrite its recorded Outcome and do not put the reason in its Assessment. In the same PR, set
  its Execution field to "`complete`: gate tripped (<reason>); discarded per the recorded decision", followed by the jj
  change ID, bookmark and PR URL, and remove its feedback file and index line.
- Any other growth (a new mechanism, cross-component plumbing, a change well outside the files the outline names, a
  product or design decision, or a strengthened test that exposes a real product failure): do not grow it and do not
  block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index line, and in the same
  PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you found and the
  realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape). Name it among
  the open questions in the plan's report.

The rest of the PR ships either way. Block per Unattended fallback only when every outcome in this plan was dropped,
since then there is no PR to deliver. Code that turned out to be already fixed on main is not a gate trip: leave the
recorded Outcome as it is, set Execution to "`complete`: already fixed by <commit or PR>" with the change ID, bookmark
and PR URL, and remove its queue item.

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
its docstring names and no longer fails on correct behavior: query stripping removed, or the feeder delivering nothing;
equal birth times on a reused inode; a checkout directory invented by reconciliation; a prompt drawn late; the shutdown
check moved before the store read; a refresh logger that logs every tick; every macOS observation failing; a forbidden
request sent after the old two-second window; a `DeleteSession` sent after a replay refusal, through plain Replace or
Replace with; the nested-runtime rejection removed; a printer descheduled for a few samples; a wedged tmux server whose
directory the sweep deleted. Also ask it to confirm that every close the tests rely on goes through
`harness.manager.shutdown()` (or the oneshot shape), never a harness drop or `fleet.kill_connection`. Tell the reviewer
the macOS-only test was not run here. Where you disagree with a finding, decide on the merits and log the DECISION. Do
not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to product code, edits outside the files the outline names beyond tests and bookkeeping), and whenever the same
component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the ledger entries, the user's decisions above, this outline, the current diff and the proposed
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
alternatives considered: in particular how the feeder's write errors reach the test, the sentinel shape, how the
recreation test reads the new birth time, how the shutdown race holds the store read and whether that stayed inside the
test, how the logging test observes a completed refresh, which of the two shapes the silent supervisor's boundary uses
and which callers changed, how the replay-refusal peers observe the close, how the nested OMP runtime is started, how
the sweep decides a server is gone and the retry tradeoff, whether the macOS cross-check ran, whether any one-off
mutation check ran, every runtime `SKIPPED` seen, the Conventional Commit type, every outcome dropped or discarded by
its complexity gate and why, and every review finding you decided not to follow. The user will ask for these later.

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
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out or discarded outcomes removed. Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
