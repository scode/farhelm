# Hosts: plans as steps, progress on the same rail, refusals in dialogs, a one-line uninstall notice

Written against main at d526038e on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Rework the hosts panel's setup, update and uninstall dialogs and what surrounds them. The helm sends the plan it is
asking the user to authorize as a structured list of steps, one for each step the run will carry out, under user-facing
names the helm owns, and the dialogs draw it as a numbered stepper. A running setup, update or uninstall shows its
progress on the same stepper, with the same steps under the same names, in the host's row. Refusals of host actions open
in dialogs instead of red text in the sidebar. The large post-uninstall notice becomes a single line.

The maintainer merged three TODO.md entries (Near term) into this one plan because they rework the same dialogs:

- "**Show host uninstall refusals in a dialog.** When Uninstall is refused because the host was set up manually or
  Farhelm cannot safely remove it, the reason currently appears as red text in the host sidebar. Show the refusal in a
  proper dialog, consistent with other host actions. Audit similar refusal and error paths for legacy sidebar text and
  bring any outliers into the current UI convention."
- "**Render host setup and uninstall plans as GUI steps.** The setup and uninstall confirmation dialogs currently show
  the planned actions in a plain text box. Present the actions as distinct items in the dialog so the user can scan what
  will happen before deciding, while keeping the paths and other details the plans currently show."
- "**Improve the post-uninstall host notice.** After uninstall, a large plain-text notice sits above the host rows in
  the sidebar. It says Farhelm was removed, names the host and the path where its data remains, and has a separate
  Dismiss button. The text wraps across several lines and dominates the narrow panel. Find a better way to present this
  result; the design details are TBD."

### Acceptance criteria

Plan steps:

- One step list per plan, used by both the confirmation and the run. Each of the plan's frozen actions
  (`ProvisioningAction` in `crates/farhelm-helm/src/provisioning/plan.rs`) is exactly one step. Each step carries:
  - its internal id (`ProvisioningAction::label()`, unchanged);
  - its user-facing name, from the table under decision (B);
  - the path or paths it touches, each in full and, when it lies under the host's home directory, as `~/…`;
  - an optional explanation.

  Steps that had no confirmation line before (`upload-farhelm`, `upload-tmux`, `daemon-reload`, `attach-supervisor`) now
  appear in the confirmation too. Conditional steps (tmux, restart) appear only in plans that contain them. The
  directories step keeps all its directories: three for setup, four for an update whose registered binary lives outside
  the lib directory (`plan.rs`, around line 483).
- Every helm reply that carries a setup, update or uninstall plan carries that step list beside a lead line (today's
  heading, "Farhelm will set up <host> (<distro>, <arch>):") and, for uninstall, the kept-data note as a separate
  callout. Those replies are the probe's `provisionable` reply and the update and uninstall plan replies
  (`ProbeResponse::Provisionable` and `HostPlanResponse` in `crates/farhelm-helm/src/provisioning/http.rs`).
- Every progress view (`ProvisioningView` in the same file) carries the same step entries, id included, beside each
  step's status and message.
- The helm writes all of that wording. The UI keeps no copy of it and never builds step text from `plan.actions`.
- The word "place" is gone everywhere, including from the plain-text `confirmation`. The helm renders that text from the
  same step list: the lead, each step under its name with its paths and explanation, then the kept note. The exact
  plan-text tests change deliberately to the new text:
  - setup's existing exact test in `crates/farhelm-helm/src/provisioning.rs` (around line 1340) is rewritten;
  - uninstall gets a new exact test (the one around line 3674 only checks fragments);
  - an update plan gets an exact test too.
- The new wording keeps every promise SPEC.md "Topology" requires of the confirmation:
  - it names each file and directory setup writes;
  - it names the systemd user unit it writes;
  - it says the supervisor runs persistently;
  - it says the supervisor starts at boot, or at login when lingering is refused;
  - for uninstall, it names each path on the host, except the private tmux server, which is named without its socket
    path.

  It also keeps SPEC_impl.md's promise ("Provisioning", around line 4267) that an existing shared directory keeps its
  permissions.
