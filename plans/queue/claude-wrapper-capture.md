# Fix: capture Claude conversations through wrappers at any depth, as Codex does

Written against main at 36c8d7b0 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Consider matching Claude's conversation capture to Codex's.** Codex's conversation capture changed recently;
> consider adjusting how Farhelm captures a Claude session's conversation to match. The maintainer knows which change
> this refers to and what matching it would mean; ask before starting.

The maintainer answered while planning, so this is no longer an open "ask before starting": "In the last couple of weeks
or so we landed a fix so that if `codex` is actually a wrapper that doesn't exec but runs real codex as a child,
conversation capture still works. We should do the equivalent for Claude." And, once told the literal case already works
for Claude: "Nothing has failed, I just felt the eventual solution we eventually got with codex made sense and I wanted
consistency in claude if there wasn't a strong reason not to."

NOTE: Describe wrapper environments only in generic terms, in code, tests, docs, commit messages and the report, as the
Codex plan required. No private tool, host or product names.

### The Codex change this matches

4423eda1, "fix: capture codex conversations through launchers (#1945)". The plan it executed is
`git show d2f4508e:plans/queue/codex-resident-launcher.md`. In short (current code, line numbers approximate):

- **Attribution** (`crates/farhelm-supervisor/src/procs/codex.rs` around 10-75): the emitter is the innermost process
  whose image is `codex`. Between the reporter (the hook invocation) and the emitter only narrow `sh -c` hook
  trampolines are allowed. Everything above the emitter is accepted as launch ancestry, and the `codex` images above it
  are returned as ancestors.
- **The pin** (`agent_kind/codex.rs` `CodexReportPin`/`PinDecision::judge` around 14-71; `service/core/vendor/codex.rs`
  `report_codex_conversation` around 63-312; the nullable `codex_report_pin` column, schema 31, in `store.rs`; the CAS
  writer `Store::admit_codex_conversation`). The first admitted report pins the emitter by pid and start token. Later,
  only the pinned process may report. A recorded ancestor that reports corrects the pin and the conversation and posts a
  once-per-launch notification (`NotificationKind::CodexPinCorrected`). A dead pin is released for a process whose chain
  does not contain it. Unavailable process evidence never releases a pin. `begin_relaunch` clears the pin. Running
  sessions start unpinned after the upgrade, which was accepted.
- Codex corrects quickly because Farhelm injects both `SessionStart` and `Stop` hooks into Codex, and an ancestor's
  `Stop` can select.
- Spec text: SPEC.md around 2248 (Codex reports) and 1777 (notifications); SPEC_impl.md "Codex attribution and
  exact-record validation" (around 2851-2867), the shared framework sentence (around 2956) and the notifications list
  (around 3996). Tests at four layers: corridor (`procs.rs`), admission (`service/core.rs` `codex_pin_*`), store
  (`codex_pin_*`), end to end (`crates/farhelm/tests/e2e/codex_identity.rs`), plus a resident-child mode in the fake
  Codex fixture.

### Claude today

- Farhelm injects one `SessionStart` hook through `--settings` (`agent_kind/mod.rs` `hook_argv` around 1250-1273).
  Claude fires it at start, resume, `/clear` and compact. Reports use the single `latest.json` slot.
- Admission is the legacy path: `report_conversation_legacy` (`service/core.rs` around 15480-15561) runs
  `attribute_claude_report` (`service/core/vendor/claude.rs`) **before** the capture claim, then writes with
  `record_reported_conversation`, fenced on generation only. Claude's `capture_ownership_version` stays 0 on purpose
  (SPEC_impl.md "Interim ownership states", around 2997-3011): moving it would withdraw existing Restart offers.
- The rule, `claude_corridor` (`procs/claude.rs` around 38-72), is positional and recognizes no executable (Claude's
  native binary is named after its version, and npm installs run under `node`). The process that ran the hook must be
  the pane process or its direct child. So one resident wrapper is tolerated, including a launcher named `claude`, and
  the literal Codex case already works. Two or more resident levels lose capture (test
  `claude_refuses_a_hook_under_a_two_level_wrapper` in `procs.rs`). One real case: a resident wrapper prefix in front of
  a launcher that also stays running.
