# Execute nine triage outcomes: development, test-run and release tooling bugs

Written against main at 9046a0db on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `ssh-config-atomic.md`: that plan rewrites how `scripts/test-provision-centos.sh` installs and
removes its block in the user's ssh configuration, which the CentOS fix here changes, so this plan starts once that work
is on main and makes the scope reset part of the block as it exists then.

## The goal

Carry out nine triage outcomes recorded in root `TRIAGE_OUTCOMES.md` in one draft PR. Their headings, which are also the
feedback file names under `review_feedback_queue/`, all with outcome `fix code`:

- `centos-test-changes-meaning-existing-global-ssh.md`
- `tmux-build-script-bash32.md`
- `plans-heading-splice.md`
- `watcher-revision-cache.md`
- `checkout-discovery-strips-valid-trailing-path-characters.md`
- `initial-result-publication-failure-erases-observed-child-result.md`
- `cutover-probe-ignores-output-asserted-boundary.md`
- `non-fragment-files-satisfy-changelog-coverage.md`
- `optional-desktop-interaction-smoke-times-out-during.md`

These are bugs in the maintainer's development tooling: scripts that can damage the user's ssh configuration while they
run, fail on a stock Mac, record wrong evidence, or pass checks they should fail. None changes what Farhelm does for
someone running it.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read every entry and feedback file before starting. Where this file and a ledger entry seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Each fix is demonstrated once as Validation describes, with the result logged.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment where root `AGENTS.md` requires one, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-10):** rather than triage the undecided review findings one by one, the user
asked for a screen of the whole queue against these criteria, in their words:

> - it is CLEARLY a bug. no judgement needed to answer whether it's something that should ideally be fixed.
> - the fix is reasonably straight forward and does not cause complexity or scope creep.
> - it is not mooted because code has changed.

The screen found 96 such findings, and the user chose to fix all of them and to "schedule them for execution in the
planning system with a complexity gate - if it turns more complicated than expected, skip and keep it unfixed for
triage", grouped "so we end up with roughly 10 plans", similar or related findings together. Each outcome's
`TRIAGE_OUTCOMES.md` entry records this and is the authoritative statement of what was decided. The screen was done by
reading code on main at `83516c6c`, not by running anything, so each assessment is a claim to confirm before fixing.

**The user's plan-time decisions (2026-10-10):** one PR per plan, overriding root `AGENTS.md`'s one-PR-per-outcome rule
for these outcomes only; each outcome has a complexity gate, and an outcome that trips it is dropped from the PR while
the rest ships; review gate gpt-6.1-sol at high effort; no-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the fixes and placements in the outline below, taken from each ledger entry's completion
criteria and checked against main by a fresh-context planning review.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; Sharing the machine with other agents; the rule
against tests that modify the test process's own environment variables; SPEC.md and SPEC_impl.md are authoritative and
changed only as decided), the user's global rules to write documentation and comment prose with the `scode-voice` skill
and to give new and changed code, tests included, docstrings and comments that explain the why, `plans/AGENTS.md`
(Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. The fix for each outcome, from its ledger entry (where the code was at
screening time in parentheses):

- `centos-test-changes-meaning-existing-global-ssh.md`: End the inserted block with a `Match all` (or `Host *`) line so
  the original configuration is read with global scope again. (`scripts/test-provision-centos.sh`)
- `tmux-build-script-bash32.md`: Expand the optional arrays with the portable empty-safe form, or refuse bash older than
  4.4 up front. (`scripts/build-private-tmux.sh`)
- `plans-heading-splice.md`: Make the guard also catch indented ATX headings and setext underlines at or above the
  section level, with a round-trip test. (`scripts/plans-queue.py`)
- `watcher-revision-cache.md`: Resolve the branch to one commit per poll and use it for both reads.
  (`scripts/plans-watch.sh`)
- `checkout-discovery-strips-valid-trailing-path-characters.md`: Remove only git's trailing newline.
  (`scripts/record-test-run.py`)
- `initial-result-publication-failure-erases-observed-child-result.md`: Keep the observed result available to the
  fallback write so it is preserved. (`scripts/record-test-run.py`)
- `cutover-probe-ignores-output-asserted-boundary.md`: Assert that no output notification appears before the cutover
  point. (`scripts/check-tmux-cutover.py`)
- `non-fragment-files-satisfy-changelog-coverage.md`: Apply the loader's top-level `*.md` rule (README excluded) to the
  paths the sweep counts. (`releasing/check-changelog.py`)
- `optional-desktop-interaction-smoke-times-out-during.md`: Raise that delete's time limit to 30 seconds, matching the
  default leg. (`scripts/desktop-smoke.sh`)

For the tmux build script, use the portable empty-safe expansion (`${arr[@]+"${arr[@]}"}`) so the build works on a stock
Mac; refusing old bash is only the fallback if that does not work.

`scripts/plans-queue.py` and `scripts/plans-watch.sh` are the planning system's own lock and watcher; this plan changes
their code only through its PR, never `plans/`.

First confirm each assessment against current main: if the problem is already gone, that is not a gate trip; record the
outcome as `discard` with "already fixed" and the fixing commit, per root `AGENTS.md`, and remove its queue item.

Commit type and changelog: `fix:` with a changelog fragment of `kind: none` and a one-line reason, since nothing changes
for someone running Farhelm (or `test:`/`ci:` where that is the more accurate type). Validate a fragment with
`python3 releasing/check-changelog.py format`.

The complexity gate for each outcome is the fix named above. A new mechanism, plumbing across components, a change well
outside the named files, or a product or design decision is a gate trip for that outcome. A regression test that would
need new machinery is skipped and logged, never a reason to drop the fix.

### Validation

`shellcheck` on every changed shell script; `bash scripts/test-plans-watch.sh` and `python3 scripts/test-plans-queue.py`
when those scripts change; `python3 scripts/test-record-test-run.py` when the recorder changes;
`python3 releasing/check-changelog.py --self-test` and `format` when the checker changes; `bash -n` and `shellcheck` for
`scripts/desktop-smoke.sh`. Prove the ssh scope reset without docker, for example by building a sample config the way
the script does in a scratch directory and checking `ssh -G` output for a host other than the test alias; do not run the
CentOS test. For the cutover probe, which has no test file, check the new assertion against a synthetic event list; if
the pinned tmux build (`.ci-tmux/`) is available, also run the stricter probe against it once, and a failure there would
contradict SPEC_impl.md and is a gate trip. The watcher's test drives a strict stand-in `gh` that will reject a new
commit-lookup call: extend the stand-in and add a case where main moves between the two reads. Add self-test cases for
the plans queue, recorder and changelog fixes in their existing test files.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-dev-tooling-fixes-log.md`
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
  `plan/dev-tooling-fixes/01-dev-tooling-fixes`.
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
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check the ssh change against how OpenSSH applies the
first matching value for each option, so a `Match all` line cannot itself change which settings apply to the test alias.
Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular how each assessment held up against current main, the Conventional Commit type
and changelog kind, every outcome dropped by its complexity gate or discarded as already fixed and why, every
strengthened test's mutation check or the reason there is none, and every review finding you decided not to follow. The
user will ask for these later.

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
