# Execute four triage outcomes: exited-tab cleanup off the status loop, the systemd flag probe, and two test fixes

Written against main at 4d94b3a3 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. The `e2e-oracle-and-cleanup-fixes` plan also edits
`crates/farhelm/tests/e2e/`, but not `hook_identity.rs`'s budget tests, and the `browser-oracle-and-fixture-fixes` plan
edits browser specs but not `e2e/start-stack.sh`'s harness wrappers; if either is unlanded when you rebase, expect a
mechanical rebase at most. The `helm-supervisor-small-fixes` plan prunes dead entries from the supervisor's `KeyedLocks`
table (the lifecycle claims live there, in `crates/farhelm-supervisor/src/service/core.rs`). Its pruning keys on a
strong count above zero, so a detached close task that still holds its lifecycle guard keeps its entry and stays safe.
If that plan lands first, re-check this during your careful rebase.

## The goal

Carry out four triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. The
planning PR that queued this plan set each one's Execution field to
`` planned in `plans/queue/supervisor-and-test-followups.md` ``; that is the state to expect at the start, not
`pending`. A different plan name, `complete`, or a changed Outcome means the entry changed after planning: leave that
outcome alone and say so in the report. Their headings, which are also the feedback file names under
`review_feedback_queue/`:

- `tab-cleanup-blocks-status-sampling.md`
- `unrelated-probe-failure-disables-argument-expansion-protection.md`
- `release-builds-fail-held-stdin-hook-test.md`
- `opt-in-real-agent-smoke-launches-fake-claude-executable.md`

What each one is about, for someone running Farhelm:

- When a terminal tab's shell exits but leaves a background process that ignores TERM and HUP, the supervisor's periodic
  status pass waits through that tab's whole cleanup (both process sweeps and the 5-second kill grace) before it samples
  agent screens. Every session on that host then shows stale status (running, idle, waiting, recent work) for about 5 s
  per such tab, around 20 s for four. SPEC.md "Waiting between operations on one host" forbids session status waiting on
  teardown grace periods.
- At startup the supervisor checks whether systemd supports `--expand-environment=no`, the flag that stops systemd from
  expanding `$` in a launch command. Any non-timeout failure of the flagged attempt followed by a successful plain retry
  is cached as "unsupported" until the supervisor restarts, so one transient error from the systemd user manager leaves
  launches whose paths contain `$` failing or running with the `$` expanded.
- One end-to-end hook test fails spuriously under `cargo test --release`, because the short hook budget it relies on
  exists only in debug builds. Nothing runs the tests that way today.
- The browser test stack always puts a fake `claude` first on PATH. So the two opt-in tests that are meant to drive a
  real Claude (set `FARHELM_REAL_AGENT=1`) launch the fake instead and fail visibly. They never pass falsely.

