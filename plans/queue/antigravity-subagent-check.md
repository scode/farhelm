# Keep Antigravity's Restart on the main conversation

Written against main at 0055c6beb on 2026-11-19. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

The TODO.md entry (bucket "Plan decisions to revisit"), verbatim:

> **Revisit accepting subagent reports for Antigravity's conversation tracking.** Decided on the maintainer's behalf by
> the landing monitor on 2026-11-19, when antigravity-support blocked: in Antigravity CLI 1.5.12 a read-only subagent
> sends SessionStart and Stop with its own conversation ID from the same process, so Restart can resume the subagent's
> conversation. The plan keeps automatic tracking and plain Restart and accepts that risk as the Goose and Pi exceptions
> do, documented on the Antigravity docs page. The alternative was shipping launch support and explicit conversation
> selection only, with automatic tracking and plain Restart unavailable until Farhelm can tell the main conversation
> from a subagent's.

The maintainer chose to keep automatic tracking and try to close the gap: run a short live check of Antigravity's
behavior, and if a signal reliably tells a subagent's report from the main conversation's at the moment the report
arrives, refuse subagent reports so Restart always follows the main conversation. Whatever the check finds, improve the
Antigravity docs.

### Where it stands (from planning, at main 0055c6beb)

What ships (v0.42.0): Antigravity's hook adapter (`parse_antigravity_payload` in `crates/farhelm/src/hook.rs`) keeps
only `conversationId` and `hookEventName` and sets no child marker, so the shared refusal of child-marked reports never
fires for it. Admission reuses the Claude/Muse positional path
(`crates/farhelm-supervisor/src/service/core/vendor/positional.rs` and `procs/positional.rs`); its pin identifies a
process, so any report from the pinned `agy` process may move the captured conversation, and its doc comment says it
cannot tell unmarked conversations in that process apart. A Stop is an ordinary selecting report, so the main
conversation takes Restart back at its next Stop. The exception is written down in SPEC.md (beside the Goose and Pi
exceptions, in the conversation-tracking rules), SPEC_impl.md, the module docs of
`crates/farhelm-supervisor/src/agent_kind/mod.rs`, the docs page `website/src/content/docs/docs/agents/antigravity.md`,
the supported-agents table's tooltip (`website/src/content/docs/docs/agents/index.mdx`) and
`website/src/content/docs/docs/using/stop-restart-resume.mdx`.

What goes wrong: from a subagent's first report until the main conversation's next Stop (seconds to minutes), the
captured conversation is the subagent's. If the agent stops in that window (Restart on a working session, quitting or
interrupting `agy` mid-turn, a crash, a host reboot or update), Restart opens the subagent's conversation, silently, and
that becomes the session's conversation from then on. The workaround is Restart with (or Replace with) and the main
conversation's ID, but neither Farhelm nor the docs say where Antigravity shows that ID.

What the earlier executor measured, live, with Antigravity CLI 1.5.12 in an isolated workspace with workspace hooks (its
working log, `farhelm-plan-antigravity-support-log.md`, in the parent directory of the checkouts; read it, it is outside
the repository): no SessionStart at the idle prompt; the first prompt produces SessionStart, every turn ends with Stop;
root and child payloads have identical key sets (SessionStart: `artifactDirectoryPath`, `conversationId`, `cwd`,
`hookEventName`, `lastUserInput`, `modelName`, `transcriptPath`, `workspacePaths`; Stop adds `continuationActive`,
`error`, `executionNum`, `fullyIdle`, `terminationReason`); every hook's parent was the same `agy` process. In a fresh
run the order was root SessionStart, child SessionStart, child Stop, root Stop; in a resumed run
(`--conversation <root> --prompt-interactive`) it was child SessionStart, root Stop, child Stop, root Stop, with no root
SessionStart. So ordering rules such as "keep the first conversation" do not work.

What planning found since, by reading Antigravity's saved state for those probe conversations read-only (under the
account's Antigravity CLI state directory; two root/child pairs, one CLI version):

| Signal                                                                      | Roots                                 | Children                                   |
| --------------------------------------------------------------------------- | ------------------------------------- | ------------------------------------------ |
| First record of the conversation's `transcript.jsonl` (under `brain/<id>/`) | `source: USER_EXPLICIT`, `USER_INPUT` | `source: SYSTEM`, `type: SYSTEM_MESSAGE`   |
| `brain/<root>/.system_generated/subagents/<child>.json`                     | n/a                                   | present under the parent, before the child |
| Conversation index row (`conversation_summaries.db`)                        | no parent, depth 0                    | the parent's id, depth 1                   |

Nobody has checked whether `transcriptPath` names that file, or whether the first record (or the subagent file) is
written by the time the hook fires. SPEC.md already allows checking the exact file a report names as verification of
that report (not identification), and Codex already verifies its exact reported transcript's root-session metadata
because process ancestry cannot tell threads in one process apart; that is the precedent to follow.

Related, and out of scope: `review_feedback_queue/antigravity-restart-may-stop-without-relaunching.md` (a conversation
report arriving during Restart can leave the agent stopped). The maintainer chose to leave it to triage. Do not fix it
here; mention in the report whether this plan's outcome makes it more or less likely.

