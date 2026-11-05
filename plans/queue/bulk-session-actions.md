# UI: select several sessions and stop, delete, pin or mark them at once

Written against main at d81a3980 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - Allow shift selection of multiple sessions for bulk actions such as closing them.

The maintainer's words when choosing it, verbatim: "only the simple actions that don't lead to follow-ups that don't
apply in bulk". On the planning page, about the bulk Delete confirmation mockup: "also the popup should be more
structured looking. like one row per agent cleanly structured with its status".

### What changes

Today "selected" means the one open session, the one in the main pane (SPEC.md's session list text, the `selected`
signal, `data-session-selected`, `aria-current`), set by clicking a row's open button (`.session-row-open` in
`crates/farhelm-ui/src/list/row.rs`, `guarded_open` in `crates/farhelm-ui/src/list/view.rs`); nothing reads modifier
keys. Sessions have no "close": Cmd+W on a session is **Delete** (SPEC.md, "Desktop window chrome"), so "closing them"
means Delete, and Stop is the other lifecycle action.

The new set of rows needs its own name everywhere (SPEC.md, code, test hooks), distinct from "selected": call it the
**marked** sessions (or another word that does not collide; log the choice).

- **Marking:** Shift+click on a row marks the range from the open session to that row, in displayed order. A click with
  Cmd or Ctrl held toggles one row; accept either modifier on every platform (Ctrl+click on a Mac is a context-menu
  click there anyway, so the decided "Cmd on macOS, Ctrl elsewhere" gesture is covered without platform detection). The
  open session does not change and keeps its own highlight; marked rows get a distinct marker (a side bar and tint in
  the mockup). The range includes the open session; a modifier click can toggle the open session like any other row. A
  plain click opens a session as today and clears the marks. Shift+click must not extend the page's text selection
  (likely a conditional `preventDefault` on `mousedown`); check this early on WebKit, and keep drag-to-select text
  working.
- **Effective marks:** the stored marks are a set of session keys; what counts (for the bar's count, its offered actions
  and the targets) is that set intersected with the rows displayed now. Hidden rows are never counted or acted on. Prune
  keys for sessions that vanished only where `commit_listing` already prunes `row_questions` (when the reply is
  authoritative).
- **The bulk bar:** while the effective marks are not empty, a bar replaces the list title row, reading "N selected" (or
  "N marked"; match the chosen name) with **Stop**, **Delete**, and a ⋯ menu holding **Pin** / **Unpin** and **Mark
  read** / **Mark unread**, plus ✕ to clear. Stop and Delete are offered for any non-empty marking and apply to every
  marked row, exactly as the row menu offers them on every row (`row_control_visibility` in `row.rs`). Pin is offered if
  some marked row is unpinned and applies to those; Unpin likewise; Mark read / unread as the row menu offers them per
  row. Escape clears the marks only when focus is in the session list, without preventing default handling, and never
  when the terminal, a menu or a dialog owns the key (the planning mockup said "✕ or Escape clears the selection").
- **Bulk Delete confirmation:** if any marked session would need a confirmation on its own
  (`DeleteTarget::needs_confirmation` in `list/shared.rs`: not ended, or has terminal tabs), one combined confirmation
  opens, unless Skip session confirmations is on. It is a `ModalDialog` with one row per session: its status mark and
  name, its host, and what Delete will do to it, worded per row by the existing `status::confirm_consequence` (which
  words every status, including Unknown without a liveness claim) and the single prompt's conditional checkout sentence
  in `row.rs` (the checkout moves to the trash only when its last session is deleted; do not promise a move from a
  client-side count). It opens through a `ConfirmSlot` (SPEC_impl.md, "Answers bind to what was drawn"; `ops.rs`) whose
  payload is the per-session snapshot the dialog draws, and each session's `DeleteGuard` is derived from that snapshot
  with `status::delete_guard`. The trash dialog (`list/trash.rs`), whose confirmation freezes the targets it drew, is
  the nearest precedent. If a listed session disappears or changes while the dialog is open, it follows what the single
  prompt does for its row (drop it or reword it); the guards keep the answer bound to what was drawn.
- **Running the actions:** Stop and Delete run one request at a time per host, hosts in parallel (the trash dialog's
  precedent). Unbounded concurrency is wrong here: each host's supervisor admits at most 8 management requests
  (`HANDLER_ADMISSION_PERMITS` in `crates/farhelm-supervisor/src/service/core.rs`, `admit_or_refuse` in
  `service/connection.rs`), refuses extra Stops at once as "host is busy", and queues extra Deletes while refusing every
  other management request on that host. Pacing needs to know when each request finishes, and today `on_stop` and
  `do_delete` in `view.rs` spawn and return nothing about the outcome: lift their request-and-bookkeeping bodies into an
  awaitable piece that both the single and the bulk paths call. Pin / Unpin and Mark read / unread call `on_pin` and
  `on_mark_seen` per session as they are; those deliberately bypass `row_pending` and need no pacing.
- **Results:** a session whose action fails keeps its row and shows its error on its own row, as a single action's
  failure does. A row the bulk run could not start (busy, its own question or rename open) gets the same "not started"
  message the header Delete path sets, rather than being skipped silently. The marks clear when the run finishes. While
  any row op runs, the open buttons are disabled (`nav_locked`), so marking cannot change during a run, and the bar's
  actions are disabled under the same lock.
- **The open session in a bulk Delete:** automatic selection after a delete must not pick another session from the same
  batch that is being deleted (exclude ids being deleted from `automatic_selection`, or delete the open session last;
  log which).

### What does not change

