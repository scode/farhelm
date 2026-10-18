# Launcher and dialogs: keep focus on gap clicks, and Enter launches from the dialog itself

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

In the session launcher, clicking the empty space between fields loses the user's place, and Enter then does nothing.
Make a click on empty dialog space leave focus where it was, make Enter launch when the launcher itself has focus, and
give every other dialog the same behavior through one shared piece of code: a gap click keeps focus, and Enter with the
dialog itself focused presses its main button.

TODO.md entry this plan covers (Near term): "**Fix launcher keyboard focus and Enter.** Clicking the background between
fields clears the selection; clicking again appears to select the whole dialog. Enter does not launch in either state
and seems to require a focused control. Make Enter launch when the dialog is ready, including after clicking between
fields."

What planning research found in the code (verify before relying on it):

- The launcher is the `form.create-session-form` (`role="dialog"`, `tabindex: "-1"`, `novalidate`) in
  `CreateSessionForm`, `crates/farhelm-ui/src/list/create_form.rs` (around line 4683). Because the form itself is
  focusable, a click on a gap moves focus from the control the user was on onto the form.
- Enter launches today only from controls: text fields through the browser's implicit submit, which reaches `onsubmit`;
  choice controls through `enter_choice` (`crates/farhelm-ui/src/launch_controls.rs`), which applies the choice and
  calls `requestSubmit()` (`resubmit_composer`). The form's own `onkeydown` only stops a held or composing Enter and
  handles Escape, so Enter on the focused form does nothing.
- Launch readiness is decided in `onsubmit` (the `LaunchPrerequisites` refusal, and the empty-command check in command
  mode). Launch uses `aria-disabled`, not `disabled`, so a refused submit still runs and shows its reason beside Launch.
  That is the path every Enter launch uses, and it is the path this plan reuses.
- Clone and Replace with are the launcher: SPEC.md (Sessions, around lines 1091-1097) says "Clone, replace with, and New
  are one launcher", so they get the launcher fix with no separate work.
- The Restart with dialog (`.restart-with-dialog`, `crates/farhelm-ui/src/restart_with.rs`, around line 455) has the
  same focusable-container shape and held-Enter guard. It opens with focus on Cancel, and `focus_restart_with_submit`
  (around line 188) deliberately parks focus on the container while its submit button is disabled. Its submit is
  natively `disabled` while the draft is unchanged, so clicking it then does nothing, as SPEC.md requires.
- The "whole dialog selected" look is either the global `:focus-visible` ring (`crates/farhelm-ui/assets/app.css`,
  around line 1796) drawn on the focused form, or a double-click text selection starting on the gap ("clicking again").
  Not confirmed in a browser; find out which. The mechanism below prevents both.
- Dialog focus already goes through three installers: `install_dialog_with_focus` in
  `crates/farhelm-ui/src/hosts/settings_dialog.rs` (around line 246; reached through `install_dialog_with_selector` and
  `install_add_dialog`) serves settings, templates, the quick switcher, welcome, feedback, the trash dialog, ssh help,
  tunnel consent, and the host settings, add, setup, removal and uninstall dialogs; `install_composer_focus_trap` in
  `create_form.rs` serves the launcher; `install_focus_trap` in `restart_with.rs` serves Restart with. Most other
  `tabindex: "-1"` hits in `crates/farhelm-ui/src` are not dialogs (hidden relay buttons, listbox options, dismiss
  buttons, a settings section).
- The header's Restart, Replace and Delete confirmations (`.header-confirm` strips in
  `crates/farhelm-ui/src/session_view.rs`, around lines 2381, 2481 and 2585) and the session row's confirmations are
  inline strips with no focusable container. Focus starts on the confirming button. A click on the strip's empty space
  or text probably blurs that button to the page, which is the same symptom by a different route. The user named Replace
  explicitly.
- The notification bell's list (`crates/farhelm-ui/src/list/bell.rs`, around line 252) puts focus on its container on
  purpose, "so a stray Enter cannot clear anything". It is out of scope for both parts of this plan.

### Acceptance criteria

- **Gap clicks keep focus (launcher).** After focusing any control in the launcher, a primary-button click on empty
  dialog space (space that is not a control, a label, a link or text) leaves focus on that control, so the next keypress
  goes where it went before the click. No focus ring appears around the whole dialog, and a double-click on empty space
  selects nothing.
- **Enter on the dialog (launcher and Restart with).** If the launcher form itself holds focus (by any route, including
  a click on static text), a plain Enter launches through the ordinary Launch path (`requestSubmit()`), which either
  launches or shows its refusal beside Launch, exactly like the other Enter launches. If the Restart with container
  holds focus, a plain Enter performs its primary action through that dialog's own rules: an unchanged or invalid draft
  does nothing, "stop and restart" is the displayed action it confirms. A held or composing Enter never launches or
  restarts.
