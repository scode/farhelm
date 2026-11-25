# Make Welcome the empty main area

Written against main at 210c22183 on 2026-11-25. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, except where this goal explicitly changes them (the spec edits it lists are part of the work).

This plan runs after `launcher-style-quick-dialogs.md` (Cmd+K, Cmd+J and Cmd+L in the launcher's look), which pulls the
launcher's pieces out into shared components under `crates/gui/farhelm-gui/src/components/`. Build on those components;
do not pull the same pieces out a second time.

## The goal

Implement the TODO.md entry under Immediate, **Make Welcome the empty main area.** Today a first launch opens the
Welcome card, a modal (`crates/gui/farhelm-gui/src/welcome.rs`; SPEC.md, "Welcome card"), over an app whose main area
says only "No sessions — create one". The maintainer was shown twenty directions for it and chose direction W7, "Empty
state": "For the welcome flow, lets use "empty state" - i like it. We'll eventually add a video thumbnail with
click-through once I have that set up and I think it composes well with ading (later) some slight coaching indicators."

So the welcome stops being a dialog. With no sessions, the main area itself introduces Farhelm and offers the first
actions, and it goes away by itself once a session exists. The video thumbnail and the coaching indicators are later
work and not part of this plan, but the layout must leave room for them (see "Room for later").

The mockup is published at `https://snippets.scode.org/s/farhelm-whats-new-directions/`, section "W7 Empty state" in
part 2 (the page has a theme switch at the top; part 2's "Today" section shows the current card). The words in this file
are the design; where the mockup and these words differ, the words win. In particular the button reads "Start a
session", not "Start your first session".

### The agreed design, in words

**Where it shows.** In the session pane's main area, exactly where the pane now shows "No sessions — create one":
whenever the helm has shown that there are no sessions at all (`session_list::empty_fleet`, the existing proof). The
other two states of that placeholder stay as they are: "Loading sessions…" before the list has been read, and blank
while there are sessions but none is open yet. The welcome replaces only the "No sessions" line, so the three-honest-
states rule in `session_pane.rs`'s module docs still holds. The pane's empty title band stays.

**What it shows**, as a single centered column about 420 pixels wide that shrinks with narrow windows and never scrolls
sideways:

- A row of agent marks: every agent Farhelm supports, as its harness mark in its mark color, small (about 14 to 16
  pixels) with even gaps. Shells are not agents and are not in it, and neither are the generic custom type or registered
  custom agents. The list is the launcher's harness order, `HARNESS_PICKER_ORDER`
  (`crates/gui/farhelm-gui/src/launcher/words.rs`, compile-checked against the built-in harnesses; make it `pub(crate)`
  or move it beside `harness.rs`), filtered by the exhaustive `LaunchHarness::is_shell`, with names from
  `LaunchHarness::display_name` and marks and inks from `harness::glyph` and its identity color. That keeps the order
  the launcher already decided and its completeness check, and adds no per-harness list or `matches!` in this view (root
  `AGENTS.md`, "Code rules"). Each mark's hover help and accessible name is the agent's display name; the row as a whole
  reads as "Works with" plus the names.
- The title **Welcome to Farhelm**, about 20 pixels, semibold, in the strong ink.
- The introduction, today's words (`welcome.rs`, `INTRODUCTION`), 13 pixels, in the body ink, centered.
- A primary button, **Start a session**, about 36 pixels tall, with the New session shortcut as a key chip where the
  platform has one (the platform's own name for it, through the existing shortcut-name constants: Cmd+N in the Mac app;
  the Linux app binds no New chord, `actions.rs` about lines 212-221, so on Linux there is no chip, and the goldens,
  which render on Linux, show none). It does exactly what the session list's New button does, through the same code:
  drawn disabled while `launcher::entry::new_available` is false (`launcher/entry.rs`, about lines 71-78), and acting
  through `launcher::entry::open_new` followed by `session_list::show_sessions`, the pattern the session pane's other
  ways into the launcher use (`session_list.rs`, about lines 1405-1414).
- Three quiet links in one row under it: **Add a remote host** (opens the Add host dialog through the existing
  `hosts::add::open`, as the host rail's Add host does), **Read Get started ↗** (opens the first-session guide, today's
  `GET_STARTED_URL`, through the app's one opener), and **Send feedback** (opens the feedback dialog through the
  existing `feedback::open`, sending nothing). From the empty state, the clicked link is the opener those dialogs return
  focus to.

**Focus.** With no sessions and nothing mounted, focus rests on **Start a session** instead of the session list's New
button, so Enter starts a session. Tab reaches the three links after it. While Start a session is disabled (a first
launch pending), it behaves as a disabled New does today: the focus coordinator counts an undrawn or unavailable home as
none (`session_pane.rs`, about lines 219-222). Everything else in SPEC.md, "Where focus rests", stays.

**The startup card goes.** No dialog opens at window start for Welcome. The helm's `welcome_seen` preference is no
longer read or written by the app; leave the column and the field in place (no migration, no protocol change), and say
in SPEC_impl.md that they are unused and why. The window-start owner (`welcome::StartDialogs`) keeps its other job: the
first batch of announcements, then the upgrade highlights check, now with nothing ahead of them, so the announcements
check runs as it does today when the card does not open. The window-start mark rules (SPEC_impl.md, near line 782) lose
the Welcome card as an exception and keep the announcements batch.

**The ? menu's Welcome.** Help → **Welcome** stays, and opens the same welcome content as a dialog: the shared
`DialogFrame` in the launcher's look, its header drawn with `DialogFrame::heading(..)` (the shared 24-pixel tile that
`launcher-style-quick-dialogs` extracts, and the title; the title stays the accessible name) and the × at its end, its
body the same component the empty state draws (marks, introduction, Start a session and the three links), and no footer
(a frame with no answers, note, lead or notices draws none). The What's new plan builds its header the same way; neither
adds a header option to the frame. Escape and × close it and focus returns to the `?` button, as today. Start a session
closes it and does what New does. Add a remote host and Send feedback dismiss it and then call the existing
`hosts::add::open` and `feedback::open` with the `?` button as their opener; Read Get started leaves it open. There is
one welcome component, drawn in two places; do not keep the old card's drawing.

With no sessions, the dialog opens over the empty state, so the component is on screen twice with the same text. Its
element ids take a per-instance prefix (the pane's and the dialog's), so `scripts/gui-agent.py`'s `id:` targets and the
tests are unambiguous, and tests find the dialog by its id or role, never by its text.

Today's Send feedback now handoff (`welcome.rs`, `hand_off`; the `handed_off` flag, the `OnClosed` continuation and the
refused-open rollback) existed only so the window-start owner could wait for the feedback dialog before the
announcements check. With the card gone it has no reader: remove it, and remove `feedback::open_returning`'s `on_closed`
parameter and `OnClosed` once no caller passes `Some` (the other caller, `rail_notices.rs` about line 918, passes
`None`).

