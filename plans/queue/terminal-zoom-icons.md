# UI: magnifier zoom controls with a size readout for terminal text

Written against main at c2789e25 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Replace the A−/A+ terminal text-size buttons with zoom icons.** The two buttons at the right end of a session's
>   tab strip label the text-size step as "A−" and "A+", which does not read as an action. Use recognizable zoom-out and
>   zoom-in icons (a magnifier with − / +) instead, keeping the tooltips and keyboard-shortcut help. Plan:
>   `plans/queue/terminal-zoom-icons.md`.

The entry's location is out of date: since the steel redesign the two buttons sit at the start of the action group on
the right of the session's top row, before Restart (`TerminalTextSize` in `crates/farhelm-ui/src/session_view.rs`,
mounted in `div.toolbar-text-size` inside `titlebar-actions`; SPEC.md's top-row paragraphs).

### What changes

The text-size control becomes one joined group of three parts sharing a single border: a magnifier-minus button, a
readout showing the current terminal text size in points, and a magnifier-plus button (`[🔍− | 14 | 🔍+]`). The buttons
step the size exactly as "A−" and "A+" do today. The readout is display only: it is not a button, clicking it does
nothing, and it shows the size every terminal on the page is using, updated after a step from either a button or the
Cmd/Ctrl+Shift+−/+ shortcut.

### What does not change

The size range (9 to 28), the default (14), its per-device storage, the shortcut, the focus hand-back to the terminal
after a click, the refit of every mounted terminal, and the buttons' tooltips ("Smaller terminal text
(Cmd/Ctrl+Shift+−)", "Larger terminal text (Cmd/Ctrl+Shift++)"), accessible names and `data-text-size` attributes. The
control's place in the top row is unchanged.

### Acceptance criteria

- The two buttons show magnifier-minus and magnifier-plus icons instead of the "A−" / "A+" text, inside one bordered
  group with the readout between them; tooltips, accessible names and `data-text-size` are as today.
- The readout always shows the current size: on first load, after each button step, after each shortcut step, after a
  reload, and after the control is rebuilt (switching to another session and back, and a stopped or interrupted session
  whose terminal area returns after Restart). It is never blank while a terminal is shown.
- The readout has a fixed width, so stepping between one- and two-digit sizes does not move either button.
- The readout is not a `<button>` and adds no tab stop; the top row's action buttons and their order are unchanged
  (`e2e/tests/header.spec.ts` counts and orders them).
- The top row still fits with the text-size control and every action fully visible from a 650px main pane (SPEC.md's
  top-row width rule; `header.spec.ts`'s 650px geometry tests).
- The existing browser test of text-size stepping (`e2e/tests/terminal-tabs.spec.ts`, the test whose name starts
  "terminal text size steps every terminal") asserts the readout at each step and after its reload, and one new case
  covers a rebuilt control (switch sessions and back).
- SPEC.md and SPEC_impl.md describe the new control; the "Work in a session" docs page no longer says "A− and A+ at the
  right end of the tab strip"; code comments that name "A−/A+" or "the tab strip" for this control are updated.
- A changelog fragment (`kind: changed`, for a `style` PR).
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-29 on a planning page with mockups of four designs:

- **Design C:** the magnifier pair sharing one border with the size in points between them, chosen over a joined pair
  without the number, two separate magnifier buttons, and one magnifier button opening a popover. The number is a
  readout only; a reset on click was explicitly left out as new behavior.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's session top-row paragraphs (the text-size buttons' place and the 650px fit
rule); SPEC_impl.md's top-row DOM-order list (the sentence listing "A−, A+, Restart") and its paragraph on the stateless
text-size buttons ("The top row's A− / A+ buttons are stateless Rust buttons"); `crates/farhelm-ui/src/icons.rs`'s
module docs on drawing icons as inline SVG; root `AGENTS.md`, "Code rules", "Tests and validation", "Conventional
Commits", "Changelog fragments"; `.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): terminal.js paints the readout,
since the size lives only there; reusing the top row's split-button styling for the joined border; one PR. A
fresh-context planning review checked the outline against the code and contributed the repaint-on-mount design, the
fixed width and non-button constraints, the 650px budget, and the list of spec, docs and comment passages to update.

## Outline

