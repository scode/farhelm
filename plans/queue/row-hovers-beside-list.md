# UI: every session-row hover opens beside the list, with a notch and a ring tying it to its target

Written against main at 66e77cf4 on 2026-11-07. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Give every session-list hover card the beside-the-bar placement, with a pointer to what it describes.** Only the
> row's own summary card opens to the right of the sidebar today (`crates/farhelm-ui/assets/tooltip.js`,
> `placeSessionCard`, opted into by the row's `data-tooltip-anchor="session-card"`); every other hover in a session row
> — status, agent mark, activity stamp, stale badge, pin, actions — still uses the ordinary above/below tooltip. Make
> them all open beside the bar like that one, and add a second cue beyond position that ties the card to its target: a
> tasteful arrow or pointer of some kind. Exact visuals and mockups are for planning.

How it works today, from a read of main at 66e77cf4 (line numbers approximate):

- `crates/farhelm-ui/assets/tooltip.js` keeps one shared `div.farhelm-tooltip`, `position: fixed`, appended to `body`.
  `tooltipTarget` (around 195) picks the nearest element with `data-tooltip` or `data-tooltip-card`. `placeTooltip`
  (around 133) centres it over the target, above with a 6 px gap, below with a 28 px gap when there is no room above.
  `placeSessionCard` (around 159-164) puts a card at `left = sidebar right + 6`, its top level with the target's top,
  clamped to the window, and falls back to `placeTooltip` when it would not fit to the right; it sets
  `data-side="right"`. It is used only for card content on a target with `data-tooltip-anchor="session-card"` (around
  345-349), and the 200 ms watch tick re-places it on resize (around 392). `data-tooltip-anchor="panel"` (around 207)
  anchors menu items to their panel.
- Hover targets in a session row (`crates/farhelm-ui/src/list/row.rs` and helpers): the pin drag grip, the row's open
  button (the summary card, the only `session-card` anchor, around 1747), the status badge and dot (`status.rs`), the
  compact ended-status icon, the pin glyph, the compact stale qualifier, the agent track and its glyphs
  (`data-tooltip-paragraphs`, up to 440 px wide), the activity stamp `.status-time`, the stale badge, the host mark
  (`icons.rs`, `HostMark`), the managed checkout mark (`icons.rs`), the notification bell overlay (`bell.rs`), and the
  actions toggle `.session-row-menu`. The actions menu's own items use `data-tooltip-anchor="panel"` in the flyout.
- No tooltip has an arrow; `app.css` (around 1822) says so deliberately for ordinary tooltips. The session actions
  flyout has a pointer notch, `.session-row-menu-pointer` (around 3055): a 10 px rotated square in `--bg-2` with
  `--border` on two sides, in a non-scrolling wrapper so the panel's scroll cannot clip it, hidden under
  `.sidebar-menu-overlaps`. The tooltip box has `overflow: hidden`, so a notch drawn inside it would be clipped.
- SPEC.md (around 2076-2078) says the summary card "sits just right of the sidebar, level with the row and shifted up
  when needed to fit the window. When there is no room to the right, it uses the ordinary above/below tooltip placement.
  Other tooltips keep their placement." SPEC_impl.md around 430-434 (the `session-card` opt-in, "nearer marks retain
  ordinary placement") and around 1302-1306 (the general tooltip rule).
- Tests: `crates/farhelm-ui/js-tests/tooltip.test.js` (around 83-112, beside placement, clamp, exact fit, fallback);
  `e2e/tests/tooltip.spec.ts` (around 93 the card's side and offset, 189 the narrow-window fallback, 222 row marks
  keeping their own hover, 308 "hovering a row mark shows its text above it", which this plan inverts);
  `e2e/tests/tooltip-coverage.spec.ts`.
