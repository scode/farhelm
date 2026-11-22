# Shared helpers: hosts, approvals, settings and the session list

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

This plan runs after `triage-gui-misc.md` has landed: that plan changes the Hosts panel, approval cards and Settings,
which this one consolidates.

## The goal

This plan covers part of one TODO.md entry (bucket "Near term", directly under the bucket, not in a subsection). Four
sibling plans split that entry by area; this one owns the Hosts panel, approvals, operations, Settings, the session list
and the session pane. Quoted verbatim below: the entry's heading paragraph, the bullets this plan owns, and the closing
line with the plan references the planning PR adds. The entry's other bullets belong to the sibling plans and to
`wide-glyph-padding-cells`.

> - **Consolidate the helpers duplicated across Farhelm.** Work through the repeated helpers from the 2026-11-21 review.
>   Reuse the existing shared module where one fits and keep each helper scoped to its actual callers. Locations below
>   are from the reviewed revision `c9575db61abb31bebe43da0f11a76712e18725c1`. Read GUI locations relative to
>   `crates/gui/farhelm-gui/src/` and desktop locations relative to `crates/farhelm-desktop/src/`.
>
>   - `approval-views-repeat-status-lookups.md`. In `approvals/host_add.rs:95–110` and
>     `approvals/tunnel.rs:173–182,194–202`, share one approval-row lookup and one in-progress operation lookup by
>     request ID in the approvals module.
>   - `gui-command-paths-repeat-reply-classification.md`. In `operations.rs:546–556,633–643`, let the keyless command
>     path project `Outcome::from_reply` into body or refusal words. Keep its command-error handling separate.
>   - `hosts-menu-and-settings-repeat-operation-logic.md`. In `hosts/menu.rs:485–494,513–518,568–574`, have Remove use
>     Delete's existing `close` helper after taking the focus mark. In `hosts/identity/phase.rs:409–417,575–586`, share
>     the suspend/resume infinitive while keeping provider-specific subjects in their lines. In
>     `hosts/progress.rs:151–180`, `hosts/status.rs:425–448` and `hosts/uninstall.rs:427–449`, share the
>     operations-store ID check and conditional update that discards old-session state and notifies only on change. Keep
>     separate globals for the observers. In `hosts/setup.rs:888–922` and `hosts/remove.rs:429–465`, share
>     permanent-answer save admission, clearing on admission and handling a busy preferences key or refused reply. Pass
>     each panel's wording.
>   - `session-list-actions-repeat-admission-and-progress.md`. In `session_list/`, share operation admission across
>     `menu.rs:601–615`, `environment.rs:625–642`, `restart.rs:278–306`, `stop.rs:226–247`, `delete.rs:366–392`,
>     `environment_delete.rs:242–267` and `replace.rs:319–346`. The helper takes the command result and completion,
>     finds the store, starts work under the session key and returns `NotStarted` on refusal. Keep guards, logging and
>     progress clocks local. Share `outcome.rs:432–437`'s in-flight lookup with `stop.rs:285–290`, `restart.rs:314–319`,
>     `replace.rs:414–419`, `delete.rs:435–442` and `environment.rs:323–344`, letting callers filter kinds. Share the
>     status label, colors and spinner for the progress title in `delete.rs:484–516` and `stop.rs:296–327` with
>     caller-supplied selector, label and flex sizing. Share the stale-cause mark's ID, name, role and tooltip in
>     `row.rs:639–660,758–769`, with the clock or text styling supplied by compact and full rows.
>   - `session-pane-repeats-last-known-status.md`. Share the last-known status and activity-age projections between the
>     Details bar and unavailable-session view, keeping their visibility rules local. See
>     `session_pane/details.rs:239–245,260–264` and `session_pane/unavailable.rs:561–569`.
>   - `settings-repeat-preference-and-focus-wiring.md`. Share the sparse `PUT /api/preferences` constructor and
>     diagnostic label in `settings/choices.rs:377–387`, `session_list/order.rs:212–220`,
>     `session_list/host_rail/expansion.rs:71–79` and `terminal/tunnels.rs:605–617`. Let Order pass its object and the
>     callers keep their write and completion policies. Share focus-mark restoration to the editor opener using the
>     stored window when the callback has none in `settings/credentials.rs:362–371` and `settings/recipes.rs:717–727`,
>     keeping editor and refresh bookkeeping local.
>   - `session-list-tests-repeat-frame-and-request-helpers.md`. Paths in this item are under `session_list/`. Share the
>     mouse move, vertical pixel scroll and settle in `list_tests.rs:57–67` and `lifecycle_tests.rs:1707–1717`. Put the
>     debug-bounds drawn predicate from `host_rail/tests.rs:1543–1546` and `menu_tests.rs:128–131` beside the existing
>     test bounds helper. Share frame parent traversal across `host_rail/tests.rs:234–264,1759–1774`,
>     `lifecycle_tests.rs:142–158,1142–1162` and `row_tests.rs:53–75`, keeping frame order, including the collection
>     root and checking every match for reused IDs. Share fresh-frame painted-text collection in
>     `environment_delete_tests.rs:94–97`, `environment_tests.rs:231–234` and `lifecycle_tests.rs:176–179`. Share the
>     row-menu toggle, item click and settle in `environment_tests.rs:224–229`, `lifecycle_tests.rs:160–165` and
>     `menu_tests.rs:146–151`, with the Delete wrapper supplying its item ID. Share a fake-helm drain returning commands
>     acknowledged with empty 200 responses in order. Keep path filters and result shape at the callers in
>     `seen_tests.rs:37–52`, `pin_drag_tests.rs:34–48`, `marking_tests.rs:396–409`, `pins_tests.rs:33–49`,
>     `retired_tests.rs:119–129` and `host_rail/tests.rs:182–188`.
>
>   Read the original findings with `git show 510f3b93721ee2393bd803d2cf91d73654415fbb:review_feedback_queue/<file>`.
>   Plan: `plans/queue/helpers-desktop-startup.md`. Plan: `plans/queue/helpers-terminal-and-controls.md`. Plan:
>   `plans/queue/helpers-hosts-and-sessions.md`. Plan: `plans/queue/helpers-launcher-and-templates.md`.

