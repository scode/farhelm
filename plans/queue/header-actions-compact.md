# Session header actions compact only when short of room

Written against main at 9e3ab60b on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Stop the session header's actions collapsing into their tunnel-pill compact style.** On 2026-11-08 the cluster at
> the right of a session's header (zoom, Stop, Restart ▾, Replace ▾, Clone, Delete) was seen drawn cramped, suspected to
> follow switching from the session view to a host view, though not reproducible on demand. The cramped state is exactly
> the compact block in `crates/farhelm-ui/assets/app.css` (the `.session-topbar:has(> .session-tunnel-pill)` rules near
> line 9583): measured against a normal shot at the same 2x scale, the gap between actions falls from 6px to 2px, each
> button's inline padding from 8px to 0, and the labels from 12px to 11px, while icons and button height do not change.
> That block exists so the header fits its 650px floor while a tunnel pill is present; the pill is rendered by
> `NativeTunnelStrip` in `crates/farhelm-ui/src/tunnels.rs` only while the session has open tunnels, so find out which
> of these holds: the selector matching a pill the user cannot see (a stale tunnel list surviving the view switch, or a
> pill shrunk to zero width, since `.session-tunnel-pill` is `flex: 0 1 auto; min-width: 0`), or a pill that was really
> there, off to the left of the screenshot's crop, with the compact style keyed off pill presence where it should be
> keyed off actual width pressure. Fix the trigger, and add a browser test that the actions keep their normal metrics
> when no pill is visible.

The maintainer confirmed while planning that it was seen in the Mac desktop app.

What is true today, from a read of main at 9e3ab60b (line numbers approximate):

- The compact block is `crates/farhelm-ui/assets/app.css` around 9577-9596: every rule is keyed on
  `.session-topbar:has(> .session-tunnel-pill)`, so it applies whenever a pill is present, at any width. Its comment
  says the authors avoided "viewport breakpoints or a new containment boundary around existing confirmations". The 11px,
  zero inline padding and 2px gap rules landed on 2026-11-07 (the change that made Stop close terminal tabs), one day
  before the sighting; a milder compaction dates from 2026-10-27. The general button padding in the block is `2px` all
  round (normal `2px 8px`, around 2079, and `2px 4px` for the Restart and Replace carets); only the named actions go to
  zero inline padding.
