# A managed checkout's clone works around git's commit-graph bug

Written against main at 1e5dd4da on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Work around git's `--reference --dissociate` clone bug on macOS": creating a session in
a managed checkout can fail at the clone stage, leaving the session's terminal with git's "fatal: unable to parse commit
<sha>" and "Clone succeeded, but checkout failed", and an empty working tree with every file staged as deleted. The
maintainer hit it at once on a Mac, creating a managed checkout of this repository with macOS's system git.

The cause, from the maintainer's write-up (https://snippets.scode.org/s/git-clone-dissociate-commit-graph-bug/) and
confirmed in planning: git before 2.55.0 fails `git clone --reference <ref> --dissociate` when the reference repository
holds a commit-graph covering the commit being checked out. During the clone, some commits are loaded through the
reference's commit-graph; `--dissociate` then repacks, drops the alternates link and closes the object store, so
checkout's lazy tree lookup has nothing to read. Fixed upstream by git commit `3d8e4004c6` ("commit: fall back to full
read when maybe_tree is NULL"), first released in 2.55.0; there is no 2.54.x point release. The workaround is to run
that one clone as `git -c core.commitGraph=false clone …`, which only disables reading commit-graphs for that command
and is not persisted into the new checkout's configuration.

Findings from planning that the TODO entry does not state:

- The bug is not macOS-only. Planning reproduced it with git 2.43 on Linux: a bare cache with a commit-graph written by
  `git commit-graph write --reachable`, then `git clone --reference <cache> --dissociate -- file://<upstream> <dest>`,
  exits 128 with every file staged as deleted; the same clone with `-c core.commitGraph=false` succeeds.
- What differs between hosts is whether the cache ends up with a commit-graph. On Linux with git 2.43, Farhelm's own
  cache fetch leaves none, so the maintainer's managed checkouts there have always worked; one appears only when git's
  automatic gc runs during that fetch or the user's configuration writes graphs on fetch. On the Mac, with git 2.54, the
  maintainer hit the failure immediately, which means the cache had a graph covering the checked-out commit. A git
  developer suggested that 2.54's new automatic maintenance strategy writes commit-graphs more often; that is
  unverified.
- A clone from a plain local path (no `file://`) uses git's local transport and does not trigger the bug; a `file://`
  URL does. The existing real-git test of the shim maps the GitHub URL to a plain path, so a regression test built the
  same way would pass on buggy git and prove nothing.
- Farhelm states no minimum git version (SPEC.md, SPEC_impl.md and the docs website are silent), and the supervisor runs
  whatever `git` the host has.

Acceptance criteria:

- The managed-checkout clone command is
  `git -c core.commitGraph=false clone --reference <cache> --dissociate -- <url> <cwd>`, and nothing else about the
  preparation sequence changes: not the cache init, not the cache fetch, not the environment scrubbing,
  `GIT_TERMINAL_PROMPT=1`, locking, or failure states. The flag goes only on that one command; do not set it on the
  cache, in the checkout's configuration, or globally.
- A comment at the call site names the upstream fix (git commit `3d8e4004c6`, released in git 2.55.0) and says the flag
  can be removed once git 2.55 is Farhelm's minimum.
- SPEC_impl.md's listing of the three preparation commands (under "Owned checkout admission and lifetime", the paragraph
  starting "Each host caches repository objects", which shows
  `git clone --reference <cache> --dissociate -- <https url> <cwd>`) shows the flag, with one sentence of why beside the
  existing explanation of `--reference` and `--dissociate`.
- The existing shim tests pass with the new argv: the fake git they install still tells the stages apart, and the test
  that asserts the clone's exact argv expects the new one.
- A regression test, running real git through the shim, in which the cache holds a commit-graph covering the commit
  being checked out and the GitHub URL is mapped to a `file://` URL of the upstream fixture. It asserts the fixture's
  premise (the graph exists in the cache and covers the upstream HEAD) and that preparation reaches Ready with the
  tracked file in the checkout. It asserts that the workaround clone succeeds, not that a plain clone fails, so it keeps
  passing after a git upgrade.
