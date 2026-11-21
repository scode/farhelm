# Triage outcomes: the supervisor's session lifecycle, shell sessions and cleanup

Written against main at e91ae9c6a on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Execute the 63 triage outcomes listed under Outline below, as a short stack of batched PRs. Each outcome is recorded in
`TRIAGE_OUTCOMES.md` under the name of its feedback file in `review_feedback_queue/`, and was decided on 2026-11-21 on
the maintainer's fewest-questions triage page (against main ca62397de). The ledger entry's Assessment and the feedback
file hold the finding; its Decision and Completion criteria say what to do, and they bind except where this file's
Decisions retarget them. The planner grouped the outcomes from their ledger entries and did not recheck each finding
against current main: line numbers in the Assessments are from ca62397de and may have drifted, so confirm each finding
in current code before changing anything.

### This plan's area, and the nine others

This plan's area is the supervisor's service layer (`crates/farhelm-supervisor/src/service/`: core, handlers, sweep,
teardown, status, the ticker and connections; `launch.rs`; its `store.rs`; the agent-screen fixtures' Rust guard
excepted) and the e2e suite with its fixtures (`crates/farhelm/tests/e2e/`, `crates/farhelm-fixtures/`): how sessions
stop, restart and are deleted, how shell sessions end, the cleanup of their processes and scopes, and the tests of all
of that. The terminal host runtime (`hosts/`) and grid output belong to `triage-terminal-host`. This is the largest
plan, mostly comment corrections and cheap test fixes.

This is one of ten plans that together execute every outcome triaged on 2026-11-21. The maintainer asked for them to be
split by area of code so they can run concurrently ("yes we definitely need to batch these in a much smaller set of
plans. please try to separate them by area of code so there is a good chance of them running concurrently."). Any of
them may run at the same time as this one. The other nine, and the areas this plan leaves to them:

- `triage-bookkeeping`: queue bookkeeping only: the 45 discards and the duplicated-code TODO entry, with no code or spec
  change.
- `triage-spec-text`: SPEC.md and SPEC_impl.md text, and user docs text, that records accepted current behavior.
- `triage-gui-sessions`: the desktop app's session list, sidebar, attention list, sign-in prompts, notices and session
  cards.
- `triage-gui-misc`: the rest of the desktop app: launcher, templates, Settings, Hosts panel, approval cards, terminal
  view and platform input.
- `triage-terminal-host`: the terminal path: the terminal host crate, the terminal emulator, the supervisor's grid
  output and host terminal runtime, the helm's terminal relay, the vendored terminal crates, and test-state cleanup of
  terminal hosts.
- `triage-helm`: the helm's provider-neutral code (hosts, provisioning, automatic update, tunnels, sessions and Trash,
  notices, agent requests, credential delivery), the CLI, and the desktop app's startup and updater.
- `triage-podman`: the Podman provider: container builds, probes and removal, container hosts and their identity, the
  connection manager's container branch, and the real-container e2e module.
- `triage-tensorlake`: Tensorlake sandboxes: the helm's Tensorlake code, adding a sandbox host, sandbox session
  lifecycle, and the Tensorlake test runner.
- `triage-tooling`: maintainer tooling: scripts, release and brick-test tooling, deflake, docs-media and agent-screen
  capture, the test recorder, worker and summary, the plans queue and landing scripts.

Do not change code in those areas unless an outcome of this plan genuinely requires it. When one does, keep the edit to
what the outcome needs, log the overlap as a DECISION, and name it in the report. Known overlaps:

- `trailing-interrupted-doc-defines-only-reboot-attribution-after-retirement.md` corrects a docstring in the protocol
  crate (`crates/farhelm-proto/src/lib.rs`), shared code whose subject here is the supervisor's interrupted status.
- `trailing-sampler-docs-promise-deleted-capture-failure-reset.md` corrects the ticker's comments and the matching
  SPEC_impl.md wording together.