None of these was reproduced at runtime; all were confirmed by reading the code. Each ledger entry (Assessment,
Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read the four
entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger entry wins
and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every changed or new test has a docstring that says what it proves and why it matters, per root `AGENTS.md` and
  `.agents/test-authoring.md`.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, and passed the review gate.
- The checks under Validation passed, or are reported as skipped with the reason.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11, recorded in `TRIAGE_OUTCOMES.md`, landed in #1922):** each entry quotes the
user's words. They differ in what a gate trip means, and that difference is binding:

- `tab-cleanup-blocks-status-sampling.md`: the user accepted the recommendation, fix code with a complexity gate. A gate
  trip (a new scheduler, or a change to the lifecycle-claim design) goes back to the user.
- `unrelated-probe-failure-disables-argument-expansion-protection.md`: the user accepted "fix code only if it is a
  one-line check ... otherwise discard". A gate trip is a discard during execution (see Complexity gate).
- `release-builds-fail-held-stdin-hook-test.md`: the user accepted "a one-line debug-only gate on the test". A gate trip
  goes back to the user.
- `opt-in-real-agent-smoke-launches-fake-claude-executable.md`: after asking what the tests cover, the user chose: "fix,
  but maintainer responsiblity to set up credentials, no complexity in the repo for it. keep them opt in". A gate trip
  goes back to the user.

**The user's plan-time decisions (2026-10-11):** asked how to schedule the outcomes decided in this round, the user
chose "All new plans": leave the existing plans untouched and put the new fix outcomes into new plans, of which this is
the supervisor and test group. Review gate: "gpt-6.1-sol high", a single fresh-context agent with the general charter,
no swarm (the same choice the user made for this batch's earlier plans). PR shape: "One PR per plan", so this plan's
outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these outcomes only. No-workhorse mode,
which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at 4d94b3a3.
Handling a gate trip by dropping that one outcome from the PR and shipping the rest follows the precedent of the earlier
grouped triage plans (#1901, #1920).

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Reproducing failures: narrow tests first; Sharing the machine with other
agents; Agent scratch space; The live install is off-limits; the rule against tests that modify the test process's own
environment variables), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle),
`.agents/test-authoring.md` for any test change, `.agents/narrow-tests.md` for focused runs, `docs/test-run-evidence.md`
for the recorder and the pinned nextest and tmux setup, and `docs/test-sleep-check.md` for any delay a test adds.

## Outline

Line numbers drift; find the code by name. Verified on main at 4d94b3a3. The four fixes touch unrelated code and do not
interact.

- Exited-tab cleanup off the status loop (`tab-cleanup-blocks-status-sampling.md`): in
  `crates/farhelm-supervisor/src/service/ticker.rs`, both `sample_pass` (the tick) and `reap_pass` (the pane-death
  wakeup) call `reap_dead_tabs(sup, &states, &entries, stop).await` before `observe_entries` and sampling.
  `reap_dead_tabs` walks dead tabs, takes the session's lifecycle claim without waiting through
  `Supervisor::try_close_tab` (`crates/farhelm-supervisor/src/service/core.rs`), and then awaits `close_tab_claimed`,
  which already runs the close on a supervisor-owned `tokio::spawn` task holding the claim, but joins it (`task.await`)
  before returning. The close therefore blocks the pass for its whole duration. The smallest shape the ledger allows:
  give the reap a way to start the owned close task without joining it (for example a `try_close_tab` variant, or a
  `close_tab_claimed` split, that returns once the task is spawned, with the outcome logging and `hint_sessions_changed`
  moved into the task, which already hints on success). Duplicate closes are already prevented: the task holds the
  session's lifecycle claim until it finishes, so a later tick's `try_close_tab` for the same session returns `None` and
  skips it. Keep `REAP_BUDGET_PER_TICK` as the bound on how many closes one pass may start, and keep the stop check
  between starts. A user's explicit `close_tab` keeps its waiting, joined behavior. Update the docs on `sample_pass`,
  `reap_dead_tabs`, `REAP_BUDGET_PER_TICK` and `try_close_tab` that describe the serial, awaited reap and the "neither
  half can delay a request" claim, so they say what is true now (the reap starts bounded owned work; sampling no longer
  waits for it). Decide and log what shutdown does with closes still in flight: the existing supervisor-owned task
  already outlives its caller, so the likely answer is "they continue or are dropped with the runtime, like a user close
  in flight", but check that a planned stop's existing cleanup does not now race a half-done close in a way it did not
  before. Regression test (ticker tests in the same file already build supervisors with tmux and dead tabs; reuse that
  machinery): hold one tab's close in its shutdown phase (a tab whose child ignores TERM/HUP, as the finding describes,
  or an existing teardown seam if one fits), and prove another session still receives a fresh sample within one or two
  ticks while the close is still running. Gate: if this needs a new scheduler or queue, a change to how lifecycle claims
  work, or more than a contained change to the reap and close paths, stop and bring it back to the user.