This plan's share of the entry: the whole `approval-views-repeat-status-lookups.md`,
`gui-command-paths-repeat-reply-classification.md`, `hosts-menu-and-settings-repeat-operation-logic.md`,
`settings-repeat-preference-and-focus-wiring.md`, `session-list-actions-repeat-admission-and-progress.md`,
`session-pane-repeats-last-known-status.md` and `session-list-tests-repeat-frame-and-request-helpers.md` bullets.

The `terminal-grid-row-traversal-duplicated.md` bullet is in none of the four helper plans: it moved to the separate
plan `wide-glyph-padding-cells`.

Every change here is a refactor with no behavior change: the same bytes on screen, the same words, the same commands
sent and the same refusals.

### Where it stands (from planning research at main b34ceee9f)

No consolidation from this entry has landed since the review (`c9575db61`); every file still exists, and the duplicates
are still there at shifted lines, with new callers since. `triage-gui-misc` will have moved some of these lines again by
the time this runs. Re-scan each pattern rather than trusting the line numbers below or the entry's. Paths are under
`crates/gui/farhelm-gui/src/`.

- **Approval lookups.** `approvals/host_add.rs` `waiting` and `answering` (about lines 95-110) and `approvals/tunnel.rs`
  `waiting` and `answering` (about 173-202) each read the pending approval by request id from the current store and the
  in-flight work under `OpKey::approval(id)`. `tunnel.rs`'s `request_subject` (about 165-171) reads the same row inside
  a subject closure.
- **Reply classification.** `operations.rs` `Outcome::from_reply` (about 572-583) and the keyless `answer` (about
  659-670) both classify a reply by status. The refusal words are already shared (`refusal_words`); what repeats is the
  2xx test. `answer` keeps its command-error handling and treats 202 and other 2xx alike.
