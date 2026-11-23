# Quarantine and rebuild wedged repository caches instead of clearing Git locks

Written against main at cb778ae6 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Harden the repository cache's Git lock repair.** Since the repo-cache-stale-git-locks plan (#3296), checkout
> preparation and background cache cleanup delete Git lock files left in a host's repository cache by an interrupted
> job, while holding Farhelm's own exclusive cache lock (`clear_git_locks` in
> `crates/farhelm-supervisor/src/repo_cache.rs`, called from `launch.rs`; SPEC_impl.md says cache Git processes "end
> with their preparation job"). The landing review left three gaps, bundled here for one decision on which to fix:
>
> - **A Git that outlives Farhelm can race a later launch.** Git's own lock file used to be the backstop: a Git process
>   still running after Farhelm's lock was released made the next fetch fail cleanly. Now Farhelm deletes that file. If
>   Farhelm's processes are killed by hand with SIGKILL during background cache cleanup, its `git` keeps running with
>   nothing to stop it, and a later launch can then race it on the cache's branch list. Nothing in Farhelm does this
>   itself. Recommended fix: on Linux, have the kernel end cache Git processes when their parent dies (a parent-death
>   signal on the spawned `git`), which also makes SPEC_impl.md's "end with their preparation job" true. Even an
>   ordinary termination signal lets Git run its own cleanup just after Farhelm's lock is released; the worst case found
>   is that launch's checkout failing once and a retry succeeding. Without the fix, soften that SPEC_impl.md wording.
> - **Reftable storage and per-branch history logs are not covered.** The repair removes the lock files of Git's default
>   branch storage (`packed-refs.lock`, `HEAD.lock`, `config.lock`, `shallow.lock` and per-reference locks under
>   `refs/`). A cache using Git's newer reftable storage, or with per-branch history logs (`logs/`) turned on, can leave
>   locks it does not remove, and checkouts then fail as before the plan. Farhelm turns on neither, but cache Git runs
>   with the user's own Git settings, so a global `init.defaultRefFormat` or `core.logAllRefUpdates` setting can. Either
>   remove those locks too or run cache Git with the settings pinned.
> - **A cache path that is a plain file gives a misleading error.** When the cache path is a file instead of a
>   directory, a checkout fails with "could not clear cache Git locks: Not a directory" rather than Git's own error. No
>   working cache is affected, but the message points at the lock repair, not the broken cache. The test that pins the
>   new message would change with it.
>
> The plan's test covers one planted `packed-refs.lock` on a launch's checkout; the walk through `refs/`, the refusal to
> follow a symlinked `refs/`, and the repair on the background cleanup path are untested and worth covering with
> whichever fix lands. Recorded by the maintainer on 2026-11-18 from the plan's review.
>
> Sent back unplanned on 2026-11-21: the maintainer does not want these gaps patched one by one. Their words: "We need
> to fundamentally rethink this. This cannot be this complicated. We cannot rely on these implementation details and
> whatever in Git; we need to fundamentally redesign this." Planning research that day also found that the parent-death
> signal recommended above would not cover background cleanup, where `git gc` hands the branch-list rewrite to a child
> process the signal does not reach, and that it is Linux-only while the cache runs on Macs too.

In product terms: stop touching Git's internal lock files inside a host's repository cache. Because every managed
checkout clones with `--reference <cache> --dissociate` (copying objects and removing `alternates` before releasing the
cache lock, with no hard links or lingering dependency from existing checkouts or ready spares), the bare repository
cache is purely disposable. Replace `clear_git_locks` with atomic quarantine-and-rebuild on `init`/`fetch` failure in
`clone_from_repo_cache`, restoring the original warm cache if the fresh retry also fails so network or credential
outages never discard a warm cache.

### Where it stands (from planning research at main cb778ae6; line numbers approximate)

- `crates/farhelm-supervisor/src/repo_cache.rs`:
  - `clear_git_locks` and `clear_ref_locks` (lines 33–71) manually delete `packed-refs.lock`, `HEAD.lock`,
    `config.lock`, `shallow.lock`, and `refs/**/*.lock`.
  - `sweep` (lines 92–152) enumerates `<name>.lock` files, acquires non-blocking flocks (`try_acquire()`), removes
    `<name>.git.trash`, and evicts caches unused for 30 days by renaming `<name>.git` to `<name>.git.trash` and calling
    `remove_if_present`.
  - `remove_if_present` (lines 156–161) currently calls `std::fs::remove_dir_all`, which fails with `ENOTDIR` on Unix
    when the quarantined entry is a plain file or symlink.