- The setup dialog (add host, and a remote row's setup rerun) and the uninstall dialog draw those steps as design "B.
  Numbered stepper", described under "The user's decisions" below. The inline update confirmation (`PlanConfirmation`)
  reuses the same stepper component, but it is a defensive path that production never reaches: a remote Update submits
  without showing a plan, and the helm refuses every local Update plan. It gets no dedicated tests, and the report says
  it is a defensive path.
- Path chips show the `~/…` form, with the full path as their hover text. Peer-supplied text (paths, host names) crosses
  the same escaping and direction-isolation boundary it crosses today (`PeerLine`/`display_peer`, `dir: "ltr"`).
- The plan can still be copied as the helm's exact text (see the planner proposal on copying).
- The dialogs keep their current focus, Escape, scrolling, phone-width and short-viewport keyboard behavior (SPEC.md
  "Lifecycle operations", the paragraphs on confirmation focus; the existing `host-setup-layout` tests in
  `e2e/tests/provisioning.spec.ts`).

Progress:

- A host's step list in its expanded row, and the update status's hover popup, draw the same rail at a smaller size,
  under the helm's step names:
  - completed steps green with a check;
  - the running step pulsing amber (static under reduced motion, as `.host-update-dot` already is);
  - pending steps grey;
  - a step that had nothing to do (`skipped`) as done in grey, labelled "unchanged";
  - `enable-linger`'s `degraded` outcome as an amber warning carrying the helm's message (the supervisor starts at login
    instead), not as a failure;
  - a failed step red, with the helm's reason.

  Each step's status stays available as text. The inline status's current step and the popup use the names too, and the
  counts are the plan's step count, since the plan and the run are the same list.
- Confirming setup or uninstall closes the dialog, as today. The host's row then opens by itself for that run, without
  turning on the global details checkbox. The user can fold it, and it stays folded for that run. Folded, the row shows
  the inline status an update shows today (pulsing dot, done/total count, current step, elapsed time, hover popup), with
  "setting up" or "uninstalling" as its verb.
- A host update's progress uses the same rail; its folding behavior is unchanged (SPEC.md "Topology": folded while
  running, opened on failure).
- A run that fails leaves the row open, so its step list and diagnostic stay visible; a run that succeeds folds it.
- Between the click on setup or uninstall and its dialog (planning runs over ssh and can take seconds), the folded row
  shows that planning is under way, through the inline pending status Update already shows
  (`HostUpdateProgress::Pending(Planning)`), now with the setup and uninstall verbs. This replaces the "planning…" line
  that only showed because the click turned on the global details checkbox.

Refusals (the audit table the maintainer accepted):

- An uninstall refused at planning time opens in the uninstall dialog in place of the plan, with Close. When the helm's
  refusal points to Remove ("remove it from the list instead"), the dialog also offers Remove, which goes through
  Remove's own confirmation exactly as the menu's Remove does.
- A setup rerun refused at planning, and an Update refused at planning, open a dialog, as for uninstall. Update
  submission refusals (one-click `Busy` or `UnknownPlan` races, often from "update all") are not in the accepted table
  and stay where they are.
- The submission POST of a confirmed setup, setup rerun or uninstall answers synchronously only for a narrow set:
  `UnknownPlan` (the one-use plan expired or the helm restarted), `Busy`, a host row that vanished, and for add host a
  registration error. Such a definite refusal opens the dialog again showing it (see the planner proposal "Confirm-time
  refusals reopen").
- A refusal from the helm's recheck after confirmation is a failed run in the row, per decision (A). `start_run` answers
  202, and the recheck runs inside the run and ends it as failed (`finish_preflight_failure` in
  `crates/farhelm-helm/src/provisioning/service.rs`, which marks the first step failed with the reason). The rail shows
  that step as failed with the helm's reason, and the row stays expanded, as a failed Update's does today. There is no
  new marker and no dialog for these.
- Remove's dialog stays open until the helm answers and shows a refusal there. Retry and adopt refusals open a short
  refusal dialog.
- An add-host setup refused after its dialog closed (one of the synchronous refusals above) shows in the add-host
  dialog, the way a destination refusal does.
- Unchanged: a failure during an Update run (the row stays expanded with its steps, per SPEC.md); "committed but
  unreadable" warnings (state, not refusals); progress-read failures and the hosts list's own read errors.
- Refusal dialogs use the framed host dialog look (`HostDialogHeader` in
  `crates/farhelm-ui/src/hosts/settings_dialog.rs`): the host's mark tile, a title, a summary line in the danger color,
  the helm's reason as body text, and footer buttons.

Post-uninstall notice:

- When an uninstall this window confirmed removes the host from the list, the hosts panel shows a single line, for
  example "removed from <host> · data kept". Its hover text gives the full sentence and the data directory's path, a
  copy button copies that path, and × dismisses it. It replaces today's multi-line `.uninstall-notice` and its Dismiss
  button.

Specs, docs and bookkeeping:

- SPEC.md "Topology" and SPEC_impl.md ("GUI: Dioxus", "Provisioning") describe the new behavior; the conflicts listed
  under "Binding repository constraints" are resolved by updating the spec text in the PR that changes the behavior.
- The docs page `website/src/content/docs/docs/using/manage-hosts.mdx` matches the new notice and refusal behavior.
- Changelog fragments per root `AGENTS.md` ("Changelog fragments").
- The last code PR removes all three TODO.md entries.

## Requirement sources

**The user's decisions (2026-10-18):**

- Merge the three TODO entries into one plan and build them as one stack.
- Plan steps: the helm sends the setup and uninstall plan as a structured step list, with each step's action text and,
  separately, the path or paths it touches. The helm stays the only author of the wording: the UI is forbidden today
  from walking the plan's actions itself (the doc comment on `PlanConfirmation` in
  `crates/farhelm-ui/src/provisioning.rs`) so that the dialog cannot promise something the helm won't do, and the
  structured list must keep that property.