- The block mixes two kinds of rule. The header's `gap: 6px` and the tab strip's `flex: 0 1 auto; min-width: 112px` are
  pill placement, not compaction: they keep the pill beside the last tab at wide widths (SPEC_impl.md around 719), and
  `header.spec.ts` asserts both (a 6px gap between the last tab and the pill at 1200px, the 112px tab floor at 650px).
  The action rules (the actions' gap, the button padding, the named actions' 11px and zero inline padding) are the
  compaction, and the menu-item padding rule exists only to undo the compact button padding, so it follows the same
  condition as they do. SPEC_impl.md around 720 describes the old trigger ("With a pill present, tighter horizontal
  action spacing …").
- The main pane's width also changes when the host rail expands or the Hosts panel opens, so a width-keyed condition
  reacts to those where today's does not.
- The header is `header.session-topbar` in `crates/farhelm-ui/src/session_view.rs` (around 2175), with children
  `.tab-strip`, `NativeTunnelStrip` (around 2273, which renders `TunnelStrip` without a wrapper, so the pill is a direct
  child) and `.titlebar-actions` (around 2281).
- The pill exists only in the native desktop build (`#[cfg(native_desktop)]`, `session_view.rs` around 79-89). It
  renders only when the session's tunnel list is non-empty (`tunnels.rs` around 890) and always has visible content. The
  list is a per-component signal of a `TunnelStrip` keyed by host, inside a `SessionView` keyed by session, so a session
  switch remounts it. Choosing a host in the sidebar only filters the list; nothing touches the header. A failed tunnel
  read keeps the old list (around 738-741), but the pill would still be visible. Tunnels outlive Stop and Restart
  (SPEC.md around 2837).
- So the planning read is that a real, long-lived pill sat far to the left in a wide window, outside the screenshot's
  crop, and presence-keyed compaction did the rest. That is a hypothesis for you to confirm or refute from code, not a
  finding; if you find a path by which an invisible or zero-width pill triggers the style, fix that too.
- Browser tests: `e2e/tests/header.spec.ts` (the preview at 650px around 53, the mocked pill in wide panes in the loop
  around 195-222, the 650px layout with no pill from around 234, action order around 623) and
  `e2e/tests/helpers/native-tunnel-preview.ts` (`mountNativeTunnelPreview`, which injects mirrored pill markup, since
  native tunnels cannot be driven in the browser). No test checks the actions' metrics without a pill or with a pill in
  a wide pane.

### Acceptance criteria

- The header's actions use their normal metrics (12px labels, normal padding, normal gaps) whenever they fit at normal
  size, pill or no pill. The compact action style applies only when the header actually lacks room for this pill, the
  tab floor and normal-size actions, judged from the header's real content (pill text varies), not from a fixed
  worst-case width; and at the 650px floor with a pill everything still fits as it does today. The condition must not
  oscillate: judge the need against normal-size widths, not against the compacted layout it produces.
- The pill-placement rules (header gap, tab-strip sizing) stay keyed on the pill's presence; only the action compaction
  and the menu-item rule that undoes it move to the new condition.
- SPEC_impl.md's description of the trigger (around 720) is reworded to match.
- Menus, the Stop and Delete confirmations, and other popovers anchored in the header still position and size as before
  at every width the existing header tests cover; if the mechanism you choose involves CSS containment on or around the
  header, prove this explicitly, since the block's comment avoided it on purpose.
- Browser tests on the engines `.agents/validation.md` names, WebKit included (the Mac app is WebKit), extending the
  existing tests in `header.spec.ts` rather than adding a new spec: no pill at a wide and at the floor width gives
  normal metrics; a mocked pill in a wide pane gives normal metrics; a mocked pill at 650px still fits with no clipped
  labels, as `header.spec.ts` already asserts.
- The working log and report state what the trigger was and the evidence, including whether the zero-width or stale-pill
  paths were ruled out.
- A `fix` PR with a changelog fragment. The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **Compact only on actual lack of room**, not on pill presence. The alternative was keeping presence-keyed compaction
  and fixing only a different bug if one turned up.
- **Seen in the Mac desktop app**, so the pill path is in play.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the confirmation above, and the decisions above.

**Binding repository constraints:** `docs/desktop-web-triage.md` (read it before investigating; this is a desktop UI bug
report); SPEC.md's session header passages and SPEC_impl.md around 719 ("Content-sized tabs keep the pill beside the
last tab at wide widths"); root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md"; `.agents/test-authoring.md`.

**Planner proposals:** the hypothesis above, and the mechanism: the smallest design found while planning toggles a class
or attribute on the header from a measurement of its width and content (a Dioxus resize handler, or the ResizeObserver
pattern in `crates/farhelm-ui/assets/outage-layout.js`), combined with the existing `:has(> .session-tunnel-pill)`
selector, so the mocked-pill tests keep working and no containment is introduced. Pure flex sizing cannot change font
size, padding or gaps, so it is not an option; a container query is allowed only with the popover proof above.

## Outline

One PR, `plan/header-actions-compact/01-compact-on-pressure`, typed `fix`: the CSS (and any small measuring code), the
browser tests, the changelog fragment, and the TODO.md removal.

### Validation

`e2e/tests/header.spec.ts` and the other specs mounting the tunnel preview
(`e2e/tests/restart-replace-progress.spec.ts`, the docs-shots visual review if `.agents/validation.md` says it applies)
on the engines `.agents/validation.md` names, through `scripts/record-test-run.py`;
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`. Rust checks only if Rust changes.

## Fallbacks

Examples of departures that need the scope reassessment: redesigning the header layout, changing the 650px floor,
changing the pill's own sizing rules beyond what the trigger needs, or changing tunnel lifetime.

Decisions to log: the trigger and its evidence, the mechanism chosen and the alternatives, how popovers were verified,
and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-header-actions-compact-log.md` in the parent directory of the checkout you run in, derived as that section
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
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/header-actions-compact/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