- `crates/farhelm-supervisor/src/launch.rs`:
  - `clone_from_repo_cache` (lines 2122–2180) acquires `repo_cache::acquire(cache)`, calls `verify_target_after_lock`,
    calls `repo_cache::clear_git_locks`, runs `git init --bare -q`, `git fetch --prune`, and
    `git clone --reference <cache> --dissociate`, then calls `lock.mark_used()`.
  - `consolidate_repo_cache` (lines 2186–2204) acquires `repo_cache::acquire(cache)`, calls
    `repo_cache::clear_git_locks`, and runs `git gc --auto`.
  - Existing tests in `launch.rs` (around lines 6900–7075) include tests for planted `packed-refs.lock` and a plain file
    occupying `repo_cache_path`.
- `SPEC.md` (lines 1871–1877): ordinary checkouts always use the cache, with no fallback to an uncached clone; a cache
  failure fails the clone stage naming the cache path. Rebuilding a fresh cache and cloning through it satisfies this
  contract.
- `SPEC_impl.md` (lines 3616–3626): currently documents `clear_git_locks` and asserts Git children "end with their
  preparation job", plus startup trash cleanup. Update this section to match the quarantine-and-rebuild design.

### Acceptance criteria

1. `clear_git_locks` and `clear_ref_locks` are deleted completely from `crates/farhelm-supervisor/src/repo_cache.rs`,
   and both calls are removed from `crates/farhelm-supervisor/src/launch.rs`. Farhelm contains no code inspecting or
   deleting Git's internal `.lock` files.
2. Background worker cancellation signaling (`bounded_command.rs`, `spare_build.rs`, `spare_builder.rs`) is NOT changed
   (no `SIGTERM` grace added; keep immediate `SIGKILL`).
3. `consolidate_repo_cache` simply drops `clear_git_locks` and does NOT quarantine or delete the cache on
   `git gc --auto` failure (preserving warm caches against transient `ENOSPC`/environment/deadline failures;
   `clone_from_repo_cache` already self-heals any cache that actually blocks checkouts).
4. In `clone_from_repo_cache`, under the held per-repository `flock` (`<name>.lock`):
   - Check whether `cache` already existed (`cache.symlink_metadata().is_ok()`) **before** running the initial
     `git init --bare` + `git fetch --prune` (using a shared local `init_and_fetch` helper so fresh-host failures never
     trigger a pointless retry or duplicate credential prompt).
   - When `init` or `fetch` fails and `cache` already existed: atomically rename `cache` to a unique sibling quarantine
     path under the same owner directory (`repo_cache::quarantine(cache, &lock)`), isolating any surviving `SIGKILL`'d
     Git child on the old inode so it cannot race the fresh cache, and retry `init_and_fetch` once into a fresh `cache`.
   - If the fresh retry succeeds: remove the quarantined entry best-effort (logging a warning if removal fails; startup
     `sweep` cleans leftovers later) and continue to `git clone --reference <cache> --dissociate`.
   - If the fresh retry also fails (e.g. network outage or expired credentials): best-effort remove any partially
     created fresh `cache`, best-effort rename the quarantined entry back to `cache` so the warm cache is preserved
     across external outages, and return the failure naming the cache path as `SPEC.md` requires.
5. Quarantine trash naming and `repo_cache::sweep`:
   - Use a separator forbidden in GitHub repository names — `<name>.git:trash-<uuid>` (same `:` convention as
     `spare_checkouts.rs`) with strict canonical UUID suffix validation — so a sibling GitHub repository whose name
     contains dots/hyphens (such as `foo.git.trash-bar`) can never be matched or deleted by `sweep`.
   - Unify `sweep`'s 30-day eviction onto the same quarantine helper instead of keeping two live trash-rename formats,
     while keeping `sweep`'s removal of legacy `<name>.git.trash` for pre-upgrade leftovers.
   - Extend `repo_cache::remove_if_present` locally via `symlink_metadata` so it removes plain files and symlinks with
     `remove_file` and real directories with `remove_dir_all` (ignoring `NotFound`), staying separate from
     `checkout_trash` and `installation`.
6. `SPEC_impl.md` (lines 3616–3626) is updated to describe quarantine-and-rebuild, warm-cache restoration on double
   failure, and trash cleanup, removing the `clear_git_locks` description and the inaccurate "end with their preparation
   job" claim.
7. Tests cover:
   - stale Git lock (e.g., `packed-refs.lock` and a lock under `refs/` or `logs/`) recovered cleanly by
     quarantine-and-rebuild in `clone_from_repo_cache`;
   - a plain file at `<name>.git` recovered cleanly by quarantine-and-rebuild (and removed without `ENOTDIR`);
   - external/remote fetch failure (both original and fresh fetch fail) restores the original warm cache intact;
   - fresh host (no pre-existing cache) failing `fetch` does not retry a second time;
   - `repo_cache::sweep` cleans up leftover `<name>.git:trash-<uuid>` (both directories and plain files) and legacy
     `<name>.git.trash` while never touching sibling repositories whose names look similar (e.g. `repo.git.trash-foo`)
     and skipping locked caches.