- The three "create or reuse directory" steps are ONE step with three paths. (Decision C below supersedes the mapping
  where they conflict: `create-directories` already is one run step, and it keeps all its paths.)
- Design "B. Numbered stepper": a vertical rail with numbered circles (accent-edge ring, accent number), each step's
  action in weight 600, its paths as small chips (well background, dim border) beneath; steps without paths show their
  explanation in muted text. The heading line ("Farhelm will set up <host> (<distro>, <arch>):") becomes the dialog's
  lead/summary. The uninstall plan's "Kept: the host's Farhelm data in <dir>. Deleting that directory by hand removes
  the data too." line becomes a separate callout under the list (amber note style), not a step. App tokens are in
  `crates/farhelm-ui/assets/app.css` (`--accent-edge`, `--accent`, `--well`, `--border-dim`, `--ok`, `--warn`, the muted
  text tones).
- Paths under the host's home directory show as `~/…`, with the full path in the hover text.
- Update's inline confirmation gets the same treatment.
- Selecting or copying the plan should still yield the helm's text where practical; the planner may keep a copy
  affordance or consciously replace the native-selection contract, with a justification (see the planner proposals).
- Progress, in the maintainer's words: "This looks really great. I like the idea of showing the progress in the same
  way. The only caveat here is that I don't want the modal dialog to force you to wait for it to complete... Can we
  carry over the style to the sidebar progress that is shown today? And also probably use the non-compact view for any
  host that is currently installing so you can easily see the status by default. But make it a one-off expansion so that
  a user can compact it if they really don't want to see it." Chosen option "1. The stepper lives in the row":
  - Pressing "set up" closes the dialog (as today). The host's row shows expanded for that run, a one-off expansion: if
    the user collapses it, it stays collapsed for that run.
  - The expanded row shows the same stepper at a smaller size: completed steps green with a check, the running step
    pulsing amber, pending steps grey. Collapsed, the row keeps a one-line status like "setting up · 3 of 5".
  - A host update's progress uses the same rail.
  - Uninstall follows the same non-blocking pattern: its dialog closes on "uninstall", progress shows in the row the
    same way, and when the host leaves the list the result is a ONE-LINE notice in the hosts panel ("removed from <host>
    · data kept" or similar): the full sentence and the data path on hover, a copy button for the path, × to dismiss.
    This replaces today's large multi-line notice, and SPEC.md "Topology" is updated to the decided behavior.
- Refusals: the accepted table, as listed in the acceptance criteria above, including what stays unchanged.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter. No-workhorse mode.

**The user's later decisions (2026-10-18), answering questions the first round left open:**

- (A) A refusal from the helm's recheck after confirmation is a failed run in the row. The accepted table said an
  uninstall "refused after the user confirmed" opens a dialog, but such refusals arrive as failed runs, not as refusals
  of the submission. The stepper marks the failing step (the recheck) as failed with the helm's reason, and the row
  stays expanded, as a failed Update does today. No marker, no dialog for these.
- (B) Step wording, in the maintainer's words: "Use internal names so they always match 1:1, but map the names to
  something user friendly. 'place' is LLM jargon. but 'upload' makes perfect sense and is completely gone in the other
  proposal. ... propose a list that is 1:1 with the internal steps but with reasonable user facing terms (its' ok to
  assume the user knows what systemd is and such just not farhelm internals)". The approved names, one per internal step
  id (`ProvisioningAction::label()`, `plan.rs` around lines 201-231):
  - Setup and update: `create-directories` → "create directories"; `upload-farhelm` → "upload the farhelm binary";
    `install-farhelm` → "install the farhelm binary"; `upload-tmux` → "upload tmux"; `install-tmux` → "install tmux";
    `write-unit` → "write the systemd user unit"; `daemon-reload` → "reload systemd user units"; `enable-supervisor` →
    "enable and start the supervisor"; `enable-linger` → "enable lingering, to start at boot"; `restart-supervisor` →
    "restart the supervisor"; `attach-supervisor` → "connect to the supervisor".
  - Uninstall: `disable-supervisor` → "disable the supervisor"; `remove-unit` → "remove the systemd user unit";
    `daemon-reload` → "reload systemd user units"; `stop-supervisor` → "stop the supervisor and its tmux server";
    `remove-directory` → "remove the install directory"; `forget-host` → "remove the host from the list".
  - The helm owns these names and sends each with its step; the app keeps no copy of the wording. A step that had
    nothing to do (Skipped) shows as done in grey with "unchanged". `enable-linger`'s Degraded outcome shows as a
    warning (the supervisor starts at login instead), not a failure.
- (C) The confirmation lists the SAME steps as the run, one for one, under the same names, with each step's paths and
  explanation underneath ("yes, same steps and names; paths and explanations under each"). So the confirmation dialog
  and the progress rail are the same list, step for step, and "place" is gone everywhere, including from the plain-text
  confirmation the copy control yields; the exact-text tests change accordingly, deliberately.
  - Steps that today have no confirmation line (`upload-*`, `daemon-reload`, `attach-supervisor`) now appear in the
    confirmation.
  - Conditional steps (tmux, restart) appear only in plans that contain them.
  - The directories step keeps its several paths.
  - The uninstall "Kept: …" note stays a separate callout.

  This supersedes the earlier "three directory lines become one step" mapping where it conflicts, and the earlier
  distinction between plan steps and run steps: there is now one step list per plan, used by both.

**Binding repository constraints:**

- SPEC.md "Topology": setup states exactly what it will do before touching the host (files and where, units,
  persistence); uninstall names each path and the kept data directory with the note that deleting it removes the data;
  its refusals and their recheck at confirmation; Remove "stays available whatever its layout, and the refusal points to
  it"; Update's folded inline status, hover popup and failure expansion; reduced motion. These paragraphs currently say
  "the window that confirmed says Farhelm was removed and where the data remains" and that a running uninstall shows
  "the way an update's does"; update both to the decided behavior.
- SPEC.md "Lifecycle operations" (around lines 1153-1164): confirmation focus starts on the enabled confirming button;
  container Enter presses the main action; the shared host-dialog installer's empty-space rule.
- SPEC_impl.md "GUI: Dioxus": the paragraph that says a setup refusal or transport failure "is reported on the panel,
  since the dialog is gone" (around line 608), and the hosts paragraph that says "Starting setup opens details before
  planning" and describes each row's effective disclosure (around lines 548-557). Both change in this plan.
- SPEC_impl.md "Provisioning": one-use plans, revalidation at confirmation by "planning again and requiring the same
  plan", and "the confirming client builds its success notice from the plan it already holds".
- SPEC.md "Upgrade compatibility and client scale": no change may break the upgrade path of the desktop app, the helm or
  the supervisor. SPEC_impl.md (the client-to-helm build-stamp paragraph, around line 4497): the web UI and the desktop
  window are always served by their own helm's build; a mismatching stamp shows a reload prompt and withdraws only
  unattended behavior, while explicit actions keep working. SPEC.md "Topology" rules out any native-app-to-remote-helm
  mode. So the wire change must be additive (see the planner proposals).
- Root `AGENTS.md`: "Talking to the user" (product terms in PR text and the report), "Changelog fragments", "Tests and
  validation", "Frozen material" (the published release-notes shots and their tests, including the `.provisioning-plan`
  locators in `e2e/docs-shots/release-notes.spec.ts`, are not touched), "Instructions by path" (read `website/AGENTS.md`
  before editing the docs page; read `.agents/docs-media.md` before editing anything under `e2e/docs-shots/`; read
  `.agents/validation.md` before touching tests), and "The live install is off-limits".