### Acceptance criteria

1. A live check, recorded in the working log as redacted observations (booleans, enum values, numbers, relative path
   shapes, never conversation content), settles four launch shapes: a fresh launch with a typed first prompt; a fresh
   launch started the way Farhelm starts a prompted launch (`--prompt-interactive=<prompt>`, with the flags Farhelm's
   compiler adds, such as `--mode auto` and the model flag; see `crates/farhelm-proto/src/launch.rs` and
   `crates/farhelm-helm/src/launches.rs`); a resumed launch (`--conversation <id> --prompt-interactive=<prompt>`); and
   an in-place new-conversation command, if `agy` has one. For each, at the moment each SessionStart and Stop hook runs,
   for the root and for a subagent: where `transcriptPath` points; whether that file exists; its first record's `source`
   and `type` and its first line's byte length; whether the subagent file under the parent exists; and the Stop-only
   payload values.
2. If the transcript test holds at report time for every observed child report and never refuses an observed root report
   that Restart needs (in all four shapes), Antigravity reports whose named transcript does not begin with a user-input
   record are refused, so the captured conversation never moves to a subagent's. A report whose transcript is missing or
   unreadable fails closed (refused); a root refused that way is recovered by its own later Stop, and the report says
   how often that happened in the live runs. A first line longer than the shared reader's bound (64 KiB,
   `RECORD_PREFIX_BYTES`) cannot be parsed and is refused: state that limit in the spec and docs text and cover it with
   a test; do not add partial-JSON scanning to rescue it. If the transcript test races but the subagent-file test holds,
   use that instead, with this polarity: refuse a report only on positive evidence, that is when
   `subagents/<reported>.json` exists under the currently captured conversation's folder; admit otherwise. It cannot
   help when nothing is captured yet or for a nested child whose parent is not the captured conversation; say so in the
   spec text, and say why checking that exact path, built from the two ids rather than named by the report, is still
   verification rather than identification. Either check reads only that one exact file, bounded, and never scans or
   correlates Antigravity's state.
3. If such a fix lands: the Antigravity exception is removed, and replaced by a statement of the check, its fail-closed
   rule, the 64 KiB limit and the CLI version it was verified against, everywhere it is written: SPEC.md (the
   Antigravity exception beside Goose and Pi; the admission sentence saying Claude, Muse and Antigravity need no
   versioned proof; the "Agent files" sentence listing which vendor files Farhelm reads), SPEC_impl.md (including
   "Farhelm uses no vendor record lookup here"), the `agent_kind` module docs and `ownership_proof_implemented`'s doc,
   the `ANTIGRAVITY` policy doc and `report_positional_conversation`'s doc in `vendor/positional.rs`,
   `parse_antigravity_payload`'s doc, the Antigravity docs page, the supported-agents tooltip and the stop/restart docs.
   The statement also says, as an accepted limit, that a later `agy` that changes its transcript format would make every
   report refused without a warning, leaving the capture where it was. If the live check shows the root's first report
   routinely refused and recovered only at its Stop, the "Restart becomes available after your first prompt" wording
   (the docs page, `AgentKind::restart_readiness`, SPEC.md's list of agents that report at the first prompt) is
   corrected to match. Tests cover a child report refused, a root report admitted, a missing transcript refused and an
   oversized first record refused, through the real hook adapter and admission path with fixture transcripts.
4. If no signal holds: today's behavior stays, and the findings go into the report and the working log.
5. Either way, the Antigravity docs page says where Antigravity shows a conversation's ID (found from `agy --help`, its
   in-app help, or its documentation, which costs no model turn; if none of those says, the conversation file names in
   its state directory are the fallback, stated as observed rather than documented), and, while the exception remains,
   that stopping or restarting while a subagent runs is what triggers it.
6. The last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning page:

- **What to do:** check, then fix (not docs only, not turning off automatic Restart).
- **If no signal holds:** keep today's behavior, with the docs improvements, and report the findings. Do not turn off
  automatic Restart and do not block for this.
