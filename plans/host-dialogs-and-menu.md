# Host dialogs and menu: an add-host dialog, a remove-host dialog, and a host menu that matches the session menu

Written against main at ea5bf905 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan has no dependency on another plan. Neighbors to read before starting, so your work fits what they landed:

- `plans/triage-confirm-ssh-identity.md`, ordered before this plan, changes the add form (its item 9: a new validation
  refusal shown in the form) and the probe lifecycle (its item 17: a dropped probe still finishes and may register the
  host). Expect both on main and carry them into the dialog.
- `plans/host-update-button.md` is ordered after this plan and edits the same host row. Nothing here waits on it.

## The goal

Three TODO.md `Near term` entries, built as one plan because they touch the same host-list code:

1. **The host menu looks and works like the session menu.** Same anchor beside the sidebar with a pointer, same header,
   icons, rounded inset rows and muted per-item descriptions, and the row's `⋯` toggle revealed on hover the way a
   session row's is.
2. **Removing a host asks in a pop-up dialog** that explains what removal does, with "remove", "remove, and don't ask
   again" and "cancel". The inline confirmation block under the host row goes away.
3. **Adding a host happens in a pop-up dialog.** It holds the ssh destination fields and the probe, and when the host
   needs setup it explains, in plain terms, every change Farhelm will make on the host, with "yes", "yes, and don't ask
   in the future" and "cancel". The sidebar no longer shows the raw step list.

Acceptance criteria are per PR, below. Overall: the three behaviors above work in the web UI on Chromium and WebKit; the
two "don't ask" answers are helm preferences that every client shares; SPEC.md, SPEC_impl.md and the user docs page
`website/src/content/docs/docs/using/manage-hosts.md` (plus the add-host page it links to, if it describes the old flow)
describe the new behavior; each PR removes its TODO.md entry.

## Requirement sources

**The user's request:** "use planning system to plan and schedule "Proper add-host dialog" and " Proper remove-host
dialog" and "Host menu should match the session menu". these are related and should be planned as one plan together."
The three TODO.md entries, verbatim as of ea5bf905:

- "**A proper dialog for adding a remote host.** Must fix. Adding a remote host still throws a wall of text into the
  sidebar. Instead, adding a host should show a clean, well-designed dialog that explains what will happen, and let the
  user answer either "yes" or "yes, and don't ask in the future"."
