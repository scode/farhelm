# Menus: arrows follow focus moved while arrow presses are landing

Written against main at 78394ca6 on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The session, host, app bar and templates menus remember where the last arrow press was headed, so rapid presses survive
late focus updates (in the desktop apps, a focus move requested by the app reaches the page one script round trip
later). The earlier plan `menu-arrows-follow-focus` (#2176) forgets that destination once the latest arrow move lands.
While arrow presses are still landing, the remembered destination still overrides focus moved by a pointer or a screen
reader. Narrow that window with a small fix, under a hard complexity gate.

TODO.md entry this plan covers (Near term): "**Consider letting menu arrows follow focus moved mid-navigation.** The
session, host, app bar and templates menus remember where the last arrow press was headed, so rapid presses survive late
focus updates. The fix that landed in #2176 forgets that destination once the latest arrow move lands, so focus moved by
a pointer or screen reader wins after navigation settles. While arrow presses are still landing, the remembered
destination still overrides such a move: press Down twice, click the fourth item before both land, press Down, and focus
goes back to the fourth item instead of the fifth. A full fix needs to know where each focus change came from, which the
menus' focus events do not say today. Consider whether such a focus-source design is worth it; it would need its own
scope and acceptance checks."

### Acceptance criteria

- The shared menu code (`crates/farhelm-ui/src/menu_panel.rs`) remembers the one focus request it has in flight. A
  focus-in on any menu item other than that in-flight target is an independent move: the remembered destination is
  dropped and the pending (queued, not yet sent) request is cancelled, so the next arrow steps from the item that has
  focus.
- A burst of arrow presses still advances once per press (the #2176 protection holds).
- Pure unit tests for the decision: a focus-in at the in-flight target keeps the destination; a focus-in anywhere else
  drops it and asks to cancel the pending request; the latest landing drops it.
- A browser test in `e2e/tests/sidebar.spec.ts`, on Chromium and WebKit, labelled as a simulation of the desktop timing:
  inside one `page.evaluate`, dispatch two synthetic ArrowDown keydowns on the focused first item and call `.focus()`
  (not `.click()`, which would run the command) on the fourth item; then assert the fourth item still has focus (this
  catches a missing cancel) and press a real ArrowDown, expecting the fifth. With the session menu's seen-state item
  hidden (`hideSeenState`), the order is Rename, Pin, Clone, Replace with, Replace, Stop, Delete.
- The PR, its changelog fragment and any comment do not claim a full focus-source fix: a request already sent to the
  page cannot be recalled, so if the independent move lands while the second request is on its way, that request still
  pulls focus back and the entry's exact example can still land one item early. Say so plainly.
- The comments that describe #2176's limits in `menu_panel.rs` are updated to the new behaviour.

## Requirement sources

**The user's decisions (2026-10-17):**

- Fix it with the small change, "hard complexity gate, abandon if it turns complicated".
- The gate trips if the fix needs any JavaScript in the product, any production change outside the shared menu code
  beyond one line per menu (`crates/farhelm-ui/src/list/row.rs`, `crates/farhelm-ui/src/hosts.rs`,
  `crates/farhelm-ui/src/app_bar.rs`, `crates/farhelm-ui/src/list/templates.rs`), or more than about 80 changed code
  lines. The count excludes tests (Rust and TypeScript alike), the changelog fragment, the TODO.md removal and any
  SPEC_impl.md sentence; rewritten comments count toward it only where they replace code.
- When the gate trips: stop and deliver one PR that only removes the TODO entry, with the reason in the report. No
  block, no notification.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC_impl.md's row menu "Keyboard" paragraph (arrows step and wrap, Home/End jump,
roving `tabindex`). The #2176 burst protection. Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- An in-flight marker on `MenuFocusQueue`, set just before the focus call and cleared only after its `await`: in Dioxus
  0.7.10's desktop build, webview events reach Rust synchronously and a focus request resolves only after its own
  focus-in has been handled, so the marker is still set when that focus-in arrives. Compare with `Rc::ptr_eq` against
  the target's handle in `record_menu_item_focus`. This costs one field plus one line per menu.
- Do not clear the destination when the queue drains: after the fix a leftover destination always equals the focused
  item, so it is redundant and needs plumbing the queue does not have.
- Leave alone the related gap that focusing the menu's toggle leaves a pending request queued; the entry does not ask
  for it.
- No web test can exercise the in-flight check itself (in the web build focus lands synchronously); only the unit tests
  guard it. An implementation that treated every non-target focus-in as independent would pass the browser tests and
  quietly break the burst protection on desktop, so the unit tests must cover the in-flight case explicitly.

## Outline

One `fix` PR: the marker, the decision function and its use in `record_menu_item_focus`, one line per menu, the comment
updates, the unit tests, the simulated browser test, a changelog fragment (`kind: fixed`) that states the remaining
limit, and the removal of the TODO entry. At most one SPEC_impl.md sentence that arrows step from focus moved
independently, even mid-burst, with the limit.

If the gate trips instead: one PR that only removes the TODO entry (type `docs`, no changelog fragment), and a report
that says what tripped the gate and what was learned.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` `menu_panel` unit tests through the recorder,
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, `python3 releasing/check-changelog.py format`,
and on Chromium and WebKit through the recorder: the new test plus the existing menu keyboard tests (`sidebar.spec.ts`
around its menu arrow tests, `terminal-multihost.spec.ts`'s host menu test, `feedback.spec.ts`'s app bar menu test,
`templates.spec.ts`'s menu tests). Not the full suite. The report says the desktop bridge itself is not exercised.

## Fallbacks

- The complexity gate above, with its agreed outcome.

Examples of departures that need the scope reassessment (and likely trip the gate): JavaScript-side focus handling, a
ledger of pending positions, changing how the queue coalesces requests, clearing on pointer events.

Decisions to log: the marker's shape, the gate count when the PR is done.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-menu-focus-source-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/menu-focus-source/<nn>-<short-name>` (for example `plan/menu-focus-source/01-in-flight-focus`).
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
