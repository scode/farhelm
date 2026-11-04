# Session list: show the hover summary card beside the list

Written against main at beb9340e on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Move the session hover pop-up to the right of the session list.** Hovering a session in the session list shows a
> pop-up that covers nearby sessions; position it beside the list so it does not hide them.

NOTE: This is a placement change for one pop-up, the session summary card. It is not a redesign of the card's content,
of the shared hover tooltip, or of any other tooltip. The maintainer called it "the trivial hover fix"; keep it that
size.

### Acceptance criteria

- **Beside the list.** When the session summary card shows (hover on a session's title or open area, or keyboard focus
  on it, in either list density), its left edge sits just past the sidebar's right edge, a few pixels of gap, over the
  start of the main pane. It never overlaps the sidebar while there is room to the right.
- **Level with the row.** The card's top is level with the hovered row's top (in practice the top of the row's open
  button, `.session-row-open`, which is the rectangle the tooltip measures). When that would put the card's bottom past
  the window's bottom edge, the card moves up only as far as it needs to stay inside the window (the shared tooltip's 4
  px edge margin), so a row near the bottom still gets the whole card.
- **No room to the right.** When the card would not fit between the sidebar's right edge and the window's right edge (a
  window barely wider than the sidebar), the card falls back to today's placement, above or below the row exactly as the
  shared tooltip places it now.
- **Only the card moves.** Every other tooltip keeps today's rule: the row's own marks (status dot, harness mark,
  permission shield, and the rest) and everything outside the sidebar. The host-update progress pop-up in the hosts
  panel is not a shared tooltip at all (it is placed by its own Rust code) and stays as it is. Show/hide timing,
  dismissal, the card's content and its accessibility stay as they are.
- **Web and desktop alike**: the shared tooltip script serves both, so nothing desktop-specific is expected.
- **Spec, tests, changelog, TODO.** SPEC.md and SPEC_impl.md say where the card shows; unit tests pin the placement
  rule, a browser test pins it in the real page on Chromium and WebKit; a changelog fragment per root `AGENTS.md`,
  "Changelog fragments"; the PR removes the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page, which showed a mockup of the card today (above the row,
covering the rows above) and proposed (just right of the sidebar, level with the hovered row):

1. **Vertical alignment:** the card's top is level with the row's top, moved up only as far as it needs to stay in the
   window, so a row near the bottom still gets a whole card. (Rejected: centred on the row.)
2. **No room to the right:** fall back to today's placement above or below the row. (Rejected: stay to the right but
   pushed left until it fits, covering part of the list.)
3. **Scope:** only the session summary card moves; short tooltips stay above what they explain, close to the pointer.
   (Rejected: every card-style pop-up in the sidebar, including host-update progress.)
4. **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
   be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute
   reviewer.
5. **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- SPEC.md, the session list's paragraph on the hover summary card (around 1800-1815: "The same card appears from the
  title or any unmarked part of the open button, in either density and on keyboard focus ...").
- SPEC_impl.md, the paragraph on the hidden summary card source (around 385-397).
- Root `AGENTS.md`: "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation",
  "Instructions by path" (`e2e/` and any test: `.agents/validation.md`; `e2e/docs-shots/`: `.agents/docs-media.md`), and
  "Frozen material" (published release shots and their tests).
- `.agents/test-authoring.md` for the test changes, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.
- `releasing/EDITORIAL_GUIDANCE.md` for the fragment's wording.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- One PR, `plan/session-summary-beside-list/01-card-beside-list`, titled along the lines of
  `fix: show the session hover card beside the list` (the type is yours to settle per root `AGENTS.md`; a `fix` or
  `feat` needs the fragment either way).
- The rule lives in `crates/farhelm-ui/assets/tooltip.js` as a pure function next to `placeTooltip` (or as a variant of
  it), exported the same way so `crates/farhelm-ui/js-tests/tooltip.test.js` tests it without a browser, and listed in
  the header's "## Testability" section with the others. Its inputs are the row's rectangle, the sidebar's right edge,
  the card's size and the viewport; when the card does not fit to the right it returns whatever `placeTooltip` returns
  for the row.
