# UI: an icon-only new-terminal button set apart from the tabs

Written against main at c2789e25 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Make the "+ Terminal" control look distinct from a terminal tab.** The button that creates a new terminal is
>   styled so close to a real tab that it reads as another tab. Visually separate it from tabs, or otherwise make it
>   clear that it creates a terminal. Plan: `plans/queue/new-terminal-button.md`.

Today the control is `button.btn.btn-neutral.tab-add` right after the last terminal tab in the session's tab strip
(`crates/farhelm-ui/src/session_view.rs`), with the text "+ Terminal" and the tooltip "+ Terminal: open a shell tab in
the session's folder". It shares one size rule with `.tab` in `crates/farhelm-ui/assets/app.css`, so it has a tab's
size, padding and radius; only the neutral fill and border set it apart. The comment above that rule records why the
neutral tier exists: a ghost "+ terminal" read as one more unselected tab (#959).

### What changes

The control becomes a square, icon-only "+" button (the existing `ActionGlyph::Add`), set apart from the last tab by a
thin vertical divider, like a browser's new-tab button. Its accessible name is "New terminal" and its tooltip "New
terminal: open a shell tab in the session's folder". The host rail's Add host button (`btn host-rail-add` in
`crates/farhelm-ui/src/hosts.rs`, with `aria_label: "Add host"` and an `ActionIcon { glyph: ActionGlyph::Add }`) is the
precedent to copy.

### What does not change

What the button does, when it is disabled (while a tab is opening or the session is busy), its `tab-add` class (every
browser test and the docs-shot spec find it by that class), its neutral tier, and everything else in the strip.

### Acceptance criteria

- The control shows only the `+` glyph, is square, and sits after a thin divider that is not part of its click target;
  its accessible name is "New terminal" and its tooltip "New terminal: open a shell tab in the session's folder".
- It keeps the `tab-add` class, the `btn btn-neutral` tier and its disabled rules; existing browser tests that use
  `.tab-add` pass unchanged.
- `e2e/tests/buttons.spec.ts`'s neutral-tier check still passes, and an assertion beside it checks the accessible name
  "New terminal".
- The Mac VM brick-test instructions no longer tell the tester to click "+ terminal": `guest_workflow.py`,
  `checkpoint_contract.py` and `RECIPE.md` under `releasing/mac-vm-test/`, and the phrase `test_guest_workflow.py` there
  asserts. The new wording describes the control so that it works for both the previous stable release and the
  candidate, for example "the new-terminal button (+) after the tabs".
- The docs page `website/src/content/docs/docs/using/work-in-a-session.mdx` no longer calls it the "+ terminal button"
  in the screenshot's alt text.
- A changelog fragment (`kind: changed`, for a `style` PR).
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-29 on a planning page with mockups of four designs:

- **Design A:** a square icon-only "+" after a divider, chosen over a terminal icon plus "New terminal" label after a
  divider, a dashed "add" outline, and moving the control into the action group on the right.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** root `AGENTS.md`, "Code rules", "Tests and validation", "Conventional Commits",
"Changelog fragments", and "Instructions by path" (`releasing/` and `website/`); `releasing/AGENTS.md` and
`releasing/mac-vm-test/BRICK-TEST.md` for the brick-test files; `website/AGENTS.md` for the docs page;
`.agents/docs-media.md` for why the docs screenshot is not recaptured here; `.agents/test-authoring.md`;
`.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the divider as a decorative
`aria-hidden` element or a pseudo-element with `pointer-events: none`; an explicit square size rather than
`aspect-ratio`; the glyph size matching nearby toolbar glyphs; one PR. A fresh-context planning review checked the
outline against the code and contributed the brick-test files, the docs alt text, the Add host precedent, and the
finding that neither SPEC file nor any test names the control by its label, so neither needs an edit.

## Outline

1. **Markup** (`session_view.rs`): replace the "+ Terminal" text with `ActionIcon { glyph: ActionGlyph::Add }`, add
   `aria_label: "New terminal"`, change the tooltip, keep the class and the `disabled` expression.
2. **CSS** (`app.css`): take `.tab-add` out of the shared `.tab, .tab-add` rule and give it its own square sizing (an
   explicit width equal to the tab height, or equal padding around a fixed-size glyph; if you use `aspect-ratio` on the
   stretched flex item, check it in WebKit, the Mac app's engine). Size the glyph like the nearby toolbar glyphs (about
   12-13px), not `ActionIcon`'s 14px default. Add the divider between the last tab and the button: either an
   `aria-hidden` span before it in the markup (the strip's gap spaces it) or a `::before` on the button with
   `pointer-events: none` so a click on the divider does not open a terminal. A `border-left` on the button would spoil
   its square look. Rewrite the comment above the rule, keeping the #959 history. `e2e/tests/header.spec.ts` measures
   the tunnel pill against the button's right edge, which a left-side divider does not move.
3. **Brick tests.** Read `releasing/AGENTS.md` and then `releasing/mac-vm-test/BRICK-TEST.md` in full before editing
   anything under `releasing/`. Reword the three instruction files and the asserted phrase, and validate with the
   coordinator's own unit tests as `BRICK-TEST.md` says (no VM needed), through `python3 scripts/record-test-run.py`.
4. **Docs.** Read `website/AGENTS.md`, change the alt text, and optionally add a short "click **+** after the tabs" to
   the page's prose, since the control no longer carries a label. This is an incidental docs edit in an unattended run,
   so start no preview server; name the changed page in the report. The published docs screenshot of the tab strip
   (`work-in-a-session/tabs`) goes stale; do not capture or commit it (only the publish scripts push screenshot refs,
   per `.agents/docs-media.md`), and say so in the report.
5. **Tests.** Add the accessible-name assertion beside `expectNeutral(".tab-add")` in `buttons.spec.ts`. No geometry
   test for the divider or the square shape.
6. **Bookkeeping.** A changelog fragment; remove the TODO.md entry.

Leave out of scope: the neighboring TODO entry about the text-size buttons (another plan), and the historical records
that still say "+ terminal" (`docs/design/steel/mockups.html`, `BUGS_BURNDOWN.md`, `lore/`).

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the UI crate's tests, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, the brick-test coordinator's unit tests, and, through
`python3 scripts/record-test-run.py`, `buttons.spec.ts`, `header.spec.ts`, `tooltip-coverage.spec.ts` and the terminal
tab specs that click `.tab-add`, on Chromium and WebKit. Run `python -B scripts/check-test-sleeps.py` when browser tests
change.

## Fallbacks

Departures that need the scope reassessment: changing what the control does or when it is disabled; moving it out of the
tab strip; renaming the `tab-add` class; restyling the tabs themselves; recapturing docs screenshots.

Decisions to log: how the divider is built, how the square size is set, whether the docs page's prose gains a sentence,
the brick-test wording, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-new-terminal-button-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/new-terminal-button/<nn>-<short-name>`
  (the expected single PR: `plan/new-terminal-button/01-icon-button`). Split only if review shows a real seam.
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
