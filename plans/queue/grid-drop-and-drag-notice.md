# Grid terminals: accept dropped files, and show the drag-copy notice

Written against main at 5e155da1 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

This plan covers two TODO.md entries (bucket "Near term"), merged into one plan at the maintainer's choice because they
change the same grid-terminal setup code, the same SPEC_impl.md sentence and the same docs list. Verbatim:

> **Dropping files onto grid terminals.** xterm.js terminals accept dropped files and images through the attachment
> handling that pasting uses; grid terminals (terminals in a terminal host) refuse a drop with a message saying to copy
> and paste instead. Left out of the terminal-host work by decision. Turning it on is mostly passing `acceptDrops: true`
> where the grid terminal installs the paste interception, plus tests on both browser engines.

> **The drag-copy notice in grid terminals.** In xterm.js terminals, a plain drag over a program that uses the mouse
> copies nothing and shows a notice saying how to select instead (Shift-drag, or Option-drag on a Mac). Grid terminals
> do not show it yet; left out of the terminal-host work by decision.

On top of the entries, the maintainer asked while planning to move the drag-copy notice so it stops covering what the
user just dragged over, in both kinds of terminal. In their words: "lets move the location of it i've noticed it's
really annoying. lets have it show up a little above the mouse pointer, we want to avoid covering the exact stuff you're
copying etc. let's say a few rows above (assuming there's space)".

NOTE: Both halves reuse what old terminals already have. Drops go through the shared attachment code; the notice reuses
its decision logic, wording, element and styling. Neither is a redesign, and the notice's text does not change.

### Acceptance criteria

- **Drops:** a drop on a grid terminal (the agent's or a tab's) behaves exactly as on an old terminal: files beat images
  beat text; files and images upload and their host paths are inserted as a paste; a folder is rejected with the
  existing visible message; dropped text is inserted like a paste; a disconnected terminal refuses. The "Dropping into
  this terminal is not supported yet; copy and paste instead" refusal, and the switch that produced it, are gone.
- **The notice on grid terminals:** a plain primary-button drag of at least the existing threshold over a grid terminal
  whose program has mouse reporting on, without the forcing modifier, followed by no clipboard write from the program
  within the existing grace window, shows the drag-copy notice, with the same text, the Codex-specific variant for a
  Codex agent terminal, the same 30 s lifetime, and the same dismissal rules (×, the next press in that terminal, a
  withheld link press dismisses and never raises it). A clipboard write from the program counts whatever its target, X11
  primary selection included, as on old terminals. A frozen grid (a takeover snapshot) never raises the notice.
- **Where the notice sits, in both kinds of terminal:** its bottom edge is 3 terminal rows (the terminal's own row
  height, so it scales with the font size) above the point where the drag was released, still centred horizontally on
  that point. When it does not fit there inside the terminal's pane, it goes 3 rows below the release point instead (its
  top edge 3 rows below). When it fits neither way (a very short pane), it falls back to today's placement, clamped
  inside the window. So it never covers the row where the drag was released, except in that last case; a drag spanning
  many rows can still have some of its earlier rows covered, which the maintainer's rule accepts. Judging "room" inside
  the terminal's pane is the planner's reading of the maintainer's "assuming there's space"; if the code makes the
  window the more natural boundary, use that and log the DECISION.
- **SPEC.md** "Attachments" (around 2398-2406) and the drag-copy notice passage (around 2291-2298) no longer say a new
  terminal lacks these, and the notice's position sentence ("just above the release pointer and clamped inside the
  window") describes the new placement. **SPEC_impl.md** around 1733-1734 drops drops and the notice from its "Not built
  for grid terminals" list; its notice implementation passage (around 1628-1643) matches the new placement.
- **Docs:** `website/src/content/docs/docs/using/new-terminals.md` (around 66-73) loses the two "not yet" bullets, and
  `website/src/content/docs/docs/using/work-in-a-session.mdx` (around 58-60 and 65-67) stops saying a new terminal
  refuses drops or lacks the hint.
- **Tests** on Chromium and WebKit prove drops on grid terminals (the attachment suite's drop gestures and its six tests
  that skip on grid terminals today) and the notice on grid terminals, and pin the new placement.
- **Changelog and TODO:** changelog fragments per root `AGENTS.md`, "Changelog fragments": one for drops (`added`), and
  for PR 2 two, since a fragment has one kind: the notice on grid terminals (`added`) and its new placement in old
  terminals (`changed`). The last code PR removes both TODO.md entries, including their `Plan:` references.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page:

