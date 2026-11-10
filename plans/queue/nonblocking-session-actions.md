# Session actions stop blocking navigation

Written against main at 9e3ab60b on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a SPEC change the maintainer asked for.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Closing or starting terminals must not block other actions.** Closing a terminal or starting new ones should not
> disable doing other things, like switching to other sessions.

What the maintainer added while planning, in their words: "in particular stopping. you have to blocking-wait as a user
on whatever is going on instead of being able to switch to another session. not sure how blocked you are on starts".

What is true today, from a read of main at 9e3ab60b (line numbers approximate):

- **The page-wide lock.** `crates/farhelm-ui/src/ops.rs` (module docs, `OpLock`, `PaneGate`) holds one live-operation
  token shared by the sidebar and the open session's view. Held by: header Stop from the click, including while its
  confirmation is open, until the reply (`session_view.rs` around 1458-1479 and 2304-2310); header Restart (around
  2129-2133, and the interrupted card's Restart around 2832); Replace (around 1500-1519, 2148, 2704, 2720, and the
  interrupted card's Replace around 2855-2862); header Delete's question (around 2578); a launch from the composer for
  its whole create request (`list/create_form.rs` around 5201, 5822), which includes Restart with and Replace with
  (`HeaderPrefillRequest::RestartWith` and `RestartBridge`, `list/view.rs` around 183-244); and the host mutations and
  add-host form (`hosts.rs` around 2110 claims the bare token and ignores the row count). `ops.rs`'s docs still mention
  a header rename that no longer exists.
- **Row admission checks the token too.** `begin_row_operation` (`list/view.rs` around 705) and `on_stop`,
  `confirm_stop`, `confirm_delete`, `on_replace`, `confirm_replace`, `on_clone` and rename start (around 2470-2976)
  refuse while the token is held; `list/view.rs` has about 25 `ops.busy_now()` sites. Today a host mutation therefore
  refuses row operations, while row operations do not refuse a host mutation.
- **The launch composer is a modal.** `.launch-composer-backdrop` covers the page (`app.css` around 6545) and the
  `ModalDialog` shell makes the rest inert; a hidden quick launch keeps a transparent backdrop on purpose (around 6555).
  SPEC.md around 2967: the quick switcher "does nothing while another modal is open". The composer owns the create
  future, and unmounting it cancels the future and loses track of whether the create happened (`list/view.rs` around
  4895, `create_form.rs` around 6497). So a launch blocks navigation through modality, not only the token. Launch
  success selects the new session (`on_created`, `list/view.rs` around 5044), and Replace success selects the
  replacement (`do_replace` around 2855, `on_replaced` in the header).
- **The sidebar path covers Stop, Delete and Replace, not Restart.** Row Stop is `admit_stop` (`list/view.rs` around
  2370-2460, notices into the row error map as `"Stop: {notice}"`); header Delete is already forwarded through
  `HeaderDeleteRequest` (around 247-264, consumed around 2792); row `do_replace` exists and `ReplaceProgress` is already
  a page-level per-source map (`replace.rs` around 60-68). There is no row Restart (`RowQuestion` in `list/shared.rs`
  around 89-96 has Stop, Delete and Replace); `RestartBridge` is the existing piece that runs a request in an owner that
  outlives the view. Header Replace's YOLO follow-up lives in a view-owned slot (`replace_yolo`, `session_view.rs`
  around 637), while the row path renders its YOLO question in the sidebar (`list/view.rs` around 5225). The per-session
  error map is local to `ListView` (`let mut errors`, around 1048); the `deleting` set is owned by `AppBody` and passed
  to both panes (`lib.rs` around 1946).
- **A view mounted mid-operation.** After Restart the view remounts the agent terminal and bumps its read epoch
  (`mount_generation`, `relaunched`, `restart_terminal_kind`, `restart_epoch`, `session_view.rs` around 1360-1386), and
  around Stop it fences its own detail reads so an old read cannot bring back the tab strip (around 1459-1484). A view
  that mounts while that session's Restart or Stop is pending must see it finish to do the same.
- **Header questions hold the token today**, and that is what stops the sidebar row starting its own Stop or Delete
  under an open header question. Row questions live in `ListView`'s `row_questions` lanes, and `admit_row_op` refuses
  while a lane is open. New terminal and Clone on a session are disabled during its lifecycle action
  (`lifecycle.busy()`, `session_view.rs` around 2265 and 2549).
- **The per-row in-flight set.** Sidebar row Stop, Delete, Rename and Replace, header Delete (forwarded to the row path)
  and bulk batches use a per-session set and a live count instead of the token (`list/view.rs` around 668-721;
  `list/lifecycle.rs`); a bulk batch keeps its rows until every queued row is done.
- **How switching is refused.** A row click is refused if the token is held or any row operation is in flight
  (`list/view.rs` around 3301-3319); the same flag disables every row's open button (around 3991, 5347; `list/row.rs`
  around 863-870, 1797), the quick switcher and automatic selection use the same refusals (around 3376-3401), and
  marking, Clear and the bulk bar are gated (around 3351, 3800, 3854). The quick switcher goes through `guarded_open`
  (around 3301) and automatic selection waits on the token and the count (around 3375-3401). Every header action, New
  terminal included, is disabled while either is held (`session_view.rs` around 2265). Closing one tab and New terminal
  do not take the lock.
- **Why the lock exists** (from `ops.rs`'s docs): (1) a launch choosing a target host and the host mutations (removal,
  adoption, retarget) invalidate each other's premises, so they exclude each other; (2) selecting a row replaces the
  keyed `SessionView` and tears down its tasks, and header Stop, Restart and Replace run inside that view, so switching
  mid-operation would cancel the task waiting for the reply and lose the result (an earlier version also stranded the
  token and left every write action disabled); (3) switching repaints the list an in-flight mutation is about to
  reconcile.
- **Timing.** The UI deadline is 60 s for all of these (`REQUEST_TIMEOUT`, `crates/farhelm-ui/src/api.rs` around 2002).
  The helm awaits the supervisor's final reply (`crates/farhelm-helm/src/sessions.rs` around 2486-2557). The
  supervisor's stop runs in its own task and survives a dropped connection; it gives the agent up to 5 s (`KILL_GRACE`),
  then closes tabs one by one, each with its own grace. Past 60 s, Stop reports a generic error; Restart, Replace and
  launch report an unknown outcome.
- **Feedback today.** Header Stop shows no progress beyond disabled buttons (a view-local `stopping` flag disables only
  its confirm button). Delete shows a spinner on the row and in the header with a scrim. Launch, Restart and Replace
  show a spinner after about 2 s (`launch_progress.rs`).
