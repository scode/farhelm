# Stop agent-started Podman containers through Podman

Written against main at 63d8ecd5 on 2026-11-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Stop containers an agent started through podman before killing processes.** Today Stop, tab close and Delete end a
> podman container that an agent or tab started (`podman run -d`, say) only by killing its processes, because the
> container's host-side helpers (podman's conmon and its network helper, pasta) stay in the scope of whatever started
> them and inherit its `FARHELM_*` markers. Killing conmon that way leaves podman's record of the container stuck
> ("stopping", or "running" with nothing behind it), and only `podman rm -f` clears it; a `--rm` container is never
> cleaned up. Stop such containers through podman first, then run the existing sweep as the backstop. Findings from
> 2026-10-28, checked against rootless podman 4.9.3: - Find them without scanning every process: read the operation's
> own scope (`cgroup.procs` of the agent's scope, a tab's scope, or for Delete every scope of the session, which Delete
> already lists by name) and keep the processes that are conmon. The scope is the attribution; no environment read is
> needed, so it also works for a process that rewrote its environment. - A container's main conmon names the container
> on its command line (`-c <id>`, `-n <name>`) and carries podman's exact storage settings in its `--exit-command-arg`
> list (`--root`, `--runroot`, `--cgroup-manager`, `--tmpdir`, and so on). Run `podman stop` with those settings so a
> container in non-default storage is still found. - A conmon with `--exec` belongs to a `podman exec` session, not to a
> container the session started: the container may be the user's own long-lived one. Never stop that container; ending
> the exec's conmon (as the sweep already does) is the right cleanup there. An exec with a terminal ends by hangup when
> its conmon dies; one without a terminal does not, which is a residual to state. - Run podman with the `FARHELM_*`
> markers removed from its environment. Podman's shared per-user pause process inherits the environment of whichever
> podman command recreates it, and it is shared with every rootless podman use on the account. Killing it is safe
> (verified: podman recreates it on the next command, joining a running container's namespaces, or making fresh ones
> when nothing runs), but a marked pause process gets swept for no reason. - Stop in parallel, under a time bound, and
> treat podman's reported state rather than its exit status as the result: `podman stop` can exit non-zero after a
> killed exec session even though the container stopped. A stop that fails or hangs falls through to today's sweep, so
> the worst case stays today's messy state, never a stuck operation. - Which operation stops which containers follows
> the scope: Stop stops what the agent started, tab close what that tab started, Delete everything the session started.
> Containers started outside Farhelm, by systemd units or the user's own shell, are never in a session's scope. - Out of
> reach, unchanged: Docker containers (started by the root daemon, not by the agent's processes) and podman run under
> `sudo` (root's conmon). On a host without a usable systemd user manager there are no scopes; the cheap fallback is
> `podman ps` and checking each listed container's conmon for the session's markers, which costs one read per container
> and covers only podman's default storage. - Farhelm manages a container session's own container. This entry covers
> containers an agent starts itself, in any session.

### Where it stands (from planning, at main 63d8ecd5)

- Every lifecycle cleanup goes through one reaper, `reap_process_tree` / `reap_process_tree_with_sweep` in
  `crates/farhelm-supervisor/src/service/sweep.rs`: validate the carried roots, decide the scope units (`ScopeUnits`),
  kill each scope (`kill_scope_with_grace`: SIGTERM, up to 5 s, SIGKILL, confirm gone), then the portable marker sweep
  unless containment was complete. Callers: Stop (live and already-exited agent, `service/handlers.rs`,
  `sweep::stop_live_agent`), Restart (`service/core.rs`, `service/core/host_restart.rs`), create rollback (`core.rs`),
  tab close and the automatic close of a tab whose shell exited (`service/core/host_tabs.rs`,
  `Supervisor::reap_tab_tree`), Delete (`service/teardown.rs`, which gathers every scope of the session, all generations
  and closed tabs), and startup cleanup of old terminals (`service/core/old_terminals.rs`).
- `podman run` and `podman exec` leave conmon (and for `run`, pasta) in the caller's cgroup with the caller's
  environment, `FARHELM_*` markers included, while the container's own processes go into a separate `libpod-<id>.scope`.
  So today's scope kill ends conmon and pasta but not the container's workload, and Podman's record is left "stopping"
  (or "running" with nothing behind it) until `podman rm -f`; `--rm` is never honored. (Checked on rootless Podman 4.9.3
  during the original sandbox design; SPEC_impl.md "Podman places, creation and images" records why the helm wraps its
  own `podman run` in a scope.)
- Scope membership is read only by `ScopeManager::process_snapshot` (`crates/farhelm-supervisor/src/scope.rs`), a
  diagnostic read of `cgroup.procs` with a 16 KiB cap that cuts its list to 32 processes for the log.
- `crates/farhelm-supervisor/src/container.rs` already runs podman with every `FARHELM_*` variable and the private tmux
  variables stripped (`control_command`), through a bounded group child (`run_control`, 30 s), resolving podman on PATH
  at use, and its tests inject a fake podman script by path.
- Session notifications exist (`crates/farhelm-supervisor/src/service/notifications.rs`: one row per session, launch and
  kind; the wording rule is that every text names something the user can do). Delete's reply already carries notices the
  app must show (`service/teardown.rs`, the checkout-not-archived messages).

### Acceptance criteria

1. On a Linux host, every operation that goes through the shared reaper first stops, through Podman, the containers
   whose main conmon is a member of that operation's scopes: Stop and Restart the agent's, tab close (including the
   automatic close) that tab's, Delete everything the session started (every scope Delete already gathers), create
   rollback and startup cleanup what their scopes hold. Then today's cleanup runs as before.
2. A conmon whose command line marks a `podman exec` session is never used to stop its container (the container may be
   the user's own long-lived one); the exec session ends as processes, as today.
3. Podman runs with the `FARHELM_*` markers and the private tmux variables stripped, from the supervisor's own PATH,
   never from a program named in a process's command line. Storage-locating settings taken from conmon's command line
   (`--root`, `--runroot`, `--tmpdir`, `--cgroup-manager`, `--storage-driver` and similar global flags) pass only
   through an allowlist, each as its own argument, never through a shell.
4. Each container is stopped with `podman stop --time 5`, all in parallel, under one deadline of about 15 s for the
   whole step; the outcome is Podman's reported state, read only when a stop fails or times out (a stop can exit
   non-zero although the container stopped). A container started with `--rm` ends removed. A stopped container that was
   not started with `--rm` is left in place, Delete included.
5. A stop that fails or times out never fails or delays the operation beyond the bound: today's cleanup runs as the
   backstop. It is logged, and the user is told: on Stop, Restart and tab close through a session notification (a new
   kind, recorded under the session's current launch, worded per the notifications module's rule, for example naming the
   container and `podman rm -f`); on Delete through Delete's existing reply notices (not for an environment session's
   Delete that destroys its environment, which drops its notices on purpose); create rollback and startup cleanup log
   only.
6. Out of scope, stated in SPEC.md: Docker, `sudo podman`, Podman on macOS (its containers run inside the Podman machine
   VM, so nothing of them is in a session's processes), and launches without a systemd scope.
7. SPEC.md "Lifecycle operations" gains a paragraph (and a sentence in the paragraph on unconfirmed cleanup: a failed
   container stop falls back to the process cleanup and is not unconfirmed cleanup); SPEC_impl.md's process-tree section
   gains the mechanism and corrects its note that scope membership is only ever a diagnostic snapshot.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning pages:

- **Every caller of the shared reaper**, not only Stop, tab close and Delete.
- **No fallback for Linux hosts without a systemd user manager.** The maintainer: "if this is about linux without
  systemd that is unsupported". Such launches have no scope, and this plan does nothing there.
- **On Delete, stopped containers not started with `--rm` are left in place.**
- **A failed or timed-out stop also produces a notice in the app**, in addition to the log.
- **Tests:** fakes for every case, plus one opt-in test against real rootless Podman in throwaway storage.
- **Release note:** "No release note needed." (no highlight; the fragments still describe the fix for the full list).
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.
- Stated on the planning page and not objected to: the 5 s / about 15 s bounds; pods get no special handling; podman
  from the supervisor's PATH; Docker, `sudo podman` and macOS out of scope.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry (with its findings from 2026-10-28) and the picks above.

**Binding repository constraints:** root `AGENTS.md` "Sharing the machine with other agents" (no fixed paths, unit or
container names in tests; derive them from the run's random directory) and "The live install is off-limits";
`.agents/validation.md`, `.agents/test-evidence.md` (a `SKIPPED` print is missing coverage, not a pass) and
`.agents/test-authoring.md` (assert the fixture premise; no unannotated sleeps; never modify the test process's
environment); root `AGENTS.md` "Conventional Commits", "Changelog fragments".

**Planner proposals** (verified against the code and reviewed by an independent planning reviewer):

- **One hook in `reap_process_tree_with_sweep`**, after the scope units are final and before the first `kill_scope`,
  with the policy in its own module; skipped entirely when podman is not on PATH.
- **Reuse `process_snapshot`'s reader** for membership (its 16 KiB `cgroup.procs` read), moving the 32-process cut to
  where it is logged, rather than adding a second reader; the scope fake gains scripted members.
- **A small conmon command-line parser** over `procs::read_process_argv`: argv[0] basename `conmon`, a 64-hex container
  id (`-c` / `--cid`), the exit-command arguments ending in `container cleanup`, no `--exec`; same uid; pin identity
  with `ProcessIdentity::read`. The storage allowlist is not a security boundary (a same-user session process can
  already edit Podman's configuration); its point is never to run a program, or load configuration, named in
  session-writable text outside every session's cleanup.
- **Reuse `container.rs`'s environment-stripping runner**, changed only to take a deadline; resolve podman at use as it
  does, so no caller has to pass a new setting.
- **Verify on Podman 4.9.3 and 5.x** whether `podman stop` itself runs cleanup and removes a `--rm` container, whether
  `--rm` shows in conmon's command line, and whether conmon still lands in the caller's scope on 5.x. Keep an explicit
  `podman rm` and a short bounded wait for conmon to leave the scope (so the scope kill does not land on Podman's own
  cleanup holding its locks; reuse `poll_until_gone`) only if that verification shows they are needed. Log each finding.
- **The reaper returns a small result** (the containers it could not stop) instead of nothing, so callers that only
  check for an error need no edits, and the callers that notify read it.

## Outline

1. `plan/stop-agent-podman-containers/01-stop-through-podman` (`fix:`): criteria 1-4 and the logging half of 5; the
   parser, membership, runner and hook; a podman seam for tests (a fake conmon process whose command line is crafted,
   for example the fixtures binary answering to the name `conmon`, and a fake podman script injected by path that logs
   its arguments and environment and answers `inspect` from a state file); unit tests (parser tables, outcome logic,
   reaper ordering: podman runs before the first scope kill while the fake conmon is alive); e2e Stop, tab close and
   Delete with fakes under the scope-gated harness; SPEC.md and SPEC_impl.md text; changelog fragment (`kind: fixed`).
2. `plan/stop-agent-podman-containers/02-stop-failure-notices` (`fix:`): the notice half of 5: the new session
   notification kind and Delete's reply notice, with tests; edits PR 1's fragment.
3. `plan/stop-agent-podman-containers/03-real-podman-test` (`test:`): one opt-in module against real rootless Podman,
   gated on the production Podman probe, using non-default storage under the run's own directory (`--root` /
   `--runroot`, which also exercises the storage allowlist), a `--rm` and a plain container and an exec session into a
   pre-created container, names derived from the run directory, cleanup in a guard; inventory entries; removes this
   plan's TODO.md entry.

## Fallbacks

- If Podman 5.x no longer leaves conmon in the caller's scope, the hook finds nothing there: keep it for 4.9, record the
  5.x finding, and say so in the report.
- If a pod's infra container refuses to stop while its members run, let it fall through to today's cleanup and state the
  residual in SPEC.md and the report; do not add pod handling.
- If no machine available to you has rootless Podman with a user manager, land PR 3's module so it skips with a clear
  `SKIPPED` message, report it as unrun coverage, and say so.

Decisions to log: each Podman behavior verified and on which version, whether the explicit `rm` and the conmon wait
survived, the notification wording, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-stop-agent-podman-containers-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-stop-agent-podman-containers-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/stop-agent-podman-containers/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it (a user-visible fix is `fixed`, not `none`, even when the release note answer is "No
  release note needed."). A later PR in this stack that amends work an earlier PR of this stack built edits that PR's
  fragment instead of adding a second one. Where what got built departs from a release note entry (a renamed control, a
  part left out, a limitation it does not mention), the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.
- A new test selection, or new coverage of an existing one, is described in `.agents/validation.md`'s inventory (and
  `.agents/run-all-tests.md`'s lists for a new selection) in the same PR.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