- "**A proper dialog for removing a host.** Removing a host from the sidebar today asks inline, in the host's row: a
  block of text ("forgetting a host leaves its supervisor and sessions running; re-adding the destination finds them
  again", then the quoted host name) above "confirm remove" and "cancel" buttons. Replace it with a clean, modern pop-up
  dialog that explains what removal will do, and add a "don't ask again in the future" option. Same direction as the
  host-add dialog entry above."
- "**Make the host pop-up menu match the session pop-up menu.** Redesign the host pop-up menu so it looks and feels
  exactly like the session pop-up menu: the same positioning, the same style, the same per-item descriptions, and so
  on."

**The user's decisions (2026-10-02):**

- U1. What the add dialog says: "it should keep what the user cares about. some read only probing etc is fine. tell them
  what we do that _changes_ things on the host." The dialog lists every change Farhelm makes on the host, in plain
  terms: which files it installs and where, the service it creates, that the service runs persistently and starts at
  boot (or at login when lingering is refused). Read-only probing and the helm's own bookkeeping are not listed. No raw
  step list.
- U2. The add dialog's buttons are "yes", "yes, and don't ask in the future", and "cancel".
- U3. "Don't ask in the future" skips only the setup confirmation: a later add that needs setup starts right after the
  probe, the way Update already submits without asking. Adding a host that already runs a supervisor never asked and
  still does not. For removal, "remove, and don't ask again" makes the menu's "remove" act at once from then on.
- U4. There is no way in the UI to turn asking back on, for now. The planning PR that added this file also added a
  `Near term` TODO entry for that; do not build it.
- U5. The host menu matches the session menu as in goal item 1, with one deliberate difference: Remove opens the new
  remove dialog rather than confirming inside the menu. On the toggle the user said "make it appear on hover just like
  for sessions", so the host toggle is hover-revealed exactly like the session toggle.
- U6. The add dialog holds the whole add flow (the destination fields, the probe, and its outcomes), not only the setup
  question.
- U7. Review gate: a fresh-context Opus 5.5 reviewer at high effort, told to review adversarially. No review swarm.
- U8. No-workhorse mode: you do all the work yourself (see How to run).

**Planner choices the user was shown and did not object to:**

- P1. Re-running a failed add on an ssh host (the row's re-run path, which today shows the plan in the row) uses the new
  add dialog's confirmation, and the "don't ask" preference skips it too.
- P2. The permanent answers are outlined, not filled, following the maintainer's 2026-09-30 rule in SPEC_impl.md (GUI:
  Dioxus, the button tiers paragraph): "yes" is the filled primary and "yes, and don't ask in the future" its outlined
  form; "remove" is the filled danger button and "remove, and don't ask again" the danger-outlined form, as the YOLO
  confirmation's "start, and don't ask again on this host" already is (`crates/farhelm-ui/src/yolo_confirm.rs` is the
  precedent for the three-answer pattern, its wording, and initial focus on cancel). Generalize that tiers paragraph
  ("the permanent answer is the outlined form of its one-off's tier") instead of listing a fifth look.
- P3. If saving a "don't ask" answer fails, the add or remove still proceeds; the worst case is being asked again next
  time. The preference write stays best-effort and silent, and the SPEC.md Errors and diagnostics bullet that names the
  helm-side preference's silently failing fields ("list order, last selection, and compact layout") gains the two new
  ones.
- P4. The setup text keeps the host's distribution and architecture (today's `host: <distro>, <arch>` line, in whatever
  plain form), so the user notices an unexpected machine. It describes the target, not a change, so U1 does not remove
  it.

**Binding repository constraints:**

- SPEC.md Topology: "Before touching the host for initial setup, the helm states exactly what it is about to do in
  concrete terms — the files it will place and where, the systemd units it will create, and that the supervisor will run
  persistently and start at boot — and proceeds only on confirmation." Amend it for U3: the helm still states this
  whenever it asks, and the user can choose not to be asked again. The plain-language text must still name the files and
  their locations, and the units.
- SPEC.md Session list: per-client persistence (browser storage and the like) is not wanted. The two answers are fields
  of the helm's preference row (`crates/farhelm-helm/src/preferences.rs`, the store, `PreferenceField` and
  `PreferenceValue` in `crates/farhelm-ui/src/api.rs`), shared by every client.
- SPEC_impl.md, GUI: Dioxus: the session-menu contract (Anchor, One at a time, Keyboard, Confirm in place). The host
  menu adopts Anchor, One at a time and Keyboard. Confirm in place does not apply to host removal, which opens a dialog.
  The host-list paragraph's "Every row always shows its name, phase dot, and muted actions toggle" and "setup's
  confirmation and active or retained progress stay under the row" both change. Progress still stays under the row.
- No native JavaScript dialogs (SPEC_impl.md: wry has none on macOS). Dialogs are Dioxus components:
  `HostSettingsDialog` (`crates/farhelm-ui/src/hosts/settings_dialog.rs`) is the closest template, and
  `crates/farhelm-ui/src/modal_isolation.rs` makes the rest of the page inert with an Escape safety net.
- The plan text is rendered once, by the helm, from the same actions execution consumes
  (`ProvisioningPlan::confirmation()` and `confirmation_line()` in `crates/farhelm-helm/src/provisioning/plan.rs`:
  "Render the plan without maintaining a second list of promises"), and the UI shows it verbatim (`PlanConfirmation` in
  `crates/farhelm-ui/src/provisioning.rs`). Keep that: rewrite the helm's rendering in place for U1; do not write a
  second, UI-side description.
