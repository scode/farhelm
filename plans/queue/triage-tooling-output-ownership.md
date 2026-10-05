# Execute the 2026-10-05 maintainer-tooling triage outcomes: tools only delete what they made

Written against main at 3b4c9d1d on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/queue/triage-tooling-output-ownership.md` ``, exactly as root `AGENTS.md` section "Execute triage
outcomes" prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack,
in this order:

1. `recorder-frames-owner.md` (fix code)
2. `recorder-stills-owner.md` (fix code)
3. `publisher-main-pin.md` (fix code)

All three are maintainer tooling (the README demo-video recorder and the docs-screenshot publisher), none of it shipped
to users, which is why they share a plan.

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms proposed to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 3 draft PRs exist, stacked in the order above (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, as refined by the plan-time decisions below.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field.
- Every PR that changes code or tests has passed the review gate.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-05):** "now lets schedule plans for all of the fix code outcomes. do them all together i
think there were minimal questions for me. group them as makes sense into some reasonable number of plans", after a
triage session on 2026-10-05 that recorded these outcomes (landed in #1636, #1638 and #1639).

**The user's plan-time decisions (2026-10-05):**

- P1. Review gate: every PR that changes code or tests is reviewed by two fresh-context agents, one on Opus 5.5 and one
  on gpt-6-astra, both at high effort, with the general charter below. In the user's words: "opus 5.5 and astra high, no
  swarm".
- P2. The thirteen code-fix outcomes from that session are grouped into four plans; this is one of them.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

The user's triage decision, recorded in each ledger entry: "fix code" all three, as straightforward hardening of
maintainer tooling that needs no product judgement, scheduled as one plan.

**Planner proposals** are the per-item mechanisms below.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Sharing the machine with other agents; Agent scratch space; The live install is off-limits),
`plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and `.agents/test-authoring.md` for test
changes.

## Per-item outline

Line numbers drift; find the code by name. Background, verified on main at 3b4c9d1d: `scripts/readme-video.sh` takes
`--output PATH` (default `target/readme-video/readme-video.mp4`) and runs the Playwright capture in `e2e/readme-video/`,
whose `Recorder` (`e2e/readme-video/recorder.ts`) derives `<output>.frames` as frame scratch and `<base>-stills/` for
the review stills from that path, and removes both with `rmSync(..., { recursive: true })`. `docs/readme-video/SPEC.md`
describes the MP4, a marks file and a directory of stills as the outputs. `scripts/publish-docs-shots.sh` decides which
snapshot to retain past its six-week window from the manifest (`website/src/data/docs-shots.json`) in the local
checkout, and has a `--self-test` against a bare repository.

None of these changes behavior for someone running Farhelm. The type is `chore:` (no changelog fragment) unless you
judge another non-user-facing type fits better; log the DECISION.

1. **Frame scratch in a private directory.** Allocate the frame directory per run with `mkdtemp`, preferably beside the
   output rather than under `/tmp`, which may be memory-backed and small (log the choice), remove only that directory,
   including on the failure paths that remove it today, and stop deriving anything from the output path for scratch.
   Nothing user-visible about the recording changes.
2. **Stills replaced only when the recorder made them.** The stills stay at `<base>-stills/`, because that is where the
   maintainer and the refresh procedure look. Replace an existing directory only when the recorder can show it made it,
   and refuse anything else with a message naming the directory and saying to move it aside. Stills directories from
   earlier recordings, which have no marker, must keep working. The smallest design is to accept a directory whose
   entries are all files named the way the recorder names stills (`NN-<slug>.png`); a marker file written at creation is
   the other option. Pick one rather than both, and log the DECISION. If `docs/readme-video/SPEC.md` describes the
   stills directory's lifecycle, add one sentence on this rule.
3. **Retain main's pinned snapshot.** Besides the local manifest's pin, read the manifest as it is on the remote's
   `main` (fetch it with `--depth 1` into the script's private temporary repository, as the keeper is fetched today, so
   a publish does not download the whole history) and never prune the snapshot it pins. Distinguish two cases
   explicitly, so the check cannot fail open: no `main` or no manifest on it (allowed, as before the first publish), and
   a fetch that failed or a manifest that cannot be parsed (refuse to prune). What refusing means, failing the publish
   before pushing anything or publishing while keeping every existing snapshot, is your call; log the DECISION. The
   self-test's stand-in remote has no `main` today, so the new case pushes one: a local manifest pinning a different
   snapshot from main's, with main's snapshot replaced more than six weeks ago, which must survive.

### Validation

Follow root `AGENTS.md` "Finishing work". Typical choices, not a checklist:

- PRs 1 and 2: whatever type check the e2e project already has (look in `e2e/package.json` and its config), plus a
  focused exercise of the changed recorder code without staging a fleet if that is cheap. A full
  `scripts/readme-video.sh` run needs the whole build and a staged fleet; run it only if nothing narrower exercises the
  change, and say which you did.
- PR 3: `scripts/publish-docs-shots.sh --self-test` and `shellcheck scripts/publish-docs-shots.sh`, as root `AGENTS.md`
  lists for that script.
- Markdown changes: `dprint check` on the changed files.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-tooling-output-ownership-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and `TRIAGE_OUTCOMES.md` Execution fields on each PR) rather than starting over. If it does not exist, this is a fresh
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/triage-tooling-output-ownership/<nn>-<short-name>`.
- One outcome per commit, bookmark and draft PR, in the order listed in The goal. Never combine outcomes. Within this
  run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect, as each item says.
  Every `fix:` or `feat:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID
  and bookmark before creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate two independent
reviews of that PR's changes, and address what both find before moving on. The user demands exactly these reviewers, and
no review swarm:

- a fresh-context agent on Opus 5.5 at high effort;
- a fresh-context agent on gpt-6-astra at high effort, shelled out to the harness that serves that model when the
  executing one cannot reach it natively.

Both get the same prompt, carrying the full charter, because neither reviewer has anything else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a separate
findings file per reviewer (in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md`
entry, its item above, and the plan-time decisions that apply to it. For a PR that changes tests or fixtures, include
the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding,
decide on the merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill
owns launch mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new shared abstraction,
a new persisted record, a change to a protocol message, a rewrite of a flow rather than a guard in it), and whenever the
same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the ledger entry, the user decisions above, this outline, the current diff and the proposed departure
(what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular each Conventional Commit type and changelog kind, the mechanism chosen where an
item lists more than one candidate, and every review finding you decided not to follow. The user will ask for these
later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. If an item needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing
one plan, step 10). Because the PRs form one linear stack, later items sit on top of the blocked one: finish the PRs
before it, record the question, and close the plan as blocked rather than building past it. If current code or specs
have moved so that a recorded decision no longer applies, that is a question for the user too, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself.

## Done criterion

The plan is complete when all 3 draft PRs exist as one linear stack, each satisfies its ledger entry's Completion
criteria as refined by the plan-time decisions, every PR that changes code or tests passed the review gate, and each PR
has updated its own `TRIAGE_OUTCOMES.md` Execution field and removed its queue item. Open, not merged: merging happens
only after the maintainer has reviewed this plan's report. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
