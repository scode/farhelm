# Enter launches from anywhere in the launcher, and a refused launch says why

Written against main at adf78aba on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan runs after `plans/queue/save-launcher-as-template.md`: that plan adds a "save as template" panel inside the
New session dialog with its own Enter handling, and this plan builds on it once it is on main.

## The goal

In the New session dialog (which also serves Clone and Replace with), Enter launches only while the text box has focus;
after clicking or tabbing to a button, Enter presses that button instead. And when the dialog cannot launch, the user
usually sees nothing at all: a greyed-out Launch makes Enter silently do nothing, and the few refusals that do have
words appear in small red text at the very bottom of a dialog that scrolls. Make Enter on a choice choose it and launch,
in the New session dialog and in the Restart with dialog, and make every refused launch in the New session dialog say
why, next to the Launch button.

Acceptance criteria:

- New session dialog, refusals (D4): whenever an attempt to launch (Enter anywhere in the dialog, or Launch) cannot
  launch, the reason appears next to the Launch button at the top of the dialog, in words, instead of silence or a line
  at the bottom. This includes every condition that greys out Launch today, and the refusals its submit handler and the
  helm already put into words. A greyed-out Launch's hover text names what is missing.
- New session dialog, Enter (D1, D3):
  - Enter on a choice button (the agent/command tabs, a harness option, an effort, permission or trust button) applies
    that choice, then launches through the ordinary Launch path; an incomplete setup gets the reason as above.
  - Enter on the YOLO radios, the resume checkbox, or the host and agent-type dropdowns launches with the value shown,
    without changing it (Space and arrows keep setting values).
  - Enter on an action button (Launch, Cancel, Reset, Browse, Retry, folder browser buttons) keeps its normal press.
  - The search box, the model field, the YOLO confirmation question and the save-as-template panel keep their own Enter
    rules; SPEC.md's existing rules for Enter in the search box hold (a non-empty query with no result never launches).
  - A held Enter (auto-repeat) never launches.
- Restart with dialog (D2): Enter on a choice button applies it, then performs the dialog's primary action exactly as
  pressing it would, including stopping a working agent when the primary action on screen says it stops and restarts.
  Enter on its radios, checkbox or text fields acts as the primary action with the values shown. Enter where the primary
  action is inactive (no setting changed) does nothing.
- SPEC.md describes the new Enter rules for both dialogs, where refusals appear, and that Enter on a choice in Restart
  with confirms "stop and restart" when that is what its primary action says.
- Browser tests cover each rule above.
- Changelog fragments describe the changes for users.
- The last code PR removes the TODO.md entry "Enter should launch from anywhere in the session launcher."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Enter should launch from anywhere in the session launcher.
Pressing Enter launches the session while the cursor is in the launcher's text box, but after clicking a button or
another control in the launcher, Enter no longer launches. Make Enter launch a complete, valid setup wherever focus is
in the launcher, keeping SPEC.md's rules for Enter in the text box (a non-empty query with no result never launches).
Decide what Enter does on a focused button, where browsers press the button itself."

When planning, the maintainer also said, about the refusal message: "I feel like I've never seen the refusal message.
Every time I start nothing happens. [...] Is it possible we need to make it more obvious?"

**What the planner found (at adf78aba; find code by name, line numbers drift).**

- The dialog is `CreateSessionForm` in `crates/farhelm-ui/src/list/create_form.rs`, a `<form role="dialog">` whose
  `onsubmit` launches. Enter launches today only through the browser's implicit submission, which presses the default
  button (Launch, `type="submit"`); a focused `type="button"` control gets pressed instead, and a disabled Launch makes
  implicit submission do nothing, silently. The search box's Enter handler relies on that on purpose (its comment says
  the platform owns the disabled check).
- Launch's `disabled` combines: busy, the selected host unavailable, the fresh destination not ready, a GitHub-scope
  search, a remembered destination no longer valid, and (on the agent tab) no structured harness or a structured choice
  error. Its hover text is "launch: start the session" whatever the state. `onsubmit` re-checks with live signals and
  puts refusals into the `error` signal, rendered as `.create-session-error` at the very end of the form, below the
  folder browser; the dialog has a `max-height` and scrolls.
