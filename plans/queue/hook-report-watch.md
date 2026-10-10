# Conversation reports are applied as soon as they are written

Written against main at 2af378bd on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan runs after `plans/queue/sweep-on-timer.md` (D1): that plan makes the supervisor's ticker the only thing that
applies report files (list and info requests stop doing it) and explicitly leaves this watch out. Start from what it
landed: read its PRs, its report (`plans/reports/sweep-on-timer.report.md` while it awaits review) and SPEC_impl.md as
they are on main, and treat anything below that describes the code as it was at 2af378bd as a pointer, not a fact.

## The goal

Agent conversation hooks write their reports as files, and the supervisor applies them on its periodic pass, every two
seconds. Make pickup immediate with a file-system watch on the report directories, so a report is applied as soon as it
is written rather than up to a pass later. The periodic pass stays as the backstop.

Acceptance criteria:

- A report written while the supervisor runs is applied promptly after its file appears, without waiting for the next
  tick, on Linux and on macOS (D2). "Promptly" means event-driven: on Linux, well under a second on an idle machine; on
  macOS, whatever latency the platform's batching adds, which is accepted. The test proves it with a ticker interval
  long enough that no tick can explain the pickup.
- The watch is built on the `notify` crate, current 8.x (D2). The supervisor creates `<state_dir>/hook-reports/` at
  startup if it is missing, with the same permissions the hook gives it, so the watch has something to attach to before
  the first hook runs; that creation must stay correct when a hook creates the directory at the same moment. New
  per-session directories are picked up as they appear.
- Applying a report on a watch event runs the whole report pass, exactly the same drain as the periodic pass (the
  take-by-rename, the drain order, retry and settle, orphan cleanup) and the same rules about which supervisor may apply
  reports (a handoff candidate does not). It never runs a whole tick, and it never blocks the ticker.
- No event is lost to a drain already in progress: today an ordinary drain is skipped when another is running
  (`try_lock`), so an event that lands during a drain must cause one more drain when it ends, not wait for the next
  tick. Bursts of events (a hook writing a temporary file and renaming it, several sessions at once) are coalesced.
- The watch must not spin. A report the drain retries is put back into its slot (by hard link), which is itself a file
  event; without a pause, a retry during a store or tmux outage would drain, retry and re-fire in a tight loop for the
  whole outage. Leave a short pause (on the order of 100 ms) between watch-triggered drains; it also coalesces each
  hook's write-then-rename into one drain.
- If the watch cannot be set up or fails later (an inotify watch limit, an unsupported file system, a backend error),
  the supervisor logs it once, keeps working on the periodic pass exactly as today, and does not crash or spin. A report
  is never applied later than it would be without the watch.
- The watch stops with the supervisor (shutdown, handoff, a test supervisor dropped): no thread, file descriptor or task
  outlives it.
- SPEC_impl.md's "Report files" paragraph, which says "There is no file watcher", describes the watch, the backstop and
  the fallback. The ticker documentation that explains the two-second interval says reports no longer depend on it.
- Tests per Validation, including the immediate pickup proof (a session's first report, written into a session directory
  created after the watch started, which is the realistic race), the event-during-drain case, a burst ending with every
  report applied (correctness only, no drain counting), and the fallback when the watch cannot be created (through a
  test seam that makes creation fail, never by exhausting the machine's real watch limit or changing the process
  environment).
- The last code PR removes the TODO.md entry "Pick up hook report files immediately." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Pick up hook report files immediately. Once conversation hooks write their reports as files
(`plans/queue/hook-report-files.md`), the supervisor reads them on its periodic reconciliation pass, every two seconds.
Make pickup immediate with an inotify-style watch (or the macOS equivalent) on the report directories, so a report is
applied as soon as it is written rather than up to a pass later. Kept out of that plan on purpose, to keep it small."

**The user's decisions (2026-10-09):**

- D1. This plan runs after `sweep-on-timer.md` has landed, because that plan rewrites the same pickup code and leaves
  this watch out on purpose.
- D2. Use the `notify` crate on every platform, so the Mac's local host gets immediate pickup too. The maintainer asked
  for "a super solid well regarded low risk library to get the portability without a bunch of downstream risk" and does
  not mind the dependency as long as it is that. `notify` (about 168 million downloads, more than 3,000 dependent
  crates, used by rust-analyzer, cargo-watch, watchexec, Deno and mdBook, 8.2.0 current in August 2026) was presented as
  that library, with macOS event batching named as its known weak spot and the periodic pass as the safety net.
- D3. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- `notify`'s recommended watcher in recursive mode on the report root, whose callback (on `notify`'s own thread) wakes a
  `tokio::sync::Notify`. One supervisor task waits on it and runs the whole report pass with `wait = true`. The
  `Notify`'s single stored permit is the coalescing and the drain-again latch: an event during a drain leaves a permit,
  so the task drains once more. The ticker keeps its `try_lock`, so it is never blocked by the watch task.
