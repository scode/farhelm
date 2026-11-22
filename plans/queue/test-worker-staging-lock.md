# Stage test worker runs per invocation

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Protect a test worker's checkout before staging.** The SSH driver copies tools, syncs and prunes source, and
> snapshots the result before `with-lock.sh` takes the execution lock. A second request can change the first run's
> checkout even when the worker then refuses it as busy. Hold ownership across staging and execution or stage in an
> isolated tree that can be installed under the lock. Keep incremental builds and the supported behavior where an
> interrupted local command leaves its remote run going. Verify that a rejected second request leaves the first run's
> tools and source byte-for-byte intact.
>
> Include standalone bootstrap, which also copies tools. Bind collection to one invocation so a request that never ran
> or a run whose lock has just been released cannot copy another run's evidence. Distinguish older artifacts when a
> caller reuses an output directory. Plan: `plans/queue/test-worker-staging-lock.md`.

### Where it stands (from planning research at main b34ceee9f)

The test worker is a Linux machine the maintainer runs, reached over ssh or `tailscale ssh`, that builds and tests the
checkout's tracked files itself (`.agents/test-worker.md`). The driver is `scripts/test-worker/test-worker.sh` (its
header, about lines 1-28, is the command reference), with worker-side scripts in `scripts/test-worker/remote/`. The
worker keeps one tree per checkout directory name under `~/farhelm-test-worker/<checkout>/` (`tools/`, `src/` with its
own `target/`, `out/<cmd>/`, `lock`, `lock.holder`, `lock.pgid`, `synced.list`, `source.txt`).

One `rust`, `gui` or `run` request (`test-worker.sh`, about lines 157-168) runs, in order:

1. `push_tools` (about lines 95-98): `rsync -a --delete` of the driver's own `remote/` directory into the shared
   `tools/`. The tools come from the driver's directory, not from `--checkout`.
2. `ensure_bootstrapped` (about lines 137-144): compares the worker's `bootstrapped` marker with the hash of
   `remote/bootstrap.sh` and runs the bootstrap when they differ. `bootstrap.sh` is worker-wide: apt, rootless Podman,
   sshd, `known_hosts` for self-ssh, lingering; it sources `env.sh` beside itself, and its `bootstrap.lock` (about lines
   24-27) only keeps two bootstraps apart.
3. `sync_tree` (about lines 104-125): `rsync -a --files-from` of the tracked files into `src/`, then `prune.py` deletes
   files the previous list named and this one does not (never `target/`), then `snapshot.sh` replaces `src/.git` with a
   fresh one-commit repository and writes `source.txt`.
4. Only now `with-lock.sh` (`remote/with-lock.sh`): `flock -n` on the tree's `lock`; when held it prints the holder and
   exits 75; otherwise it writes `lock.holder`, starts the runner with `setsid` so it outlives the ssh session (the lock
   fd is inherited), records `lock.pgid`, and waits.
5. `fetch_out` (about lines 128-133): rsync of `out/<cmd>/` into the local `--out`, whatever step 4 returned, without
   clearing `--out` first.