- Once, during the run, confirm that the regression test fails with the flag removed on this host's git (a mutation
  check, not committed), and record the result in the working log and the report. If the host's git is 2.55 or newer,
  the mutation check cannot fail; say so in the report instead.
- A changelog fragment, `kind: fixed`: creating a session in a managed checkout no longer fails at the clone step with
  "unable to parse commit" on hosts with git older than 2.55, including macOS's built-in git.
- The TODO.md entry is removed.
- One draft PR (or a short linear stack if review finds a reason to split), each passing its review gate. No PR is
  marked ready, and nothing is merged.

Out of scope: repairing checkouts that already failed this way (the user deletes the session and creates it again, as
for any failed clone), a minimum-git-version check, warnings about the host's git version, and any other preparation
change. The write-up's untested alternative (dropping `--dissociate` and repacking by hand) is not wanted.

## Requirement sources

**The user's request (2026-10-13):** plan the Near term TODO entry about working around the git bug. Its text:

> A managed checkout is made with `git clone --reference <cache> --dissociate`, and git before 2.55.0 (including macOS's
> system git, 2.54.0 Apple Git-157) can transfer every object and then fail the checkout with "fatal: unable to parse
> commit <sha>" and "Clone succeeded, but checkout failed", leaving an empty worktree with every file staged as deleted.
> It only happens when the cache's commit-graph covers the commit being checked out, so it looks intermittent. The
> workaround is to run that one clone as `git -c core.commitGraph=false clone …`, with a comment at the call site naming
> the upstream fix (git commit `3d8e4004c6`) and saying the flag can go once git 2.55 is the minimum. A regression test
> has to run `git commit-graph write --reachable` in the reference repository first, or it proves nothing, and should
> assert that the workaround clone succeeds rather than that the plain one fails.

**The user's decisions (2026-10-13):**

- The maintainer noted they have never hit the failure on Linux, only on macOS, where they hit it at once cloning this
  repository. Planning's explanation (the cache's commit-graph, above) was given in conversation, and the plan is the
  TODO entry's workaround unchanged.
- The commit message and PR description must carry the nuances above, in the maintainer's words: "make sure the nuances
  here is carried on in the actual commit mesasge/prdescription byt he executor". That means: the bug is in git before
  2.55 on every platform, not only macOS; what triggers it is a commit-graph in the cache covering the checked-out
  commit; macOS's git 2.54 hits it routinely, while Linux hosts with older git rarely do, because Farhelm's cache fetch
  there normally leaves no commit-graph; it is fixed upstream in git 2.55.0 (commit `3d8e4004c6`), and the flag can go
  once that is the minimum; and the regression test clones through `file://` with a commit-graph written into the cache,
  because without both it passes on buggy git.
- Review gate: a single fresh-context Opus 5.5 reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC_impl.md, "Owned checkout admission and lifetime" (the repository cache paragraphs): the preparation commands,
  their environment, locking, and the absence of a timeout or fallback. This plan changes only the clone's argv and its
  explanation.
- Root `AGENTS.md`: Conventional Commits; Finishing work (Rust tests through the recorder,
  `scripts/check-test-sleeps.py` when changing Rust tests); Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents; The live install is off-limits (never touch the maintainer's own Farhelm or its repository
  cache to test this).
