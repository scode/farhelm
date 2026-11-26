# Plan: Point the landing tooling README at the agent records doc

Add one line to `scripts/plans-landing/README.md` pointing readers to `.agents/agent-records.md`, and remove the
matching entry from `TODO.md` in the same PR.

## Goal and acceptance criteria

The landing monitor tooling README (`scripts/plans-landing/README.md`) explains the landing monitor's `landing` and
`round/<N>` records and `cargo xtask records`, but never says where agent records themselves are documented. Add one
line pointing to `.agents/agent-records.md`. Keep the change to that one line (plus removing the covered TODO entry):
this is also the first plan to run on agent records, so the work is deliberately minimal.

Acceptance criteria:

- `scripts/plans-landing/README.md` gains one line directing readers to `.agents/agent-records.md` for how agent records
  work.
- No other code, documentation, or test files change beyond removing the covered entry from `TODO.md` in the same PR.
- Validation passes: `dprint check` on the changed files and `python3 scripts/check-agents-md.py`.
- The PR exists as a draft on GitHub and has passed its review gate.

## Decisions already made

- **Scope:** Keep the edit to one line in `scripts/plans-landing/README.md` plus removing the TODO entry from `TODO.md`,
  delivered as a single PR. Do not reorganize or expand `scripts/plans-landing/README.md`.
- **Delegation mode:** No-workhorse mode (required by `plans/.agents/planning.md`).
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter (settled by the
  maintainer).
- **Release note:** `No release note needed.` (settled by the maintainer; recorded in `## Release note` below).
- **Queue dependencies:** None.

## Implementation outline and requirement sources

### Requirement sources

1. **User request and settled decisions:**
   - Original TODO entry (`TODO.md`, `## Immediate`): "Point the landing tooling README at the agent records doc.
     `scripts/plans-landing/README.md` describes the landing monitor's records but never says where agent records are
     documented; add one line pointing to `.agents/agent-records.md`. This is also the first plan to run on agent
     records, so keep it to that one line."
   - Maintainer planning decisions: review gate is a single fresh-context Opus 5.5 agent at high effort with the general
     review charter; release note is `No release note needed.`.
2. **Binding repository constraints:**
   - `AGENTS.md`, "TODO.md and FLAKES.md": remove the covered TODO entry in the PR that addresses it.
   - `AGENTS.md`, "Conventional Commits" and "Changelog fragments": use Conventional Commit format (`docs: ...`); no
     changelog fragment is needed because the change has no user-facing effect.
   - `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: for a documentation-only change with no doctest
     or generation role, run only the checks that inspect the changed files (`dprint check` on the changed files and
     `python3 scripts/check-agents-md.py`), not Rust, JavaScript, Python, desktop, installer, or provisioning tests.
   - `plans/AGENTS.md`, "Editing rules", "Unattended means no questions", and "Sub-agents this file requires";
     `plans/.agents/executing.md`; `plans/.agents/outside-files.md`.
   - `scode-voice` skill for documentation prose.
3. **Planner-proposed implementation choices:**
   - Place the pointer line in the opening paragraph of `## Arguments every command shares` in
     `scripts/plans-landing/README.md` (where the `landing` record and `cargo xtask records` are first introduced), or
     directly below the opening `NOTE:` block if that reads better; either fits in one sentence without disturbing
     surrounding prose.
   - Ship everything in one `docs:` PR.

### Outline

Edit `scripts/plans-landing/README.md` in place to add one sentence pointing to `.agents/agent-records.md` (the
repository's reference for the `agent-records` layout, host files, and records tool), and delete the matching bullet
from `TODO.md`. No new mechanisms, scripts, code, or persistent state are introduced, and no existing mechanism is
duplicated or generalized. Size and uncertainty are minimal: two Markdown files, verified by `dprint check` and
`python3 scripts/check-agents-md.py`.

## Scope reassessment

No departure from the outline is expected for this change. If anything unexpected forces a substantial departure (a new
mechanism, script, or change beyond the one-line README update and TODO removal), run a fresh-context review at the
executing session's own model and effort before implementing it, supplying the original request, decisions, outline,
current diff, and proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out)
with this charter:

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

