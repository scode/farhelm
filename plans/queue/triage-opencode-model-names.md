# Execute the 2026-10-02 OpenCode model-name triage outcomes: bare names work, typing never switches harness

Written against main at 9e1dde03 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/queue/triage-opencode-model-names.md` ``, exactly as root `AGENTS.md` section "Execute triage
outcomes" prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack,
in this order:

1. `opencode-bare-model-rejected.md` (fix spec+code)
2. `opencode-bare-model-switches-harness.md` (other: closed by PR 1's change; bookkeeping only)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms proposed to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 2 draft PRs exist, stacked in the order above (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, as refined by the plan-time decisions below.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field.
- PR 1 has passed the review gate.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "now use the planning system to plan execution of what we triaged", after a triage session on
2026-10-02 that recorded these outcomes (landed in #1475).

**The user's triage decisions (2026-10-02),** recorded in full in each ledger entry's Decision field:

- "choosing a model name should not switch harness unless it is 100% special case and its only possibly available in one
  harness, but i can't think of such a caes. so no, dont make users type opencode/".
- An explicit pick of a harness-labelled row in the model list's "Show all" view still switches the harness.
- With no harness selected yet, a bare name such as `gpt-6-luna` picks Codex, "because it's the 'primary' harness. no
  separate asking dialog." (This is today's behavior for that case; keep it.)

**The user's plan-time decisions (2026-10-02):**

- P1. Review gate: one fresh-context Opus 5.5 reviewer at high effort for PR 1, with the general charter below. PR 2 is
  bookkeeping only and gets no review.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the per-item mechanisms below.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live
install is off-limits), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

## Per-item outline

Line numbers drift; find the code by name. Background, verified on main at 9e1dde03: every model lookup compares the
typed text with the launch catalog exactly as typed. The bare entries `gpt-6-luna`, `gpt-5.6-terra`, `gpt-6.1-sol` and
`gpt-6-astra` (and others, such as `claude-fable-5`) belong to other harnesses in the catalog
(`crates/farhelm-helm/src/launches.rs`), while OpenCode's entries carry the `opencode/` prefix. So:

- `validate_selection` (`launches.rs`) refuses an OpenCode launch of a bare name as belonging to a different harness,
  although `opencode_model_argument` would qualify it correctly; the existing bare-name test uses a name with no
  competing entry.
- In the new-session form, `model_enter_target` (`crates/farhelm-ui/src/launch_composer.rs`) keeps the selected harness
  only if it owns the exact typed id; otherwise `apply_model_option` (`crates/farhelm-ui/src/list/create_form.rs`)
  switches harness. A test asserts that typing `CLAUDE-FAST` with Codex selected switches to Claude.
- The browser's `selection_is_compatible` and `compatible_efforts` and the custom-model history filter compare raw ids
  too, so a UI-only fix would still disable Launch, and a server-only fix would still switch harness on Enter.

1. **Bare names resolve inside the selected harness; Enter never switches a selected harness.** `fix:`.
   - Proposal: one exhaustive per-harness method giving a typed model's catalog form: OpenCode turns a bare name into
     `opencode/<name>`, every other harness returns the name unchanged. Put it where root `AGENTS.md` (Harness-specific
     code) and the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs` say per-harness launch choices live;
     `LaunchHarness` in `crates/farhelm-proto/src/launch.rs` is the likely home, since both the helm and the UI need it.
     No `kind == OpenCode` checks in shared code.
   - Use it in the helm's `validate_selection` catalog lookup and effort check (the bare-name half of
     `opencode_model_argument` can reuse it; its provider refusal stays) and in the UI's `model_enter_target`,
     `selection_is_compatible`, `compatible_efforts` and history filter. The user's typed text stays the stored
     selection.
   - In `model_enter_target`, when a harness is already selected, Enter never switches it. A typed model that only
     another harness offers keeps the harness and shows an inline error naming the harness that offers it; it does not
     become a custom model, since both the browser check and the server would refuse the combination. Leave the
     no-harness-selected behavior and the "Show all" row picks as they are.
   - Spec: SPEC.md's "A known model identifies its owning harness" is narrowed to filling a harness that is not yet
     selected, and the bare-name contract for OpenCode is stated plainly if it is not already.
   - Tests: reverse the `CLAUDE-FAST` switch test; both spellings of each overlapping name under OpenCode on the server
     and on Enter; a model only another harness offers, with a harness selected, shows the error and keeps the harness.
   - Changelog fragment: `kind: fixed` (OpenCode accepts model names without the `opencode/` prefix, and typing a model
     name no longer silently changes the selected harness).
2. **Bookkeeping.** `docs:`. Confirm PR 1 fixed the reported switch, then do the queue and ledger bookkeeping; nothing
   else.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-opencode-model-names-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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
  `plan/triage-opencode-model-names/<nn>-<short-name>`.
- One outcome per commit, bookmark and draft PR, in the order listed in The goal. Never combine outcomes. Within this
  run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect, as each item below
  says. Every `fix:` or `feat:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID
  and bookmark before creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- PR 1: `cargo fmt --all -- --check`, clippy on the changed crates, focused nextest selections for the helm's `launches`
  tests and the UI's `launch_composer` and `create_form` tests, and the Playwright spec that drives the new-session
  form's model field, if one exists, on Chromium and WebKit through the recorder after the builds root `AGENTS.md`
  names.
- PR 2: Markdown only, so `dprint check` on the changed files.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.
- A PR that only changes Markdown: `dprint check` on the changed files, nothing else.

### Review gate

Before finishing PR 1, use the active galaxy-brain skill to delegate a review of that PR's changes. The user demands the
reviewer: a fresh-context agent on Opus 5.5 at high effort, with the general charter below. No review swarm. The prompt
carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md` entry, its item above, and
the plan-time decisions that apply to it. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Address what the reviewer finds before moving on;
where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a model-alias table, changes to the catalog's
spelling, a confirmation dialog, changes to the "Show all" view; these are examples, not a blacklist), and whenever the
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
alternatives considered: in particular where the per-harness method lives and its name, the inline error wording, and
which UI checks needed it, each changelog kind, and every review finding you decided not to follow. The user will ask
for these later.

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

The plan is complete when all 2 draft PRs exist as one linear stack, each satisfies its ledger entry's Completion
criteria as refined by the plan-time decisions, PR 1 passed the review gate, and each PR has updated its own
`TRIAGE_OUTCOMES.md` Execution field and removed its queue item. Open, not merged: merging happens only after the
maintainer has reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