1. **One plan, two PRs** (drops, then the notice), instead of two plans that could run concurrently and conflict.
2. **Remove the off switch** (`acceptDrops` and `DROP_UNSUPPORTED_TEXT` in `crates/farhelm-ui/assets/terminal.js`) and
   its message once grid terminals accept drops; nothing else turns drops off.
3. **Exact parity for drops** with old terminals, including dropped text and the folder refusal; the six tests that skip
   on grid terminals today run there too.
4. **"The program copied it itself"** follows the old terminals' rule: any clipboard write from the program within the
   grace window, primary-selection writes included. The grid terminal's clipboard callback must therefore report primary
   writes to the notice logic even though it does not copy them.
5. **Move the notice** so it does not cover what was dragged: 3 rows above the release point, or 3 rows below it when
   there is no room above (both were the recommended choices). The notice component is shared, so old terminals move
   too. The last-resort fallback for a pane too short for either is the planner's addition.
6. **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
   be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute
   reviewer.
7. **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two TODO.md entries, the maintainer's words on the notice's position quoted above, and the
decisions.

**Binding repository constraints:**

- SPEC.md: "Attachments" (around 2398-2406), the copy-on-select and drag-copy notice passage (around 2286-2300), and the
  withheld link press (around 2304-2311).
- SPEC_impl.md around 1628-1643 (the notice's implementation) and 1733-1734 (the grid gaps).
- Root `AGENTS.md`: "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation",
  "Instructions by path" (`website/AGENTS.md` before the docs edits), and "Frozen material". The released `CHANGELOG.md`
  sections that mention drops being refused (v0.34.0) or the notice ("once per page", which is already out of date) are
  frozen; leave them.
- The docs Overview page (`website/src/content/docs/docs/index.mdx`) says "Paste or drop one into the terminal", which
  becomes true again; it is the maintainer's page and stays untouched.
- `.agents/test-authoring.md` for the test changes, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- PR 1 `plan/grid-drop-and-drag-notice/01-grid-drops`: drops on grid terminals, the switch and message removed, the
  attachment tests switched, the SPEC/SPEC_impl/docs sentences about drops, its fragment.
- PR 2 `plan/grid-drop-and-drag-notice/02-grid-drag-notice`: the notice on grid terminals and its new placement for both
  kinds, its tests, the SPEC/SPEC_impl/docs sentences about the notice, its fragment, both TODO.md entries removed.
- For the notice, add nothing to the grid component. Everything the old terminals' driver reads has a public grid
  equivalent, so wire it in `adoptGrid` in `terminal.js`, which the grid file's header names as the owner of clipboard
  policy: a capture-phase press listener on `grid.element` (which also sees scrollbar presses, for dismissal), mouse
  tracking from `grid.model.state.mouse`, a write counter in the existing `onClipboard` callback (counting primary
  writes), a flag set in the existing `linkPress` callback for withheld presses, and a document mouseup listener. Lift
  the old terminals' driver (around 5832-5925) into a small function both paths call if that removes duplication; logged
  either way.
- A frozen grid needs no detection mechanism. A grid is frozen only during the session terminal's teardown, which runs
  `stopCopyOnSelect` first (`terminal.js` around 6270, then the freeze around 6313); that slot is null for grids today
  (around 5381). Put the grid notice's teardown there, clearing the grace timer, and a frozen grid has no notice
  listeners left.
- No Rust changes are expected: the notice element and its inputs (`notice`, `copyHint`) are already rendered for both
  terminal kinds.

## What the code looks like at 5e155da1

Line numbers are approximate; verify before relying on them.

**Drops, shared code** (`crates/farhelm-ui/assets/terminal.js`):
`installAttachments(spec, baseUrl, policy, pasteText, dismissSelectionSoon, isConnected, acceptDrops = true)` around
2109, documented around 2061-2108 and in the file header around 242-266. `DROP_UNSUPPORTED_TEXT` around 2052-2059 is
used only by `onDrop` (around 2682-2712), which shows it and returns when `!acceptDrops`; otherwise it applies the
precedence table from Rust, uploads files and images one at a time and inserts the shell-quoted host path
(`accept`/`send` around 2584-2650), rejects folders, inserts text, and refuses on a disconnected terminal. `onDragOver`
(around 2717-2721) sets `dropEffect` to "none" when drops are off. All insertion goes through `insert()` (around 2235).
A page-wide drop guard (around 2930-2970) treats both kinds the same.

**Where the grid turns drops off:** `adoptGrid()` (around 5299-5379) calls
`installAttachments(..., (text) => grid.paste(text), () => grid.clearSelection(), () => ws.readyState === WebSocket.OPEN, false)`;
the trailing `false` is around 5378. Comments that say grid drops are not built: around 35-41, 2052-2074, 2684-2720,
5292-5294, 5367-5370. Grid insertion goes through `grid.paste()` (`crates/farhelm-ui/assets/grid-terminal.js` around
1427), which splits text into paste messages the supervisor brackets (`crates/farhelm-supervisor/src/grid_input.rs`
around 44-47, 221-250); grid paste already uses this for copied files. `grid.paste()` returns `false` on a frozen grid
and `insert()` ignores that. That cannot lose a dropped file's path: teardown disposes the drop handling (aborting
uploads) and closes the socket before freezing, and the frozen screen is moved out of the pane element, so a drop on it
reaches the page-wide guard. Confirm this and log it; build no handling for it.