Repeated corrective reviews of the same component also require reassessing necessity before another repair round. Any
same-model sub-agent gets bounded units within the outline; unapproved new mechanisms are outside its task, and if
discovered mid-unit it leaves them unimplemented and reports the dependency. Apply the decision boundary in Unattended
fallback below.

## The checkout

This goal runs inside the checkout the executing session runs in: the nearest Git worktree or jj workspace containing
the session's working directory (a `.git` file counts as well as a `.git` directory; never fall back to an enclosing
checkout). Confirm first that this nearest checkout belongs to GitHub repository `scode/farhelm` (any remote pointing at
that repository, compared as host, owner, and name, regardless of SSH/HTTPS or `.git` suffix). If it does not, stop
before touching the working log, write nothing, and report the mismatch. Every project path in this file is relative to
that checkout's root.

## Resume protocol

Right after confirming the checkout, resolve the plan's record directory with
`cargo xtask records path plan/landing-readme-agent-records` (`plans/.agents/outside-files.md`, "Files outside the
repository"). The working log is `log.md` in that directory, with `notes/` beside it; never create a working log beside
the checkout.

Invoke the `agent-resumeable` skill with that `log.md` path. If the log file already exists, read all of it and resume
from where the previous holder left off — cross-checking the log against GitHub and VCS state per
`plans/.agents/executing.md`, "Executing one plan", step 7 — rather than starting over. If it does not exist yet, this
is a fresh start.

## Execution mode

No-workhorse mode applies for the entire run:

- Do not send any unit of the executing agent's own decomposition to a weaker model or a lower effort than its own.
- Do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point. If `scode-galaxy-brain` is
  already active when the run starts, this explicit no-workhorse requirement instructs the executing session to stop it.
- Same-model sub-agents at the executing agent's own model and effort are allowed whenever they help finish sooner
  without costing quality, at the executing agent's judgment (and may use separate worktrees or jj workspaces when
  concurrent work would otherwise conflict), while the executing agent still owns the `jjstack` stack and every PR.
- The three fresh-context, read-only native sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires"
  (the resume check and the cold reads of a Blocked question and of a report) are exempt from any delegation restriction
  and run on the executing session's own model. The review gate keeps its demanded model and effort below, and scope
  reassessment runs at the executing session's own model and effort. When a demanded model is unavailable as a native
  sub-agent type, launch it through `scode-harness-shellout`.

## PR discipline

Build the change as a linear stack (one PR here) using the `jjstack` skill, set up per `plans/.agents/executing.md`,
"Executing one plan":

- Base the stack on `main@origin` (or on the plan's own open `plan/landing-readme-agent-records/*` PRs when resuming),
  never on another plan's PRs.
- Name bookmarks `plan/landing-readme-agent-records/<nn>-<short-name>` (for example
  `plan/landing-readme-agent-records/01-readme-records-link`).
- Before every push of a plan bookmark, follow the claim check, record push, and log recording rules in
  `plans/.agents/executing.md`, "Executing one plan", step 9.
- Keep every PR a draft; never mark one ready and never merge.
- If a PR needs correcting (including one built by an earlier run), amend and restructure it in place rather than
  stacking a follow-up fix commit on top.
- Never touch anything under `plans/` in the code PR (`plans/AGENTS.md`, "Editing rules").
- Remove the covered TODO entry from `TODO.md` in the PR (`AGENTS.md`, "TODO.md and FLAKES.md"), use Conventional Commit
  format (`AGENTS.md`, "Conventional Commits"), and follow `AGENTS.md`, "Changelog fragments" (a `docs:` PR with no
  user-facing effect adds no fragment).

## Review gate

Before finishing any PR, delegate a review of its changes to a single fresh-context **Opus 5.5** agent at **high**
effort (natively if available, otherwise through `scode-harness-shellout`; do not substitute another model or effort).
Supply the full charter below, the root of the checkout the executing session runs in (filled in at run time), the
commit range or bookmark to review, and a private findings file path inside the executor's scratch directory (the
reviewer has no other context):

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code and prose for the languages involved. Report findings to the named findings
> file, most severe first, with the file, the quoted content, what is wrong, and the smallest fix. Edit nothing and
> touch no VCS state.

Address every valid finding before moving on, amending the PR cleanly rather than stacking a fixup commit.