- Recent-setup rows already implement "Enter applies this and launches": their `onkeydown` prevents default, applies the
  setup and calls `resubmit_composer()` (`requestSubmit()` on the form), which ignores the disabled state and goes
  through `onsubmit`. SPEC.md requires that row behavior. Tab and harness clicks call `focus_composer_surface()`, so
  after a click on those Enter already works; keyboard focus on them, and clicks on the effort, permission and trust
  buttons (which do not refocus), show the bug.
- The effort, permission and trust buttons live in `LaunchControls` (`crates/farhelm-ui/src/launch_controls.rs`), shared
  with Restart with (`crates/farhelm-ui/src/restart_with.rs`). The model field swallows Enter on purpose. The model
  list's option rows are `tabindex="-1"` and never hold focus.
- Restart with is not a form. Its primary action is a `type="button"` whose `onclick` sends the edit captured at render
  time, gated by a render-time `may_submit` that requires a changed setting; so a handler that sets a choice and then
  calls that path in the same turn would send the pre-choice edit. Its label says it stops and restarts when the agent
  is working; SPEC.md (Lifecycle, Restart with) says "A working agent is stopped first with the user's confirmation on
  that action", and the 2026-10-01 rule that a destructive confirmation authorizes only what the prompt on screen said.
- The YOLO confirmation (`yolo_confirm.rs`) focuses its Cancel by default; Enter there must keep pressing Cancel.
- The save-as-template panel (plan `save-launcher-as-template`, landing at planning time) adds a name field whose Enter
  saves, checkboxes, and Save/Cancel buttons inside the same form. Read what actually landed before starting.
- Browser tests: `e2e/tests/sidebar.spec.ts` (composer Enter tests, recent-row Enter, model Enter),
  `composer-word-search.spec.ts`, `launcher-tabs.spec.ts`, `yolo-guard.spec.ts`, `modal-focus.spec.ts`,
  `templates.spec.ts`, `restart-with.spec.ts`, `launch-button-alignment.spec.ts`.

**The user's decisions (2026-10-08):**

- D1. Enter on a choice control applies the choice, then launches if the setup is complete and valid. Action buttons
  keep their normal press. The search box, model field, YOLO confirmation and save-template panel keep their own Enter
  rules.
- D2. The same rule in the Restart with dialog, in full: Enter on a choice restarts, including when that stops a working
  agent. (The maintainer was shown that this makes Enter the confirmation for "stop and restart" and chose it over
  limiting it to an idle agent or dropping Restart with.)
- D3. Enter on the resume checkbox, the YOLO radios, and the host and agent-type dropdowns launches with the value
  shown; Enter never toggles the checkbox.
- D4. Add visible reasons in this plan: when Enter or Launch cannot launch, say why next to the Launch button at the
  top, including for a greyed-out Launch, instead of silence or a line at the bottom. Enter on a choice then always
  gives either a launch or a visible reason.
- D5. Review gate: gpt-6.1-sol high, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** SPEC.md's launcher rules (the launch composer bullet on Enter in the search box,
Recent setups, the model field), Lifecycle "Restart with" and the 2026-10-01 destructive-confirmation rule, the hover
text rule; SPEC_impl.md "GUI: Dioxus"; root `AGENTS.md` (Finishing work, including the browser test rules; Conventional
Commits; Releases and the changelog; Sharing the machine with other agents; Agent scratch space; The live install is
off-limits), `docs/test-sleep-check.md`, and `.agents/test-authoring.md` for test changes.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

### PR 1: a refused launch says why, next to Launch

`feat:` with a changelog fragment (kind `changed`: when the New session dialog cannot launch, it now says why next to
the Launch button, instead of doing nothing or showing the reason at the bottom of the dialog).

- One function names the first unmet condition behind a greyed-out Launch, in words, from the same inputs as its
  `disabled` expression, so the two cannot drift (derive `disabled` from it, or test that they agree).