1. **Glyphs.** Add `ZoomOut` and `ZoomIn` to `ActionGlyph` in `crates/farhelm-ui/src/icons.rs`: a magnifier (circle and
   handle) with a − or + inside, in the existing style (14-unit viewBox, `currentColor` stroke, the same stroke width,
   `aria-hidden`, a `data-glyph` token). They are Farhelm originals, so `THIRD_PARTY_NOTICES.md` does not change. Read
   `docs/harness-marks.md` before editing `icons.rs`, as root `AGENTS.md`, "Instructions by path", requires.
2. **Markup.** In `TerminalTextSize`, render the two buttons as icon buttons (`ActionIcon`) with their current tooltips,
   `aria-label`s and `data-text-size`, and a readout `span` between them. Keep the `.terminal-text-size` container
   class: `stepFontSize`'s focus hand-back in `crates/farhelm-ui/assets/terminal.js` looks for it. The readout span has
   no children in rsx, so Dioxus never patches the text terminal.js writes into it; mark it with a data attribute
   terminal.js can find.
3. **Painting the number.** The size lives only in terminal.js, and nothing about it flows back to Rust; keep it that
   way. Add a small paint function to `window.farhelmTerm` (for example `paintFontSize()`) that writes the current size
   into every readout element. Call it when the script loads and at the end of `stepFontSize`. Also call it from the
   readout's `onmounted` in Rust (`if (window.farhelmTerm) window.farhelmTerm.paintFontSize();`), because the control is
   rebuilt on every session switch and when `keep_terminal_surface(...)` turns back on inside a view, and terminal.js
   can finish loading after Dioxus has rendered. Whichever side arrives second does the paint.
4. **Styling.** In `crates/farhelm-ui/assets/app.css`, give the group one shared border with squared inner corners,
   following the top row's existing split-button pattern (`.header-action-split`) rather than the segmented control
   (which is a choice control with pressed states, and taller). The readout gets a fixed width with tabular numerals
   (about two characters) and the quiet text tone. The buttons keep the compact toolbar padding.
5. **Spec, docs, comments.** SPEC.md's top-row sentences that name A−/A+; SPEC_impl.md's DOM-order list and the
   stateless-buttons paragraph, which should now record that terminal.js paints the readout and that the readout asks
   for a paint when it mounts (the cross-boundary contract a later reader would otherwise rediscover);
   `website/src/content/docs/docs/using/work-in-a-session.mdx` (read `website/AGENTS.md` first; this is an incidental
   docs edit in an unattended run, so start no preview server and name the changed page in the report); the doc comment
   on `TerminalTextSize`; and the terminal.js and `terminal-tabs.spec.ts` comments that say "A-/A+" or "tab strip".
6. **Tests.** Extend the existing text-size test in `e2e/tests/terminal-tabs.spec.ts` to assert the readout at every
   step it already takes and after its reload, and add the rebuilt-control case. No existing test selects on the "A−" /
   "A+" text; they use `[data-text-size=...]`, which stays. Run `header.spec.ts` for the button count, order and the
   650px geometry, and `tooltip-coverage.spec.ts`.
7. **Bookkeeping.** A changelog fragment; the `steel-session-toolbar.md` fragment in `releasing/changelog.d/` is
   unreleased and already describes the top row's text-size controls, so editing it instead of adding a second entry is
   acceptable (log the choice). Remove the TODO.md entry.

Leave the `.tab-text-size` class name alone even though it dates from the tab-strip era.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the UI crate's tests, the JS unit tests if any cover
terminal.js's text-size code, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and,
through `python3 scripts/record-test-run.py`, the changed `terminal-tabs.spec.ts` test and `header.spec.ts` and
`tooltip-coverage.spec.ts` on Chromium and WebKit. Run `python -B scripts/check-test-sleeps.py` when browser tests
change.

## Fallbacks

Departures that need the scope reassessment: making the readout interactive or adding a reset; moving the size state
into Rust or adding a JS-to-Rust channel for it; changing the size range, step, default or storage; moving the control
within the top row; dropping the readout to fit the 650px budget.

Decisions to log: how the readout is painted and where the paint is triggered, the styling pattern used for the joined
border, whether the changelog entry is new or folded into `steel-session-toolbar.md`, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-terminal-zoom-icons-log.md` in the parent directory of the checkout you run in, derived as that section
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
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/terminal-zoom-icons/<nn>-<short-name>`
  (the expected single PR: `plan/terminal-zoom-icons/01-zoom-controls`). Split only if review shows a real seam.
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, what changes and what does not,
acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
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
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

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
passed its code review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
