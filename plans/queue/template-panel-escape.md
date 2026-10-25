# UI: Escape in the save-as-template panel closes only the panel

Written against main at aa245430 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The session launcher's "save as template" panel closes on Escape and returns to the launcher (SPEC.md around 729-741,
"Escape returns to the launcher"). That breaks once keyboard focus is lost while the panel is open, as happens after a
refused save. The TODO.md entry (bucket "Near term"), verbatim:

> **Escape in the launcher's save-as-template panel.** With the panel open but keyboard focus lost (for example after a
> refused save), Escape closes the whole launcher and discards its draft instead of closing just the panel. Complexity
> gate: fix it only if the fix is simple; otherwise leave it. From the review of `modal-dialog-shell` on 2026-10-23.

The cause, from a read of main at aa245430 (line numbers approximate):

1. **Focus is lost.** An asynchronous save natively disables the panel's controls (`SaveTemplatePanel`,
   `crates/farhelm-ui/src/list/save_template.rs` around 179-305; `saving` set around 213, the controls `disabled` while
   saving). SPEC_impl.md (around 625) records that a focused control that is natively disabled drops focus to `body`.
   When the save is refused (a taken name around 222-226, a refused PUT around 232-235), the controls come back but
   nothing returns focus to them.
2. **Escape goes to the wrong Cancel.** With focus on `body`, Escape reaches neither the panel's own handler nor the
   launcher's `on_escape` (`crates/farhelm-ui/src/list/create_form.rs` around 5918-5923, which already closes the panel
   first). The modal isolation safety net (`crates/farhelm-ui/src/modal_isolation.rs`, `ISOLATE_TEMPLATE` around
   196-209) catches it and clicks the dialog's `escape_target`, which is the launcher's `.launch-composer-cancel`
   (`create_form.rs` around 5866). That button closes the whole launcher.

How sure the planner is: high on the routing; medium-high that focus lands on `body` in each engine (WebKit may not
focus a clicked button at all). The existing taken-name test re-focuses the field by hand before pressing Escape, which
hides the bug.

### Acceptance criteria

- **Escape routing.** While the save-as-template panel is open, an Escape that the isolation safety net handles (focus
  on `body` or otherwise outside the dialog) closes only the panel, exactly as the panel's own Cancel does, and leaves
  the launcher and its draft intact. With the panel closed, that Escape still closes the launcher as today. During an
  in-flight save it stays a no-op, as today.
- **Focus after a refused save.** After an asynchronous refusal re-enables the panel's controls, focus returns to the
  panel's name field, but only when focus is actually lost (on `body`, the document element, the dialog root, or a
  disabled control), so a launcher control the user clicked during the save keeps its focus.
- This holds in Restart with as well, which shows the same panel.
- Browser tests, in `e2e/tests/templates.spec.ts` unless another spec fits better, on both browser engines:
  - The taken-name test (`launcher template save refuses a taken name and Escape keeps the launcher`, around 1903) drops
    its manual `.focus()`, asserts focus is on the name field after the refusal, presses Escape, and asserts the panel
    closed while the launcher and its draft remain. Cover submission by Enter in the name field, the documented focus
    loss; a Save click as well if it does not double the test's cost.
  - A new case: with the panel open, focus moved to `body` (`blurToBody` in `e2e/tests/restart-with.spec.ts` around 849
    shows how; share or copy it), Escape closes only the panel, and a second Escape then closes the launcher.
- Re-run on both engines the tests that pin the neighbouring rules:
  `restart with pulls stray focus back and still cancels on Escape` and the held-request body-Escape case in
  `restart-with.spec.ts`, and the composing-Escape test in `templates.spec.ts` (around 634).
- The comments around the escape target, the launcher Cancel button and the panel's refusal branches say why each is
  shaped the way it is (in particular why the selector is not a plain comma list; see Planner proposals).
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Fix both parts**, the Escape routing and returning focus after a refused save, chosen over the routing alone.
  Without the focus part, the first key typed after a refusal is swallowed by the safety net.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

Stated to the maintainer on the planning page, without objection: the spec already settles the behavior, and the fix
passes the entry's complexity gate.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's launcher sections on the save-as-template panel (around 657 and 729-741)
and on Escape closing the innermost thing first (around 848); SPEC_impl.md's shared modal dialog and isolation
paragraphs (around 598-625) and the panel's save flow (around 5447-5454); root `AGENTS.md`, "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py` for any timed wait the tests need.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Routing: keep the launcher's escape target a static selector, since the isolation script bakes it in at mount. Mark
  the launcher's Cancel while the panel is open (an attribute such as `data-template-panel-open`) and use a selector
  like
  `.save-template-cancel, .launch-composer-cancel:not([data-template-panel-open]), .launch-composer-loading-cancel`. A
  plain comma list adding `.save-template-cancel` does not work: `querySelector` returns the first match in document
  order, which is the launcher's Cancel. Avoid `:has()`: an engine without it would make `querySelector` throw inside
  the safety net and silently disable body Escape for the whole launcher. The panel's Cancel is natively disabled while
  saving, so the click stays a no-op during a save.
- Focus: in both asynchronous refusal branches, after `saving` is cleared, schedule a guarded focus of
  `.save-template-name`. Reuse the launcher's existing guarded deferred focus (`focus_composer_surface` in
  `create_form.rs` around 1557-1576, with its `requestAnimationFrame`, visible-dialog check and "leave focus alone if an
  enabled, visible control in the dialog has it" guard, or `restore_shortcut_focus_if_lost` around 1583-1597) through a
  focus target for the name field, rather than writing another copy of that script. Plain focus, not selecting its text,
  matching how the field is focused on open.
- Not chosen: making the safety net re-dispatch Escape to each dialog's own handler (shared by every modal, each with
  its own busy rules), switching the panel's controls to `aria-disabled`, or making the launcher's Cancel button close
  only the panel.

## Outline

One PR, `plan/template-panel-escape/01-panel-escape`: the selector and attribute in `create_form.rs`, the guarded focus
in `save_template.rs`, the browser tests, the changelog fragment and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-ui`, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, and
through `python3 scripts/record-test-run.py` the changed and re-run browser tests named above on both engines, plus the
other launcher Escape tests if the selector change could reach them (`quick-launch.spec.ts`, `clone.spec.ts`,
`modal-trap.spec.ts`).

## Fallbacks

Examples of departures that need the scope reassessment: changing the shared isolation script's behavior for every
dialog, changing how the panel's controls are disabled, or new focus-tracking state. If either part turns out not to be
simple after all, the entry's complexity gate applies: do not build a large change. Deliver what is simple, and explain
in the report what was left and why; the TODO entry stays if the routing part cannot be fixed simply.

Decisions to log: the final selector and marking, which engines lost focus where, every test changed and why, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-template-panel-escape-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/template-panel-escape/<nn>-<short-name>`,
  as the outline names them.
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