- The reason line sits beside or directly under the action row at the top. The existing `error` messages move there too;
  do not keep a second error line at the bottom.
- An attempt on an incomplete setup shows the reason: Enter in the search box with an empty query and the text fields'
  implicit submission (today silently dropped by the disabled default button), and a press on the greyed-out Launch. How
  the attempt is caught (for example making Launch `aria-disabled` rather than `disabled` and refusing in `onsubmit`, or
  handling Enter explicitly) is the executor's choice; keep the search box's no-result rule and the `onsubmit` guards
  intact. A greyed-out Launch's hover text names the reason.
- Decide when the reason appears without an attempt (for example only after an attempt, cleared when the condition
  clears) so a freshly opened, still-incomplete dialog does not open with an error; log it.
- Browser tests: Enter on an empty search box with no harness chosen shows the reason by Launch and posts nothing; a
  refusal from `onsubmit` appears by Launch.

### PR 2: Enter on a choice launches, in the New session dialog

`feat:` with a changelog fragment (kind `changed`: Enter now launches from any choice in the New session dialog).

- Choice controls opt in with an explicit marker (a `data-` attribute or a caller-supplied callback), not by role or
  input type: the save-as-template panel's checkboxes and buttons sit in the same form and must not become launch
  triggers. `LaunchControls`' segment buttons take their Enter behavior from the dialog that renders them, so Restart
  with does not inherit New session's submit.
- Enter on a choice button: prevent default, ignore auto-repeat, respect the existing busy/transition guards, apply the
  choice as a click would, then submit through the ordinary path (`resubmit_composer()`, the recent-row precedent),
  which reads live signals and so sees the new choice. Enter on the radios, checkbox and dropdowns: submit with the
  value shown.
- SPEC.md: the launcher's Enter rules.
- Browser tests: Enter on an effort button and on a harness option launches with that choice; Enter on a dropdown and on
  the resume checkbox launches without changing the value; Enter on Cancel still cancels; held Enter does not launch;
  the YOLO question and the save-as-template name field are unchanged.

### PR 3: Enter on a choice restarts, in the Restart with dialog

`feat:` with a changelog fragment (kind `changed`: Enter on a choice in Restart with now restarts with it).

- The primary action must send the edit built from live state at the moment it runs, not the one captured at the last
  render (or the Enter path waits for the re-render); pick the smaller change and log it. The working-agent precondition
  the request carries must match what the primary action on screen says, exactly as a press does.
- Enter on a choice where the primary action is inactive afterwards (nothing changed) does nothing.
- SPEC.md: Restart with's Enter rule, including that Enter on a choice confirms "stop and restart" when that is what the
  primary action says, consistent with the 2026-10-01 rule.
- Browser tests in `restart-with.spec.ts`: Enter on an effort button restarts with it; with a working agent, Enter on a
  choice stops and restarts (the request carries the stop confirmation); Enter on the already-selected choice does
  nothing.
- Remove the TODO.md entry.

Out of scope: other dialogs; the order or layout of the launcher's controls beyond placing the reason line; the helm's
refusal texts themselves.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit
tests through `scripts/record-test-run.py` (a narrow nextest selection), and the browser specs listed above that touch
the launcher and Restart with, on Chromium and WebKit through the recorder, after building the web UI as root
`AGENTS.md` describes. `python -B scripts/check-test-sleeps.py` (per `docs/test-sleep-check.md`).
`python3 releasing/check-changelog.py format` for the fragments.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-enter-launches-anywhere-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/enter-launches-anywhere/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a dialog-wide keyboard dispatcher replacing the
existing per-control handlers, a post-render "submit pending" mechanism, persistent always-visible validation for an
untouched dialog, or changes to other dialogs; these are examples, not a blacklist), and whenever the same component has
needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying
the user's request and decisions above, this outline, the current diff and the proposed departure (what changed, why it
is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how a launch attempt on an incomplete setup is caught, when the reason line
appears and clears, how choice controls are marked, and how Restart with's primary action reads live state, and every
review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
