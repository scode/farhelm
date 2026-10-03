# Host confirmations toggle: a settings dialog that turns host setup and removal confirmations back on

Written against main at dc1ec664 on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan runs after `plans/queue/host-dialogs-and-menu.md` (its `INDEX.md` line says so): that plan adds the two "don't
ask" answers this one makes reversible. When it was written, that plan's PRs (#1472, #1496, #1502) were open and being
landed. Read what actually landed on main, not this file's description of it, before starting; names below come from
those PRs and may have moved.

## The goal

The TODO.md `Near term` entry "A way to turn host add and remove confirmations back on" (verbatim below). Once the host
dialogs plan has landed, adding a host that needs setup asks in a dialog with "yes", "yes, and don't ask in the future"
and "cancel", and removing a host asks with "remove", "remove, and don't ask again" and "cancel". Both permanent answers
are helm preferences shared by every client, and nothing in the UI undoes them. This plan adds the way back:

1. A gear button in the app bar, immediately to the right of the version number, opens a modal **settings** dialog.
2. The dialog holds exactly two checkboxes, "set up new hosts without asking" and "remove hosts without asking", each
   ticked when its "don't ask" answer is in force, each with a help line that describes the current state. Unticking one
   turns its confirmation back on; ticking one is the same choice the host dialog's permanent answer makes.
3. Under each host dialog's permanent answer, a short line tells the user it can be undone, naming the gear as the place
   (for example "you can turn this back on with the gear at the top of the sidebar"). See P3 for why it names the gear
   rather than saying only "in settings".

Acceptance criteria: the three behaviors above work in the web UI on Chromium and WebKit and compile into the desktop
build; a change made in the settings dialog takes effect at once in the same client (the next add or remove asks, or
does not); SPEC.md, SPEC_impl.md and the hosts page of the docs website say where the switch lives; the PR removes the
TODO entry.

## Requirement sources

**The user's request:** "using planning system to plan "desktop without a network port" and "re-enable host
confirmations"" (the desktop entry is a separate plan). The TODO.md entry, verbatim as of dc1ec664:

- "**A way to turn host add and remove confirmations back on.** The host dialogs plan adds "yes, and don't ask in the
  future" to the add-host dialog and "remove, and don't ask again" to the remove-host dialog, both kept as helm
  preferences shared by every client, but no way in the UI to undo either answer: decided 2026-10-02 to leave that out
  of the plan and add it later. Farhelm has no app-wide settings screen to put it in, so where the switch lives is the
  first question."

**The user's decisions (2026-10-03):**

- U1. Placement: "a gear immediately to the right of the version number in the top left of the screen." The app bar
  (`AppBar` in `crates/farhelm-ui/src/app_bar.rs`) is the sticky row at the top of the sidebar: wordmark, the macOS
  window drag region, then the version. The gear goes right after the version.