- Root `AGENTS.md`: every `feat` PR carries a changelog fragment; a PR that addresses a TODO.md entry removes it;
  `website/AGENTS.md` and `website/EDITORIAL_RULES.md` govern docs pages.

## Implementation outline

Three PRs, in this order. Line numbers drift; find the code by name.

### PR 1: the host menu matches the session menu (`feat:`)

- Today: the host menu (`HostRow` and `HostMenuAction` in `crates/farhelm-ui/src/hosts.rs`) hangs below and left of its
  toggle via `menu_panel_placement_style` in `crates/farhelm-ui/src/menu_panel.rs`, with plain text items and no header.
  The session menu (`crates/farhelm-ui/src/list/row.rs`) opens beside the sidebar via `session_menu_placement_style` and
  `session_menu_pointer_style`, with a header, `MenuActionIcon` icons, groups and descriptions linked by
  `aria-describedby`. `menu_panel.rs` already shares the mechanics; item markup is deliberately per row.
- Move the host menu onto the session anchor and pointer, and delete `menu_panel_placement_style` and its unit tests (it
  has one caller) rather than keeping two placements.
- Header: the host's display name, and a muted summary line with its destination and Farhelm version, from what the row
  already knows. Pick the exact wording; keep peer text escaped the way the row already escapes it.
- Each command gets a small line icon in the `MenuActionIcon` style, the session menu's inset rounded rows and hover
  fill, groups separated by rules (connection: retry, adopt; provisioning: re-run, set up automatically, update;
  settings; remove — adjust if the code suggests a better grouping), and a muted description with `aria-describedby` on
  every item whose label does not say what it does. Draft the descriptions in the session menu's voice (short, lower
  case, what happens, e.g. "new session, keep this one").
- The host toggle is hidden until hover, focus, the row's selection-equivalent state, or the menu being open, exactly as
  the session toggle (app.css around `.session-row-menu, .host-row-menu`). Remove the CSS comments and the
  `.host-row-menu { opacity: 1 }` rule that justify the old exemption, and add `.host-row-menu` to the coarse-pointer
  `@media (hover: none), (any-pointer: coarse)` rule so touch users keep the menu.
- Removal still uses today's inline confirmation in this PR; PR 2 replaces it.
- SPEC_impl.md: the host menu follows the session-menu contract except Confirm in place; the host-list paragraph's
  "always shows ... toggle" sentence changes.
- Tests: `menu_panel.rs` unit tests; the `HostRow` render-count tests in `hosts.rs` if props change;
  `e2e/tests/helpers/fleet.ts` `openHostMenu` waits for `left: auto`, which only the old placement emits, so wait on the
  session placement's signal instead; the host-menu specs in `e2e/tests/terminal-multihost.spec.ts`
  (`host-menu-survives-the-longest-phase-word`, `host-menu-keyboard-contract...`, `automatic-setup-menu-order...`, the
  removal-prompt tests), `e2e/tests/sidebar.spec.ts` (menu-gutter and one-menu-at-a-time tests), and
  `e2e/tests/profiles.spec.ts` where they open the host menu.
- Changelog fragment `kind: changed`. Remove the TODO entry "Make the host pop-up menu match the session pop-up menu".

### PR 2: removing a host asks in a dialog (`feat:`)

- Today: the menu's `remove` opens `confirming_remove` (a `ConfirmSlot` in `hosts.rs`), which renders
  `.host-confirm-remove-panel` under the row; confirm calls `DELETE /api/hosts/{id}`.
