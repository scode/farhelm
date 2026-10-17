# Name a quick-launched session while it starts

Written against main at 78394ca6 on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `quick-launch-chords.md`: it may only start once that plan's work is on main, because the naming
dialog must open once after a composed launch and never between the keys of a launch sequence. Read what that plan
landed (its key sequence and the generalized quick-launch path) before you start.

## The goal

When a quick launch starts a session without ever showing New, open the rename dialog over the new session right away,
so the user can name it while the agent is already starting. It is a rename after the launch, never a step the launch
waits for. Pressing Enter keeps the default name.

TODO.md entry this plan covers (Near term): "**Name a session while it starts.** Start a session with a rename dialog
open, so the session is already starting while you type its name. Make this the default for quick launches, where just
pressing Enter immediately accepts the default name. Details TBD at plan time."

### Acceptance criteria

- The dialog opens for every quick launch that never showed New: ⚡, the number keys, composed launch sequences, and the
  quick switcher's quick-launch row. A launch that fell back to New (a refusal, the YOLO question, an incomplete result)
  gets no dialog, because New already showed the name field.
- The launch is never held back: the session is created and the agent starts while the dialog is open. The terminal
  takes no keystrokes while the dialog is open.
- The field holds the name the session got (the folder name, `repo-N` for a new managed checkout, or the template's
  name), fully selected and focused, so typing replaces it. A template that sets a name still gets the dialog.
- Enter with the name unchanged, or Escape, closes the dialog and sends nothing. Enter with a changed name renames the
  session as the rename dialog does today.
- Every way of closing the dialog hands focus to the new session's terminal.
- A Settings switch, beside "skip session confirmations", turns the behaviour off; it is on by default, including on
  installs that have never set it. With it off, quick launches behave exactly as today.
- A managed checkout's folder keeps its `repo-N` name when the session is named this way; SPEC.md says so.
- SPEC.md (Creation's quick-launch bullet, Lifecycle operations' rename, Managed checkouts, the Session list paragraph
  that lists the shared preferences and describes the Settings dialog), SPEC_impl.md (quick-launch paragraph,
  preferences list, the "Helm schema N adds ..." paragraph) and the docs describe the behaviour.

## Requirement sources

**The user's decisions (2026-10-17), in their words where given:**

- "make sure the naming here is _while the session is starting_ so it's a rename, not blocking making progress on agent
  spawn."
- Flows: every quick launch that never showed New (⚡, number keys, the switcher's quick-launch row; composed sequences
  via the dependency on `quick-launch-chords.md`).
- A Settings switch, on by default, beside "skip session confirmations".
- Escape or an unchanged Enter keeps the default name; the field is pre-filled with the session's name, selected; a
  template's own name still prompts; the checkout folder keeps its name and the spec says so.
- No downgrade path: the user accepted on 2026-10-17 that adding the preference moves the helm database to a new schema
  version, after which an older helm refuses it ("Accept, as for earlier settings").
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC.md "Upgrade compatibility and client scale" (the no-downgrade notice above
satisfies its tell-the-maintainer rule for this preference); the shared-preference pattern in the helm (every shared
preference is a column of the helm's preferences row). Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- Copy the preference pattern of "skip session confirmations" and the welcome-card setting. Store it so that unset reads
  as on: an upgraded helm starts with no value, and a positive column read as off would ship the feature disabled for
  every existing install. Take the next free helm schema number when you run, not a number written here
  (`quick-launch-chords` or another plan may have bumped it).
- The created session carries one yes/no fact, "New was never shown", taken from the hidden quick-launch state when the
  create succeeds (`crates/farhelm-ui/src/list/create_form.rs`: `quick_launch_hidden`, `quick_origin`). No enum of
  launch sources.
- Naming mode is one flag on the existing rename editor (`crates/farhelm-ui/src/rename.rs`, opened through
  `on_rename_start` in `crates/farhelm-ui/src/list/view.rs`). In that mode: compare the submitted title with the current
  one and send nothing when they match (normal rename keeps today's behaviour); select the field's text on open and
  focus the field explicitly (the dialog's focus trap only pulls focus in from the page body or the row's rename
  control, and native autofocus works once per page); on close hand focus to the terminal with
  `window.farhelmTerm.focusSelected()` instead of the row's menu button. Open the dialog in the same step that selects
  the new session in `on_created` (`view.rs`), skipping `focus_new_session_button()`, so the dialog is there before the
  terminal can take focus.
- No new input blocking: the terminal already refuses focus while any modal dialog is open (`keyboardHeldElsewhere` in
  `crates/farhelm-ui/assets/terminal.js`); test it rather than adding code.

## Outline

1. `feat`: the preference and its Settings switch, schema bump, spec and SPEC_impl text, changelog fragment. Tests: the
   helm store's preference tests, `e2e/tests/settings.spec.ts`.
2. `feat`: naming mode and the quick-launch hook, spec and docs, changelog (edit the first PR's fragment if it describes
   the same feature). Tests: `e2e/tests/quick-launch.spec.ts` updated for the dialog (it appears; Enter keeps the name
   and sends nothing; typing renames; the terminal has focus after; no bytes reach the agent while it is open); no
   dialog after a revealed New (YOLO, refusal); the switcher row; one assertion for a composed sequence; the setting off
   restoring today's behaviour. Existing quick-launch tests elsewhere (notably `e2e/tests/github-checkouts.spec.ts`)
   press Enter or Escape on the dialog rather than turning the setting off. Unit tests for the decision helper. Removes
   the TODO entry.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the touched crates' unit tests through the recorder,
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, `python3 releasing/check-changelog.py format`,
and a selected browser subset on Chromium and WebKit through the recorder (`quick-launch.spec.ts`, `settings.spec.ts`,
`terminal-replay-rename.spec.ts`, `modal-focus.spec.ts`, `ime-composition.spec.ts`, `github-checkouts.spec.ts`, and the
specs you changed). Not the full suite.

## Fallbacks

None needed beyond the general ones.

Examples of departures that need the scope reassessment: holding the create until a name is typed, an inline name field
outside the rename dialog, renaming the managed checkout's folder, per-template naming settings.

Decisions to log: the preference's storage shape and default handling, how naming mode is threaded through the editor.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-name-session-while-starting-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/name-session-while-starting/<nn>-<short-name>` (for example
  `plan/name-session-while-starting/01-naming-preference`).
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