- **Hosts menu close.** `hosts/menu.rs`: Remove's click (about 513-522) inlines the weak-menu lookup and `layers::close`
  that the module's `close` helper (about 598-602) does for every other item, after taking the click's focus mark.
- **Suspend/resume infinitive.** `hosts/identity/phase.rs`: the provider line (about 422-431) matches
  `(direction, starting)` to `suspend`/`start`/`resume`; the generic line (about 598-610) matches `direction` alone to
  `suspend`/`resume`. One has a "start" arm and the other does not; sharing must keep that difference exactly. Provider
  subjects come from `ProviderUi(provider).lifecycle_asked()`.
- **Session-scoped Hosts state.** `hosts/progress.rs` `Kept`, `seen_now`, `change_current` (about 140-180),
  `hosts/status.rs` `Kept`, `kept`, `change` (about 425-455) and `hosts/uninstall.rs` `Notices`, `notices`, `change`
  (about 424-454) each keep a global under the current operations-store id (`hosts.rs` `current_operations_id`, about
  1046), start over when the id changed, and set the global only when the value changed. Each keeps its own global so
  its own observers redraw.
- **Permanent-answer preference admission.** `hosts/setup.rs` `stop_asking` (about 894-928) and `hosts/remove.rs`
  `stop_asking` (about 429-470) build a preference command, start it under `OpKey::preferences()`, clear the panel line
  on admission, and show a line for a busy key or a refused reply; the two differ in the line's type and wording.
- **Sparse preference command.**
  `Command::new(Work::Save, Method::Put, ApiPath::api(&["preferences"]), body, "PUT /api/preferences")` is in
  `settings/choices.rs` `preference_command` (about 487-495), `session_list/order.rs` `preferences_write` (about
  211-220), `session_list/host_rail/expansion.rs` (about 71-79) and `terminal/tunnels.rs` (about 625-640). Other places
  build a `/api/preferences` path since the review: `session_list/selection.rs` (about 318, a keyless put),
  `templates/shortcuts.rs` (about 202), `sidebar.rs` (about 256), `welcome.rs` (about 185), `feedback.rs` (about 317),
  `whats_new.rs` (about 90) and `terminal/text_size.rs` (about 144). Include a site only when it builds the same
  operation command: `templates/shortcuts.rs` does. A site that sends keylessly through `operations::send` (as
  `sidebar.rs`, `welcome.rs`, `whats_new.rs` and `terminal/text_size.rs` do) or through its own `put`
  (`session_list/selection.rs`) is a different contract and stays out.
- **Settings editor focus restore.** `settings/credentials.rs` (about 405-416) and `settings/recipes.rs` (about 712-723)
  restore focus to the editor's opener with a mark, through the stored window when the callback has none.
- **Session action admission.** Each action finds the current operation store, maps a bad path to `NotStarted::Path`,
  and starts work under the session key: `session_list/menu.rs` (about 730), `environment.rs` (about 652), `restart.rs`
  (about 296), `stop.rs` `start` (about 249-271), `delete.rs` `start` (about 364-392), `environment_delete.rs` (about
  247-268) and `replace.rs` (about 338). New since the review: session groups (`session_list/groups.rs`, about 577,
  `ops.start` under a group key, `OpKey::session_group`). Bulk actions add no copy of their own: their starts go through
  `stop::start` and `delete::start`, and `bulk.rs` `hold` reserves a row rather than starting work, so it stays out.
  Guards, logging and progress clocks stay at each caller.
- **In-flight lookup.** `session_list/outcome.rs` `in_flight` (about 439-444) is the lookup by session key; the same
  read is written out in `restart.rs` (about 331), `replace.rs` (about 430), `stop.rs` (about 308-311), `delete.rs`
  (about 433-451), `environment.rs` (about 298-325) and, new since the review, `rename.rs` (about 154). Callers filter
  kinds.
