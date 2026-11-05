# UI: Tensorlake fixes: pinned recipe Save, confirmed suspend, honest launch, structured add-host dialog

Written against main at d81a3980 on 2026-11-04, right after the Tensorlake host stack (#2932–#3010) landed. This is a
goal file for an unattended run, executed through the planning system in `plans/AGENTS.md`. You, the executing agent,
have none of the conversation that produced it; everything you need is here or in the repository files it names. SPEC.md
and SPEC_impl.md stay authoritative over anything this file says, except where this file records a change the maintainer
approved; that change is yours to make in the spec.

## The goal

Five TODO.md entries (bucket "Near term", subsection "Tensorlake"), verbatim, merged into one plan with the maintainer's
agreement because they touch the same few files (the add-host dialog, the recipe editor, the Hosts panel, the helm's
suspend and resume), and two of them (the early "Suspended" and the launch flash) share one root cause, the lack of a
suspending or resuming host state:

> - **TensorLake recipe Save button hidden until scrolling.** When adding a TensorLake recipe, the Save button is not
>   visible until the view scrolls down to it; it must be reachable without hunting. Details TBD.
> - **TensorLake sandbox suspension must be confirmed before shown as suspended.** Suspending a TensorLake sandbox looks
>   suspiciously fast, so the user can get the impression it is suspended before that state is actually confirmed. The
>   UI must not present the sandbox as suspended until the suspension is confirmed.
> - **TensorLake "Refresh sandboxes" button feels inert.** In the TensorLake settings menu, "Refresh sandboxes" sits
>   below "New recipe" as a bare button and appears to do nothing; there may be an empty list above it. Unclear UX; fix
>   TBD.
> - **Transient "selected host is unavailable" while launching on a suspended sandbox.** Launching a session on a
>   suspended TensorLake sandbox briefly shows a "selected host is unavailable"-style message before the session starts
>   (launch resumes the sandbox, so starting is expected). The UI should not flash a misleading unavailable state when
>   launch will resume the sandbox.
> - **TensorLake add-host sandbox dialog lacks structure.** The dialog for adding a TensorLake sandbox host is just a
>   block of text with no visual structure. Fix TBD.

A sixth entry in the same subsection overlaps the launcher fix so closely that the planner folded it in (stated to the
maintainer in the planning PR):

> - **Show progress while a launch resumes a suspended host.** Launch on a suspended Tensorlake host resumes it first: a
>   lifecycle request with a 150-second client budget, then up to 60 seconds waiting for the new connection. Nothing
>   shows progress during that time; the launch-progress status ("Launching on …") deliberately starts only when the
>   session create is sent, so it cannot be reused as is without claiming a launch that has not happened. Add a
>   separately worded "Resuming host" preparation status (in the quick-launch surface too, where it can be reached),
>   with its own failure wording, which must not promise the session-create retry guarantees.

The maintainer's words when choosing the add-host entry, verbatim: "mock up a decent solution directly, check what it
ACTUALLY looks like and propose a real mockup fix". The planner captured the real screens from the stack with the
browser stack's fake Tensorlake and settled the designs below on them.

### What the captures showed (the starting point)

- **Recipe editor in Add host** (Add host → Tensorlake sandbox → Recipe → "New recipe…"): the editor (`RecipeEditor`,
  `crates/farhelm-ui/src/tensorlake/recipe_editor.rs`) renders inside the dialog's scrolling body
  (`crates/farhelm-ui/src/tensorlake/add_host.rs`), and the dialog's pinned `DialogFooter` is removed while editing
  (`if !editing() { DialogFooter … }`). Its Cancel and Save (`DialogActions { class: "recipe-editor-actions" }`) sit at
  the end of roughly 1,300 px of scrolling. It also carries its Settings navigation: a full-width "‹ Recipes" button and
  a small "New recipe" label rather than a heading, and the Name field is not focused.
- **Recipe editor in Settings → Tensorlake:** Save is again at the end of the scrolling panel; the Settings dialog's
  pinned footer holds only Close (`crates/farhelm-ui/src/settings.rs`). The editor opens scrolled to the middle of the
  panel, with the previous paragraph cut off at the top, and its field labels (CPUs, Claude Code base URL, …) are larger
  than its own "Size" heading and than the same labels in Add host.
- **Suspend:** the helm publishes `HostState::Suspended` before it calls Tensorlake
  (`crates/farhelm-helm/src/tensorlake/lifecycle.rs`, `change()`, through `hold_connection`), then POSTs suspend and
  polls up to 60 s. So the Hosts panel shows Suspended, a greyed Resume, and the detail "Suspended outside Farhelm,
  noticed at …" (Farhelm records its own suspend time only after confirmation) while the request is still in flight.
  Resume has a similar window: after Tensorlake confirms running, the helm republishes `Suspended { observed_at: now }`
  under the hold and clears the suspend timestamp before releasing the hold, so the row can read "Suspended outside
  Farhelm, noticed at <now>" in the middle of a resume. The suspend confirmation (`hosts.rs`, "Suspend host" dialog) has
  no header, its text sits flush to the card edge, and Cancel / Suspend render as plain centered text. The Tensorlake
  host's details line in the Hosts panel ("Sandbox: … · Recipe: … · 2 CPUs · 4 GB memory") renders as large unstyled
  text flush with the panel edge.
- **Launch on a suspended host:** the launcher (`crates/farhelm-ui/src/list/create_form.rs`) counts a host available
  when connected or suspended. Its submit path calls `api::resume_host_connected` under its operation guard, which
  covers Tensorlake's resume and the wait for the new connection. After resume the helm releases its hold and the host
  is published as Connecting, which the launcher counts as unavailable, so the amber note "The selected host is
  unavailable; choose a connected host before launching" shows for up to 60 s while the launch is in fact proceeding.
  Nothing shows progress during the resume.
- **Refresh sandboxes:** the button (`crates/farhelm-ui/src/tensorlake/orphans.rs`) re-reads the "Sandboxes not on this
  helm" list, which is hidden whenever it is empty (SPEC.md says so), so a refresh that finds nothing changes nothing on
  screen: no progress, no "none found". The button and "New recipe" above it stretch to full width; "Refresh sandboxes"
  has no heading of its own, and the "empty list above it" is the Recipes section's "No recipes yet.".
- **Add-host form and confirmation:** the form step has Name and a native, thinner Recipe select, without help text. The
  confirmation step is an unclassed `h3` "Create Tensorlake sandbox?", plain paragraphs separated by large gaps, and an
  `ol.provisioning-plan-steps` whose bare `li`s render as unnumbered lines that run together. The SSH flow's
  `SetupPlanConfirmation` (`crates/farhelm-ui/src/provisioning.rs`, with `PlanStepList`) already has a `tensorlake` mode
  (title "Create Tensorlake sandbox · name", billing text, Cancel / "Create sandbox"), used only for setup reruns.
- **Also noticed:** "API key" appears twice in Settings → Tensorlake (heading and field label); the experimental banner
  says "Try at your your own risk" (`crates/farhelm-ui/src/tensorlake_settings.rs`); SPEC_impl.md says the fake's
  "Suspend and resume answer 501", but the fake implements both.

### What changes

1. **Suspending… and Resuming… host states.** The helm gains one new host state for an explicit lifecycle call in
   progress, with a direction (suspending or resuming) and the time the request was made. Suspend publishes it under the
   hold instead of the early `Suspended`; on Tensorlake's confirmation the helm publishes `Suspended` under the hold (so
   the UI flips on confirmation, not on the actor's next status call) and then records Farhelm's suspend time and
   releases the hold as today, so the detail reads "Suspended from Farhelm at …". A failed or timed-out suspend leaves
   Suspending… and reconnects as after any hold (through Connecting, since Farhelm's SSH masters were already closed),
   with the error reported as today. Resume publishes Resuming… under a hold around the Tensorlake call, including the
   window after confirmation where today's code republishes `Suspended`; it takes that hold only when the actor is not
   already connected, because the code deliberately keeps live viewers attached on a repeated Resume. After release, the
   host shows the existing Connecting… and then Connected, as after any hold: Resuming… does not extend through the
   reconnection. Farhelm's own SSH masters still close before the suspend call, as SPEC_impl.md requires. Tensorlake's
   own `Suspending` status, which the connection gate folds into Suspended, stays folded (no new mapping). Hosts panel:
   the row shows "Suspending…" or "Resuming…" with a spinner and how long ago Tensorlake was asked, and Resume greyed
   while suspending.
2. **The launcher shows its own resume.** While the launcher's own submit is resuming its host (a local "this form is
   resuming host H" signal, set around `resume_host_connected`), that host counts as available to this form and the
   amber note is replaced by a progress line, worded per path: on the existing-folder path "Resuming gpu-dev, then
   connecting… the session starts when it's ready"; on the fresh-checkout path, which resumes and then asks the user to
   review the preview and launch again, wording that does not promise the session will start (for example "Resuming
   gpu-dev, then connecting… you'll review the checkout and launch again"). A helm-published Suspending… or Resuming…
   does not count as available in general: a launch on a host another surface is resuming would pass the form's check
   and then be refused (the submit path only resumes suspended hosts, and the helm answers 409 while the provisioning
   lock is held). The amber note remains for a host that is genuinely unavailable. A failed resume gets its own failure
   wording, which must not promise the session-create retry guarantees (the folded-in entry's requirement). Where the
   quick-launch surface can launch on a suspended host, it shows the same "Resuming host" preparation status; if it
   cannot reach a suspended host, say so in the report and change nothing there.
3. **The suspend confirmation and the host's sandbox details line get standard structure:** the confirmation becomes a
   header / body / footer dialog in the app's confirmation anatomy, with a title ("Suspend gpu-dev?"), the existing
   text, and Cancel / Suspend buttons in the footer; the details line uses the Hosts panel's normal detail styling.
4. **Recipe editor actions are always visible.** In Add host, the editor renders its own dialog body and pinned
   `DialogFooter` as direct children of the dialog, following `SetupPlanConfirmation`'s pattern (`add_host.rs` already
   drops its own footer while editing), so its Cancel / Save sit in the true pinned footer; no footer-slot prop. In
   Settings → Tensorlake, the editor's action row is sticky at the bottom of the scrolling body
   (`position: sticky; bottom: 0` with the dialog background), above the unchanged Close footer. This is a planner
   refinement of the page's "the footer swaps Close for Cancel / Save": Settings keeps every panel mounted, so a footer
   swap would put recipe actions in another section's footer when the user switches sections with the editor open. The
   leave and delete questions keep their slot beside the actions. In Add host the editor drops the Settings-style "‹
   Recipes" back button, uses a proper "New recipe" heading, and focuses Name on open. In Settings the editor opens
   scrolled to its top, and its field labels match its section headings' hierarchy and the Add host version.
5. **Sandboxes not on this helm is always shown** once an API key is saved: a heading, its help text, and Refresh as a
   small button beside the heading. Refresh shows "Checking…" while it runs; an empty result shows "None found · checked
   HH:MM"; a failure shows its reason as today. "New recipe" stops stretching to full width. SPEC.md's "The list is
   hidden when empty" is amended accordingly. This is local state in `OrphanList`.
6. **The add-host dialog is structured.** The form step adds help text under Name ("Shown in the rail and on session
   rows. The sandbox is named farhelm-host-…") and Recipe ("What the sandbox runs. New recipe… opens the editor here."),
   a select styled like the app's other inputs, and a one-line summary of the chosen recipe (size, software, tunnels)
   under the select. The confirmation step reuses `SetupPlanConfirmation` in its Tensorlake mode, which needs small new
   props: a header-summary override (the size), an optional Back, and the billing and 24-hour suspend text merged into
   one callout. These props also change the setup-rerun confirmation that uses the same mode; that is intended. The
   reused component keeps Copy plan where `PlanStepList` puts it (in the body), even though the planning mockup drew it
   in the footer.
7. **Small fixes from the captures:** the duplicated "API key" heading/label, the "your your" typo, and SPEC_impl.md's
   stale "answer 501" line. The planner's page also listed "the first setup step always says 'packages, Claude Code and
   Codex', even for a recipe that installs neither"; the recipe editor's own help (`recipe_editor.rs`) says the snapshot
   build always installs Claude Code and Codex, and the step text lives in
   `crates/farhelm-helm/src/provisioning/tensorlake_add.rs`. Check the build: if it always installs both, the step text
   is accurate; change nothing and say so in the report. If it does not, make the step name what the build installs.

### What does not change

The Tensorlake API calls, timeouts (60 s suspend, 120 s resume, 60 s reconnect wait) and the rule that a suspended host
is never resumed on its own; who can suspend and when Suspend asks first; the add-host flow's steps and its wording that
browser tests pin (the billing sentence, the pricing link, "Create sandbox", the "Name" and "Recipe" field names),
except where the new structure needs a test updated (`tensorlake-hosts.spec.ts` pins the old title "Create Tensorlake
sandbox?"); the fresh-checkout path that resumes and then asks to launch again.

### Acceptance criteria

- The helm never publishes Suspended for an explicit suspend before Tensorlake confirms it; during the call the host is
  Suspending…; on failure or timeout it reconnects with the error reported; resume shows Resuming… during the Tensorlake
  call and never "Suspended outside Farhelm" mid-resume. Every `matches!` on Suspended that the compiler will not flag
  gets a decision for the new state, in particular the Update/setup refusal in
  `crates/farhelm-helm/src/provisioning/service.rs`, which must refuse a Suspending or Resuming host too (a held state
  can stay published between release and the next probe), and in the UI `hosts.rs` (the Suspend versus Resume choice,
  the session banner and Resume host), `list/shared.rs` and `api.rs` (`resume_host_connected`).
- Launching on a suspended host never shows the "selected host is unavailable" note while this launcher's resume and
  reconnect are in progress, shows the per-path progress line, and shows its own failure wording on a failed resume; a
  host that is unavailable for other reasons, or being resumed by another surface, still shows the note.
- The suspend confirmation and details line look like the app's other dialogs and detail lines in both themes.
- The recipe editor's actions are visible without scrolling wherever the dialog's own footer is pinned (very short
  windows scroll the whole card, footer included, as in every framed dialog); keyboard order still reaches them after
  the fields; the leave and delete questions still appear and work.
- Settings → Tensorlake always shows the Sandboxes section with Refresh beside its heading, visible progress, and "None
  found · checked HH:MM" on an empty result.
- The add-host form and confirmation match the designs above, with the confirmation built on `SetupPlanConfirmation`.
- The small fixes are in.
- Each PR carries the SPEC.md and SPEC_impl.md text for what it changes (the host-state list and the hold paragraph in
  SPEC_impl.md's Tensorlake hosts section; SPEC.md's suspend, add host, and Settings → Tensorlake text; "The list is
  hidden when empty"), and the docs page `website/src/content/docs/docs/using/tensorlake-sandboxes.md` (and
  `manage-hosts.mdx`'s Tensorlake section) matches by the end.
- Tests: the real suspend and resume transitions (held, confirmed, failed, timed out, repeated Resume on a connected
  host) are tested in the helm's existing lifecycle harness in `crates/farhelm-helm/src/tensorlake/hosts_tests.rs` (it
  already has a `suspend_gate`). Browser tests stay on `tensorlake-lifecycle.spec.ts`'s existing approach (rewrite
  `/api/hosts` and gate the suspend and resume POSTs with its `routeGate`), now publishing the new suspending and
  resuming phases and the post-resume Connecting phase while a request is held, so the launcher's progress line and the
  Hosts panel rows are exercised. Do not add browser tests that create a real Tensorlake host row: the helm would dial
  Tensorlake's real SSH gateway (the stack has no fake for it; the planner's capture saw "Permission denied (publickey)"
  from the real gateway). Update `tensorlake-hosts.spec.ts`, `tensorlake-recipes.spec.ts` and
  `tensorlake-tunnels.spec.ts` for the new structure.
- Changelog: the Tensorlake stack is in no release yet, so these changes amend unreleased work: edit the stack's
  existing fragments in `releasing/changelog.d/` (`tensorlake-hosts.md`, `tensorlake-settings.md`,
  `tensorlake-host-model.md`) where a change alters what they say, and add none (root `AGENTS.md`, "Changelog
  fragments"). Before each commit, confirm the stack is still in no stable tag and the fragments still exist; add a new
  fragment only if a stable release has shipped Tensorlake in the meantime. Polishing behavior no user has seen may need
  no fragment text at all.
- TODO.md: each PR removes the entries it addresses (root `AGENTS.md`, "TODO.md and FLAKES.md"), including the folded-in
  "Show progress while a launch resumes a suspended host".

## Decisions already made

Settled with the maintainer on 2026-11-04 on a planning page with the real captured screens and mockups:

- **One plan, about four PRs,** rather than three or five plans.
- **Recipe editor actions pinned in both places:** Add host and Settings → Tensorlake. (Mockup: the Add host dialog with
  a footer reading "New recipe" on the left and Cancel / Save recipe on the right.)
- **Suspending… and Resuming… states,** chosen over keeping the previous state until confirmation. (Mockup: host rows
  reading "gpu-dev · Suspending… asked Tensorlake 4 s ago" with a greyed Resume, "tl-scratch · Suspended at 14:02 ·
  Resume to use it", and "box-2 · Resuming…"; and the launcher with "Resuming gpu-dev, then connecting… the session
  starts when it's ready" in place of the amber note.)
- **Restructure the suspend confirmation and style the sandbox details line** in this plan.
- **Refresh:** always show the Sandboxes section with Refresh beside its heading, "Checking…", and "None found · checked
  HH:MM". Chosen over a transient status and over removing the button.
- **Add-host dialog:** reuse the SSH setup-plan confirmation, add help text and a recipe summary. (Mockups: the form
  with help lines under Name and Recipe and a bordered summary "gpu: 4 CPUs · 16 GB · 50 GB · packages: …"; the
  confirmation titled "Create Tensorlake sandbox · gpu-dev" with the size in its subtitle, four numbered steps on a
  rail, an amber callout for billing and the 24-hour limit, and a footer with Back and Create sandbox.)
- **Include the small fixes** the capture found (all four the page offered; the setup-step one subject to the check in
  item 7, since the planner's description of it may have been wrong).
- **Plan as if the stack had landed,** which it then did.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

Planner refinements after the page closed, from a fresh-context planning review against the code, stated to the
maintainer in the planning PR: the folded-in sixth entry; Resuming… covers the Tensorlake call, not the reconnection;
Tensorlake's own `Suspending` status stays folded; Settings uses a sticky action row instead of swapping the footer's
Close; Copy plan stays in the confirmation's body.

## Requirement sources

**The user's request:** the TODO.md entries, the maintainer's words and the decisions above.

**Binding repository constraints:** SPEC.md's Tensorlake sections (hosts, suspend and resume, add host, recipes,
Settings → Tensorlake, sandboxes not on this helm); SPEC_impl.md's "Tensorlake account", "Adding Tensorlake hosts",
"Tensorlake recipes", "Tensorlake hosts" and the recipe tunnels and unowned sandboxes section;
`lore/2026-11-03-tensorlake-sandbox-hosts-design.md` for intent (frozen; do not edit); `.agents/tensorlake.md` if a test
path needs it; the modal anatomy in `crates/farhelm-ui/src/modal_dialog.rs` and `.host-dialog-framed` in `app.css` (only
the body scrolls; a `DialogFooter` that is a direct sibling of the body stays pinned); root `AGENTS.md`, "Code rules",
"Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `website/AGENTS.md`;
`.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code: the new state as one `HostState` variant with a direction and request time (removal
and setup publish `Unreachable` with a reason during their holds, but that cannot carry the label and elapsed time the
maintainer chose), with the UI's `HostPhase` and display helpers extended to match; four PRs: (1) the helm and UI
transitional states with the launcher progress, removing the suspension, launch-flash and launch-progress entries; (2)
the suspend confirmation and details line; (3) the recipe editor in both places, the Settings editor fixes, the
Sandboxes section with Refresh and the "API key" duplication, removing the Save and Refresh entries; (4) the add-host
dialog, the typo, the setup-step check and the stale SPEC_impl.md line, removing the add-host entry. Reorder if reviews
go better another way.

## Outline

1. **States and launcher.** Helm: the new state under the suspend and resume holds, `Suspended` published on
   confirmation, the conditional resume hold, refusals for the new state. UI: phases, labels, the spinner and elapsed
   time, the greyed Resume; the launcher's local resuming signal, per-path progress line, failure wording, and
   quick-launch coverage. Helm lifecycle tests; browser tests on the route-gated approach. Spec text and fragment edits.
2. **Suspend confirmation and details line.**
3. **Recipe editor and Settings.** The Add host editor's own body and footer, heading, focus and missing back button;
   the Settings editor's sticky action row, scroll and label sizes; the Sandboxes section with Refresh; "New recipe"
   width; the "API key" duplication.
4. **Add-host dialog.** Help text, styled select and recipe summary; the confirmation on `SetupPlanConfirmation` with
   its new props; the typo; the setup-step check; the stale SPEC_impl.md line; docs (an incidental docs edit in an
   unattended run: start no preview server and name the changed pages in the report).

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy on the helm and UI crates,
the helm's Tensorlake tests (`crates/farhelm-helm/src/tensorlake/`) and provisioning tests if the refusal changes, the
UI crate's tests, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and, through
`python3 scripts/record-test-run.py`, the `tensorlake-*.spec.ts` browser specs plus `provisioning.spec.ts` (shared
setup-plan confirmation) and the launcher specs touched by the availability change, on Chromium and WebKit. Run
`python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: changing Tensorlake API usage or timeouts; a general "transitional state"
framework for all host kinds; extending Resuming… through the reconnection; mapping Tensorlake's own `Suspending`
status; restructuring the recipe editor's fields beyond label sizing; new Settings sections; a fake for Tensorlake's SSH
gateway.

Decisions to log: the shape of the new host state, each `matches!` site's handling of it, the progress-line and failure
wording per path, quick-launch coverage, the setup-step text check's outcome, which fragments were edited, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tensorlake-ui-fixes-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/tensorlake-ui-fixes/<nn>-<short-name>`
  (for example `plan/tensorlake-ui-fixes/01-transitional-states` and `plan/tensorlake-ui-fixes/04-add-host-dialog`).
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
passed its code review gate, and the last PR has removed the last of the six TODO.md entries this plan covers (each PR
removes the ones it addresses). Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
