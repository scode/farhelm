# Keyboard shortcuts to delete and rename the open session

Written against main at 78394ca6 on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan. The `name-session-while-starting` plan also changes how the rename dialog
opens and closes; if it has landed by the time you run, build on what it left rather than around it.

## The goal

Add keyboard shortcuts that delete and rename the open session (the one shown in the main pane), so neither needs the
mouse. Today no shortcut acts on a session: Rename lives only in the sidebar row's actions menu, and Delete is a header
button (and a row menu item).

TODO.md entry this plan covers (Near term): "**Session action keyboard shortcuts.** Add keyboard shortcuts to delete and
rename a session. Consider shortcuts for Restart, Replace, and the other session actions too."

### Acceptance criteria

- **Delete:** ⌘W in the Mac app, Ctrl+Shift+Backspace in browsers (Mac browsers included). It does exactly what clicking
  the header's Delete button does for the open session: it asks when the button would ask (the "skip session
  confirmations" setting included) and acts at once when the button would. With no session open, the key does nothing.
- **Rename:** ⌘L in the Mac app, Ctrl+Shift+L in browsers. It opens the existing rename dialog for the open session;
  when the dialog closes, focus returns to the terminal.
- Both work while the terminal has focus, and no byte of the chord reaches the terminal program. Both yield to an open
  modal dialog, like the existing shortcuts. The browser Delete key leaves text fields other than the terminal alone,
  where Ctrl+Shift+Backspace edits text.
- In the Mac app, ⌘W no longer closes the window or quits: the "Close Window" entry is removed from the app's Window
  menu. The window's close button and ⌘Q still close the app.
- **Regression guard for ⌘W (the user's explicit demand):** a test fails if anything gives the Mac app a menu entry that
  owns ⌘W again, such as restoring the menu library's built-in Close Window. The test's docstring explains why it
  exists: ⌘W is Farhelm's delete-session shortcut, the menu library's built-in Close Window entry hard-wires ⌘W, and a
  menu accelerator is taken by macOS before the page sees the key, so restoring it silently turns ⌘W back into "quit
  Farhelm".
- Escape cancels each header confirmation question (Restart, Replace, Delete) and returns focus to the terminal.
- The keys appear in tooltips: the Delete hint on the header Delete button (not on the row menu's Delete, which acts on
  its own row and deletes an ended session at once), and the Rename hint on the open session's Rename menu item, or
  worded as acting on the open session. The docs that describe Delete and Rename mention them.
- SPEC.md describes the shortcuts where it lists the others; SPEC_impl.md where it describes the existing shortcut
  installers.

## Requirement sources

**The user's decisions (2026-10-17), in their words where given:**

- Delete and Rename only; no shortcuts for the other actions in this plan.
- "lets use cmd-w for delete. it currently quits the whole app, but cmd-w perfectly fits what I and i think most users
  would expect for deleting a single session. same story as button - confirm when the button would confirm, do it
  immediately when it would not."
- Rename: "cmd-l for rename seems fine".
- Browsers: Delete is Ctrl+Shift+Backspace ("unless we can do ctrl+shift+w which is consistent with other cases but if
  the browser will act upon that then backspace is fine"; Chrome and Firefox close the window on Ctrl+Shift+W and a page
  cannot take it over). Rename is Ctrl+Shift+L, after checking that Chrome, Firefox and Safari leave it free; otherwise
  pick another letter and record why.
- ⌘W with no session open does nothing.
- Remove "Close Window" from the Mac app's Window menu (rather than keeping a custom entry without a shortcut).
- "make sure there is test coverage so if in the future an agent decides to restore it, the test will fail and have an
  explanation of why we are testing for this" (about the ⌘W change).
- Keys shown in tooltips and the docs. Escape cancels a header confirmation question and focus returns to the terminal.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC.md's shortcut rules near its desktop window chrome section (browsers keep their
Cmd shortcuts, so the Mac-app chord is chosen at build time, as for ⌘N and the number keys) and Lifecycle operations
(Delete's confirmation rules, the skip setting). Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- One shortcut script for both keys, modelled on `crates/farhelm-ui/assets/template-number-shortcuts.js` (a pure key
  matcher with JS unit tests, because Playwright cannot press the Mac-app chords), installed from `AppBody` in
  `crates/farhelm-ui/src/lib.rs` like the existing installers: capture phase, yield to a visible
  `[role="dialog"][aria-modal="true"]`, ignore repeat and IME composition, click a trigger.