- **Specs.** SPEC.md: Stop under Lifecycle operations (closes tabs, the reply names failed tabs and the client shows
  that notice); the bulk actions passage around 1949 ("Open buttons, marking, Clear and bar actions obey the same live
  navigation lock. Automatic selection after Delete waits until the accepted batch has settled"); launch progress around
  1253; Delete around 1764-1768; Restart around 1690-1694 and Replace around 1738-1742; the quick switcher around 2973
  ("using the same busy refusals as a sidebar row"); and around 4524, "In the desktop app, an action the user started is
  never lost silently: it either completes and reports its outcome, or reports that its outcome is unknown."
  SPEC_impl.md, "GUI: Dioxus", around 1092, says automatic selection needs no exclusion set because "ordinary selection
  already waits for the count".
- **Tests that pin today's blocking** and will flip: `e2e/tests/terminal.spec.ts` around 3302 (row B's open button
  disabled while A's stop is held, around 3389), `e2e/tests/quick-switcher.spec.ts` around 230,
  `e2e/tests/sidebar.spec.ts` around 1416 (row selection disabled during a held Restart); probably also `header.spec.ts`
  around 1016, `bulk-session-actions.spec.ts` around 955 and 1042 and `stop-tabs.spec.ts` around 218.
  `terminal-tabs.spec.ts` around 1201 and 1348 (per-tab close concurrency) and `sidebar.spec.ts` around 1060 (teardown
  on a direct switch) do not pin the lock and may need no change. Tests hold a request with `routeGate()`
  (`e2e/tests/helpers/route-gate.ts`) and use Playwright's fake clock for deadlines.

### Acceptance criteria

- **Navigation is never blocked by a pending action.** While any of Stop, Restart, Replace, Delete (row, header or bulk)
  or a launch is waiting on its reply, the user can open any other session from the sidebar, the quick switcher and
  automatic selection, and can mark, Clear and use the bulk bar on other sessions.
- **Other sessions stay fully usable.** While session A's action is pending, every action on session B works, including
  its own Stop, Restart, Replace and Delete. The only exclusion left across sessions is reason 1 above: a launch and
  host mutations still exclude each other exactly as today.
- **The acted-on session stays locked.** Its own lifecycle actions, New terminal and Clone stay disabled (and are
  refused in the handler, not only greyed) until its pending action's reply arrives, so two actions never overlap on one
  session. An open header question (Stop, Restart, Replace or Delete confirmation) holds the same per-session claim, so
  the sidebar row cannot start an action under it. Opening the session while its action is pending is allowed and shows
  the pending state, and a view mounted mid-operation sees the operation finish (terminal remount and read fencing after
  Restart, read fencing and refresh after Stop) exactly as the view that started it would have.
- **Results survive a switch.** Header Stop, Restart and Replace run in an owner that outlives the session view (where
  sidebar row actions already run), so their replies and failures are never lost when the user switches away, and
  SPEC.md's "never lost silently" rule holds across switches.
- **A pending Stop shows.** A spinner and "Stopping…" on the session's row and in its header (with its actions
  disabled), as on the planning mockup, from the click until the reply. Restart, Replace and Delete keep their existing
  progress displays, which must also be shown when the user returns to the session mid-operation; whether Restart and
  Replace also get a row indicator is your call, logged as a DECISION.
- **Outcomes reach the user after a switch.** A failure, an unknown outcome or a partial result (such as tabs that could
  not be closed) for a session the user has left shows on that session's row, as row action errors do today, and in its
  header when the user returns.
- **Launch.** Navigation is freed while a launch is pending (Restart with and Replace with included): after submit the
  launch window shrinks to its progress pill and stops covering the page, and comes back for a failure, an unknown
  outcome or a follow-up question. The composer stays busy until its own launch finishes; no second launch from it in
  the meantime.
- **No unwanted jumps.** A finished Replace opens its replacement only if the user is still on the original session;
  otherwise the user stays where they are and the replacement appears in the sidebar. A finished New or Clone opens its
  new session only if the user has not moved on since their last answer to its launcher (revised 2026-11-10; see
  "Revision of 2026-11-10" below, rule 5); otherwise it appears in the sidebar and the user stays where they are.
- **Completions leave the user alone.** Every completion in the scope of "Revision of 2026-11-10" below follows its
  rules 1 to 5, and SPEC.md and SPEC_impl.md say what rule 6 asks.
- **Delete edge cases.** Automatic selection after a bulk Delete still waits until the accepted batch has settled and
  never opens a session the batch is still deleting, by waiting while `AppBody`'s `deleting` set is non-empty (every
  admitted Delete, queued batch rows included, enters it at admission); bulk Stop batches stop affecting selection.
  Selecting a row whose Delete is pending opens it in its existing "Deleting…" state.
- SPEC.md (the bulk passage's navigation lock, the quick switcher's refusals, the Stop passage's pending display, and
  any other passage that describes the lock) and SPEC_impl.md ("GUI: Dioxus", including the automatic-selection
  sentence) describe the new behavior; `ops.rs`'s module docs are rewritten to match.
- The tests that pin blocking are changed to pin the new behavior, and new browser tests cover: switching away during a
  held Stop and seeing its failed-tab notice on the row and in the header on return; acting on session B during A's held
  Restart; the quick switcher during a held Stop; refusal of a second action on the same session; the composer staying
  busy during its launch while navigation works.
- A PR typed by its user-visible effect, with a changelog fragment. The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08, after the planning page explained the three reasons for the lock:

- **All actions** stop blocking navigation: Stop, Restart, Replace, Delete, bulk batches and launches. The alternatives
  were Stop only, or Stop and Delete.
- **Per-session lock**: while A's action is pending, B is fully usable, apart from the launch-versus-host-mutation
  exclusion. The alternative was freeing navigation only.
- **Pending Stop shows** a spinner and "Stopping…" on the row and in the header. The alternatives were row only, or
  nothing beyond disabled buttons.
- **Outcomes after a switch** show on the session's row and in its header on return. The alternative was a page-wide
  notice.
- **The composer stays busy** for its own launch. The alternative was allowing a second launch.
- **Launches are included even though they need the composer made non-modal while pending**, as the stack's last PR,
  decided after the planning review found the composer blocks through modality and not only the lock. The alternative
  was leaving launches as they are.
- **Replace opens its replacement only if the user is still on the original.** The alternatives were never moving a user
  who has switched, or always moving them as today. A launch originally always opened its new session; the maintainer
  revised that on 2026-11-10: a New or Clone now opens its new session only if the user has not moved on, because
  opening it while the user types in another session's terminal sends the rest of their typing to the new agent (see
  "Revision of 2026-11-10" below, rule 5).
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

Changing SPEC.md's navigation-lock wording is the maintainer's request through these decisions, not your own initiative.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words, and the decisions above.

