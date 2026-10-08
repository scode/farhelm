# Cache repositories on each host for faster fresh checkouts

Written against main at a38ea524 on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

Every fresh GitHub checkout (a `gh:` launch, or Clone on a session in a GitHub checkout) clones the whole repository
from GitHub again, which is slow for large repositories. Keep a cache of each repository on the host, kept current by a
fetch before each checkout, and make the checkout borrow the cache's objects so only what is new crosses the network,
while the checkout stays an ordinary, independent clone of the GitHub repository.

Acceptance criteria:

- A fresh checkout on a host first brings that host's cache of the repository up to date (creating it the first time),
  then clones from GitHub using the cache's objects. Both steps run inside the session terminal, in the user's own
  environment with their own credentials, as the clone does today, so progress, credential prompts and failures stay
  visible.
- The checkout is independent of the cache: `origin` is `https://github.com/<owner>/<name>.git` exactly as today (read
  with `git config remote.origin.url`), there is no `objects/info/alternates`, and deleting the cache or archiving the
  checkout never breaks it.
- The cache lives under the supervisor's state directory, holds branches and tags only (never GitHub's `refs/pull/*`),
  and is deleted once unused for 30 days, by a sweep when the supervisor starts.
- A failure in the cache step fails the checkout's clone stage like any other clone failure. There is no fallback to a
  plain clone anywhere, including git's own: `--reference-if-able` is not used.
- Two simultaneous checkouts of the same repository on one host take turns on its cache.
- SPEC.md, SPEC_impl.md and the user docs describe the cache.
- The last code PR removes the TODO.md entry "Cache repositories locally for faster fresh checkouts."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Cache repositories locally for faster fresh checkouts. Every
fresh GitHub checkout clones the whole repository from GitHub again. Keep a local cache of each repository's contents on
the host, and make a fresh checkout clone from it, fetching only what is new from GitHub, so starting a session in a new
checkout of a large repository is fast. Decide where the cache lives, how it is kept current and bounded, and how a
checkout made from it stays an ordinary, independent clone of the GitHub repository."

**The user's decisions (2026-10-08):**

- D1. Location: the host's Farhelm state directory (the supervisor's), not the working-copy root.
- D2. Bound: a repository's cache is deleted once unused for 30 days, swept when the supervisor starts.
- D3. Failure: no fallback. In the maintainer's words: "no fallback just fail. this is local disk i/o crap etc we don't
  add complexity to deal with failures. this is not some remote systme we need to treat as optional. keep it simple". Do
  not add recovery machinery for disk errors or corrupt caches; a failure message that names the cache's path is enough
  for the user to remove it by hand.
- D4. Always on, with no setting.
- D5. Review gate: "gpt-6.1-sol high effort, no swarm for reviewers."
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:**

- SPEC.md "Fresh GitHub checkouts": clone, configured hook and agent run in that order inside the session terminal, so
  progress, authentication prompts and failures are visible; the clone uses the constructed HTTPS GitHub URL with the
  target user's ordinary credentials; a failed stage prevents later stages and preserves partial content; a completed
  preparation permits restart without repeating clone or hook.
- SPEC.md "VCS neutrality": the system never performs VCS mutations implicitly; an explicit GitHub checkout request runs
  the clone. The cache fetch belongs to that explicit request, and this plan amends both sections to say so.
- SPEC_impl.md "Leftover files": leftovers are acceptable when bounded, removed by the next attempt, or removed at the
  next start of the owning process.
- SPEC_impl.md "Owned checkout admission and lifetime": existing fresh-checkout fingerprint encodings are frozen.
- SPEC_impl.md "Checkout configuration and discovery": repository discovery reads `remote.origin.url` and lists only
  validated GitHub origins, so checkouts must keep `origin` as the GitHub URL.
- Root `AGENTS.md` (Conventional Commits; Finishing work; Releases and the changelog; Sharing the machine with other
  agents; Agent scratch space; The live install is off-limits), `plans/AGENTS.md` (Executing), and
  `.agents/test-authoring.md` for test changes.