- **Progress title.** `session_list/delete.rs` `progress` (about 475-507) and `stop.rs` `progress` (about 319-350) draw
  the same status label, ink and spinner in place of the title. `outcome.rs` (about 255-290) draws a wait line with
  seconds, a different shape; leave it unless it shares cleanly.
- **Stale-cause mark.** `session_list/row.rs`: the compact row's `stale_mark` (about 767-787) and the full row's cause
  (about 893-905) build the same `StatusMark` (id with `CAUSE_ID_SUFFIX`, name, `Role::Status`, row help); the compact
  one adds a clock cell, the full one ink.
- **Last-known status in the session pane.** `session_pane/details.rs` (about 255-281) and `session_pane/unavailable.rs`
  `Band::of` (about 587-597) project the row into a status (live, else ended words) and an activity age
  (`age::of(row.info.effective_activity(), now, zone)`); the details bar hides both for a stale row and the age while
  deleting.
- **Session-list test helpers.** Wheel scrolling in `session_list/list_tests.rs` (`scroll_list`, about 58) and
  `lifecycle_tests.rs`; the drawn check in `menu_tests.rs` (`drawn`, about 133) and `host_rail/tests.rs`, to sit beside
  `test_support.rs`'s `bounds` (about 275); parent walks in `host_rail/tests.rs`, `lifecycle_tests.rs` (`drawn_inside`,
  about 143) and `row_tests.rs` (`texts_within`, about 55); painted-text collection in `environment_delete_tests.rs`,
  `environment_tests.rs` and `lifecycle_tests.rs` (`texts`, about 177); the row-menu click in `environment_tests.rs`,
  `lifecycle_tests.rs` (`choose`, about 161) and `menu_tests.rs`; fake-helm drains in `seen_tests.rs`,
  `pin_drag_tests.rs`, `marking_tests.rs`, `pins_tests.rs`, `retired_tests.rs` and `host_rail/tests.rs`. Find each by
  name; most of these files changed heavily since the review.

### Acceptance criteria

1. Each sub-item above has one shared helper (or is skipped per "Decisions already made", with the reason logged and
   reported), placed in an existing shared module where one fits and scoped to its actual callers.
2. The suspend/resume words stay exactly as now: the provider line still says "start" for a starting resume, the generic
   line never does.
3. Each Hosts state module keeps its own global and observers; each preference caller keeps its own write and completion
   policy; each session action keeps its own guards, logging and progress clocks.
4. No per-provider or per-harness branching is added (see "Requirement sources", Code rules).
5. No behavior changes anywhere, and every GUI golden stays byte-identical (see "No visual change").
6. This plan's part of the TODO entry is removed by the rule under "PR discipline".

## Decisions already made

Settled with the maintainer during planning (2026-11-21):

- **Split by area into four plans**, of which this is one; siblings are `helpers-desktop-startup`,
  `helpers-terminal-and-controls` and `helpers-launcher-and-templates`.
- **Dependency:** this plan runs after `triage-gui-misc.md` (in flight at planning time; it touches the Hosts panel,
  approval cards and Settings).
- **Skipping a sub-item:** you may skip a sub-item you judge not worth sharing (for example two copies that differ in a
  way sharing would blur), recording why in the working log as a DECISION and in the report.
- **New copies are in scope by default**, in particular the new operation-admission caller in session groups
  (`session_list/groups.rs`). Re-scan each pattern rather than trusting the old line numbers.
- **No visual change:** byte-identical GUI goldens are the oracle. A golden that changes is a red flag, not something to
  accept (see "No visual change").
- **Commit types:** every PR is `refactor:` (or `test:` for a test-only one) and behavior-free, so none needs a
  changelog fragment.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- **Code rules (a fence).** Root `AGENTS.md`, "Code rules", governs the suspend/resume infinitive in
  `hosts/identity/phase.rs`: "Every disposable-environment provider decision belongs in its implementation or an
  exhaustive method on the provider kind, place, binding, recipe base, recipe state, host kind or shared recipe
  projection. [...] Shared code must not compare individual providers, use `matches!` over providers or add a wildcard
  provider arm." The shared helper matches on the lifecycle direction (and the starting flag) only, the provider subject
  keeps coming from `ProviderUi`, and nothing here adds provider or harness branching.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Tests and validation", and
  its "Instructions by path" for every path you touch (`.agents/validation.md` for any test).
