# Hover help placement across the app

Written against main at 63d8ecd5 on 2026-11-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Audit all the hover positioning in the app.** An example is the hover on the Help menu, which covers the menu: the
> tooltip for "Announcements" opens above that item and hides the "Keyboard shortcuts" entry above it. Fix these hover
> positioning issues across the entire app in one fell swoop, settle some standards, and identify all the hovers that
> exist.

The maintainer's note from the planning picker, verbatim:

> yeah please find all the hovers as part of planning. the hovers for the sessions is already fixed after it was super
> annoying, but there are problem N other ones in various places in the app and i don't want to whack-a-mole them. we
> need some consistent rule if psosible, or just at last enumerate all the cases. please use realistic mockups (at least
> realistic positioning wise) to help me pick appropriate positions, then we can perhaps infer rules from the picks

Planning enumerated the hovers and the maintainer picked a placement per class of control on mockups drawn at the
desktop app's real geometry. This plan builds those placements, writes them down as a standard in SPEC.md and
SPEC_impl.md, and leaves no hover help in the app outside the standard.

### Where hover help stands today (from the planning inventory, at main 63d8ecd5)

- There is one hover-help mechanism: the layer manager's single tooltip (`crates/gui/farhelm-gui/src/layers/tooltip.rs`,
  placed by `layers/beside.rs`, drawn by the overlay in `layers.rs`). Every component's constructor requires a
  `HoverHelp` (`components/mod.rs`), wired through `with_hover_help`, so every control has help, shown after a 300 ms
  rest and on keyboard focus. There are about 443 construction sites.
- Two placements exist. Ordinary (`around_target`): centered above the control with a 6 px gap, else 28 px below
  (`GAP_BELOW`, clearing the pointer), else the roomier side, clamped 4 px inside the content area. Beside the sidebar
  (`beside_sidebar` via `.beside()` / `HelpPlace::Row`): 6 px right of the sidebar's outer edge, vertically centered on
  the control, with a left notch and a 1 px accent ring, falling back to ordinary when there is no room. Only session
  rows, their marks, the row ⋯ button, the bell, the attention list's working-host progress card and the Hosts panel's
  progress card use it. The window has exactly one help edge (`LayerStack::beside_edge`, drawn by the sidebar).
- Keyboard-focus help uses the ordinary rule against the focused control, except that beside help shown by focus is
  beside too (the overlay lets beside win, `layers.rs` around the `Against::Focused` dispatch).
- Nothing avoids neighbours, so every menu item's help covers the items above it (the reported Help menu case:
  Announcements' help covers Release notes and Keyboard shortcuts), and so does every vertical stack that is not a
  session row. Full-height controls (the sidebar divider, the terminal scrollbar) find no room above or below and pin
  their help to the window's top or bottom edge, far from the pointer.
- SPEC.md ("Session list", the paragraph on session-row hovers) ends: "Tooltips outside session rows, including the
  actions menu's items, keep their placement without these cues." That sentence changes with this plan. SPEC_impl.md's
  layer manager section holds the numbers.

### The classes and the maintainer's picks

The planning inventory grouped hover help into ten classes. The picks (2026-11-19 planning page):

- **C1 Menus and dropdown choices** (every `MenuItem`, so every menu: Help, session row actions, group, Mute submenu,
  marked-sessions, Sort, host actions, version update menu, Restart ▾ / Replace ▾ caret menus, template key menu; every
  select's choices; plus the notifications panel's, the row bell list's and the tunnels list's buttons): **beside the
  menu panel**, 6 px past its right edge, vertically centered on the item, with a notch pointing at it; when there is no
  room on the right, beside the panel's left edge with a mirrored notch; when neither fits, today's ordinary rule.
- **C2 Non-session rows in the sidebar** (session group headers and their attention dot, attention list rows and their
  Details / Update buttons and header controls, Hosts panel rows, marks, progress lines, ⋯ and lifecycle buttons, the
  "show all" links in the empty-list sentence): **beside the sidebar**, the session-row rule as it stands (notch and
  ring).