- Several e2e tests here run real terminal hosts (`triage-terminal-host`'s code); change only the tests.

### What each kind of outcome asks for

The kind is read from the outcome's ledger Decision; the outline tags each outcome with it.

- **Fix, approved as straightforward** (Decision: Q8, "approve as listed for any where the fix is straight forward
  ..."): apply the recommendation its Completion criteria quote, as written, with a test where practical. Their "stop
  and bring it back to the maintainer" and "execution blocks" clauses are replaced by the complexity gate under
  Decisions. Where a recommendation offers "or discard" as its alternative, discard only if the fix turns out not to be
  worth it, and log why.
- **Other recorded decisions** (Decision: Q1, Q3, Q4, Q5 or Q6 on the triage page): build exactly what the Completion
  criteria describe; the question's picked answer is quoted in the Decision.
- **Correct the text** (Decision: Q12, "Correct the text"): change only the comment, doc or spec prose so it matches the
  accepted current behavior, as the Assessment describes. No behavior change.
- **Test gap: fix only if cheap** (Decision: Q11, "Fix only the cheap ones that guard product behavior"): fix the test
  only if the fix is a few lines (about 20 at most) and the test guards a user-facing behavior (a retired test whose
  behavior still applies counts); otherwise make no change. Either way the PR removes the item, and its Execution field
  says which way it went. The "Recommendation for reference" in the criteria is advice for that judgment, not a
  decision.
- **Tooling: fix only what corrupts evidence or crashes** (Decision: Q14): fix it only if the bug crashes a documented
  command, loses or misattributes test or release evidence, turns a real verdict into a harness failure, or lets private
  data into a fixture; otherwise make no change. Either way the PR removes the item, and its Execution field says which
  way it went. The "Recommendation for reference" is advice, not a decision.

### Acceptance criteria

1. Every outcome listed under Outline is executed exactly once, in one PR of this stack, as its ledger entry's Decision
   and Completion criteria say, retargeted only by the Decisions below. Each ends one of four ways: fixed as decided;
   judged no change where its criteria allow it; discarded under the unattended fallback; or deferred through the
   complexity gate.
2. A code fix makes the code match the rule its Assessment quotes, or builds the approach its Decision picked, with a
   test where practical. A fix that changes behavior SPEC.md or SPEC_impl.md describes updates them in the same PR.
3. Each outcome's Execution field, as moved to `TRIAGE_OUTCOMES_ARCHIVE.md`, names its PR (change ID, bookmark and link)
   and, where its criteria leave a choice or it was discarded or deferred, which way it went and why.
4. Each PR removes exactly its outcomes' feedback files and `review_feedback_queue/INDEX.md` lines, moves exactly their
   entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md`, and lists them in its description. On
   the stack's tip none of this plan's outcomes remains in `TRIAGE_OUTCOMES.md` or the queue.
5. Each outcome deferred through the complexity gate has its Near term TODO.md entry in the PR that removes it from the
   queue.
6. No PR exceeds about 300 lines of diff (not counting the ledger, the queue removals, changelog fragments or TODO
   entries), unless one outcome's accepted medium-size fix needs it on its own.

## Decisions already made

Settled with the maintainer at triage (each outcome's Decision) and on the 2026-11-21 planning page:

- **Ledger decisions are binding:** the triage-page decisions recorded in each outcome's ledger entry are executed as
  they stand in `TRIAGE_OUTCOMES.md`; this plan does not re-litigate them. The batching and complexity gate below change
  how they are executed, not what was decided.
- **Batching:** execute the outcomes as one PR per coherent group (outcomes touching the same module or of the same
  kind, such as all comment corrections in one crate), at most about 300 lines of diff per PR, roughly 3 to 10 PRs per
  plan. This deliberately overrides the one-PR-per-outcome rule in `review_feedback_queue/.agents/execute-outcomes.md`
  for these plans. Each PR carries the queue removals and ledger graduations for exactly the outcomes it executes and
  lists them in its description.
- **Complexity gate:** an outcome whose fix turns out not to be straightforward, or to exceed its recorded size gate
  (about 100 lines of diff, not counting the ledger, the queue removal or a changelog fragment), becomes a Near term
  TODO.md entry describing the work, with enough of the finding's detail (what is wrong, where, the rule it breaks, the
  decided fix) that a later agent needs nothing else. The PR that would have carried the fix adds the entry, removes the
  feedback file and its index line, and graduates the ledger entry with an Execution field saying it was deferred to
  that entry; the report says so too. The plan keeps going; this is not a block. It replaces the ledger criteria's "over
  that, execution blocks and brings the item back to the maintainer" and "stop and bring it back to the maintainer"
  clauses. An outcome whose criteria set no size gate ("Medium; accepted") is held to the not-straightforward test only.
  Adding these entries is the maintainer's instruction, not an entry on your own initiative.
- **Review gate:** one fresh-context `gpt-6.2-sol` agent at `high` effort per PR, with the general review charter quoted
  under Review gate below. Its findings are inputs, not orders.
- **Release note:** this plan's `## Release note` section is exactly `No release note needed.` PRs still add changelog
  fragments when their type requires one (root `AGENTS.md`, "Changelog fragments").
- **Unattended fallback:** decided outcomes that are no longer relevant are discarded rather than brought back to the
  maintainer (maintainer, 2026-11-21); the wording is under Unattended fallback below.
- **No-workhorse mode:** the plans default (`plans/.agents/planning.md`), stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the recorded triage outcomes (each ledger entry's Decision and Completion
criteria), the maintainer's note on execution quoted above, and the decisions above.

**Binding repository constraints:**

- `review_feedback_queue/AGENTS.md` and `review_feedback_queue/.agents/execute-outcomes.md` (read both before changing
  anything under `review_feedback_queue/` or the ledgers); the batching decision above is the only departure from the
  latter, apart from the unattended fallback.
- The SPEC.md and SPEC_impl.md sections each outcome's Assessment cites.
- `.agents/test-authoring.md` for every test added or changed.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md" (for
  TODO entries), and its "Instructions by path" for every path you touch.
- `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module docs before `bash-stop-loses-unsaved-history.md`: its
  criteria say to decide the hangup per launch kind in the harness layer, never by comparing kinds at the call site
  (root `AGENTS.md`, "Code rules").

**Planner proposals:**

- The grouping of outcomes into PRs under Outline. You may regroup when a group turns out incoherent (a PR that would
  exceed about 300 lines, or outcomes a reviewer cannot judge together), logging why as a DECISION; every outcome stays
  in exactly one PR of this plan.
- Keep each fix to what its outcome needs; no refactors beyond it.

## Outline

1. `plan/triage-supervisor/01-shell-hangup`: end Bash and Zsh main shells with a hangup so they save history. Executes:

   - `bash-stop-loses-unsaved-history.md` (fix as the criteria say): Stop forces Bash down before it can save shell
     history

2. `plan/triage-supervisor/02-lifecycle-and-cleanup`: a gone terminal shown as missing, failed-relaunch cleanup,
   Delete's recorded tab scopes and gone hosts. Executes:

   - `gone-terminal-retained-handle-hides-recovery.md` (fix, approved as straightforward): A terminal Farhelm knows is
     gone can stay stuck reconnecting
   - `trailing-failed-host-relaunch-warn-scope-error-definitive-rollback.md` (fix, approved as straightforward):
     failed-relaunch warning policy and restoration route predate the full range
   - `trailing-delete-recorded-tab-scope-kept-derived-under-unusable-manager.md` (fix, approved as straightforward):
     Scoped-tab Delete policy gap predates the whole review range
   - `trailing-delete-held-gone-host-skips-adopted-process-confirmation.md` (fix, approved as straightforward): Known
     Delete confirmation bypass is separate from guarded tab-close identity

3. `plan/triage-supervisor/03-service-comment-corrections`: comment and doc corrections in the supervisor's service
   code. Executes:

   - `host-lock-table-contradicts-startup-span.md` (correct the text): The host-adoption lock table falsely forbids
     holding the lock across a host connection
   - `trailing-launch-comment-terminal-host-inherits-tab-marker.md` (correct the text): marker-inheritance documentation
     wrongly puts tab markers on terminal hosts
   - `trailing-tab-settle-gate-bare-link-out-of-scope.md` (correct the text): TabSettleGate retains an unresolved
     documentation link through the pinned tip
   - `trailing-restart-definitive-comment-promises-no-external-effects.md` (correct the text): Definitive-Restart
     documentation overpromises absence of external effects
   - `trailing-restart-reply-stale-tab-rediscovery-comment.md` (correct the text): Restart reply comment promises a
     removed tab rediscovery path
   - `trailing-held-start-documentation-retains-the-retired-uncancelled-tmux-child-lifetime.md` (correct the text):
     held-start documentation retains the retired uncancelled-tmux child lifetime
   - `trailing-host-scan-lock-map-forbids-required-host-io.md` (correct the text): Host-scan lock map forbids the
     terminal calls its fence must span
   - `trailing-sentinel-reload-comment-denies-recorded-host-delete-fallback.md` (correct the text): Reload comment
     incorrectly makes the runtime handle Delete's only host ownership record
   - `trailing-supervisor-clone-doorway-testdoc-universal-host-requirement.md` (correct the text): Creating-verb test
     documentation still requires a host for every clone
   - `trailing-environment-delete-docs-overstate-admission-timeout-as-decision-bound.md` (correct the text): Delete
     documentation overstates the admission timeout's decision guarantee
   - `trailing-stop-comment-falsely-bans-retaining-original-final-screen.md` (correct the text): Stop comment
     incorrectly says full-screen final frames are never retained
   - `trailing-connection-writer-comment-points-to-deleted-outputstream-checkpoint.md` (correct the text): Connection
     writer diagnostics still point to the deleted stream checkpoint
   - `trailing-sampler-docs-promise-deleted-capture-failure-reset.md` (correct the text): Sampler field documentation
     still promises the deleted capture-failure reset
   - `trailing-interrupted-doc-defines-only-reboot-attribution-after-retirement.md` (correct the text): Interruption
     documentation still claims boot-id change is its only detector
   - `trailing-outcome-backfill-comment-falsely-promises-live-probe-after-legacy-retirement.md` (correct the text):
     Outcome-backfill comment promises live reconciliation that legacy rows no longer receive

4. `plan/triage-supervisor/04-test-doc-corrections`: comment corrections in the e2e suite, its fixtures and test docs.
   Executes:

   - `systemd-isolation-comments-point-to-active-ledger.md` (correct the text): Systemd isolation comments point to the
     wrong flake ledger
   - `trailing-terminal-tabs-shell-harness-docs-retain-deleted-conformance-caller.md` (correct the text): shell harness
     docs still describe the deleted conformance battery
   - `trailing-terminal-tabs-shared-gate-docs-retain-removed-local-hook-installations.md` (correct the text): shared
     gate docs claim hook installations that the module no longer has
   - `host-flood-stall-window-claim.md` (correct the text): The paused-terminal test overstates its stall-timeout
     coverage
   - `trailing-terminal-less-host-orphan-doc-asserts-no-survivors.md` (correct the text): Terminal-less test
     documentation treats host loss as proof no program survives
   - `trailing-stopped-altscreen-test-comment-claims-inactive-buffer-erasure.md` (correct the text): Correct the stopped
     alternate-screen test's emulator-retention explanation
   - `trailing-70bc66a-host-size-wait-comment-denies-public-status-size.md` (correct the text): Resize wait comment
     gives a false synchronization rationale
   - `trailing-flood-fixture-false-child-exit-terminal-lifetime-rationale.md` (correct the text): Flood fixture docs
     incorrectly say program exit ends its terminal

5. `plan/triage-supervisor/05-restart-and-create-test-gaps`: Restart, relaunch, Stop and create test gaps (fix only the
   cheap ones that guard product behavior). Executes:

   - `restart-test-counts-zombie-as-running-agent.md` (test gap: fix if cheap): Restart ordering test treats terminated
     zombies as running agents
   - `last-output-fixture-readiness-precedes-trap.md` (test gap: fix if cheap): Last-output regression restarts before
     its exit-output trap is established
   - `restart-screen-captured-before-prompt-settles.md` (test gap: fix if cheap): Orderly-restart screen comparison
     captures before the fixture's final prompt
   - `restart-test-types-before-safe-prompt-barrier.md` (test gap: fix if cheap): Restart fixture types before its
     safe-to-type barrier
   - `trailing-failed-restart-loses-the-live-tab-preservation-regression-oracle.md` (test gap: fix if cheap): failed
     Restart loses the live-tab preservation regression oracle
   - `child-hangup-masks-missing-stop-tree-sweep.md` (test gap: fix if cheap): Stop's descendant test does not
     distinguish a tree sweep from session-leader hangup
   - `host-preservation-test-can-select-starter-pid.md` (test gap: fix if cheap): Reserved-host preservation test can
     mistake its temporary starter for the host
   - `stop-annotation-exit-witness.md` (test gap: fix if cheap): Stop annotation test still assumes synchronous host
     exit observation
   - `trailing-removed-completed-keyed-create-reboot-replay-oracle.md` (test gap: fix if cheap): Deleting the kind test
     also removes the completed-create reboot retry oracle
   - `trailing-deleted-ambiguous-create-test-loses-published-handle-and-pending-intent-oracles.md` (test gap: fix if
     cheap): deleted ambiguous-create test loses published-handle and pending-intent oracles

6. `plan/triage-supervisor/06-tab-takeover-and-launch-test-gaps`: tab, takeover, lifecycle-edge, launch and harness test
   gaps (fix only the cheap ones). Executes:

   - `tab-retry-test-measures-observation-time.md` (test gap: fix if cheap): Tab-end backoff regression measures
     observation time rather than retry time
   - `kicked-input-test-local-filter.md` (test gap: fix if cheap): Kicked-input assertions can omit the attack
   - `final-stack-drop-claim-window.md` (test gap: fix if cheap): Host restart-gap fixture can expose an inherited
     claim-drop race
   - `trailing-deleted-integrated-unmarked-tab-shell-delete-oracle.md` (test gap: fix if cheap): Unmarked tab-shell
     cleanup loses its integrated regression oracle
   - `trailing-647f022-pre-reap-tab-live-only-attach-oracle-not-ported.md` (test gap: fix if cheap): Dead-but-unreaped
     tab refusal gap remains applicable to host tabs at tip
   - `trailing-lone-surviving-old-tab-loses-its-guard-and-stop-regression-oracles.md` (test gap: fix if cheap): lone
     surviving old tab loses its guard and Stop regression oracles
   - `shell-session-e2e.md` (test gap: fix if cheap): No end-to-end test starts a first-class Bash or Zsh session
   - `grid-text-omits-blank-columns.md` (test gap: fix if cheap): Grid text helper loses spaces omitted from wire runs
   - `antigravity-first-refused-pin-test.md` (test gap: fix if cheap): Antigravity tests miss pin changes on the first
     unverified report

7. `plan/triage-supervisor/07-unit-test-gaps`: supervisor unit test gaps (fix only the cheap ones that guard product
   behavior). Executes:

   - `stop-wait-test-releases-exit-before-wait-witness.md` (test gap: fix if cheap): Stop regression never establishes
     that Stop is waiting for the exit record
   - `foreign-host-refusal-test-cannot-observe-shutdown.md` (test gap: fix if cheap): Foreign-host refusal test cannot
     observe an unintended Shutdown
   - `cancellation-test-releases-encoder-before-abort-witness.md` (test gap: fix if cheap): Running-runtime cancellation
     regression does not establish cancellation before releasing its encoder
   - `seed-cancellation-test-accepts-lost-emulator.md` (test gap: fix if cheap): Ended-runtime seed cancellation test
     accepts a lost emulator
   - `trailing-checkout-status-borrower-reload-keeps-origin-runtime.md` (test gap: fix if cheap): borrower reload
     fixture retains a competing origin runtime
   - `trailing-failed-spec-test-accepts-an-attempted-host-start-as-never-started.md` (test gap: fix if cheap):
     failed-spec test accepts an attempted host start as never started
   - `trailing-deleted-startup-error-artifact-readonly-oracle.md` (test gap: fix if cheap): Deleted reload test loses
     read-only Error-artifact ownership oracle
   - `trailing-request-conformance-target-testhost-owner-dropped-at-preparation-return.md` (test gap: fix if cheap):
     Keep the target host alive through conformance observation
   - `trailing-migrated-seal-constraint-test-accepts-duplicate-metadata-key.md` (test gap: fix if cheap): Migrated seal
     CHECK assertion accepts a duplicate-key failure
   - `trailing-omp-capture-survives-owner-exit-test-loses-exit-barrier.md` (test gap: fix if cheap): Host-backed
     teardown removes the owner-exit test's exit barrier
   - `trailing-delete-directory-admission-test-observes-only-earlier-agent-fence.md` (test gap: fix if cheap):
     Directory-admission test observes the earlier fence, not the blocked directory wait
   - `trailing-test-one-shot-sample-before-host-catch-up.md` (test gap: fix if cheap): Persistence test can skip an
     uncaught-up host
   - `trailing-busy-host-list-limit-negative-oracle-never-reads-client-output.md` (test gap: fix if cheap): List-limit
     test checks an undrained reply cache

8. `plan/triage-supervisor/08-test-harness-items`: test harness items (fix only what corrupts evidence or crashes).
   Executes:

   - `blocking-test-cleanup-prevents-async-listener-release.md` (tooling: fix only if it corrupts evidence or crashes):
     Blocking directory cleanup waits on an aborted stand-in task that cannot run
   - `reboot-concurrency-early-release.md` (tooling: fix only if it corrupts evidence or crashes): Reboot tests release
     their concurrency slot before successor retirement
   - `trailing-old-server-guard-loses-directory-in-consumed-restart.md` (tooling: fix only if it corrupts evidence or
     crashes): Restart failure deletes the tmux socket before the explicit server guard runs

## Fallbacks

- An outcome whose fix needs a design choice that the spec and its ledger entry do not settle, or much more than its
  size gate, takes the complexity gate under Decisions rather than blocking.
- An outcome whose finding is already fixed on main, or whose code no longer exists, follows the unattended fallback
  below.
- Decisions to log: every regrouping, every overlap with another plan's area, every fix-or-no-change judgment and its
  reason, every use of the complexity gate, every outcome discarded under the unattended fallback, every review finding
  declined with its reason, and any criterion found already met or moot.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-triage-supervisor-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-triage-supervisor-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. A PR that changes only comments, docs or spec prose with no doctest or generation role is
documentation-only: no runtime tests, only the formatters and checks that inspect the changed files.

- The supervisor selections `.agents/validation.md` lists for the code each PR touches (restarting host agents, host
  terminal switch points, terminal tabs in terminal hosts, creating agents in terminal hosts, host terminals across
  supervisor restarts, host terminals under a real supervisor process), through the recorder; read every runtime
  `SKIPPED` as missing coverage.
- `python -B scripts/check-test-sleeps.py` whenever Rust tests change.

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
without costing quality (independent outcomes in one PR investigated in parallel, say). Concurrent agents that might
otherwise conflict can work in separate jj workspaces whose results you merge back; you still own the stack and every
PR. Launch a sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you
need is not available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies
the launch mechanics.

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

- One reviewable commit, stable bookmark and draft PR per outline step, in a single linear stack, using the `jjstack`
  skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/triage-supervisor/01-shell-hangup`, `plan/triage-supervisor/02-lifecycle-and-cleanup`,
  `plan/triage-supervisor/03-service-comment-corrections`, `plan/triage-supervisor/04-test-doc-corrections`,
  `plan/triage-supervisor/05-restart-and-create-test-gaps`,
  `plan/triage-supervisor/06-tab-takeover-and-launch-test-gaps`, `plan/triage-supervisor/07-unit-test-gaps`,
  `plan/triage-supervisor/08-test-harness-items`.
- Keep each PR to its group and about 300 lines of diff (not counting the ledger, the queue removals, changelog
  fragments or TODO entries). Avoid churn: code added in one PR and deleted in a later one means the stack should have
  been shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and
  that applies to PRs an earlier run built too.
- Each PR executes its outcomes per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes",
  except that it carries several outcomes: for each one it removes the feedback file and its
  `review_feedback_queue/INDEX.md` line (or narrows the file, for a partial outcome), and moves the entry from
  `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` with its Execution field replaced by the change ID,
  bookmark and draft PR link, plus which way it went where the criteria leave a choice (fixed; no change, and why;
  discarded under the unattended fallback, and why; or deferred to a named TODO.md entry). Record the change ID and
  bookmark before creating the PR, then add the link and update the PR. Each entry's Execution reads
  ``planned in `plans/queue/triage-supervisor.md` `` until then. The PR description lists the outcomes it executes.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): a PR of comment and doc corrections is `docs`, test-only changes are `test`, queue bookkeeping is
  `chore`, and a PR that fixes something users see is `fix`. Add a changelog fragment in the same commit when the type
  calls for one (root `AGENTS.md`, "Changelog fragments"): one fragment may cover a PR's user-visible fixes when they
  read as one entry, otherwise one per fix, and a `kind: none` fragment with its reason when nothing user-facing
  changed. This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. Every other plan of the ten edits them too,
  so expect such conflicts. A conflict anywhere else (SPEC.md included) is a careful rebase
  (`.agents/careful-rebase.md`).
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
findings file in the scratch directory, and the acceptance criteria: this file's goal, acceptance criteria and
decisions, and for each outcome the PR executes, its full ledger entry and its feedback file. When a PR changes tests or
their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md`
requires.

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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file. If current code already does what an outcome asks as a
whole, or the code it targets no longer exists, execute that outcome as a discard (queue removal and ledger bookkeeping,
with the reason in its Execution field), log it as a DECISION, and say so in the report. This departs from
`review_feedback_queue/.agents/execute-outcomes.md`, which returns an invalidated decision to the user: the maintainer
asked on 2026-11-21 that decided outcomes no longer clearly relevant be discarded rather than brought back. An
acceptance criterion that is already true by design (one that only says what must be kept) is not a reason to discard.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished; an outcome
that would need such a decision takes the complexity gate instead whenever that keeps the rest moving.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outline step in the order the outline gives (or
per a regrouping you logged), together executing every outcome listed under Outline exactly once and meeting the
acceptance criteria above, each having passed its review gate and each carrying its outcomes' queue removals and ledger
moves. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. The report maps every outcome to its PR and says how
each ended (fixed, no change, discarded, or deferred to a TODO entry), and names every overlap with another plan's area.
Never edit `plans/` yourself.

## Release note

No release note needed.