- Root `AGENTS.md`, "Code rules": "UI wording, marks and help belong in one exhaustive UI module", which for provider
  words is `ProviderUi` in `crates/gui/farhelm-gui/src/environment_ui.rs`.
- `.agents/test-authoring.md` for every test added or changed, and `python -B scripts/check-test-sleeps.py` for the
  test-only PR.
- `.agents/validation.md`: the GUI crate's tests and the GPUI goldens entry.

**Planner proposals:**

- Approval lookups as two small functions in the approvals module taking a request id.
- The 2xx classification shared by having `answer` project `Outcome::from_reply`, as the entry says; if that forces the
  keyless path to carry markers or a status it does not want, skip it and log why.
- A generic session-scoped holder for the Hosts state (the store id plus the value, a read and a change-if-different),
  each module instantiating it with its own global type.
- One admission helper in `session_list/` taking the operation key, the command result (or a builder) and the
  completion, returning `NotStarted`, which session groups (a group key) also use.
- The progress title as one builder taking the selector, label and flex sizing; the stale-cause mark as one base
  `StatusMark` builder in `row.rs` that each row finishes.

## Outline

1. `plan/helpers-hosts-and-sessions/01-approvals-and-replies`: the approval lookups and the reply classification.
   `refactor:`, no fragment.
2. `plan/helpers-hosts-and-sessions/02-hosts-panel`: the Hosts menu close, the suspend/resume infinitive, the
   session-scoped state and the permanent-answer admission. Split into two PRs if it grows past one reviewable change.
   `refactor:`, no fragment.
3. `plan/helpers-hosts-and-sessions/03-preferences-and-editor-focus`: the sparse preference command and the Settings
   editors' focus restore. `refactor:`, no fragment.
4. `plan/helpers-hosts-and-sessions/04-session-admission`: the session action admission and the in-flight lookup.
   `refactor:`, no fragment.
5. `plan/helpers-hosts-and-sessions/05-row-progress-and-cause`: the progress title and the stale-cause mark.
   `refactor:`, no fragment.
6. `plan/helpers-hosts-and-sessions/06-pane-status`: the session pane's last-known status and age projections.
   `refactor:`, no fragment.
7. `plan/helpers-hosts-and-sessions/07-session-list-test-helpers`: the shared session-list test helpers; this plan's
   part of the TODO entry removed. `test:`, no fragment.

Small neighbouring sub-items may share a PR, and a large one may split, as long as each PR stays one reviewable
refactor; log the shape you chose.

## Fallbacks

- A sub-item whose copies turn out to differ in a way sharing would blur: skip it, log a DECISION, and say so in the
  report. Likely candidates: the permanent-answer admission (different line types), a preference site that sends
  keylessly rather than as an operation, the groups admission if a shared helper would need a special case.
- If a sub-item has already been consolidated on main by other work (`triage-gui-misc` included) when you reach it,
  record that and move on.