8. The TODO.md entry ("Harden the repository cache's Git lock repair.") is removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-21 and 2026-11-22):**

- Fundamentally redesign instead of patching `clear_git_locks` edge cases one by one; do not rely on Git's internal lock
  file layout.
- Rely purely on quarantine-and-rebuild — no `SIGTERM` grace window in the background worker (`spare_build.rs`,
  `spare_builder.rs`, `bounded_command.rs`).
- Retry once into a fresh cache on visible or background `init`/`fetch` failure when a cache already existed, restoring
  the quarantined warm cache on double failure so network/auth outages never wipe a warm cache.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only if user-visible).

**Binding repository constraints:**

- `SPEC.md` (lines 1871–1877): ordinary checkouts always use the cache with no fallback to an uncached clone, and a
  cache failure reports the cache path.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals (refined by fresh-context planning review):**

- Do not quarantine or delete the cache in `consolidate_repo_cache` (just drop `clear_git_locks` there):
  `consolidate_repo_cache` has no fetch/restore path, so deleting on `gc --auto` failure would destroy healthy warm
  caches on transient `ENOSPC`/PATH/deadline failures while `clone_from_repo_cache` already self-heals any cache that
  blocks checkouts.
- Use `<name>.git:trash-<uuid>` (with canonical UUID validation) and unify `sweep`'s 30-day eviction onto the same
  quarantine helper while keeping legacy `<name>.git.trash` cleanup.
- Keep `repo_cache::remove_if_present` local and separate from `checkout_trash` and `installation` (distinct trust
  boundaries).
- Deliver as 1 focused PR (`fix: rebuild wedged repository caches instead of clearing Git locks`).

## Outline

1. `plan/repo-cache-lock-redesign/01-quarantine-and-rebuild-repo-cache`:
   - `crates/farhelm-supervisor/src/repo_cache.rs`: delete `clear_git_locks` and `clear_ref_locks`; extend
     `remove_if_present` via `symlink_metadata` for files/symlinks and directories; add `quarantine(cache, _held)` using
     `<name>.git:trash-<uuid>`; unify `sweep` eviction onto `quarantine` and sweep both `<name>.git:trash-<uuid>`
     (strict canonical UUID suffix) and legacy `<name>.git.trash` under the per-repo `try_acquire()` lock.
   - `crates/farhelm-supervisor/src/launch.rs`: remove both `clear_git_locks` calls; extract a local `init_and_fetch`
     helper in `clone_from_repo_cache`; capture `cache_existed` before the first `init`; on failure when
     `cache_existed`, quarantine + retry `init_and_fetch` once, deleting the quarantine best-effort on success or
     removing the fresh attempt and restoring the quarantined cache on double failure. Leave `consolidate_repo_cache`
     without quarantine.
   - `SPEC_impl.md`: update lines 3616–3626 to document quarantine-and-rebuild, warm-cache restoration, and trash sweep.
   - Update and add tests in `repo_cache.rs` and `launch.rs` covering every acceptance criterion above.
   - Add the PR's changelog fragment in `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments", and remove
     the TODO.md entry ("Harden the repository cache's Git lock repair.").
   - `fix: rebuild wedged repository caches instead of clearing Git locks`.

## Fallbacks

- If an existing test in `launch.rs` pins the old `"could not clear cache Git locks: Not a directory"` failure message
  for a plain file at `repo_cache_path`, update it to verify quarantine-and-rebuild recovery (or, for the unrecoverable
  case where the remote also fails, that the error reports the cache path cleanly without referencing lock repair), and
  log the update as a DECISION.
- Note on changelog fragment kind: per root `AGENTS.md`, "Changelog fragments", a `fix:` PR leaves a fragment under
  `releasing/changelog.d/` (`kind: fixed` describing the improved recovery in user-facing terms for the folded full
  list, or `kind: none` if you judge there is no user-visible change relative to the last stable release); check whether
  #3296 already shipped in the previous stable tag before choosing and log as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-repo-cache-lock-redesign-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-repo-cache-lock-redesign-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted supervisor tests covering `repo_cache::`, the repo-cache/consolidation tests in `launch::`,
`spare_build::`, and `service::spare_builder::`, plus `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, `dprint check` on changed
Markdown, `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` as
`docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to every test you add or change. Never take a
state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Harden the repository cache's Git lock
  repair."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