- The watch task shares the ticker's lifecycle and its `Weak` reference to the supervisor, so it ends when the
  supervisor does.
- On failure, log once and never re-create the watcher; the periodic pass carries on.
- Create the report root with the same `DirBuilder` settings the hook uses (`hook_report.rs`), so both creators agree.
- macOS backend: `notify`'s default (FSEvents). Choose kqueue only if FSEvents misbehaves for this layout (renames into
  a watched directory), and log why.
- The test seam: a supervisor seam that replaces watcher creation with a failure, beside the existing
  `SupervisorSeams::ticker_interval`.

**What the planner found (at 2af378bd; re-check against what `sweep-on-timer` landed).**

- The writer: `farhelm internal hook` (`run_inner` in `crates/farhelm/src/hook.rs`) calls `hook_report::write_report`
  (`crates/farhelm-supervisor/src/hook_report.rs`, `REPORTS_DIR = "hook-reports"`), which creates
  `hook-reports/<session-id>/` (mode 0700) itself, writes `.tmp-<slot>-<pid>-<nanos>` with `create_new` and mode 0600,
  and renames it onto its slot (`latest.json`, `selection.json`, `enrichment.json`). The Goose hook and the Pi and OMP
  assets go through the same command.
- The reader: `crates/farhelm-supervisor/src/service/report_files.rs`. `apply_report_files` lists the root, drains each
  published session's directory (`drain_session_dir`) and removes orphans; drains are serialized by the `report_drain`
  mutex, ordinary callers `try_lock` and skip, Restart and `reconcile_for_test` wait. Today it is reached through
  `capture_now`/`capture_pass` (`service/capture.rs`), which `sweep-on-timer` narrows to the ticker, startup, reload and
  Restart.
- The runtime: tokio multi-thread. `start_ticker` (`service/ticker.rs`) is one task selecting over stop, the tick
  deadline, and a pane-died wake that runs `reap_pass` outside the schedule, the closest existing pattern for an
  out-of-schedule wake. An accepted report already hints helms (`finish_reported_admission` → `capture::advance_capture`
  → `hint_sessions_changed`).
- Dependencies: no file-watching crate is in `Cargo.lock`. Release targets (`dist-workspace.toml`) are
  `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` and `aarch64-apple-darwin`; on the Mac the supervisor also
  runs inside the desktop app.
- Tests: unit tests in `report_files.rs`; end-to-end tests in `crates/farhelm/tests/e2e/hook_identity.rs` using
  `hook_harness()`, which runs the real `serve()` and ticker; their helpers call `sup.reconcile_for_test()`, which an
  immediacy test must not call. `wait_for_hook_log_words` shows the bounded-poll pattern with a `// sleep-ok:` reason.

**Binding repository constraints:** root `AGENTS.md`: Finishing work (targeted validation through the recorder with the
pinned nextest and tmux; the test-sleep check; `cargo clippy -p farhelm --bins` for the shipped binary's configuration),
Releases and the changelog (a fragment for `perf`), Testability (no tests that change the process environment),
Reproducing failures: narrow tests first, Sharing the machine (tests must not exhaust shared kernel resources such as
the inotify watch limit), Agent scratch space, The live install is off-limits. `.agents/test-authoring.md` for any test
change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: watch the report directories

`perf:` with a changelog fragment (status and conversation tracking update sooner after an agent reports). The
dependency, the root directory created at startup, the watcher and its lifecycle, the event-to-drain task with
coalescing and the drain-again flag, the fallback and its single log line, SPEC_impl.md and the ticker docs, the
immediacy, event-during-drain, burst and fallback tests. Remove the TODO.md entry.

### Out of scope

Watching anything other than the report directories (launch status files, transcripts, checkout state), changing the
report file format or the hook, changing the tick interval, and removing the periodic drain.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the supervisor's report
tests and the `hook_identity`, `codex_identity` and `restart_with_resume` end-to-end tests through the recorder
(narrowest first, per `.agents/narrow-tests.md`), a repetition run of the new immediacy test to show it is not timing
sensitive, the test-sleep check, `cargo check -p farhelm-desktop` (the desktop app links the supervisor),
`dprint check`, and `python3 releasing/check-changelog.py format`. For macOS, try
`cargo check -p farhelm-supervisor --target aarch64-apple-darwin` (adding the target with `rustup` if it is missing); if
it cannot run on this machine, say so. Either way, say in the report whether the FSEvents path was compiled, and that it
was not exercised unless you exercised it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-hook-report-watch-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/hook-report-watch/<nn>-<short-name>`.
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

Before implementing a substantial departure from the outline above (watching other files, a hand-rolled inotify or
kqueue layer instead of `notify`, a second report format, changing the tick interval; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the user's request and decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the `notify` version and features and the macOS backend, how coalescing, the
drain-again latch and the pause between drains work, how the root directory is created, and the fallback behavior, and
every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