- Replace that block with a modal dialog (template `HostSettingsDialog`, with `modal_isolation`): a title naming the
  host, a plain explanation (Farhelm forgets the host; its supervisor and sessions keep running; adding the destination
  again finds them), and "remove" (filled danger), "remove, and don't ask again" (danger-outlined), "cancel" (neutral,
  initial focus). Escape and cancel close it; focus returns to the row's toggle as the settings dialog does. Errors
  still land on the row's error line. Delete the inline block and its CSS.
- New helm preference, a nullable boolean (name it for what it records, e.g. `skip_host_remove_confirmation`): one store
  migration, the `Preferences`/`PreferencePatch` fields, the UI `PreferenceField`/`PreferenceValue` and write-queue
  slots, client-writable. Like every other preference it is read once after authentication: another client that is
  already open keeps asking until it reloads, which errs toward asking. No live sync. "remove, and don't ask again"
  writes it best-effort (P3) and removes. When it is set, the menu's `remove` removes at once.
- SPEC.md: the Topology/Session list text about removal, and the Errors and diagnostics preference bullet (P3).
  SPEC_impl.md: the removal flow. Docs: `manage-hosts.md`.
- Tests: rewrite the removal e2e tests in `terminal-multihost.spec.ts` (fits-the-sidebar becomes a dialog-fits test,
  cancel forgets nothing with focus on cancel, failed removal stays visible, remove and re-add) and add: "don't ask
  again" removes and a later remove acts without a dialog; the preference route accepts and returns the field (helm
  preferences tests); the store migration.
- Changelog fragment `kind: changed`. Remove the TODO entry "A proper dialog for removing a host".

### PR 3: adding a host happens in a dialog (`feat:`)

- Today: the heading's `add` button toggles `AddHostForm` (inline in `hosts.rs`), which probes (`POST /api/hosts/probe`)
  and, for a host that needs setup, replaces its fields with `PlanConfirmation` showing the helm's text verbatim;
  "confirm setup" calls `POST /api/hosts/provision` with the one-use `probe_id`. Progress then shows under the new row.
- The `add` button opens a modal dialog holding the same fields and the probe. Outcomes: an existing supervisor is added
  as today and the dialog closes; a manual or unvalidated outcome shows its message inside the dialog; a host that needs
  setup shows the plain-language text and "yes" (filled primary), "yes, and don't ask in the future" (primary-outlined),
  "cancel" (initial focus). Accepting closes the dialog; progress shows under the new row as today. Keep the existing
  check that the fields did not change between probe and confirm. Closing the dialog while a probe is in flight must
  keep whatever the inline form's unmount does today (the list refresh, and the item-17 behavior from
  `triage-confirm-ssh-identity.md` if it landed).
- New helm preference (e.g. `skip_host_setup_confirmation`), same shape and best-effort write as PR 2's. When set, a
  probe that answers "needs setup" submits the plan at once, the way `automatic_update` in `provisioning.rs` already
  submits remote updates; the helm still requires the one-use probe id, so no protocol change. The re-run of a failed
  add (P1) uses the same dialog confirmation and the same preference. Update `automatic_update`'s doc comment, and the
  test `only_remote_updates_submit_without_confirmation`, for the new rule.
- Rewrite `ProvisioningPlan::confirmation()`/`confirmation_line()` in place for U1 and P4: a short heading naming the
  destination and its distribution and architecture, then one plain line per change on the host: directories created
  (paths), Farhelm (and tmux, when installed) placed at its path, the user unit written at its path, the service enabled
  and running persistently, and boot start through lingering or login start when lingering is refused. Leave out
  temporary files, digest checks, atomic renames, the systemd reload and attaching the supervisor. The update-only
  restart line gets any honest plain line; remote updates never show this text and the helm offers no local plan. Update
  the helm's exact-text tests.
- SPEC.md Topology amendment (U3); SPEC_impl.md GUI host-list paragraph (confirmation moves to the dialog) and the
  button tiers paragraph (P2). Docs: `manage-hosts.md` and the add-a-remote-host page.
