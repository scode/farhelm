# Queue cleanup for discarded review findings

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says.

## The goal

Execute seven settled `discard` entries in `TRIAGE_OUTCOMES.md` per `review_feedback_queue/.agents/execute-outcomes.md`
in one maintainer-approved grouped bookkeeping PR, with no code or spec edits:

1. `trailing-release-notes-recapture-parks-on-tooltip-field.md` (`discard`)
2. `provisioning-orphan-sweep-deletes-concurrent-helm-upload.md` (`discard`)
3. `release-payload-cache-misses-private-dir-hardening.md` (`discard`)
4. `podman-settle-claim-incumbent-state-non-atomic.md` (`discard`)
5. `trailing-hook-hint-keeps-stale-save-error.md` (`discard`)
6. `trailing-trust-hint-hide-survives-reconnect.md` (`discard`)
7. `trailing-relative-claude-lock-in-old-cleanup-records.md` (`discard`)

### Acceptance criteria

1. **PR 1 (`plan/triage-queue-discards/01-queue-discards`) — all 7 discard entries:**
   - Make zero product code, test, or specification edits.
   - Move all 7 entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (preserving existing frozen
     entries untouched), updating each entry's `Execution` field to record `complete` with the stable jj change ID,
     bookmark, and PR URL.
   - Remove all 7 feedback files from `review_feedback_queue/` and remove their matching lines from
     `review_feedback_queue/INDEX.md`.
   - Verify `review_feedback_queue/INDEX.md` matches the files on disk 1-to-1 and run `dprint check` on
     `TRIAGE_OUTCOMES.md` and `review_feedback_queue/INDEX.md` (and format the newly appended archive entries without
     rewrapping earlier frozen entries in `TRIAGE_OUTCOMES_ARCHIVE.md`). Do not run Rust, JS, or GUI runtime suites.

## Decisions already made

### Maintainer decisions (binding)

- **Grouped plan and single grouped PR:** Approved by the maintainer on 2026-11-25.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- Follow `AGENTS.md` ("Conventional Commits", "Tests and validation" — Markdown/bookkeeping only, no runtime suites),
  `review_feedback_queue/AGENTS.md` ("Lifecycle"), and `review_feedback_queue/.agents/execute-outcomes.md` (with the
  maintainer-approved grouped PR above).
- Keep `TRIAGE_OUTCOMES_ARCHIVE.md` append-only; do not rewrite earlier frozen records.

### Implementation outline and reviewed choices

This outline was reviewed by a fresh-context planning reviewer before finalization:

- Pure queue/ledger bookkeeping across `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`,
  `review_feedback_queue/INDEX.md`, and the 7 deleted queue files.

## Scope reassessment

Before implementing a substantial departure from the outline, run a fresh-context review at your own model and effort
with this charter verbatim:

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

Supply the request, decisions, outline, current diff, and proposed departure.

## How to run

### Checkout check

This goal runs inside the checkout the executing session runs in, which must be a checkout of GitHub repository
`scode/farhelm` (the nearest `.git` or `.jj` checkout containing the working directory). Confirm that first. If it is
not, stop before the resume protocol, write nothing (not even the log), and report the mismatch. Every project path in
this file is relative to that checkout's root.

### Resume protocol

Right after confirming the checkout, invoke the `agent-resumeable` skill with the working log at
`farhelm-plan-triage-queue-discards-log.md` in the parent directory of the executing checkout. If the log already
exists, read it and resume where the previous session left off (cross-checking against VCS state and open PRs) rather
than starting over. If it does not exist, this is a fresh start.

### Execution mode

No-workhorse mode (explicit). Do not send any unit of your own decomposition to a weaker model or lower effort than your
own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active, stop it). Same-model sub-agents at your own effort are allowed at your judgment; you
still own the `jjstack` stack and PR. The sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires"
(resume check, cold reads of Blocked questions and reports), are exempt from any delegation restriction. The per-PR
review gate keeps its demanded model and effort below.

### PR discipline

Build one clean draft PR using the `jjstack` skill, based on `main@origin` (or this plan's own open PR on resume, never
another plan's PRs), per `plans/.agents/executing.md`, "Executing one plan". Use bookmark
`plan/triage-queue-discards/01-queue-discards`. Keep the PR as a draft — never mark it ready and never merge. Never edit
files under `plans/`.

### Review gate

Before finishing the PR, run a fresh-context review of its changes using a **single fresh-context agent on `gpt-6.2-sol`
at `high` effort** (launched natively if available or through `scode-harness-shellout`, never hardcoding a launch
command here). Pass the executing checkout root, commit range or bookmark, and a private findings file in your scratch
directory, plus this full charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the supplied file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address all valid findings before moving on.

### Resource watchdog

Right after the resume protocol and before launching sub-agents, start the `scode-build-goal` skill's `scripts/watchdog`
(resolved via `realpath` next to that skill's `SKILL.md`), with status file
`farhelm-plan-triage-queue-discards-watchdog.json` beside the working log, watching the executing checkout root,
`$HOME`, `/tmp`, the log's parent directory, and your scratch directory, at a 60-second interval with default
thresholds. Follow the `scode-build-goal` skill's Resource watchdog section for harness wiring, startup verification,
acting on alerts, and stopping the monitors at close.

### Decision logging and unattended fallback

Log major choices in the working log as `DECISION`.

This plan runs unattended. Never ask the maintainer anything in conversation — no direct questions, no harness question
tool, no optional follow-up offers, and no turn ending waiting for confirmation. If a genuine fork requires a maintainer
decision, push all consistent work and block per `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking).
Things the maintainer should know go into the delivery report.

### Done criterion

The draft PR exists on GitHub, graduates all 7 `discard` entries to `TRIAGE_OUTCOMES_ARCHIVE.md` and removes their queue
files/index lines with zero code/spec edits, satisfies the latest `## Decisions` entry (if any), and has passed its
`gpt-6.2-sol` high review gate. Delivering the report is the executor's closing step (step 11 of
`plans/.agents/executing.md`), not part of the goal.

## Release note

No release note needed.