- **Live check budget:** the executor may spend real Antigravity prompts, up to about five, under the account on the
  machine the plan runs on, each asking for one small read-only subagent. Linux only; whether a Mac runs hooks through
  zsh (which would weaken the process pin, see the landing notes in `plans/reports/antigravity-support.report.md` if it
  is still on main, or the plan's landing commit otherwise) stays a follow-up for the report.
- **Related review item:** leave `antigravity-restart-may-stop-without-relaunching.md` to triage.
- **Release note:** "No release note needed." (no highlight; the fragment still describes the change for the full list).
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the picks above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Code rules": every per-harness decision lives in the named places mapped in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` (read that map before coding); no `kind == X`, `matches!` over
  harnesses, or `_` arm over harnesses in shared code.
- SPEC.md's conversation-tracking rules: verification of the exact file a report names is allowed; correlating vendor
  files to identify a conversation is not.
- `crates/farhelm-proto/src/lib.rs`: `PROTOCOL_VERSION` bumps on any new wire variant or launch field. This plan should
  need none; if it does, bump it and say so in the fragment and report.
- `.agents/real-agent-tui.md` before relying on a claim about how `agy` behaves; root `AGENTS.md` "Conventional
  Commits", "Changelog fragments", "Tests and validation"; `.agents/validation.md`; `.agents/test-authoring.md`;
  `website/AGENTS.md` for docs edits.
- "The live install is off-limits" (root `AGENTS.md`): the live check runs `agy` only, with workspace hooks in a scratch
  workspace, never against the maintainer's own Farhelm, and never edits the account's global Antigravity configuration.

**Planner proposals** (verified against the repository and reviewed by an independent planning reviewer):

- **Probe first, in a scratch workspace with a recording hook** adapted from the earlier executor's (its working log
  says where its probe files were; if they are gone, write a small one), recording per callback the redacted facts in
  acceptance criterion 1. Spend the budget on the four shapes there, one prompt each, with the fifth held for a rerun.
  Before the first run, check whether the account's global Antigravity hooks name Farhelm; either way, run the probe
  `agy` with Farhelm's session variables (the `FARHELM_*` session id, agent id, spawn token and supervisor socket)
  removed from its environment, so a global Farhelm hook cannot write a report into the session the executor itself runs
  in.
- **One per-kind answer on the existing positional admission, not a fork of it.** Add a field to the
  `PositionalReportPolicy` constants in `vendor/positional.rs` (each literal states it: Claude and Muse "no record
  check", Antigravity names its check), and call it after the pin decision and before `admit_pinned_conversation`, under
  the capture claim as Codex's check runs. Do not copy `report_positional_conversation` into a `vendor/antigravity.rs`
  the way Codex has its own path. A refused child Stop is acknowledged silently, as a rival Stop already is, so subagent
  turns do not log a conflict on every turn. The Antigravity-specific reading of the first record goes in a small
  function in `agent_kind/antigravity.rs` (the map's place for per-kind vendor-file verification), with
  `parse_antigravity_payload` keeping `transcriptPath`.
- **Reuse the shared bounded first-line reader** (`agent_kind::read_record_header`, which Codex and Grok use), not
  `read_record` / `AgentIntegration::parse_record`, whose correlator contract answers a different question;
  `AntigravityIntegration::parse_record` stays as it is.
- **Restart gating stays as it is.** No locator, no ownership or proof version, no Restart-time recheck: a
  conversation's root-ness is fixed once its first record exists, so checking at admission is enough. Needing any of
  these is a departure for the scope reassessment.

## Outline

A linear stack on `main@origin`, at most two PRs:

1. `plan/antigravity-subagent-check/01-refuse-subagent-reports` (`fix:`), only if a signal holds: the adapter change,
   the admission check, its tests, the spec and code-doc updates that remove the exception, and the changelog fragment
   (`kind: fixed`).
2. `plan/antigravity-subagent-check/02-antigravity-docs` (`docs:`): the docs page (where to find a conversation ID; the
   exception's trigger while it remains, or the check that replaced it), the tooltip and stop/restart docs as the
   outcome requires, and the TODO.md entry removed. If PR 1 does not exist, this is the only PR, based on `main@origin`.

## Fallbacks

- If the live check cannot run (no `agy` on the executing host, or its login has expired), block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10): the decision to check first is the maintainer's, and
  only they can log the CLI in. Do not fall back to planning from saved state alone.
- If both signals hold, prefer the transcript test (it checks the file the report names); log it.
- If a signal holds for children but refuses root reports that Restart needs (a root whose transcript is never flushed
  before its Stop, or a prompted launch whose first record is not user input, say), it does not hold: treat it as no
  signal.
- Do not add a notification, version gate or other machinery for a future `agy` format change; state it as an accepted
  limit (acceptance criterion 3).
- If the installed CLI is not 1.5.12, run the check on what is installed and record its version in the spec text.

Decisions to log: each live observation that settles a signal, the signal chosen or the finding that none holds, and
every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-antigravity-subagent-check-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-antigravity-subagent-check-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files, the probe workspace
included, go in the agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. The live
Antigravity runs are probes, not tests: report them as such, with no recorder id.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work. A resumed run reuses live observations already in the log rather than
spending the prompt budget again.

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
  `plan/antigravity-subagent-check/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it (a user-visible fix is `fixed`, not `none`, even when the release note answer is "No
  release note needed."). Antigravity support shipped in v0.42.0, so a fix here is a fix to released work, not an amend
  of an unreleased fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- Public-repo hygiene (`plans/AGENTS.md`): no absolute paths, account names or conversation content in PRs, fragments,
  docs or the report.
- The last PR of the stack removes this plan's TODO.md entry.

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
and outline), plus the redacted live observations the PR relies on. When a PR changes tests or their fixtures, include
the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

The plan is complete when a linear stack of draft PRs exists, one per step the outline gives for the outcome the live
check settled, together meeting the acceptance criteria above, each having passed its review gate, and the last PR
removing this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report (with the live findings, the outcome, and whether the related review item became more or less likely) through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