- What the depth limit protects: a `claude` started under the foreground Claude (through its Bash tool's shell, which
  does not exec) cannot replace the foreground's conversation. The pin preserves that property at any depth. A known
  hole closes along the way: a child the foreground starts directly, with no shell between, is admitted today.
- Nothing else depends on depth. Exit follows the pane process, status comes from the screen, the stop sweeps use
  process-group closure and environment markers, and resume runs the stored command, wrapper prefix included.

### Acceptance criteria

- **Attribution at any depth.** Claude's emitter is the first non-trampoline link above the hook invocation, as today,
  but at any depth below the pane. Every link above the emitter is recorded as an ancestor, because Claude cannot be
  recognized by name. A Claude two or more resident wrapper levels below the pane captures its conversation and resumes.
- **The Codex pin model for Claude.** The first admitted selecting report pins the emitter (pid and start token).
  Afterwards only the pinned process may change the conversation. A recorded ancestor that reports takes the pin back,
  switches the session to its own conversation, and posts a once-per-launch notification. A dead pin is released for a
  process whose chain does not contain it. Unavailable process evidence never releases a pin. Relaunch clears the pin.
  The pin is read and written under the capture claim, with the conversation and notification in one CAS transaction, as
  Codex's is.
- **Claude gets an end-of-turn hook.** Farhelm's injected `--settings` also carries a `Stop` hook, so a wrongly pinned
  child is corrected at the parent's next end of turn, not only at its next start, resume, clear or compact. Like
  Codex's Stop, it carries no instructions announcement. A Claude Stop is an ordinary report judged by the same pin as
  `SessionStart`: from the pinned process it is accepted (it names the conversation already selected, or the current
  one), from a recorded ancestor it corrects, and from anyone else it is refused **silently**, acknowledged without a
  warning, so a nested Claude does not log one warning per turn. Claude keeps its single `latest.json` report slot.
  Codex needs two slots only because its Stop confirms a pending `/clear` and is refused without its `SessionStart`;
  Claude has no transcript check and nothing to confirm, and since every report carries the current conversation, the
  newest report winning the slot is correct. Before relying on any claim about what real Claude sends on Stop, observe
  it with an installed Claude per `.agents/real-agent-tui.md`: that the payload carries the current conversation id,
  whether sub-agents fire `Stop` or only `SubagentStop`, whether `/clear` fires it, and whether Claude draws visible
  "running stop hook" text that could confuse Farhelm's status reading of the screen. If Stop turns out not to carry the
  current conversation, that is a departure: run the scope reassessment before adding a second slot or other machinery.
  The maintainer accepted the real turns these checks cost when choosing the hook.
- **Claude stays at capture ownership version 0**, so no existing Restart offer is withdrawn.
- **Upgrade:** running Claude sessions start unpinned and keep their old argv without the Stop hook until relaunch, the
  same accepted gap Codex had. No compensating machinery.
- **Tests at the same layers as #1945:** corridor (rewrite `claude_refuses_a_hook_under_a_two_level_wrapper` into an
  acceptance test, and keep the shelled-out-child cases as refusals, now at admission); admission (pin accept, refuse a
  rival, ancestor correction with its notice once, dead-pin release, a pinned Stop accepted, an ancestor's Stop
  correcting, a rival's Stop refused silently); store (CAS with Claude's ownership version staying 0, relaunch clear,
  and a migration test only if a new column is added); end to end in the Claude identity tests (a two-level resident
  wrapper captures and resumes; a child-first capture is corrected by the parent's Stop and resumes the parent's
  conversation; `a_shelled_out_child_cannot_replace_the_foreground_report` keeps passing, with its expected reason
  updated). Extend the fake Claude fixture only where the existing explicit report typing cannot express a case.