- The systemd flag probe (`unrelated-probe-failure-disables-argument-expansion-protection.md`): in
  `crates/farhelm-supervisor/src/scope.rs`, `probe_round_trip` runs `probe_once(..., true)` and, on any failure that
  `should_retry_without_flag` allows (everything but a timeout), retries `probe_once(..., false)`; success there returns
  `Ok(false)`, which `probe_systemd` stores as `Tools::expand_environment_flag = false`. A failure anywhere in the
  flagged round trip (scope visibility, kill, collection), not only the flag being rejected, therefore turns the flag
  off for the supervisor's life. Do not route a transient failure anywhere new: `probe_systemd` maps every non-timeout
  `Err` from the round trip to `ProbeOutcome::Absent` (no manager, cached for the supervisor's life), so turning the
  flagged failure into an error would lose cgroup containment entirely, which the ledger rules out ("treats any other
  failure as inconclusive (retried later or treated as supported)"). The smallest correct design: keep
  `probe_round_trip`'s retry exactly as it is, and change only the value it returns after the unflagged retry succeeds.
  Return `Ok(true)` (flag supported) unless the flagged failure was specifically the flag being rejected, and
  `Ok(false)` only then. Decide "rejected" from `systemd-run --help` not naming `--expand-environment` (a short, bounded
  command, run with the probe's existing timeout discipline and `kill_on_drop`), rather than by capturing stderr from
  the long-running probe child. Unit-test the decision on canned help text (one that names the option, one that does
  not); the probe itself is not unit-testable without a manager. Update `probe_round_trip`'s doc comment to say what
  "the flag is unsupported" now rests on. Gate: the user allowed this only as "a one-line check". If the fix is more
  than a few lines plus a small test (for example it needs a new probe protocol or reshapes `ProbeOutcome`), discard it
  during execution as Complexity gate says.
- Release-profile hook test (`release-builds-fail-held-stdin-hook-test.md`): in
  `crates/farhelm/tests/e2e/hook_identity.rs`, `a_hook_whose_stdin_is_never_closed_still_finishes_in_budget` passes
  `hold_stdin = true` to `assert_silent`, which fails after `SILENCE_DEADLINE` (7 s). The child's 5 s budget comes from
  `FARHELM_TEST_HOOK_BUDGET_MS` set on the child command, which `hook_budget` in `crates/farhelm/src/main.rs` honours
  only under `#[cfg(debug_assertions)]`; a release child keeps the 30 s production budget and is killed at 7 s. The e2e
  test binary and the `farhelm` binary it spawns (`env!("CARGO_BIN_EXE_farhelm")`) are built in the same cargo profile,
  so the test's own `cfg(debug_assertions)` matches the child's. Mark that one test
  `#[cfg_attr(not(debug_assertions), ignore = "...")]` with a reason naming the debug-only override, and say in its
  docstring why. Ignore rather than compile out, so a release run reports it as ignored instead of silently dropping it
  (the ledger: "neither fails nor silently weakens it"). Do not lengthen `SILENCE_DEADLINE` and do not change `main.rs`.
  Check the other `assert_silent` callers: they do not hold stdin, so they finish on EOF and are unaffected. Gate:
  anything beyond that attribute and its doc.
- Opt-in real-Claude tests (`opt-in-real-agent-smoke-launches-fake-claude-executable.md`): `e2e/start-stack.sh` writes a
  fake wrapper for each structured harness name (the loop over `codex claude muse goose pi omp opencode`) into the run's
  `structured-bin` directory and prepends that directory to PATH in the private login profile (`.bash_profile` in the
  run's `structured-home`), which every session's login shell reads. The two opt-in tests, the last test in
  `e2e/tests/real-agent.spec.ts` (it drives a real Claude through the trust dialog to a detected reply) and the last
  test in `e2e/tests/spawn.spec.ts` (a real Claude creates a jj workspace and spawns into it), launch plain `claude` and
  so reach the fake. The fix: when `FARHELM_REAL_AGENT=1` is set in the environment `start-stack.sh` runs with, skip
  writing the `claude` wrapper only, and keep every other fake so a harness switch in any other spec still cannot reach
  a real vendor CLI; the login shell then finds whatever `claude` is next on the inherited PATH. First confirm that
  `FARHELM_REAL_AGENT` reaches `start-stack.sh`: `e2e/playwright.config.ts` starts it as its `webServer` with an `env`
  that names only the stack-port variable, and Playwright merges that with the runner's own environment; if it turns out
  not to, pass the variable through that same `env` object, not a new mechanism. The user ruled out any credential
  handling in the repository: the sessions run with the private login home, so making the real `claude` usable there
  (its login, any configuration) is the maintainer's job. Add one sentence saying exactly that where the opt-in is
  documented (the header comments of both specs, and the start-stack comment above the wrapper loop), and keep both
  tests opt-in with their loud skip lines. Ordinary runs (the variable unset) must install exactly what they install
  today. Also say in those comments that with the variable set, any structured spec that selects the `claude` harness
  would reach the real one too, so an opt-in run should be limited to the two real-agent tests (the existing
  instructions already run them by file). Gate: anything that copies, links or otherwise handles credentials, or changes
  more than the `claude` wrapper's installation and those comments.
- Changelog: the tab-cleanup fix changes what users see (status no longer goes stale while an exited tab is cleaned up),
  and the probe fix changes launches on affected hosts, so the PR type is `fix:` with one changelog fragment under
  `releasing/changelog.d/` per root `AGENTS.md` (Releases and the changelog) and `releasing/EDITORIAL_GUIDANCE.md`,
  written for someone running Farhelm: one entry for the status fix, and the probe fix in the same or its own entry if
  it ships. The two test-only outcomes get no fragment. If both product outcomes trip their gates, the type becomes
  `test:` and no fragment is written.

The complexity gate for each outcome is its bullet's Gate line above, together with its ledger entry.

### Validation

Follow root `AGENTS.md` "Finishing work" and "Reproducing failures: narrow tests first"; no full battery. Choose by what
actually changed:

- `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo clippy -p farhelm --bins -- -D warnings`.
- Focused nextest runs through `scripts/record-test-run.py` (`--runner nextest`, `--kind development`, the pinned
  nextest and tmux first on PATH per `docs/test-run-evidence.md`, `--concurrency '4 nextest slots; retries 0'`, a
  selection naming exactly what runs):
  - `-p farhelm-supervisor` for the ticker tests (the new regression and the existing reap tests in `ticker.rs`,
    selected by module) with `--tmux required`, and the `scope` module's tests with `--tmux none` if the probe changed.
  - `-p farhelm` for `test(=hook_identity::a_hook_whose_stdin_is_never_closed_still_finishes_in_budget)` in the default
    debug profile (it must still run and pass), with `--tmux none` unless the module needs tmux. Establish the release
    behavior by reasoning about the `cfg_attr` (the attribute applies exactly when `debug_assertions` is off, the same
    condition under which `main.rs` compiles the override out), not by a release build: even
    `cargo nextest list --release` compiles release test binaries, which is not warranted for one attribute. Log that
    reasoning.
- Read the retained output for runtime `SKIPPED` lines; a skipped changed test is not a pass.
- The stack change: `bash -n e2e/start-stack.sh` and `shellcheck e2e/start-stack.sh`, and read the wrapper loop to
  confirm that with `FARHELM_REAL_AGENT` unset it installs exactly the same wrappers as before. That is enough for a
  change confined to whether one wrapper is written. Run a browser spec only if a concrete risk remains after that
  reading (for example the condition is written in a way that could also skip the wrapper when the variable is unset),
  and then only one cheap structured-harness test that selects `claude`, through the recorder on Chromium, after the
  builds root `AGENTS.md` names. Never run either opt-in test: it needs the maintainer's vendor login and spends a real
  turn. Say in the report that the opt-in path was checked by reading `start-stack.sh` and the login profile, not by
  running it.
- If a test adds a delay, `python -B scripts/check-test-sleeps.py` with the isolated interpreter from
  `docs/test-sleep-check.md`; it must report zero unannotated delays.
- `python3 releasing/check-changelog.py format` for the fragment, and `dprint check` on the changed Markdown
  (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`, the fragment).

No desktop, installer, provisioning or website runs: nothing they cover changes. A failure under another agent's load is
an environment event (root `AGENTS.md`, Sharing the machine): keep the retained run, re-run narrowly, and do not log a
flake from one observation.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-supervisor-and-test-followups-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/supervisor-and-test-followups/01-followups`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (see Changelog in
  the outline). Changelog fragments follow root `AGENTS.md` (Releases and the changelog), validated with
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

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in its outline bullet. If
one turns out to need significantly more than the outline describes, do not grow it and do not block the whole plan for
it. What happens next depends on the outcome, as the user decided:

- `unrelated-probe-failure-disables-argument-expansion-protection.md`: never rewrite the recorded Outcome. Set its
  `TRIAGE_OUTCOMES.md` Execution field to "`complete`: gate tripped (<reason>); discarded per the recorded decision",
  followed by the jj change ID, bookmark and PR URL, and remove its feedback file and index line in this PR.
- `tab-cleanup-blocks-status-sampling.md`, `release-builds-fail-held-stdin-hook-test.md` and
  `opt-in-real-agent-smoke-launches-fake-claude-executable.md`: leave that outcome out of the PR, keep its feedback file
  and index line, never rewrite its recorded Outcome, and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to
  `pending`, waiting on the user, followed by what you found and the realistic options with a recommendation (see
  `installer-startup-prune.md` in the ledger for the shape). Name it among the open questions in the plan's report.

The rest of the PR ships. Block per Unattended fallback only when every outcome in this plan was dropped or discarded,
since then there is no fix to deliver; even then, if a discard alone makes a PR, deliver that. Code that turned out to
be already fixed on main is not a gate trip: keep the recorded Outcome, set Execution to `complete` with "already fixed"
and the fixing commit, per root `AGENTS.md`, and remove its queue item.

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
`AGENTS.md` requires. Also ask the reviewer to check specifically: that the status pass can no longer wait on a tab
close, that two closes of the same tab can never run at once, that the per-pass bound still holds, that a user's
explicit close still waits for its result, and that shutdown does not leave a half-finished close in a worse state than
before; that the regression test would fail if the reap went back to awaiting the close; that a transient non-timeout
probe failure on a systemd that knows the flag no longer turns the flag off, while an old systemd that rejects it still
falls back; that the release-profile hook test is reported as ignored, not compiled out, and still runs in debug; and
that with `FARHELM_REAL_AGENT` unset the test stack installs exactly what it did before, that no credential is copied or
referenced, and that only the `claude` wrapper is skipped when it is set. Where you disagree with a finding, decide on
the merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns
launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new scheduler or task
registry for closes, a change to `KeyedLocks` or the lifecycle claim, a new probe protocol, credential plumbing for the
test stack, edits outside the files the outline names beyond tests, docs and bookkeeping), and whenever the same
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
alternatives considered: in particular how the reap starts a close without joining it and where the outcome logging
moved, what shutdown does with closes in flight, how the regression test holds a close open, how the probe decides
"rejected" and the exact help text it matches, how `FARHELM_REAL_AGENT` reaches `start-stack.sh`, the Conventional
Commit type and the fragments written, every outcome dropped or discarded by its gate and why, every runtime `SKIPPED`
seen, every check skipped and why, and every review finding you decided not to follow. The user will ask for these
later.

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
complexity gate (and records the discards and returns the gate called for), meets the acceptance criteria above, and
passed the review gate, with every outcome's `TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the
carried-out and discarded outcomes removed. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
