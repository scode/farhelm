# UI: pinned sessions keep a manual order the user can rearrange

Written against main at d81a3980 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - Allow the user to manually reorder pinned sessions.

The maintainer's words when choosing it, verbatim: "lets have pinned sessions always be manually ordered. the last one
pins defaults to the bottom of the pinned list. please mock up and suggest things right away".

### What changes

Today pins follow the list's chosen sort (activity, created or title) inside a pinned group that a stable partition
lifts to the top of the sidebar (`rows::pinned_first` in `crates/farhelm-ui/src/rows.rs`, applied in `commit_listing` in
`crates/farhelm-ui/src/list/view.rs`). SPEC.md, "Pinned sessions" area of the session list (search for "Pins have no
manual or pin-time order"), says pins have no manual order. That sentence is reversed by the maintainer's decision
above:

- The pinned group has one manual order, stored by the helm with the pins, so every client and device sees the same
  order (pins are already shared helm data in `session_pins`, `crates/farhelm-helm/src/store.rs`).
- Pinning a session puts it at the bottom of the pinned group.
- The user can move a pinned session by dragging its row within the pinned group, and by **Move up** / **Move down**
  items in a pinned row's menu, placed directly after **Unpin**, disabled at the ends of the group (and hidden on
  unpinned rows).
- The pinned group ignores the list's sort choice; unpinned rows below the separator keep following it.

### What does not change

Pin and Unpin stay menu actions; a drag never crosses the pin separator (no pinning or unpinning by drag). The quick
switcher keeps its activity order and does not group pins. Restart keeps a pin on the same id, and Replace and Replace
with carry it to the new session, now also keeping its position. Deleting a session removes its pin. Unpinning and
pinning again puts the session at the bottom. Host and status filters still apply to pinned rows; a filter hides rows
without changing the stored order.

### Acceptance criteria

- The helm stores a pin position in a new NOT NULL column. Do not change `SESSION_PINS_PAIRED_SCHEMA` in place: the
  historical 62→63 step also uses it, and a changed constant breaks that step. Add a new DDL constant for the next
  schema version, used by both the fresh-create branch and a table-rebuild upgrade step (a rebuild suits, since seeding
  the order needs a full pass anyway). Like the recent steps, the new step tolerates rewound test databases that already
  have the column. Existing pins get a starting order: oldest session first, by the creation time the helm records for
  the session (it has one for cached sessions); pins whose creation time the helm does not know follow, in (host id,
  session id) order.
- Positions may tie, ties break by (host id, session id), and each move renumbers the whole set. Ties are allowed
  because `inherit_pin` deliberately copies the pin to a Replace's new session, so source and replacement share a
  position until the source's delete is confirmed (and keep sharing it if that delete fails); a UNIQUE constraint would
  make Replace abort. Pins of sessions deleted elsewhere can also linger, so the stored order can contain positions no
  client sees.
- New pins go to the bottom. Unpin removes the row. Replace and Replace with carry the position with the pin.
- A move request names the moved session and a visible neighbor ("place before session X" or "after session X"), not an
  index: filters hide pins, the hold can show a different order, and lingering pins are invisible, so a visible index
  does not map to a stored one. The helm applies it in one store call. `HelmStore` runs every query on one connection
  behind a mutex, so that is already atomic and the later of two concurrent moves wins; no revision check or extra
  locking is wanted. Every client learns the new order through the fleet feed it already uses for pin changes.
- The sidebar renders the pinned group in stored order regardless of the sort choice. The order hold keeps working as
  SPEC.md describes, with one change: rows that are pinned in both the held snapshot and the new reply are shown in
  stored order within the slots they already hold, so a moved row does not snap back while the pointer is still over the
  list. Everything else about the hold stays: pin and unpin changes update in place during a hold, and a newly pinned
  row held below the pinned run does not move the separator. Such a row cannot be moved (no grip, no Move items) until
  the hold ends and it joins the run.
