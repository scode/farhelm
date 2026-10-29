# UI: menus highlight one item at a time

Written against main at 1f23e648 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Fix menu highlighting.** In the host, session and restart menus, the first item stays highlighted when the pointer
>   moves over another item, leaving two items highlighted. Check the other menus for the same problem and fix the
>   shared cause if there is one. Plan: `plans/queue/menu-single-highlight.md`.

### The cause, found while planning

Every `role="menu"` in the UI shares one item style that fills an item on `:hover` and on `:focus` alike
(`crates/farhelm-ui/assets/app.css`: the `.session-row-menu-item` / `.host-row-menu-item` hover-and-focus rules in "Row
actions and confirmations", their icon and description companions above "Shared menu and dialog anatomy", and the
`.menu-item` rules in that section). Every menu also opens with keyboard focus on its first item, including when opened
with a pointer (`MenuOpenIntent::First` at each toggle's `onclick`; SPEC_impl.md's session-menu keyboard paragraph:
"opening it (pointer, Enter, Space, ArrowDown) lands focus on the first command"). So after a click-open the first item
is filled because it has focus and the item under the pointer is filled because it is hovered.

The menus that share this, as of 1f23e648: the session row menu (`crates/farhelm-ui/src/list/row.rs`), the host row menu
and the rail avatar's host menu (`crates/farhelm-ui/src/hosts.rs`), the session header's restart and replace split menus
(`crates/farhelm-ui/src/session_view/action_menu.rs`), the sort menu (`crates/farhelm-ui/src/list/sort_menu.rs`), the
`?` and update menus (`crates/farhelm-ui/src/app_bar.rs`), and the template number-key menu
(`crates/farhelm-ui/src/list/templates.rs`). All of them use the shared mechanics in
`crates/farhelm-ui/src/menu_panel.rs` and the page-side guard in `crates/farhelm-ui/assets/menu-focus.js`. Recheck this
list against main when you start.

### What changes

Moving a mouse or pen pointer onto a menu item moves keyboard focus to that item, the way native macOS menus behave.
Inside a menu, the fill then marks the focused item only, never merely the hovered one. The menu has one current item
and one highlight, and that holds in both directions: hovering moves the highlight to the hovered item, and an arrow key
pressed while the pointer rests on an item moves the highlight away from it. Because the hovered item is the focused
item, an arrow key after hovering steps from the hovered item, and Enter runs the item that is highlighted. When the
pointer leaves the open menu without clicking, focus and the highlight stay on the last hovered item.

Dropping the hover half of the fill inside menus is what makes the second direction hold: with hover moving focus but
the hover fill kept, a pointer resting on item A while ArrowDown moves focus to B would leave both filled, the reported
bug in reverse. It changes which item is filled, not how a fill looks, so it is within the decision below. It also has
to cancel the generic `.btn:hover` background on menu items, or that shows through on a hovered but unfocused item, and
it applies only to items inside `role="menu"`: the tunnel popover's "Close all" button reuses `.menu-item` outside any
menu and keeps its hover fill.

### What does not change

Opening still lands focus on the first item (ArrowUp on the toggle still opens onto the last); arrows still step and
wrap; Home/End still jump; the menu is still one tab stop; Escape and Tab still close and hand focus back to the toggle;
`aria-disabled` items stay focusable, refuse on activation and never gain a fill (hovering one moves focus there,
exactly as arrowing onto it does). Touch taps behave as today. Menu placement, the outside-click dismissal, the session
row's confirmation prompts, and every item's action are untouched. The pointer moving over a menu never scrolls
anything.

### Acceptance criteria

- In every menu listed above, after a pointer click opens it and the pointer moves onto another item, exactly one item
  has the highlight fill, and it is the hovered one. Browser tests check this by computed style on Chromium and WebKit,
  at least for the three menus the entry names (session, host, restart), and fail on main before the fix.
- After hovering an item, ArrowDown focuses the next item after it and ArrowUp the one before it; Enter activates the
  hovered item. With the pointer still resting on the item it hovered, after ArrowDown exactly one item has the fill,
  the newly focused one.
- Opening by keyboard still focuses the first (or last) item; with the pointer resting outside the menu, the keyboard
  behavior is unchanged.
