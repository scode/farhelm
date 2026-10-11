# Show on the version readout that "check for updates" is working, and how it ended

Written against main at 3d97d46b on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Show that "check for updates" is doing something": in the desktop app, clicking "check
for updates" in the `?` menu gives no visible sign that a check started or is still running. You wait, and maybe the
version readout turns red. The check's outcome (up to date, installing, update ready, or failed) is only shown in the
readout's hover, which is easy to miss.

Acceptance criteria:

- While a check the user started is running, the version readout in the sidebar's top bar shows it inline: a small
  spinner and "checking…" beside the version, and "installing…" while the update it found installs.
- When that check ends:
  - up to date: the readout shows "up to date" inline for about five seconds, then returns to the plain version;
  - failed: the readout shows "check failed" inline with a warning look, and keeps showing it until the next check
    starts or the user clicks it; the reason stays in the hover, as today;
  - an update was installed: the red update-ready readout takes over, unchanged from today;
  - up to date while an update is already waiting: nothing inline, since the hover already lets the update-ready text
    win and "up to date" beside a red readout would contradict it.
- A failed restart to update (`Activity::RestartFailed`) is not a check outcome: it shows nothing inline and stays in
  the hover, as today.
- The hover wording stays as it is today for every state.
- Automatic background checks stay invisible, as SPEC.md requires; only a check the user started (from the `?` menu or
  the local host row's Update item), or an automatic one a user's request joined, shows anything. The web UI is
  unchanged.
- The spinner respects reduced motion, as the existing spinners do. The inline text is designed to fit the sidebar's top
  bar at its narrowest supported width (a bounded width with an ellipsis, or equivalent CSS) without pushing other
  controls out of place. Nobody can see it on the executing machine, so the report asks the maintainer to check it on a
  Mac.
- Screen readers hear the start and the outcome through a polite live region.
- SPEC.md "Installation and updates" and SPEC_impl.md "The desktop app's updater" describe the inline progress and
  outcome, in the same PR.
- Unit tests of the pure functions for every `Activity`, with and without an update waiting and with the reinstall
  warning, including when the inline outcome expires and what dismisses a failure.
- One draft PR, passing its review gate, with its changelog fragment, and removing the TODO.md entry.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-11):** plan this TODO entry, through the todo planning flow.

**The user's decisions (2026-10-11):**

- The feedback goes on the version readout itself: a spinner and "checking…" (or "installing…") while it runs, then the
  outcome inline for a while before it returns to the plain version; the red update-ready readout is unchanged. (Offered
  and not chosen: progress in the `?` menu item with the menu kept open; a new toast component.)
- How long the outcome stays: "up to date" for about five seconds; "check failed" stays, with its warning look, until
  the next check or a click on it, with the reason in the hover as today. (Offered and not chosen: five seconds for
  every outcome; every outcome until the next check.)
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Installation and updates": "A check started this way always ends in a visible state, shown in the readout's
  hover: up to date, installing, update ready (the red readout), or the check failed, with the reason." "Without an
  update waiting, the readout behaves as it always has, apart from showing the result of a check the user started in its
  hover." Automatic failures are not shown, except the reinstall warning. "None of this applies to the web UI". These
  sentences change to describe the inline progress and outcome; the rest stays.
- SPEC_impl.md "The desktop app's updater": one published state (idle, checking, installing, up to date, failed); "Only
  a run the user started, or an automatic run that a user's request joined, shows its progress and outcome there"; "The
  readout renders it through pure functions (its class, glyph and hover text)." Keep that shape: the new inline text and
  its class come from pure functions too.
- SPEC_impl.md "Version and skew": the readout's hover selection, including build skew. Do not disturb it.
- Root `AGENTS.md`: Conventional Commits; Finishing work (the desktop compile and desktop Rust targets); Releases and
  the changelog; Agent scratch space; Sharing the machine with other agents.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism and test shapes in the outline below, checked against main and corrected by a