**Planner proposals (implementation choices; each may be revisited under "Unattended fallback" if it proves wrong):**

- _One step model in the helm._ Replace `ProvisioningAction::confirmation_line` with a method in
  `crates/farhelm-helm/src/provisioning/plan.rs` that turns each action into one step. A step is a small type: id from
  `label()`, name from decision (B)'s table, paths, optional explanation.
  - `ProvisioningPlan` exposes the step list, the lead and the uninstall kept note.
  - `ProvisioningPlan::confirmation()` renders the plain text from them by one rule, since the wording changes
    deliberately anyway. A suggested rule: the lead; then one line per step, `- <name>`; indented lines under it for
    each full path and for the explanation; then the kept note.
  - `ProvisioningView::for_plan` (`http.rs`) builds its step entries from the same method.

  The explanations carry what the old lines said beyond the name: the unit's name and the persistent-run statement for
  `enable-supervisor`, the login fallback for `enable-linger`, that existing directories are reused and a shared one
  keeps its permissions for `create-directories`, and that the binary and any private tmux go with `remove-directory`.
  Reason: one list for confirmation and run is decision (C). Building both the confirmation and the progress view from
  one method means the names cannot drift between them, and the helm stays the single author.
- _Home directory._ Record the inspected host's home (`Reach::home`) on `ProvisioningPlan` beside `host_distro_id`, set
  it in both plan constructors, and let the helm emit each path in full and in its `~/…` form (`Path::strip_prefix`,
  component-wise, so a sibling directory whose name merely starts with the home's name is not under it). Reason: the
  helm already knows the home, and the UI cannot. The plan is compared for equality at confirmation; the home comes from
  the same inspection, so a rerun produces the same value.
- _Additive wire._ Add one new field next to `confirmation` in both plan reply types, for example `steps` holding
  `lead`, `steps[] {id, name, paths[] {full, short}, explanation}` and `kept`. Keep `confirmation` and `plan` as they
  are; `confirmation` now carries the new text. Add the same per-step fields (`name`, `paths`, `explanation`) to each
  progress entry (`StepView`), keeping `step` (the id) and `status`. On the UI side, decode the new fields with
  `#[serde(default)]`. When the plan's field is absent, render the old plain-text block. When a progress entry has no
  name, show its id. Reason for additivity: an old bundle in a stale tab ignores the unknown field (the UI's reply types
  do not deny unknown fields). Reason for the fallback: not upgrade safety, since a new bundle is always served by its
  own helm and never talks to an older one, but it is cheap and keeps the many hand-built `provisionable` stubs in
  `e2e/tests/provisioning.spec.ts` working. Nothing is persisted: plans and progress live in helm memory only, so there
  is no schema change.
- _Copying._ Replace the native-selection contract with a copy control in the plan's region ("copy plan") that copies
  the helm's `confirmation` string exactly, through `crate::session_view::copy_to_clipboard`. Reason: a stepper with
  numbered circles, chips showing `~/…` and a separate callout cannot make a native selection equal the helm's text
  without hidden duplicate text that screen readers would read twice. The e2e helpers `expectPlanCopyAndIndent`
  (`e2e/tests/provisioning.spec.ts`) and `expectUninstallText` (`e2e/tests/terminal-multihost.spec.ts`) change to click
  that control and assert the captured value equals the reply's `confirmation`, by stubbing
  `window.__farhelmNativeClipboardWrite` the way `e2e/tests/header.spec.ts` does; their glyph-indent checks move to
  whatever the stepper's layout needs (for example that a wrapped action line stays inside its step's column). Say in
  the PR description that the native-selection contract was replaced, and why.