- Hovering an `aria-disabled` item shows no fill on any item, and activating it still refuses.
- A pointer moving across items while a menu focus request is still crossing the desktop bridge never runs a command.
  Where focus ends up in such a race is covered by SPEC.md's accepted-race paragraph ("Confirmed 2026-10-17: when the
  user moves between controls ..."): in particular, the menu's opening focus request is deliberately ungated by the page
  guard (SPEC_impl.md), so a hover that lands before it can be undone by it. That is accepted; do not add the epoch
  check to the opening request to prevent it.
- The hover help on menu items behaves as pointer hover help when hovering moves focus: it still waits for the pointer
  to rest and still hides when the pointer leaves the item or the menu, even though focus stays on the last hovered
  item. A script focus made after keyboard use can match `:focus-visible`, which the tooltip treats as keyboard focus
  (no rest delay, no hide on pointer leave); that must not leave help showing after the pointer has left.
- SPEC_impl.md's menu paragraphs (the session menu's "Hover and focus fill each command" and its Keyboard sentence, and
  any other paragraph that describes menu focus or fills) and SPEC.md's split-menu keyboard sentence say that hovering
  moves focus; comments in `menu_panel.rs`, `menu-focus.js` and `app.css` that describe hover and focus as separate
  fills are updated.
- A changelog fragment (`kind: fixed`).
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-29, on a planning page that showed a before/after mockup (today: the first item
filled with "focus" and the hovered Stop filled with "pointer"; after: only Stop filled, with "pointer + focus"):

