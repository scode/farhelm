# Execute twelve triage outcomes: UI refresh, takeover, layout and hidden-character fixes

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. It may touch the same code as `helm-supervisor-small-fixes.md`, which adds
a total catalog size cap for templates (possibly in `check_template_shape` in `crates/farhelm-proto/src/launcher.rs` or
in the helm's template save path); this plan adds a session-name check to `check_template_shape`. The two edits are
independent; if both are in flight, expect a mechanical rebase, not a design conflict.

## The goal

Carry out twelve triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, all `fix code`, in one draft PR. Their headings,
which are also the feedback file names under `review_feedback_queue/`:

- `header-actions-skip-listing-read.md`
- `build-mismatch-turns-unattended-mounts-into-displacing.md`
- `seen-state-queue-acknowledge-newer-choice-sending-it.md`
- `rename-editor-conceals-title-characters-editable-title-editable-title-seed.md`
- `template-supplied-session-names-rendered-raw.md`
- `host-actions-disappear-supported-sidebar-widths.md`
- `ended-session-status-pushes-header-actions-beyond-promised.md`
- `delayed-provisioning-confirmations-make-two-dialogs-mutually.md`
- `session-refresh-failures-silently-retain-apparently-current.md`
- `partial-templates-cannot-retain-non-yolo-approval-choices.md`
- `save-followed-immediately-delete-restores-outdated-local.md`
- `rejected-permission-toggles-depend-observing-intermediate-busy.md`

All twelve are in the web and desktop UI (`crates/farhelm-ui/`), plus one save-time template check in
`crates/farhelm-proto/`. In product terms:

- After Replace or Restart from the session header, while the "reload for the new version" notice is up, the sidebar
  keeps the deleted session (whose actions fail) and never shows the replacement, or keeps a restarted session's old
  ended status.
- In a window running an outdated page after the helm was updated, a newly discovered terminal tab can attach on the
  route that takes the session away from whoever is using it in another window, without anyone asking for a takeover.
- With two windows open, marking a session read can report success while the session stays unread, when the other window
  marked it unread while the first request was still out.
- The Rename dialog shows an agent-set title raw, so invisible or direction-changing characters stay hidden while the
  user edits it.
- A template's session name can carry such characters into the launcher's name field, which then looks different from
  what is submitted.
- At the narrowest supported sidebar width, an unreachable host's long status can push its ⋯ menu (Retry, Settings,
  Remove) off-screen.
- An ended or failed session's longer status badge can push Delete off the session header between 650 and roughly 850
  px, where SPEC.md promises every action.
- Opening host Settings while an uninstall plan is being prepared can leave both dialogs unresponsive.
- A session view whose background refresh keeps failing shows old status and Resume availability with no hint.
- A template with no agent type offers only the YOLO approval choice, so OMP and Goose users cannot save an
  approvals-only template.
- Deleting a template right after saving it, then choosing Undo, could restore the version from before the save (the
  trigger looks unreachable today).
- A refused change to a host's permission switches puts the switches back only if the dialog happens to observe its busy
  state go on and then off.

None of these was reproduced at runtime; all were confirmed or judged likely by reading the code. Each ledger entry
(Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read
the twelve entries and feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger
entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Every outcome dropped by its gate is handled the way its gate says (Complexity gate below): closed as discarded per
  the recorded decision, documented instead, or set back to `pending` waiting on the user. The recorded Outcome field is
  never rewritten.
- Each outcome's Execution field reads "planned in `plans/queue/ui-layout-and-escaping-fixes.md`" when this plan starts
  (the planning PR set it); that is the expected starting state, not a discrepancy.
- The PR removes each carried-out or discarded outcome's feedback file and index line, updates every outcome's
  `TRIAGE_OUTCOMES.md` Execution field, carries a changelog fragment, and passed the review gate.
- The checks under Validation passed, with recorded browser runs on Chromium and WebKit for the changed behavior.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11):** recorded per outcome in root `TRIAGE_OUTCOMES.md` (landed in #1919, plus
the earlier `rejected-permission-toggles-depend-observing-intermediate-busy.md` entry). They differ in what happens when
a fix turns out not to be simple, so read each Decision field. In short:

- `header-actions-skip-listing-read.md`, `build-mismatch-turns-unattended-mounts-into-displacing.md`,
  `seen-state-queue-acknowledge-newer-choice-sending-it.md`: the user asked for findings "whose outcome is
  straightforward and needs no significant complexity or redesign to be triaged without discussion"; each entry says to
  stop and bring it back to the user if the fix grows.