- **C3 Rail avatars and the rail foot** (All hosts, host avatars, the unregistered-host placeholder, the unreadable list
  status, Add host; bell, Hosts, Settings, Help, Expand/Collapse): **beside the rail's own edge** (x ≈ 62 collapsed; it
  moves with the expanded rail), with notch and ring. Not beside the sidebar's edge.
- **C4 Controls along the top**: the sidebar heading band (Trash, Templates, New, Sort, count), the filter row (status
  segments, host chip), the marked-sessions bar, the attention header, the Hosts panel header, the pane's top row
  (actions, text size, tabs, tunnels pill), main-pane banners, the version readout, the Linux drawn title bar: **below
  first everywhere, 28 px gap**; above only when there is no room below.
- **C5 Dialog footer buttons and question answers** (every dialog's footer buttons, every anchored question's answers
  and default cancel): **below the button, away from the content**; above when there is no room below. This is the same
  placement as C4.
- **C6 Dialog close ×**: unchanged (help above lands over the dimmed window).
- **C7 Form controls in dialogs and Settings** (fields, switches, radio segments and cards, selects, checkboxes): keep
  today's placement (above).
- **C8 Lists inside dialogs and popovers** (launcher suggestion lists: search results, model, host picker, folder and
  repository lists; quick switcher rows; Templates list; Trash rows; Settings recipe and credential lists): **beside the
  list's edge**, notch and ring; today's rule when there is no room. No flip to the left side for these.
- **C9 Small marks outside lists** (details bar, approval card marks, path chips): unchanged.
- **C10 Tall controls** (sidebar divider, terminal history scrollbar, approval card scroll boxes): **at the pointer's
  height, beside the control** (the divider's help to its right, the scrollbar's to its left, a scroll box's to its side
  with room).

### Acceptance criteria

1. Every class above behaves as picked, for pointer hover and for keyboard focus alike: arrowing through a menu shows
   each item's help beside the menu, not over the item just left.
2. The Help menu's Announcements help no longer covers any menu item; a golden scene shows it (an open Help menu with
   Announcements' help showing beside the panel).