**Binding repository constraints:** the SPEC.md passages listed above (updated as described); SPEC.md's "never lost
silently" rule, which must keep holding; `docs/desktop-web-triage.md`; root `AGENTS.md`, "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`.agents/validation.md`.

**Planner proposals:** moving header Stop, Restart and Replace onto the sidebar's existing per-session operation path
rather than a new owner; extending the existing per-row in-flight set to be the per-session lock; the PR split below.

## Outline

The smallest design found while planning reuses what is there: the sidebar already runs row Stop, Delete, Rename and
Replace per session in a component that outlives the session view, with per-row busy state and per-row errors. The work
is to route the header's Stop, Restart and Replace through that path, then take the page-wide token out of navigation
and row admission, leaving it only for reason 1 (launch versus host mutations) and the composer's own prefill (Clone).

Authorized, following existing precedent (not "a new global operation manager"): moving the per-session error map to
`AppBody` or sharing it the way `deleting` is; a `stopping` set and per-session Restart progress and outcome state
beside `deleting`, which any mounted view of that session watches; a new Restart operation in `ListView` built on
`RestartBridge`'s pattern; and holding the per-session claim for an open header question (reserve the row operation
while the question is open and hand it over to the forwarded action, or put header questions into the `row_questions`
lane; your choice, logged). Once the header actions run in `ListView`, the view claims nothing on the shared token, so
expect `PaneGate` and the `row_ops` count to be removed rather than narrowed, and the token to become `ListView`'s own
again. Sort each of the `ops.busy_now()` sites into: launch versus host mutation (stays), the composer's prefill
(stays), navigation and row admission (goes). Row admission dropping the token means row Stop and Delete become possible
during a host mutation, as a host mutation already runs alongside row operations in the other order; confirm with a read
of `commit_listing` that an adoption's cache purge landing under a row Stop's refetch does nothing worse than a stale
read, and log the DECISION. Header Replace's YOLO follow-up question stays in the header, or moves to the sidebar with a
logged DECISION saying why.

1. `plan/nonblocking-session-actions/01-header-actions-on-row-path`: header Stop, Restart and Replace (the interrupted
   card's included) run in `ListView`, with the shared stopping, error and restart-outcome state the header reads to
   show progress and outcomes. Not behavior-neutral: launches, host mutations and other rows' actions can start while a
   header action is pending, and header errors show on the row; type it by that effect and say so in the PR text.
2. `plan/nonblocking-session-actions/02-per-session-lock`, typed by its effect (likely `feat`): navigation and other
   sessions' actions no longer wait on a pending lifecycle action; the per-session claim in every handler; the
   "Stopping…" display; outcomes on row and header; the Delete edge cases; SPEC.md, SPEC_impl.md and `ops.rs` docs; the
   flipped and new tests; the changelog fragment.
3. `plan/nonblocking-session-actions/03-launch-without-modal`, typed by its effect (likely `feat`): after submit, the
   composer stays mounted but switches to the existing hidden quick-launch presentation (only the launch-progress pill
   shows) and drops its click-blocking backdrop and page isolation while the request is pending; a failure, an unknown
   outcome, a YOLO question or an ambiguous-create recovery shows the modal again, as the hidden quick-launch path
   already does (`quick_launch_hidden.set(false)` on those paths). New and the launch-sequence trigger keep refusing
   while the launch holds the token (`list/view.rs` around 4905 and 4535), which gives "no second launch". Covers
   Restart with and Replace with, which use the composer. SPEC.md's launch-progress passage (around 1250) and the quick
   switcher's modal rule are updated; browser tests; the fragment edited rather than a second one added; the TODO.md
   removal. It is last so PRs 1 and 2 stand on their own if this one blocks.

If PR 1 cannot stand alone, fold it into PR 2 and log the DECISION.

### Validation

The browser specs listed above plus `e2e/tests/launch-progress.spec.ts`, `restart-replace-progress.spec.ts` and any
other spec your change touches, on the engines `.agents/validation.md` names, through `scripts/record-test-run.py`;
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`; `cargo clippy`, `cargo fmt --check` and the
`farhelm-ui` Rust tests; `python3 releasing/check-changelog.py format`.

## Fallbacks

Examples of departures that need the scope reassessment: a new global operation manager or queue, changing helm or
supervisor behavior (an early acknowledgement, say), changing the 60 s deadline, a page-wide notification surface, or
relaxing the launch-versus-host-mutation exclusion.

Decisions to log: where the header actions now run and why, how the per-session lock is enforced in handlers and how an
open header question holds it, the sorting of the `ops.busy_now()` sites, row actions during host mutations, where
Replace's YOLO question shows, what Restart and Replace show on the row, how a mid-delete row opens, and every review
finding you decided not to follow.

## Revision of 2026-11-10: close the completion class structurally

The maintainer revised this plan on 2026-11-10 while it was held, after both landings so far had stopped on the same
class of problem. Read this section before the `## Decisions` entries below. It does two things:

- It changes one earlier decision. The maintainer decided this in conversation while the revision was written: a
  finished New or Clone opens its new session only if the user has not moved on since their last answer to its launcher
  (rule 5). The bullet under "Decisions already made" and the "No unwanted jumps" acceptance criterion are updated to
  match. The latest `## Decisions` entry's item 2 says focus after a launch goes where opening a session normally puts
  it "when the plan already takes the user to the launched session (a New or Clone, which still opens its new session,
  or a Replace with whose source is still selected)". That parenthetical described the plan as it stood then. Its rule
  still holds: focus follows the new session only when the plan takes the user there, and after this revision a New or
  Clone takes the user there only when rule 5 allows it. The same entry says that a later change of the maintainer's
  mind "comes as a follow-up to the plan; it does not stop it". This revision is that change, made in this plan's file
  while it is held, so it is part of this run, not a later follow-up. Neither is a contradiction to resolve by blocking.
- It says how to satisfy the rest of that entry's item 2, what a completion does to wherever the user is now: with the
  rules below rather than one fix per finding. Every sub-bullet of item 2 remains a requirement. The rules are how to
  fix its focus and closing findings. Its other points are fixed as item 2 says, whether or not the rules touch them:
  - confirm in a browser and fix the launcher showing again between waking a suspended sandbox and the create, keeping
    it hidden and released (not isolating the page) across that step, for a quick launch too;
  - the create waiting behind another dialog;
  - focus returning to a row menu's button when it closes during a pending Clone, Replace with or Restart with;
  - focus coming back after the quick switcher refuses a quick launch;
  - SPEC.md's sentence about bulk buttons.

  Items 1, 3, 4, 5 and 6 of that entry also still apply as written.

### Why the per-finding approach did not converge

