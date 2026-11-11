# Host progress card: open it from the whole sidebar row, and show the end of long errors

Written against main at cb501937 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Two TODO.md entries, merged into one plan with the maintainer's agreement because they change the same hover card, the
same tooltip code and the same spec paragraphs. Verbatim, from Near term:

> **Open the host update hover card from the progress bar too.** Hovering a host's update flow in the sidebar shows its
> step list only when the pointer is over the message above the progress bar; make the progress bar itself show the same
> hover.

And from Plan decisions to revisit:

> **Revisit cutting long connection errors short in the host progress hover card.** Decided on the maintainer's behalf
> on 2026-11-09 when host-progress-hover blocked: when a host's connection detail or a run's message is too long to fit
> beside a readable current step, the hover card for a working host shows a shortened preview and says the full details
> are in Hosts, and stays a plain, non-interactive tooltip. A message that fits is shown whole. The alternative was a
> scrollable area inside the card, which keeps the whole text readable on hover but makes the card interactive (pointer,
> dismissal and keyboard handling).

The card is the shared host progress card the `host-progress-hover` plan landed (#3272): one Rust-rendered hidden source
per host or attention row, cloned into the page's single tooltip by `crates/farhelm-ui/assets/tooltip.js`, and
pointer-inert like every hover display in the UI.

### Where things are (as of cb501937; correct line numbers to the truth as you go)

- The sidebar's update flow is the attention list (`crates/farhelm-ui/src/hosts/attention_list.rs`, `AttentionHostRow`,
  around 205-388), rendered only while the Hosts panel is closed. A working row is a `div.host-attention-row` holding
  `button.host-attention-open` (mark, name, "Updating · <step>"; it carries `data-tooltip-card` and
  `data-tooltip-anchor="session-row"`, or a plain `data-tooltip` before any step data exists) and, as a sibling,
  `<progress class="host-attention-progress">` on its own grid line (app.css around 10252-10297). The bar, the 4px grid
  gap and the row padding carry no tooltip attribute, so `tooltipTarget` (tooltip.js around 282-291) finds nothing from
  them and hides the card.
- The Hosts panel has no bar: its status span carries the card itself (`hosts.rs`, `UpdateProgressLabel`, around
  3961-3996). Its card shares `HostProgressCard` (`hosts.rs` around 4029-4088) and the same sizing code.
- Sizing: `fitHostProgress` (tooltip.js around 247-274) runs on every fresh clone (the card's source re-renders once a
  second for its clock, so it is re-cloned that often); it reserves the current step's height and, when the diagnostics
  do not fit, un-hides "Full details in Hosts" and sets a pixel `maxHeight` on each oversized text, split by
  `diagnosticPreviewHeights` (around 228-239). The detail texts have `overflow: hidden` (app.css around 5871-5917). The
  cut lands at a pixel height, usually through a line, and shows the start of the text.