- Global instructions: tests never modify the test process's own environment variables. The existing real-git tests
  isolate git through per-command environment (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_COUNT`) passed to
  the child; follow that.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the test shape in the outline below.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-supervisor/src/launch.rs`, `clone_with_repo_cache`: runs, as argv under the repository lock, the cache
  `git init --bare`, the cache `git … fetch --prune`, and `git clone --reference <cache> --dissociate -- <url> <cwd>`,
  each through `launch_child_command` with `REPOSITORY_ENV_VARS` removed and `GIT_TERMINAL_PROMPT=1`.
- Tests in the same file: `fake_git_script`, the fake git most shim tests install, which records each stage's argv;
  `d1_fake_git_hook_agent_run_in_order_with_exact_argv`, which asserts `init\nfetch\nclone\n` and each command's exact
  argv; two real-git d2 tests, both mapping the URL to a plain path, among them
  `d2_real_git_offline_clones_the_instead_of_fixture`, which runs the real shim with real git, maps
  `https://github.com/example/repo.git` to a bare fixture through `GIT_CONFIG_COUNT`/`url.<path>.insteadOf` in the
  child's environment, and checks Ready, the tracked file, no alternates, the GitHub origin and the cache's refs.

Here's the work, as a proposal:

1. Add `-c core.commitGraph=false` before `clone` in `clone_with_repo_cache`, with the comment the acceptance criteria
   ask for.
2. Update the fake git the shim tests install (`fake_git_script`). It picks its stage from its first argument and treats
   `-c` as the fetch, so once the clone also starts with `-c`, it records a second fetch and exits 99, failing the
   exact-argv test and every other fake-git shim test. Make it tell the stages apart another way (for example by the
   first non-`-c` subcommand, or the first two arguments), update its docstring, and update the exact-argv test's
   expected clone argv.
3. Add the regression test. Keep it narrow; do not copy the d2 tests' tag, pull-ref, second-checkout, alternates, origin
   or cache-removal checks, which are not this test's subject, and do not fold it into them. It needs:
   - a one-commit upstream in a bare repository;
   - the cache pre-created at the fixture's `repo_cache_path` before the shim runs: bare init, a fetch from the upstream
     with the shim's refspecs, then `git commit-graph write --reachable`, as the TODO entry asks. The shim's own fetch
     then brings nothing new, so the graph still covers HEAD. (Planning considered `fetch.writeCommitGraph=true` in the
     child's environment and rejected it: it writes a split graph elsewhere, its effect on the bug is unverified, and it
     would apply to all three commands.)
   - the GitHub URL mapped to `file://<bare upstream>` through the child's `GIT_CONFIG_COUNT`/`insteadOf` entries, not a
     plain path;
   - premise checks before the shim runs: the cache's `objects/info/commit-graph` exists, `git commit-graph verify`
     passes, and the cache's branch points at the upstream HEAD (verify alone does not show the graph covers it);
   - assertions that preparation reaches Ready and the tracked file is in the checkout, with no hook.
4. The mutation check from the acceptance criteria.
5. SPEC_impl.md's command listing and one sentence of why.
6. Changelog fragment, and remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-supervisor launch tests (the preparation tests in `launch.rs`, by filter) with the
  pinned tmux, per `.agents/narrow-tests.md`;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown and `python3 releasing/check-changelog.py format`.

No browser, desktop, installer or full-battery runs: nothing else is affected. The report asks the maintainer to create
a managed checkout of this repository on the Mac once the change ships, since no automated check here runs macOS's git.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-git-clone-commit-graph-workaround-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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
  `plan/git-clone-commit-graph-workaround/<nn>-<short-name>` (for example
  `plan/git-clone-commit-graph-workaround/01-clone-without-commit-graph`).
- This is expected to be one PR. Split only if review gives a concrete reason, each PR leaving main correct on its own.
  Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits; this is a `fix`. Its PR adds the changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. The
  fix PR's commit message and description are not empty: they carry the nuances the maintainer asked for (Requirement
  sources, the user's decisions), and the cold read checks that a reader who never saw this plan comes away with them.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on Opus 5.5
at high effort, a native sub-agent where the executing harness can launch one, otherwise shelled out to a harness that
serves that model. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that the flag is on the clone only, and nowhere else;
- that the regression test's fixture really sets up the trigger (a commit-graph in the cache covering the checked-out
  commit, and a `file://` transport), with the premise asserted, so it would fail on git before 2.55 without the flag;
- that the test isolates git through the child's environment and never changes the test process's own.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a git version check, a retry or
  recovery path after a failed checkout, or changing the cache fetch (examples, not a blacklist);
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
alternatives considered. In particular:

- how the fake git tells the stages apart;
- the comment and SPEC_impl.md wording;
- the mutation check's result;
- the stack's shape;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a git version check or warning, repairing
already-failed checkouts, changing the cache fetch, or the dissociate-free alternative.

If the regression test cannot be made to fail without the flag on this host's git (older than 2.55) however the fixture
is built, that is a finding, not a reason to weaken the test: record what you tried in the report and ship the fix with
the test that asserts the trigger's premise.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