- **Enter on the dialog (every other dialog with a main action).** The same holds for every dialog the shared installers
  serve that has one main action, destructive ones included: host removal, uninstall, host setup, permanent trash
  deletion, tunnel consent, and any other the check finds (welcome's "got it", say). A plain Enter with the container
  itself focused presses that dialog's main (confirming) button, exactly as a click would. A disabled main button does
  nothing, and a held or composing Enter never confirms. Dialogs without a single main action (settings, templates, the
  quick switcher, ssh help, feedback, if that is what the check finds) do nothing on Enter at the container. The bell
  stays as it is.
- **Every existing Enter rule is unchanged:** the initial-prompt box inserts a newline; the search box with a query or
  an open result list keeps its own rules; the model field, the folder field's highlighted entry, the custom permission
  flags editor, the YOLO confirmation and the save-as-template panel keep theirs; action buttons (Cancel, reset, browse,
  retry, clear prompt) keep their normal press; Restart with still opens on Cancel. The existing
  `e2e/tests/launcher-enter.spec.ts` and `e2e/tests/restart-with.spec.ts` cases keep passing.
- **Text stays selectable, and scrolling keeps working.** Drag-selecting text in a dialog (a refusal message, say) still
  works, and a focused, scrollable dialog container still scrolls with the arrow keys and Page Up/Down. The launcher and
  the Restart with dialog are themselves scroll viewports (`app.css`, around lines 5289 and 3501); that never takes them
  out of scope.
- **Other dialogs get the gap-click behavior.** The shared behavior is wired into the three installers above, so every
  dialog they serve keeps focus on a gap click. The header and row confirmation strips (Restart, Replace, Delete) get
  the same behavior where a click on their empty space loses focus today. The bell is left alone. A short check of each
  installer's dialogs and of the strips goes in the working log as a DECISION and in the report, with anything left out
  and why.
- **Spec.** SPEC.md's launcher keyboard paragraph (Sessions › Creation, the "Enter on an empty box launches..." text,
  around lines 600-616) gains a sentence that Enter with the launcher itself focused launches through the ordinary
  Launch path. The Restart with paragraph (around lines 1053-1066) gains the matching sentence. A sentence states that a
  click on empty space in a dialog or confirmation keeps focus on the control in use; put it where SPEC.md describes
  dialog focus (the confirmation-focus paragraph around line 1123, or the launcher text, whichever reads better). The
  confirmation-focus paragraph (around line 1123) also gains a sentence that Enter with a dialog itself focused, rather
  than one of its controls, presses that dialog's enabled main button. SPEC_impl.md records the shared helper if it
  documents dialog focus.
- **Tests,** on Chromium and WebKit, one per installer path rather than one per dialog: for the launcher, Restart with,
  one dialog served by `install_dialog_with_focus`, and the header Replace strip (if changed), focus a control, click
  empty space, assert focus is still on the control. For the launcher and Restart with, also press Enter with the
  container focused and expect exactly one launch or restart (or the refusal beside Launch when the selection is
  incomplete; nothing for an unchanged Restart with draft). For one destructive dialog served by
  `install_dialog_with_focus` (permanent trash deletion or host removal), Enter with the container focused confirms it
  once, and does nothing while its confirming button is disabled; for one dialog without a main action, Enter at the
  container does nothing. One test that a drag starting on dialog text still selects it. A new spec file for the shared
  helper is fine; `e2e/tests/modal-focus.spec.ts` currently tests rename focus, not dialogs.

## Requirement sources

**The user's decisions (2026-10-17):**

- After clicking a gap: "Focus stays on the control you were on, and Enter from the dialog itself also launches" (a
  launch that cannot go yet shows its reason, as Enter does elsewhere).
- The focus ring around the whole dialog: hide it.
- Restart with gets the same fix in this plan, "And any other dialogs that have the same problem, like Replace with,
  Replace, Clone, etc., all of them should behave similarly here."
- When focus is on a dialog itself rather than one of its controls, Enter confirms every dialog's main action, including
  single-action and destructive ones (host removal, uninstall, permanent trash deletion, host setup, tunnel consent):
  the user's answer "B — Yes, every dialog's main action" to that question, asked after the planning review.
