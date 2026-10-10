# Execute the omp-bun-pane-proof triage outcome: apply the OMP pane check to every launch type

Written against main at bd8d5d76 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this outcome's own PR amends SPEC_impl.md as described below.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcome recorded in root `TRIAGE_OUTCOMES.md` under the heading `omp-bun-pane-proof.md` (outcome
`fix code`), exactly as root `AGENTS.md` section "Execute triage outcomes" prescribes: one reviewable commit, one stable
bookmark and one draft PR.

The feature is Resume for OMP sessions. Farhelm loads a reporter into the OMP it launches, and the supervisor accepts a
conversation report only after checking the process chain from the reporting process up to the session's terminal pane
(the "corridor"), so a nested OMP cannot claim the session. One rule in that check, refusing a Bun or Node pane process
that is not itself the reporting runtime, applies only to launches of the installed `omp` command. For a direct
`bun <entry> ...` command launch whose pane arguments cannot be read (in practice, over 64 KiB), a nested reporting OMP
below it could be accepted, and Resume would open the wrong conversation. The trigger is close to unreachable; the user
chose the fix because it is tiny and makes the rule uniform.

The ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do. Read
it and the feedback file `review_feedback_queue/omp-bun-pane-proof.md` before starting. The ledger entry
`omp-corridor-uncounted-pane-runtime.md` records the earlier fix for installed `omp` launches that this extends. Where
this file and the ledger seem to disagree, the ledger entry wins and the disagreement is a DECISION to log (or a
question, per Unattended fallback).

Acceptance criteria:

- One draft PR exists in which, for every OMP launch program, a Bun or Node pane process that is not the reporting
  runtime refuses attribution unless its arguments are readable and match that launch program's expected launcher, so
  behavior for installed `omp` launches is unchanged.
- Unit tests cover the unreadable Bun pane under a Bun launch (refused) and a readable npm launcher AT the pane position
  (admitted); the existing `bun x` positive control stays.
- SPEC_impl.md's corridor text no longer scopes the rule to installed `omp`.
- The PR removes the feedback file and its index line, updates its own `TRIAGE_OUTCOMES.md` Execution field, and passed
  the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's decision (2026-10-09 triage):** "yeah fix code with gate", on a recommendation that described the fix as
about ten lines plus two or three unit tests and a SPEC_impl.md sentence, with the gate to bounce back if it is much
more.

**The user's plan-time decisions (2026-10-09):** review gate "gpt-6.1-sol high reviewer, no swarm"; one plan per
outcome; no-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanism and test shape below. A fresh-context planning review checked them against the
code and found no unnecessary complexity.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code — read the `agent_kind` module map before touching per-harness
behavior; Agent scratch space), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle),
and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on main at bd8d5d76.

- In `crates/farhelm-supervisor/src/procs/omp.rs`, `omp_corridor`, the pane-process rule is wrapped in
  `if matches!(program, OmpLaunchProgram::Omp)`. Drop that guard and check the pane against the launch program's own
  launcher rule: for a Bun launch, a pane that is a Bun or Node image and not the emitter is admitted only when its argv
  is readable and `is_omp_bun_launcher` matches; for an npm launch, `is_omp_npm_launcher`; for installed `omp`, no
  launcher is accepted, which is today's behavior. Use whatever per-program launcher matcher the module already has
  rather than adding a new one. The change is about three lines plus the matcher selection.
- Behavior change to state in the PR description and the changelog fragment: a launcher sitting directly at the pane
  position was never checked before, so non-exact Bun or npm launchers there (a version-pinned package, extra flags)
  were admitted and now lose their Resume offer. That matches SPEC_impl.md's "exact package selection" wording; it is
  fail-closed and rare.
- Tests: the existing positive tests in `crates/farhelm-supervisor/src/procs.rs` (search for the `bun x` and npm
  launcher corridor cases) put the launcher in a middle link under a shell pane. The new positive control must put the
  readable npm launcher AT the pane position. Add the refusal case for an unreadable (argv `None`) Bun pane under a Bun
  launch with a nested reporting OMP below it.
- SPEC_impl.md, the OMP corridor paragraph (search for "a Bun or Node pane process that is not the runtime itself"):
  drop the installed-`omp`-only scope. The next sentence, "Attribution repeats around the evidence", is stale (admission
  now checks the recorded chain once); correct it while there.
- Changelog: `fix:` with `kind: fixed`, written for someone running Farhelm (OMP sessions launched through a custom Bun
  or npm command), or `kind: none` with a reason if nothing user-facing can honestly be said.

### Complexity gate