- **Hover moves focus.** Chosen over two alternatives the page described: a CSS-only rule hiding the focus fill while
  the pointer is inside the menu (rejected on the page because the keyboard would still sit on the first item, so
  ArrowDown after hovering would jump to the second item, and the focus ring would remain as a second marker), and
  filling only keyboard-originated focus (rejected because a keyboard-opened menu that is then hovered still shows two
  fills, and because whether the opening focus counts as keyboard differs between Chromium and the Mac app's WebKit).
- **When the pointer leaves the menu, the highlight stays on the last hovered item.** Chosen over clearing it, which
  would need a "focused but not highlighted" state in every menu.
- **Runs after `steel-sentence-case.md`** has landed, because that plan rewrites the labels in these menus and the
  browser tests that find menu items by label. Use whatever labels main has when you start.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No visual review gate.** The fix changes which item is filled, not how a fill looks; the browser tests check the
  fill directly.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scope**, stated on the page: the menus listed above. The launcher's suggestion lists and the quick switcher are
  listboxes with a separate selected style, not menus; check them for the same double highlight and fix them here only
  if the same shared cause applies, otherwise name what you found in the report.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC_impl.md's session-menu paragraph (the one containing "**Keyboard:** it is a
real `role="menu"`") and the menu-focus race design it relies on; SPEC.md's accepted-race paragraph (dated 2026-10-17)
and its split-menu keyboard sentence ("Split menus follow the app's shared keyboard rules"); root `AGENTS.md`, "Code
rules", "Tests and validation", "Conventional Commits", "Changelog fragments"; `.agents/test-authoring.md`;
`.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): one page-side listener rather than
a handler on every item; `pointermove`; mouse and pen only; `preventScroll`; matching items by `data-menu-focus`; the
fill following focus only inside `role="menu"`; one PR. A fresh-context planning review checked the outline against the
code and contributed the focus-only fill, the `pointermove` choice, the `data-menu-focus` match, the opening-request
race note and the tooltip check.

## Outline

The smallest design that fits the shared cause is one delegated listener on the page, installed once with the existing
menu-focus script, rather than a handler on each of the menus' items:

1. In `crates/farhelm-ui/assets/menu-focus.js` (installed once per page by `menu_panel::install_menu_focus`), add a
   capture-phase `pointermove` listener that, for a `pointerType` of `mouse` or `pen`, finds the nearest
   `[data-menu-focus]` element inside an open `[role="menu"]` and, if it is not already `document.activeElement`, calls
   its `focus({ preventScroll: true })` directly. Three details matter:
   - `pointermove`, not `pointerover`: `pointerover` can fire when content moves under a still pointer (the panel
     scrolling during arrow navigation, an item withdrawn by a refresh), which would pull focus back from the arrow
     keys. `pointermove` fires only on real movement, and still covers a panel that opens under a resting pointer once
     the pointer moves.
   - `[data-menu-focus]`, not `[role="menuitem"]`: the sort menu's items are `menuitemradio` and `menuitemcheckbox`.
     Every menu item already carries `data-menu-focus`, the script's own item identity.
   - A direct `element.focus()`, not the script's `__farhelmMenuFocus`: that function marks the move as app-owned, and
     this move must count as independent (next step). `preventScroll` because the menus' max-height panels can scroll,
     and a focus that scrolls would move the item out from under the pointer.
2. That focus arrives in Rust as an ordinary independent focus-in: the script's own `focusin` listener counts it as
   independent (it is not the app's `focusing` target), which bumps the epoch, so `record_menu_item_focus` clears
   `requested`, cancels unsent work, and sets `focused`, and the page guard refuses an older arrow-key request already
   in flight. That is the existing path for a pointer click or a screen reader moving focus, and the planning review
   confirmed that every item in all six menu files routes its focus-in through `record_menu_item_focus`. The opening
   request is the one exception the guard does not cover; see the acceptance criteria.
3. Confirm the roving `tabindex` follows (it reads `focused`), so Tab from a hovered item still leaves the menu as a
   single stop.
4. The hover help (`crates/farhelm-ui/assets/tooltip.js`) already ignores a second request for the item it is showing,
   so it cannot show twice. The risk is the other one in the acceptance criteria: a hover-made focus that matches
   `:focus-visible` is treated as keyboard focus, so the help would skip the rest delay and stay up after the pointer
   leaves. Test that case (keyboard step, then hover, then leave the menu) and fix it in the tooltip's own logic if it
   fails.
5. CSS (`crates/farhelm-ui/assets/app.css`): inside `role="menu"`, drop the `:hover` half of the item fill rules (the
   row-menu rules, their icon and description companions, and the `.menu-item` rules) and cancel the generic
   `.btn:hover` background, so the fill follows focus only; leave `.menu-item` outside a menu (the tunnel popover's
   "Close all") as it is. Update the comments that describe hover and focus as sharing one fill.
6. Tests: unit-test the new listener in `crates/farhelm-ui/js-tests/menu-focus.test.js` (mouse focuses, touch does not,
   an already-focused item is not refocused, nothing outside an open menu is touched). Add browser tests under
   `e2e/tests/` that open the session, host and restart menus with a click, hover another item, and assert exactly one
   item has the fill (computed background) and that it is the hovered one; then ArrowDown lands on the next item and the
   fill moves with it, with the pointer left where it was. The rail avatar's host menu opens by right-click or the
   ContextMenu / Shift+F10 keys, not a toggle click; open it that way, or test the host menu through the row's `⋯`. The
   existing helpers in `e2e/tests/helpers/session-actions.ts` and the menu specs (`restart-with.spec.ts`,
   `sort.spec.ts`, `header.spec.ts`) show how each menu is opened. Run the existing menu specs to show nothing else
   moved.
7. Spec and comments per the acceptance criteria; changelog fragment; remove the TODO.md entry.

If the delegated listener cannot work in one of the engines (the desktop webview included), adding the handler per item
through the shared `menu_panel.rs` helpers is the fallback within scope; log it as a DECISION.

Before touching these paths, read the instruction file root `AGENTS.md`, "Instructions by path", names for them:
`.agents/validation.md` and `.agents/test-authoring.md` for the tests.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the UI crate's tests, the JS unit tests that cover
`menu-focus.js`, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and, through
`python3 scripts/record-test-run.py`, the new browser tests and the existing menu specs on Chromium and WebKit. Run
`python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: changing what any menu item does, how menus open, close or are placed, or
the keyboard contract; changing the shared fill's look (making it follow focus only inside menus, as above, is not a
change of look); clearing the highlight when the pointer leaves; gating the opening focus request; touching listboxes
beyond the scope decision above.

Decisions to log: listener versus per-item handlers, the pointer event chosen, anything the tooltip needed, what you
found in the listboxes, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-menu-single-highlight-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/menu-single-highlight/<nn>-<short-name>`
  (the expected single PR: `plan/menu-single-highlight/01-hover-focus`). Split only if review shows a real seam.
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, cause, what changes and what does
not, acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
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