The first landing stopped on something the user did during a pending launch (opening Rename lost the launch's result).
The second stopped on what a launch's completion does after the user has moved on. Both are the same mistake: code
written when every pending action was modal, and the user could only be where the action left them, still assumes so.
The second landing's findings (focus jumping to New, the Hosts panel and menus closing, a session stuck on "Restarting…"
behind another dialog) were each one place where a completion acts on a surface the user has left. This revision asks
for the rules that make that impossible, enforced in one place, instead of another round of instances.

Focus is moved imperatively across the UI: dozens of places in `crates/farhelm-ui/src` move focus from Rust (most by
evaluating JavaScript that focuses whatever a CSS selector matches), plus the focus rules in `assets/terminal.js`,
`assets/menu-focus.js` and `assets/grid-terminal.js`. A move made in the user's own event handler is fine. A move made
when an asynchronous reply arrives is not, unless it first checks where focus is, because by then the user may be typing
in another session's terminal. Some moves already check: `DialogHandle::close_immediately_if_document_focused` and
`RecoveryFocus` in `crates/farhelm-ui/src/modal_dialog.rs`, #3278's `restore_composer_modal`, and the terminal's own
`takesFocus()` and `keyboardHeldElsewhere()` in `assets/terminal.js`. SPEC_impl.md, "GUI: Dioxus", already says
"Asynchronous retirement uses the existing document-focus guard so a newer approval or control retains focus." Build on
these, not beside them.

A launch's completion is not one handler. Success is `on_created` in `crates/farhelm-ui/src/list/view.rs`, and a Restart
with's success is `on_restarted` beside it. Failures, unknown outcomes and questions go through the composer's request
future in `crates/farhelm-ui/src/list/create_form.rs`, the restart bridge, #3276's `on_replace_failed`, and #3278's
`on_retired` and `admit_composer_return`. Two different holds are involved. The acted-on session's claim (the source of
a Restart with or Replace with) is per session. The composer's operation token is what keeps New, launch sequences and
quick launches from replacing a draft that is still owed back to the user. Some completion behavior is not in any
handler. The layout effect in `list/view.rs` (around line 1445 on main) reads `show_create()` and closes every open row,
host and sort menu whenever it changes, so unmounting a finished launcher closes the user's menus.

### The rules

The rules apply to the completion of every action this plan stops blocking: a launch (New, Clone, a quick launch,
Restart with, Replace with) and a Stop, Restart, Replace or Delete started from a row, the header, the interrupted card
or the bulk bar. They apply whether or not this plan's PRs edit that code, together with any reactive effect a
completion sets off. Nothing else is in scope.

