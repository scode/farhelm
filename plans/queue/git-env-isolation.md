# Execute two triage outcomes: keep an inherited GIT_DIR from steering Farhelm's git commands into another repository

Written against main at 814a1129 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out two triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `checkout-cache-refresh-overwrite-another-repositorys-branches.md`
- `git-fixture-setup-modify-delete-callers-repository.md`

Git lets environment variables such as `GIT_DIR` choose which repository a command acts on, and they override the
directory a command names. When a session is created with a fresh checkout, the supervisor runs git to prepare it
(initialize or refresh a cached copy, then clone), and those git processes inherit the user's login environment. If that
environment sets `GIT_DIR`, the cache refresh's forced fetch with prune acts on that other repository and can overwrite
and delete its branches and tags. Separately, the repository-discovery test fixtures run git without scrubbing these
variables, so running the tests with `GIT_DIR` set (git hooks export it) damages the developer's own repository; a
scratch reproduction showed its `.git` directory replaced with a pointer into a temporary directory that is later
deleted. The supervisor's production repository discovery already strips these variables for exactly this reason.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read both entries and both feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- A regression test shows checkout preparation leaves a repository named by an inherited `GIT_DIR` untouched, with the
  variable supplied to a child process, never by changing the test process's own environment.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-10):** a screening of the highest-priority review findings picked out the ones
that are clearly bugs with straightforward fixes, and the user decided: "given the criteria, let's just triage them all
for fixes unless you have a reason to disagree. every one should have a complexity gate so that if anything turns out to
be more complicated than your assessment, it bounces back to me." The criteria were: clearly a bug, no judgment needed;
and fixing it is straightforward, adding no significant complexity or scope. The decisions are recorded in root
`TRIAGE_OUTCOMES.md`, one entry per feedback file, each with its own complexity gate.

**The user's plan-time decisions (2026-10-10):** "group them together into a reasonable number of PRs that are
reasonably related or similar so we don't have 18 distinct plans with tiny amounts of work". Asked to confirm, the user
chose five plans with one PR each, so this plan's outcomes share one PR, overriding root `AGENTS.md`'s
one-PR-per-outcome rule for these outcomes only. When an item trips its complexity gate: drop it from the PR and ship
the rest (see Complexity gate below). Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which
`plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; the rule against tests that modify the test process's
own environment variables), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on main at 814a1129.

- Production reference: `crates/farhelm-supervisor/src/repository_discovery.rs`, the scanner's async `run_git`, removes
  `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR` and every `GIT_CONFIG_*` from its child, and refuses to re-add them from
  its configured child environment.
- Checkout preparation: `crates/farhelm-supervisor/src/launch.rs` builds every preparation child with
  `launch_child_command`, which the user's preparation hook and the agent itself also go through. Scrub only the git
  children (the `init`, `fetch` and `clone` commands in `clone_with_repo_cache`, built from
  `launch_child_command("git", ...)`), not the shared builder: the agent's own environment and the hook's are out of
  scope. There is no uncached checkout path any more, whatever the ledger entry says. Remove the variables that select
  which repository git works on, as one constant list looped over those three commands: what
  `git rev-parse --local-env-vars` prints (git 2.43: `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_OBJECT_DIRECTORY`,
  `GIT_DIR`, `GIT_WORK_TREE`, `GIT_IMPLICIT_WORK_TREE`, `GIT_GRAFT_FILE`, `GIT_INDEX_FILE`, `GIT_NO_REPLACE_OBJECTS`,
  `GIT_REPLACE_REF_BASE`, `GIT_PREFIX`, `GIT_SHALLOW_FILE`, `GIT_COMMON_DIR`) minus the three config variables it also
  prints (`GIT_CONFIG`, `GIT_CONFIG_PARAMETERS`, `GIT_CONFIG_COUNT`). Keep the config variables: the existing launch
  tests pass a `GIT_CONFIG_COUNT`/`KEY_0`/`VALUE_0` `insteadOf` rewrite into the shim's git to clone offline, and
  removing them would break that seam and change behavior users may rely on.
- No shared helper across the three call sites: repository discovery builds a tokio `Command` with a different list, and
  changing production discovery is out of scope. Leave it as it is.
- Regression test: the launch tests run the shim in a child with an explicit environment
  (`run_shim_in_child_with(child_path, envs, removes)`); one existing test removes the runner's own `GIT_DIR` and
  `GIT_WORK_TREE` from that child. Add a test that instead sets `GIT_DIR` in the child's environment to a separate
  scratch repository with a branch and a tag, runs one preparation (every preparation runs the forced fetch), and
  asserts the scratch repository's refs and `.git` are unchanged while the checkout still comes out `Ready`. In that
  helper, added variables are applied before removals, so the new test must not also list `GIT_DIR` among the removals.
- Fixture: `repository_discovery.rs`'s test helper `run_git` (`Command::new("git")` in the test module) removes what
  production discovery removes (`GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR` and every `GIT_CONFIG_*`) on its child
  `Command`; that is the whole fix for the second outcome. Never set or remove variables on the test process itself.
- Changelog: `fix:` with `kind: fixed`, for someone running Farhelm: creating a session with a fresh checkout no longer
  touches another repository when `GIT_DIR` or a similar variable is set in the login environment. The fixture change is
  not user-facing.

The complexity gate for these two is a few lines per site plus the regression test. Changing the shared preparation
builder for the hook or the agent, new plumbing to carry an environment through the supervisor, or a test that needs a
new harness seam is a gate trip.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, clippy on `farhelm-supervisor` with
`--all-targets`, and focused nextest selections for the touched `launch` and `repository_discovery` tests through
`scripts/record-test-run.py` (the launch shim tests may need tmux; see `docs/test-run-evidence.md` for the mode). Run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` and apply `.agents/test-authoring.md`.
`dprint check` on the changed Markdown.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-git-env-isolation-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/git-env-isolation/01-scrub-git-env`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
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
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that the agent's and the preparation hook's
environments are unchanged, and that no test changes its own process environment. Where you disagree with a finding,
decide on the merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill
owns launch mechanics.

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
alternatives considered: in particular the exact variable list and whether `GIT_CONFIG_*` is included, whether a shared
helper was introduced, the Conventional Commit type and changelog kind, every outcome dropped by its complexity gate and
why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

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
