# Menus: a focus move already sent to the desktop page cannot pull focus back

Written against main at be722d41 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan.

## The goal

In the desktop app, a menu's arrow-key focus move is handed to the webview the moment Rust requests it, and Rust cannot
take it back. So a rapid arrow press whose focus move is already on its way can land after the user has moved focus
somewhere else, and pull focus back. The menu focus fix (#2232) discards moves that were not yet sent; this plan closes
the remaining case, where the move was already sent.

TODO.md entry this plan covers (Near term): "**Stop rapid menu arrow presses from pulling focus back.** In the desktop
app, a focus move already sent to the page cannot be recalled, so a rapid arrow press whose request was already on its
way can still pull focus back after the user moved it: Down twice, click the fourth item, Down can still end on the
fourth item when the second press's focus move was already sent. The menu focus fix (#2232) narrowed this and removed
the earlier TODO entry; this entry tracks the remaining case."

The agreed approach: every menu focus move that Rust requests goes through a small page-side function, which drops the
move if focus moved independently (by the user, not by the app's own menu focus) since the navigation key that caused
it. Rust stays in charge of the keyboard rules; the page only refuses stale moves.

### Acceptance criteria

- In the desktop app, after Down, Down, then an independent focus move to another item (or out of the menu), a menu
  focus move that was already sent no longer moves focus. The next arrow press steps from where focus actually is.
- The fix covers every menu that uses `crates/farhelm-ui/src/menu_panel.rs`: the session row menu, the host row menu,
  the sidebar's help and update menus, and the Templates number-key popup.
- A burst of arrow presses with no independent focus move still advances once per press, as today (the #2232 behavior
  stays).
- Focus requests that are not caused by a navigation key (the focus that lands on the first item when a menu opens) are
  not dropped by the new check, and the page does not count them as an independent move: they go through the same page
  function with an "ungated" flag, not through plain `set_focus`. Otherwise a fast second arrow press right after a
  keyboard open is dropped and the press after it skips an item. The focus handback to the menu's toggle needs nothing.
- Rust needs no handling of a dropped move: the user's focus event reaches Rust before the page runs the stale move (on
  desktop through the synchronous event bridge, on the web synchronously), so #2232's code has already cleared the stale
  target and cancelled queued work. Rust awaits the page call and may ignore its result; no request sequence number.
- The web UI keeps working as today (its focus moves are synchronous, so in practice the check never drops one there).
- A browser test reproduces the race deterministically by holding a focus move in flight (for example by wrapping the
  page-side function so a call waits until the test releases it), moving focus independently, releasing the held move,
  and asserting that focus did not move back and that the next Down steps from the independently focused item. JS unit
  tests cover the page-side check. No sleeps; see `.agents/test-authoring.md` and the test-sleep checker.
- SPEC_impl.md's row-menu "Keyboard" paragraph drops its exception sentence ("in the desktop app, an arrow's focus move
  that has already been sent across the webview cannot be recalled, so it can pull focus back…") or rewrites it to what
  remains true. The doc comments in `menu_panel.rs` that describe the race (for example on `cancel_menu_focus`) say what
  is true afterwards.
- A changelog fragment (`kind: fixed`); #2232's fragment already shipped in v0.30.0, so this is a new one.
- The last code PR removes the TODO.md entry.

**Complexity gate (the maintainer's):** if the fix turns out much bigger than a small focused change, stop and block the
plan with what was found, rather than carrying on. The maintainer's threshold, in the planner's words they accepted:
"well over ~150 lines or touching every menu's wiring". Interpretation (planner): count product code lines in Rust and
JavaScript, excluding comments, blank lines and tests; a line or two at each menu's call site (as #2232 needed), or one
marker attribute per menu item (below), is not "touching every menu's wiring", while restructuring how each menu builds
or focuses its items is. The planning review estimated 90-130 such lines for the design below. Record the measured count
in the log and the report.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Approach: "Send each menu focus move through a small page-side function that drops it if the user moved focus since
  the key press", chosen over handling the arrow keys entirely in page-side JavaScript and over accepting the race. This
  choice also lifts the earlier fix's rule against product JavaScript for this change, since every complete fix needs
  some.
- Budget: "Stop and ask (block the plan) with what was found" if it grows past the gate above.
- Review gate and mode are in the run instructions below.

**Binding repository constraints:** SPEC.md's accepted-race rule (near-simultaneous input races are accepted, "still
worth fixing when the fix is reasonably simple", and a race never runs a command the user did not activate on a visibly
focused control) and the sentence that menus use their own arrow keys; SPEC_impl.md's row-menu "Keyboard" paragraph, the
host menu sentence that it follows the same rules, and the Templates popup's use of the `menu_panel` helpers. Root
`AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main and the Dioxus 0.7.10 sources (verify them):

- Keys are handled in Rust: each item's `onkeydown` calls `handle_menu_key` in `menu_panel.rs`. The step origin is
  `requested.or(focused).or(event_origin)` (`menu_step_origin`). `focus_menu_item` sets `focused` and `requested` and
  calls `request_menu_focus`, a single drainer that coalesces to the latest target and records `in_flight` while it
  awaits `set_focus`.
- On desktop, `MountedData::set_focus` evaluates `window.interpreter.setFocus(id, true)` in the webview immediately
  (`dioxus-desktop` `element.rs` and `query.rs`), before the future is polled, and the page's `setFocus` is a plain
  `node.focus()`. Nothing can recall it. On the web, `set_focus` calls `focus()` synchronously.
- #2232 (commit b339a8a3) added `in_flight`, `record_menu_item_focus` (compares the focused item's handle with
  `in_flight` by `Rc::ptr_eq`) and `requested_after_item_focus` (a focus-in at the in-flight target keeps `requested`;
  any other clears it and cancels the queued target). Its plan and report are at
  `git show a46e009d:plans/queue/menu-focus-source.md` and
  `git show a46e009d:plans/reports/menu-focus-source.report.md`.
- The remaining race, step by step: Down #1 sends a move to item 1; Down #2 queues item 2; item 1 lands, item 2 is sent;
  the user focuses item 3 (cancel path runs, nothing left to cancel); the sent move to item 2 runs anyway and its
  focus-in matches `in_flight`, so it is accepted; Down #3 steps from item 2.
- Page-side precedents: `crates/farhelm-ui/assets/session-action-shortcuts.js` and `template-shortcuts.js` (idempotent
  `__farhelm…` install guards, capture-phase listeners, `module.exports` for `js-tests/`, loaded with `include_str!` and
  `document::eval`); epoch and generation guards in `template-shortcuts.js` (the modal epoch snapshot consumed by
  `launch_sequence.rs`), `__farhelmModalOpening` in `modal_isolation.rs`, `__farhelmSyncGeneration` in
  `session_view.rs`; on the Rust side, `measurement_outcome`'s generation check in `menu_panel.rs`.
- Tests today: unit tests in `menu_panel.rs`; the #2232 "desktop timing simulation" in `e2e/tests/sidebar.spec.ts`
  (around 1671-1711), which covers only unsent work; other menu keyboard tests in `sidebar.spec.ts`, `feedback.spec.ts`,
  `templates.spec.ts` and `terminal-multihost.spec.ts`.

Proposed shape (planner proposal), one `fix` PR:

- A page-side script, installed once, that keeps an "independent focus" epoch (bumped by a capture `focusin` that is not
  the app's own gated focus call) and records the epoch at each navigation keydown in a menu; and a gated focus function
  that focuses the target only if no independent focus move happened since the latest navigation key, returning whether
  it focused.
- `request_menu_focus` awaits that function through `document::eval` instead of `set_focus`, for navigation-caused moves
  (gated) and the open-intent focus (ungated flag). The function finds the target by a marker attribute on each item,
  keyed by the item's action and unique per menu instance (one attribute line per item: roughly 8 in the session row
  menu, 9 in the host row menu, and one each in the looped bar menus and the Templates popup). Do not locate items by
  panel plus index: `menu_panel.rs` holds that a position is never a durable identity, and a refresh between the request
  and the lookup would focus the wrong item.
- Coexist with modal isolation's keydown net for the Templates popup.
- The browser test above (holding the gated function's calls is what makes a move genuinely in flight on the web), and
  JS unit tests for the epoch logic.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the UI crate's unit tests, the JS unit tests, the menu keyboard browser
specs named above through the recorder on both engines, and the test-sleep checker. Consider `scripts/desktop-smoke.sh`
if the change could break the desktop bridge (`.agents/validation.md` says when it applies); say in the report that no
automated test exercises the real desktop webview's timing.

## Fallbacks

The complexity gate above. Examples of departures that need the scope reassessment (and that probably trip the gate):
moving arrow handling into page JavaScript, a ledger of pending positions, changing how the queue coalesces requests, or
changing Dioxus.

Decisions to log: the marker's form, the epoch's exact rules (which focus-ins count as independent), and the measured
size of the product change against the gate.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-menu-arrow-focus-race-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/menu-arrow-focus-race/<nn>-<short-name>` (for example `plan/menu-arrow-focus-race/01-gated-focus`).
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
