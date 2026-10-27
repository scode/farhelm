# Link refusals never cover a session confirmation's warning

Written against main at 6b7bbcda on 2026-10-26. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute the triage outcome `link-refusal-conceals-delete-warning.md` (`fix code`), recorded on 2026-10-26, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Its `TRIAGE_OUTCOMES.md` entry is the
binding statement of what to do; read it in full from `main@origin`, together with the feedback file under
`review_feedback_queue/`.

When Farhelm refuses to open a terminal link because the link's preview is covered, it raises an explanation band across
the top of the terminal while the pointer stays on the link. A session's header confirmations (Delete, and the Restart
and Replace panels that share the same markup) hang over the terminal's top right, below that band in the same stacking
context. The band covers the first line or two of the confirmation's consequence text while its focused affirmative
button stays visible; the Delete shortcut opens the panel without moving the pointer, and Enter confirms.

What the code does today, from triage's read of main at 2977e6c9 (line numbers approximate):

- `crates/farhelm-ui/assets/app.css`: the refusal status line (around 1378-1411; raised to z-index 51, dropped to 12
  under a modal); the layer ladder comment (around 1759-1762 and 1796-1804), which already says the consequence text
  "must never be covered" and records this overlap as a known follow-up; `.header-confirm` (around 1812-1852, z-index
  30).
- `crates/farhelm-ui/assets/terminal-files.js`: raising and `lowerRefusal` (around 379-409), cleared on the next
  terminal click (around 229-231, 457-478). Nothing on keydown or focus lowers it.
- `crates/farhelm-ui/assets/session-action-shortcuts.js` (around 7-53): the Delete chord clicks `.header-delete`.
- `crates/farhelm-ui/src/session_view.rs` (around 2353-2399): the header confirmation markup, autofocused.

### Acceptance criteria

- While any session header confirmation is open, a raised link refusal never covers its consequence text, whichever came
  first: a confirmation opened over a raised refusal, and a refusal raised while a confirmation is already open.
  Lowering the refusal only when a confirmation opens misses the second order.
- The layer-ladder comment in `app.css` is updated to match.
- Tests cover both orders: the Delete confirmation opened by its shortcut while a raised refusal is up, and a refusal
  raised while the confirmation is open. Each shows the refusal is below the confirmation.
- SPEC.md's exception letting the refusal cover approval cards (around 2045-2047) stays as it is.
- `fix:` with a changelog fragment.

## Decisions already made

- **The outcome**, picked by the maintainer on 2026-10-26: "It may not cover the warning".
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the `TRIAGE_OUTCOMES.md` entry named above.

**Binding repository constraints:** SPEC.md's destructive-confirmation rules (around 1350-1364: the answer applies to
"the wording on screen at the click") and the approval-card exception (around 2045-2047); root `AGENTS.md`, "Tests and
validation", "Changelog fragments", "Conventional Commits"; `.agents/test-authoring.md`;
`review_feedback_queue/.agents/execute-outcomes.md`. The queued Steel plans restyle the toolbar and dialogs but state
that behavior does not change; this plan does not wait for them.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- One CSS rule that drops the raised refusal below the confirmation layer whenever the session view contains an open
  `.header-confirm`, using `:has()`, built like the existing rule that lowers it under a modal (`[inert]`, around
  1407-1411 in `app.css`; the stylesheet already uses `:has()` elsewhere). It covers both orders and the Restart,
  Replace and Delete panels at once, with no script change. Do not raise the confirmation above 51 instead, which would
  make it a link cover in its own right.

## Outline

One PR, `plan/link-refusal-confirm-layer/01-lower-refusal-under-confirm`: the change, the comment, the test and the
fragment.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect the JS unit harness
(`cd crates/farhelm-ui/js-tests && node --test`) if the logic is testable there, the relevant browser spec on Chromium
and WebKit through `python3 scripts/record-test-run.py`, `cargo fmt` check and clippy on `farhelm-ui` if Rust changes,
and `python -B scripts/check-test-sleeps.py` when you change browser tests.

## Fallbacks

Examples of departures that need the scope reassessment: reordering the whole layer ladder, moving the refusal band, or
changing SPEC.md's approval-card exception.

Decisions to log: the shape chosen, whether Restart and Replace panels are covered by the same change, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-link-refusal-confirm-layer-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/link-refusal-confirm-layer/<nn>-<short-name>`, as the outline names them.
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

The plan is complete when a linear stack of draft PRs exists, one per triage outcome in the order the outline gives,
each meeting that outcome's completion criteria in `TRIAGE_OUTCOMES.md` (as refined by Decisions already made above),
each having passed its review gate, and each recording its execution in the outcome's Execution field (change id,
bookmark and PR URL) and removing its feedback file and `review_feedback_queue/INDEX.md` line, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