fresh-context planning review.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-ui/src/desktop/updater.rs`:
  `Activity { Idle, Checking, Installing(String), UpToDate(String), Failed(String), RestartFailed }` inside
  `UpdaterState`, published on a `tokio::sync::watch` channel. `check_now` marks the user as waiting and publishes
  `Checking` at once; automatic runs stay `Idle` unless a user's request joined them. A successful install ends as
  `Idle`, and the installed version makes the readout red. `readout()` and `activity_readout()` turn the state into
  `crate::app_updater::Readout { update_ready, needs_reinstall, tooltip }`. The outcome stays in the published state
  until the next run starts.
- `crates/farhelm-ui/src/app_updater.rs`: `Readout`, `ReadoutMark` (`WARNING_MARK`, `UPDATE_MARK`), `readout_marks`,
  `version_tooltip`, `idle_tooltip`; `use_app_updater()` copies each published state into a component signal. In the web
  build `AppUpdater` is empty, so nothing here can reach the web UI.
- `crates/farhelm-ui/src/app_bar.rs`, `AppBar`: the `?` menu's `BarMenuItem::CheckForUpdates` calls
  `AppUpdater::check_now()` and closes the menu. The readout is a plain `span.app-version` (grey;
  `.app-version-unverified` with the warning), or, while an update waits, the update menu's toggle
  (`.app-version-update`, red). A visually hidden `p.feedback-notice role=status` already exists in the bar for the
  feedback thanks message.
- `crates/farhelm-ui/assets/app.css` styles the readout. Spinners exist as `.delete-spinner` (used in `list/row.rs` and
  `session_view.rs`), with a reduced-motion rule.
- Tests: `updater.rs` has a `Rig` with a fake probe, install, clock and installed record (for example
  `an_on_demand_check_ends_visibly`, `the_readout_hover_carries_a_user_started_check`); `app_updater.rs` tests the pure
  tooltip and mark functions. No browser test can reach the updater: it runs only in the installed Mac app bundle.

Here's the work:

1. **Inline status as a pure function.** Extend `Readout` (or add a sibling) with the inline status: none, checking,
   installing, up to date, failed. Derive it from `Activity` next to `activity_readout`: `Idle` and `RestartFailed` give
   none, and `UpToDate` gives none while an update is waiting. It never shows for automatic runs, because the published
   state already hides those (`run_check` publishes outcomes only for a run a user is waiting on).
2. **Expiry and dismissal in the component.** The published state keeps `UpToDate` and `Failed` until the next run
   starts, so expiry lives in the UI. One component signal holds the activity whose inline status is suppressed: the
   five-second timer sets it to that `UpToDate(v)`, a click on "check failed" sets it to that `Failed(reason)`, and it
   is cleared whenever the published activity differs from it. Key the timer and the reset on the activity, not on every
   published change: `UpdaterState` also changes when the installed record does, and resetting on that would bring an
   expired "up to date" or a dismissed failure back, or stretch the five seconds. The pure function takes the activity,
   whether an update is waiting, and the suppressed activity; the component only holds the timer and the click. Whether
   the click target needs a button element for accessibility is yours to decide; log the DECISION.
3. **Rendering, preferred shape.** One element placed right after the readout, mounted whenever an updater exists, with
   `role="status"`: its visible content is the spinner (reuse `.delete-spinner` or a sibling class with the same
   reduced-motion rule) and the pure function's short words, with "check failed" in the existing warning colour. It
   stays mounted while empty, because a live region inserted with its text already in place is announced unreliably
   (`list/trash.rs` records this for WebKit). Being a sibling, it sits beside both the plain readout and the red
   update-ready toggle without a second copy, and being the live region itself, it needs no hidden duplicate. Do not
   share the bar's `p.feedback-notice`, or the feedback thanks and the update status would overwrite each other. If the
   layout needs the text inside the readout instead, do that and log the DECISION.
4. **SPEC.md and SPEC_impl.md** as above.
5. **Tests.** Unit tests in `app_updater.rs` (and next to `activity_readout` in `updater.rs` if the mapping lives there)
   for the inline status of every `Activity`, with and without an update waiting and with the reinstall warning, and for
   the expiry and dismissal function. No new `Rig` tests: existing ones already prove that automatic runs publish
   nothing, and this change does not touch the worker or `Shared`; if it ends up doing so, that is a change to the
   published state machine and needs the scope reassessment.
6. **Bookkeeping.** Changelog fragment, `kind: changed`. Remove the TODO.md entry.

Nothing here can be seen running on this machine: the updater only runs in the installed Mac app. Say so in the report,
and say what the maintainer should look at on a Mac (the readout while checking, after "up to date", after a failure, at
the narrowest sidebar width).

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, which is the only clippy run that lints
  the desktop updater code (the plain runs build the UI crate without the desktop feature);
- `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop`;
- the desktop Rust targets through the recorder (`cargo nextest run -p farhelm-ui --features desktop`), and
  `-p farhelm-ui` without the feature if shared code changed;
- `dprint check` on changed Markdown, and `python3 releasing/check-changelog.py format`.

No browser tests: the web UI is unchanged and cannot reach the updater. Do not run the full battery without a specific
risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-update-check-feedback-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/update-check-feedback/<nn>-<short-name>` (for example `plan/update-check-feedback/01-inline-status`).
- One PR. Keep it focused and avoid churn: code added in one PR and deleted in a later one means the stack should have
  been shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and
  that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`feat`). A `feat`
  PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

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

The PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that automatic runs never show anything new;
- that the expiry timer cannot hide a later state (a new check started within the five seconds);
- that the reinstall warning, the update-ready toggle and the build-skew hover all still show as before;
- that nothing changes in the web build.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a change to the updater's published
  state machine, a toast or notification component, a desktop-only test seam that renders updater states in the web
  build, or a change to automatic checks (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- where the inline status and its expiry live, and how they are kept pure;
- the exact inline words and the spinner;
- how a failure is dismissed by a click, and the element it uses;
- what shows beside the red update-ready toggle;
- how the text fits the narrowest sidebar;
- where the status element sits and its live-region wording;
- the SPEC.md and SPEC_impl.md wording;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: showing automatic checks, a toast, keeping the
`?` menu open, changing the hover wording, or making "up to date" stay longer or failures expire on a timer.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