**Room for later.** Keep the content a component with an obvious place above the title for a future media slot (the
video thumbnail) and do not build that slot. Nothing for the coaching indicators.

**Agent runs.** `scripts/gui-agent.py`'s `--start-dialogs` switch and `agent_startup::start_dialogs_off` keep governing
the window-start dialogs that remain (announcements, upgrade highlights). The empty state is not a dialog and shows in
agent runs like any other pane state; update the docs and scripts that describe the Welcome card opening at start
(`docs/gui-agent.md`, `scripts/gui-agent.py`, `scripts/test-gui-agent.py`) to say what now happens.

### Where it stands (from planning, at main 210c22183)

- `welcome.rs` holds the card (`WelcomeDialog`, `open`, `open_card`, `mark_seen`, `dismiss_card`, `hand_off`, `item`)
  and `StartDialogs` (`decide`, `card_gone`, `handoff_closed`, `run_due_check`, `remaining_dialogs`), with tests in
  `welcome/tests.rs`. Its module docs describe the whole start sequence.
- `session_pane.rs`: `placeholder(collections)` returns the three placeholder strings (about line 868), and the module
  docs explain the three-states rule and New as home with no sessions (about lines 72-77, 220, 374, 425, 524).
  `session_pane/card.rs` has the "no sessions at all" helper used for focus.
- `help_menu.rs` opens the card from the `?` menu (about line 392).
- `main_view.rs` owns `StartDialogs` for the window's life (about lines 44, 108, 157).
- `feedback.rs` has `open` and `open_returning`.
- The preference is `welcome_seen` in `crates/farhelm-proto/src/ui_channel.rs` (about line 761) and helm schema 51.
- Goldens: `welcome-dialog` (`goldens/mod.rs`, `welcome_dialog_setup`; `goldens/other_dialogs.rs` opens the card). No
  Rust golden shows the empty main area. About forty GUI test and golden files set `welcome_seen: Some(true)` to keep
  the card away; with the field kept in the protocol those settings become inert. Leave them as they are: removing them
  is churn with nothing to gain.
