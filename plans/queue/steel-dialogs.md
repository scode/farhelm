# UI: Steel dialogs: settings, the new-session launcher, confirmations and menus

Written against main at a48bf50d on 2026-10-25. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where the design section below records a change the maintainer approved; that change is yours to make in
the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Visual redesign: Steel.** Restyle the app as the "Steel" design the maintainer settled on 2026-10-25 from mockups:
>   blue-gray and periwinkle colors with a light counterpart, Inter for the interface, a host rail with a hosts panel,
>   the sort order as the list title over a status filter, split Restart and Replace buttons, a tunnel pill, one dialog
>   and menu anatomy, and sentence case. The reference mockups are `docs/design/steel/mockups.html`. Plans:
>   `plans/queue/steel-foundation.md`, `plans/queue/steel-sidebar.md`, `plans/queue/steel-session-chrome.md`,
>   `plans/queue/steel-dialogs.md`, `plans/queue/steel-sentence-case.md`.

This is one of five plans for that entry: the dialogs and menus. It runs after `steel-foundation.md` has landed (and
after `report-failed-settings-saves.md`, `template-panel-escape.md` and `restart-with-template-refusal.md`, which change
settings and the launcher), which supplies the palettes, fonts, the shared switch, segmented control, menu anatomy,
dialog anatomy, every icon this plan needs, and the visual-review capture. Use those; do not add parallel versions. The
maintainer asked for the dialogs to "feel consistent": this plan brings each one to the shared anatomy.

## The Steel design (shared by all five Steel plans)

The maintainer redesigned Farhelm's look over several rounds of mockups on 2026-10-25 and then asked for it to be
planned for executors. This section is the same in all five plans: `steel-foundation.md`, `steel-sidebar.md`,
`steel-session-chrome.md`, `steel-dialogs.md` and `steel-sentence-case.md`. Each plan builds its own part, listed under
"This plan's part" below.

The maintainer's words, condensed: "change the visual style of farhelm. it still feels like it's probably not appealing
to most people. but i still want it to be _distinctive_, not just generic typical AI slop default"; "we want to keep
inter and jetbrains mono nerd font wise"; then, picking from mockups: "lets settle on steel (and the light version for
light)", "lets do solid selection adjust for steel theme", "i like sort as list title", "no state as words - takes too
much room", "definitely NOT moving the host/session name back up, i moved it down for a functional reason - i want it at
the bottom", a host rail "as long as we have an 'all hosts' option", icons on actions but "tight vertical yet
icons+text", "fewer buttons ... assuming they pop up a menu to choose what kind of restart/replace", and tunnels "more
visually obvious than today and if we can use space better so it doesn't add a row".

**The visual reference** is `docs/design/steel/mockups.html`. Open it in a browser from a checkout (it loads the fonts
the repository vendors by relative path). It shows the main window dark and light, the Restart menu, the tunnel list,
the hosts panel with a host menu, settings dark and light, the new-session launcher, and a confirmation. They are
mockups, not captures: sizes and spacing are approximate, the session and host names are invented, and wherever a
picture and the words in this section or SPEC.md disagree, the words win. The mockups' terminal is darker than today's;
see "Terminal" below.

### Colors

- **Steel** (dark, the default): blue-gray surfaces at three levels, about `#181c24` (ground), `#1d222c` (chrome:
  sidebar, header, bars) and `#282e3a` (raised: controls, menus); dialogs about `#1f2430`; body text about `#e2e6f0`,
  secondary about `#a2aabb`, quiet about `#747d90`; hairlines a few percent of white. The accent is periwinkle
  `#8ea2f5`. Status colors keep their roles at moderate saturation: running green about `#7fcf9a`, attention amber about
  `#e5bb6e`, waiting/danger red about `#ee8a8a`, new output in the accent family.
- **Steel Light**: about `#f4f6fa` (ground), `#e9edf4` (chrome), white raised controls with a faint outline; text about
  `#1b2130` / `#4f5a70` / `#7f8aa0`; accent `#4a63d8`; status colors darkened enough for white grounds. Host colors,
  which are pastels chosen for dark grounds, need light-ground variants too.