3. Session rows behave exactly as today.
4. A control asking for beside placement that is not inside any help edge falls back to the ordinary rule (today such a
   control's help goes to the sidebar's edge, away from it; `HoverHelp::beside`'s docs warn of this).
5. SPEC.md states the standard in product terms (where help appears relative to what it describes, per class, and the
   principle that help never covers a neighbour in a stack the user is scanning), replacing the sentence quoted above;
   SPEC_impl.md states the mechanism and numbers. Each PR carries the spec text for the behavior it ships, so SPEC.md
   never describes behavior main does not have.
6. Every hover-help site in the app falls under one of the classes. Any site the executor finds that fits none is placed
   by the closest class and listed in the report.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning pages:

- **The ten placements above**, picked per class on full-size mockups.
- **Keyboard-focus help follows the same placements** (stated on the planning page).
- **C6, C7 and C9 stay as they are**, and the standard says so.
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Release note:** the maintainer wrote their own entry; it is reproduced verbatim under `## Release note` below.
- **Scheduled as `[pending]`**, with no dependency on other queued plans.

Planner decisions, after an independent planning review (yours to revisit only through the scope reassessment):

- **Menu items get the notch but no ring.** A hovered or arrowed menu item already wears the accent fill (the pointer
  moves focus in a menu), so a ring adds nothing; the maintainer's C1 pick and its mockup show the notch only. C2, C3,
  C8 and C10 keep the ring the session-row rule draws (C10's control is its own target, so ring it only if that reads
  well; log the choice).
- **The flip to the left side is C1's only.** C8 falls back to today's rule, as picked; the sidebar and rail never have
  room on their left.
- **Main-pane banners count as top-band (C4)**, so their help goes below them.
- **C10 under keyboard focus** has no pointer: center the help on the visible part of the focused control instead.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the picker note, and the picks above.

**Binding repository constraints:**

- SPEC.md's session-row hover paragraph (the precedent this plan generalizes) and its sentence pinning other tooltips'
  placement, which this plan amends; SPEC_impl.md's layer manager section and its component rule that every control
  carries hover help (a constructor's description counts as hover help only where SPEC_impl.md says so).
- `.agents/validation.md` for GUI changes (the `farhelm-gui` test selections, clippy with `-D warnings`, which rejects
  dead code, and the screenshot goldens).
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and
  `.agents/test-authoring.md`.

**Planner proposals** (verified against the code and reviewed by an independent planning reviewer):

- **Generalize the one help edge into several, and keep `.beside()` as the only opt-in.** Today
  `LayerStack::beside_edge` holds one edge per window ("a second edge drawn in the same window replaces the first"). Let
  any number of edges be drawn with the same `beside_edge` pattern (an `Anchor` plus its tracker), and redefine
  `.beside()` as "beside the nearest help edge around this control". The sidebar keeps its current call, so session rows
  change nothing. The rejected alternative (from the inventory) was a hint carrying its own `Anchor`: every menu, list
  and rail site would have to mint an anchor and pass it into each control (`layers::panel(cx)` has no window or key, so
  menu panels cannot make one themselves), and the sidebar edge would remain a second way to name an edge.
- **How "nearest" is found is yours to decide, after trying it** (a close call; log it as a DECISION): (a) geometric,
  the smallest drawn edge containing the control's visible center (its tracked bounds intersected with its tracked clip,
  both already recorded), with a stated tie rule for overlapping layers; or (b) tree-scoped, an edge in force while its
  children prepaint, recorded by the control's tracker the way the clip is (`window.content_mask()` in
  `HelpTarget::track`), which needs one small wrapper element.
- **The overlay draws one edge around every popover** (where it wraps popover content before `PlacedPopover` in
  `layers.rs`), so every menu, select list, the notifications panel, the bell list and the tunnels list get an edge with
  no per-panel edits; `MenuItem` (and so select choices) defaults to beside. The panel header and footer buttons in the
  notifications panel, the bell list and the tunnels list opt in with `.beside()` per site.
- **The rail draws one edge** (one site) for C3. C8's in-tree lists (quick switcher, Templates list, Trash rows, the
  Settings recipe and credential lists; about five) draw their own; the launcher's suggestion lists are popovers and get
  the overlay's edge. `OptionRow` (and `ListRow` where used for these lists) defaults to beside in the C8 PR, not
  earlier, so no PR flips a default a later PR sets.
- **One beside placement function**, generalizing `beside_sidebar`: an edge's bounds, a side (right; right then left for
  C1; left for the scrollbar), and a vertical center (the control's, or the pointer's for C10), plus the mirrored notch
  drawing (`beside.rs` draws the notch only on the card's left edge today). C10 is "beside, with the control itself as
  the edge, at the pointer's height", set inside `components/divider.rs`, `scrollbar.rs` and `scroll_box.rs`.
- **One below-first variant for C4 and C5** (the ordinary rule with its preferences swapped, `GAP_BELOW` 28), as a knob
  on `HoverHelp` beside `.beside()`, not on each component's builder. It is set centrally where possible (anchored
  question answers in `prompt.rs`, `DialogFrame`'s footer) and per site elsewhere (about 15 sidebar-heading and
  panel-header controls; the pane's top row and the Linux title bar already show help below, but mark them so the rule
  holds if the layout changes).
- **Focus parity:** the overlay's `Against::Focused` arm dispatches on the hint's placement instead of always calling
  `around_target`; beside already wins there, so C1's arrow keys are fixed by the beside default.
- **Correct the doc nit** in `layers/beside.rs`'s module docs (they say the fallback measures the card at its maximum
  width; the code uses its laid-out width) in the PR that touches that module first.

## Outline