- _Selectors._ Keep `.provisioning-plan` as the class of the plan's container, and `.provisioning-steps`,
  `.provisioning-step`, `.provisioning-step-name`, `.provisioning-step-status` with `data-step`, `data-status` and
  `data-current` on the progress rail. Reason: the docs-shots specs and many browser tests locate them, and the
  published release-notes shots must not need edits.
- _Lead and summary._ Put the helm's lead as the first line of the dialog body, above the rail and inside the
  `.provisioning-plan` container, and leave the header's own summary line as it is ("Farhelm is not running there yet"
  for setup, the host's status for uninstall). Reason: the header summary is the host-dialog convention for host state;
  this is the smallest reading of "lead/summary". The lead must sit inside `.provisioning-plan` because the frozen
  `e2e/docs-shots/release-notes.spec.ts` expects the ssh destination, which only the lead names, inside that container.
- _Progress labels._ The row's rail and the update popup show each step's name, with its id kept in `data-step` for
  tests and styling. Whether the row-sized rail also shows paths and explanations, or keeps them to the dialog and a
  hover, is the executor's call for the narrow sidebar; log it as a DECISION. Browser tests that assert
  `.provisioning-step-name` text change to the names; tests that key on `data-step` keep working.
- _One-off expansion._ Each row's provisioning panel (`ProvisioningPanel` in `crates/farhelm-ui/src/provisioning.rs`)
  remembers, client-side and per run id, a running setup or uninstall it has observed (including one another window
  started) and whether the user folded it, and publishes the expansion through the existing per-row automatic disclosure
  (`provisioning_auto_details`). A fold control in the run's header line appears only while the row is open because of
  that expansion, not because of the global checkbox or an update failure. Reloading the page forgets a fold; that is
  acceptable for a one-off. Explicit ADD and Uninstall requests stop setting the global details checkbox
  (`on_reveal_details` and the `details_open.set(true)` in the row's `on_provisioning` in
  `crates/farhelm-ui/src/hosts.rs`), which means the setup and uninstall dialogs must render whether or not the row's
  details are open: today they sit inside the `if details_open && has_details_content` block. The
  `details_open.set(true)` calls in `on_retry` and `on_adopt` (`hosts.rs`, around lines 1258 and 1283) exist only so
  their refusals show in the row; remove them in the PR that moves those refusals into dialogs. Reason: reuses the
  disclosure the hosts panel already ORs per row instead of adding a second one, and the existing `uninstall_clock`
  (first-seen time per run id) is the natural thing to generalize. Observing runs rather than only this window's
  submissions is also what makes the add-host case work, since the new row's panel never submitted that run.