- Dragging, with pointer events in the style of `crates/farhelm-ui/assets/sidebar-width.js` (chosen up front: HTML drag
  and drop would have to cooperate with the document-level file-drop guard in `terminal.js` and behaves differently in
  the desktop webviews): a grip appears on a pinned row on hover and stays hidden on unpinned rows; dragging shows the
  row lifted and a drop line between pinned rows; dropping moves the session there; Escape or releasing outside the
  pinned group cancels. Unattended verification is Playwright on Chromium and WebKit; say in the report that dragging in
  the desktop app itself was not exercised (it needs a devbuild on the maintainer's Mac).
- Move up / Move down in the row menu do the same move relative to the neighbor shown above or below. At the ends of the
  displayed pinned group, follow the row menu's existing convention for actions that do not apply (it hides them); the
  planning mockup said "disabled at the ends", and either reads as the same decision. After a move, keep keyboard focus
  on the moved row's menu button (a planner choice, not an existing SPEC rule).
- An open row menu closes when its row moves, as `rows::menu_row_reordered` already does for other moves.
- SPEC.md: the pins paragraph says pins keep a manual, shared order, new pins at the bottom, and how to move them,
  replacing both "Pins have no manual or pin-time order" and "each group preserving the chosen list order"; the order
  hold text gains the exception above; the row menu list gains Move up / Move down. SPEC_impl.md: where the position
  lives, the schema step and why it rebuilds, ties, and the move endpoint. Docs:
  `website/src/content/docs/docs/using/session-list.mdx`, "Pin a session to the top" (no longer "both groups keep your
  chosen sort order") and its hold sentence ("Rows wait to move…") with the same exception.
- Tests: helm store and route tests (new pin at bottom, move before and after a neighbor, unpin, Replace sharing the
  position, ties, migration order including pins with unknown creation time, a move notifying the fleet feed); browser
  tests for drag, the menu items, the order surviving a reload, a move during a hold not snapping back, and the sort
  choice not affecting pins. Update `e2e/tests/pins.spec.ts`, which today asserts title order inside the pin group.
- A changelog fragment (`kind: changed`, since pin order changes for existing users), added by the PR that makes pins
  manually ordered and edited by the later PR that adds dragging; removal of the TODO.md entry.
- The schema bump is stated in the report and the PR description, because root `AGENTS.md`, "Flows", requires a devbuild
  of a schema-changing target to say so.

## Decisions already made

Settled with the maintainer on 2026-11-04 on a planning page with mockups (a dark mockup of the sidebar with a grip on
each pinned row, the dragged row lifted and a blue drop line between two pins; and a row menu with Move up / Move down
directly after Unpin):

- **Always manual, new pins at the bottom.** The maintainer's own words above. This reverses SPEC.md's "Pins have no
  manual or pin-time order"; amend it.
- **Gesture:** drag a pinned row, plus Move up / Move down in its menu. Chosen over Alt+↑/↓ on a focused row and over
  menu items only.
- **No dragging across the separator:** pin and unpin stay menu actions.
- **Starting order for existing pins:** oldest session first. Chosen over title order and over activity at upgrade.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words and the decisions above.

**Binding repository constraints:** SPEC.md's pins, order hold, row menu and keyboard-reachability text in the session
list sections; SPEC_impl.md on the listing and the fleet feed; the helm store's schema-step conventions in
`crates/farhelm-helm/src/store.rs` (current `SCHEMA_VERSION`; one shared DDL constant for fresh and upgraded schemas);
root `AGENTS.md`, "Code rules", "Tests and validation", "Conventional Commits", "Changelog fragments";
`website/AGENTS.md` for the docs page; `.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code: a new position column on `session_pins` rather than a new table; a move route next to
the pin route (registered in `crates/farhelm-helm/src/lib.rs`, handled in `crates/farhelm-helm/src/sessions.rs`) taking
the neighbor session and before/after; the listing (`crates/farhelm-helm/src/aggregate.rs`, `SessionRow`) carrying the
position so the UI sorts the pinned partition by it; three PRs: helm storage and route, then the sidebar order, hold
exception and menu items (with the changelog fragment), then dragging, docs and the TODO removal.

## Outline

1. **Helm.** Schema step adding the position and seeding existing pins as the acceptance criteria say; `set_pinned`
   appends at the bottom; `inherit_pin` carries the position; a move operation in the store plus its route; the listing
   carries each pin's position. Store and route tests.
2. **UI order and menu.** Sort the pinned partition by position in `rows::pinned_first` (or its replacement), keep the
   separator logic, add the hold exception in `rows::held_display_order`, add Move up / Move down to the row menu
   (`MENU_ACTIONS` and its markup in `crates/farhelm-ui/src/list/row.rs`) calling a new `api` function, and refresh as
   `on_pin` does (`commit_listing` is a closure in `crates/farhelm-ui/src/list/view.rs`).
3. **Dragging.** Pointer-event drag with grip, lifted row and drop line, the move on release, cancel on Escape or
   outside.
4. **Spec, docs, bookkeeping.** SPEC.md and SPEC_impl.md, the docs page (an incidental docs edit in an unattended run:
   start no preview server and name the changed page in the report), the changelog fragment, the TODO.md entry removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy on the helm and UI crates,
the helm's store and sessions tests, the UI crate's tests, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and, through `python3 scripts/record-test-run.py`, `pins.spec.ts`,
`sort.spec.ts`, `sidebar.spec.ts` and `replace.spec.ts` plus the new drag tests on Chromium and WebKit. Run
`python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: a separate ordering table or a per-client order; dragging across the
separator; reordering unpinned rows; a keyboard chord for moving; changing the quick switcher's order.

Decisions to log: the migration's handling of pins without a known creation time, the move request's shape, how the hold
exception is implemented, hide versus disable for the Move items at the ends, and every review finding you decided not
to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-pinned-session-order-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/pinned-session-order/<nn>-<short-name>`
  (for example `plan/pinned-session-order/01-helm-pin-position` and `plan/pinned-session-order/02-sidebar-order`).
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