**The desktop app** already disables the native drop handler (`.with_disable_drag_drop_handler(true)`,
`crates/farhelm-ui/src/desktop.rs` around 187-264), so drops reach the page as DOM events; no shell change.

**The notice on old terminals:** decision and wording in `crates/farhelm-ui/assets/copy-on-select.js`:
`dragMayHaveCopiedNothing` (around 163), `pressForcesSelection` (around 184), `forcingModifier` (around 117),
`isOsc52Write` (around 196), `OSC52_GRACE_MS = 1500` (around 134), `dragCopyNoticeText` (around 209),
`DRAG_NOTICE_MS = 30_000` (around 219), and `createDragCopyNotice` (around 234-306), which places the notice with
`placeTooltip`, centred horizontally on the release point and clamped 6 px above it. That placement is what the
maintainer finds annoying. The driver is in `terminal.js` around 5812-5998: the OSC 52 write counter (around 5828), the
press handler (around 5846), `showDragCopyNotice` (around 5865), the mouseup decision (around 5894), withheld presses
(around 5935-5952), teardown `stopCopyOnSelect` (around 5980). Codex's hint: `drag_copy_hint` in
`crates/farhelm-ui/src/lib.rs` around 350. The notice element is rendered by Rust per terminal
(`crates/farhelm-ui/src/session_view.rs` around 1662-1663, 1689, 2896, 2940, id from `drag_copy_notice_element_id`
around 3117) and styled in `crates/farhelm-ui/assets/app.css` around 7600-7640.

**The grid's mouse handling:** `grid-terminal.js` around 1541-1725. The press (around 1677) computes `forced` via
`copyOnSelect.pressForcesSelection` and `tracking = model.state.mouse !== "off"`, then sets `reporting`, `selecting` and
`withheld` (around 1694-1705); `onDocumentMouseUp` (around 1590) ends the gesture. Shift/Option-drag selection already
works (`e2e/grid-client/grid-terminal.spec.ts` around 895). The grid's own press handler is a plain listener on the
container, and the scrollbar (around 970-974) stops propagation, so that handler misses scrollbar presses too; a
capture-phase listener on `grid.element` sees them. Program clipboard writes reach `terminal.js` through
`onClipboard(text, target)` (around 1900); the callback in `adoptGrid` ignores `primary` writes, so decision 4 needs it
to count them. Grid clipboard messages arrive only once a write completes and pass through the supervisor and helm. The
sender keeps the latest write per target and drops only writes over 8 MiB and writes to selectors the grid does not
report (`crates/farhelm-proto/src/grid.rs` around 206-225, 297); a dropped write can raise a false notice, which is
acceptable guidance-only behavior, but say so in the report. `stopCopyOnSelect` in `adoptGrid` (around 5359) disposes
the old notice instance when the grid takes over.

**Tests:** `e2e/grid-client/grid-attach.spec.ts` (fake socket, both engines): the drop refusal test (around 567-578)
becomes a positive test, the paste test (around 544-565) is the pattern, `dispatchPayload` (around 513) builds stand-in
drag data because WebKit cannot build a populated one; it passes `notice: null, copyHint: null` (around 209) and can
host deterministic notice tests (grace window, primary target, withheld presses).
`e2e/tests/terminal-attachments.spec.ts` runs in the host-terminal suite (`FARHELM_E2E_SUITE_TERMINALS=host`, listed in
`e2e/host-terminal-specs.ts`): its `FILE_GESTURE` (around 278-285) switches grid terminals to paste; six tests skip on
grid terminals (around 776, 822, 1296, 1392, 1794, 2216); the bracketed-paste test (around 741-748) leaves out the drop
leg. `e2e/tests/mouse-modes.spec.ts` holds the notice tests (around 443-696; generic around 528, Codex around 647),
skipping on terminal hosts (around 535, 654), with xterm-only helpers (`waitForMouseTracking` around 485,
`dragAcrossTerminal` around 500, the input check around 591) whose grid equivalents are in `e2e/tests/helpers/term.ts`
(around 42, 71, 225). JS unit tests: `crates/farhelm-ui/js-tests/copy-on-select.test.js` (around 128-314).
`e2e/host-terminal-specs.ts` around 16-17 and `.agents/validation.md` around 627 ("drop refusal") name the gaps; update
them.

