# Tensorlake setup guards and repository-cache sweep isolation

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says, and
where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This merged plan groups three small backend reliability and test `TODO.md` entries (two from "Immediate", one from "Plan
follow-ups"), built as a linear stack of three small PRs. Verbatim entries:

> **Keep cached repository cleanup moving past one repo's error.** At supervisor startup, if cleaning up an old cache
> generation for one repository fails (for example because a leftover git process is still writing in it), the sweep
> stops early and skips every remaining repository until the next restart
> (`crates/farhelm-supervisor/src/repo_cache.rs`). Log and continue across repositories while still stopping cleanup of
> an affected repository on selector or directory-sync failures, and add a test proving later repositories are still
> cleaned while selected caches stay intact.

> **Refuse setup reruns on suspended or deleting Tensorlake sandboxes.** In `provisioning/tensorlake_add.rs`,
> `TensorlakeHost` can offer setup even when it should tell the user to resume the sandbox or finish its deletion.
> Refuse before ensuring a host key or keeping a plan. Check again under the provisioning lock when confirming an older
> offer, before taking a connection hold or publishing replacement progress. Keep the sandbox's lifecycle state and
> earlier failure so the user can run setup again once eligible. Cover both refusal points, retained progress and the
> reason shown in Hosts.

> **Assert that Tensorlake host setup never reads agent settings.** In
> `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs`,
> `codex_subscription_and_none_enter_setup_without_an_endpoint` checks that no Codex settings cleanup was recorded
> during setup, but never asserts against the stand-in backend that setup made no `read-agent-settings` call. Add that
> negative assertion.

### Acceptance criteria

1. **Isolate repository-cache startup sweep errors (`crates/farhelm-supervisor/src/repo_cache.rs`, PR 1):**
   - Isolate errors at three levels inside `repo_cache::sweep`:
     1. **Owner directory level:** an error reading one owner directory or entry logs a warning, records a failure, and
        continues to remaining owner directories (while reading `<state_dir>/repo-cache` root itself still returns
        `Ok(())` on `NotFound` and `Err` on other errors).
     2. **Per-repository safety barriers:** lock open/acquire/mtime failures, unreadable `:current` selector,
        `sync_owner()` failures, and eviction detachment failures (`rename` of legacy `<name>.git` to `<name>.git.trash`
        or `remove_file` of `:current`) MUST stop further deletions for **that** repository (preserving its crash-safety
        invariants and selected cache), log a warning, record a failure, and continue to remaining repositories.
     3. **Per-debris deletion errors:** once `:current` is verified / detached and `sync_owner()` has succeeded, failing
        to delete one dead path in `paths` logs a warning, records a failure, and continues deleting the remaining dead
        paths for that repository and all subsequent repositories. For `<name>.git.trash` after its `sync_owner()`,
        failing to delete `trash` logs a warning, records a failure, and skips renaming legacy `<name>.git` onto an
        unremoved `trash` while still continuing with independent debris and other repositories.
   - Return `Err` at the end of `repo_cache::sweep` if any owner, repository, selector, spare-checkout sweep, or debris
     step failed (after finishing all remaining work), matching `spare_checkouts::sweep_locations`.
   - Extract an internal helper taking two narrow closures (`sync_owner: &mut impl FnMut(&Path) -> io::Result<()>` and
     `remove: &mut impl FnMut(&Path) -> io::Result<()>`, matching `spare_checkouts::sweep_with_namespace_sync`, without
     introducing a general filesystem trait) so tests are deterministic and root-safe. Sort owner and lock paths (or
     verify deterministically) so tests prove a failing first repository does not prevent a later repository from being
     cleaned.
   - Add unit tests in `repo_cache.rs::tests` covering:
     - a lock-open failure (e.g. directory at `<repo>.lock`) skipping that repo while cleaning a later repo,
     - a `sync_owner` failure stopping that repo and retaining its selected/unselected generations while cleaning a
       later repo,
     - a malformed `:current` selector retaining that repo's generations while cleaning a later repo,
     - a debris removal failure leaving only the failing debris while removing sibling unselected debris, keeping the
       selected generation intact, and cleaning a later repo,
     - `sweep` returning `Err` after completing the remaining repos.
   - Remove the covered repo-cache `TODO.md` entry in this PR.
2. **Refuse setup Re-run on suspended or deleting Tensorlake sandboxes
   (`crates/farhelm-helm/src/provisioning/tensorlake_add.rs`, PR 2):**
   - Share one small deleting/sealed refusal helper
     (`self.manager.is_sealed(host) || matches!(self.manager.state(host), Some(HostState::Deleting { .. }))`) and reuse
     `self.suspended_refusal(&row)` at both refusal points:
     - In `probe_tensorlake` (for `ProbeDestination::TensorlakeHost { host }`): check deleting/sealed and
       `self.suspended_refusal(&row)` **before** calling `tensorlake.ensure_host_key()` or storing a plan in
       `memory.tensorlake_plans`.
     - In `start_tensorlake_add` (for `plan.host = Some(host)`): under the host provisioning lock (`rerun_write`), check
       row/sandbox validity, deleting/sealed, and `self.suspended_refusal(&row)` **before** inserting `host` into
       `memory.busy`, overwriting `memory.runs`, or taking a connection hold (`hold_connection`). On refusal, preserve
       both the sandbox's lifecycle state (`Suspended` / `Deleting` / `TensorlakeChanging`) and any previous failed
       setup run in `memory.runs` so Re-run stays available once eligible.
   - Remove `withdraw_claim` once moving those checks ahead of `memory.busy`/`memory.runs` makes it dead code.
   - Update/replace the existing test `a_setup_refused_after_its_claim_withdraws_it_through_the_feed` in
     `tensorlake_add_tests.rs` (which currently expects a claim-and-withdraw feed bump) so it asserts the stronger
     invariant: refusal happens before claiming or touching `memory.runs`, leaving prior failed progress and
     `events().revision()` undisturbed.
   - Add helm tests in `tensorlake_add_tests.rs` covering both probe-time and confirm-time refusals for suspended and
     deleting/sealed sandboxes, verifying no host key is ensured on probe refusal, no plan is retained, prior failed run
     progress is preserved on confirm refusal, and host lifecycle state stays intact.
   - Add or extend a desktop Hosts test in `crates/gui/farhelm-gui/src/hosts/` verifying that a refused Re-run displays
     the refusal reason in Hosts while keeping the prior failed setup state.
   - Remove the covered Tensorlake setup rerun `TODO.md` entry in this PR.
3. **Assert that Tensorlake host setup never reads agent settings
   (`crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs`, PR 3):**
   - In `codex_subscription_and_none_enter_setup_without_an_endpoint`, assert against `f.backend_events()` that no event
     has `event["event"] == "read-agent-settings"` (for both `subscription = true` and `subscription = false`).
   - Do not broaden this PR to rewrite unrelated flake handling unless needed; note any observation in the report.
   - Remove the covered settings-read assertion `TODO.md` entry in this PR.

## Decisions already made

### Maintainer decisions (binding)

- **Grouping:** Group these three small backend reliability and test items into this one plan, one PR per item.
- **`repo_cache::sweep` return value on partial failure:** Return `Err` after finishing all remaining owners/repos
  (logging each failure as it occurs), matching `spare_checkouts::sweep_locations`.
- **Review gate:** Single fresh-context agent with the general charter on `Opus 5.5` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `SPEC_impl.md` ("Owned checkout admission and lifetime"): Never delete cache generations or legacy paths without
  holding the repo lock, reading a valid selector (when active) or detaching first (when expired), and completing
  `sync_owner()`.
- `SPEC_impl.md`: Provisioning checks suspended and deleting states directly before probing and again under the
  provisioning lock before a confirmed run's preflight/progress replacement.
- `AGENTS.md`: Never mutate process environment variables in tests; never touch the live Farhelm install; annotate every
  test sleep with `// sleep-ok:` passing `scripts/check-test-sleeps.py`.

### Planner-chosen implementation shape

- **Two-closure internal `sweep_with_effects` helper in `repo_cache.rs`** (`sync_owner` and `remove`, matching
  `spare_checkouts::sweep_with_namespace_sync`) rather than a general filesystem trait or `chmod` permission hacks that
  fail under root containers.
- **Check validity, deleting/sealed, and `suspended_refusal(&row)` before `memory.busy` / `memory.runs` mutation in
  `start_tensorlake_add`** (matching ordinary SSH/container `start_add` in `provisioning/service.rs`) and remove the
  now-dead `withdraw_claim` helper rather than mutating and rolling back `memory.runs`.

## Outline

### Existing mechanisms reused

- `crates/farhelm-supervisor/src/repo_cache.rs` (`sweep`, `sweep_generations`, `PreparationLock`, `current`,
  `remove_if_present`) and `crates/farhelm-supervisor/src/spare_checkouts.rs` (`sweep_with_namespace_sync` pattern).
- `crates/farhelm-helm/src/provisioning/tensorlake_add.rs` (`probe_tensorlake`, `start_tensorlake_add`),
  `crates/farhelm-helm/src/provisioning/service.rs` (`suspended_refusal`), and
  `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs` (`Fixture::backend_events`).
- `crates/gui/farhelm-gui/src/hosts/` (existing Hosts setup/refusal tests).

### PR stack

1. **`plan/tensorlake-setup-and-repo-cache-fixes/01-isolate-repo-cache-sweep-errors`** — Isolate owner, repo-barrier,
   and debris errors in `repo_cache.rs`, return `Err` after completing remaining repos, add deterministic unit tests
   using a two-closure helper, add changelog fragment, and remove the repo-cache `TODO.md` entry.
2. **`plan/tensorlake-setup-and-repo-cache-fixes/02-refuse-suspended-deleting-tensorlake-rerun`** — Refuse suspended and
   deleting/sealed Tensorlake setup reruns at both probe and confirm before host-key lookup or progress replacement,
   remove `withdraw_claim`, update `a_setup_refused_after_its_claim_withdraws_it_through_the_feed`, add helm and Hosts
   GUI tests, add changelog fragment, and remove the Tensorlake rerun `TODO.md` entry.
3. **`plan/tensorlake-setup-and-repo-cache-fixes/03-assert-tensorlake-setup-no-settings-read`** — Add the
   `read-agent-settings` negative assertion to `codex_subscription_and_none_enter_setup_without_an_endpoint` and remove
   its `TODO.md` entry.

## Fallbacks

- In PR 2, check whether an active unsealed delete (`HostState::Deleting { failure: None }`) needs slightly different
  wording than a sealed/failed delete ("use Retry to finish deleting it") and log as a `DECISION`.
- Scope-reassessment triggers (examples, not a blacklist): changing `repo_cache` on-disk layout or lock protocol,
  changing `SPEC.md` host lifecycle states, or rewriting unrelated Tensorlake setup steps.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tensorlake-setup-and-repo-cache-fixes-log.md` in the parent directory of the checkout you run in, derived
as that section says, and the watchdog's status file is
`farhelm-plan-tensorlake-setup-and-repo-cache-fixes-watchdog.json` beside it. Resolve both to absolute paths before you
start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`, "Agent
scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- Targeted Rust tests (through `record-test-run.py` with pinned nextest on `PATH`):
  - `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^repo_cache::/) | test(/^spare_checkouts::/)'`
  - `cargo nextest run -p farhelm-helm --lib -E 'test(tensorlake_add)'`
  - `cargo nextest run -p farhelm-gui --lib -E 'test(/^hosts::/)'`
- Formatting, lints, and checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
  - `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`
  - `dprint check` on changed Markdown files
  - `python3 releasing/check-changelog.py format`
  - `python3 scripts/check-agents-md.py` (if headings or instruction files change)

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Each PR removes the `TODO.md` entry it covers, so all three covered entries are removed by the top of the stack, as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred items added to `TODO.md` go under
  `## Plan follow-ups`.
- Commit messages, PR titles, and changelog fragments follow root `AGENTS.md`, "Conventional Commits" and "Changelog
  fragments". This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8); never edit that section.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `Opus 5.5` at `high` effort,
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular active vs. sealed deleting wording, and any review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block. Things the maintainer should know and possible follow-ups go into the report as statements.

### Done criterion

The goal is achieved when every PR in the linear stack exists on GitHub as a draft PR and has passed its review gate,
and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is your closing step per
`plans/.agents/executing.md`, "Executing one plan" (step 11), not part of the goal.

## Release note

No release note needed.
