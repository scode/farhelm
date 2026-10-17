# Compose quick launch templates with a launch key sequence

Written against main at 78394ca6 on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. The `name-session-while-starting` plan waits for this one, because it opens
a naming dialog after quick launches, composed ones included.

## The goal

Let a quick launch combine several launch templates from the keyboard: "do X, but with Codex". A prefix key starts a
launch sequence, number keys queue templates in the order pressed, and Enter launches the result of stacking them, later
templates winning where they overlap. One number key or ⚡ on its own still launches at once, exactly as today.

TODO.md entry this plan covers (Near term): "**Compose quick launch templates with multi-key chords.** Support
Emacs-style key sequences that combine a base template with a modifier template: "do X, but with Codex". For example,
bind the template for X to Cmd-1 and a "use Codex" template to Cmd-2, then compose them through the sequence. This
captures the concept; the exact keybindings, composition rules, and launch interaction are to be settled during TODO
planning."

### Acceptance criteria

- **Prefix key:** ⌘X in the Mac app, Ctrl+Shift+X in browsers, starts a sequence, except while focus is in a text field
  (New's prompt, the rename field, search boxes), where the key keeps its normal meaning (Cut; Firefox's text-direction
  switch). The terminal is not a text field for this purpose, even though its input element is a textarea: ⌘X from the
  terminal starts a sequence.
- **Inside a sequence:** a number key, bare or with the platform's number-key modifier (1 or ⌘1 / Ctrl+Shift+1), queues
  that key's template; the ⚡ chord queues the ⚡ template; Enter launches the queued templates stacked in the order
  pressed. The user's example: ⌘X, ⌘1, ⌘5, Enter launches key 1's template with key 5's stacked on top. Enter with
  nothing queued cancels the sequence and goes nowhere.
- **Leaving a sequence:** there is no timeout. Escape cancels (and is consumed). A click, switching windows, or a dialog
  opening cancels. Any other key cancels and goes on to its target as usual (the terminal, typically). Bare presses of
  ⌘, Ctrl or Shift on the way to a chord do not count as "another key".
- **Launch:** goes straight through, exactly like a single key, with the usual fallbacks to New (a refusal, the YOLO
  question, an incomplete result), the error naming the template that caused it. An unbound key in a sequence falls back
  at Enter the way a single unbound key does today (Templates opens with the binding hint).
- **Indicator:** while a sequence is in progress, a pill floats over the bottom of the terminal area, like an editor's
  echo area, naming the queued templates in order with "Enter launch · Esc cancel"; right after the prefix key it reads
  "Launch sequence: press number keys to queue templates, Enter to launch, Esc to cancel." With no session open it sits
  at the bottom of the main pane. It is a non-modal `role="status"`, never `aria-modal` (that would stop the shortcut
  listeners). Mockup, option B: https://snippets.scode.org/s/farhelm-launch-prefix-indicator/