- `rename-editor-conceals-title-characters-editable-title-editable-title-seed.md` and
  `template-supplied-session-names-rendered-raw.md`: the user chose "Fix both"; bring back to the user if not contained.
- `host-actions-disappear-supported-sidebar-widths.md`, `ended-session-status-pushes-header-actions-beyond-promised.md`,
  `delayed-provisioning-confirmations-make-two-dialogs-mutually.md`,
  `session-refresh-failures-silently-retain-apparently-current.md`: "fix all simple ones with complexity gate, discard
  rest", so a fix that is not simple is recorded as `discard` instead.
- `save-followed-immediately-delete-restores-outdated-local.md`: "fix SUPER cheap ones with complexity gate", so only a
  one- or two-line guard; otherwise `discard`.
- `partial-templates-cannot-retain-non-yolo-approval-choices.md`: "fix simple ones with complexity gate, if there is a
  known issue likely to matter to a typical user ensure the website's section on that specific agent calls out the
  caveat", so if the code fix is not simple, document the caveat on the website's OMP and Goose pages instead.
- `rejected-permission-toggles-depend-observing-intermediate-busy.md`: "fix code with hard cmplexity gate"; bring back
  to the user unfixed if it needs more than a small, contained change.

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries". Asked how to group them, the user chose "8 plans as proposed"; this plan is the UI group. Review gate:
"gpt-6.1-sol high", a single fresh-context agent with the general charter, no swarm. PR shape: "One PR per plan", so
this plan's outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these outcomes only.
No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms in the outline below, verified against main at d3b28820. Handling a gate trip
by dropping that one outcome from the PR and shipping the rest follows the precedent of earlier grouped triage plans.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Reproducing failures: narrow tests first; Sharing the machine with other
agents; Agent scratch space; Harness-specific code, for the approval-choice change; the rule against tests that modify
the test process's own environment variables), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md`
(Lifecycle), `.agents/test-authoring.md` for test changes, `.agents/narrow-tests.md` for focused runs,
`docs/test-run-evidence.md` for the recorder, `docs/test-sleep-check.md` for any delay a test adds, and, for any change
under `website/`, `website/AGENTS.md` and `website/EDITORIAL_RULES.md` (read in full first, as that file demands).

## Outline

Line numbers drift; find the code by name. Verified on main at d3b28820.

- Header refresh (`header-actions-skip-listing-read.md`): in `crates/farhelm-ui/src/session_view.rs`, the header's
  Replace success arm (`Ok((new_session, notice))` inside the `replace` closure) publishes the delete notice and calls
  `on_replaced`, and the restart closure's success path calls only `refresh_after_restart(Trigger::Explicit)` (the
  view's own detail). The sidebar's own actions call `refresh(Trigger::Explicit)` in
  `crates/farhelm-ui/src/list/view.rs`. The existing one-shot header-to-list bridges (`HeaderDeleteRequest`,
  `HeaderPrefillRequest`: a signal AppBody owns in `crates/farhelm-ui/src/lib.rs` and ListView consumes once) are the
  pattern: add the smallest equivalent (a counter or one-shot request signal) that header Replace and Restart bump on
  success and ListView answers with `refresh(Trigger::Explicit)`. Explicit demand already bypasses the build-skew
  withdrawal (`reader.rs`). Gate (bring back): anything beyond such a bridge.