So a second request from the same checkout (a re-run after the local command was interrupted while the remote run went
on, or two worker commands started at once) rewrites the first run's tools and source and replaces its `.git` before it
is refused, and then copies the first run's in-progress or finished `out/<cmd>/` as if it were its own. Standalone
`bootstrap` (about lines 153-156) also overwrites `tools/` without the lock, and any checkout's bootstrap can run apt or
`podman system migrate` while another checkout's tests use Podman and ssh. Runners source `$base/tools/env.sh` and write
`$base/out/<cmd>/` (`run-rust.sh`, `run-gui.sh`, `run-run.sh`: `summary.txt`, `console.log`, `smoke.log`,
`lifecycle.log`, `runs.tar.gz`); `stop` and `status` (about lines 170-176) also use the shared `tools/` and `out/*/`.
Nobody has observed an affected run; nothing visible goes wrong when it happens, which is why triage treated it as
evidence corruption (triage record `test-worker-sync-precedes-lock.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, about line 34090).

What exists to reuse: the tree lock and its `setsid` detachment (the detached-run contract), the `bootstrap.lock` flock
pattern, `prune.py` and `snapshot.sh` (both work unchanged when run later), rsync `-a` keeping the development machine's
mtimes (cargo's incremental fingerprints depend on them), `--config FILE` (so a test can point the driver at a stand-in
worker), and `status`. The harness's checks today are `shellcheck -x` on the driver and remote scripts and `py_compile`
of `prune.py` (`.agents/validation.md`, about lines 1187-1191; `.agents/run-all-tests.md`, about lines 149-151); there
is no test of the driver.

### Acceptance criteria

1. Every invocation gets an id, printed at its start. Before it holds the tree lock it writes only into its own staging
   directory on the worker (its tools, source and file list); nothing shared in the tree (`src/`, `.git`, `source.txt`,
   `synced.list`, another invocation's staging or tools) changes until it holds the lock, and no invocation ever writes
   tools another invocation executes.
2. Under the lock, the invocation installs its staged source into `src/` keeping file mtimes (so incremental builds stay
   incremental), prunes with its own file list, snapshots, and runs from its own staged tools; the run still survives
   the local command being interrupted and is ended only by `stop`.
3. A request refused because its tree is busy leaves the running invocation's tools, `src/` (outside `target/`), `.git`
   contents, `source.txt` and `synced.list` byte-for-byte unchanged, removes its own staging, and copies nothing back.
4. Standalone and automatic bootstrap run from their invocation's own staged tools and never write another invocation's.
   Two bootstraps still queue on `bootstrap.lock` as today, so two fresh checkouts' first runs both work. A bootstrap
   that finds any checkout's run active on the worker refuses at once, with an explanation naming the run holding the
   worker lock and pointing at `status` and `stop`. A run that finds a bootstrap in progress waits for it, for a bounded
   time (bootstrap takes minutes), then refuses with the reason.
5. Evidence lives per invocation on the worker. A request copies back only its own invocation's evidence, and only when
   it took the lock. A reused local `--out` is cleaned of the harness's own artifact names before anything else and
   stamped with the invocation id and the source commit, so older artifacts cannot be mistaken for this run's.
6. `test-worker.sh collect --out DIR <id>` fetches a finished invocation's evidence later; for one still running it says
   so and copies nothing; for an unknown or pruned id it says so.
7. The worker keeps the newest 20 invocations' evidence per tree, pruned under the lock, never the running one; stale
   staging left by a driver killed mid-staging is swept under the lock without ever touching a request that is staging
   now.
8. A committed test drives the real driver against a stand-in worker faked locally and covers criteria 1-7; it never
   reaches the real worker or reads `~/.farhelm-test-worker`.
9. The driver's header, `.agents/test-worker.md`, and (for the new test) the inventories in `.agents/validation.md` and
   `.agents/run-all-tests.md` describe the new behavior; the reused-output caveat becomes a description of the stamping.
10. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Mechanism:** per-invocation staging on the worker, installed into the tree only under the lock, preserving rsync
  `-a` mtimes for incremental cargo builds and the detached-run contract (an interrupted local command leaves the remote
  run going).
- **Evidence:** bound to one invocation; nothing copied back on refusal; a reused local `--out` cleaned of the harness's
  own files and stamped with the invocation.
- **Standalone bootstrap is covered.**
- **Both extras:** (a) worker setup (bootstrap) waits for, or refuses during, any checkout's running tests, since
  bootstrap is worker-wide; (b) a `collect <id>` command fetches an interrupted run's evidence later instead of
  re-running. Settled during planning review: bootstrap against bootstrap keeps queuing on `bootstrap.lock` as today;
  bootstrap against any checkout's running tests refuses at once, naming the holder; a run that finds a bootstrap in
  progress waits for it, bounded.
- **Remote evidence retention:** the planner's choice (see Planner proposals: the newest 20 invocations per tree).
- **Validation never touches the real worker:** a committed test with a stand-in `ssh` and worker faked locally; update
  `.agents/test-worker.md`, and the inventories in `.agents/validation.md` and `.agents/run-all-tests.md` for the added
  check script.
- **Shape:** two `chore:` PRs, no changelog fragments (maintainer tooling; nothing shipped changes).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- `.agents/test-worker.md`: the worker is used only when the maintainer asks for it in the task; its name stays out of
  the repository; what may cross to it and back.
- Root `AGENTS.md`, "Tests and validation": a changed or added check or test script updates the inventory in
  `.agents/validation.md` and the lists in `.agents/run-all-tests.md` in the same change; an instruction file change
  runs `python3 scripts/check-agents-md.py`. "Instructions by path": `scripts/` and any test name
  `.agents/validation.md`.
- Root `AGENTS.md`, "Sharing the machine with other agents" (no fixed paths, ports or names in the test harness) and
  "Agent scratch space".
- `.agents/test-authoring.md`, including its rule that a test never modifies its own process's environment variables:
  environment changes go only into the child processes' environments.

**Planner proposals:**

- Staging: the driver mints a uuid, rsyncs `remote/` into `<base>/stage/<id>/tools/` and the tracked files into
  `<base>/stage/<id>/src/` with `--copy-dest` pointing at the tree's `src/` (unchanged files are copied on the worker
  rather than sent again; a relative `--copy-dest` resolves against the destination directory, so spell it accordingly;
  not `--link-dest`, whose hard links would share inodes with the live tree), and sends the file list and source id
  alongside. It then runs `bash <base>/stage/<id>/tools/with-lock.sh <base> <id> run-<cmd>.sh ...`.
- Under the lock, `with-lock.sh` installs with a local `rsync -a stage/<id>/src/ src/` (never `--delete`: `target/` and
  the suites' own ignored files live under `src/`; `prune.py` with the staged list does the deleting), runs
  `snapshot.sh`, removes the staged source copy, and starts the runner from `stage/<id>/tools/` with `setsid` as today.
  The staged tools must outlive the install, because the runner executes from them for the whole run.
- Runners take their tools directory from their own location (`$(dirname "$0")`) instead of `$base/tools/`; `stop` and
  `status` stop depending on the shared `tools/` (stage them like any invocation, or send the script on stdin). An old
  `tools/` left on a worker from before this change is harmless; deleting it under the lock is your call.
- Worker-wide setup lock (extra a): one lock file under `~/farhelm-test-worker/`, held shared by every run for its
  lifetime and exclusively by a bootstrap. Lock order, the same everywhere: a bootstrap takes `bootstrap.lock`
  (blocking, as today) and only then the worker lock exclusively and non-blocking, refusing with the holder's name if a
  run holds it; taking them the other way round would let two bootstraps refuse each other instead of queueing. A run
  takes the worker lock shared, waiting a bounded few minutes if a bootstrap holds it, and then its tree lock
  (non-blocking, as today). Keep a record of who holds the worker lock shared (the checkout and invocation), so the
  refusal can name it. The bootstrap refusal falls under the unattended rule in `.agents/test-worker.md`, "When the
  worker cannot be used" (notify, fall back after 10 minutes).
- Pitfall: the tree lock today is held through an fd the `setsid` runner, cargo, the tests and anything they leak all
  inherit. Holding the worker lock the same way would let one leaked process from one checkout's run block every
  checkout's bootstrap indefinitely (today such a leak blocks only its own tree; `.agents/test-authoring.md` also warns
  about locks carried across spawns). Holding it in the wrapper that waits for the runner, rather than in an fd every
  child inherits, avoids that, but the wrapper can die with the ssh session while the detached run goes on, which would
  drop the lock under a running run. The mechanism is yours to choose; log the tradeoff, and note it in
  `.agents/test-worker.md` and the report.
- Evidence: runners write `out/<id>/`; `with-lock.sh` writes an invocation marker there only once it holds the lock; the
  driver fetches `out/<id>/` only when that happened, and on 75 prints the refusal and copies nothing. Local `--out`: at
  the start of every invocation, delete `summary.txt`, `console.log`, `smoke.log`, `lifecycle.log`, `runs.tar.gz` and
  `invocation.txt` (the harness's own names, nothing else), then write `invocation.txt` with the id and source commit,
  and record the outcome there at the end (refused, exit status, copy failure).
- Retention: keep the newest 20 invocations' `out/<id>/` per tree, pruned by the lock holder. Chosen over 7 days because
  a count bounds the worker's disk however busy a tree gets (failed runs keep `runs.tar.gz`), it is deterministic for
  the test, and `collect` serves the agent that was interrupted, normally within its own session, long before 20 later
  runs from the same checkout. Staging dirs have no completion marker, so sweep them by age (far beyond any staging
  time, for example a day) or by a per-stage lock the staging driver holds; either way a request staging right now is
  never swept.
- The committed test (for example `scripts/test-worker/test-test-worker.py`, Python `unittest` like
  `deflake/test-deflake.py`): a stand-in `ssh` first on the child processes' `PATH` that runs the remote command locally
  with `HOME` set to a stand-in worker home inside the test's own temporary directory (rsync's `-e ssh` goes through it
  too) and refuses any destination but the stand-in one; a stand-in config passed with `--config`; a copy of the driver
  beside a copy of `remote/` whose `bootstrap.sh` is a stub (the driver takes its tools from its own directory), and the
  stand-in refuses to run the real `bootstrap.sh`, which calls `sudo apt-get`; two fixture git checkouts with the same
  directory name in different random parents; a blocker run (`run -- <command waiting on a sentinel file>`) holding the
  lock. Test safety extends to the runners, which act on the machine they run on: the fixture checkouts contain no
  `rust-toolchain.toml` and no `scripts/`; stand-ins for `rustup`, `cargo`, `systemctl` and `podman` come first on the
  child processes' `PATH` and only record their calls (so `use_pinned_toolchain`'s `rustup default` and
  `keep_disk_free`'s `cargo clean` reach nothing real); the ssh stand-in refuses `localhost` like any other destination
  but the stand-in one (so `substrate`'s self-ssh never leaves the test); and the test drives only `run`, the stub
  `bootstrap`, `stop`, `status` and `collect`, with the stand-in refusing `run-rust.sh` and `run-gui.sh` as it refuses
  the real bootstrap. Scenarios: hash run A's tools, `src/` outside `target/`, `.git`, `source.txt` and `synced.list`;
  request B with a changed tracked file and a changed remote tool gets 75, the hashes are unchanged and B's `--out`
  holds none of A's artifacts; killing A's local driver leaves A running, and after the sentinel A's evidence is
  complete, stamped, and fetched by `collect <id>`; a reused `--out` holding a stale `runs.tar.gz` is cleaned; bootstrap
  is refused while A runs, naming it; a run started during a (stub, sentinel-held) bootstrap waits and then proceeds;
  two bootstraps queue rather than refuse; retention keeps 20; an installed file keeps its mtime.

## Outline

1. `plan/test-worker-staging-lock/01-stage-per-invocation`: invocation ids, staging and install under the lock,
   per-invocation tools for runs, bootstrap, `stop` and `status`, the worker-wide setup lock, the stand-in test with the
   byte-for-byte, detached-run, mtime and bootstrap scenarios, the driver header and `.agents/test-worker.md`, and the
   new test in `.agents/validation.md` and `.agents/run-all-tests.md`. `chore:`, no changelog fragment.
2. `plan/test-worker-staging-lock/02-evidence-per-invocation`: `out/<id>/` and the invocation marker, no copy on
   refusal, local `--out` cleaning and stamping, `collect`, retention, `status` narrowed to the newest few summaries
   (with 20 kept it would otherwise print them all), their test scenarios, the documentation and inventory wording, and
   the TODO entry's removal. `chore:`, no changelog fragment.

## Fallbacks

- Never run the harness against the real worker, not even a smoke run: the maintainer did not ask for it in this task
  (`.agents/test-worker.md`). If something can only be shown on a real worker, say so in the report as a check the
  maintainer may want.
- If two PRs turn out to need churn (code PR 1 adds that PR 2 deletes), merge them into one PR and log it.
- A distinct exit status for "evidence copy failed" versus "tests failed" (noted in the triage-tooling report) is out of
  scope; mention it as a possible follow-up only.
- Runs from different checkouts still run at the same time on one worker; that is unchanged and out of scope.
- The `shellcheck` command in the inventories uses a glob; list the remote scripts by name in your own command line
  (root `AGENTS.md`, "Claude Code shell commands"), and keep the inventory wording consistent with the scripts that
  exist.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-test-worker-staging-lock-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-test-worker-staging-lock-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

For this plan: the new stand-in test, `shellcheck -x` on the driver and every remote script, `py_compile` of the Python
files, `python3 scripts/check-agents-md.py`, and `dprint check` on the changed Markdown.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/test-worker-staging-lock/01-stage-per-invocation`, `plan/test-worker-staging-lock/02-evidence-per-invocation`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Protect a test worker's checkout before
  staging"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
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