- The docs screenshot batch (`docs/gpui/screenshot-scenes.json`, governed by `docs/gpui/screenshot-batch.md`) has a
  `welcome-dialog` scene, and an `app-empty-fleet` scene (about line 469, reference `empty-fleet`) that is the existing
  picture of exactly the surface this plan changes: update its title and reference. Every other zero-session scene (for
  example `app-trash-dialog-empty`, about line 486) will now show the welcome behind its dialog; say so in the report.
- `scripts/gui-e2e-smoke.py`: `test_52_welcome_reopens_from_the_help_menu_…` checks the reopened card and
  `welcome_seen`; `test_53_a_window_start_opens_the_card_only_when_the_run_asks_for_it` (about lines 1466-1513) is the
  end-to-end proof of `--start-dialogs`, and its only oracle is the card and `welcome_seen`. Without the card a fresh
  helm on Linux shows nothing at window start (no upgrade highlights outside the installed Mac app; announcements only
  when the built-in table is non-empty), so test 53 cannot be updated mechanically. Decided fallback: if the smoke
  script can seed a pending announcement for the `--start-dialogs` run through an existing helm route or the run's own
  setup, keep both halves with the announcements dialog as the oracle. Otherwise drop test 53, and say in the report
  that `--start-dialogs` keeps only its unit tests. Do not build a new seeding mechanism for it.
- Docs that describe the card or those smoke steps: `.agents/validation.md` (the smoke inventory, about lines 911-913
  and 966), `docs/gpui/screenshot-batch.md` (about line 39, "Welcome card needs `--start-dialogs`"),
  `docs/gpui/foundation-ownership.md` (about lines 503 and 540), and `docs/gpui/parity/other-dialogs.md` (about line
  156; check whether that parity record is still maintained before editing it).
- User docs: `website/src/content/docs/docs/get-started/first-session.mdx` mentions the Welcome card.
- SPEC.md: "Welcome card" (about line 5040), the Help menu paragraph's "Welcome shows the welcome card again" (about
  5036), "Announcements" ("Initial Welcome, including its Send feedback now handoff, closes first", about 5063), the
  What's new paragraph ("after Welcome and any pending Announcements", about 900), "Where focus rests" (about 3837), and
  the failure-handling bullets that mention marking the welcome card seen (about 4982 and 5003; the What's new bullet
  defines its handling "like marking the welcome card seen" and must be reworded to stand alone). SPEC.md has no
  description of the empty main area yet; the first PR adds one. SPEC_impl.md: near lines 782-786 and 5171-5176. The
  What's new plan (`launcher-style-whats-new.md`) edits the same SPEC.md What's new paragraph; whichever lands second
  rebases over the other's wording knowingly.

### Acceptance criteria

1. With no sessions, the main area shows the welcome as "The agreed design, in words" says, in both appearances, at the
   default window size and at a narrow one; "Loading sessions…" and the blank state are unchanged.
2. No Welcome card opens at window start; announcements and upgrade highlights open as they did when the card did not
   open; nothing reads or writes `welcome_seen`.
3. Start a session, Add a remote host, Read Get started and Send feedback do what the design says, from the empty state
   and from the `?` menu's dialog, with the focus and return rules above; tests cover each.
4. Focus rests on Start a session with no sessions; with a session, focus rules are unchanged.
5. The marks row is built from the launcher's harness order filtered by `is_shell`, so a new agent harness appears
   without touching this view, and no shell or custom mark appears.
6. Goldens, light and dark: the empty main area with the welcome (default size, and one narrow window), and the `?`
   menu's Welcome dialog (replacing `welcome-dialog`). The visual review passes for each.
7. SPEC.md and SPEC_impl.md say what now happens, everywhere listed under "Where it stands"; the user docs, the agent
   scripts and docs, the e2e smoke test and the docs screenshot scene follow (read `website/AGENTS.md`,
   `docs/gpui/screenshot-batch.md` and `.agents/docs-media.md` first where they apply).
8. The last PR removes the TODO.md entry ("Make Welcome the empty main area.").

## Decisions already made

Settled with the maintainer on 2026-11-25, on the mockup page and the planning page:

- **Direction W7, "Empty state"**, with no startup card.
- **Shown whenever the helm shows there are no sessions at all**, not only until the first session; the button reads
  "Start a session".
