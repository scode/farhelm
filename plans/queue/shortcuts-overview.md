# UI: a keyboard shortcuts dialog on Cmd+/ and in the ? menu

Written against main at 462b0268 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Show all keybindings with command-/.** Add a keybinding that shows every keyboard shortcut at a glance, so users
>   can discover them instead of having to remember them.

### What changes

A new **Keyboard shortcuts** dialog lists Farhelm's keyboard shortcuts, grouped by area, with the chords this client
actually uses. It opens two ways:

- **The chord:** Cmd+/ on macOS, Ctrl+Shift+/ everywhere else, matched by the physical Slash key (`code === "Slash"`),
  so layouts where Shift+/ types `?` still work. The macOS/elsewhere split follows the quick switcher's Cmd+K /
  Ctrl+Shift+K rule (SPEC.md, "Desktop window chrome"). Ctrl+/ alone was rejected because terminal programs receive it
  as Ctrl+_ (undo in shells and editors).
- **The rail's ? menu:** a new **Keyboard shortcuts** item, so the dialog is reachable where a browser keeps the chord
  for itself.

The dialog lists:

- every app-wide shortcut (New in the Mac desktop app, Delete, Rename, quick switcher, template list, quick launch ⚡,
  template number keys, quick-launch sequence prefix, terminal text size, and this dialog's own chord);
- the user's current ⚡ template and number-key templates by name, read fresh each time the dialog opens;
- the keys inside a quick-launch sequence (number keys, the ⚡ chord, Enter, Escape).

A new **Keyboard shortcuts** docs page lists the same shortcuts for both the Mac desktop app and other clients.

### What does not change

Every existing shortcut, its matcher, its modal and terminal rules, and its tooltip hints. No shortcut is added or
rebound other than the dialog's own chord. Keys inside dialogs (arrows, Enter, Escape in lists) are not listed; the
maintainer left them out.

### Acceptance criteria

- Cmd+/ on macOS (desktop app and browser) and Ctrl+Shift+/ elsewhere open the dialog from anywhere in the signed-in
  app, including a focused terminal. The chord never reaches the terminal's program, ignores key repeat and composition,
  and does nothing while another modal is open or before sign-in and preferences loading finish, like Cmd+K.
- The ? menu has a **Keyboard shortcuts** item that opens the same dialog; SPEC.md's item list and order for that menu
  are updated.
- The dialog uses the app's ordinary modal frame (`ModalDialog`, its header with Close, focus isolation): Escape, Close
  or a click outside closes it and restores focus to where it was (the terminal, or the ? button when opened from the
  menu).
- It shows one form per shortcut: the form this client actually uses. Two different rules decide that today, and the
  dialog follows each listener's own rule. Chords that follow the operating system (quick switcher, template list, ⚡,
  text size, and this dialog) use Cmd on macOS, in the desktop app and in Mac browsers alike, and Ctrl+Shift elsewhere.
  Chords that follow the build (New, Delete, Rename, number keys, sequence prefix) use Cmd only in the Mac desktop app.
  New appears only where Cmd+N is handled. The maintainer saw this one-form rule stated on the planning page.
- The ⚡ row names the marked template, or says none is marked; each assigned number key names its template; unassigned
  keys are not listed individually. These come from the shared preferences, refreshed once per open the way the template
  list (Cmd+J) refreshes its labels; a failed refresh keeps the cached names, as Cmd+J does.
- Labels reuse the constants and helpers the tooltips already use where those give the one form above
  (`SESSION_DELETE_SHORTCUT`, `SESSION_RENAME_SHORTCUT`, `TEMPLATE_SHORTCUTS` for the build-following chords). Today's
  label helper renders OS-following chords as both forms ("⌘P / Ctrl+Shift+P"), so the dialog needs a small one-form
  variant fed by one platform read at open using the same `/Mac/` test the listeners use (`feedback.rs`'s platform eval
  is a precedent). A shared Rust constant for a label that a JS listener matches is still a second copy of that chord;
  the browser tests, not the constant, keep the two in step.
- If the `typed-quick-launch` plan (typed template names inside a quick-launch sequence) is already on main when you
  run, the sequence section also lists typing names, Space, Tab and Backspace as that feature defines them.
- A new docs page, `website/src/content/docs/docs/using/keyboard-shortcuts.mdx` (or the name the docs site's conventions
  call for), lists every shortcut for both the Mac desktop app and other clients, and the existing pages that describe
  individual shortcuts link to it where it fits naturally.
- Browser tests cover opening by chord on both platform forms, opening from the ? menu, focus restore, the chord not
  reaching the terminal, the chord ignored under another modal, and the live template bindings. `feedback.spec.ts`,
  which counts and orders the ? menu's items, is updated for the new item.
- SPEC.md describes the dialog and its chord in "Desktop window chrome" and the menu item in "Feedback"; SPEC_impl.md
  records where the listener lives and how it fits the listener order in `AppBody`.
- A changelog fragment (`kind: added`, for a `feat` PR).
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-29 on a planning page with mockups:

- **Chord:** Cmd+/ on Mac, Ctrl+Shift+/ elsewhere, plus a **Keyboard shortcuts** item in the ? menu.
- **Content:** every app-wide shortcut, the current ⚡ and number-key templates by name, the keys inside a quick-launch
  sequence, and a docs page. Keys inside dialogs were offered and not chosen.