- **Keys for any template:** any template can hold a number key, including one that cannot launch on its own (a "use
  Opus" template that sets only the model). What must be launchable is the queued result. Pressed alone, such a key
  opens New with that template applied, and a prominent banner at the top of New, which cannot be missed, explains that
  the template cannot launch on its own and how to combine it with ⌘X. It is announced to screen readers and goes away
  after several seconds or once the user starts editing. The ⚡ mark keeps its current eligibility rule.
- Outside a sequence, single number keys and ⚡ behave exactly as today.
- SPEC.md (Creation's quick-launch and number-shortcut bullets, the managed-checkout consent sentence, the "an
  ineligible template cannot acquire a key" rule, the window chrome shortcut list), SPEC_impl.md, and the docs
  (`website/src/content/docs/docs/using/launch-templates.mdx`, including the eligibility rule near its line 54,
  `session-list.mdx` and `start-a-session.mdx`) describe the new behaviour.

## Requirement sources

**The user's decisions (2026-10-17), in their words where given:**

- "we don't want to break the 'just hit one chord to spawn session'. Dedicated prefix chord means we allow both. i
  suggest cmd-x (emacs style but cmd instead of control)"; ⌘X starts a sequence everywhere except while typing in a text
  field, where it stays Cut; Ctrl+Shift+X in browsers with the same exception.
- Enter is the launch key ("how about we use _enter_ as the launch key, not cmd-p"); ⌘P inside a sequence queues the ⚡
  template like a number key; Enter with nothing queued cancels.
- "Make sure we're on the same page. Here will be an example: cmd-x, cmd-1, cmd-5, [Enter] Would launch the result of
  stacking 5 on top of 1." Several templates stack in the order pressed. Number keys work with or without ⌘ inside a
  sequence.
- No timeout; Escape, a click, switching windows or opening a dialog cancels; any other key cancels and goes on to the
  terminal as usual.
- Launches directly, exactly like a single key.
- Any template can hold a number key; pressed alone, one that cannot launch on its own opens New with it applied. "We
  need to make it obvious, though. perhaps a non-modal card for 3 seconds or so"; then, since a card outside New would
  be shut off while New is open: a hint inside New, "make sure its prominent and cannot be missed and temporary".
- Indicator: mockup option B, the pill over the bottom of the terminal.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC.md's template rules (Concepts; Creation's template, quick-launch and
number-shortcut bullets; "a browser that reserves a chord keeps its behavior"); template stacking in
`crates/farhelm-proto/src/launcher.rs` (`apply_template`, `apply_templates`: later wins, a field that does not apply
refuses its template). Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- The app (Rust) owns the queue and draws the pill. JavaScript keeps only an "armed" flag and a pure classifier that
  decides, inside the key handler, whether to consume a key or let it pass, and reports each action by clicking hidden
  triggers (begin, queue key N, queue ⚡, launch, cancel), the pattern the number-key and ⚡ triggers already use
  (`crates/farhelm-ui/assets/template-number-shortcuts.js`, `crates/farhelm-ui/src/lib.rs`, hidden triggers in
  `crates/farhelm-ui/src/list/view.rs`).
- The sequence listener must run before the ⚡ listener (installed first in `lib.rs`), or the ⚡ listener must hold off
  while armed, so ⌘P queues instead of launching on its own.
- Cancel on a dialog opening needs no detector: the existing visible-dialog check at each keypress plus click and blur
  cancellation cover it.
- The launch path takes an ordered list: generalize `quick_launch` (`view.rs`) to resolve the queued references from one
  fresh read and have the hidden New form replay them one per pass through its existing pending-template step
  (`create_form.rs`), whose error already names the failing template. Judge "can this launch" on the combined result,
  kept as coarse as today's check (`quick_launch_eligible`, `crates/farhelm-ui/src/list/templates.rs`): a last-wins fold
  over launch kind, agent and command. Do not build full launcher state; the quick-launch path has no model catalog or
  host list.
- In a Mac browser, quick launch is ⌘P while number keys use Ctrl+Shift: inside a sequence accept whichever chord ⚡
  uses on that platform. A bare P inside a sequence is "another key" and cancels.
- No storage change: key bindings stay a digit-to-template map; only the eligibility rule for holding a key relaxes.

## Outline

1. `refactor`: the quick-launch path takes an ordered list of template references and replays them in order; the
   combined launchable check. Unit tests for the combined check. No behaviour change.
2. `feat`: the sequence: classifier and its JS unit tests, hidden triggers, queue state, the pill, cancellation, spec
   and docs, changelog fragment. Browser e2e on the web build (Ctrl+Shift chords): a composed launch including ⚡, a
   cancelling key that still reaches the terminal, and the text-field exception. The classifier's other cases are JS
   unit-test rows.
3. `feat`: any template can hold a number key, with the banner in New; the Templates dialog's key menu and hover help;
   spec and docs. Browser e2e for binding a model-only template and pressing its key alone. Removes the TODO entry.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the touched crates' unit tests and the JS unit tests through the
recorder, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
`python3 releasing/check-changelog.py format`, and a selected browser subset on Chromium and WebKit through the recorder
(`quick-launch.spec.ts`, `templates.spec.ts`, and the specs you changed). Not the full suite. The Mac-app chords cannot
be exercised from Linux, and in the Mac app ⌘X also belongs to the Edit menu's Cut: say in the report's "things you
should know" that the sequence needs a check in the Mac app (the maintainer can use the devbuild flow), including that
⌘X still cuts in a text field.

## Fallbacks

- If the Mac app's Edit menu takes ⌘X before the page sees it outside text fields, do not remove Cut from the menu on
  your own: build the rest, record what you found, and put it in the report as a follow-up for the maintainer.
- If the banner's "several seconds" needs a number, use about 6 seconds, gone earlier once the user starts editing.

Examples of departures that need the scope reassessment: a general keymap or prefix system for other commands, a stored
role on key bindings, timeouts, full launcher-state validation in the quick-launch path.

Decisions to log: the classifier's shape, where the queue lives, the combined eligibility fold, the pill's placement
with no session open.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-quick-launch-chords-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/quick-launch-chords/<nn>-<short-name>` (for example `plan/quick-launch-chords/01-ordered-launch`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