- The session row opts in from Rust, the way split-menu items opt into `data-tooltip-anchor="panel"`, on the open button
  (`crates/farhelm-ui/src/list/row.rs`, where it sets `data-tooltip-card`). The smallest form is a new value of that
  same `data-tooltip-anchor` attribute rather than a second attribute, with the sidebar found by
  `target.closest(".app-sidebar")`; no new container marker is needed. The session card is today the only
  `data-tooltip-card` user, so "every card inside the sidebar" would behave the same; the explicit opt-in is preferred
  because it matches decision 3 literally and a future card elsewhere would not inherit a sidebar-specific rule.
- The gap between the sidebar and the card is 6 px, reusing `GAP_ABOVE`. That is not arbitrary: the main pane's minimum
  width is 320 px (`.app-main`), and 310 (the card) + 6 (gap) + 4 (`EDGE`) = 320, so with 6 px the card fits beside the
  list exactly whenever the page is not scrolling sideways. A larger gap would make the card fall back in a band of
  ordinary window widths for no visible reason.
- Setting `data-side` (today `above` or `below`, written in `position`) to a new value such as `right` is free and makes
  the placement visible to tests; nothing reads it today.
- Do not name the new rule or function "beside" without checking `crates/farhelm-ui/src/hosts.rs`: the host-update
  pop-up has its own unrelated Rust `PopupAnchor::beside` (around 3708), and a shared name invites confusion.

## What the code looks like at beb9340e

Line numbers are approximate.

**The shared tooltip** is `crates/farhelm-ui/assets/tooltip.js`. Its header explains the design: one `position: fixed`
element appended to `body`, filled with text (`data-tooltip`) or with a clone of a hidden Rust-rendered card source
(`data-tooltip-card`), placed from the target's measured rectangle so the sidebar's scroll container cannot clip it. The
"## Placement" section of that header (around 78-92) states today's rule (above, 6 px, centred, shifted to stay in the
window; below with a 28 px gap when there is no room above) and the split-menu `data-tooltip-anchor="panel"` opt-in. The
pieces this plan touches:

- `placeTooltip(target, size, viewport)` (around 114-147), a pure function returning `{left, top, side}`;
- `tooltipAnchor(target)` (around 190-194), which swaps in the enclosing `[data-tooltip-panel]` rectangle for an
  opted-in split-menu item;