- Review gate: a single fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC.md's launcher keyboard rules in Sessions › Creation (each control's Enter
behavior, "A held Enter never launches", refusals beside Launch); the Restart with keyboard rules (SPEC.md around
1053-1066); SPEC.md's confirmation focus rule (around line 1123: focus starts on the enabled confirming button, Restart
with and the YOLO confirmation start on Cancel) and the rule that a destructive confirmation authorizes only what its
prompt said; the modal isolation and focus traps (`crates/farhelm-ui/src/modal_isolation.rs` and the three installers),
which park focus on the container as a fallback and must keep working; root `AGENTS.md` (Tests and validation, Changelog
fragments).

**Planner proposals (checked by a fresh-context planning review):**

- The mechanism: one `mousedown` listener per dialog that cancels the default action when the primary button goes down
  on genuinely empty space, meaning the target is not inside an input, textarea, select, button, label, link or other
  focusable element and has no text of its own. Focus then never moves, so no focus memory, pointer-origin flag or
  restore step is needed; it cannot fight the focus traps or the isolation's keydown net because it never calls
  `focus()`; it also prevents a double-click selection on the gap; and it works on the confirmation strips, which have
  no focusable container. Export it from one module (beside `modal_isolation`'s install JavaScript, say) and call it
  from the three installers and the strips.
- A click on static text (a `p`, `dt`, `dd`) still moves focus to the container, since cancelling there would break text
  selection. For the launcher and Restart with, Enter-on-the-container covers that case. For a strip, a text click may
  still blur its button; the executor checks this and logs what it found.
- Fallback mechanism, only if the scoped cancel fails a browser test: on pointer focus of the container, put focus back
  on the last focused control, reusing the existing `__farhelmFocusMemory` / `__farhelmLastFocus` properties that
  `install_dialog_with_focus` already records (and `templates.rs` reads), not a second memory. It must not undo the
  deliberate parking in `focus_restart_with_submit` or the isolation net. Log the choice.
- Do not remove `tabindex="-1"` from the containers: the traps and the isolation net park focus there, and Restart
  with's Shift+Tab logic counts the container as the start of the cycle.
- Enter on the container: a keydown check (Enter, `event.target` is the container, not repeating, not composing), then
  `requestSubmit()` for the launcher form, or a `click()` on the dialog's main button otherwise (Restart with's primary
  button is natively disabled for an unchanged draft, and a click on a disabled button does nothing). For the dialogs
  `install_dialog_with_focus` serves, add one optional main-button selector argument, passed by each caller that has a
  main action and omitted by the rest; do not infer the main button from the initial-focus selector, which for some
  dialogs is Cancel or Close. Pressing the button the user sees keeps SPEC.md's rule that a destructive confirmation
  authorizes only what its prompt said.
- Hide the ring with a rule scoped to these dialog containers' own `:focus-visible`, not by changing the global rule.

## Outline

One `fix:` PR by default: the shared mouse-down helper and its wiring into the three installers and the confirmation
strips, Enter-on-the-container for the launcher, Restart with and every other dialog with a main action (through the
installers' new main-button argument), the scoped ring rule, the SPEC.md sentences, the browser tests, a changelog
fragment (`kind: fixed`), and the removal of the TODO.md entry. Split into two PRs (the launcher first, then Restart
with and the other dialogs) only if wiring the other dialogs turns out to need real per-dialog work; then the second PR
edits the first PR's fragment and carries the TODO.md removal.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
`python3 releasing/check-changelog.py format`, and through the recorder on Chromium and WebKit: the new tests plus
`launcher-enter.spec.ts`, `launcher-tabs.spec.ts`, `restart-with.spec.ts`, `modal-layout.spec.ts`, `clone.spec.ts`,
`replace.spec.ts`, `trash-dialog.spec.ts`, and the existing specs of the header confirmations and of whichever
`install_dialog_with_focus` dialog the test uses. Not the full suite. The report says the desktop app's webview itself
is not exercised (WebKit stands in for it).

## Fallbacks

- The restore-on-pointer-focus fallback above, if the scoped mouse-down cancel fails a browser test.
- A dialog whose container focus is deliberate (the bell is the known one) is left out of the gap-click behavior; log
  it. This never applies to the launcher or Restart with, whose scrollable bodies are in scope regardless.
- If the "whole dialog selected" look is something other than the ring or a gap double-click, fix it if the fix is local
  to the dialog's styles; otherwise describe it in the report.

Examples of departures that need the scope reassessment: removing the containers' `tabindex`, rewriting the focus traps
or modal isolation, a global change to `:focus-visible`, per-dialog Rust keydown handlers instead of the shared helper.

Decisions to log: the installer and strip check with each disposition, including which dialogs got a main button; scoped
cancel versus restore; what the "whole dialog selected" look turned out to be; whether strip text clicks still blur.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launcher-enter-focus-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/launcher-enter-focus/<nn>-<short-name>` (for example `plan/launcher-enter-focus/01-dialog-focus`).
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