1. **Each completion has up to three parts: settle, return and leave alone.**
   - **Settle, always, as soon as the reply arrives** (before any wait for another dialog to close): clearing any
     "Stopping…", "Replacing…" or "Restarting…" state, releasing the acted-on session's claim, re-enabling its actions,
     putting a failure, unknown outcome or partial result on its row, reattaching its terminal, and refreshing the
     listing.
   - **Settle, only when no dialog is owed back** (a success that opens no naming dialog, or a launcher opening that was
     retired, and then only while that opening is still the mounted one, as #3278's `on_retired` checks, so a newer
     opening keeps its seeds): clearing the pill (rule 5's short report replaces it when nothing navigates), mirroring
     the submitted launch into the held preferences (success only, as today), clearing the launcher's draft seeds
     (`chosen_host`, `clone_prefill`, `initial_create_action`, `ordinary_new_cwd`, the restart bridge), releasing the
     composer's operation token, and unmounting the submitted launcher. Leaving the seeds set would make the next New
     open as a stale Replace with or on the old host. When a dialog is owed back, the pill, the draft and the token stay
     until that dialog is shown, as #3278 already does, so no new launch can replace a draft the user is still owed and
     a failed New is never lost.
   - **Return** is bringing a dialog back: the launcher for a failure, an unknown outcome, a YOLO question or an
     ambiguous-create recovery, and the naming dialog after a quick launch (rule 5 says when it is skipped). A returning
     dialog waits for another open dialog as #3278 already does, then opens and takes focus as that dialog's normal
     opening does. Rule 2 does not apply to a modal's own opening focus: an open modal takes the keyboard anyway
     (`modal_isolation.rs`).
   - **Leave alone** is everything else: a completion never moves focus to New or the marked-sessions menu button, never
     closes a panel, menu or dialog, never installs modal isolation (a hidden launcher re-isolating itself included)
     except as a Return, and never changes the selected session, except as rule 5 allows and automatic selection after a
     Delete (rule 4). Surfaces that belonged to a session the action removed go with it.

   A reply that needs the user's answer (a YOLO question, an ambiguous-create recovery) is not a completion. It is not
   settled as a completion: rule 5's count is read at the answer, not at this reply, and the launcher's Return applies.
   It settles progress as today: the question replaces the progress display, Restart with's terminal reattaches after
   every request as the restart bridge requires, and the composer's operation token is released once the question is
   shown, so the answer's resubmit can take it. What keeps other actions off the session meanwhile is what keeps them
   off today: the returned launcher is modal, and an inline question holds its session's question lane. Showing such a
   question is still asynchronous. A modal question (the launcher) takes focus as its opening does. An inline one, such
   as the YOLO question of a row or header Replace (`YoloConfirmation` in `crates/farhelm-ui/src/yolo_confirm.rs`, which
   focuses its Cancel when it first shows), moves focus only through the rule 2 helper.
2. **Focus moved after an asynchronous event checks first.** Add one helper beside `FocusTarget` in
   `crates/farhelm-ui/src/modal_dialog.rs`, for example a method on `FocusTarget` used only by such paths. It moves
   focus only if, at the moment of the move, nothing holds focus (the document body or document element, the same test
   `close_immediately_if_document_focused` uses) or focus is still inside the surface the move is leaving. The check and
   the `focus()` call are in one evaluated script, so nothing can move focus in between, and it keeps `FocusTarget`'s
   quiet (`focusWithoutTooltip`) behavior. Every focus move that runs after a reply, a timer or another asynchronous
   event in the scope above goes through it, except a returning modal's own opening focus (rule 1). A move made in the
   user's own event handler is the user's own and does not go through the helper. That includes a move the handler
   defers by a frame (`requestAnimationFrame` in `DialogHandle::close`) and an `onmounted` handoff of an element the
   user's own handler mounted. It does not include an effect or `onmounted` focus on something a completion rendered,
   such as an inline question. "Does not go through the helper" does not mean it needs no fix: item 2's row-menu and
   quick-switcher focus returns are such moves and must be fixed. Note one interaction: for the frame after a user
   closes a dialog, focus is on the body until the close's return runs. A completion arriving in that frame may take
   focus, and a return that only happens when nothing holds focus is then skipped. Do not build on the opposite
   assumption.
3. **Reactive effects count.** Include in the inventory every effect that closes, opens, focuses, scrolls or selects
   something and reads state a completion's settle changes, and make sure none acts unless the layout or state it guards
   actually changed for the user. The menu-closing effect above is the known case: a hidden launcher's unmount moves no
   anchor. Another is #3278's presentation effect, which re-isolates the launcher when its pending state clears, which
   is why the pill and pending state stay until a returned dialog is admitted (rule 1).
4. **Whether the user is still there is decided from state.** Where a completion's behavior depends on whether the user
   is still where the action started, write the decision as one small function per kind of action, using state that is
   reliable: the launcher opening the launch came from (the same identity item 3 of the latest Decisions entry ties the
   reply to), the selected session, and rule 5's count. Do not use "focus is on nothing" as evidence that the user is
   still there: on WebKit, a click on a button or on plain content leaves focus on the body. Write such a function only
   where some behavior depends on it and the existing logic is not already one such check: rule 5, and Replace and
   Replace with opening the replacement. Automatic selection after a Delete keeps its existing logic.
5. **A finished New or Clone opens its new session only if the user has not moved on.** New here includes every quick
   launch (the bolt and number shortcuts, launch sequences, the shortcut that opens a quick launch, and the quick
   switcher's "New session named X"). The user has moved on if they have pressed a key or a pointer button anywhere in
   the page since their last answer to this launcher: the submit, or their answer to its YOLO or recovery question.
   Count presses with one listener on `window` in the capture phase, installed when the page loads, keeping a count on
   `window` the way `modal_isolation.rs` keeps `__farhelmModalOpening`, and read the count at the answer and at the
   reply. Do not use the modal-opening count itself: it also counts the launcher's own isolation. Ignore auto-repeat
   (`event.repeat`), a modifier key pressed alone, and the second press of a double click or a repeated Enter on the
   launcher's own submit (`event.detail > 1`, or the same key again within the double-click interval), which SPEC.md
   already guards against submitting twice. For a quick launch, which has no submit the user presses, the baseline is
   read when the launch dispatches. When a completion waits behind another dialog before it acts (the naming dialog
   behind a host result, say), read the count again when it acts. The launcher's own steps (waking a sandbox host,
   dispatching the create, hiding itself) and selection changes the user did not make (automatic selection after a
   Delete) neither reset nor advance the count. Selecting another session or opening a dialog takes a press, so it
   counts.
   - If the user has not moved on, the new session opens exactly as opening it from the sidebar would: the sidebar's
     selection does what it always does (including closing the Hosts panel if it is open, as `on_created` does today),
     and the terminal's reveal rules in `assets/terminal.js` decide focus. The completion moves no focus itself, and the
     comment beside `focusOrigin` in `terminal.js` that relies on the launcher handing focus to New is updated.
   - If the user has moved on, the session is created and appears in the sidebar, nothing navigates, and focus, panels
     and menus stay as they are. A quick launch whose naming dialog would have opened skips it, because that dialog is
     part of opening the session. The session keeps its generated name and can be renamed from its row. It does not
     persist the new session as the remembered selection either, so a reload still opens the session the user was on.
     Whenever nothing navigates, the outcome must still be reported, since the new row may be hidden (by the host choice
     or status filter, or because the Hosts panel is open): the pill shows that the session started, with its name, for
     a few seconds before it goes, and stays display-only. That report is owned outside the launcher, by the list that
     renders it. It does not delay the launcher's unmount, the token's release or the seeds' clearing, so New and
     shortcuts behave as if no launch were pending.
   - Replace and Replace with keep their existing rule (open the replacement only if the user is still on the original),
     and a Restart with never navigates.
6. **Write the rules down.** Add rules 1, 2 and 3 to SPEC_impl.md, "GUI: Dioxus", as the rule for the completions of
   session actions and launches (the scope named above), with the reason: a pending action no longer owns the page, so
   its completion must not act as if it did. Name the helper there. Say that other asynchronous paths in the GUI predate
   the rule, and that the TODO.md entry "Stop asynchronous completions from moving focus or closing things the user has
   moved on from, across the whole UI" covers them. Change SPEC.md wherever it describes a finished launch opening its
   session or the naming dialog appearing, so that it states rule 5, including the pill's short report when nothing
   navigates.

### How to apply them

- Start from item 1's inventory, which has one row per place a pending launch used to block. Add a second table with one
  row per completion path in the scope above: what it settles at once, what it settles when no dialog is owed back, what
  it returns, what it leaves alone, and which rule 4 function, if any, decides. Build it from the code (handlers, the
  composer's future, and effects), not from the earlier review findings. The report gives it in product terms, as
  `plans/.agents/maintainer-writing.md` asks. The list of every focus call site the rules touch, each with how it is
  handled, goes in the working log.
- Put the helper and the SPEC_impl.md text in the lowest PR that needs them, #3276 (its Stop, Restart, Replace and
  Delete replies are completions, and its row Replace shows the inline YOLO question), stated there for session actions,
  and have #3278 extend the passage to launches, so #3276 stands on its own if #3278 blocks. If that does not fit the
  stack, add a PR at the bottom instead.
- Log as DECISIONs, beside those Fallbacks already names: the classification of each completion path, where the helper
  went, and how rule 5's count is kept and read.
- At least two existing tests in #3278's `e2e/tests/nonblocking-launch.spec.ts` change meaning, because they press
  something during a pending launch, which is now moving on. "quick-launch naming waits for a switcher opened during its
  create" must now pin that no naming dialog appears and nothing navigates. The New half of "New selects its accepted
  session after switching during its request" must now pin that the user stays on the session they switched to; its
  Replace with half stands. Search the specs #3278 touched for any other test that presses something during a pending
  New or Clone and then expects its session to open. Keep the naming dialog's wait behind another dialog (a host result
  can still be open); remove only what exists to defer naming behind a dialog the user opened.