- _Inline status for setup and for planning._ Extend `derive_row_update_progress` and `UpdateProgressSummary::verb` to
  running ADD runs ("setting up"), the way uninstall was added, and reuse `UpdateProgressLabel` and its popup unchanged
  apart from the rail styling. Extend the same inline pending status (`HostUpdateProgress::Pending(Planning)`) to ADD
  and Uninstall planning, so a folded row shows that planning is under way between the click and the dialog. Reason:
  SPEC.md already defines that status for update and uninstall; the decision's "setting up · 3 of 5" is an example of
  it, not a new label; and reusing the pending status is smaller than opening the row at the click and closing it again
  on cancel.
- _Refusal dialogs._ One small component built on `HostDialogHeader`, which gains an optional danger tone for its
  summary line, with the helm's reason through `PeerLine`, a Close button and an optional Remove button. The hosts panel
  owns the pending refusals as a host-keyed map shaped like `uninstall_notices` (`Signal<HashMap<HostId, _>>`, holding
  which action was refused, the text, and whether Remove is offered), shared with each row's provisioning panel the way
  `uninstall_notices` is. A newer refusal for the same host replaces the older one. The panel shows one entry at a time,
  in host-list order, while no other host dialog is open (settings, remove, add host, a setup or uninstall
  confirmation). Reason: "update all" can produce several refusals at once, two modals must never install focus
  isolation together, and a map avoids a FIFO's ordering questions. The uninstall and setup refusals use their own
  dialog's title ("uninstall Farhelm · <host>", "set up host · <host>"), so they read as the same dialog with the reason
  in place of the plan. Focus starts on Close (a refusal is not a confirmation), Escape and container Enter close, and
  closing returns focus to the row's menu toggle through the shared installer.