## Resource watchdog

Right after the resume protocol and before any delegation, start the `scode-build-goal` skill's resource watchdog to
monitor disk and memory for the rest of the run (`scode-build-goal`, "Resource watchdog", with the status-file placement
override from `plans/.agents/planning.md`):

- **Script:** `scripts/watchdog` next to the `scode-build-goal` skill's `SKILL.md`, with symlinks resolved (`realpath`).
  If `deno` is unavailable, give the claim back (`queue unclaim`), close, and notify the maintainer per
  `plans/.agents/executing.md`, "Executing one plan".
- **Status file:** `farhelm-plan-landing-readme-agent-records-watchdog.json` in the parent directory of the executing
  checkout (found by walking up `$PWD` to the outermost checkout's parent, per `scode-build-goal`, "Placing the files"),
  never inside the `plan/landing-readme-agent-records` record directory.
- **Watched paths and thresholds:** Pass `--path` for the root of the checkout the executing session runs in, the home
  directory, `/tmp`, the directory holding the status file, the executing session's scratch directory, and any worktree
  or build directory later created on another filesystem. Use a 60-second interval and the script's default alert
  thresholds.
- **Verification and harness wiring:** Before starting the real monitors, verify delivery on throwaway status files in
  the scratch directory with `--interval 5` (`--synthetic-alert`, killing a sampler by its status-file `pid`, and
  `--stall-after 2` watched by an observer armed first), then stop the throwaway runs. Under Claude Code, run
  `watchdog observe --status <file>` and `watchdog sample --status <file> --path <dir>...` as paired 30-minute Monitor
  tools (observer first, kept in step, followed by `check`). Under Codex, arm both through
  `skillette-codex-wake-on-exit` (observer with `--exit-on-event` first, then sampler, then `check`). If the harness has
  neither mechanism or verification fails, log that, run the sampler as a plain background process, and rely on
  `watchdog check --status <file>` before every new launch and at natural boundaries.
- **Acting and lifecycle:** Run `watchdog check --status <file>` before every workload launch and at natural boundaries.
  On `ALERT`, `WORSE`, or a failing `check`, pause new launches and free run-owned leftovers until `check` passes; if a
  disk alert cannot be cleared, give the claim back (`queue unclaim`), close, and notify the maintainer per
  `plans/.agents/executing.md`, "Executing one plan". On `STALE`, `ERROR`, or an unexpected monitor exit, restore
  monitoring before launching again. Record the handles, status file, watched paths, and delivery mechanism in `log.md`;
  if a fresh heartbeat from an earlier session exists at startup, stop that earlier sampler before starting this
  session's own. Stop owned monitors at close (`plans/.agents/executing.md`, "Executing one plan", step 12).

## Decision logging

Log any consequential decision in `log.md` as it happens with a scannable `DECISION` label and the alternatives
considered.

## Unattended fallback

Follow `plans/AGENTS.md`, "Unattended means no questions": the executing session never asks the maintainer anything in
conversation — no direct question, no harness question tool prompt, no offer of optional follow-ups, and no waiting for
confirmation. Decide routine implementation forks within the agreed scope, log consequential choices as `DECISION` with
alternatives considered, and carry on. Remove unnecessary planner-invented obligations when the requested behavior and
repository constraints still hold.

If a material scope expansion, weakened guarantee, omitted required behavior, or unresolvable conflict arises and no
already-agreed fallback or `## Decisions` entry covers it, block per `plans/.agents/executing.md`, "Executing one plan",
step 10 (without notifying the maintainer). Anything the maintainer should know or optional follow-up ideas go into the
delivery report, not a question or block.

## Done criterion

This goal is achieved when the PR exists as a draft on GitHub, adds the one-line `.agents/agent-records.md` pointer to
`scripts/plans-landing/README.md`, removes the covered entry from `TODO.md`, passes `dprint check` on the changed files
and `python3 scripts/check-agents-md.py`, has passed its Opus 5.5 high review gate, and satisfies the latest
`## Decisions` entry (if any). Delivering the report and closing the plan are the executor's closing steps
(`plans/.agents/executing.md`, "Executing one plan", steps 11 and 12), not part of the goal.

## Release note

No release note needed.