- Tests, beside item 4's browser tests:
  - The helper: a browser test showing that focus held by another control (a terminal's input, say) is left alone, and
    that focus on nothing is moved where a real path exists. Test focus inside the surface being left only where a real
    path exists. A Rust test of the generated script may cover what a browser cannot reach without a test-only product
    hook. Do not add such a hook.
  - The case that found this class: focus in another session's terminal when a Replace with finishes, with the next
    keystroke checked to reach that terminal.
  - A row Replace's YOLO question arriving while focus is in another session's terminal leaves focus there, and the next
    Enter reaches that terminal rather than cancelling the Replace.
  - A New failing while the quick switcher is open: once the switcher closes, the launcher returns with its draft, and a
    "New session named X" picked meanwhile was refused rather than replacing that draft.
  - Rule 5, each with the next keystroke checked where it applies:
    - a New and a Clone finishing while the user types in another session's terminal leave the user there, with the new
      session in the sidebar;
    - a New finishing when the user has not moved on opens the new session as before;
    - a New that needed a YOLO answer, then succeeded with no further input, opens its session;
    - a quick launch to a suspended sandbox host with no input after it starts opens its session, and one while the user
      types in another session's terminal never takes that terminal's focus or swallows a key during the wake;
    - a quick launch with naming finishing after the user moved on shows no naming dialog;
    - a New finishing after the user moved on to the Hosts panel shows the pill's short report, and New pressed during
      that report opens a fresh launcher;
  - Rule 3: a row menu opened during a pending launch stays open when the launch finishes.

### Scope

All of this is authorized and is not one of the departures under Fallbacks. One shared helper, a settle, return and
leave-alone split of existing completion code, a press count and the pill's short success report are not "a new global
operation manager" or "a page-wide notification surface". Do not rewrite how open dialogs isolate the page:
`modal_dialog.rs` and `modal_isolation.rs` already keep focus inside an open dialog. Do not change completion paths
outside the scope named under the rules; the TODO.md entry named in rule 6 covers the rest of the UI. If applying the
rules shows that #3276's or #3278's structure needs more than this (moving a handler's state to another component, say),
that is a scope reassessment, as Fallbacks describes.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-nonblocking-session-actions-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/nonblocking-session-actions/<nn>-<short-name>`, as the outline names them.
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
already made, outline, the "Revision of 2026-11-10" section and the `## Decisions` entries). When a PR changes tests or
their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md`
requires.

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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-11-10: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-11-10 (claim 73d7db).
>
> ### Landing stopped: a launch's result can be lost, and browser tests the plan did not run now fail
>
> This plan makes pending session actions stop blocking the rest of the page. Before it, while a Stop, Restart, Replace
> or Delete was waiting for its reply, or while a new session was launching, you could not switch to another session or
> act on one: the whole page was locked until the reply came. Now only the session being acted on refuses competing
> actions, and a submitted launch hides its launcher window and shows a small progress indicator at the bottom of the
> window (the "pill") while you carry on elsewhere. If the launch fails or needs a question answered, the launcher comes
> back with your choices.
>
> The landing monitor stopped the landing before merging anything: neither of the plan's PRs (#3276 and #3278) is on
> main, and main holds nothing of the plan. Two things stopped it: one product bug and a set of browser tests that still
> expect the old, locked page.
>
> #### The product bug: renaming a session during a launch loses the launch's result
>
> Rename is one of the actions the plan now allows on other sessions while a launch is pending, as its SPEC.md amendment
> says. But the launcher keeps a count of how often it has been opened, so that a reply belonging to an earlier,
> abandoned opening is ignored rather than shown in the wrong place. Opening Rename still advances that count, so the
> pending launch's reply looks as if it belongs to an abandoned launcher and is thrown away. When that reply is a
> failure, an unknown outcome or the question about launching in YOLO mode, the user sees:
>
> - no error and no question, and the pill counting up for as long as the page stays open;
> - for Replace with, the original session showing "Replacing…" until the page is reloaded;
> - for Restart with, the session showing "Restarting…", with its Stop, Restart, Replace, Delete, New terminal and Clone
>   disabled, until the page is reloaded.
>
> The quick switcher, which the plan also opened up during launches, does not advance the count; Rename should not
> either. As it stands this breaks SPEC.md's rule that a launch's outcome is never lost silently.
>
> #### The failing browser tests
>
> The landing ran the whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes (cloud test machines) at
> the maintainer's request: 3,117 passed, 14 skipped by design, 49 failed (runs `830543a7`, `bec04404`, `b7c14276`,
> `bd2730d7`, `cf210c48`, `7cd61551`, `eff5805d`, `10f321ec`, `506cb9ab`, `e229c9a9`). The executor's own run covered 11
> of the suite's spec files; most of these failures are in others.
>
> - 8 come from main, not from this plan: the host-settings footer and pinned-controls tests in the multi-host spec, and
>   the feed spec's reader count. A separate draft fix (#3275) covers them. Until it lands, a rerun shows them failing
>   again, and that is not this plan's doing.
> - 33 fail because the tests still expect the locked page: the launcher staying visible and focused while a request is
>   pending, the launch's progress showing inside the launcher rather than in the pill, or an open Stop or Delete
>   question disabling that session's open button. They are in the Restart with spec (12), the YOLO question spec (10),
>   the Tensorlake lifecycle spec (6), the window chrome spec's "selected session identity stays legible while
>   confirmation disables navigation" (2), the template shortcut spec's "preserves editor focus when clearing a
>   quick-launch YOLO question" (2), and the sidebar spec's "composer menu-closed Tab order" (WebKit only).
> - 8 need a closer look, because they may be real regressions rather than stale tests:
>   - On both engines, the session confirmations spec's "skip session confirmations walks row header interrupted card
>     and tab actions": after a Delete, the session Farhelm picks next is not selected. The plan changed when that
>     automatic pick happens.
>   - On one engine only, and in code the plan changed: the bulk actions spec's "bulk Stop reports an engaged row
>     without skipping its eligible sibling", and the multi-host spec's "a moved host is a new intent" (both WebKit).
>   - On one engine only, and probably unrelated: the trash dialog's "header and global confirmation remain reachable"
>     at two widths (Chromium), the terminal spec's colour for a session with unseen output, and the recipe snapshot
>     test (both WebKit).
>
> #### Smaller points from the landing's review
>
> - Suspend and Resume of a sandbox, when started from a session's row or its top bar, no longer wait while a launch or
>   a change to the host list is pending, so they can now overlap. The Hosts panel's own Suspend and Resume still wait.
>   Whether the overlap does harm is unknown; the goal file lists relaxing this as a departure that needs reassessment,
>   and the report does not mention it.
> - The row's new "Stopping…" status has no styling, unlike the existing "Deleting…" status beside it.
> - The sidebar's question about replacing a session in YOLO mode still shows its buttons disabled during any launch,
>   although they work.
> - One older browser spec's opening comment still describes the page lock the plan removed.
> - The changelog entry leaves out the "Stopping…" status, results staying on a session's row after you switch away, and
>   Replace no longer pulling you away from a session you switched to.
>
> #### Question
>
> How should the plan go on?
>
> - Send it back to fix (recommended). The executor fixes the Rename bug the way the quick switcher avoids it, keeping
>   Rename available during a launch as the amended spec says, and adds a test for it. It updates the 33 stale tests,
>   reruns the 8 uncertain failures narrowly on both engines and fixes those the plan caused (for the Delete case, the
>   amended spec still selects another session once a Delete settles, so the product is fixed unless the test's own
>   premise is what changed), and fixes the smaller points. It also settles the Suspend and Resume point by keeping them
>   waiting as before, the smaller change; the alternative is to keep the overlap and say in the report why it is safe.
>   Cost: another executor round and another landing run.
> - Send it back, but make Rename refuse during a launch, as before the plan. A smaller fix, but it keeps one piece of
>   the locked page the plan set out to remove, and the spec amendment would need narrowing.
> - Land it as it is and fix forward. Main would get the Rename bug, 33 failing browser tests that every other landing
>   would then have to tell apart from its own, and up to 8 possible regressions, among them a Delete that no longer
>   selects the next session.

The maintainer's answer:

The landing monitor answered this on the maintainer's behalf, under `plans/.agents/landing.md`, "Answering blocks": the
landing failed mainly because tests fail, plus one product bug, so the plan goes back to the executor to fix them. A
later change of mind is a follow-up, not a reason to stop the plan.

Fix, then deliver again:

- Opening Rename while a launch is pending must never lose that launch's outcome. Keep Rename available during a launch,
  as the amended SPEC.md says, and keep it from retiring the hidden launcher, the way the quick switcher already does.
  Add a browser test that opens Rename during a held launch that is then refused and checks the refusal comes back.
  Where a pending launch finds its launcher retired, it should still clear its pending state (the pill, Replace with's
  "Replacing…", Restart with's claim) rather than leave it stuck.
- Update the 33 stale browser tests listed in the Blocked section to the new behavior.
- Rerun the 8 failures the Blocked section says need a closer look narrowly on both engines. Fix those the plan caused;
  for the session confirmations case, the amended spec still selects another session once a Delete settles, so fix the
  product unless the test's own premise is what changed. For the others, record in the report what the narrow reruns
  showed.
- Keep session-facing sandbox Suspend and Resume (from a session's row and top bar) exclusive with a pending launch and
  with host changes, as the Hosts panel's own Suspend and Resume still are. The goal file lists relaxing that exclusion
  as a departure needing reassessment, and keeping it is the smaller change. This is a judgment call the monitor made on
  the maintainer's behalf, recorded in TODO.md's "Plan decisions to revisit".
- Fix the smaller points: style the row's "Stopping…" status like "Deleting…", let the sidebar's Replace YOLO question
  answer during a launch, update the session-view-claims spec's header comment, and cover the "Stopping…" status,
  results staying on the row after switching away, and Replace no longer pulling you away in the changelog fragment.
- Before delivering, run on both engines every spec file named in the Blocked section, together with the 11 the plan
  already ran. Before choosing that selection, search the other spec files for tests that wait on the launcher while a
  request is pending, since the whole suite is what found these. The landing runs the whole suite again. Failures that
  main causes, which a separate fix covers (the multi-host host-settings tests and the feed spec's reader count), can be
  named as such in the report.

### 2026-11-10: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-11-10 (claim cafa53).
>
> ### Landing stopped: a finished launch still acts as if the launcher owned the page
>
> This plan makes pending session actions stop blocking the rest of the page. Before it, while a Stop, Restart, Replace
> or Delete waited for its reply, or while a new session launched, the whole page was locked until the reply came. Now
> only the session being acted on refuses competing actions, and a submitted launch hides the launcher and shows a small
> progress indicator at the bottom of the window (the "pill") while you carry on elsewhere.
>
> The plan has two PRs. #3276 frees the page during a pending Stop, Restart, Replace or Delete. #3278 frees it during a
> launch, which covers New, Clone, and the launcher-based "Restart with" and "Replace with" (where you edit the launch
> before it runs).
>
> The landing monitor stopped this landing before merging anything: neither PR is on main, and main holds nothing of the
> plan.
>
> #### What the maintainer asked for
>
> The previous landing found that opening Rename during a launch threw away that launch's result. The maintainer then
> asked for two things if this landing failed again: send it back requiring the executor to close the whole class of
> problems where something the user does during a pending action collides with it, rather than one finding at a time,
> because every landing so far has found another one; and put the plan on hold, so that other plans make progress
> meanwhile.
>
> #### What the landing found
>
> The landing's independent review read the code for every place the old page lock used to block (the row menus, the top
> bar, the sidebar, the quick switcher, keyboard shortcuts, the host rail and its dialogs, the bulk bar, Templates, New,
> Clone, Rename, Settings, Trash, sandbox Suspend and Resume). It found no remaining way to lose or strand a pending
> launch: each of these either refuses while a launch is pending or leaves the launch alone, and a launch whose launcher
> was closed or reopened meanwhile still clears the pill, Replace with's "Replacing…" and Restart with's lock on the
> session.
>
> What remains is the other side of the same class: when a launch finishes, it still treats the page as if the launcher
> had owned it, which was true before this plan and is not now. All of this is in #3278. The landing monitor confirmed
> in the code that a successful launch moves focus to New and closes the Hosts panel; everything else below comes from
> the review's reading of the code. None of it was observed in a browser.
>
> - After a launch succeeds, keyboard focus moves to the New button wherever the user is. Run Replace with on one
>   session, switch to another and type into its terminal: when the replacement finishes, you stay on the other session
>   (as the plan intends), but your keystrokes stop reaching its agent, and the next Enter or Space opens a new
>   launcher. A successful launch also closes the Hosts panel if you had opened it meanwhile, and closes any row, host
>   or sort menu you had opened. No test checks focus or open panels after a launch completes.
> - When Restart with or Replace with fails while another dialog is open (Settings, Rename, the quick switcher), the
>   acted-on session keeps showing "Restarting…" or "Replacing…", with its actions disabled and its failure hidden,
>   until that dialog closes. SPEC.md says only the launcher waits for another dialog, not the session's own state.
> - A launch to a suspended sandbox host appears to show the launcher again for a moment between waking the sandbox and
>   creating the session, which could take focus from whatever you are typing into; and the create itself waits while
>   another dialog is open, although the pill says the session starts when it is ready.
> - Smaller points: while a Clone, Replace with or Restart with is pending, closing any row menu loses focus; a refused
>   quick launch from the quick switcher does not give focus back; and one SPEC.md sentence still describes bulk buttons
>   locked during a launch, which no longer happens.
>
> #### Checks
>
> Everything else the landing ran passed. The whole browser suite ran on Chromium and WebKit, on ten Tensorlake
> sandboxes (cloud test machines) at the maintainer's request, with
> `scripts/tensorlake/sandbox-tests.ts run --template <template> --shards 10` and no projects named, each shard recorded
> through the recorder's generic mode: 3,175 passed, 14 skipped by design, 3 failed (runs `22625548`, `72ebf196`,
> `ed7247b1`, `53af9bd7`, `c86e5443`, `a0b82874`, `61e890a4`, `a2de92ab`, `8432fda2`, `1d7062fc`). The three failures
> are flakes already on record, in code this plan does not touch: two Chromium host-setup tests in the provisioning spec
> ("expanded setup failure closes the menu below it" and "incarnation churn keeps a followed run while a target change
> detaches it") and the window chrome spec's Chromium version double-click. The previous landing had 49 failures: 8 came
> from main and were fixed separately by #3275, which has since landed, and the other 41 no longer fail after the
> executor's changes (stale tests updated, the regressions among them fixed). The UI crate's Rust tests passed
> (`cargo nextest run -p farhelm-ui` through the recorder, 944 passed, run `b76fcf27`), as did
> `cargo fmt --all -- --check`, `scripts/check-test-sleeps.py`, `dprint check`, `releasing/check-changelog.py format`,
> `scripts/check-agents-md.py`, `cargo build`, the release web build
> (`dx build --package farhelm-ui --platform web --release`), `cargo clippy -p farhelm-ui --all-targets -- -D warnings`
> and `cargo check -p farhelm-ui --features desktop`.
>
> #### Question
>
> How should the plan go on?
>
> - Send it back to close the completion side of the class as a whole, and put it on hold (recommended, and what the
>   maintainer asked for if this landing failed again). The executor would go through everything a launch's completion
>   does (success, failure, unknown outcome, question) to wherever the user now is: focus, open panels, menus and
>   dialogs, selection, and the acted-on session's own status. It would fix each place that disturbs the user and pin it
>   with tests, run the whole browser suite before delivering, and get a second fresh review aimed at what the user can
>   do while a launch, Stop, Restart or Replace is pending. Cost: another round once the maintainer takes it off hold.
> - Fix the focus jump in the landing and land now. Users get the whole plan at once, and the remaining points are
>   smaller. But it patches one instance of a class the maintainer asked to see closed as a whole, and leaves the rest
>   of the completion behavior unexamined.
> - Land #3276 alone now, and split #3278 into its own plan, as the plan's outline allows. Users would get a pending
>   Stop, Restart, Replace or Delete that no longer locks the page right away, and every finding above stays with the
>   launch half. Not recommended for now: the launch half is converging (this round found no lost or stranded launch,
>   only side effects of a launch finishing, and the browser suite is clean apart from known flakes), and splitting
>   costs a replan.

The maintainer's answer:

The landing monitor answered this on the maintainer's behalf, under `plans/.agents/landing.md`, "Answering blocks",
following two instructions the maintainer gave for this plan before this landing: a further send-back must have the
executor close the whole class of problems rather than the individual findings, because fixing them one landing at a
time has not converged; and if the plan came back blocked again, it goes on hold so that other plans make progress
meanwhile. So the monitor took the first option: send it back to close the class, and put the plan on hold. It runs
again only when the maintainer takes it off hold. Both PRs stay in this plan and are delivered together. Should the
maintainer later want any of the instructions below changed, that comes as a follow-up to the plan; it does not stop it.

The class is everything the user can do while a launch, Stop, Restart, Replace or Delete is pending, and everything that
pending action's completion does to wherever the user has gone meanwhile. Items 1 to 4 cover launches (#3278); item 2
also covers the completion of a pending Stop, Restart, Replace or Delete (#3276). Close it, then deliver again:

1. List, in the report, every place where a pending launch used to block the user, either by refusing the action or by
   the launcher covering the page: row menu items, the top bar's actions, the sidebar, the quick switcher, keyboard
   shortcuts, the host rail and its dialogs, the bulk action bar, Templates, New, Clone, Rename, Settings, Trash, and a
   sandbox's Suspend and Resume. For each, say what it does to the pending launch (whether it can reopen, replace or
   hide the launcher, change its draft or destination, or affect Replace with's or Restart with's lock on their session)
   and why that is safe. Build this table yourself from the code. The landing's review found every one of them either
   refuses while a launch is pending or leaves the launch alone; the report should show that for each, and fix any that
   does not.
2. Do the same for completion: what a launch's success, failure, unknown outcome, YOLO question or the dialog that asks
   to name a new session does, and what a pending Stop, Restart, Replace or Delete's reply does, to keyboard focus, the
   Hosts panel and other open panels, open menus, other open dialogs, the selected session, and the acted-on session's
   own status. Fix each place that disturbs a user who has moved on. In particular:
   - A successful launch must not move focus to New, close the Hosts panel or close a menu the user opened meanwhile,
     except when the plan already takes the user to the launched session (a New or Clone, which still opens its new
     session, or a Replace with whose source is still selected); then focus goes where opening a session normally puts
     it. A Restart with never moves the user, so its success leaves focus and panels alone.
   - When Restart with or Replace with fails or has an unknown outcome while another dialog is open, settle the acted-on
     session at once: clear "Restarting…" or "Replacing…", re-enable its actions, show the failure on its row, and
     reattach its terminal. Only the returning launcher waits for the other dialog.
   - The landing's review read from the code, without seeing it in a browser, that a launch to a suspended sandbox host
     shows the launcher again for a moment between waking the sandbox and creating the session, and that the create
     waits while another dialog is open. Confirm that in a browser first; if it holds, keep the launcher hidden across
     that step and do not hold the create back for another dialog.
   - While a Clone, Replace with or Restart with is pending, closing a row menu must return focus to the menu's button,
     and a quick launch that the quick switcher refuses must give focus back.
   - Update SPEC.md's sentence about bulk buttons being locked during a launch, which no longer happens.
3. As hardening (the landing's review found no current path that strands a launch): make sure that whenever a launch's
   reply arrives after its launcher was closed or reopened, the progress pill, Replace with's progress and Restart
   with's lock on the session are still cleared. Today a reply finds its launcher by a counter read when the launch was
   submitted; tie it instead to the launcher opening it was submitted from, so that nothing advancing the counter in
   between can leave a launch unsettled, and pin that with a test.
4. Pin item 2 with browser tests that check focus and open panels, not only which session is selected, after each kind
   of completion while the user is on another session.
5. Before delivering, run the whole browser suite on both engines, not a named subset, through the test recorder as
   `.agents/validation.md` describes for the full browser run, and account in the report for every failure. Main's own
   browser-test fix (#3275) has landed, so the host settings footer and feed reader-count failures should no longer
   appear; any such failure is new.
6. Besides the usual review gate (the fresh GPT review the plan already uses), get one fresh-context review from a
   Claude Opus session (started with `claude -p`), aimed at what the user can do while a launch, Stop, Restart or
   Replace is pending, and what each one's completion does to where the user is.

The landing's whole browser suite on this delivery had three failures, all flakes already on record in code this plan
does not touch (two host-setup tests in the provisioning spec and the window chrome spec's version double-click, all on
Chromium). Name them as such in the report if they come back, and account for anything else.