- **Moderate contrast is a requirement**, not a style: the maintainer asked for nothing that strains the eye. No pure
  white text on near-black, no neon. The existing WCAG AA floor for quiet text (SPEC_impl.md, GUI styling) holds in both
  palettes.
- **Selection and primary actions share one solid fill**: a deeper periwinkle (`#5268d0` in Steel, `#4a63d8` in Steel
  Light) with white text. It marks the selected session row, the selected rail entry, the open settings section, the
  highlighted menu item, the chosen segment, and primary buttons. Status shapes stay visible on a selected row.
- **Terminal**: the terminal's own background and ANSI palette stay as they are today (SPEC_impl.md, `--terminal-bg`
  mirrors xterm.js's theme). In Steel Light the terminal stays dark. Only the frame around it follows Steel. The
  mockups' darker terminal is approximate and not a requirement.

### Light mode

Steel Light applies when the system is in light mode, and a per-device choice in Settings (System, Dark, Light; System
the default) overrides it. It is stored per device in the browser, the way sound choices and the sidebar width are, and
never in the helm. It lives in a new **appearance** section of Settings.

### Fonts

Inter for the interface: labels, buttons, titles, menu items, session and host names. JetBrains Mono Nerd Font stays for
the terminal, paths, versions, commands, ages and the small Nerd Font glyphs. Inter is vendored in the UI crate the way
the mono font already is (the docs website already vendors Inter WOFF2 files under `website/public/fonts/`), with its
license and a `THIRD_PARTY_NOTICES.md` entry; no CDN.

### Controls

- **Buttons**: the existing tiers, restyled: primary (the solid selection fill), neutral (raised gray), ghost (text
  only) and danger (solid red); a permanent "…, don't ask again" answer keeps today's outlined form of its tier.
- **Switches** for on/off settings (today's host-settings switch, generalized; still native checkboxes underneath, as
  SPEC_impl.md says). **Segmented controls** for two to five exclusive choices. **Dropdowns** for longer lists. Controls
  whose native behavior the spec relies on keep it: the launcher's YOLO choice and Resume checkbox launch on Enter
  (SPEC.md around 680), so they stay native radios and a checkbox, restyled only.
- **Menus**: a header naming the subject (title and a quiet summary line), items with an icon, a label and a smaller
  description line, a separator before destructive items, which are red; the highlighted item uses the selection fill.
- **Dialogs**: a header with an optional icon tile, the title, a one-line summary and a close control; a body; a footer
  with an optional hint on the left and buttons on the right, Cancel first, a "don't ask again" variant next, the main
  action last.
- **Notices** stay inline (a tinted band with an icon, one line, an action, a dismiss); no toasts.

### Sidebar

- **Host rail** at the far left: **All** first (the default, selected), then one avatar per host showing the host's
  chosen icon in its chosen color (the existing host mark), then add host. Each avatar carries the host's state: a
  connection dot, an update-available badge, a ring while setup, update or uninstall runs, dimmed while unreachable;
  hovering names the host and its phase in words. Clicking an avatar narrows the session list to that host; All widens
  it again. The rail replaces today's host selector. At the rail's foot: the hosts-panel toggle, settings and help.
- **Hosts panel**: the hosts-panel toggle swaps the session list for a panel holding everything today's host list has:
  add host, update all, the details switch, each host's status words, the inline update button, setup/update/uninstall
  progress, and each host's menu (the rail avatar's context menu opens the same menu).
- **Header**: the sort order is the list's title ("Recently active ▾") with the session count beside it ("48", or "12 of
  48" when narrowed); the compact choice moves into the sort menu, its helm-wide preference and default unchanged. Trash
  (with its count), templates and new become compact icon buttons beside the title; new stays the sidebar's one filled
  control.
