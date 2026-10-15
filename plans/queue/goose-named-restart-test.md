# Test that a named Goose restart stops offering the old conversation

Written against main at 507dbcbb on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

"Restart with" can start a session in a conversation the user names (#2012). For Goose, Farhelm does not learn which
conversation a named resume ends up in: Goose's reporter is attached only to fresh starts. So a named Restart with on a
Goose session makes the supervisor forget the conversation it had captured; otherwise a later plain Restart would resume
the old conversation instead of the one the user named. If the restart fails definitively before Goose starts (nothing
outside the supervisor changed), the old capture is put back.

Today only the stored rows are tested (`named_relaunch_clears_capture_and_definitive_abort_restores_it` in
`crates/farhelm-supervisor/src/store.rs`). Nothing checks the supervisor's in-memory state, which is what the session
list publishes and what a plain Restart offers. Add a supervisor test that covers it.

### Acceptance criteria

- A supervisor test (or two, if that reads better) that, on a Goose agent launch with a captured conversation:
  1. performs a successful named Restart with, then asserts that the published session list offers no captured
     conversation (the row's `restart_offer` is `NotCaptured`, not `Resume`), that the in-memory capture is gone, and
     that the stored row's capture columns are cleared;
  2. performs a named Restart with that fails definitively before Goose starts, then asserts that the old conversation
     is back in memory (the listing offers `Resume` again and the in-memory capture names it) and in the database, and
     that the restart really got as far as clearing it: the durable generation advanced, so this is not a refusal that
     never touched the capture. That, with the restored capture, is the proof the failure was the definitive kind (only
     that kind restores; an ambiguous failure leaves the capture cleared), so no hook exposing the disposition is
     needed. If a further cheap check is wanted, assert that the row's outcome is not `Launching`.

  The order is free: running the failing restart first, against the original capture, and then the successful one needs
  only one recorded conversation.
- The test uses an agent launch (`SessionLaunch::Agent` with `LaunchHarness::Goose`), because a named start is refused
  for command launches and the capture is cleared only for harnesses that do not capture named conversations. This
  fixture is new: the existing restart tests drive `sh -c` only through command launches, and `named_test_launch()` is
  for admission tests that never spawn. A valid shape is start `["sh","-c",<script>,"{farhelm_args}"]` and resume
  `["sh","-c",<script>,"farhelm-test-resume","{conversation}","{farhelm_args}"]` (`{farhelm_args}` exactly once in each,
  `{conversation}` exactly once in resume); Goose's start-time arguments land as unread positional arguments of `sh -c`.
- The restart is driven through `restart_session_named`, the code under test, never by calling `relaunch` directly:
  `restart_session_named(id, true, Some(<the same Goose launch>), None, Some(NamedConversationStart { conversation, argv: IntegrationSnapshot::of(&launch).filled_named_resume_argv(conversation)? }))`,
  with `stop_if_running` true so the live `sh` agent does not trigger the consent refusal. Check the exact signature in
  the code.
- The test is deterministic, uses its own private state directory and tmux server like its neighbors, does not depend on
  the host's systemd (scopeless seams), and has no unannotated sleeps (`python -B scripts/check-test-sleeps.py` passes;
  see root `AGENTS.md`).
- A docstring per root and user rules: why the test matters (the in-memory side is what a plain Restart and the session
  list act on, and the store test cannot see it) and what it checks, spec-style.
- No product code changes, apart from a `cfg(test)` helper if one is genuinely needed. If the test exposes a product
  bug, block per `plans/AGENTS.md`, unless the fix is small and obviously within SPEC.md: then fix it in its own `fix:`
  PR before the test, with a changelog fragment, and log the DECISION.
- The TODO.md entry ("Test that a named Goose restart stops offering the old conversation.") is removed in the last PR.

Out of scope: tests for other harnesses, end-to-end tests through the helm, and any change to how capture works.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Test that a named Goose restart stops offering the old conversation.** Restart with a named conversation on a Goose
> session makes the supervisor forget the conversation it had captured (Goose does not report after a named resume), and
> a definitive failure before Goose starts puts the old one back (#2012). Only the stored rows are tested
> (`named_relaunch_clears_capture_and_definitive_abort_restores_it` in the supervisor's store); nothing checks the
> in-memory state the session list publishes. Add a supervisor test that performs a named Restart with of a Goose
> launch, asserts the session lists no captured conversation afterwards (so plain Restart cannot resume the old one),
> and after an injected definitive pre-start failure asserts the old conversation is back in both memory and the
> database.

**The user's decisions (2026-10-14):** review gate is a single fresh-context gpt-6.1-sol reviewer at high effort, no
swarm; no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md and SPEC_impl.md on named conversations and Restart; root `AGENTS.md` on
tests (no environment mutation, recorder runs, the sleep checker) and `.agents/test-authoring.md`;
`crates/farhelm-supervisor/src/agent_kind/mod.rs`'s per-harness map (no harness-specific branches in shared code: the
test may name Goose, product code may not start comparing harnesses).

**Planner proposals:** the in-crate placement, the fault seam, and asserting the generation advanced.

## Outline

As found at planning time (find code by name; line numbers drift):

- The path: `Supervisor::restart_session_named` in `crates/farhelm-supervisor/src/service/core.rs` decides
  `clear_capture` from `captures_named_conversation()` (false for Goose). `Supervisor::relaunch` calls
  `store.begin_relaunch_named(.., clear_capture)` and replaces the entry's capture cell with an unclaimed one. On a
  `RelaunchDisposition::Definitive` failure it calls `store.abort_relaunch` and, if restored, puts
  `CaptureState::Reported` back from `claim.prior.cleared_capture`.
- Where the test goes: the in-crate tests module of `service/core.rs`, which can read memory directly (`sup.sessions`,
  `entry.run.capture`) and through the listing (`crate::service::listing::list_all`, as
  `a_listing_during_a_restart_keeps_the_session_as_last_listed` does), and the store through `session_snapshot`.
- Fixtures to reuse: `StateDir`, `Supervisor::new_with_seams` with `scopeless_seams()`, `create_session` with an agent
  launch, `record_conversation_for_test`, `IntegrationSnapshot::of(&launch).filled_named_resume_argv(..)` to build the
  named argv (`named_test_launch()` is the Claude analogue). `create_resumable_session` makes a command launch, which
  cannot take a named start.
- Definitive pre-start failure, default: before the failing restart, create a file at the next generation's launch spec
  path (`crate::launch::spec_path_for_launch(<state dir>, &id, <current generation> + 1)`; the claim increments the
  generation by one). The spec write never overwrites an existing file (`write_private_file`; see
  `write_private_file_is_0600_and_never_overwrites`), so the spawn fails with `SpawnFailure::Spec`, which is definitive,
  right after the generation is claimed and the capture cleared, before tmux does anything, on the ordinary reused-pane
  restart. It needs no tab, pane kill, fault hook or toggle. Remove the planted file before any later restart that must
  succeed.
- Alternatives only if that unexpectedly fails: `faults.replacement_fault` at `ReplacementStage::BeforeCreation` (fires
  only when the agent pane is gone and a tab keeps the tmux session; recipe
  `replacement_failures_preserve_tabs_and_clean_temporary_windows`; installed at construction, so it fires on every such
  restart). Do not copy `aborted_restart_republishes_prior_launch_boot_finality`'s recipe: its empty launch directory
  and its direct `sup.relaunch(..)` call with `clear_capture: false` are specific to a seeded row that never launched,
  and the direct call bypasses the code under test.

Expected shape: one `test:` PR with the test and the TODO.md removal.

### Validation

Run the new test (and its module's neighbors if you touched shared fixtures) through the recorder with the pinned
nextest and tmux (`.agents/narrow-tests.md` has the single-test recipe for `farhelm-supervisor --lib`), repeated enough
times to trust it is not flaky (for example `--kind repetition` a handful of times). `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and the sleep checker.

## Fallbacks

Agreed fallbacks: if none of the routes above reaches a definitive failure after the capture was cleared (not expected:
planning-time review traced all of them), add the smallest `cfg(test)` fault hook at that point, following the existing
`FaultHooks` pattern, and log it.

Examples of departures that need the scope reassessment: a new end-to-end harness, product changes beyond a test hook, a
fake Goose binary.

Decisions to log: the fault seam used and why, one test or two, how memory is read.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-goose-named-restart-test-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/goose-named-restart-test/<nn>-<short-name>` (for example
  `plan/goose-named-restart-test/01-goose-named-restart`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `test`. No changelog fragment for the test; a `fix:` PR,
  if one turns out to be needed, gets one.
- The last PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