- _Remove remedy._ Add a typed refusal to `ProvisioningRequestError` for refusals whose remedy is Remove, and have
  `provisioning_error` set a response header for it (a new constant pair in `crates/farhelm-proto/src/http.rs`, in the
  style of `YOLO_CONFIRMATION_HEADER`). Use it for the refusals whose text names Remove today, which is the symlinked
  lib-directory refusal in `crates/farhelm-helm/src/provisioning/service.rs`. The UI reads the header, never the prose.
  Only the uninstall planning reply needs it: `plan_host_uninstall` in `crates/farhelm-ui/src/api.rs` returns the flag
  with its error. `SubmissionError`, `uninstall_host` and the confirm-time path do not change, because a refusal found
  after confirmation never reaches the submission reply (decision A). Reason: refusals are matched by type, not by
  wording (`provisioning_error`'s doc), and the header is additive.
- _Planning failures are refusals._ `plan_host_update`, `plan_host_uninstall` and the setup rerun's planning return the
  same `Err(String)` for a helm 409, a 502 host failure and a transport error. Route every planning failure of an action
  the user chose to the dialog, without teaching the API to tell them apart, and log this as a DECISION. Reason: it is
  the smaller choice, and it is the only distinction the UI has today. Update planning outcomes that are not failures
  (`Unvalidated`, which may have registered something) stay in the row as state.
- _Confirm-time refusals reopen._ Confirming still closes the dialog at once. A definite synchronous refusal of the
  submission (`UnknownPlan`, `Busy`, row gone, add host's registration error) opens the dialog again showing the
  refusal: the uninstall and setup refusal dialogs through the map, and for add host the add-host dialog itself, with
  the destination filled in and the refusal where a destination refusal shows. Reason: the POST cannot be recalled, so
  keeping the dialog open with a live Cancel during it would offer a cancel that does not cancel. A lost reply
  (`SubmissionError::Ambiguous`) is not a refusal: it may have committed, so it stays a line in the row, like the
  committed-but-unreadable warnings. Recheck refusals after confirmation stay failed runs in the row (decision A).
- _The local row._ The local row never shows a setup plan. With nothing running, the local probe answers with the manual
  `farhelm helm setup` handoff (`ProbeResponse::Manual`), and Update and Uninstall refuse local rows, so no production
  path mints a local plan. Only browser route stubs reach `SetupPlanConfirmation { local: true }`. That handoff text is
  the row's remedy (state, not a refusal) and stays in the row as today. Do not design or test a local setup dialog. The
  module doc at the top of `crates/farhelm-ui/src/provisioning.rs` ("A provisionable reply replaces the old manual-start
  hint") is stale for this reason; do not rely on it.
- _Leftovers._ Once retry, adopt and remove refusals no longer go into the hosts panel's per-host `errors` map, check
  whether `field_outcome`'s "at the top" case in `settings_dialog.rs` still has a producer, and remove it if not.
  Removing in skip-confirmation mode has no dialog to keep open, so its refusal goes into the refusal map as a short
  refusal dialog titled for removal.

## Outline

A linear stack, steps first, then progress, then the notice, then refusals. Suggested slicing (bookmarks
`plan/host-uninstall-dialogs/<nn>-<short-name>`):

1. `01-plan-steps-wire`: the helm's one step model, built from the run's actions with id, name, paths and explanation;
   `host_home`; the new plan reply field; the same entries on each progress view step; and `confirmation()` rendered
   from the step list. Helm tests:
   - exact new-text tests for setup (rewritten), uninstall and update;
   - one step per action in action order, including the upload, daemon-reload and attach steps;
   - conditional tmux and restart steps present only when planned;
   - the directories step with all its directories, including the four-directory update case;
   - the `~/…` form and its boundary case;
   - the uninstall kept note separate from the steps;
   - progress entries matching the plan's steps one for one;
   - no "place" in any text.

   The plain-text confirmation changes visibly (it is what the copy control will yield), so the title is user-visible
   and gets a changelog fragment that PR 2 extends.
2. `02-stepper-dialogs`: decode the field in `crates/farhelm-ui/src/api.rs` (probe, update and uninstall replies) and
   carry it in `PendingPlan` and the add-host offer; one shared stepper component used by `SetupPlanConfirmation` and
   `UninstallConfirmation`, and reused without dedicated tests by the defensive `PlanConfirmation`; lead inside
   `.provisioning-plan`, kept callout, chips with full-path hover; the copy control; styles in `app.css` with light and
   dark tokens; the plain-text fallback; e2e helper and layout test updates; the SPEC_impl.md "Provisioning" sentence
   about how the plan is shown and copied, and how the confirmation and the run share one step list. Changelog fragment
   (PR 1's, extended).
3. `03-progress-rail`: the rail styling for `.provisioning-steps` in the row and the update popup, showing the helm's
   step names and the step states from decision (B) (unchanged, warning, failed with the helm's reason); the one-off
   per-row expansion with its fold control; the dialogs rendered outside the details gate; the inline status extended to
   setup, and the inline pending status to setup and uninstall planning; the global reveals on explicit setup and
   uninstall removed; SPEC.md "Topology" (setup and uninstall progress) and SPEC_impl.md "GUI: Dioxus" (disclosure
   paragraph); unit tests for the expansion decision (observed running run expands once, a fold sticks for that run id,
   a new run expands again, success folds, failure stays open, global details and update disclosure unaffected) and for
   the setup inline summary; a browser test that a recheck refusal after confirming uninstall shows as the failed step
   with the helm's reason in the expanded row (decision A); browser tests updated where they assert the global details
   checkbox after setup or uninstall, or a step's displayed name. Changelog fragment.
4. `04-uninstall-notice`: the one-line notice with hover sentence, path copy and ×; `uninstall_notice_parts` and its
   unit test (`crates/farhelm-ui/src/hosts.rs` around line 4632); the e2e notice assertions in
   `e2e/tests/terminal-multihost.spec.ts`; SPEC.md "Topology" sentence; the docs page's notice sentence in
   `manage-hosts.mdx`. Changelog fragment.
5. `05-uninstall-refusals`: the Remove-remedy refusal type and header on the uninstall planning reply, with helm tests;
   the refusal dialog component and its host-keyed map; uninstall and setup rerun planning failures, Update planning
   failures, and the synchronous submission refusals of a confirmed setup rerun or uninstall moved out of the row's
   `action_error`/`update_error` lines into it (Update submission refusals stay where they are, and recheck refusals
   after confirmation stay failed runs per decision A); the Remove button path; Rust tests for which outcomes go to the
   dialog and which stay in the row; browser tests updated from `.provisioning-error` to the dialog. Changelog fragment.
6. `06-host-action-refusals`: Remove's dialog staying open with its refusal (and the skip-confirmation path), Retry and
   adopt refusal dialogs with their `details_open.set(true)` calls removed, add-host's synchronous submission refusal
   shown in the reopened add-host dialog; SPEC_impl.md "GUI: Dioxus" sentence about panel reporting; the `field_outcome`
   cleanup if it applies; the docs page's refusal sentences if they describe where a refusal appears; removal of all
   three TODO.md entries. Changelog fragment (edit PR 5's if it is still unreleased and covers the same change).

Restructure the slicing if a different cut avoids churn; keep the order steps, progress, notice, refusals unless a
dependency forces otherwise, and keep the TODO removal in the last code PR.

### Validation

Choose per `.agents/validation.md` and `.agents/test-evidence.md`; at least:

- every PR: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check` on changed
  Markdown, `python3 releasing/check-changelog.py format` when a fragment changes;
- PRs touching the helm (1, 5): the `farhelm-helm` provisioning unit tests through the recorder (narrow nextest
  selection over the provisioning module, not the workspace);
- PRs touching the UI (2-6): `cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` provisioning and hosts
  unit tests through the recorder, and `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` when
  tests change;
- browser tests through the recorder on Chromium and WebKit, after `cargo build` and the release `dx build` the
  inventory names: the changed and affected tests in `e2e/tests/provisioning.spec.ts` (setup layout, plan copy, refusal
  and add-host error tests, the update popup and inline status tests) and in `e2e/tests/terminal-multihost.spec.ts`
  (uninstall, uninstall progress, row error tests). Not the full suite unless a specific risk needs it;
- the docs page edit: the website build only if a link or heading changes; no preview server (an unattended run names
  the changed page in the report instead).

The report says which of the docs screenshots (`e2e/docs-shots/add-a-remote-host.spec.ts`) now show an outdated dialog,
for a later "refresh the docs screenshots"; this plan does not capture or publish screenshots.

## Fallbacks

- If the copy control cannot be tested reliably on WebKit through the clipboard stub, keep the control and assert its
  copied value on Chromium, and log the gap as a DECISION.
- If a step's explanation cannot carry an old line's promise without becoming a second name (for example the unit's name
  for `write-unit`), put it in the step's paths or explanation as the helm sees fit, keep decision (B)'s name unchanged,
  and log it.

Examples of departures that need the scope reassessment: renaming a step away from decision (B)'s table, adding, merging
or hiding steps so the confirmation and the run stop matching one for one, a recheck marker on the progress view or a
dialog for recheck refusals (decision A rules both out), persisting fold state, changing which helm refusals point to
Remove beyond those whose text already names it, holding the setup or uninstall dialog open across its submission
(Remove's dialog stays open by decision and is not a departure), splitting planning failures by HTTP status, moving
Update submission refusals or session-list errors (both outside the accepted table) into dialogs.

Decisions to log: the wire field's final shape and name, the plain-text layout and each step's explanation wording, what
the row-sized rail shows besides names, the copy control's placement and wording, the fold control's placement, routing
every planning failure to the dialog, which outcomes stay in the row, the slicing actually used.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-uninstall-dialogs-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/host-uninstall-dialogs/<nn>-<short-name>` (for example `plan/host-uninstall-dialogs/01-plan-steps-wire`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the three TODO.md entries this plan covers (each ends with this plan's `Plan:` reference).
  TODO.md is a shared bookkeeping file: on a rebase conflict there, keep both sides.
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
each PR passed its review gate, and the last code PR removes the three TODO.md entries. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a
closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
