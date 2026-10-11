# Execute five triage outcomes: helm routing and upload tests, and supervisor portability and timing tests

Written against main at 3f430cc4 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out five triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `browse-routing-test-discards-wrong-host-assertion.md`
- `commit-race-test-exercise-only-pre-commit-abort-path.md`
- `dead-tab-regression-cannot-pass-ordinary-macos.md`
- `upload-memory-probe-assumes-linux-kib-pages-rss-rss-procfs-observation-macos-portability.md`
- `idle-flush-test-permits-deadline-extension-it-claims.md`

All five are test-only fixes. No product behavior changes; each test either fails to check what it claims, or cannot run
on a Mac at all:

- The New dialog's directory browser asks the helm to list a directory on the host the user picked. The test for that
  routing gives the helm's own local host a supervisor that must never be asked anything, then throws that supervisor's
  verdict away instead of waiting for it. A helm that sent the browse to the local host as well as the named one would
  still pass.
- When the user attaches a file to a session, the helm streams it to the supervisor and then asks it to commit the
  upload. The test meant to cover the supervisor aborting the upload while that commit request is in flight actually
  aborts before the helm sends the commit, so it exercises the earlier "already aborted" path and leaves the in-flight
  path untested.
- The supervisor removes tabs whose pane has exited, and the test for that starts a background daemon with `setsid` and
  watches it through `/proc`. Neither exists on an ordinary Mac, so the test cannot pass there, even though most of what
  it checks is portable.
- The test that checks a large upload does not grow the supervisor's memory reads `/proc/self/statm`, which is
  Linux-only, but is not gated to Linux.
- In the terminal, the supervisor holds back a partial escape sequence from a session's output for a short idle period
  and then flushes it. The test meant to prove that unrelated tmux traffic does not keep pushing that flush back waits
  without a bound under a paused clock, so a supervisor that did push the deadline back would still pass.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the five entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- The browse-routing test fails when a browse also reaches the local host. The commit-race test fails when the helm
  stops watching for an abort while its commit is in flight. The idle-flush test fails when unrelated traffic renews the
  flush deadline. Each was confirmed once with a temporary mutation that was then reverted. The dead-tab coverage has a
  portable test plus a Linux-only daemon test. The memory test is gated to Linux.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
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
Asked how to group the 31 outcomes, the user chose seven plans with one PR each; this plan is the helm and
portability/timing tests group, so its outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for
these outcomes only. Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also
a planner proposal: it follows the precedent of earlier grouped triage plans and is how this plan honors the user's
"bounce back to me".

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; the rule against tests that modify the test process's
own environment variables), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle),
`.agents/test-authoring.md` for test changes, `docs/test-sleep-check.md` (the sleep checker every Rust test change must
pass), `docs/test-run-evidence.md` (the recorder and the pinned nextest and tmux setup), and `.agents/narrow-tests.md`
(selection shapes for focused and repeated runs).

## Outline

Line numbers drift; find the code by name. Verified on main at 3f430cc4. All five are still present there; none is
already fixed.

- Browse routing: `crates/farhelm-helm/src/sessions_tests.rs`,
  `browse_directory_routes_to_the_named_host_and_preserves_its_reply`, spawns `silent_supervisor` (in
  `crates/farhelm-helm/src/rest_harness.rs`) as the local host's peer and ends with `local_task.abort()`. That discards
  the only check that the local host was not asked. Replace the abort with `local_task.await.expect(...)`, placed while
  `harness` is still in scope, the way sibling tests await their silent peers. `silent_supervisor` returns after its
  bounded silence window or on a clean EOF, and panics only when a frame arrives, so the await terminates. Update the
  test's doc comment to say both hosts' verdicts are checked. Confirm once by a temporary misroute in which the remote
  reply still arrives, for example making the `browse_directory` handler in `crates/farhelm-helm/src/sessions.rs` also
  send the browse to the helm's local host from a spawned task (or awaited with its result discarded), within the same
  request and so inside the silence window, so the response still comes from the remote host and only the awaited local
  peer can catch the misroute. If the extra local call decided the response, the local peer's panic would close its
  connection and the helm would answer with an error, failing the status assertion, which the old abort would have
  failed too; the same holds for a misroute that skips the remote host. Neither would prove the fix. Revert the
  mutation. The ledger puts the two-second silence window out of scope: a separate queued item,
  `shared-silence-observers-have-same-premature-success-boundary.md`, owns it. The create-routing test earlier in the
  same file also aborts its `local_task`, with a comment saying that is deliberate; it is not part of this finding, so
  leave it and name it in the report as a possible follow-up if it looks like the same gap.