Rename, Clone, Replace, Replace with and Restart stay single-session: they have follow-ups that do not apply in bulk.
Keyboard shortcuts (Cmd+W, Cmd+L and the rest) keep acting on the open session only, whatever is marked. The quick
switcher, filters and sort are unchanged. The single-row menu keeps working on its own row whether or not that row is
marked. No new helm endpoints.

### Acceptance criteria

- Shift+click and Cmd/Ctrl+click mark rows as described without opening the clicked session; text selection by drag
  still works; a click on a row's controls (menu button, status dot, bell) behaves as today.
- The bar appears, counts and offers actions from the effective marks, and runs them on exactly those sessions.
- Bulk Stop of more sessions on one host than the supervisor admits at once stops all of them, one at a time per host.
- Bulk Delete opens the combined, structured confirmation exactly when at least one marked session would need one and
  confirmations are not skipped; it goes through a `ConfirmSlot`; every session carries its own guard from the drawn
  snapshot.
- Failures and refusals are reported on their rows without undoing the successes; automatic selection never opens a
  session that the same batch is deleting.
- Escape in the terminal still reaches the terminal and does not clear the marks.
- SPEC.md describes marking (with its own word), the bar, which actions apply, the pacing, the combined confirmation and
  its authorization rule (a confirmation authorizes only what it listed), and that shortcuts act on the open session
  only. SPEC_impl.md records where the marks live, the awaitable Stop/Delete core and the per-host pacing. Docs:
  `website/src/content/docs/docs/using/session-list.mdx` and `stop-restart-resume.mdx` ("Stop or delete").
- Browser tests on Chromium and WebKit: range and toggle marking, the bar's offered actions, bulk Pin and Mark read,
  bulk Stop on more than eight sessions of one host if the stack can make that many cheaply (otherwise a test that
  pacing holds one request per host at a time), bulk Delete with and without the combined confirmation, Skip session
  confirmations, a refused row, Escape in the list and in the terminal, and shortcuts still acting on the open session.
  The desktop evidence is the WebKit run plus a compile of the UI crate's desktop build; the executor cannot drive the
  Mac app, and says so in the report.
- A changelog fragment (`kind: added`) and removal of the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-04 on a planning page with mockups (a dark mockup of three tinted, side-barred
rows under a bar reading "3 selected · Stop · Delete · ⋯ · ✕", with the open session keeping its blue highlight; and a
"Delete 3 sessions?" confirmation, which the maintainer asked to make more structured, one row per session with its
status):

- **Gestures:** Shift+click range from the open session, plus Cmd+click (Ctrl+click elsewhere) to toggle one row.
- **Actions:** Delete, Stop, Pin / Unpin, Mark read / unread. Nothing with a follow-up that cannot apply in bulk.
- **Bulk Delete of sessions that need confirming:** one combined confirmation, structured as one row per session with
  its status (the maintainer's words above). Chosen over deleting only sessions that need no confirmation.
- **Shortcuts:** keep acting on the open session only; Cmd+W does not delete a marking.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words and the decisions above.

**Binding repository constraints:** SPEC.md's session-list text on the selected session and automatic selection, the row
menu, Stop, Delete and its confirmation and delete-precondition rules, Skip session confirmations, "Waiting between
operations on one host", and "Desktop window chrome" for shortcuts; SPEC_impl.md, "Answers bind to what was drawn", and
its account of per-row operations (`crates/farhelm-ui/src/ops.rs`); root `AGENTS.md`, "Code rules", "Tests and
validation", "Conventional Commits", "Changelog fragments"; `website/AGENTS.md`; `.agents/test-authoring.md`;
`.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code: the marks as a page-local signal of session keys beside `row_questions` in `view.rs`,
not persisted; the bar rendered in place of the list title in the same container; two PRs, each with its own SPEC.md,
docs and changelog text: (1) marking, the bar, bulk Pin / Unpin and Mark read / unread, adding the fragment; (2) the
awaitable Stop/Delete core, paced bulk Stop and Delete with the combined confirmation, editing PR 1's unreleased
fragment, and the TODO.md removal.

## Outline

1. **Marking and the bar.** Modifier-aware clicks on the row's open control, the marks set and effective marks, row
   markers, ✕ and list-scoped Escape, pruning; the bar with Pin / Unpin and Mark read / unread; SPEC.md, docs and
   fragment for this much.
2. **Stop and Delete.** Lift `on_stop` and `do_delete` into an awaitable core used by both paths; per-host pacing;
   refused-row messages; the combined confirmation through a `ConfirmSlot`; automatic selection excluding the batch;
   SPEC.md, SPEC_impl.md and docs completed (an incidental docs edit in an unattended run: start no preview server and
   name the changed pages in the report); the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy on the UI crate (including
its desktop feature build), the UI crate's tests, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and, through `python3 scripts/record-test-run.py`, a new bulk spec plus
`sidebar.spec.ts`, `session-confirmations.spec.ts`, `delete-precondition.spec.ts`, `session-action-shortcuts.spec.ts`
and `pins.spec.ts` on Chromium and WebKit. Run `python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: new helm or supervisor batch endpoints; raising the supervisor's admission
limit; bulk Rename, Clone, Replace or Restart; shortcuts acting on the marks; keyboard-only range marking
(Shift+arrows); marks persisting across reloads; a bar-level failure summary.

Decisions to log: the name for the marked set, how the awaitable core is shaped, per-host pacing details, how the open
session is kept out of automatic selection during a batch, what the combined dialog does when a listed session changes,
and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-bulk-session-actions-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/bulk-session-actions/<nn>-<short-name>`
  (for example `plan/bulk-session-actions/01-selection` and `plan/bulk-session-actions/02-stop-delete`).
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