- Tests: `e2e/tests/provisioning.spec.ts` add-flow tests and its `probeRemote` and `holdLockWithAdd` helpers,
  `terminal-multihost.spec.ts` add-form tests (closing a stray menu, discovery, blank optional fields),
  `buttons.spec.ts` (tier tests naming `.add-host-form`), `profiles.spec.ts` focus on `.add-host-button`. Add: "yes, and
  don't ask" then a second add starts setup with no dialog question; the failed-add re-run uses the dialog.
- Changelog fragment `kind: changed`. Remove the TODO entry "A proper dialog for adding a remote host". This is the
  plan's final PR: mark this plan's line in `plans/INDEX.md` `[executed]`.

### What not to build

- No shared generic dialog framework beyond what falls out of the two dialogs naturally; a small shared component for
  the three-answer confirmation is fine if it removes duplication.
- No UI for turning asking back on (U4), no live preference sync, no per-host variant of the two answers.
- No change to discovery (an existing supervisor is still added without asking) or to Update's confirmation behavior.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-dialogs-and-menu-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain.

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

- Use the `jjstack` skill. The stack's base is not main but the tip of the plan stack, set up per `plans/AGENTS.md`
  (Executing, step 4). PRs already in the plan stack, from earlier plans or an earlier blocked run of this one, are the
  base and are not rewritten.
- PR 1, PR 2, PR 3 in that order, one commit, bookmark and draft PR each. Within this run, if a PR needs correcting,
  restructure it rather than stacking a correction on top, and do not add code in one PR that a later PR of this plan
  deletes (PR 1 keeps the inline removal block untouched precisely so PR 2 deletes it once).
- Commit messages and PR titles use Conventional Commits; all three PRs are `feat:`. Each adds its changelog fragment
  under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- Every PR: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins
  -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, `dprint check` on
  changed Markdown, the farhelm-ui unit tests for the touched modules.
- Browser: this plan is UI work, so run the affected Playwright specs on Chromium and WebKit through the recorder (build
  per root `AGENTS.md` first): the host-menu, removal and add-flow tests named above, plus `buttons.spec.ts` for the
  tier changes. Not the full suite unless a specific risk needs it.
- PR 2 and PR 3: the helm store, preferences and provisioning tests that cover the new fields and the rewritten plan
  text; `python3 releasing/check-changelog.py format`; the website build
  (`cd website && bun install --frozen-lockfile &&
  bun run build`) when docs pages change.
- Any PR that changes Rust or browser tests or their helpers: `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of that PR's changes. The user
demands a fresh-context agent on Opus 5.5 at high effort, reviewing adversarially (U7), shelled out to the other harness
if the executing one cannot reach that model natively. No review swarm. The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its section above, The goal, U1-U8 and P1-P4. For a
PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md`
requires. Address what the reviewer finds before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a generic dialog framework, a live preference feed,
a new helm route or protocol change, a UI-side rendering of the setup plan, a second menu placement kept alongside the
session one; these are examples, not a blacklist), and whenever the same component has needed repeated corrective review
rounds, run a fresh-context review through galaxy-brain with this charter, supplying the request, the user decisions
above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the menu groups and every description's wording, the header summary wording, the
preference field names, the plain-language plan lines, how closing the add dialog mid-probe behaves, whether a shared
confirmation component was extracted, and every reviewer finding you declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing, step 7). Because the PRs
form one linear stack, if one PR blocks, do not build the later ones on top of it.

## Done criterion

The plan is complete when the three draft PRs exist as one linear stack on the plan stack's tip, each satisfies its
section above and the acceptance criteria, each has passed the review gate, each has removed its TODO.md entry, and PR 3
has marked this plan's `plans/INDEX.md` line `[executed]`. Open, not merged: merging is the user's job. Then close the
plan per `plans/AGENTS.md` (Executing, step 8): write its report, write a closing entry in its log, and stop the
watchdog.
