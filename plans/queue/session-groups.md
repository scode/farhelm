# Session groups

Written against main at d4bb8515 on 2026-11-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan's first PR writes the new behavior into SPEC.md.

This plan runs after `mute-sessions.md` has landed, and builds on the mute rules that plan wrote into SPEC.md.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Session groups.** Let users group related sessions in the session list and collapse each group. Keep sessions
> together as activity changes and show when a collapsed group contains a session that needs attention.

Users put sessions into named groups by hand. Groups sit in the session list below the global pins, in an order the user
sets, each collapsible, with its own pinned members on top. A collapsed group shows the status dot of its most urgent
member. Groups, membership, order and collapse state are helm state shared by every client, like pins.

Despite its TODO subsection, this is a feature: new SPEC.md text, new helm tables and routes, and substantial
session-list UI in the desktop app, including drag interactions.

### Acceptance criteria

Behavior (each is also a SPEC.md statement the first PR writes):

1. **Layout.** From top to bottom: the global pins (pinned sessions in no group) in their manual order, as today; then
   each group in its manual order; then ungrouped unpinned sessions in the list's chosen order. Each group is a header,
   then its pinned members in their own manual order, then its unpinned members in the list's chosen order (Recently
   active, Newest created or Title A–Z, with mute's ordering rules). The existing heavier separator after the global
   pins keeps its meaning (when anything follows). A new separator of the same weight follows the last group when
   ungrouped sessions follow. Inside a group a lighter separator follows its pinned members when unpinned members
   follow.
2. **Making groups.** The row menu offers **Move to group ▸ New group… / each existing group / Remove from group**. "New
   group…" asks for a name and creates the group holding that session. The helm trims leading and trailing spaces and
   refuses an empty or whitespace-only name and a name another group already has, ignoring case (two identical entries
   in "Move to group ▸" could not be told apart); the refusal shows in the naming dialog. "Move to group ▸" is the
   shared submenu `mute-sessions.md` built, in both menus. For a pinned session in a group, the entry that takes it out
   of its group is named **Global pins**. The marked-rows metadata menu offers the same Move to group submenu (New
   group…, each existing group, Remove from group), applying to every marked row not already where it would go. A
   session is in at most one group.
3. **Pins inside groups.** Pin to top on a grouped session pins it inside its group, at the bottom of that group's pins.
   Unpin leaves a session where it is (in its group, or ungrouped), now unpinned. Moving a pinned session into a group,
   by menu or drag, keeps it pinned and puts it at the bottom of that group's pins; moving it to Global pins puts it at
   the bottom of the global pins. Moving an unpinned session into a group adds it to that group's unpinned members. Move
   up and Move down rearrange a pin only within its current area (the global pins, or one group's pins).
4. **Dragging pins.** The pin grip drag that rearranges pins today can drop a pinned session into any pin area: the
   global pins or the pin area of any expanded group, with the drop line showing where it lands. A drop into an empty
   pin area of a group makes it that group's first pin. Dragging still never pins or unpins a row, and unpinned rows
   still have no grip. (Dragging unpinned sessions into groups is a separate TODO entry, "Drag sessions into groups".)
5. **Ordering groups.** A group header has a grip that drags it among the other groups, and its menu offers Move up and
   Move down, hidden when unavailable, like pins. New groups go at the bottom of the groups.
6. **Header.** A group header shows a collapse toggle, the group's name and its member count (**N**, or **N of M** when
   a filter narrows it). Its menu offers Rename, Ungroup, Move up and Move down. It is keyboard-reachable and exposes
   its expanded state to assistive technology.
7. **Collapse.** Collapsing hides every member, pinned ones included. The collapsed state is stored by the helm and
   shared by every client, like the navigation rail's expansion, so a window opens with each group as it was last left.
8. **Attention.** A collapsed header shows the status dot of its most urgent member: waiting (red), then unseen idle
   (blue), then running (green), using the same marks the rows use. Only members the current filters display count, and
   a muted member counts only while it is waiting. Hovering the dot names that session and its status. An expanded
   header shows no aggregate dot.
9. **Keeping together.** Members stay inside their group whatever their activity: activity reorders members within the
   group, never moves them out of it, and never moves a group.
10. **Filters.** With a host or status filter, a group shows only its matching members and is hidden when none match;
    its count reads "N of M". Marks, counts and actions follow SPEC.md's existing rules on displayed rows.
11. **Ungroup and empty groups.** Ungroup removes the group. Its members become ungrouped; its pinned members stay
    pinned and go to the bottom of the global pins in their group order. A group whose last member leaves or is deleted
    disappears, including when its last members go because their host entry was removed (a group can span hosts, so its
    row cannot simply cascade from one host).
