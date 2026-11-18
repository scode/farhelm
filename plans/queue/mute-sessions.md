# Mute a session

Written against main at d4bb8515 on 2026-11-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan's first PR writes the new behavior into SPEC.md.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Mute a session.** the ability to "mute" a session so it doesnt turn blue and doesn't promote as recent for a while
> or permanently

The user can mute a session for 1 hour, until the next 06:00, or until they unmute it. A muted session never shows as
unseen (blue), is not promoted by new work in the "most recent activity" order, and plays no turn-finished sound. A
session that starts waiting on the user (a question or an approval) breaks through the mute: it still turns red,
promotes, sounds and sends its alert. The mute is helm state shared by every client, like pins and the seen state.

Despite its TODO subsection, this is a feature: new SPEC.md text, a new helm table and routes, a sort rule on the helm,
and session-list UI in the desktop app.

### Acceptance criteria

Behavior (each is also a SPEC.md statement the first PR writes):

1. **Muting.** The row menu, next to Pin to top / Unpin, offers **Mute ▸ For 1 hour / Until tomorrow morning / Until I
   unmute** on an unmuted session and **Unmute** on a muted one. The marked-rows metadata menu offers the same, applying
   Mute only to unmuted marked rows and Unmute only to muted ones, the way bulk Pin and Unpin work today (SPEC.md
   "Session list", "**Pin** applies only to unpinned marked rows"). "Mute ▸" is a real submenu in both menus, built as
   criterion 1a says.