- History: `145fabea` (#2972, 2026-11-04) moved the summary card beside the list because "the hover card covered
  neighbouring sessions, getting in the way of browsing the list".

### Acceptance criteria

- **Beside the list.** Every hover target inside a session row listed above, and the notification bell, shows its
  tooltip or card just right of the sidebar, with the same horizontal offset as the summary card today. Tooltips outside
  session rows (the hosts list, toolbars, dialogs, the actions menu's own items) keep today's placement and have no
  notch.
- **Centred on the target.** The box is vertically centred on the hovered target, shifted only as far as needed to fit
  the window. This applies to the summary card too (centred on its row, no longer top-aligned).
- **The notch.** A pointer notch on the box's left edge, in the look of `.session-row-menu-pointer`, points at the
  vertical centre of the hovered target, and stays level with the target when the box was shifted to fit the window
  (clamped so it never leaves the box's straight edge). It follows the light and dark themes as the box does.
- **The ring.** While its tooltip or card shows, the hovered target wears a thin accent ring. This includes the summary
  card's target (the row's open button). The ring goes away when the tooltip hides.
- **Fallback.** When there is no room to the right, the box uses the ordinary above/below placement with no notch and no
  ring, as the summary card does today.
- **Behavior otherwise unchanged:** the show and hide timing, keyboard focus, touch, the watch tick, and the summary
  card's contents.
- SPEC.md's tooltip paragraph (around 2076-2078) and SPEC_impl.md (around 430-434 and 1302-1306) describe the new
  placement, the notch, the ring and the fallback.
- Tests: `tooltip.test.js` covers the centred placement, the clamp with the notch kept level with the target, and the
  fallback; `e2e/tests/tooltip.spec.ts`'s row-mark test (around 308) is inverted to assert the beside-the-list placement
  with the notch and ring (not deleted), and the card tests are updated for centring; the coverage sweep still passes.
  Browser tests run on Chromium and WebKit.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: changed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-07, on the planning page (mockups shown there):

- **Cue: a notch plus a ring** (the page's option B). A pointer notch on the box's left edge, like the actions menu's,
  level with the hovered target, and a thin accent ring on the hovered target while its box shows.
- **Vertical: centred on the hovered target**, shifted only to fit the window. In the maintainer's words: "also do the
  same centering for the summary card for the session, which works now that we will have this new style."
- **The summary card gets the same cue.** Planner reading, recorded as decided: "the same cue" means both the notch and
  the ring, the ring on the row's open button.
- **Scope: everything inside a session row, the notification bell included.** Tooltips outside session rows keep today's
  above/below placement with no arrow; the actions menu's own items keep their placement beside the menu.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's tooltip paragraph (around 2076-2078); SPEC_impl.md around 430-434 and
1302-1306; SPEC_impl.md's looping-indicator and motion rules if the ring or notch animates (they should not need to);
`docs/desktop-web-triage.md`; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Opt in explicitly: generalize today's `data-tooltip-anchor="session-card"` into an attribute value every in-scope
  target carries (and the bell), rather than matching anything inside the row with `closest()`, which would also catch
  the actions menu's items. The card keeps its own content path. The planning review recommended this.
- Draw the notch as its own element beside the tooltip box (not inside it, because the box clips), positioned by
  `tooltip.js` from the target's centre; reuse `.session-row-menu-pointer`'s styling rather than a new design.
- Set the ring with a class `tooltip.js` adds to the target while shown and removes on hide (including when the target
  is removed or the tooltip moves to another target).
- The 28 px cursor-clearance gap applies only to the below placement and stays as it is for the fallback.

## Outline

One PR, or two if it grows past a comfortable review:

1. `plan/row-hovers-beside-list/01-row-hovers-beside-list`: the opt-in on every in-scope target and the bell, centred
   beside-the-list placement for all of them and for the summary card, the notch element, the ring class, the fallback
   without notch or ring, SPEC.md and SPEC_impl.md, the JS unit and browser tests, the changelog fragment and the
   TODO.md removal. If it is too large, split the placement change (with its tests) from the notch and ring, keeping
   each PR complete on its own.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-ui`, the JS harness (`cd crates/farhelm-ui/js-tests && node --test`), `python -B scripts/check-test-sleeps.py`
with the interpreter `docs/test-sleep-check.md` prepares, and, through `python3 scripts/record-test-run.py`,
`e2e/tests/tooltip.spec.ts`, `e2e/tests/tooltip-coverage.spec.ts` and any other spec that asserts tooltip position, on
both engines.

## Fallbacks

Examples of departures that need the scope reassessment: changing tooltips outside session rows, a new tooltip element
per target, animating the notch or ring, or changing the summary card's contents.

Decisions to log: the opt-in mechanism, how the notch is positioned and clamped, every test changed and why, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-row-hovers-beside-list-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/row-hovers-beside-list/<nn>-<short-name>`, as the outline names them.
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