- Commit race: `crates/farhelm-helm/src/uploads.rs`,
  `upload_attachment_abort_racing_the_commit_surfaces_the_abort_reason`. Its fake supervisor reads the data frame and
  sends `UploadAborted` straight away, so the abort is recorded before the helm calls `UploadGuard::commit`
  (`crates/farhelm-helm/src/client.rs`). `commit` returns an already-recorded abort without sending, so the in-flight
  arm (the `wait_ended` branch of the `select!` around sending `CommitUpload`) never runs. Change the peer to read the
  data frame, then `peer.recv()` the `CommitUpload`, asserting its variant and channel, then send `UploadAborted` with
  the sentinel reason and never answer the commit. Keep the 5-second response bound, the status assertion and the
  sentinel reason check unchanged, as the ledger requires. Rewrite the doc comment to describe the in-flight race.
  Confirm once by temporarily removing the `wait_ended` arm from `commit`: the test must then fail by timing out, not
  pass. Revert.
- Upload memory probe: `crates/farhelm-supervisor/src/files.rs`,
  `staged_stream_does_not_grow_memory_with_the_size_of_the_upload`, reads RSS from `/proc/self/statm` with no platform
  gate. The ledger decided cfg gating, not a portable probe: add `#[cfg(target_os = "linux")]` after the
  `#[farhelm_testtrace::test]` attribute, which is how other Linux-only supervisor tests are gated (the launch tests,
  and the provisioning gates #1834 added), and say in the doc comment why it is Linux-only. The test still runs on
  Linux. It re-execs its own test binary with a child marker and removes `FARHELM_TEST_TRACE_DIR` from that child's
  command; that sets the child's environment, not the test process's, and stays as is. If a helper or import becomes
  used only by this test, gate it the same way so the macOS build has no dead-code or unused-import warnings.
- Dead-tab reaping: `crates/farhelm-supervisor/src/service/ticker.rs`,
  `dead_tab_panes_are_reaped_by_the_tick_and_live_tabs_survive`, makes its first dead tab with a `setsid` daemon that
  writes a pid file, waits for the pid with `wait_for_daemon_pid`, asserts `/proc/<pid>` exists, runs `sample_pass`,
  then waits for the daemon to vanish with `wait_for_process_absence`, which reads procfs. Split it, as the ledger
  decided. The portable test keeps two plain dead tabs made with `dead_tab_in`, the live sibling, the listing assertions
  (dead tabs hidden from the ordinary listing, present in the including-dead one), the reaping pass, and the checks that
  both dead panes are gone and the live tab survives; no `setsid` and no procfs. A new `#[cfg(target_os = "linux")]`
  test keeps the daemon scenario with its pid-file wait, the `/proc` liveness premise before reaping, and the absence
  wait after it. `wait_for_daemon_pid` and `wait_for_process_absence` are used only by that scenario on main, so gate
  them with it; `TEST_READY_POLL_INTERVAL` and `dead_tab_running` have other users and stay portable. A portable
  liveness probe like the `kill(pid, 0)` helper #1834 added in `terminals.rs` would not make the daemon test useful on
  macOS: the ledger's assessment notes that macOS withholds the environment of platform binaries such as `/bin/sleep`,
  so the daemon sweep cannot be checked there. Gate it; do not port it. Give both tests doc comments that say what each
  covers and why the split exists.
- Idle flush: `crates/farhelm-supervisor/src/tmux/stream.rs`,
  `unrelated_control_traffic_does_not_starve_the_idle_flush`, runs on a paused clock. It feeds an own-pane `\x1b[`,
  advances 10 ms four times while writing `%window-renamed` lines, advances 10 ms more, and then awaits the read without
  a bound. Tokio's paused clock jumps to the next timer whenever the runtime is idle, including while the reader waits
  on the `cat` pipe, so a renewed deadline is simply reached by auto-advance and the test passes either way. In
  production, `next_output` sets `query_strip_deadline` to `QUERY_STRIP_IDLE_FLUSH` (50 ms) after an own-pane payload
  that leaves a held prefix, and the foreign-pane branch calls `silence_late_pane`, which writes a
  `refresh-client -A "%N:pause" -A "%N:off"` line to the stream's command sink. Two facts make a deterministic in-place
  shape possible: `query_strip_deadline` is only a field until `next_output` builds its `timeout_at`, so no timer is
  armed between calls, and `timeout_at` polls the line read first, so buffered lines are consumed at the current
  instant. The precedent is `the_idle_flush_fires_on_a_busy_stream_once_the_deadline_has_passed` in the same module,
  which writes everything first, asserts with `stream.reader.fill_buf()` that the bytes are buffered, and calls
  `next_output` in place with no spawn. Proposed test-only shape, on the existing fixture with `start_paused = true`:
  1. Feed own-pane `A\x1b[` and call `next_output` in place. It returns `A` and holds `\x1b[`, which arms the deadline.
     Take `T0 = Instant::now()`. Optionally assert `stream.query_strip_deadline == Some(T0 + QUERY_STRIP_IDLE_FLUSH)` as
     the premise.
  2. `tokio::time::advance(40ms)`. No timer is pending, so this is deterministic.
  3. Write the unrelated lines in one write, for example four `%window-renamed @0 unrelated` lines and a first `%output`
     from a foreign pane (`%7`), which also covers the foreign-pane branch.
  4. Assert with `stream.reader.fill_buf()` that the buffer ends with the last written line, as the busy-stream test
     does.
  5. Call `next_output` in place. It consumes every buffered line at `T0 + 40ms` and then parks on the pipe, and the
     paused clock auto-advances to whatever deadline is pending: `T0 + 50ms` when correct, `T0 + 90ms` if unrelated
     traffic renewed it.
  6. Assert the event is `\x1b[` and that `Instant::now() - T0` is under 60 ms (allowing for timer-wheel rounding).

  No fixture variant, spin helper, command-sink witness or `sleep-ok` interval is needed. Confirm once by temporarily
  resetting the deadline on unrelated traffic in `next_output` (the chatter or foreign-pane branch). The test must then
  fail. Revert. The ledger's gate names this outcome's likely trip directly: a fix that needs a new feed seam or
  observation point in production code no longer fits. If the `fill_buf` premise cannot be made to hold without such a
  change, trip the gate for this outcome alone. Repeat the test per Validation.
- Interactions: the five edits touch different test modules and share no helpers. The two supervisor portability
  outcomes follow the same cfg pattern; keep it consistent. Check for unused imports after the split and the gating when
  compiling for a macOS target.
- Changelog: `test:`, with no fragment. Every change is in tests and test fixtures, and nothing changes for someone
  running Farhelm. Root `AGENTS.md` requires a fragment only for `feat`, `fix`, `perf`, `style`, `revert` or breaking
  types.

The complexity gate for these five is a small edit per test plus doc comments. Any change to production code in
`client.rs`, `sessions.rs`, `files.rs`, `ticker.rs` or `stream.rs` is a gate trip; the temporary mutation checks are
reverted and do not count. So is a new shared test harness, a rework of `rest_harness.rs` beyond what the fix needs, or
a portable process-inspection layer for macOS.

### Validation

Follow root `AGENTS.md` "Finishing work". Choose the smallest set that covers the change:

- `cargo fmt --all -- --check`.
- `cargo clippy --all-targets -- -D warnings`. The changes are test-only, so `cargo clippy -p farhelm --bins` adds
  nothing.
- Focused nextest runs through `scripts/record-test-run.py --runner nextest --kind development`, with selections taken
  from `.agents/narrow-tests.md`:
  - `farhelm-helm` lib tests, with `--tmux none`: the browse-routing test and the commit-race test, for example
    `-p farhelm-helm --lib -E 'test(browse_directory_routes_to_the_named_host) | test(upload_attachment_abort_racing_the_commit)'`.
  - `farhelm-supervisor` lib tests, with `--tmux required` and the pinned tmux first on PATH using the guarded setup in
    `docs/test-run-evidence.md`, because the ticker tests drive tmux: the idle-flush test, both dead-tab tests and the
    memory test.
- Repeat the idle-flush test under the recorder with `--kind repetition`, as `.agents/narrow-tests.md` describes, a few
  dozen times; the in-place shape is deterministic by construction, so that is enough to support "passes reliably under
  load". Repeat the two helm tests a few times as well, since both race real tasks. If other work is running on the
  machine, note that in the log.
- The three mutation checks from the Outline: run each mutated test once, keep each failing run's record as evidence,
  and revert before committing. Log each one.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter `docs/test-sleep-check.md` describes.
- `dprint check` on the changed Markdown (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`).
- macOS: the dead-tab ledger entry requires "Confirm both compile on macOS". No CI job compiles `farhelm-supervisor`'s
  tests on macOS, and reading the cfg gates misses a portable test that still calls a now-gated helper, or an import or
  helper left unused on macOS. Run, with a separate target directory under the scratch directory:
  `CC_aarch64_apple_darwin=true AR_aarch64_apple_darwin=true CARGO_TARGET_DIR=<scratch>/darwin-target cargo clippy --locked -p farhelm-supervisor --tests --target aarch64-apple-darwin -- -D warnings`.
  The no-op C tools are needed because `libsqlite3-sys`'s build script otherwise invokes the host `cc` with Apple flags
  it rejects; clippy never links, so they are never really used. At planning time this passed on main at 3f430cc4 with
  zero warnings, so a failure after the change is attributable to it, not to the toolchain. Do not install a target. If
  `rustup target list --installed` shows no darwin target on the executing host, the macOS confirmation cannot be given:
  treat the two macOS outcomes as a gate trip (drop them, Execution back to `pending` waiting on the user, saying the
  cross check could not run), rather than shipping them unconfirmed. The focused macOS CI suite
  (`gh workflow run ci.yml -f suite=uninstall-macos`) is not a substitute: it runs only the `farhelm` binary's
  `uninstall::` tests and never builds `farhelm-supervisor`'s tests.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-helm-and-portability-test-fixes-log.md` in the parent directory of the checkout you run in, derived as
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
  `plan/helm-and-portability-test-fixes/01-helm-portability-tests`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. This PR is
  test-only, so its type is `test:` and it carries no changelog fragment. If a gate-trip resolution ever put a
  user-visible change in it, a `feat`, `fix`, `perf`, `style` or `revert` PR adds a fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog), validated with
  `python3 releasing/check-changelog.py format`.
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
cross-component plumbing, a change well outside the files the outline names, or a product or design decision), do not
grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index line,
and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you
found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape).
Name it among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback only when
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
the user's decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that the browse test would fail if the local host
also received the browse, and that the commit-race test would fail without the in-flight abort watcher in
`UploadGuard::commit`. Ask it to check that the idle-flush test has no unbounded wait or external `.await` that lets the
paused clock auto-advance, and that it would fail if unrelated traffic renewed the deadline. Ask it to check that the
portable dead-tab test uses neither `setsid` nor procfs, that the Linux-only test keeps its liveness premise before
reaping, and that the cfg gating leaves no unused helper or import on macOS. Where you disagree with a finding, decide
on the merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns
launch mechanics.

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
alternatives considered. In particular: the idle-flush test's shape and its elapsed bound, cfg gating versus a portable
probe for the two macOS outcomes, the misroute mutation used to confirm the browse test, the outcome of each mutation
check, what the macOS cross clippy showed, the Conventional Commit type, every outcome dropped by its complexity gate
and why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Nobody is watching this run. Never ask the maintainer anything in conversation: not a required question, not an optional
one, not an offer of more work, and not a harness question tool's prompt. A fork is either decided within the agreed
scope and logged, or it is a block per `plans/AGENTS.md` (Executing one plan, step 10). Anything the maintainer should
know, and any possible follow-up (such as the create-routing test's deliberate abort), goes into the report.

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. A host without a darwin target drops the two macOS
outcomes, per Validation. Block per `plans/AGENTS.md` (Executing one plan, step 10) only when no outcome is left to ship
or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