- U2. A modal "settings" dialog holding the two checkboxes, worded like the existing YOLO switch ("start YOLO sessions
  here without asking" in the host settings dialog), with a help line per state.
- U3. "only those switches. Things that have a natural place in the main ui can stay there, accessible." The session
  list's compact checkbox and sort order, and every per-host setting, stay where they are and do not move into or get
  duplicated in this dialog.
- U4. The pointer-back line under both permanent answers: yes.
- U5. Review gate: a fresh-context Opus 5.5 reviewer at high effort, no swarm.
- U6. No-workhorse mode: you do all the work yourself (see How to run).

**Planner choices, shown to the user without objection:**

- P1. The dialog is called "settings" and is app-wide, not per host; its title or a short intro line says the choices
  apply to every host and every client of this helm, since that is what distinguishes it from a host's own settings.
- P2. Checkbox state reads the preference the way the host dialogs do (`== Some(true)` means "without asking"). A change
  writes the explicit boolean through the existing preference write path; no new route, field or migration. If the host
  dialogs plan landed a different reading rule, follow it.
- P3. The pointer-back line names the gear, not just "settings": each host's own menu has an item labelled `settings`
  (the host settings dialog, titled "host settings · <name>"), and both host dialogs sit right next to it, so "in
  settings" would send the user to the wrong dialog. For the same reason the new dialog's title must read differently
  from "host settings · …". The user approved the line's purpose (U4); this sharpens its wording, it does not change it.

**Binding repository constraints:**

- SPEC.md Session list: these choices are helm preferences shared by every client; per-client persistence (browser
  storage, a desktop state file) is not wanted. Read SPEC_impl.md's preferences paragraph
  (`GET`/`PUT
  /api/preferences`, read once after authentication by `PreferencesGate`, sparse patch writes) and use
  that machinery: `Preferences`, `PreferenceField`, `PreferenceValue` and the write queue in
  `crates/farhelm-ui/src/api.rs`, and the `SharedPreferences` context the host dialogs read.
- SPEC.md Errors and diagnostics names which preference writes fail silently; the host dialogs plan added these two
  choices there. A failed write from the settings dialog follows the same rule unless the code already gives preference
  writes a visible outcome; do not invent a new error surface for it.
- No native JavaScript dialogs (SPEC_impl.md: wry has none on macOS). Template: `HostSettingsDialog` in
  `crates/farhelm-ui/src/hosts/settings_dialog.rs`, with `crates/farhelm-ui/src/modal_isolation.rs` making the rest of
  the page inert. Follow its focus handling (initial focus, Escape and close, focus returned to the gear). Copy only
  that focus skeleton: its busy state, per-field outcome lines and refocus-after-write exist because host writes have
  visible outcomes under the operation lock, and preference writes have neither. The removal dialog the host dialogs
  plan added (`HostRemoveDialog`) is the closer shape, and that plan factored the isolation script into a helper in
  `hosts/settings_dialog.rs` (`install_dialog_with_selector` in #1502); reuse it, making it `pub(crate)` if needed,
  rather than copying the script. Give the new dialog its own class for selectors and scripts: `HostRemoveDialog` reuses
  `host-settings-dialog`, which the existing isolation selector and e2e locators match. It may share the backdrop and
  card styling.
- SPEC_impl.md, GUI: Dioxus, the button tiers paragraph; the gear is an icon button like the app's other icon-only
  controls, with an accessible name ("settings"). Icons are drawn in `crates/farhelm-ui/src/icons.rs` style; read
  `docs/harness-marks.md` only if you touch harness marks (you should not).
- The app bar's CSS (`.app-bar`, `.app-version`, `.window-drag-region` in `crates/farhelm-ui/assets/app.css`): the
  version ellipsizes before the wordmark shrinks, and on macOS the empty drag region is the only draggable surface. The
  gear must stay clickable (outside the drag region), must not shrink away, and must keep the bar one row high,
  including the narrow-window fixed band and the build-skew layout.
- Root `AGENTS.md`: a `feat` PR carries a changelog fragment; a PR that addresses a TODO.md entry removes it;
  `website/AGENTS.md` and `website/EDITORIAL_RULES.md` govern docs pages.

## Implementation outline

One PR (`feat:`). Line numbers drift; find the code by name.

- `AppBar`: add the gear after the version span, and keep the dialog's open state local to `AppBar`. Render the dialog
  as a sibling after the bar's `div` (a fragment), never inside `.app-bar`: the bar is `position: sticky` (or `fixed` in
  the narrow macOS layout) with `z-index: 5`, so a backdrop inside it would be capped below main-pane layers such as the
  header confirmation and the row-menu flyouts. `HostsPanel` rendering its dialogs outside any z-indexed element is the
  precedent.
- Other floating surfaces: row menus and the profiles popup already close on an outside pointer press or focus-out, and
  the open modal makes everything behind it inert, so the dialog should not need to join the list page's
  one-surface-at-a-time effects (which would mean passing state into `AppBar`, which takes no props today). Add a
  browser test that clicking the gear with a row menu open leaves no menu visible; wire into those effects only if it
  fails, and log the DECISION.
- A new `SettingsDialog` component, placed where it can reuse the dialog isolation helper (log the DECISION). Two
  labelled checkboxes, help lines that change with the state (draft them in the YOLO switch's voice, for example
  "Farhelm shows what setup will change on a new host and asks before doing it." / "Farhelm sets up a new host as soon
  as it has checked it, without asking."), and a close button. Each checkbox writes its preference on change and updates
  `SharedPreferences` at once, so the next add or remove in this client follows it. Another open client picks the change
  up when it next reads preferences, as with every other preference; no live sync.
- The two host dialogs (`HostRemoveDialog` and the setup confirmation, wherever the host dialogs plan put them): add the
  pointer-back line under the permanent answer, muted and short.
- Specs and docs: SPEC.md where the host dialogs plan describes the two permanent answers (say they can be turned back
  on in the app's settings dialog) and wherever the Session list text describes the shared preference row; SPEC_impl.md
  GUI section (the app bar's contents, the settings dialog, and that it holds only app-wide choices with no natural
  place elsewhere in the UI, per U3); `website/src/content/docs/docs/using/manage-hosts.md` and
  `website/src/content/docs/docs/get-started/add-a-remote-host.md`, wherever they describe the two permanent answers.
  The host dialogs plan's SPEC.md Session list text says a client that already loaded its preferences "keeps asking
  until it reloads"; that was only true while the answer went one way. Generalize it (a client keeps its previous
  behavior until it reloads), and do not let the dialog's intro line imply that other open clients see a change at once.
- Tests: Playwright (`e2e/tests/`, alongside the host dialog specs the host dialogs plan landed): the gear opens the
  dialog and focus lands per the template; clicking the gear with a row menu open leaves no menu visible; after "remove,
  and don't ask again", the settings dialog shows "remove hosts without asking" ticked, and unticking it brings the
  remove dialog back on the next removal; the same round trip for setup if the provisioning spec's fixtures make it
  cheap, otherwise one direction plus a unit test; Escape closes and focus returns to the gear; the gear stays visible
  and clickable with a long version stamp. Unit tests for any pure helper you add.
- Changelog fragment `kind: added`. Remove the TODO entry "A way to turn host add and remove confirmations back on".

### What not to build

- No other settings in the dialog (U3), no settings page or route, no per-host variant, no live preference feed.
- No change to the preference storage, its route, or the host dialogs' behavior beyond the pointer-back line.
- No native menu bar item.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-confirmations-toggle-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
  `plan/host-confirmations-toggle/01-settings-dialog`.
- One commit, bookmark and draft PR. If it needs correcting within this run, restructure it rather than stacking a
  correction on top.
- Conventional Commits; the PR is `feat:`. It adds its changelog fragment under `releasing/changelog.d/` in the same
  commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run the commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  the PR description empty when the diff and title say everything.
- The PR stays a draft. Never mark it ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, `dprint check` on
  changed Markdown, the farhelm-ui unit tests for the touched modules.
- Browser: run the new and affected Playwright specs (the host dialog specs, and `e2e/tests/window-chrome.spec.ts` and
  `e2e/tests/sidebar.spec.ts`, which assert the app bar's geometry; `forceBuildSkew` there makes a long version stamp
  cheap) on Chromium and WebKit through the recorder, building per root `AGENTS.md` first. Not the full suite unless a
  specific risk needs it.
- The website build (`cd website && bun install --frozen-lockfile && bun run build`) since a docs page changes.
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`, since
  the PR changes browser tests.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot reach that
model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: The goal, U1-U6 and P1-P3. Include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires for test changes. Address what the reviewer finds
before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or in the log;
the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a settings page or route, a generic settings
registry, a new preference mechanism, live preference sync, moving existing controls into the dialog; these are
examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the request, the user decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the dialog's title and intro wording, the checkbox labels and help lines, the
pointer-back wording, where the component lives, how the gear fits the app bar's narrow and macOS layouts, and every
reviewer finding you declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10). If
what the host dialogs plan landed differs from this file's description in a way that changes the user-visible result
(for example the permanent answers are no longer helm preferences), block rather than adapting the goal.

## Done criterion

The plan is complete when its one draft PR exists, satisfies the goal and its acceptance criteria, has passed the review
gate, and has removed its TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this
plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
