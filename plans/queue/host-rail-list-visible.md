# Host rail: tooltips instead of hover widening, and a button that expands the rail

Written against main at d25daf0d on 2026-10-30. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

One TODO.md entry (bucket "Near term"), verbatim:

> - **Revisit the host rail's widened names taking no clicks.** Decided on the maintainer's behalf on 2026-10-29 when
>   the host-short-names landing failed: the rail's widened part, which shows full host names over the session list, is
>   display only. It takes no clicks and does not keep the rail open, so the rail widens only while the pointer is over
>   the narrow rail or keyboard focus is inside it, and picking a host with a click no longer leaves the list's sort
>   control covered. The other option was to let the widened names be clicked to pick a host, which needs the rail to
>   stay open while the pointer is over them and some other way to reach the controls underneath. Plan:
>   `plans/queue/host-rail-list-visible.md`.

The maintainer's words while picking it: "I was on the fence on this, but now I'm very convinced because once I actually
use the interface, I'm realizing it's incredibly annoying the way it currently works. The host buttons are meant to be
used as filters, but the thing you're filtering, which is a session list, isn't even visible when you do. It's already
really annoying and broken, so we want to fix this." And: "It also looks visually weird now because the top of the rail
expands, but not the entire rail at the bottom."

Offered four options (clickable names with the rail collapsing on a pick; no widening with full names in tooltips; a
widened rail that pushes the list aside; a button that pins the rail open), the maintainer answered: "how about we do
the hover tooltip AND there is a button to fully expand the rail." They then approved full-size mockups of that design,
described below, without changes.

So the rail stops widening on hover and on keyboard focus altogether. Full names move into each avatar's tooltip, and a
button in the rail's foot expands the whole rail, foot included, into a labeled host list that sits beside the session
list rather than over it.

### What changes on screen

