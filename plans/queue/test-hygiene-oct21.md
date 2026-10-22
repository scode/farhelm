# Eight test fixes from the 2026-10-21 auto-triage

Written against main at d0f14346 on 2026-10-21. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Eight small test-only fixes. Four tests arm cleanup too late, so a failed setup step leaves a session, a child process
or a changed setting behind for later tests on the same stack. Four tests can pass without proving what they claim, or
ignore their own deadline. No product code changes.

Carry out these 8 triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md`, in this order (ledger
order, which `review_feedback_queue/.agents/execute-outcomes.md` asks for; the PRs touch different files, so nothing
requires another), one PR each. Read every entry in full first. All eight are `fix code`, recorded without asking under
the maintainer's instruction of 2026-10-21 (quoted in each entry's Decision), with a gate of about 100 lines; over it,
block and bring it back.

1. `approval-poll-http-await-exceeds-deadline.md`
2. `changed-template-test-allows-missing-retry-key.md`
3. `held-reveal-fixture-pins-before-cleanup.md`
4. `model-effort-test-never-proves-model-accepted.md`
5. `notification-fixtures-register-ownership-too-late.md`
6. `replacement-cleanup-test-never-proves-child-lived.md`
7. `settings-test-restores-before-final-spare-save.md`
8. `spare-owner-tests-register-child-cleanup-after-readiness.md`

## Outline

Eight PRs, in that order, each of type `test` (no changelog fragment). Facts from the triage session's reading of main
at 287bd81c and the planning review (verify them; line numbers drift):

- PR 1: in `crates/farhelm/tests/e2e/agent_listing_real_stack.rs`, `await_cli_approval` (around 205–227) awaits each
  HTTP poll without a bound and checks its 90 second deadline only between polls. Bound each poll with the helper's
  deadline, mirroring the readiness probe `await_local_host` in the same file (around 509–524), which uses
  `timeout_at(deadline, get_json(..))`. Scope is this helper only; a test that holds a response open is not required.
- PR 2: in `e2e/tests/quick-launch.spec.ts` (around 1426) the changed-template test asserts only that the second
  launch's retry key differs from the first, so a missing key passes. Also assert it is a non-empty string, copying the
  assertion in `e2e/tests/custom-permissions.spec.ts` (around 94–95 and 129–131).
- PR 3: in `e2e/tests/clone.spec.ts` (around 686–692) the held-reveal test creates a session, then runs `pinAutoSelect`
  and adds an init script, and only then enters the `try` whose `finally` deletes the session. Start the `try` right
  after creation so the session is owned by cleanup as soon as it exists.
- PR 4: the model-effort test in `e2e/tests/custom-permissions.spec.ts` (around 306–328) checks the model field while it
  is still open, where it shows the typed draft (`crates/farhelm-ui/src/launch_controls.rs` around 169–171), and both
  rounds reuse one model id. Use a distinct model id per round, and prove acceptance before the effort assertions. A
  successful Enter closes the field (custom-model acceptance calls `ModelTargets::complete`,
  `crates/farhelm-ui/src/list/create_form/draft.rs` around 1198), visible as `aria-expanded="false"`, after which the
  field shows the committed model; blur also closes it and discards an unaccepted draft
  (`crates/farhelm-ui/src/list/create_form.rs` around 6171–6178). Either way, assert the closed field shows this round's
  id; your choice of which.
- PR 5: in `e2e/tests/notifications.spec.ts` (around 458–460 and 515–517) two tests create two sessions and push both
  ids into `created` only after the second create. `createSession` (`e2e/tests/helpers/fleet.ts`) throws on failure and
  registers nothing itself. Push each id right after its own create, as the three-session test (around 193–198) already
  does. These are the only two paired sites in the file.
- PR 6: `scripts/test-start-stack-cleanup.sh` (around 378–386) backgrounds a stand-in supervisor
  (`exec -a "farhelm supervisor run --state-dir ..." sleep 300`), writes its pid, and signals the stack without checking
  the stand-in is alive; `await_gone` (around 192–205) passes on absence. After writing the pidfile, poll for a bounded
  time until `ps -p <pid> -o args=` matches the pattern `e2e/start-stack.sh` uses to find a supervisor (around 283–284),
  and fail the phase otherwise. Editing an existing test script changes nothing the `.agents/validation.md` inventory
  describes, so no inventory change; log that as a DECISION, since the root rule reads literally as "any check or test
  script change".
- PR 7: in `e2e/tests/settings.spec.ts` (around 105–108) the prepared-checkout settings test's final `toggle.check()`
  save passes through the intercept untracked, while cleanup waits only for the held request before its restore PUT; the
  cleanup comment (around 122–123) says restoration must wait until "the owned write settles". Record every PUT the page
  sends to `/api/checkout-spare` and await each response (tolerating failure) before the restore, as the checkout-folder
  tests in the same file do (PR #2154 fixed the same class there).
- PR 8: in `crates/farhelm/tests/spare_builder.rs`,
  `spare_builder_owner_sigkill_retires_guard_without_writer_inheritance` (around 603–620) and
  `spare_builder_owner_sigkill_after_rc_retires_cache_fetch` (around 677–702) wait for the `unrelated` pid file and the
  other readiness files together, and arm `Indirect(Some(unrelated))` only afterwards. A readiness timeout panics, the
  `Owned(owner)` drop SIGKILLs the owner, the owner's own cleanup of the `sleep 300` never runs, and the sleep outlives
  the test. In both tests wait for the `unrelated` file alone, arm `Indirect`, then wait for the rest. The other
  `sleep 300` sites in the file are reaped by a tethered group and stay as they are.

### Validation

No PR changes product code, so one `cargo build` and one web build at the stack's base serve every PR that needs them.
PRs that change Rust or browser tests (1, 2, 3, 4, 5, 7, 8) run `python -B scripts/check-test-sleeps.py` with the
isolated interpreter `docs/test-sleep-check.md` prepares; it does not read shell scripts, so PR 6 skips it.

- Browser PRs (2, 3, 4, 5, 7): the changed spec file on Chromium and WebKit through the recorder
  (`.agents/narrow-tests.md`).
- PR 1: the helper's two callers, `browser_link_cli_waits_without_fencing_and_reports_each_answer` and
  `keyed_agent_create_and_clone_need_fresh_approval_after_delete`, through the recorder with the pinned nextest and tmux
  setup.
- PR 8: the two changed tests, the same way.
- PR 6: `bash scripts/test-start-stack-cleanup.sh` through the recorder (it needs the builds above and boots several
  real stacks). Its parent-SIGTERM phase has a known open flake (TODO.md's Deflake section and `FLAKES.md`); the
  evidence for this PR is the two replacement phases passing. `shellcheck` on it already reports three SC2015 notices on
  main; the bar is no new findings.

A temporary local fault injection (not committed) that shows the new assertion catches the failure is welcome for PRs 2,
4 and 6, not required, and pointless for the reorder-only PRs. No full battery.

## Fallbacks

Each outcome's gate is binding: over about 100 lines, block and bring it back. Examples of departures that need the
scope reassessment: changing a shared helper in `e2e/tests/helpers/` that other specs use, changing product code to make
a test observable, or touching sibling sites the entry does not name (log them as follow-ups in the report instead).

Decisions to log: each outcome's measured size, for PR 4 how the test proves the model was accepted, and for PR 6 the
validation inventory call.

## Gates, acceptance and requirement sources

### Size gates

Each outcome's gate, where it has one, is in its `TRIAGE_OUTCOMES.md` entry's Completion criteria and is binding: about
100 lines of diff (code, tests and docs, not counting `TRIAGE_OUTCOMES.md`, the queue removal or the changelog
fragment). The maintainer prefers size gates to time estimates. Each entry also says what happens over the gate, and
that differs by outcome: some say to block and bring it back to the maintainer (then block per
`plans/.agents/executing.md`, "Executing one plan", step 10, after building and pushing the plan's other outcomes, with
a question that describes the larger fix, its measured size, and asks whether to build it or discard the outcome),
others say to discard the outcome instead (record Execution as complete with the gate tripped, the measured size and the
reason, remove the feedback file and index line, and change no code). Follow whichever the entry says. Measure and log
each outcome's size. The entry's described fix is the triaging session's proposal; a different fix within the same gate
that meets the same completion criteria is fine, logged as a DECISION.

### Acceptance criteria

- Each outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry, including any test it names.
- Each outcome's PR removes its feedback file and `review_feedback_queue/INDEX.md` entry and updates its Execution
  field.
- A `fix` PR carries a changelog fragment per `AGENTS.md`, "Changelog fragments" (`kind: fixed` unless the change has
  nothing user-facing, then `kind: none` with the reason); `docs`, `test`, `chore` and bookkeeping PRs carry none.

## Requirement sources

**The user's decisions (2026-10-21):** each outcome's Decision field records why it was decided without asking: the
maintainer asked that queue items be triaged without asking when they clearly fit ("it's DEFINITELY a bug or can be
discarded due to spec (no ambiguity or judgement needed relative to spec/product)" and "fixing it is reasonably
straight-forward - it's not a major refactor, complexity creep or scope creep"), after a read-only verification and a
fresh-context adversarial review. While scheduling, the maintainer agreed to the nine pending outcomes of that triage
being scheduled as two plans (`test-hygiene-oct21.md` for the eight test fixes, `duplicate-uninstall-retry-advice.md`
for the uninstall refusal) and chose the review gate below: a single fresh-context Opus 5.5 agent at high effort with
the general charter, no swarm. No-workhorse mode, which `plans/.agents/planning.md` requires.

**Binding repository constraints:** the rule each entry's Assessment cites;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits", "Code rules" and "Tests and validation", with `.agents/validation.md`;
`.agents/test-authoring.md` for any test change; and anything this file's outline names as binding for the plan.

**Planner proposals:** the PR order and the outline above, including the notes a fresh planning review added. A planner
proposal is not a requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-test-hygiene-oct21-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one reviewable commit, stable
  bookmark and draft PR per outcome (unless this file groups some outcomes into one PR, which the maintainer agreed for
  those cases only), in a single linear stack in the order given under The goal above. Each PR carries its outcome's
  agreed change, the removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its
  `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field before creating
  the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/test-hygiene-oct21/<nn>-<short-name>` (for example `plan/test-hygiene-oct21/01-short-name`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment in the same commit; `test`
  for a change only to tests or their fixtures; `docs` for documentation and specification text only; `chore` for
  maintainer tooling and queue bookkeeping. Only a fragment-requiring type adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TRIAGE_OUTCOMES.md`, `review_feedback_queue/`, `TODO.md` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high
effort, launched as a native sub-agent where your harness serves that model and otherwise through
`$scode-harness-shellout` (that skill owns the launch mechanics). The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry, including any size gate (ask the reviewer to flag anything beyond the gate).

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, whether each gated outcome met its gate (with
the measured diff size) and why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. If current code or specs invalidate a recorded triage decision (for example,
the finding turns out already fixed or wrong), that is a block: `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless this file
groups some into one PR (an outcome discarded under its gate still gets its bookkeeping PR; one whose gate says to bring
it back makes the plan block instead, after its other outcomes are pushed), that together meets the acceptance criteria
above, each PR passed its review gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID,
bookmark and PR. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