- **Help → Welcome opens the same content as a dialog**, built from the same component.
- **Focus rests on Start a session** with no sessions.
- **The marks row lists every supported agent**, from the harness map.
- **Later, not now:** a video thumbnail with click-through, and light coaching indicators. Leave room; build neither.
- **Review gates:** a single fresh-context `claude-opus-5-5` (Opus 5.5) agent at `high` effort per PR for the code, and,
  in addition, the mandatory visual review by Opus 5.5 at `high` effort described under "Visual review". The
  maintainer's words for the visual review: "make sure there is a visual review spec by opus to visually inspect it
  besides usual code review. no nit picking on precise pixels but has to look basically right."
- **Release note:** none (see "Release note" below).
- **No-workhorse mode:** the plans default, stated on the planning page.
- **Runs after `launcher-style-quick-dialogs`**, whose shared components it uses.

Planner decisions:

- `welcome_seen` stays in the schema and the protocol, unused; dropping it would be a migration and a protocol change
  for no user benefit.
- Sizes as given above, from the mockup.
- The ? menu dialog's links dismiss it and call the existing openers with the `?` button as opener. Today's handoff
  machinery and `feedback::open_returning`'s close continuation are removed as dead.
- The marks row's source (the launcher's harness order filtered by `is_shell`), the Linux chip rule and New's
  availability reuse, from the planning review.