- **Status filter**: below the header, a segmented "slide": All, Running, Waiting, Idle, without counts. It sets the
  existing single-status list filter (`SessionFilter.status` in `crates/farhelm-ui/src/api.rs`, which no UI sets today);
  ended sessions appear only under All. Status sounds follow it as they follow the host choice (SPEC.md, Status: "follow
  the sidebar's current host selector and filters"; the maintainer kept that).
- **Rows**: today's rows, in Inter, with today's status shapes and marks; compact and non-compact modes unchanged.
- **macOS window**: the native traffic lights stay in the window's top-left title band; the rail starts below them. The
  title band keeps its drag and zoom behavior (SPEC.md, "Desktop window chrome").

### Session view

- **Toolbar**: tabs on the left. On the right, buttons with an icon and a label, at today's button height: text size (A−
  A+), Restart ▾ and Replace ▾ as split buttons, Clone, Delete. A split button's main part does what Restart or Replace
  does today; its caret opens a menu with that action and "Restart with…" or "Replace with…". Greyed-out conditions and
  their explanations apply to both parts.
- **Tunnels** (desktop app only, per session): a pill beside the tabs, in a color of its own, showing the tunnel's
  mapping when there is one and the count when there are more; clicking it opens a list with a close button per tunnel
  and Close all. No extra row; today's strip goes away. Errors still show.
- **Bottom bar**: unchanged, and it stays at the bottom.

### Text

Sentence case for every label, button, title, heading and menu item ("Restart with…", "Add host", "Recently active"),
replacing today's all-lowercase. Session names, host names, paths, commands, protocol words and anything a user typed
keep their own case. Frozen material is never touched (root `AGENTS.md`, "Frozen material"): published release notes and
CHANGELOG sections, release shots, the docs Overview page, `lore/`.

### Spec changes the maintainer approved

These rewrite deliberate passages; each plan updates the passages its part touches, in the same PR as the change:

- SPEC_impl.md's font paragraph (the interface deliberately uses the terminal's mono face): now Inter for the interface.
- SPEC_impl.md's selection construct (a quiet tint and an accent bar so that status colors stay the loudest thing in the
  sidebar): now the solid fill, with status shapes carrying status on a selected row.
- SPEC.md's "Per-host connection state is always visible in the host list, which names each host and pins its current
  phase" (Session list, and its copy under the requirements list): now the rail always shows each host's state, with the
  name and phase on hover and in the hosts panel.
- SPEC.md's "all six actions remain fully visible ... from a 650px main pane" and its SPEC_impl.md copy: now text size,
  Restart ▾, Replace ▾, Clone, Delete and the tunnel pill fit at 650px, with the "with" variants in the split menus.
- SPEC.md's tunnel strip under the top row and its SPEC_impl.md copy: now the pill and its list.

Changes that follow from those and from the design above, decided while planning (they amend the spec the same way): the
always-visible host selector becomes the rail; the gear moves from beside the version to the rail's foot; the sidebar's
default, minimum and maximum widths each grow by the rail's width, so the session list keeps today's width; the quick
switcher, which resets a host choice that hides the chosen session, also resets the status filter to All; the new
appearance setting joins SPEC.md's list of per-device settings; the hosts panel's details checkbox becomes a switch.

### Not in these plans

Refreshing the docs screenshots, the README hero and the demo video (separate requests once everything has landed);
per-status counts on the filter (would need the helm to report them); a tunnel mark on session rows (the helm keeps
tunnels per open session only, so every row would need new data).

## This plan's part

1. **Settings** (`crates/farhelm-ui/src/settings.rs` and the section files it uses): an icon beside each section in the
   section list (sessions, hosts, terminal, checkouts, agent trust, notifications, app, and the appearance section
   `steel-foundation.md` added), the open section in the selection fill, switches for every on/off setting, the dialog
   anatomy (header with the gear tile, title and summary; footer hint and Close). Fields, saving and behavior unchanged,
   including the narrow-window back button.
2. **The new-session launcher** (`crates/farhelm-ui/src/list/create_form.rs`, `launch_controls.rs`): the same fields and
   behavior, regrouped into labeled rows as in the reference mockup (search; destination with folder or managed
   checkout, host, folder or repository, name; the agent and command tabs with their fields), and its buttons (the
   launch, replace or restart button, Cancel, Reset choices, Save as template) moved from the top into a pinned footer
   whose left side keeps today's one-line summary of what will launch where. Restart and replace modes, errors,
   warnings, banners, recent setups and the save-as-template panel keep working. The YOLO choice and the Resume checkbox
   stay native controls that launch on Enter (SPEC.md around 680).