**How the clone works today (verified at a38ea524; find code by name):** the helm resolves the repository and checkout
root and sends a resolved checkout to the supervisor; the supervisor allocates the directory and builds a
`CheckoutPreparation` (in `crates/farhelm-supervisor/src/service/core.rs`; the type is in
`crates/farhelm-supervisor/src/launch.rs`); the launch shim inside the tmux pane runs `run_checkout_preparation`, which
takes a per-working-copy flock (`PreparationLock`, `acquire_preparation_lock`, CLOEXEC), writes durable state
(`NotStarted` → `CloneStarted` → `HookStarted` → `Ready` | `Failed{stage, detail}`, path from `preparation_state_path`,
derived by the supervisor and handed to the shim), and runs `git clone -- <https url> <cwd>` with
`GIT_TERMINAL_PROMPT=1` and the session markers that let Stop and Delete reap it, with no timeout. The shim re-validates
the URL's shape. Tests: fake-git argv tests (`d1_fake_git_hook_agent_run_in_order_with_exact_argv` pins the exact argv),
real-git offline tests (`d2_real_git_offline_clones_the_instead_of_fixture`, redirecting the GitHub URL to a local bare
repository with `insteadOf`), the Rust e2e tests in `crates/farhelm/tests/e2e/github_checkouts.rs`, and
`e2e/tests/github-checkouts.spec.ts`. Startup sweeps live beside `sweep_tmux_config_temp_files` in `core.rs`.

**What the planning review established with real git (2.43, against an `insteadOf` fixture shaped like the e2e ones):**

- The option is `--dissociate`, not `--dissolve`. `git clone --reference <abs cache> --dissociate -- <https url> <dir>`
  leaves no alternates, packs that are copies (link count 1), `remote.origin.url` equal to the GitHub URL, and a clean
  `fsck --connectivity-only`. `git remote get-url` shows the `insteadOf`-rewritten URL, so tests read
  `git config remote.origin.url`.
- `--reference-if-able` silently falls back to a full network clone when the cache is missing (info line, exit 0); plain
  `--reference` fails (exit 128). D3 requires `--reference`.
- `git init --bare` on an existing repository is a harmless re-init, and an interrupted first fetch leaves a valid
  repository that the next fetch completes, so cache creation and update can be one sequence with no temporary directory
  or rename.
- Refspecs given on the fetch command line write no remote configuration and keep `refs/pull/*` out.
- A fetch ends with an automatic gc that detaches into the background by default and would outlive the lock. Running the
  fetch with `-c gc.autoDetach=false -c maintenance.autoDetach=false` keeps it in the foreground under the lock. Do not
  disable gc: an actively used cache is never evicted and would grow loose objects forever.
- The checkout clone sends the cache's refs as `have` lines, so only negotiation and new objects cross the network. A
  private repository without a credential helper prompts for credentials twice (fetch, then clone); that is consistent
  with visible prompts and needs only a sentence in the docs.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

Line numbers drift; find the code by name.

### PR 1: checkouts go through a per-host repository cache

`perf:` or `feat:` (your call; log it) with a changelog fragment: fresh GitHub checkouts reuse a per-host cache of the
repository, so a second checkout of a large repository fetches only what is new; the cache costs disk space roughly the
size of one copy of each repository, and is removed after 30 days without use.

- Cache path: the supervisor derives `<state_dir>/repo-cache/<owner>/<name>.git` from the validated, lowercased
  repository pair, at the place in `core.rs` that builds `CheckoutPreparation`, and passes it in a new field (as it
  already does for the preparation state path). The shim checks that it is absolute. The field is derived by the
  supervisor, not request input, so it stays out of the frozen fingerprints. A launch spec from before the field existed
  must deserialize (serde default) and fail the clone stage with a clear message rather than clone without the cache.
- In the shim's clone stage, after `CloneStarted` is written and still inside the terminal:
  1. Take a blocking per-repository lock: a lock file beside the cache, `<state_dir>/repo-cache/<owner>/<name>.lock`,
     reusing `PreparationLock`'s CLOEXEC flock so git children never inherit it. Lock order is always preparation lock,
     then repository lock. A one-line "waiting for another checkout of <owner>/<name>" message after a failed try-lock
     is optional; keep it only if it stays that small.
  2. `git init --bare -q <cache>`, then
     `git -c gc.autoDetach=false -c maintenance.autoDetach=false -C <cache> fetch --prune <https url> '+refs/heads/*:refs/heads/*' '+refs/tags/*:refs/tags/*'`.
  3. `git clone --reference <cache> --dissociate -- <https url> <cwd>`.
  4. Record last use as the lock file's modification time (set while holding the lock), then release the lock. Hold the
     lock through the checkout clone; that is simpler than arguing a concurrent fetch and gc are safe during another
     checkout's `--dissociate` copy.
  - All three git commands carry the same session markers, environment scrubbing and `GIT_TERMINAL_PROMPT=1` as today's
    clone, so Stop and Delete reap them and prompts reach the terminal.
  - Any failure is `Failed{stage: clone}` with a message naming the cache path. No new durable state variant:
    `CloneStarted` already covers the cache step for restart refusal.