- Unattended mounts under a build mismatch (`build-mismatch-turns-unattended-mounts-into-displacing.md`):
  `crates/farhelm-ui/assets/terminal.js`, `unattendedMount(opening)` returns
  `!opening && !reclaiming && capabilityOf(syncedPolicy) === "auto"`, and `sync()` passes that to `mountWhenReady`; the
  attach then picks `spec.pathUnowned` only when that value is true, so under `manual-only` an unattended mount falls to
  the displacing `spec.path`. Separate "this mount is unattended" from "unattended attaching is allowed". Under
  `manual-only`, an unattended mount must not take the displacing route; since the capability is withheld precisely
  because the helm may not serve the refusing route, the safe shape is to not attach it and leave it in the existing
  latched "Detached … take control" state the user resolves explicitly (decide and log; the ledger accepts either
  mounting non-displacing or waiting for the user). Opening, `reclaim()` and manual reconnect keep their intent paths.
  Add a regression test in `e2e/tests/terminal-tabs.spec.ts` (or the JS unit harness under `crates/farhelm-ui/js-tests/`
  if the decision function can be tested there without new machinery) if one fits. Gate (bring back): more than a
  contained change to the mount decision. Another plan, `browser-oracle-and-fixture-fixes.md`, also edits
  `e2e/tests/terminal-tabs.spec.ts`, and its pending-mount test relies on `mountWhenReady`'s `tryMount` retry loop in
  `terminal.js` checking `window.Terminal` (and the tab's element) before it mounts. Keep this fix to the
  `unattendedMount` decision and the attach route; if it does change `mountWhenReady` or `tryMount`, say so in the
  plan's report ("things you should know") so that plan's executor and the landing monitor can see the interaction.
- Seen-state queue (`seen-state-queue-acknowledge-newer-choice-sending-it.md`): `crates/farhelm-ui/src/api.rs`,
  `SeenWrites::finished` returns `Final` when `slot.latest == sent`, so a choice recorded during the PUT that equals the
  in-flight value is never sent. `PreferenceWrites` beside it keeps `recorded_since_send` for exactly this. Add the same
  flag to `SeenWriteSlot`, set it in `record` while `in_flight`, clear it in `next_to_send`, and treat it as
  `Superseded` in `finished`; keep the `Final { waiting }` hand-back for the case where nothing was recorded after the
  send. Add a unit test for the equal-valued re-record. Gate (bring back): anything beyond this queue.
- Rename seed (`rename-editor-conceals-title-characters-editable-title-editable-title-seed.md`): the Rename dialog's
  draft is seeded in `crates/farhelm-ui/src/list/view.rs` (`on_rename_start`, `rename_draft.set(title)`) and edited in
  `crates/farhelm-ui/src/rename.rs` (`RenameForm`, which submits the draft verbatim). Reuse the create form's escaped
  seed pattern (`reseed_cloned_field` and `submitted_field` in `crates/farhelm-ui/src/list/create_form.rs`, with
  `peer::display_peer`): show the escaped title, keep the raw title as the seed, track whether the user edited the
  field, and submit the raw seed when untouched. A title with no unsafe characters must behave exactly as today. Keep
  `RenameForm`'s verbatim-submit contract for what the user typed and update its docs to say how the seed works. Gate
  (bring back): not a contained change.
- Template session name (`template-supplied-session-names-rendered-raw.md`): `check_template_shape` in
  `crates/farhelm-proto/src/launcher.rs` checks the launch and resume commands with
  `crate::text::is_presentation_unsafe` and the template's own name only for control characters; the session-name field
  is `TemplateFields::name`. Extend the presentation-unsafe check to `fields.name` with the same kind of message. It is
  called on every save path (UI, helm `templates.rs`, agent requests), so one change covers them; add a unit test beside
  the existing command cases. A template saved before this rule still launches and only further saves are refused, as
  for commands. Update SPEC.md's "Confirmed 2026-10-10" template-command paragraph (around "a template's launch and
  resume commands cannot contain control characters") to name the session name too, and the matching sentence in
  `website/src/content/docs/docs/using/launch-templates.mdx` ("A template whose command contains hidden or control
  characters still launches…"). The unsafe set includes the zero-width joiner, so a template session name with a joined
  emoji (a family or flag sequence, for example) is refused too; that follows the user's decision to extend 7e, but it
  is user-visible, so say it plainly in the changelog fragment and in the plan's report ("things you should know"), not
  only in the log. Gate (bring back): not contained.
- Sidebar ⋯ menu (`host-actions-disappear-supported-sidebar-widths.md`): `crates/farhelm-ui/assets/app.css`,
  `.host-status` is `flex: 0 0 auto; white-space: nowrap`, while `.host-status.updating` already shows the shrink
  discipline (`flex: 0 1 auto; min-width: 0`, the step text giving up width). Let the phase statuses shrink the same way
  (the words ellipsize, the dot stays; full wording stays available, e.g. in the existing tooltip mechanism) so the ⋯
  toggle stays reachable at the 240 px minimum sidebar width SPEC.md supports with the longest phase wording. Extend a
  browser test in `e2e/tests/sidebar-resize.spec.ts` or `e2e/tests/provisioning.spec.ts` that sets the minimum width and
  asserts the toggle is inside the sidebar for an unreachable host, if it fits the existing fixtures. Gate (discard if
  not simple).
- Ended-session header (`ended-session-status-pushes-header-actions-beyond-promised.md`): `e2e/tests/header.spec.ts`
  measures only a live session (`SUPPORTED_MAIN_PANE_WIDTH = 650`). Let the session header's status badge shrink with an
  ellipsis (full text in its tooltip) so all actions fit from 650 px for ended and failed sessions too, and add an
  ended-session case to the header width test. Find the header badge's CSS in `app.css` by its class in
  `session_view.rs`. Gate (discard if not simple).
- Dialog lockout (`delayed-provisioning-confirmations-make-two-dialogs-mutually.md`): uninstall planning in
  `crates/farhelm-ui/src/provisioning.rs` sets `planning` without claiming the page operation token, and
  `crates/farhelm-ui/src/hosts.rs` permits Settings when the row is not busy; the two dialogs each install their own
  isolation (`crates/farhelm-ui/src/modal_isolation.rs`, `hosts/settings_dialog.rs`). The smallest fix is to refuse
  (disable) Settings while that host's provisioning is planning, the way `setup_disabled` already folds in
  `provisioning_menu.planning`; closing Settings when the confirmation arrives is the alternative. Decide and log. Gate
  (discard if not simple).
- Failing detail refresh (`session-refresh-failures-silently-retain-apparently-current.md`): in `session_view.rs`,
  `read_detail` returns `false` on a failed `fetch_session` and the surface reader retries silently. Record that the
  latest detail read failed (cleared on the next answered read) and show a short "could not refresh this session"
  notice, reusing the view's existing notice band (where `stale_notice` renders) and the hosts list's `refresh_error`
  wording as the model. The ledger says "persistently failing", so show the notice only after two consecutive failed
  detail reads (a single transient failure stays silent), and clear it on the next answered read. Gate (discard if not
  simple).
- Approvals-only templates (`partial-templates-cannot-retain-non-yolo-approval-choices.md`):
  `crates/farhelm-ui/src/list/templates.rs`, `permission_options(None)` filters to YOLO only. Offer every
  `LaunchPermission` when no agent type is set; launch already re-checks the choice against the agent type
  (`crates/farhelm-proto/src/launcher.rs`, the `offers_permission` refusal when applying templates). Check that the
  helm's template save path does not refuse a non-YOLO permission without an agent type; if it does, that is the gate.
  Follow root `AGENTS.md` "Harness-specific code" (no per-harness branches in shared code). Gate: if the fix is not
  simple, leave the code and add the caveat (an approvals-only template needs the agent type set) to
  `website/src/content/docs/docs/agents/omp.md` and `website/src/content/docs/docs/agents/goose.md`, per the website
  rules; record that as the outcome's execution.
- Save-then-Delete guard (`save-followed-immediately-delete-restores-outdated-local.md`): `templates.rs`, the `delete`
  closure checks `busy()` and that the template is in `templates.peek()`, but not whether the `templates` resource is
  still reloading after Save's `templates.restart()`. Add a one- or two-line guard that refuses while the list is
  reloading (for example the resource's pending state). Gate: anything more closes the outcome as discarded per the
  recorded decision (Complexity gate below).
- Permission switches (`rejected-permission-toggles-depend-observing-intermediate-busy.md`):
  `crates/farhelm-ui/src/hosts/settings_dialog.rs` puts the switches back only when its effect sees busy go from true to
  false. Reset both switches from the failure outcome itself, using the existing `reset_checkboxes()` that already
  handles a write that never started. Add a test if one fits without new machinery. Hard gate (bring back unfixed): new
  scheduling machinery, a refactor of the dialog or of the host mutation wrapper, or a test seam that cannot be added
  simply.
- Changelog and type: several outcomes change what users see (sidebar refresh, takeover, layout, notices, hidden
  characters, approvals-only templates), so the PR type is `fix:` and it adds one fragment under
  `releasing/changelog.d/` per root `AGENTS.md` (Releases and the changelog), written for someone running Farhelm per
  `releasing/EDITORIAL_GUIDANCE.md`, validated with `python3 releasing/check-changelog.py format`. If the template
  session-name check ships, the fragment says that saving a template whose session name contains hidden characters,
  including joined emoji, is now refused.

The complexity gate for every outcome is a change confined to the files the bullet names, plus tests, docs and
bookkeeping. A new subsystem, cross-component plumbing beyond the named bridge, or a design decision the ledger did not
anticipate is a gate trip.

### Validation

Follow root `AGENTS.md` "Finishing work"; no full battery.

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop` (the UI crate's desktop build compiles nowhere else).
- Focused Rust unit tests for the changed modules (the seen-state queue, the template shape check, the rename seed if it
  gains a helper test), through `scripts/record-test-run.py` with `--runner nextest`, `--kind development`,
  `--tmux none`, and a selection naming exactly those tests.
- `cd crates/farhelm-ui/js-tests && node --test` if `terminal.js` or another asset script changes.
- Browser runs through the recorder on Chromium and WebKit, after `cargo build` and the dx web build root `AGENTS.md`
  names, narrowed with Playwright's `-g` (and the spec file) to the tests you added or changed plus the existing tests
  that exercise the code you touched: for example the header width test in `header.spec.ts`, the host-row width test you
  extend, the terminal-tab takeover and mount tests in `terminal-tabs.spec.ts`, the template editor's permission and
  delete tests in `templates.spec.ts`, the uninstall and Settings tests in `provisioning.spec.ts` and
  `settings.spec.ts`, and the session-view refresh and header Replace or Restart tests you add. Not whole spec files and
  not the full suite; widen only for a concrete risk the narrow selection leaves uncovered, and record the selection.
- `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md` if any browser
  or Rust test changes; zero unannotated delays. Apply `.agents/test-authoring.md` to every changed test.
- If anything under `website/` changes: `dprint check` on it and
  `cd website && bun install --frozen-lockfile && bun run build`. Start no preview server (unattended run).
- `dprint check` on the changed Markdown (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`, SPEC.md, the changelog
  fragment).

A failure under another agent's load is an environment event (root `AGENTS.md`, Sharing the machine): keep the retained
run, re-run narrowly, and do not log a flake from one observation.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-ui-layout-and-escaping-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/ui-layout-and-escaping-fixes/01-ui-fixes`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`fix:` here, with
  the changelog fragment described in the outline).
- For every outcome the PR carries out or discards by its gate, it removes the feedback file under
  `review_feedback_queue/` and its line in `review_feedback_queue/INDEX.md`, and updates that outcome's
  `TRIAGE_OUTCOMES.md` Execution field (it starts as "planned in `plans/queue/ui-layout-and-escaping-fixes.md`") to
  `complete`, or to the gate-trip wording under Complexity gate, with the jj change ID, bookmark and PR URL. The
  recorded Outcome field is never rewritten. Record the change ID and bookmark before creating the PR, then add the URL
  to the same change and push again; no separate bookkeeping PR. An outcome sent back to the user is handled as
  Complexity gate says.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome carries the user's complexity gate, stated in its ledger entry and summarized under Requirement sources.
When a fix turns out to need significantly more than the outline describes (a new mechanism, cross-component plumbing, a
change well outside the files the outline names, or a product or design decision), do not grow it and do not block the
whole plan for it. What happens next depends on the outcome:

- "discard if not simple" outcomes (the four layout and freshness outcomes, and the Save-then-Delete guard): leave the
  code, never rewrite the recorded Outcome field, and set Execution to "`complete`: gate tripped (<reason>); discarded
  per the recorded decision" followed by the jj change ID, bookmark and PR URL. Remove its feedback file and index line
  in the PR.
- The approvals-only templates outcome: leave the code and add the caveat (an approvals-only template needs the agent
  type set) to the website's OMP and Goose agent pages instead, per the website rules; record that in its Execution
  field with the change ID, bookmark and PR URL, and remove its queue item.
- Every other outcome: leave it out of the PR, keep its feedback file and index line, and in the same PR set its
  `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you found and the realistic
  options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape). Name it among the open
  questions in the plan's report.

The rest of the PR ships. Block per Unattended fallback only when every outcome in this plan was dropped back to the
user, since then there is no PR worth delivering. Code that turned out to be already fixed on main is not a gate trip:
set its Execution to "`complete`: already fixed by <commit>" with the change ID, bookmark and PR URL, leave the recorded
Outcome as it is, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. If the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Ask the reviewer to check in particular that: an untouched Rename and an untouched launcher name
still submit the original bytes; no unattended mount can take the displacing route under any capability; the seen-state
queue still hands waiting reports their outcome; the layout changes hold at 240 px sidebar and 650 px main pane on both
engines; and nothing adds a per-harness branch in shared code. Where you disagree with a finding, decide on the merits
and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new context or registry
beyond the one-shot bridge, a rework of the reader or modal isolation, edits outside the files the outline names beyond
tests, docs and bookkeeping), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the ledger entries, the user's decisions above,
this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the shape of the header-to-list refresh bridge, whether a forbidden unattended
mount waits latched or mounts non-displacing, the seen-state flag's placement, how the Rename seed tracks edits, the
wording of the new template refusal and its SPEC.md and docs sentences, how the host and header statuses shrink and
where their full text goes, Settings refused versus closed during planning, the failing-refresh notice's threshold and
wording, the approvals-only template fix or its docs caveat, the Conventional Commit type and changelog wording, every
outcome dropped by its complexity gate and why, and every review finding you decided not to follow. The user will ask
for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is its complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate (and handles every one that did as its gate says), meets the acceptance criteria above, and passed the
review gate, with every outcome's `TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out
and discarded outcomes removed. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