- **Concurrency.** The siblings `helpers-desktop-startup`, `helpers-terminal-and-controls` and
  `helpers-launcher-and-templates` touch disjoint files except: `helpers-terminal-and-controls` also edits
  `test_support.rs` (shared key-event helpers) and `components/spinner.rs` (the tick counter), and
  `helpers-launcher-and-templates` edits `templates/` files, of which this plan touches only `templates/shortcuts.rs` if
  that site joins the preference command; `helpers-desktop-startup` may edit `session_pane/details.rs` (its
  `placeholder` markers, beside this plan's pane status projections). Those rebases are mechanical: keep both sides.
- **Bulk-action plans.** Two plans planned at the same time may land before or alongside this one; neither is a
  dependency, and picking keeps conflicting plans apart. `bulk-actions-bound-to-drawn` edits `session_list/bulk.rs`
  (bulk lifecycle and target capture), `stop.rs` (`guard`), `menu.rs` (`RowMenu::action_frame`), `marking.rs` and uses
  `test_support.rs`; this plan touches different functions in those files (`stop.rs` start, in-flight and progress;
  `menu.rs` admission; `test_support.rs` beside `bounds`), so rebase across it carefully (`.agents/careful-rebase.md`).
  `pace-bulk-metadata-changes` changes only the helm's command-queue limit and its helm tests
  (`crates/farhelm-helm/src/ui_channel.rs` and `ui_channel_tests.rs`), so it shares no file with this plan.
- **Fence: no visual change.** See "No visual change" under "How to run".

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-helpers-hosts-and-sessions-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-helpers-hosts-and-sessions-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes: the touched modules' GUI tests (approvals,
operations, hosts, settings, session list, session pane), and the goldens for every PR that touches drawing code. Apply
`.agents/test-authoring.md` to every test you add or change, and run `python -B scripts/check-test-sleeps.py` as
`docs/test-sleep-check.md` prepares it when Rust tests change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

### No visual change

These PRs must not change what the app draws, so there is no visual review. Byte-identical GUI goldens are the oracle:
run the goldens (`python3 scripts/check-gui-goldens.py` through the recorder, `.agents/validation.md`) for every PR that
touches drawing code, in compare mode, never `--update`. Any golden that changes must be investigated and the change
reverted, unless it is a pre-existing nondeterminism, which you show by reproducing it on the PR's base and log (with
the scene and the evidence) in the working log and the report. Never publish new golden pictures from this plan.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/helpers-hosts-and-sessions/<nn>-<short-name>` as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- **The shared TODO entry.** Four sibling plans (`helpers-desktop-startup`, `helpers-terminal-and-controls`,
  `helpers-hosts-and-sessions`, `helpers-launcher-and-templates`) split the TODO entry "Consolidate the helpers
  duplicated across Farhelm.", and which of them lands last cannot be known. So each plan's last PR deletes only that
  plan's own part of the entry: its group bullets, and for a bullet split between plans only its own sentences (the
  `templates-repeat-lookup-and-control-wiring.md` bullet: the last two sentences, from "Share physical keypad digit
  lookup" to the end, are `helpers-terminal-and-controls`'s, the rest `helpers-launcher-and-templates`'s; the
  `gui-screenshot-scenes-repeat-fixture-wiring.md` bullet: the reinstall hover formatter sentence is
  `helpers-desktop-startup`'s, the rest `helpers-launcher-and-templates`'s). The text of a sub-item the plan skipped, or
  found already consolidated, is deleted with the rest of its share; the report carries the skip and its reason. The
  same PR deletes the plan's own `Plan:` reference from the entry's closing line. The plan that removes a split bullet's
  last part deletes the bullet. If that deletion leaves the entry with no group bullets at all, the same PR also deletes
  the entry's heading paragraph and its closing "Read the original findings" line with the plan references. While any
  bullet remains, the heading stays; that includes the `terminal-grid-row-traversal-duplicated.md` bullet, which the
  separate plan `wide-glyph-padding-cells` removes under this same rule. Apply this rule to `TODO.md` as it is on
  `main@origin` when you rebase for delivery; on a rebase conflict in this entry, keep every deletion from both sides
  and apply the rule again.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`). Every PR in this plan is `refactor:` (or `test:` if test-only) with no user-visible effect, so none adds
  a changelog fragment (root `AGENTS.md`, "Changelog fragments"). This plan's `## Release note` section is the
  maintainer's answer for the release's highlights, not an instruction to you (`plans/.agents/executing.md`, "Executing
  one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). Tell it that every PR here must be behavior-free, so any behavior difference is a finding, and quote it
the Code rules fence above. When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
alternatives considered: in particular those named under Fallbacks above, every sub-item you skip, and every review
finding you decided not to follow. The maintainer will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, with
every GUI golden unchanged, and the last removing this plan's part of the TODO.md entry by the rule under "PR
discipline". Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