- Spec text: SPEC.md around 140-145 (the bounded preview, "Full details in Hosts", non-interactive), SPEC.md around
  2586-2587 ("Their identity button opens the shared progress card on hover or keyboard focus ... and their marks and
  bars have no separate plain tooltip"), SPEC_impl.md around 1428-1435 (sizing, pointer-inert).

### Acceptance criteria

1. In the sidebar, hovering anywhere on a working host's attention row (the bar, the gaps, the padding, the name)
   behaves exactly as hovering its name button: the same rest delay, the same card in the same place with the same ring
   on the button, no hide-and-reshow when the pointer moves between the bar and the name, and a press anywhere on the
   row dismisses it the way a press on the button does. Nearer targets inside other row kinds (Update or Details
   buttons) still win. The button keeps its own tooltip attribute (`e2e/tests/tooltip-coverage.spec.ts` requires it).
   This also covers a row still waiting for its first step, which shows the button's plain tooltip.
2. When a connection detail or run message does not fit, the card shows a whole number of its lines, taken from the end
   of the message, and still says "Full details in Hosts". Messages that fit stay whole. The card stays a plain,
   pointer-inert hover card. Both surfaces (sidebar and Hosts panel) behave this way.
3. SPEC.md and SPEC_impl.md say what now happens, in the places listed above.
4. The last code PR removes both TODO entries.

## Decisions already made

Settled by the maintainer on the 2026-11-11 planning page:

- **Merged:** one plan, two PRs.
- **What opens the card:** the whole working row, behaving exactly like hovering the name (no flicker, the card stays
  put). Rejected: the bar as a second trigger of its own.
- **Long errors:** whole lines only, from the end of the message, still with "Full details in Hosts"; the card stays a
  plain hover card. Rejected: a scrollable area inside the card (the decision's alternative), and whole lines from the
  start.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Size gate:** about 400 lines of diff across both PRs (code, tests and docs, not counting changelog fragments or the
  TODO.md removal). Over it, block and bring it back.

## Requirement sources

**The user's request:** the two entries above and the decisions just listed.

**Binding repository constraints:** SPEC.md and SPEC_impl.md as cited; `e2e/tests/tooltip-coverage.spec.ts` (every
visible button carries its own tooltip attribute); the pointer-inert rule for hover displays (tooltip.js comments around
31-36, the terminal link code's reliance on it in `assets/terminal-files.js`); root `AGENTS.md`, "Tests and validation",
"Changelog fragments", "Conventional Commits", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- PR 01: mark a working attention row in Rust (a data attribute) so that `tooltipTarget` resolves a pointer anywhere in
  it, with no nearer target, to the row's `.host-attention-open` button. A redirect alone is not enough: tooltip.js
  judges "inside the target" against the resolved element, so a sweep across the bar would not restart the rest delay
  (`pointermove`, around 585) and a press on the bar would not keep the card dismissed (`pointerover`, around 557).
  Judge containment by "`tooltipTarget(x)` is the current target" (or by the delegating row) in those places too.
  `tooltipTarget` is not exported (export list around 681): export it for the unit test or test through the existing
  fake-DOM `install` harness in `crates/farhelm-ui/js-tests/tooltip.test.js`.
- PR 02: in `fitHostProgress`, make each oversized preview a whole number of lines (measure a line, or give the boxes an
  explicit line height, since a computed `line-height` can be `normal`) and show the end of the text, for example by
  scrolling the clipped box to its bottom (setting `scrollTop` on the clone is compatible with the tooltip's scroll-hide
  listener, as the current-step scrolling already shows). A leading ellipsis marking the cut start is welcome only if
  trivial; "Full details in Hosts" already says the text is incomplete.

## Outline

1. `plan/host-progress-card-fixes/01-row-hover`: the whole-row hover. JS unit tests (rest delay and press dismissal over
   the bar, nearer targets winning) and a browser test in `e2e/tests/host-updates.spec.ts` that hovers the bar and moves
   between bar and name; SPEC.md around 2586-2587. `fix:` with a changelog fragment.
2. `plan/host-progress-card-fixes/02-error-tail`: whole lines from the end. Update the preview tests
   (`host-updates.spec.ts` around 597-694, and any that assert the start of a clipped text) to assert whole lines and
   the tail visible, on both surfaces or with a stated reason why one covers both; SPEC.md around 140-145 and
   SPEC_impl.md around 1428-1435. Removes both TODO entries. `fix:` with a changelog fragment.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect the JS unit tests for tooltip.js, the touched
browser specs (`host-updates`, `tooltip`, `tooltip-coverage`, and `provisioning` where it hovers the card) on Chromium
and WebKit through `python3 scripts/record-test-run.py`, `cargo fmt` check and clippy on `farhelm-ui` if Rust changed,
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and
`python3 releasing/check-changelog.py format`. No cloud calls.

## Fallbacks

Examples of departures that need the scope reassessment: making the card interactive in any way, changing how other
tooltips choose their target, or restructuring the attention row's markup (moving the bar inside the button would hide
its progressbar role, which tests query).

If showing the tail turns out impossible without breaking the step-list reservation or the once-a-second refresh, that
weakens a maintainer decision: finish PR 01, then block with what was tried and the options left.

Decisions to log: how the row delegates, how containment is judged, how lines are measured, how the tail is shown, and
every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-progress-card-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  `plan/host-progress-card-fixes/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