A stack of five PRs, each a `fix:` with its spec text, tests and goldens. Build each piece of mechanism in the PR that
first uses it; no mechanism-only PR (clippy rejects dead code, and shapes get reworked when the first user lands).

1. `plan/tooltip-placement-audit/01-menus`: several help edges and the nearest-edge rule; the overlay's popover edge;
   the generalized beside function with the right-then-left flip and the mirrored notch; `MenuItem` defaulting to beside
   with no ring; the per-site `.beside()` on the notifications panel, bell list and tunnels list buttons; focus parity;
   SPEC.md's sentence replaced by the start of the standard (C1 and the principle) and SPEC_impl.md's numbers; the new
   Help menu golden; the changelog fragment (`kind: fixed`), whose entry later PRs in the stack amend. Test that an open
   submenu (the Mute item's) is not covered by its trigger's help.
2. `plan/tooltip-placement-audit/02-sidebar-column`: the rail's edge; C2 and C3 sites opt in with `.beside()`; spec text
   for both.
3. `plan/tooltip-placement-audit/03-below-first`: the below-first knob; C4 and C5 sites; focus parity for it; spec text.
4. `plan/tooltip-placement-audit/04-dialog-lists`: `OptionRow`/list rows default to beside; the in-tree list edges; C8
   spec text.
5. `plan/tooltip-placement-audit/05-tall-controls`: C10 in the three components, its focus fallback; the standard's
   remaining text (C6, C7, C9 unchanged, stated); a final sweep that every hover-help site falls under a class; removes
   this plan's TODO.md entry.

Tests: pure placement tests in `layers/beside_tests.rs`; behavior tests in `components/tests/hover_help.rs`; surface
tests asserting `TooltipSide` (grep `shown_tooltip` and `TooltipSide` per PR, since about 27 test files read the shown
tooltip); goldens that capture tooltips (`tooltip`, `tooltip-paragraphs`, `tooltip-beside`, `session-row-hover-beside`,
`session-summary-card`, `version-readout-tooltip`) plus the new ones each PR adds. The docs screenshots
(`scripts/docs-shots/`) mostly avoid hover help; if one shows help whose placement changes, say so in the report rather
than recapturing (publishing docs screenshots belongs to a docs refresh, `.agents/docs-media.md`).

## Fallbacks

- If publishing updated GUI goldens is blocked because the executing machine lacks push access to the private golden
  storage repository (`scripts/gui-goldens.py` says where), keep the updated captures verified locally and block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), rather than landing failing goldens.
- If the nearest-edge rule cannot tell overlapping layers apart reliably (a select list over a dialog's list, say), use
  the tree-scoped variant, or prefer the edge drawn in the higher layer, and log it.
- If a site does not fit any class, place it by the closest class and list it in the report.
- If C10's pointer-height placement needs pointer tracking the overlay does not have, place at the control's visible
  center for both hover and focus, log it, and say so in the report.

Decisions to log: the nearest-edge rule, every site placed by judgment, the ring choice for C10, every golden changed
per PR and why, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tooltip-placement-audit-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-tooltip-placement-audit-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

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
  `plan/tooltip-placement-audit/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it (a user-visible fix is `fixed`, not `none`, even when the release note answer is "No
  release note needed."). A later PR in this stack that amends work an earlier PR of this stack built edits that PR's
  fragment instead of adding a second one. Where what got built departs from a release note entry (a renamed control, a
  part left out, a limitation it does not mention), the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- GUI drawing changes update the screenshot goldens in the same PR, as `.agents/validation.md` describes:
  `scripts/check-gui-goldens.py --update`, look at every changed or added picture its report names before committing,
  then `python3 scripts/gui-goldens.py publish` and commit the rewritten manifest lines. A golden change you did not
  expect is a finding to explain or fix, not a picture to accept.
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
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

Took a pass over all hover tooltips in the UI - various hovers should now be showing in a much less annoying position.
In general, less likely to cover something you care about while hovering.