- **Collapsed (the default, and what every device starts with).** The rail is 56px wide, with avatars and their
  short-name captions exactly as today. Nothing widens when the pointer rests on the rail or keyboard focus moves into
  it. Each host avatar's tooltip names the host's display name, its destination when that differs from the name, and its
  state (today's tooltip carries only the name and the state words, plus the existing "last state read" and "needs
  attention" suffixes, which stay).
- **The toggle.** A new button is the last item in the rail's foot, below Hosts, Settings and Help. Collapsed it shows a
  » icon with the tooltip "Expand host rail"; expanded it shows « with the label "Collapse" and the tooltip "Collapse
  host rail". It is an ordinary button with `aria-expanded`, reachable by keyboard like its neighbors.
- **Expanded.** The whole rail, foot included, is 220px wide (164px more than collapsed) and reads as a labeled list:
  each host row shows its avatar, the display name and, underneath in smaller text, the destination when it differs. All
  hosts and Add host show their labels; Hosts, Settings and Help show their labels beside their icons. Captions under
  the avatars are not shown while expanded. Each host row is a single click target: clicking anywhere on it, name
  included, picks that host, and the rail stays expanded. The selected host's whole row takes the selection fill, and a
  hovered row highlights as a whole. A name too long for the row truncates with an ellipsis, and the tooltip carries it
  in full. Right-click and keyboard host menus, state badges, update arrows and the selected state behave as on the
  collapsed avatars.
- **Where the width comes from.** The session list keeps its width: the sidebar as a whole grows by the rail's extra
  164px, and the session pane beside it gives that width up. A sidebar the user has dragged wide keeps its width plus
  the 164px; when the window cannot fit that and still leave the session pane its minimum width, the sidebar grows only
  as far as the window allows and the list narrows by the rest. Collapsing gives the width back.
- **Remembered per device.** Each device remembers whether its rail is expanded, the same way it remembers the sidebar
  width (in the page's local storage, nothing sent to the helm), so every client of a helm can choose differently. A
  fresh device starts collapsed.
- **Touch screens** behave like any other device. The touch-only exception added at the last landing (no hover widening
  on devices that cannot hover) disappears along with hover widening.

The maintainer saw mockups of exactly this (the collapsed rail with a tooltip beside the gpu-01 avatar and » under Help;
the expanded rail at 220px with labeled rows, the foot labeled down to « Collapse, the sidebar grown and the terminal
narrowed). The page lives outside the repository; the description above is the agreed design.

### What does not change

The collapsed rail's width, avatars, captions, order, badges, menus and scrolling. Host choice semantics (choosing a
host filters the list server-side; All widens it again). The short-name rule and every other place host names appear
(the Hosts panel, session rows, the bottom bar, dialogs). The sidebar's resize handle and its stored width's meaning.
The Hosts, Settings and Help controls' behavior.

### Acceptance criteria

- The rail never widens on hover or on keyboard focus, on any engine or device class.
- Host avatar tooltips (and their accessible names, which use the same text today) carry display name, destination when
  different, and state.
- The toggle expands and collapses the rail and its foot together, as described, in Chromium and WebKit; while expanded,
  clicking a host's name picks the host and leaves the rail expanded; the session list's width is unchanged by expanding
  when the window has room; the expanded state survives a reload on the same device; a fresh profile starts collapsed.
- SPEC.md's Session list rail paragraph and SPEC_impl.md's rail implementation paragraph describe the new behavior and
  drop the hover, focus and touch widening rules.
- A changelog fragment (`kind: changed`: the hover widening shipped in v0.32.0).
- The TODO.md entry quoted above is removed in the plan's last PR.

## Decisions already made

Settled with the maintainer on 2026-10-30 on a planning page with full-size mockups:

- **Design:** tooltips plus an expand button, in the maintainer's words above, built as the mockups showed. Not chosen:
  clickable names with the rail collapsing on a pick; a hover-widened rail that pushes the list; a pin button on its own
  without tooltips.
- **Width:** the sidebar grows and the session pane gives up the width ("The sidebar grows; the terminal gives up the
  width"), not "the sidebar keeps its width; the list narrows".
- **Toggle placement and labels, per-device memory, expanded rows as single click targets that keep the rail open, and
  the narrow-window fallback:** the planner's proposals, stated on the page beside the mockups and accepted.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **No docs page** is added; none describes the rail today.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words quoted above, and the decisions.

**Binding repository constraints:** SPEC.md, Session list, the host rail paragraph ("One flat list across all registered
hosts, with an always-visible host rail"); SPEC_impl.md's rail paragraph ("The rail keeps 40px avatar triggers at fixed
coordinates"); the stylesheet's token rule that use sites hold no literals (the `:root` block's comment in
`crates/farhelm-ui/assets/app.css`, enforced by the UI's JavaScript tests);
`crates/farhelm-ui/assets/sidebar-width.js`'s contract that sidebar width belongs to the device and is never sent to the
helm; root `AGENTS.md`, "Code rules", "Tests and validation", "Conventional Commits", "Changelog fragments";
`.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the page script owns the expanded
state and its root class; the existing `--host-rail-width` token is redefined under that class instead of adding a new
width token; the script sets `--sidebar-width` to the shown width; the full-row highlight is built with a 40px box that
keeps the badges and menu anchor in place; captions are removed from layout while expanded; one PR. A fresh-context
planning review checked the outline against the code and contributed the script ownership, the token redefinition, the
width cap that keeps the narrow-window classes untouched, the badge and menu-anchor problem with widened buttons, the
four-control foot geometry, and the SPEC sentences and tests listed below.

## Outline

Expected as one PR. If it splits, the only coherent line is (1) the tooltip destination plus removing hover and focus
widening, then (2) the toggle; each half leaves full names reachable. Do not split along file lines.

1. **Remove hover and focus widening** (`crates/farhelm-ui/assets/app.css`, the rail section, about lines 615 to 860).
   Delete the `.host-rail:hover` and `.host-rail:has(:focus-visible)` widening, the `(hover: hover)` blocks for the rail
   and its captions and names, the rail's `transition: width`, and `--host-rail-expanded-width` (in `:root` and in the
   `SHARED_TOKENS` list of `crates/farhelm-ui/js-tests/app-css-tokens.test.js`). With no widening, the display-only
   pointer-events pair (`pointer-events: none` on `.host-rail` and `auto` on its `.host-rail-hover` child) does nothing;
   delete those declarations but keep the child element, because "host rail scrolls from padding and gaps"
   (`e2e/tests/host-short-names.spec.ts`) still guards wheel scrolling inside the scroller. Renaming the child is
   optional churn; if it keeps its name, rewrite its comment, since the hover rationale is gone. Do not add a width
   transition to the sidebar either: the terminal resize observer sends every grid change without debounce
   (SPEC_impl.md, the `sidebar-width.js` paragraph), so each toggle reflowing the terminal once is the expected cost of
   the maintainer's width choice, not something to mitigate.
2. **Tooltip text** (`crates/farhelm-ui/src/hosts.rs`, `HostRailAvatar`). Add the destination when it differs from the
   display name, keeping the name first and the state after it (`filters.spec.ts` asserts the label starts with the
   alias followed by a colon for hosts without destinations; a shape like `"{name} ({destination}): {state}"` keeps
   that). Pull the formatting into a small pure function with a unit test beside the existing `rail_presentation` tests.
   The local host gets no destination suffix. The same string is the button's `aria-label`, so the accessible name gains
   the destination too.
3. **Expanded state and width** (`crates/farhelm-ui/assets/sidebar-width.js`, which already owns the per-device sidebar
   width, sets `--sidebar-width` and the narrow-window classes before the app mounts, and is shared by the web and
   desktop builds). Add a localStorage key for the flag and a root class (for example `host-rail-open` on `<html>`). The
   script owns the state; Rust does not hold a copy. Compute the shown width once at the top of `updateLayout()`: the
   stored width while collapsed, and while expanded the stored width plus 164px, capped at the window width minus 322px
   but never below the stored width. That cap matches the existing `width + 321` narrow-window cutoff, so expanding
   alone never switches `sidebar-narrow-window` or `sidebar-menu-overlaps` on; state that invariant in a comment. Feed
   the shown width into the existing `--sidebar-width` and class lines, so every consumer of `--sidebar-width` follows
   with no edits. Keep the stored total as the value the drag, double-click, arrow keys, `remember()` and
   `aria-valuenow` use, so the resize handle needs no change. One accepted rough edge: while the cap applies, dragging
   moves the stored width but not the visible edge until the window has room again; build nothing for it. The 164px
   lives in both the script and the stylesheet (220 = 56 + 164); cross-reference the two with a comment on each side
   rather than having the script read the CSS. Export pure helpers (flag parsing, shown-width math) for node tests in
   `crates/farhelm-ui/js-tests/sidebar-width.test.js`.
4. **The toggle button.** Render it in the rail's foot (`crates/farhelm-ui/src/app_bar.rs`, where `.host-rail-footer` is
   built) as the last control, with static collapsed defaults (`aria-expanded="false"`, "Expand host rail", the » glyph
   and the hidden "Collapse" label), and bind it from an `onmounted` hook through an idempotent bind in the page script,
   the way the sidebar resize handle is bound. Bind on the element, not with a document-level listener (the script's
   header avoids document-wide input listeners), and keep this bind separate from the resize handle's, since they live
   in different components. On click the script flips the root class, stores the flag, re-runs the layout, and sets
   `aria-expanded`, `aria-label` and `data-tooltip` ("Expand host rail" / "Collapse host rail");
   `crates/farhelm-ui/assets/tooltip.js` reads `data-tooltip` when it shows, so the update is picked up. Picking a host
   needs no code to keep the rail open: nothing collapses it except the toggle.
5. **The expanded layout** (app.css, plus label spans in Rust). Redefine the existing token under the open class,
   `html.host-rail-open { --host-rail-width: 220px; }` (`.macos-window` overriding `--sidebar-title-height` is the
   precedent). The rail, its scroller child, the foot, the content's left margin and the host-update card's margin then
   all follow. Do not write the selector as `:root.host-rail-open`: the token test's palette detection treats any
   top-level `:root` block that declares a token as a palette and fails. In the open state show the names and labels,
   and remove captions from layout (`display: none`) so rows are 40px with no dead strip. The mockup filled the whole
   selected row and highlighted the whole hovered row, so make each row one full-width target: the badges
   (`.host-rail-state`, `.host-rail-update`, `.host-rail-attention`, the Hosts toggle's update mark), the progress ring
   (`::after` with `border-radius: 50%`) and the host and Help menus' anchors are all positioned against today's 40px
   trigger, and the `HostRailAvatar` docstring promises they stay put. Keep them on a 40px box (wrap the icon and badges
   in a `position: relative` 40px element, or anchor menus to it), so widening the row moves none of them. Add
   `aria-hidden` label spans to All hosts, Add host, Hosts (rendered in `crates/farhelm-ui/src/list/view.rs` and passed
   to the app bar as `hosts_toggle`), Settings, Help (a `BarMenuToggle` in `app_bar.rs`, whose label is an extra child)
   and the toggle. Keep the foot controls at their current x. The foot grows from three controls to four:
   `.host-rail-footer { height: 128px }` and `.host-rail { bottom: 128px }` become 168px, and the update card's arrow
   (`.host-update-card::before { bottom: 94px }`, commented as the centre of the foot's first control) moves with the
   Hosts control; check the card's `min-height` too.
6. **Spec.** SPEC.md, Session list: the rail paragraph (drop the hover, focus and touch widening; the collapsed rail;
   tooltips; the toggle; the expanded layout; "the Hosts, Settings and Help controls at the foot of the rail stay 56px
   wide" inverts); the per-device settings list ("appearance, sidebar width, terminal text size and sound switches")
   gains the rail's expanded state; the sidebar-width paragraph (296 to 656px, the 240 to 600px session column, the
   320px main-pane floor) gains the expanded case. SPEC_impl.md: the rail paragraph ("The rail keeps 40px avatar
   triggers at fixed coordinates", including its accessible-name sentence, which now includes the destination) and the
   `sidebar-width.js` paragraph (the new key, and that `--sidebar-width` now carries the shown width rather than the
   stored total).
7. **Tests.** The hover and focus widening assertions live only in `e2e/tests/host-short-names.spec.ts` ("hover keeps
   icons fixed and releases the list..." and "keyboard expansion leaves sort usable..."); rewrite those as tests that
   hover and keyboard focus do not widen the rail, that the tooltip carries name, destination and state, that the toggle
   expands rail and foot together and collapses them, that clicking a host's name while expanded picks it and leaves the
   rail expanded, that the session list's width is unchanged by expanding when the window has room, that the state
   survives a reload, and that a fresh profile starts collapsed. `filters.spec.ts`'s "fits a narrow window" test stays
   valid and is the natural place for the narrow-window fallback assertion. `host-updates.spec.ts` checks the update
   card's arrow against the Hosts control's centre within 2px, so it catches a missed arrow edit; update it only as the
   geometry requires. Adjust `app-css-tokens.test.js` for the removed token. Do not rewrite the sort, Hosts panel or
   sidebar specs without a concrete failure. Use web-first waits on the toggle's state rather than timeouts; the 250ms
   hover dwell is gone, so nothing needs to wait for it.
8. **Changelog and TODO.** A fragment (`kind: changed`): resting the pointer on the rail no longer widens it over the
   session list; full names are in each host's tooltip and in the expanded rail, opened with a new button at the rail's
   foot. Remove the TODO.md entry quoted above.
9. **Documentation pass.** Rewrite the comments the old behavior justified: app.css's shell comment (about lines 570 to
   573), the rail comments around lines 617 to 622, 645 to 668 and 716 to 718 (the hover and touch rationale), the
   `HostRailAvatar` docstring (hosts.rs, about lines 667 to 669), and the `host-rail-hover` comment in hosts.rs (about
   line 2568).

The docs screenshots and the README hero and video will show the new foot button at their next refresh. Those paths are
fenced (`.agents/docs-media.md`); leave them alone and say so in the report.

Before touching these paths, read the instruction files root `AGENTS.md`, "Instructions by path", names for them:
`.agents/validation.md` and `.agents/test-authoring.md` for the tests.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy with `-D warnings` on
`farhelm-ui`, the UI's Rust tests and its JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`) through
`python3 scripts/record-test-run.py` per `.agents/test-evidence.md`, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, the UI's release web build, and on Chromium and WebKit through the
recorder the rewritten `host-short-names.spec.ts` plus the specs that touch the rail and its foot: `filters.spec.ts`,
`sort.spec.ts`, `hosts-panel.spec.ts`, `sidebar.spec.ts`, `host-updates.spec.ts`, `host-rail-warning.spec.ts`,
`settings.spec.ts` and the hover-text sweep. Run `python -B scripts/check-test-sleeps.py` with the isolated interpreter
in `docs/test-sleep-check.md` when Rust or browser tests change.

## Fallbacks

Departures that need the scope reassessment: any hover or focus widening, even partial; storing the expanded state in
the helm or sharing it between clients; a second width variable consumed beside `--sidebar-width`; animating the sidebar
or the rail; changing the resize handle's stored-width semantics; rewriting rail specs beyond those named above without
a concrete failure; changing the collapsed rail's layout beyond the foot's fourth control.

Decisions to log: whether the stack is one PR or two, how the full-row target keeps badges and menu anchors in place,
the tooltip's exact format, the root class and storage key names, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-rail-list-visible-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/host-rail-list-visible/<nn>-<short-name>`
  (expected: `01-rail-toggle`; if split, `01-rail-tooltips` and `02-rail-toggle`).
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, what changes on screen and what
does not, acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text
of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