1a. **The shared submenu and a dynamic marked-rows menu** (the maintainer's decision on the reuse page). The app's own
menus have no submenu today (the only one is the native macOS menu bar), and this plan and `session-groups.md` need four
between them. This plan builds one shared submenu component in `crates/gui/farhelm-gui/src/components/`, beside
`menu_item.rs` and `menu_keys.rs`: a child popover anchored beside its parent item (the layer manager's parent rules in
`layers.rs`, "Who opened what", already support a popover opened from another layer and closed with it), keeping
SPEC.md's one-highlight rule for menus, with Right and Left (and Enter and Escape) into and out of it, focus returning
to the parent item, and a dynamic list of items. The marked-rows menu (`marking.rs`'s `MetadataAction` with its fixed
`ALL` array, and `MarkedMenu`'s `items: [FocusHandle; 4]` indexed by declaration order) becomes a dynamic list with a
focus handle per item, so groups can list existing groups there. SPEC.md states the submenu's keyboard and focus rules
where it describes menus. `session-groups.md` reuses both and must not build another. 2. **Durations.** "For 1 hour"
ends one hour after the click. "Until tomorrow morning" ends at the next 06:00 in the local time of the device that
muted it, computed once on that device and stored as a fixed moment, so muting at 02:00 ends at 06:00 the same day.
"Until I unmute" never ends by itself. The helm ends a timed mute at its moment on its own, with no client connected,
and pushes the change. 3. **Shared and durable.** The helm stores the mute, every client shows it, and a change reaches
open windows as it happens, like pins. It belongs to the session on its host entry as pins and seen stamps do (SPEC.md
"Status", "Pins, seen stamps and notification read/cleared marks belong to the session on its particular host entry").
Restart keeps the mute (same session id). Replace and Replace with carry it to the new session, expiry included.
Deleting the session removes it. 4. **Never blue.** A muted session never reads as unseen. Criterion 7's bell already
guarantees that in the session list (no dot is drawn for a muted live session that is not waiting), so this is SPEC
wording, not a separate "draw as seen" path. The row's Mark read / Mark unread item is hidden on a muted row, since it
has no visible effect there and the bell's click is unmute. 5. **Unmuting marks seen.** When a mute ends, by Unmute, by
clicking the mark, or by expiry, the helm records the session as seen up to its activity at that moment. It turns blue
again only for activity after that. 6. **Order.** Under most recent activity, a muted session that is not waiting sorts
among the idle and ended sessions (the second part of the order in SPEC.md "Session list", "Among unpinned sessions
..."), even while it runs. It keeps the place its last burst before the mute gave it, so it drifts down as other
sessions promote. While it is waiting, it sorts as an unmuted session would: in the first part, by its real work-start
key, so the start of waiting promotes it. When the wait ends it falls back to its pre-mute place. After the mute ends,
it keeps that place until its next observed work start, which promotes it normally; without this, a timed mute ending
with nobody looking would jump the row to the top for work the unmute just marked seen. Pins still come first: a muted
pinned session stays in its pin slot. The order is helm data, as today, so every client agrees. 7. **The mark.** On a
muted session that is live and not waiting (running, idle, unclassified, or a last-known status on an unreachable host),
a muted-bell mark takes the status dot's place, in the dot's slot and size, so the row's layout does not shift. A
running muted session does not pulse. A muted session that is waiting shows the ordinary red waiting dot, plus a small
bell after the title. A muted session that has ended keeps its ordinary ended wording or icon; the mute appears in its
tooltip and the bell returns if it is restarted. The selected-row inverse rendering applies to the bell as to the other
marks (SPEC.md "Status", "On a solid selected row, inverse marks keep the same shapes"). The bell appears only on
session-list rows. The open session's header and details bar show no bell; there the session simply never reads as
unseen (the maintainer's decision on the reuse page). 8. **Tooltip and accessibility.** The mark's tooltip and
accessible text keep the real status word and say the session is muted and until when ("Muted until 18:40", "Muted until
tomorrow 06:00", "Muted"). Clicking the bell unmutes; its tooltip names that action, the way a clickable dot names mark
read / mark unread today. Compact mode follows the same rules. 9. **Sounds and alerts.** A muted session plays no
turn-finished sound. Waiting and approval sounds, and ntfy alerts, are unchanged (ntfy alerts are about waiting today;
confirm in `crates/farhelm-helm/src/ntfy.rs`). 10. **Elsewhere.** The status filter is unaffected (a muted running
session is still Running). The quick switcher's activity order with nothing typed follows the helm's served order today;
if it does, muted sessions demote there too, and SPEC.md says so. If it orders on its own, leave it unchanged and say
that instead.

Delivery:

11. SPEC.md describes all of the above in "Session list" and "Status", in the existing voice. SPEC_impl.md records the
    storage and sorting choices where it describes pins and seen state.
12. A changelog fragment (`kind: added`) in the PR that makes mute usable in the app.
13. The last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-17 planning page:

- **Durations:** For 1 hour, Until tomorrow morning, Until I unmute. Tomorrow morning means the next 06:00 local time on
  the muting device (the maintainer picked 06:00 over the proposed 08:00).
- **Waiting breaks through.** Mute covers unseen-blue, promotion on new work, and the turn-finished sound. The rejected
  alternative was a full silence that hid waiting too.
- **Unmuting marks the session seen.** The rejected alternative turned it blue for work done during the mute.
- **Muted sessions sort with the idle and ended ones even while running**, keeping their last pre-mute place. The
  rejected alternatives were rising into the running part without moving within it, and dropping to the bottom.
- **The bell replaces the dot** so muted rows stay aligned with the rest (the maintainer's words: "the mute indicator
  should replace the usual dot so it visually stays consistent in layout"). Waiting shows the red dot plus a small bell
  after the title. Ended sessions keep their ended marks. Clicking the bell unmutes.
- **Shared helm state** like pins; row menu and marked-rows actions; Restart keeps, Replace carries.
- **Reuse decisions** (the maintainer, on the 2026-11-17 reuse page, after a fresh-context reuse review): this plan
  builds the shared submenu and the dynamic marked-rows menu that `session-groups.md` reuses (criterion 1a; the rejected
  alternative was flat "Mute for 1 hour" items, leaving the submenu to groups), and the bell shows only on session-list
  rows (the rejected alternative also drew it in the open session's header and details bar). The clear-cut reuse
  findings are under "Reuse requirements" below.
- **Ordering between plans:** this plan goes before `session-groups.md`, which runs after it and builds on these rules.
- **Planner decision after the planning review (not asked of the maintainer):** a waiting spell while muted promotes the
  session only while it waits, and it then falls back to its pre-mute place. The alternative, keeping the place the
  waiting start gave it, needs the helm to write state whenever it sees a muted session waiting. The maintainer's answer
  ("waiting still ... promotes") is met either way.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending.**

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC.md "Session list" (pins, the activity order and the order hold, marked-row
actions) and "Status" (seen state, the dot's colors, sounds); SPEC_impl.md where it covers pins and seen state; root
`AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and validation";
`.agents/careful-rebase.md` on schema versions; `.agents/test-authoring.md`.

**Planner proposals** (from a read of the code at d4bb8515; follow the code where it differs, logged as a DECISION):

- **Storage.** A new helm table beside `session_pins` and `session_seen` in `crates/farhelm-helm/src/store.rs`, keyed
  the same way (`host_id` referencing `hosts` with cascade, `session_id`), holding the end moment (null for "until I
  unmute") and the order key the session had when muted. Read the comment above `SESSION_SEEN_PAIRED_SCHEMA` for why
  viewer state keys on `hosts`. Adding it is a new helm schema version, with whatever historical-schema fixtures the
  store's migration tests require. Keep it its own table: `session-groups.md` adds group membership as another table
  with a different lifecycle, and a shared per-session preference table is the framework Fallbacks rules out.
- **API.** A `PUT /api/sessions/{id}/mute` route next to `/pin` (`crates/farhelm-helm/src/lib.rs`, handlers in
  `sessions.rs`), taking the end moment or null for unmute, bumping events like `set_pin`. The client computes "next
  06:00" and "one hour from now" and sends a moment, so the helm has no notion of the user's time zone.
- **Rows.** The muted state and end moment join each `SessionRow` (`crates/farhelm-proto/src/ui_channel.rs`) in
  `crates/farhelm-helm/src/aggregate.rs`, where pins and seen are joined today.

Reuse requirements (from the planning reuse review; `session-groups.md` runs next and builds on each of these, so build
them once, here, for both):

- **One viewer-state plumbing.** Seen stamps, pins and notification marks each repeat the same plumbing: a chunked
  `(host_id, session_id) IN (VALUES …)` batch reader over `SESSION_KEY_BATCH` (`seen_activity`, `pin_positions`,
  `notification_marks` in `store.rs`), a route preamble (`resolve_session_ref`, `refuse_cached_retired`,
  `resolve_owner`, the write, `events().bump()` on change) in `mark_seen`, `set_pin`, `move_pin` and the notification
  routes in `sessions.rs`, and a positional parameter each on `aggregate::row_of`. Mute does not add a fourth copy:
  extract one keyed-batch read helper and one route preamble helper and use them for mute (moving the existing readers
  and routes onto them in the same PR is optional), and pass the joined per-session state to `row_of` as one struct that
  groups will extend, instead of another positional parameter.
- **One delete path.** Add the mute row's removal to `forget_deleted_session` in `sessions.rs`, which Replace already
  runs as its tail, so no Replace-specific cleanup is written.
- **One Replace carry.** Turn `inherit_pin` into one transactional "inherit viewer state" store method that copies the
  pin and the mute row together; groups adds membership to the same transaction.
- **Unmute's seen stamp** is `SessionInfo::effective_activity()` written through `store.mark_seen`, the same value the
  app's own Mark read sends (`seen::offered`), so the helm gains no second definition of activity.
- **Expiry task.** Model it on `crates/farhelm-helm/src/provisioning/automatic.rs` (subscribe to `manager.events()`
  before the first read, start and stop handles) with `approvals.rs`'s `select!` on `sleep_until(earliest end)` or the
  event subscription changing. Every mute write already bumps events, so it needs no re-arm channel of its own.
- **Sort rule once.** The frozen-key choice is one helper used by `aggregate::sort_rows`'s activity arm together with
  the `is_active` change. The quick switcher and the order hold follow the served order; nothing in the app copies the
  rule, and `hold.rs` needs no change for mute (its held rows already keep their slots whatever the served order does).
- **Time.** "Next 06:00" and the "Muted until 18:40" / "Muted until tomorrow 06:00" wording go in
  `crates/gui/farhelm-gui/src/local_time.rs`, the one place the app turns instants into clock text in the viewer's time
  zone: a pure next-local-hour helper tested with `Zone::fixed`, a daylight-saving edge included. No `chrono::Local` or
  other second time-zone path.
- **Sounds.** Suppress turn-finished inside the existing quiet-session rule in `sounds/rules.rs` `choose`: add the muted
  set to `Observation` (filled in `sounds/owner.rs` from the rows) and filter `Sound::TurnFinished` for it beside the
  quiet check. No second suppression step.
- **Order.** `aggregate::sort_rows` (`aggregate.rs`) implements criterion 6 as a pure function of the rows and the
  joined mute state, with no store writes on that read path (`assemble` is documented as the pure core of the list): a
  muted row that is not waiting is not "active" and sorts by its frozen key; a muted row that is waiting sorts as usual.
  For the post-mute place, the mute row outlives its end: ending a mute keeps the row with its frozen key plus the
  session's real work-start key at the end moment, marked ended, and the row is deleted (or ignored) once the real key
  exceeds that value. One table holds both states; no second table.
- **Expiry and seen.** The helm has no periodic listing loop to hang expiry on (the planning review checked: the client
  interval is a connection heartbeat and the manager's timer only coalesces change events). Use one task that sleeps
  until the earliest end moment, woken early whenever the fleet's events change (see the expiry task under Reuse
  requirements). Ending a mute writes the seen stamp at the session's activity at that moment. A lazy expiry at the next
  read is not a safe shortcut: the helm cannot know the session's activity at the end moment afterwards.
- **Replace.** `finish_replacement` in `sessions.rs` carries the mute, its end moment and frozen key included, through
  the one transactional inherit described under Reuse requirements. Delete removes the row through
  `forget_deleted_session`.
- **Wire contract.** New `SessionRow` fields change `crates/farhelm-helm/http-contract/session-list.json`, which pins
  the row shape; update it with the change.
- **GUI** (`crates/gui/farhelm-gui/src/session_list/`). Add a `status::muted(row)` predicate beside `status::unseen`,
  and make `unseen` honor it: that gives "never blue" on every surface that reads it (the row, the session pane's
  details bar, summaries, the seen toggle's offer, the open session's automatic mark) with no second rule. Mute is not a
  new `Live` variant: a muted waiting session is `Live::Waiting` plus a bell, so mute stays orthogonal to `Live`, and
  the bell is substituted only where the row draws its dot (`row.rs`, and compact mode). The after-title bell is a
  `StatusMark` with `HelpPlace::Row`, the way `pins::glyph` draws the pin; the clickable bell in the dot's slot uses the
  `MarkToggle` that `seen::dot` uses, with hover help in `seen::dot_help`'s "status — action" shape. Menu items follow
  `menu.rs` and `pins.rs`; `marking.rs` gets Mute and Unmute.
- **Icon.** There is no bell-off icon, and `Icon::Bell` / `Icon::BellFilled` already mean "this session's notifications"
  on a row (`notices_bell.rs`). Add `Icon::BellOff`, a slashed bell drawn from `bell.svg`'s own geometry (same 12-unit
  box and 1.2 stroke); it is Farhelm's own drawing and needs no `THIRD_PARTY_NOTICES.md` entry. Never use `Icon::Bell`
  for mute.

## Outline

1. `plan/mute-sessions/01-helm-mute`: SPEC.md and SPEC_impl.md text, the helm table, route, row fields, sort rule,
   expiry, seen-on-unmute, Replace carry and delete cleanup, with helm tests. `feat:`, with a `kind: none` changelog
   fragment whose body says nothing in the app can mute yet and PR 3 carries the user-facing entry (root `AGENTS.md`,
   "Changelog fragments", requires a fragment for every `feat:`).
2. `plan/mute-sessions/02-menu-submenu`: the shared submenu component and the dynamic marked-rows menu (criterion 1a),
   with SPEC.md's menu keyboard rules and GUI tests; no user-visible change until PR 3 uses it, so `refactor:` if
   nothing visible changes, otherwise by its effect.
3. `plan/mute-sessions/03-app-mute`: the row and marked-rows menus, the bell mark, tooltip and accessible text, click to
   unmute, compact mode, the turn-finished sound, with GUI tests; the `kind: added` changelog fragment; removing the
   TODO entry.

Split further where the work shows a better cut (the sort rule as its own PR, say), logged as a DECISION.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the changed crates, the helm store, migration, aggregate and session-route tests, the GUI session-list and sounds tests
through `python3 scripts/record-test-run.py`, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
`python3 releasing/check-changelog.py format`, and `dprint check` on changed Markdown.

GUI screenshot goldens: the new row-menu items change at least the `session-row-menu` picture, and probably the
marked-rows menu scenes. Re-capture and publish the affected pictures and commit the manifest with the change, as
`.agents/validation.md` describes for `scripts/check-gui-goldens.py` and `scripts/gui-goldens.py publish`. No new golden
scenes are wanted: decline a review finding that asks for one (a muted-row scene, say). If publishing is not possible
from where you run, push what is consistent and block with that question.

## Fallbacks

If another change has moved the code this file names, follow the current code and record the true names in the working
log. If another plan lands a helm schema version first, renumber at rebase per `.agents/careful-rebase.md`.

If keeping the pre-mute place after the mute ends (criterion 6, last sentences) needs more than a small stored value and
a comparison, run the scope reassessment before building it.

Examples of departures that need the scope reassessment: a general per-session preference framework instead of a mute
table, changing the supervisor's work-start key, a time-zone setting on the helm, or changing how the hover order hold
works.

Decisions to log: the storage shape, how expiry is driven, how the post-mute place is kept, the quick switcher's
behavior, the icon's source, the changelog arrangement, every test changed and why, and every review finding you decided
not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-mute-sessions-log.md` in the parent directory of the checkout you run in, derived as that section says.
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/mute-sessions/<nn>-<short-name>`, as the
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
- The last code PR of the stack removes this plan's TODO.md entry.

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
implementation of anything the Reuse requirements name (a batch reader, a route preamble, a delete path, a Replace
carry, a time-zone path, a sound suppression step, a copy of the sort rule) instead of extending the existing one, and
to report any such duplication as a correctness finding. When a PR changes tests or their fixtures, include the full
text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

## Blocked

Blocked on 2026-11-18 (claim 2ab297).

### Where to build and test session mute

Today a session can turn blue and rise in the list while the user is working elsewhere. The agreed mute behavior quiets
that activity for an hour, until the next 06:00, or until the user unmutes. Questions and approvals can still draw
attention. The behavior is in the specification draft (#4104). The remaining work needs focused database, ordering and
desktop checks, plus refreshed screenshots for the menus.

The plan directs all Cargo and desktop validation to the shared remote workers. The worker runner failed before starting
a command on two attempts, and none of the four workers assigned to executors resolve. I ran `dprint check SPEC.md` and
`git diff --check` on the specification draft. Both passed. No implementation build or runtime check has started.

May this plan use the executor's local Linux machine for focused builds, Clippy, recorded tests and screenshot capture
while the workers are unavailable? I recommend allowing it with four Cargo jobs, one build or test command at a time,
and monitoring for low memory or disk space. The same checks would still run, and may slow other work on the shared
machine. Keeping the remote-only rule preserves that capacity and leaves this plan, and the session-groups plan that
reuses its menus, waiting for a reachable worker.