- Test 53's fallback and leaving the inert `welcome_seen` settings in tests, as "Where it stands" says.
- `StartDialogs` stays in `welcome.rs` unless its module docs cannot be made accurate there; moving it is optional.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the words quoted above, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md",
  "Code rules" (no per-harness branching or hand-kept harness lists in shared code: the marks row comes from the places
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` names; read `docs/harness-marks.md` before touching `marks.rs` or
  `marks/`), and its "Instructions by path" for every path you touch (`website/AGENTS.md` before `website/`,
  `.agents/docs-media.md` before `scripts/docs-shots/`, `.agents/validation.md` before `scripts/` or any test).
- `.agents/validation.md` for the GUI test selections, the e2e smoke script and the screenshot goldens (re-capturing
  them with `scripts/check-gui-goldens.py --update`, inspecting every changed picture, and publishing them with
  `python3 scripts/gui-goldens.py publish` before any rebase, as it describes).
- `.agents/test-authoring.md` for every test added or changed.
- SPEC.md's sections listed under "Where it stands", and "Keyboard focus" (things the user did not start never open a
  layer over what they are doing); the GUI architecture rules in SPEC_impl.md ("GUI architecture"); the theme module's
  rule that views never hold a color (`crates/gui/farhelm-gui/src/theme.rs`, module docs).

**Planner proposals** (after a fresh-context planning review): see "Planner decisions" above and the outline below.

## Outline

1. `plan/welcome-empty-state/01-welcome-content`: the shared welcome component, drawn in the empty main area in place of
   "No sessions — create one", with focus on Start a session; its goldens; SPEC.md's "Where focus rests" and a new
   description of the empty main area.
2. `plan/welcome-empty-state/02-no-startup-card`: the startup card removed, `StartDialogs` reduced to announcements and
   upgrade highlights, Help → Welcome opening the component as a dialog, `welcome_seen` unused; its golden replacing
   `welcome-dialog`; the remaining SPEC.md and SPEC_impl.md edits, the user docs, agent scripts and docs, the e2e smoke
   test and the docs screenshot scene; the changelog fragment; the removal of the TODO.md entry.

Each PR carries its own SPEC edits for what it changes, its goldens, and the changelog fragment its type calls for (root
`AGENTS.md`, "Changelog fragments"). Between the two PRs a first launch would show both the card and the empty state;
that is acceptable inside one stack that lands together, but say so in the first PR's description.

## Fallbacks

- If drawing the component inside `DialogFrame` for the `?` menu does not fit the frame as it stands (`heading`, no
  footer), a new frame option is a last resort: run the scope reassessment below first, check whether the What's new
  plan already added one to reuse, and never keep the old card's drawing alongside.
- If the session pane cannot host interactive content where the placeholder is without changing its focus rules more
  than "Focus" above says, run the scope reassessment below before going further.
- Decisions to log: the exact sizes and gaps, the narrow-window behavior, every golden whose change you judged intended,
  which way test 53 went, and every review finding declined, with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-welcome-empty-state-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-welcome-empty-state-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what the PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change.

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

The review gate and the visual review keep the model and effort this file demands; the scope reassessment runs at your
own model and effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the
resume check, and the cold reads of a blocked question and of the report) are exempt from the rules above and run as
that file says: native, read-only, at your own model.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: `plan/welcome-empty-state/01-welcome-content`,
  `plan/welcome-empty-state/02-no-startup-card`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. `crates/gui/farhelm-gui/tests/goldens.txt`
  and the golden `SCENES` list conflict mechanically with other plans that add scenes: keep both sides, then re-run the
  goldens as `.agents/validation.md` describes.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `claude-opus-5-5` (Opus 5.5) at
`high` effort, launched natively, or through `$scode-harness-shellout` when your harness cannot serve that model
natively. If it cannot be reached when a review is due and is still unreachable after a reasonable retry, block the plan
per `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute
another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Visual review

Mandatory, in addition to the review gate, for every PR that changes what the empty main area or any dialog looks like
(here, both PRs): the maintainer demands a fresh-context agent on `claude-opus-5-5` (Opus 5.5) at `high` effort,
separate from the review gate's agent, launched natively or through `$scode-harness-shellout`, which must be satisfied
that the rendered result matches the intent of the agreed mockup. Never substitute another model, and never skip it
because the review gate passed. If it cannot be reached after a reasonable retry, block per
`plans/.agents/executing.md`, "Executing one plan" (step 10).

- **What it looks at.** Run `scripts/check-gui-goldens.py --update` (as `.agents/validation.md` describes) so the PR's
  golden scenes are captured, light and dark, into `target/gui-goldens/captures`, and give the reviewer the paths of
  every new or changed picture for the PR's surfaces. Give it the mockup page,
  `https://snippets.scode.org/s/farhelm-whats-new-directions/` (section "W7 Empty state", in part 2; for the `?` menu's
  dialog, which the mockup does not draw, section "W2 Launcher sibling" shows the frame), with the instruction to
  compare in both of its themes (the page's Dark and Light buttons), and the design words of this file ("The agreed
  design, in words", the acceptance criteria and the decisions), which win where the mockup and the words differ. Tell
  it that the goldens render on Linux, where Start a session has no key chip (the mock's ⌘N is the Mac app's), and that
  the mock's six marks stand for the full agents row. If the reviewer cannot fetch the page, it judges against the words
  alone and says so.
- **Its scope is this PR's.** Name in the prompt what this PR builds (its outline step) and what later PRs will add.
  Elements of the mockup that a later PR adds are not findings, and neither are older parts the PR does not touch; the
  last PR's review judges the whole surface.
- **Its charter**, carried in full in the prompt:

  > Compare the rendered screenshots of this PR's surfaces with the agreed mockup and the written design. Judge whether
  > each rendered picture matches the mockup's intent: layout and hierarchy, grouping, spacing rhythm, type sizes and
  > weights, colors and color roles, marks and icons, the states shown, and the wording. The bar is that it looks
  > basically right; do not nitpick precise pixels. Pixel identity is not the bar; an implementation detail the mockup
  > could not know (a real font's metrics, a real control's focus ring) is fine. A missing or misplaced element, a wrong
  > hierarchy, a wrong color role, an element the design does not have, text that is cut off or overlapping, or a state
  > that looks broken is a finding. Report findings to the named file, most severe first, each naming the picture, what
  > differs from the intent, and what would bring it in line, and end with one line: SATISFIED or NOT SATISFIED. Edit
  > nothing and touch no VCS state.

- **Looping.** Fix what it finds that is a real mismatch, re-capture, and run a fresh visual review on the new pictures,
  until it reports SATISFIED. A finding you judge wrong (it misreads the mockup, or asks for something the written
  design rules out) is declined with a DECISION in the log and the picture and reason named in the report; a reviewer
  that still reports NOT SATISFIED only on declined findings counts as passed. If the same mismatch survives three
  rounds, run the scope reassessment below on it before another round.
- **Publishing.** Publish the accepted captures with `python3 scripts/gui-goldens.py publish` and commit the manifest
  change with the PR, as `.agents/validation.md` describes.

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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file. If current code already does what an entry asks, or the code
it targets no longer exists, record that in the working log and the report, and build only what is still missing.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate and its visual review, with the last one
removing the TODO.md entry this plan covers ("Make Welcome the empty main area."). Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