- **Docs:** SPEC.md's Claude reports paragraph (around 2278-2286, including "a wrapper chain deeper than that loses hook
  capture") and the notifications list (around 1777); SPEC_impl.md "Interim ownership states" (around 2997-3011), the
  shared framework sentence (around 2956), the per-launch hook text (around 3018-3058: Claude now gets Stop too), and
  the notifications list (around 3996); the website pages `agents/claude.md`, `agents/agent-wrappers.md` and
  `how-it-works/agent-hook-injection.md` (read `website/AGENTS.md` first); the doc comments in `procs/claude.rs`,
  `vendor/claude.rs` and the legacy admission path.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Do it for consistency with Codex.** No Claude failure has been observed. The planning page stated the one cost, and
  the maintainer accepted it: if the parent's report is lost (hook failure, supervisor outage, sessions spanning the
  upgrade), a nested Claude that reports first is pinned until the parent corrects it, where today it is refused. A
  nested Claude only reports at all if it carries Farhelm's hook, which Farhelm adds to the outer agent's command line
  only, so this is rare.
- **The Codex model:** pin at any depth, ancestor correction with a notice, dead-pin release, relaunch clears.
- **Add the end-of-turn (`Stop`) hook to Claude**, chosen over correcting only at the next start, resume, clear or
  compact, even though it adds hook handling and real-agent checks.
- **Identify Claude by position, not by executable, argv or environment**, keeping the reasons recorded in
  `procs/claude.rs` and SPEC_impl.md.
- **Keep Claude at capture ownership version 0.**
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the maintainer's words quoted above, and the decisions above.

**Binding repository constraints:** the SPEC.md and SPEC_impl.md sections named under Docs above; root `AGENTS.md`,
"Code rules" (read the harness map in the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs` first;
per-harness behavior goes where it says, never `kind == X` or matches over harnesses in shared code);
`.agents/real-agent-tui.md`; `website/AGENTS.md` for the docs pages; root `AGENTS.md`, "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Storage: reuse the existing pin column for Claude. A session has exactly one agent kind, relaunch already clears the
  column, and the pin value (pid and start token) is not Codex-specific. Generalize the pin type and the CAS writer so
  the writer takes the ownership version as an input (Codex passes 1, Claude 0), in a no-behavior-change refactor PR
  first, keeping per-harness decisions in the places the harness map names. Whether to rename the column (a migration)
  or keep its name and document that it serves both is yours; log it.
- Notification: a new `ClaudePinCorrected` kind beside `CodexPinCorrected`, with the same once-per-launch flags. That is
  smaller than generalizing the existing kind and leaves its stored value alone.
- Claude's admission leaves the generic legacy write and gets its own function under the capture claim, modeled on
  `report_codex_conversation`, but with no transcript verification: Claude has none today, and none is added.
- Put the changelog fragment in whichever PR first changes user-visible behavior, and amend it as later PRs land.

## Outline

1. `plan/claude-wrapper-capture/01-shared-report-pin`: generalize the pin type and CAS writer (ownership version as an
   input), no behavior change.
2. `plan/claude-wrapper-capture/02-claude-pin`: attribution at any depth, the Claude pin, admission under the capture
   claim, the correction notification, the tests at all four layers, and SPEC, SPEC_impl and docs pages.
3. `plan/claude-wrapper-capture/03-claude-stop-hook`: the injected Stop hook, Stop judged by the pin with silent
   refusals, the real-Claude observations, their tests, the docs that describe injection, and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
the crates you touch, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares,
and, through `python3 scripts/record-test-run.py`, the supervisor's corridor, admission and store tests for Claude and
Codex (a shared pin touches Codex), the hook binary's tests (`crates/farhelm/src/hook.rs`), and the Claude and Codex
identity and wrapper end-to-end tests. Read the retained output: an end-to-end test that returned early on missing
systemd is not evidence.

## Fallbacks

Examples of departures that need the scope reassessment: recognizing Claude by executable, argv or environment;
transcript verification for Claude; moving Claude's ownership version; an upgrade-window guard; more hooks than Stop; a
second report slot for Claude; changing Codex's behavior beyond a no-behavior-change refactor.

Decisions to log: how the pin column is named and shared, what the real-Claude observations showed (version, command
line, quoted screen or payload facts), how Stop reports are judged, every test changed and why, and every review finding
you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-claude-wrapper-capture-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/claude-wrapper-capture/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