12. **Lifecycle.** Restart keeps a session in its group (same id). Replace and Replace with carry the membership, and
    the pin and its position within the group, to the new session, as they carry pins today. Deleting a session removes
    its membership. Membership belongs to the session on its host entry, as pins do; removing a host entry drops its
    sessions' memberships. A group may hold sessions from several hosts.
13. **Order hold.** The hover order hold (SPEC.md "Session list", "A client holds back reordering while the pointer is
    over the list") covers groups: no row or group moves under the pointer during a hold, and the hold's existing rules
    for pins carry over to group pins and group order. A membership change during a hold (Move to group from a row's
    menu, which keeps the hold while open) follows SPEC.md's rule for pin changes: the row updates in place and moves to
    its new group when the hold ends. A pin dragged into another area is a manual move and applies at once, like any
    manual pin move during a hold. Group headers are derived from the held rows, not snapshotted separately. Collapsing
    or expanding a group is the user's own action and applies at once.
14. **Elsewhere.** The quick switcher is unchanged.

One implementation (the maintainer's requirement, 2026-11-17):

15. **Manual ordering exists once.** Today's pin ordering is the first and only manual order in the list: the helm's
    neighbor-and-placement move and renumbering (`ordered_pins`, `renumber_pins`, `move_pin` in
    `crates/farhelm-helm/src/store.rs`), the app's Move up / Move down neighbors (`pins.rs`, `neighbors`), the grip drag
    (`session_list/pin_drag.rs`: grip, pointer capture, Escape, focus, drop line), and the hover hold's pin rules
    (`hold.rs`). Groups add two more ordered lists: each group's pins, and the groups themselves. Both must run on that
    same code, not on a copy. In the app (neighbors, drag drop bands, the hold) the code is generalized over "an ordered
    area" and "what kind of item is being moved". On the helm, pins keep one position sequence across all areas (the
    maintainer's decision on the reuse page), so a pin's area is only its session's group and the pin queries and
    `ordered_pins` / `renumber_pins` / `move_pin` stay as they are for moves within an area; what the helm generalizes
    is the item kind only (pin rows keyed by session, group rows keyed by group id): one pure "remove, insert before or
    after the neighbor" over keyed positions, with renumbering and the overflow-safe append, used for both. In
    particular the header grip drag is the pin grip drag, not a second drag implementation, and group order uses the
    same move and renumber logic as pin order. Where the existing code is too pin-specific to extend cleanly, the
    generalization comes first as its own `refactor:` PR with no behavior change, proven by the existing pin tests
    passing unchanged, before any group code uses it. The rule also covers these existing pieces, which groups needs and
    must not re-create: the helm's append at `MAX(position) + 1` with compaction on overflow in `set_pinned` (new
    groups, Pin to top in a group, moves into an area and Ungroup all append); `PinPlacement`, `MovePinReq` and the
    `PinMoveError` to `Conflict` mapping in `sessions.rs` `move_pin` (the group-order route shares the request type and
    the mapping); `pins.rs`'s `Direction` (the Move up / Move down items' labels, help, icons and ids), `menu_items`,
    `send_move` and `move_command` (the header menu's Move up / Move down are the same items); and `pins::separator` and
    `separator_after` (criterion 1's separators come from them, with a weight parameter and area boundaries, not new
    separator elements). `pin_drag.rs`'s `PinDrag.movable` and `bounds`, and `menu::RowLayers.open`, are keyed by
    session-key strings; headers get a key namespace that cannot collide with a session key.
16. **Reuse from mute.** `mute-sessions.md` lands first and builds the shared per-session viewer-state plumbing (one
    keyed-batch read helper, one route preamble, one struct of joined state passed to `aggregate::row_of`), puts its
    cleanup in `forget_deleted_session`, and makes Replace carry viewer state in one transaction. Groups uses all of
    them: membership is read through the same helper, its routes use the same preamble, it extends the same struct, its
    cleanup joins `forget_deleted_session`, and membership joins the same Replace transaction. For groups that
    transaction is a correctness requirement, since a pin's position only means something with its group. If mute landed
    without one of these, build it here once rather than adding a parallel path.

Delivery:

17. SPEC.md describes all of the above in "Session list", in the existing voice, and revises the pin paragraphs that
    groups change (pins as one global group, "Dragging never ... crosses the separator"). SPEC_impl.md records the
    storage choices where it describes pins, and says that pins and groups share one manual-ordering implementation.
18. A changelog fragment (`kind: added`) in the PR that makes groups usable in the app.
19. The last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-17 planning page:

- **Manual membership** through the row menu and marked rows. The rejected alternatives were automatic grouping (by
  repository or checkout) and having spawned sessions join their parent's group. Dragging any session into a group is a
  separate Near term TODO entry ("Drag sessions into groups"), recorded in the planning PR at the maintainer's request.
- **Groups are ordered by hand like pins, after the global pins.** The maintainer's words: "let's make groups always be
  manually ordered like pinned sessions. but they come after pins. you can also have pinns INSIDE of groups. so a global
  pinned session works like today (and always above groups), but within each groups pinned works the same but locally
  within the group. you can move a pinned session between groups or to global pinned using the same mechanism used to
  move pinned ordering today." The rejected alternative placed a group like one row by its best member.
- **Unpinned members follow the list's chosen order.**
- **Crossing pin areas:** the grip drag drops into any pin area; the row menu's Move to group (with Global pins) is the
  keyboard route and keeps the pin; Move up and Move down stay within an area. The rejected alternative let Move up and
  Move down at an area's end carry a pin into the next area.
- **Pin and Unpin inside groups** as criterion 3 says.
- **Group reordering:** header grip drag plus Move up / Move down in the header menu.
- **Collapsed header:** name, count, the most urgent member's dot, hover names the session. A muted member counts only
  while waiting.
- **Collapse is shared** helm state. **Empty groups disappear.** **Filters** show matching members only and hide empty
  groups. **Ungroup keeps pins**, moving them to the bottom of the global pins.
- **Stored by the helm and shared**; Restart keeps, Replace and Replace with carry, Delete removes, quick switcher
  unchanged (stated on the page and not overridden).
- **No duplicated ordering or drag code** (the maintainer, after the planning page: "i dont want the grip-and-drag stuff
  to somehow get duplicated or similar", and "a separate grip and drag for groups completely separately implemented from
  pinned sessions would be bad"). Criterion 15 is the rule; it is a hard requirement, not a style preference, and a PR
  that adds a second grip drag, a second move/renumber routine or a second hold rule for ordered items does not pass its
  review gate.
- **Reuse decisions** (the maintainer, on the 2026-11-17 reuse page, after a fresh-context reuse review; each took the
  recommended option):
  - Use the shared submenu and dynamic marked-rows menu `mute-sessions.md` builds; never build a second submenu.
  - One pin position sequence across all areas (rejected: numbering per area, which makes every pin query join
    membership).
  - A pin dragged to another area travels on the existing pin-move request: the moved pin joins its neighbor's group in
    the same step, so there is one placement path (rejected: placement on the new set-group request).
  - Generalize the session Rename dialog (`session_list/rename.rs`) to name a session or a group, with the session-only
    parts kept on the session side (rejected: a separate group dialog).
  - Group names: trimmed; empty, whitespace-only and case-insensitive duplicates refused (rejected: duplicates allowed).
  - Collapse writes generalize the rail's queued save (`session_list/queued_preference.rs`) to a queue per group, with
    collapse stored on the group (rejected: storing collapsed groups as one shared preference, where two windows could
    overwrite each other).
- **Ordering between plans:** after `mute-sessions.md` (the maintainer chose mute first).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, after `mute-sessions.md`.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC.md "Session list" (pins, drag, separators, the activity order and the order
hold, marked-row actions, filters and counts) and "Status" (the marks, mute); SPEC_impl.md where it covers pins; root
`AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and validation";
`.agents/careful-rebase.md` on schema versions; `.agents/test-authoring.md`.

**Planner proposals** (from a read of the code at d4bb8515; follow the code where it differs, logged as a DECISION):

- **Storage.** Two helm tables in `crates/farhelm-helm/src/store.rs`: groups (an id, name, position among groups,
  collapsed flag), and membership keyed like `session_pins` (`host_id` referencing `hosts` with cascade, `session_id`,
  group id). Pins stay in `session_pins` with their one global position sequence: a pin's area is the session's group
  (or none), and the app sorts each area's pins by position. Read the comment above `SESSION_SEEN_PAIRED_SCHEMA` for why
  viewer state keys on `hosts`, and `ordered_pins` / `renumber_pins` / `move_pin` / `inherit_pin` for the pin mechanics
  to extend (criterion 15). One new helm schema version. Empty groups are removed in the same transaction as the
  membership change that empties them; for host removal, where the cascade removes memberships, remove groups left empty
  in the same operation, or have every read ignore a group with no members and delete it opportunistically. No triggers
  or sweepers.
- **API.** Few routes, beside `/pin` and `/pin-order` in `crates/farhelm-helm/src/lib.rs` and `sessions.rs`: one `PUT`
  on a session that sets its group (an existing id, a new name, or none), with no placement: a pinned session it moves
  lands at the bottom of the target area through the existing append. A drag across areas uses the existing `/pin-order`
  request unchanged on the wire: `move_pin` makes the moved pin adopt the neighbor's group in the same transaction. Only
  a drop into an empty group pin area uses the set-group `PUT`, where "bottom" is also "first". Then one `PATCH` on a
  group for name and collapsed; the existing neighbor-and-placement move for group order (the same handler logic as pin
  order, per criterion 15); and a `DELETE` on a group for Ungroup. Name validation (criterion 2) is the helm's, in one
  place used by create and rename.
- **Naming dialog.** Generalize `session_list/rename.rs` over what is being named (a session title, a group name, or a
  new group with an empty draft), keeping its settled rules for all of them: the draft follows helm pushes until edited,
  Save is busy rather than disabled while a save is in flight, a refusal shows in the dialog, the layer manager's
  dismissal hook, and focus return. The session-only parts (its naming mode, `listing_proves_gone`) stay on the session
  side.
- **Collapse writes.** Generalize `QueuedPreference<T>` (`session_list/queued_preference.rs`) over its `OpKey` (today
  `send` hard-codes `OpKey::preferences()` and `Field` points at one preference field), and use one queue per group id
  for collapse, so the choice shows at once, the latest wins, and a click during a write in flight is queued, never
  dropped (SPEC.md, "An action the user started is still never lost silently"). The rail's expansion keeps working
  through the same type unchanged.
- **Reaching clients.** The session list is filtered on the helm per client query, so under a filter the app never
  receives non-matching members and cannot count M in "N of M" itself. Put the group on each `SessionRow`
  (`crates/farhelm-proto/src/ui_channel.rs`), joined in `crates/farhelm-helm/src/aggregate.rs`: id, name, position,
  collapsed, and the group's unfiltered member count, denormalized the way `host_name` already is. Since empty groups
  disappear and groups with no matching member are hidden, every group the app draws has at least one row in the reply,
  so no separate group collection or endpoint is needed. A rename or collapse re-pushes the member rows, which the
  per-row diff already handles. New row fields change `crates/farhelm-helm/http-contract/session-list.json`; update it
  with the change.
- **Partitioning in the app.** Today the helm serves the chosen order and the app groups pins on top
  (`session_list/pins.rs`, `pinned_first`). Do the same for groups: the helm keeps serving the chosen order (with mute's
  rules), and the app partitions rows into global pins, groups and the rest. Where collapse sits decides whether the
  hold works: today `session_list.rs` (around line 818) runs `hold.apply(pins::pinned_first(listed))`, then marks,
  neighbors, the movable list, separators and entries. Make it: partition (replacing `pinned_first`), then `hold.apply`
  on the uncollapsed partition, then drop collapsed members, then marks, neighbors, separators and entries. The held
  snapshot then includes collapsed members, so expanding a group during a hold does not send its members to the bottom
  (which `hold::display` does to rows new to the hold), collapse applies at once (criterion 13), and marks and counts
  follow displayed rows (criterion 10). `Held.original_pins` becomes a record of each original pin's area. The
  virtualized list's entries are session keys today; headers get an entry kind of their own.
- **Header menus and dialogs register where the hold looks.** The hold stays on while `RowLayers::any_open` (`menu.rs`,
  around line 228) reports an open row menu or question. Build the header menu from the row menu's parts (`MenuHeader`,
  `MenuItem`, `menu_keys`, the layer manager's beside anchor) and record it, and the naming dialog opened from a row or
  a header, in `RowLayers` under a header key, so the hold cannot release under an open header menu.
- **Bulk New group….** The marked-rows menu sends one request per target (`marking.rs`, around line 790). A "New group…"
  for N marked rows must create one group: the first write creates it and the rest are sent by its id, or a create route
  takes the list of sessions. Moving rows to an existing group, or out of their group, fits the per-row dispatch as it
  is.
- **Small reuse points.** The attention dot's ranking (waiting, then unseen idle, then running) is one method on
  `status::Live` beside `Live::mark`, with mute's `status::muted` applied, not ranked in the header code. The header's
  "N of M" uses the list's own count format (`session_list.rs`, around line 1314, `CountLine::Count`). Each area's pin
  sort reuses `pins::order_key` (position, then host, then bare id, the helm's tie break).
- **GUI** (`crates/gui/farhelm-gui/src/session_list/`): headers and collapse in `session_list.rs` and `row.rs`; menus in
  `menu.rs` and `pins.rs`; marked rows in `marking.rs`; the attention dot from `status.rs`. Drag: generalize
  `pin_drag.rs` so one drag serves pinned rows (with drop targets in every pin area) and group headers (with drop
  targets among the groups); its module docs describe the gesture's rules, all of which apply to both.

## Outline

0. Only where criterion 15 needs it: `refactor:` PRs that generalize the existing manual-ordering code (helm move and
   renumber, the app's neighbors, the grip drag, the hold's pin rules) over ordered areas and item kinds, with no
   behavior change and the existing pin tests passing unchanged. Number them first in the stack (for example
   `plan/session-groups/01-generalize-pin-order`) and shift the numbers below.
1. `plan/session-groups/01-helm-groups`: SPEC.md and SPEC_impl.md text; the helm tables, routes and row fields; pin
   areas; Replace carry, delete and empty-group cleanup, ungroup; helm tests. `feat:` with a `kind: none` fragment
   saying the app PRs carry the user-facing entry.
2. `plan/session-groups/02-app-groups`: the layout, headers, collapse, attention dot, counts, filters and order hold;
   row, header and marked-row menus (Move to group, Rename, Ungroup, Move up / Move down); GUI tests.
3. `plan/session-groups/03-app-group-drag`: the header grip drag and pin drag across areas; GUI tests; the `kind: added`
   changelog fragment; removing the TODO entry.

Split or merge where the work shows a better cut, logged as a DECISION.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the changed crates, the helm store, migration, aggregate and session-route tests, the GUI session-list tests (pins,
hold, marking, drag) through `python3 scripts/record-test-run.py`, `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md`, `python3 releasing/check-changelog.py format`, and `dprint check` on changed Markdown.

GUI screenshot goldens: the new row-menu items change at least the `session-row-menu` picture, and probably the
`session-list` and marked-rows scenes. Re-capture and publish the affected pictures and commit the manifest with the
change, as `.agents/validation.md` describes for `scripts/check-gui-goldens.py` and `scripts/gui-goldens.py publish`. No
new golden scenes are wanted: decline a review finding that asks for one (a group-header scene, say). If publishing is
not possible from where you run, push what is consistent and block with that question.

## Fallbacks

If another change has moved the code this file names, follow the current code and record the true names in the working
log. If another plan lands a helm schema version first, renumber at rebase per `.agents/careful-rebase.md`. If
`mute-sessions.md` changed what this file assumes about mute (the dot's place, the sort rule), follow SPEC.md as it
stands on main and log the DECISION.

Generalizing the existing pin drag to serve group headers and every pin area is expected work (criterion 15), not a
departure. What needs the scope reassessment first is anything else: rewriting the gesture's behavior, or any design
that would add a second drag, move or hold implementation, which criterion 15 forbids outright. If the drag still cannot
be finished, deliver the earlier PRs complete, and block with the drag question rather than shipping a half-working drag
or a duplicate.

Examples of departures that need the scope reassessment: nested groups, groups that sort by activity, a general
"sections" framework beyond what criterion 15's generalization needs, dragging unpinned sessions (the separate TODO
entry), or changes to the quick switcher.

Decisions to log: the storage shape, how pin areas are stored, where partitioning happens, how the existing ordering and
drag code was generalized (and why any part of it could stay pin-only), how the hold handles groups, every test changed
and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-session-groups-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Build and test workers

Run every cargo build, test, clippy and GUI bridge run on the shared remote build workers, not on the machine you run
on, which many agents share. How to use them (the runner, which workers executors may use, how to run tests through the
recorder there, and the rules that keep worker names out of the repository) is in the file `boatrunner-info.md` in the
home directory of the user running you. Read it in full before your first build. Never copy a worker's name, account or
any path from that file into the repository, a PR, a Blocked question or your report. Formatters and small checks that
need no build (`dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`) may
run locally.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/session-groups/<nn>-<short-name>`, as the
  outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry. Leave the "Drag sessions into groups" entry alone.

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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). For this plan the prompt also asks the reviewer, explicitly, whether the PR adds a second
implementation of anything criterion 15 names (a grip drag, a neighbor-and-placement move, a renumbering, a hold rule
for ordered items), or of anything the Reuse decisions and criterion 16 name (a submenu, a naming dialog, a queued
write, viewer-state plumbing, a Replace carry), instead of extending the existing one, and to report any such
duplication as a correctness finding. When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (merging or
splitting steps where the work showed a better cut, logged as a DECISION), together meeting the acceptance criteria
above, each having passed its review gate, and the last code PR removing this plan's TODO.md entry. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