Keep the test changes small. `mouse-modes.spec.ts` already runs in the host-terminal suite, so un-skipping its two
notice tests runs them on both kinds; the placement test is an edit to its existing position assertion (around 582),
with the drag point chosen deliberately (the current one, around 502, would land in the below branch), and the
`copy-on-select.js` unit tests pin the above/below/fallback arithmetic exactly. The one non-mechanical conversion is the
input check (around 588-591), which uses `__farhelmTerm.onData`, which grid terminals lack. Grid-client specs only need
what the shared tests cannot reach: a primary-selection write suppressing the notice, and a withheld link press. In
`terminal-attachments.spec.ts`, `FILE_GESTURE` can simply be deleted; explicit paste tests still cover file pastes.

**Docs details:** after PR 2, `new-terminals.md`'s "A few things do not work in a new terminal yet" list is down to the
input-method bullet; reword its lead-in. `work-in-a-session.mdx` says the hint shows where you let go; make it match the
new placement.

**Manual check:** `crates/farhelm-ui/src/attachments.rs` around 56-104 is the manual desktop checklist for drops. A real
drag from Finder into the Mac app cannot be done unattended; the report lists it as a manual check for the maintainer on
a grid terminal (a file onto the agent terminal, a file onto a tab, a folder), and the checklist's wording is updated to
cover both kinds.

## Outline

1. `plan/grid-drop-and-drag-notice/01-grid-drops`: remove the trailing `false` and the switch and message from
   `installAttachments`/`onDrop`/`onDragOver`; the comment sweep; the attachment suite's drop gestures for grid
   terminals, its six skipped tests, the bracketed-paste test's drop leg, the grid-client refusal test turned positive;
   SPEC.md "Attachments", SPEC_impl.md's list, the two docs pages' drop sentences, `host-terminal-specs.ts` and
   `.agents/validation.md`; the manual checklist; the fragment.
2. `plan/grid-drop-and-drag-notice/02-grid-drag-notice`: the notice wiring in `adoptGrid` (listeners on the grid's
   element and document, counting primary writes, teardown in the copy-on-select slot); the new placement in the shared
   notice component; the notice tests above; SPEC.md's notice passage, SPEC_impl.md, the two docs pages, the file
   headers; the two fragments; both TODO.md entries removed.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `dprint check` on the changed files, and the JS unit tests for `copy-on-select.js`;
- `cargo fmt --check` and clippy on `farhelm-ui` if any Rust changes;
- `python3 releasing/check-changelog.py format`;
- `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares;
- `cd website && bun run build` for the docs pages;
- through the recorder, on Chromium and WebKit: `e2e/grid-client/` specs that change, and `terminal-attachments.spec.ts`
  and `mouse-modes.spec.ts` both in the default suite (old terminals, which the placement change touches) and in the
  host-terminal suite, after the web build `.agents/validation.md` describes.

## Fallbacks

Examples of departures that need the scope reassessment:

- any change to the notice's text, its 30 s lifetime, its grace window or its dismissal rules;
- a rate limit on the notice;
- native (desktop shell) drag-and-drop handling;
- Rust protocol or supervisor changes for clipboard reporting.

Decisions to log: whether the driver is shared, the exact placement arithmetic and how row height is measured, the
boundary used for "room", every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-grid-drop-and-drag-notice-log.md` in the parent directory of the checkout you run in, derived as that
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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

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

- A linear stack of reviewable commits, one stable bookmark and draft PR each, using the `jjstack` skill. The stack's
  base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/grid-drop-and-drag-notice/<nn>-<name>`,
  as the outline names them.
- Avoid churn: code added in one PR and deleted in a later PR of the same stack means the stack should have been shaped
  differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and the changelog fragment in the same commit as the change.
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
already made and outline). These PRs change tests, so include the full text of `.agents/test-authoring.md` verbatim in
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
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. The
changed docs pages go there too, named by their paths, in place of a preview link, along with the manual Finder drag
check.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that meets the acceptance criteria above, each PR passed
its review gate, and the last code PR removes both of this plan's TODO.md entries. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