- **Design A, a dialog grouped by area**, chosen over a non-modal cheat-sheet strip over the terminal. The mockup showed
  a titled dialog with small uppercase group headings (Sessions, Launching, Terminal) and two columns of rows, each an
  action name on the left and its keys as `kbd` chips on the right; template rows read like "⚡ myproject-codex ⌘P" and
  "1 · myproject ⌘1". Match the app's existing dialog and chip styling rather than the mockup's exact look.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md, "Desktop window chrome" (one paragraph per shortcut, the modal rule, the
browser-reserved-chord caveat), "Feedback" (the ? menu's items in order), and "Creation" (quick launch, number keys,
sequences); SPEC_impl.md's account of the capture-phase listeners; the listener ordering in `AppBody`
(`crates/farhelm-ui/src/lib.rs`, the composition guard first, every listener before xterm); the modal convention
(`role="dialog"` with `aria-modal="true"` keeps the keyboard); root `AGENTS.md`, "Code rules", "Tests and validation",
"Conventional Commits", "Changelog fragments"; `website/AGENTS.md` for the docs page; `.agents/test-authoring.md`;
`.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code: the quick switcher (`crates/farhelm-ui/src/list/quick_switcher.rs` and its inline
listener in `install_quick_switcher_shortcut` in `lib.rs`) as the model for the chord listener, opening by hidden relay
and restoring the opener's focus; its `kbd` chip styling (`.quick-switcher-template-keys kbd` in `app.css`) for chord
chips, with labels from `TEMPLATE_SHORTCUTS.bolt` and `TEMPLATE_SHORTCUTS.number_chord`; its binding refresh
(`fetch_preferences`, `template_bindings_after_read`, `apply_template_bindings`, widening visibility if needed) for the
template rows; one Rust table in a new module describing the dialog's rows, with no refactor of the listeners themselves
into a registry; the menu item placed directly after **Documentation**; two PRs (the dialog with its chord, menu item,
spec and tests; then the docs page) or one, whichever reviews better.

## Outline

1. **Chord.** Add a capture-phase listener for the dialog's chord next to the others installed in `AppBody`, following
   the quick switcher's inline one (or extending it to match both `KeyK` and `Slash` with different relays): physical
   `Slash`, Cmd on macOS and Ctrl+Shift elsewhere, no other modifiers, ignore repeat and `isComposing`, `preventDefault`
   and stop it reaching xterm, do nothing while a modal is visible or before sign-in and preferences are loaded.
2. **Dialog.** A `KeyboardShortcuts` component in the `ModalDialog` frame with grouped rows and one-form labels as the
   acceptance criteria describe. Where a label exists only inline today (the quick switcher has none in Rust; text size
   is hand-typed in its tooltip), lift it to a shared constant rather than typing it a third time. Template rows come
   from the shared preferences with Cmd+J's once-per-open refresh.
3. **Menu item.** Add **Keyboard shortcuts** to the ? menu in `crates/farhelm-ui/src/app_bar.rs`, opening the same
   dialog with focus returning to the ? button on close.
4. **Spec.** SPEC.md: a "Desktop window chrome" paragraph for the dialog (chord, modal and terminal rules, what it
   lists, browser-reserved caveat) and the updated "Feedback" menu list. SPEC_impl.md: the listener and the dialog's
   data sources.
5. **Docs.** The new docs page, written per `website/AGENTS.md` for someone running Farhelm, with both chord forms;
   links from `launch-templates.mdx`, `start-a-session.mdx`, `session-list.mdx`, `work-in-a-session.mdx` and
   `stop-restart-resume.mdx` where they describe shortcuts. The `using/` sidebar is autogenerated, so the page needs
   only its own frontmatter order. This is an incidental docs edit in an unattended run: start no preview server and
   name the changed pages in the report.
6. **Tests.** Browser tests in a new `e2e/tests/keyboard-shortcuts.spec.ts` (platform forms by overriding
   `navigator.platform`, as `quick-launch.spec.ts` does). Update `feedback.spec.ts` for the ? menu's new item and count.
   Check `tooltip-coverage.spec.ts` and the modal focus/trap specs still pass with the new menu item and dialog. If the
   matcher ends up in a separate JS asset, a unit test beside the existing JS matcher tests is cheap and fine.
7. **Bookkeeping.** A changelog fragment (`kind: added`); remove the TODO.md entry in the last PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the UI crate's tests, the JS unit tests, `dprint check` on
changed Markdown, the website's own checks for the new page, `python3 releasing/check-changelog.py format`, and, through
`python3 scripts/record-test-run.py`, the new browser spec plus `feedback.spec.ts`, `tooltip-coverage.spec.ts`,
`modal-trap.spec.ts` and `modal-focus.spec.ts` on Chromium and WebKit, and `quick-switcher.spec.ts` if its listener
changed. Run `python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: refactoring the existing listeners into a central registry; changing,
adding or removing any shortcut other than the dialog's chord; making the dialog editable (rebinding keys from it);
listing keys inside dialogs; a non-modal surface instead of the dialog.

Decisions to log: where the menu item sits in the ? menu, where the matcher and the row table live, any label lifted to
a shared constant, how the one-form labels get the platform, the docs page's name and placement, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-shortcuts-overview-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/shortcuts-overview/<nn>-<short-name>`
  (for example `plan/shortcuts-overview/01-dialog` and `plan/shortcuts-overview/02-docs-page`).
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