- `position(content)` inside `install` (around 297-330): fills the element, measures it at the origin, measures the
  anchor, calls `placeTooltip`, writes `left`, `top` and `data-side`, and records `placedAt` (the anchor's rectangle),
  which the 200 ms watch compares to re-place a tooltip whose target moved without a scroll (the sidebar re-sorting, a
  window resize);
- the exports (around 528-533): `placeTooltip`, `instantFollowOn`, `scrollMovesTarget`, `install`, on `window` and under
  CommonJS.

Update that header's "## Placement" section to describe the new rule, the opt-in, and the fallback.

**The card** is `crates/farhelm-ui/src/list/summary_card.rs` (`SessionSummarySource`, a hidden sibling of the row). The
row's open button names it with `data-tooltip-card` in `crates/farhelm-ui/src/list/row.rs` (around 1658). In
`crates/farhelm-ui/assets/app.css`, `.farhelm-tooltip[data-kind="card"]` (around 1821-1827) sets the card's width to
`min(310px, calc(100vw - 8px))`.

**The sidebar** is `.app-sidebar` (`app.css` around 590), always on the left, holding the host rail and the list. Its
width is set by the user (`crates/farhelm-ui/assets/sidebar-width.js`: 296 to 656 px, default 396, rail included). When
the window is narrower than the sidebar plus the main pane's minimum, the page scrolls sideways rather than shrinking
the sidebar, which is when the no-room fallback applies.

**SPEC.md** (around 1807-1813) says what the card contains and where it appears from, not where it is drawn; add that it
shows beside the list, level with the row, and falls back above or below in a window with no room beside the list.
**SPEC_impl.md** (around 393-397) describes the card source and its cloning; add the placement opt-in in a sentence. The
docs page `website/src/content/docs/docs/using/session-list.mdx` (around 25-26) mentions the summary without saying
where it appears; it needs no change.

**Tests that see the card:**

- `crates/farhelm-ui/js-tests/tooltip.test.js` (around 31-78) tests `placeTooltip` as a pure function; the new rule's
  unit tests go beside them: beside the sidebar, level with the row; moved up near the window's bottom; the exact-fit
  boundary (fits with nothing to spare, then one pixel short falls back); and the fallback matching `placeTooltip`.
- `e2e/tests/tooltip.spec.ts`: "a session's title and open area share its summary card" (around 93-182) shows the card
  from the title, the open area and keyboard focus in both densities, checking content, not geometry. Add geometry to it
  or a test beside it, once per density (title, open area and focus all go through the same placement, so one check per
  density is enough): the card's left edge at or past the sidebar's right edge and its top level with the open button's
  top; plus the fallback in a window too narrow for the card. "hovering a row mark shows its text above it after the
  delay" (around 270) is the existing check that a mark's tooltip stays above; it should pass unchanged.
- `e2e/tests/popover-identity-movement.spec.ts` (around 228-245 and 326-332) rests the pointer on a row and loads fonts
  so the delayed card "cannot disturb" its geometry premise. A card that now covers the start of the main pane instead
  of the rows could change what that test measures; run it.
- `e2e/docs-shots/visual-review.spec.ts` (around 2549-2555) photographs the card for development review (no docs page
  shows it). Read `.agents/docs-media.md` and `docs/docs-shots/SPEC.md` before touching it; whether to run it (it needs
  a staged fleet) is your judgment, logged.
- `e2e/docs-shots/release-notes.spec.ts` only asserts that no tooltip is visible; its shots are frozen. Leave it alone.

## Outline

1. `plan/session-summary-beside-list/01-card-beside-list`, one PR and one commit:
   - the placement rule and its opt-in in `tooltip.js`, with the header's Placement section updated;
   - the opt-in attribute on the row's open button in `row.rs`;
   - the unit tests in `tooltip.test.js` and the browser test geometry above;
   - SPEC.md and SPEC_impl.md;
   - the changelog fragment, and the TODO.md entry removed, including its `Plan:` reference.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `dprint check` on the changed files, `cargo fmt --check`, and clippy on `farhelm-ui`;
- `cargo check -p farhelm-ui --features desktop`;
- the JavaScript unit tests for `tooltip.test.js` (`node --test`, as `.agents/validation.md` says);
- `python3 releasing/check-changelog.py format`;
- `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares;
- through the recorder, on Chromium and WebKit, after the web build `.agents/validation.md` describes: `tooltip.spec.ts`
  and `popover-identity-movement.spec.ts`. (The `sidebar.spec.ts` tests that mention the card read its hidden source's
  text and never show it, so they cannot see this change.)

## Fallbacks

Examples of departures that need the scope reassessment:

- changing placement for any tooltip other than the session summary card, or changing the card's content or size;
- a placement to the left of, or inside, the sidebar, or any layout change to the sidebar or main pane;
- making the card hoverable or interactive (it stays a pointer-transparent visual aid);
- touching published release shots or `e2e/docs-shots/release-notes.spec.ts`.

If a test that was not written about the card fails because the card now covers the start of the main pane (something it
points at sits under the card), fix the test's premise (park the pointer, or wait for the card to hide) rather than
moving the card, and log it as a DECISION.

Decisions to log: the attribute name, the gap, how the sidebar is found, whether placement is a new function or a
variant of `placeTooltip`, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-session-summary-beside-list-log.md` in the parent directory of the checkout you run in, derived as that
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
without costing quality; for a change this small, doing it yourself is probably quickest. Concurrent agents that might
otherwise conflict can work in separate jj workspaces whose results you merge back; you still own the stack and every
PR. Launch a sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you
need is not available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies
the launch mechanics.

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

- One reviewable commit, stable bookmark and draft PR, using the `jjstack` skill. The stack's base is `main@origin`, or
  this plan's own open PR when it resumes, set up per `plans/.agents/executing.md`, "Executing one plan" (step 6); never
  another plan's PRs. The bookmark is `plan/session-summary-beside-list/01-card-beside-list`, as the outline names it.
  If the work turns out to want more than one PR, they form one linear stack with bookmarks
  `plan/session-summary-beside-list/<nn>-<name>`, logged as a DECISION.
- Avoid churn: within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to a PR an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and the changelog fragment in the same commit.
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
already made and outline). This PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim in
the prompt, as root `AGENTS.md` requires.

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

The plan is complete when a draft PR (or a linear stack of them) exists that meets the acceptance criteria above, each
PR passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