- Startup sweep, beside the existing ones in `core.rs`: for each `<owner>/<name>` in `repo-cache/`, try-lock its lock
  file first; if the lock is held, skip it (a launch shim runs in a tmux pane that survives a supervisor restart). Under
  the lock, read the last-use time; if it is older than 30 days, rename the cache to a trash name (for example
  `<name>.git.trash`) while still holding the lock, then remove the trash, so an interrupted sweep never leaves a
  half-deleted cache at the live path. Also remove any trash left by an interrupted earlier sweep.
  `std::fs::remove_dir_all` does not follow symlinks; no custom walker. Never delete lock files: a checkout blocked on a
  deleted lock file would later run alongside a new one with no mutual exclusion. The lock files that remain (one empty
  file per repository ever cached) are the accepted leftover; say so in SPEC_impl.md.
- Tests, per `.agents/test-authoring.md`:
  - Update the fake-git argv test to the three exact argvs (init, fetch, clone) in order, and the other fake-git tests
    whose expectations depend on the argv.
  - Real-git offline tests against an `insteadOf` fixture: a first checkout creates the cache and has no alternates,
    origin equal to the GitHub URL (read with `git config`), and only branches and tags in the cache (a
    `refs/pull/1/head` in the fixture does not arrive); a second checkout after a new upstream commit sees that commit;
    deleting the cache afterwards leaves the checkout's `fsck --connectivity-only` clean; a missing or broken cache path
    fails the clone stage (no full-clone fallback).
  - Sweep tests by backdating the lock file's modification time (no injected clock): a cache older than 30 days is
    removed, a fresh one is kept, one whose lock is held is kept, leftover trash is removed, and lock files survive.
  - The Rust e2e and Playwright GitHub checkout tests use `insteadOf` with a private `GIT_CONFIG_GLOBAL` and
    `GIT_ALLOW_PROTOCOL=file`; make sure the cache fetch works under them, and adjust fixtures only if it does not.
- SPEC.md: amend "Fresh GitHub checkouts" (the clone stage brings the host's cache of the repository up to date and
  clones using it; the cache sits in Farhelm's state directory on that host, holds branches and tags, is removed after
  30 days without use, and a failure in it fails the clone stage) and "VCS neutrality" (the cache fetch is part of the
  explicit checkout request). If the uninstall sections enumerate retained data, add the repository cache where that
  reads naturally.
- SPEC_impl.md: in "Owned checkout admission and lifetime" (or a new subsection beside it), the cache layout, lock, git
  commands and why (`--reference` not `--reference-if-able`, `--dissociate` for independence, foreground gc,
  command-line refspecs), the sweep and its ordering, and the lock-file leftover. Add `repo-cache/` to the state
  directory list.

### PR 2: user docs

`docs:`. Update `docs/github-checkouts.md` and the fresh-checkout part of
`website/src/content/docs/docs/using/start-a-session.mdx` (following `website/AGENTS.md` and
`website/EDITORIAL_RULES.md`): repeated checkouts of a repository are faster because each host keeps a cache; where it
is and how much disk it takes; that it is removed after 30 days without use and can be deleted by hand at any time; and
that a private repository without a credential helper asks for credentials twice. This may fold into PR 1 if that keeps
the stack simpler; log it.

### Validation

Follow root `AGENTS.md` "Finishing work":

- PR 1: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `python -B scripts/check-test-sleeps.py` (per
  `docs/test-sleep-check.md`), and through `scripts/record-test-run.py` (with the pinned nextest and tmux setup from
  `docs/test-run-evidence.md`) the supervisor's launch tests, the new sweep tests, and the Rust e2e GitHub checkout
  tests. Then the browser spec `e2e/tests/github-checkouts.spec.ts` on Chromium and WebKit through the recorder (builds
  per root `AGENTS.md`), since it drives real clones through the UI.
- PR 2: `dprint check` on changed files and the website build
  (`cd website && bun install --frozen-lockfile && bun run build`).
- `python3 releasing/check-changelog.py format` for the fragment.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-repo-clone-cache-log.md`
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
  `plan/repo-clone-cache/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate a review of that PR's
changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

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

Before implementing a substantial departure from the outline above (a fallback to a plain clone in any form, recovery or
repair of a corrupt cache, a cache configuration setting, a size-based cap, a cache shared between users or placed under
the working-copy root, a supervisor-side background fetch outside the session terminal, a new durable preparation state;
these are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run
a fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the PR type and changelog kind, the new field's name and its behavior for old
launch specs, the trash naming, whether the waiting message was kept, whether PR 2 folded into PR 1, and the exact
SPEC.md wording, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code or tests passed the review gate. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