The user's gate: if the fix turns out to need significantly more than the above (roughly ten lines in the corridor plus
two or three unit tests and the spec sentence), for example a new launcher classification, changes to the reporter
asset, or changes outside `procs/omp.rs` and its tests, do not grow it. Block per Unattended fallback with what you
found and the options.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, clippy on the touched crate, and a focused
nextest selection for the supervisor's `procs` OMP tests through `scripts/record-test-run.py` (`--tmux none` is fine if
the selection does not use tmux; see `docs/test-run-evidence.md`). Run `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md` and apply `.agents/test-authoring.md`. `dprint check` on the changed Markdown.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-omp-pane-guard-all-launches-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution field on the PR) rather than starting over. If it does not exist, this is a fresh
start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
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
  `plan/omp-pane-guard-all-launches/01-omp-pane-guard`.
- One outcome, one commit, one bookmark, one draft PR, per root `AGENTS.md` (Execute triage outcomes). Within this run,
  if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an earlier
  run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- The PR removes the feedback file `review_feedback_queue/omp-bun-pane-proof.md` and its line in
  `review_feedback_queue/INDEX.md` (or narrows them, where this file says so), and updates its own `TRIAGE_OUTCOMES.md`
  Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID and bookmark before
  creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, if it changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
its changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: the `TRIAGE_OUTCOMES.md` entry, the outline above, and the user's
decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as
root `AGENTS.md` requires. Also ask the reviewer to check specifically that installed `omp` launches behave exactly as
before. Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here
or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new launcher classification, changes to the
reporter asset, changes outside `procs/omp.rs` and its tests beyond the spec sentence; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the ledger entry, the user's decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the pane rule picks the launcher matcher per launch program, the Conventional
Commit type and changelog kind, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. The complexity gate above is a block trigger. If the work needs such a decision, record the concrete
tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10). If current code or specs have moved so that the
recorded decision no longer applies, that is a question for the user too, per root `AGENTS.md` (Execute triage
outcomes); never re-triage the item yourself.

## Done criterion

The plan is complete when its one draft PR exists, meets the acceptance criteria above, and (if it changes code, tests
or scripts) passed the review gate, with its `TRIAGE_OUTCOMES.md` Execution field updated and its queue item removed or
narrowed as this file says. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Blocked

Blocked while landing on 2026-10-10 (claim 88c8ce).

### The agreed fix would likely take Resume away from OMP sessions started through npm or npx on macOS

This plan carries out the triage decision on `omp-bun-pane-proof.md`. That decision is about which program in an OMP
session's chain of processes Farhelm trusts when Farhelm's reporter, which runs inside OMP and tells Farhelm which
conversation is open, says which conversation a session can resume. The gap: when an OMP session is launched through Bun
or npm rather than the installed `omp` command, and the session's top process (the first program the session started;
OMP runs at or beneath it) has command-line arguments Farhelm cannot read (over 64 KiB), a second OMP running inside the
session could have its conversation taken for the session's own, so Resume would open the wrong conversation. You
decided to close it with a small rule: for every way of launching OMP, the session's top process must either be the OMP
whose reporter is speaking, or have readable arguments that match that launch method's expected launcher. The completion
criteria asked for a test showing a readable npm launcher at the top is still accepted.

The plan's PR (#1779, not merged; nothing of this plan is on main) implements exactly that, and its tests pass. The
landing review found the rule rests on an assumption that does not hold for npm: npm (which runs as a Node process)
rewrites its own command line when it starts, so a real npm or npx process never shows the command line the rule
expects. The plan's npm test passes because it uses that expected shape, not one a live npm process has. The reviewer
confirmed the rewrite with a live `npx` run on Linux; Bun's `bun x` and `bunx` keep their arguments and do match.

What that means for users depends on the system shell that npm uses to start the program:

- Linux, where `/bin/sh` is usually dash: no change. npm's own shell stays in the chain and is refused by the current
  code as an unrecognized program in between, so these sessions lack Resume before and after this change; only the
  wording of the refusal changes.
- macOS, where `/bin/sh` is bash: bash hands its process over to the program, so npm's process is the session's top
  process and was accepted until now. With the new rule it is refused every time, so Resume for npm- and npx-launched
  OMP sessions on macOS would most likely stop working. This was not tested on a Mac.

Meanwhile the gap being closed needs a top process with over 64 KiB of arguments and a nested OMP that loads Farhelm's
reporter, which triage judged negligible.

### Options

1. Narrow the rule: refuse a non-reporting Bun or Node top process only when its arguments are unreadable, which is the
   actual gap, and keep accepting readable ones as today. Readable Bun or Node programs at the top stay accepted even
   when they are not the expected launcher, exactly as today. This closes the gap with no change for npm or npx
   launches, but revises your earlier decision: the launcher match is dropped. Recommended.
2. Apply the launcher match only to Bun launches (where the arguments really are kept), and keep npm launches as they
   are today. This keeps the stricter check where it can work, at the cost of a rule that differs per launcher.
3. Land it as is, accepting that npm and npx OMP launches on macOS likely lose Resume.
4. Drop the fix, given how unlikely the gap is.

Separately, and not caused by this plan: Farhelm's documentation lists `npx` launchers as recognized for OMP, and the
same npm rewrite means that recognition cannot match a live npm process wherever npm sits in the chain. Whatever you
choose here, that claim may need correcting.