- Delete clicks the live header Delete button (`.header-delete`, `crates/farhelm-ui/src/session_view.rs`), so its
  question, the skip setting and the busy state come for free; with no session open there is no button and the key does
  nothing (in the Mac app it must still be swallowed so nothing else acts on it).
- Rename uses a hidden trigger in the list (`crates/farhelm-ui/src/list/view.rs`) calling `on_rename_start` for the open
  session, taking the title from the app's current session because its row may be filtered out of the sidebar.
- The Mac menu: Dioxus 0.7's default menu builder is private and muda's predefined Close Window item carries a fixed ⌘W,
  so build the Mac app's menu explicitly with the default's other items and no Close Window. Build it from a description
  that compiles and is unit-tested on Linux (CI and the executors run on Linux; nothing compiles macOS-only code before
  a release build), and install it only in the macOS desktop build, the same pattern the app uses for ⌘N. Leave the
  Linux desktop menu alone. The ⌘W regression test checks that description. Closing the window through its close button
  must still save the window's size and position as today.
- Escape: one shared key handler for the three header questions, handing focus back with
  `window.farhelmTerm.focusSelected()`. Only on Escape; after an answer the app already decides focus.

## Outline

1. `fix`: Escape cancels the header confirmation questions, focus to the terminal. Tests: browser e2e in
   `e2e/tests/header.spec.ts` or `session-confirmations.spec.ts`.
2. `feat`: the shortcut script with both matchers' Delete half, the Mac menu change and its regression test, the Delete
   tooltip, spec and docs (`website/src/content/docs/docs/using/stop-restart-resume.mdx` and `work-in-a-session.mdx`
   describe Delete). Tests: JS unit tests for the Mac-app and browser matchers; the Rust unit test on the menu
   description; browser e2e for Ctrl+Shift+Backspace (asks like the button, honours the skip setting, no bytes reach the
   terminal as `e2e/tests/quick-switcher.spec.ts` proves for its chord, yields to dialogs and text fields).
3. `feat`: Rename, its tooltip, spec and docs (`session-list.mdx`), focus back to the terminal on close. Tests: browser
   e2e for Ctrl+Shift+L, JS unit test for ⌘L. Removes the TODO entry.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the touched crates' unit tests and the JS unit tests through the
recorder, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
`python3 releasing/check-changelog.py format`, and a selected browser subset on Chromium and WebKit through the recorder
(the specs you changed plus `quick-switcher.spec.ts`, `sidebar.spec.ts` and `tooltip-coverage.spec.ts`). Not the full
suite. The Mac-app chords and menu cannot be exercised from Linux: say in the report's "things you should know" that ⌘W,
⌘L and the menu need a check in the Mac app (the maintainer can use the devbuild flow).

## Fallbacks

- If Ctrl+Shift+L is taken in Chrome, Firefox or Safari per their published shortcut lists, pick another free letter and
  record why.
- If it turns out the Mac app's menu can drop Close Window's accelerator without rebuilding the menu, do the smaller
  thing; the regression test still applies.

Examples of departures that need the scope reassessment: a shortcut registry or settings for customizing keys, a native
menu entry per shortcut, shortcuts for further actions.

Decisions to log: the Rename letter in browsers, the menu construction, how the browser Delete key recognizes a text
field.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-session-action-shortcuts-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/session-action-shortcuts/<nn>-<short-name>` (for example `plan/session-action-shortcuts/01-header-escape`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
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
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