3. **Confirmations and other dialogs**, each to the shared dialog anatomy and button order (Cancel first, the "don't ask
   again" variant outlined, the main action last): rename (`rename.rs`), add host and its setup plan (`hosts.rs`,
   `provisioning.rs`), host settings (`hosts/settings_dialog.rs`), remove host, uninstall, the refusal dialog, the
   tunnel consent dialog (`tunnels.rs`), templates (`list/templates.rs`), and the trash dialog, plus the YOLO
   confirmation (`yolo_confirm.rs`), which stays inline but takes the same button kinds and order.
4. **Menus**: every remaining menu that does not use the shared menu anatomy yet (the session row's menu and its inline
   delete and replace confirmations in `list/row.rs`, the help menu, any other) brought to it, with destructive items
   red after a separator.
5. **Spec**: SPEC.md where it describes a dialog's layout (the launcher's action row moving to a footer, the settings
   section list) and SPEC_impl.md's dialog and menu paragraphs, amended to what was built.

Labels this plan adds or changes are written in sentence case already. Behavior does not change anywhere in this plan.

### Acceptance criteria

- Settings, the launcher, every dialog listed above and every menu match the shared anatomy and the reference mockups in
  Steel and Steel Light.
- The launcher launches, restarts and replaces exactly as before; its buttons stay visible however long the form gets;
  Enter on the YOLO choice and Resume checkbox still launches.
- Keyboard behavior is unchanged: focus order, Escape, focus return, and the modal rules SPEC.md describes.
- Browser specs for settings, the launcher, templates, hosts' dialogs and menus updated for moved elements, on both
  engines; no test weakened to pass.
- SPEC.md and SPEC_impl.md describe what was built.
- A changelog fragment (`kind: changed`) for the consistent dialogs and the launcher's footer.

## Decisions already made

Settled with the maintainer on 2026-10-25, on mockups and planning pages:

- Everything in the design section above, including the approved spec changes.
- **The launcher: labeled rows and a pinned footer**, as mocked, over moving only the buttons.
- **Switches for on/off settings, segmented controls for two to five choices, dropdowns for longer lists.**
- **Code reviewer:** gpt-6.1-sol at high effort. **Visual reviewer:** Opus 5.5 at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words quoted in the design section ("we need to make them
feel consistent"), and the decisions above.

**Binding repository constraints:** SPEC.md's launcher, settings, templates, hosts and confirmation passages (behavior
unchanged); SPEC_impl.md's GUI styling, dialog and modal paragraphs; root `AGENTS.md`, "Code rules", "Tests and
validation", "Conventional Commits", "Changelog fragments"; `.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the section icons; the launcher's
row grouping and label column; the PR boundaries below.

## Outline

A linear stack, roughly:

1. `plan/steel-dialogs/01-settings`: section icons, switches, anatomy.
2. `plan/steel-dialogs/02-launcher`: labeled rows and the pinned footer; the spec.
3. `plan/steel-dialogs/03-confirmations`: the listed dialogs and the YOLO confirmation to the shared anatomy.
4. `plan/steel-dialogs/04-menus`: the remaining menus; the changelog fragment.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on changed files, clippy and the
UI crate's tests, and through `python3 scripts/record-test-run.py` the browser specs for settings, the launcher,
templates, hosts, rename, YOLO confirmation, trash and tunnels' consent, on both engines. Run
`python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Examples of departures that need the scope reassessment: changing any field, default or behavior; splitting the launcher
into steps; adding toasts; restyling the sidebar or the session toolbar (other plans own those).

Decisions to log: the launcher's grouping, each dialog whose structure had to change to fit the anatomy, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-steel-dialogs-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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

The two review gates keep the models and efforts this file demands; the scope reassessment runs at your own model and
effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and
the cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

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

On every path, check heartbeat freshness before each new build, test run, capture, sub-agent or review launch. A dead
monitor, or a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead
watchdog.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/steel-dialogs/<nn>-<short-name>`, as the
  outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). One fragment for this plan's visible change is enough; later PRs of the plan edit it rather than adding
  more.
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
findings file in the scratch directory, and the acceptance criteria (this file's design section, this plan's part, its
acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Visual review gate

Every PR that changes anything a person can see also gets a visual review before it is finished, in addition to the code
review gate. The maintainer asked for "a special opus reviewer whose job is to review the visual look through
multi-modal inspection and make sure it looks correct (using similar process to the 'appshots' we use for
documentation)". The maintainer demands exactly this reviewer: a single fresh-context agent on Opus 5.5
(`claude-opus-5-5`) at high effort, able to read images. If it cannot be reached when a review is due and is still
unreachable after a reasonable retry, block the plan as for the code review gate; never substitute another reviewer.

**Captures.** Take them from the PR's head with the visual-review shot spec that `steel-foundation.md` adds,
`e2e/docs-shots/visual-review.spec.ts`, run through `scripts/docs-screenshots.sh --only visual-review`: the docs
screenshots' staged fleet with a real helm, real supervisors and real terminals (`docs/docs-shots/SPEC.md`). The PNGs
land in the gitignored `website/public/docs-shots-local/` and are never published or committed. Extend the spec with
shots of every screen and state the PR changes (an open menu, a dialog, a narrowed list, a host mid-update), each in
Steel and Steel Light (Playwright's `emulateMedia({ colorScheme })`, or the appearance setting once it exists), at a
wide window and, where the layout depends on width, with a 650px main pane. Extending the spec is part of the PR that
needs the shots. Before `steel-foundation.md`'s first PR exists, that PR's own captures come from the spec it adds.

Two things the web capture cannot show: the tunnel pill, which is built only into the desktop app, and the macOS title
band. For those, capture the closest the web build allows (force the desktop shell's layout the way
`e2e/tests/window-chrome.spec.ts` does, and a test-only way to show the pill if one fits without touching product
behavior), and give the reviewer the CSS and markup as well; say in the report which parts it judged from code alone.

When the capture prerequisites (`docs/docs-shots/SPEC.md`, "Prerequisites") are missing and cannot be set up within the
checkout and the scratch directory, block the plan with that as the question; never skip the gate and never change ssh
configuration or anything else outside the checkout and scratch directory to make it run.

**The prompt** names the PNG files (with what each one shows), the reference `docs/design/steel/mockups.html` (rendered
to a PNG with the e2e suite's Playwright Chromium, so the reviewer can look at it), this file's design section and this
plan's acceptance criteria, and a findings file. It carries this charter:

> Inspect every image as a person using Farhelm would see it. Report, most severe first: anything broken (overlapping or
> clipped text, misaligned or wrapped controls, content cut off at the shown width, focus or selection that is hard to
> see), anything hard to read (low contrast, text that strains the eye, a status that is hard to tell apart), anything
> inconsistent (a control, menu, dialog or button that does not follow the shared design, the wrong font for its role, a
> color used for the wrong role, a light-mode screen that differs from its dark counterpart other than in color), and
> every departure from the written design and the reference mockup that a person would notice. The mockup's sizes,
> spacing and invented names are approximate; the written design wins where they differ. For each finding give the
> image, where in it, what is wrong, and what it should look like. Say plainly when everything looks right. Edit nothing
> and touch no VCS state.

Fix what it finds, capture again, and repeat the review on the new images until it reports nothing you have not decided
against; log each finding you decide not to follow as a DECISION with the reason.

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
alternatives considered: in particular those named under Fallbacks above, every value you settled that the design gives
only approximately, and every review finding you decided not to follow. The user will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, and each
PR passed its code review gate and, where it changes anything visible, its visual review gate. This plan does not remove
the TODO.md entry; `steel-sentence-case.md` does, last. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Decisions

### 2026-10-28: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-10-28 (claim c0c30e).
>
> ### Dialog layout and keyboard order
>
> This plan gives the session launcher and other dialogs a shared header, scrolling body and pinned action footer, with
> Cancel first and the main action last. It also promises that keyboard focus order stays unchanged. Those requirements
> conflict. Today the launcher's Launch, Cancel, Reset choices and Save as template buttons precede its fields; Rename
> has Save before Cancel. Moving those actions into the approved order and adding a header Close changes the sequence
> reached by Tab. The existing keyboard tests explicitly check those endpoints.
>
> May these dialogs adopt the new visual reading order for Tab: header Close, fields in their existing relative order,
> then footer actions with Cancel first and the main action last? The same permission is needed for Rename, Templates'
> unsaved-edit question and tunnel consent. The inline YOLO question needs the new answer order only; it stays inline
> and gains no modal frame or header Close. Their existing initial focus, Escape behavior, focus return, modal
> isolation, native radio/checkbox keys and Enter-to-launch behavior would remain unchanged. Where YOLO offers a
> separate guarded start, it would remain an action between Cancel and the permanent/one-time YOLO answers.
>
> I recommend approving that order. It uses the existing native controls and focus trap, and keeps visual and keyboard
> reading order together. If the exact old Tab sequence is required, the common action order and added Close control
> need explicit exceptions; visual-only reordering would make keyboard traversal disagree with what is on screen.
>
> ### Trash's pinned actions
>
> Trash currently keeps its title, totals, Delete all and Close in a pinned header while the archive list scrolls. The
> product specification deliberately says there is no footer, and its tests verify that layout. The plan's generic
> footer requirement conflicts with it.
>
> Should Trash retain that header-action exception while adopting the shared title and styling, or move its global
> actions into a pinned footer and amend the product specification and keyboard-order tests? I recommend retaining the
> exception: its totals, progress and global actions remain together, and the archive list keeps its current usable
> height. Either choice preserves the existing deletion, confirmation and busy-state rules.
>
> ### Finished independent work
>
> The independent work is finished in two draft PRs:
> [Settings icons and consistent headings, #2756](https://github.com/scode/farhelm/pull/2756/changes), and
> [command-menu descriptions and destructive grouping, #2757](https://github.com/scode/farhelm/pull/2757/changes). The
> latter also corrects the gear-location hints and menu tooltip overlap. No conflicting structural dialog changes have
> been implemented; the plan is not complete and neither draft has merged.
>
> Both drafts passed fresh Sol 6.1/high code review and Opus 5.5/high visual review in both palettes. Targeted browser
> checks ran on Chromium and WebKit; `playwright test <selected menu, template, hover-help and host cases>` passed ten
> cases (run `be69d882-faa6-48be-8768-83d00719fe75`). `scripts/docs-screenshots.sh --only visual-review --no-build`
> completed staging and dark/light captures at both widths (run `ac476805-6377-412d-af9e-de04dcd7a61a`). Settings'
> native App section was reviewed from source only. The full runtime battery was skipped because these bounded
> presentation changes did not warrant it.

The maintainer's answer:

Answered by the landing monitor on the maintainer's behalf, under the maintainer's standing instruction for the Steel
plans, given in conversation on 2026-10-27, verbatim:

> if "steel" related plans if plans come back again and the main problem is just that a bunch of tests fail just send it
> back to fix. feel free to record near-term todo items on my behalf to resolve any judgement calls like "how does a
> user tell an update failed". i just want actual progress here and agnets keep stopping either by their own judgement
> or because of harness bugs.

Decisions, taking the Blocked question's recommendations:

- Dialog keyboard order: approved. The session launcher, Rename, Templates' unsaved-edit question and tunnel consent
  adopt the new visual reading order for Tab: header Close, the fields in their existing relative order, then footer
  actions with Cancel first and the main action last. Update the keyboard tests to that order. The inline YOLO question
  takes the new answer order only, stays inline and gains no modal frame or header Close; where it offers a separate
  guarded start, that stays an action between Cancel and the permanent/one-time YOLO answers. Initial focus, Escape,
  focus return, modal isolation, native radio/checkbox keys and Enter-to-launch stay as they are.
- Trash: keep its pinned-header exception. Trash adopts the shared title and styling, but its totals, progress, Delete
  all and Close stay together in the pinned header and the archive list keeps its height; there is no footer, as the
  product specification says.

Both are recorded as near-term TODO.md entries so the maintainer can revisit them; a later change of mind is a
follow-up, not a reason to stop this plan.

### 2026-10-28: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-10-28 (claim 13764e).
>
> ### Landing stopped: the Steel dialogs break part of the browser test suite
>
> The landing monitor did not merge this plan. None of its four PRs (#2756, #2757, #2761, #2762) has merged, and main
> holds nothing of it. The plan restyles Settings, menus, confirmations and the session launcher for Steel, with the Tab
> order and Trash layout decided in the previous answer.
>
> Its executor ran selected browser tests only. At landing, the whole browser suite (`npx playwright test`, Chromium and
> WebKit, no retries) ran against this plan on current main, with nothing else landing, in run
> `3092e14a-46a0-403f-b084-4a1eb0571c87`: 2701 passed, 57 failed and 14 skipped. One of the UI's own stylesheet tests
> (`node --test` in `crates/farhelm-ui/js-tests`, run `bd6be3c2-d9a5-4e60-b3b0-e204db02298d`) also failed: "solid
> selection uses inverse roles at actual owners, metadata, marks and focus". The source checks, builds, the UI's Rust
> tests and the desktop asset check passed. In product terms:
>
> - 34 failures are launcher and YOLO-question tests that time out on both engines: the YOLO question's answers,
>   remembered permissions, OpenCode, Goose and Pi launcher options, a template's Guarded request, unchanged destination
>   choices, a guarded checkout answer and launching a recent setup with Enter. The tests do find their controls by
>   name, but those controls never become visible, enabled and steady enough to click. That points at the launcher's new
>   layout, where only its body scrolls, rather than at old names; whether users would hit it is not established.
> - Some fail on names or text the plan changed and tests outside its diff still use, as a fresh-context review
>   predicted: the Trash dialog is now named "Trash" rather than "trash" (managed-checkout Trash tests), the Launch
>   button no longer carries the target in its text (checkout composer tests), the destination block shows "Browse"
>   where a test expects "local (this machine)", and the Settings heading's color changed (appearance test). A docs
>   screenshot spec has the same old Launch text and would fail at the next docs capture.
> - The rest: Replace with's tests look for a warning element that the restyled launcher no longer has, the hover-text
>   sweep finds a launcher control with the browser's native tooltip, two host-settings switches fail on WebKit, the
>   named-conversation help's layout check fails, and one file-drop test fails on Chromium. That last one has no clear
>   link to dialogs and may not be the plan's; no run without the plan was made.
>
> The review found that the plan otherwise follows the previous answer exactly (Tab order, the inline YOLO question,
> Trash's header), and that SPEC_impl.md still quotes the permanent YOLO answer in lowercase and does not list
> number-key options among the hover-text exemptions. It also noted that a launcher test no longer guarantees the
> launcher fits a 1440×900 window without scrolling; the pinned footer now keeps Launch reachable instead.
>
> ### What should happen to this plan?
>
> - **Send it back to its executor (recommended).** Answer with: run the whole browser suite on both engines and the
>   UI's JavaScript tests against the stack; fix the product or the test for each failure, saying in the report which;
>   update the docs screenshot spec and the two SPEC_impl.md phrases.
> - **Land it now and fix the failures afterwards.** Main would carry about 28 failing browser tests per engine and a
>   failing stylesheet test until a follow-up lands.

The maintainer's answer:

Answered by the landing monitor on the maintainer's behalf under `plans/.agents/landing.md`, "Answering blocks": a
landing that failed mainly because tests fail goes back with an instruction to fix them.

Decision: send it back to its executor.

- Run the whole browser suite on both engines (`npx playwright test`) and the UI's JavaScript tests against the stack.
- For each failure in the Blocked question, find whether the product or the test is wrong and fix that one, and say in
  the report which it was. Start with the 34 launcher and YOLO-question tests that time out because their controls never
  become clickable: if the restyled launcher's scrolling body makes real controls hard or impossible to reach, that is a
  product fix, not a test update.
- Update the docs screenshot spec that still expects the old Launch button text, and the two SPEC_impl.md phrases (the
  permanent YOLO answer's wording and the hover-text exemption for number-key options).
- The decisions already recorded for this plan (the dialogs' Tab order, the inline YOLO question, Trash's pinned header)
  stand.

No judgment call is made here, so no TODO entry is recorded.
