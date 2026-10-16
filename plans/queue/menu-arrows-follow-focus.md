# Menus: arrow keys step from the item that actually has focus

Written against main at 2ddc1811 on 2026-10-16. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Farhelm's popup menus (the session row's "⋯" menu, the host row's menu, the app bar menu and the templates menu) are
keyboard menus: arrows step and wrap. When an arrow is pressed, the next item is computed from the position of the last
keyboard step the menu itself requested, ahead of the item that actually has focus. That preference exists on purpose:
it fixes a real race where an older, still in-flight focus request lands after a newer key press and would otherwise
make a burst of presses repeat a step. But the item focus handlers never clear that remembered request, so when focus
moves inside the open menu some other way (a pointer press that focuses an item without activating it, or a screen
reader moving focus), the next arrow steps from the old position instead of the focused item. In the session menu that
can move focus to Delete unexpectedly; deleting still needs Enter or Space on it.

The maintainer chose `fix code` on 2026-10-16, on the condition that it is fixable at the GUI level, which it is, with
care to keep the burst fix.

Carry out this triage outcome, recorded under its heading in `TRIAGE_OUTCOMES.md` (read it in full first):

1. `menu-request-overrides-independent-focus.md` (`fix code`)

### Acceptance criteria

- After focus moves to a menu item independently (not as the landing of a request the menu itself made), the next arrow
  steps from that item, in every menu that uses the shared menu wiring.
- A burst of arrow presses still advances once per press, including when an older focus request lands after a newer
  press.
- Pure unit tests in `crates/farhelm-ui/src/menu_panel.rs` for: a stale focus-in mid-burst keeps the remembered request;
  the latest request's own landing clears it; an independent focus-in after that makes the next step start from the
  focused item.
- A changelog fragment (`kind: fixed`) per root `AGENTS.md`.

## Requirement sources

**The user's decisions (2026-10-16):** `fix code` if it can be fixed easily at the GUI level. Review gate: a single
fresh-context gpt-6.1-sol reviewer at high effort, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC_impl.md's row menu "Keyboard" paragraph (arrows step and wrap, Home/End jump,
roving `tabindex`). The outcome's complexity gate in `TRIAGE_OUTCOMES.md`: confined to `menu_panel.rs` and the menu
wiring in the components that use it; if it needs a new cross-component focus mechanism or regresses the burst
behaviour, stop and return the item. Root `AGENTS.md` "Tests and validation" and its changelog rule.

**Planner proposals (checked by a fresh-context planning review):** clear the remembered request (`requested` in
`MenuWiring`) when an item focus-in arrives at the requested position itself, meaning the menu's latest request has
landed; a focus-in at any other position only updates the focused position, as today. This keeps the burst fix because
the menu's focus queue (`MenuFocusQueue`) keeps at most one focus request in flight and awaits each before the next, so
focus-ins arrive in request order and a stale one always precedes the latest request's own. Once the request has landed,
any later focus-in is independent and the existing `requested.or(focused)` fallback steps from it. Express the decision
as a pure function (for example `requested_after_item_focus(requested, position)`) because the module's impure handlers
cannot run under a headless `VirtualDom`; route every item `onfocusin` handler in the four menu users through one helper
that applies it, rather than each hand-coding it. Rejected by the planning review: tracking a set of pending requested
positions (the focus queue coalesces requests, so an overwritten request never lands and would stay "pending" forever);
clearing only on a pointer press (misses screen-reader focus moves, so it does not meet the acceptance criteria).

## Outline

`menu_step_origin`, `focus_menu_item`, `remember_menu_item` and the new pure decision function in
`crates/farhelm-ui/src/menu_panel.rs`, and the item `onfocusin` handlers in the four components that build a
`MenuWiring`: `crates/farhelm-ui/src/list/row.rs`, `crates/farhelm-ui/src/hosts.rs`, `crates/farhelm-ui/src/app_bar.rs`
and `crates/farhelm-ui/src/list/templates.rs` (the bell menu does not use this wiring). Update the doc comments on
`menu_step_origin` and `MenuWiring::requested`, which explain the precedence, to describe when the request is cleared
and why. One PR.

Known residue to decide on, not a reason for more machinery: a request whose target already holds DOM focus (Home
pressed on the first item, a one-item menu) produces no focus-in, so the request stays set until the next landing. A
narrow guard, or accepting the residue and noting it in the report, are both proportionate; log the DECISION.

### Validation

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` `menu_panel` unit tests through the recorder,
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and
`python3 releasing/check-changelog.py format`. Because the change rewires focus handlers in four components, also run a
selected browser subset through the recorder on Chromium and WebKit per `.agents/validation.md`: at least
`e2e/tests/sidebar.spec.ts` and `e2e/tests/templates.spec.ts`, plus whichever specs exercise the host row menu and the
app bar menu by keyboard (find them by searching the specs for those menus' arrow-key use). Not the full suite.

## Fallbacks

None needed.

Examples of departures that need the scope reassessment: a new cross-component focus tracking mechanism, a per-position
ledger of requests, or changing `MenuFocusQueue`'s coalescing.

Decisions to log: the pure function's shape, and how the no-focus-in residue is handled.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-menu-arrows-follow-focus-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md`
(Executing one plan, step 7) before any work.

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

- Follow root `AGENTS.md` "Execute triage outcomes": one reviewable commit, stable bookmark and draft PR per outcome, in
  a single linear stack in the order given under The goal above. Each PR carries its outcome's agreed change, the
  removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution
  update. Record the jj change ID and bookmark in that Execution field before creating the PR, then add the PR's URL
  (with `/changes` appended, per the user's PR-link rule) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/menu-arrows-follow-focus/<nn>-<short-name>` (for example `plan/menu-arrows-follow-focus/01-focus-landing`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits. Type `fix` for the outcome's PR, with its changelog fragment
  (`kind: fixed`) in the same commit, per root `AGENTS.md`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above. If current code or specs invalidate a recorded triage decision, that is a block: root `AGENTS.md`
(Execute triage outcomes) returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless the
acceptance criteria above say otherwise, that together meets the acceptance criteria above, each PR passed its review
gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID, bookmark and PR. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.

## Blocked

Blocked on 2026-10-16 (claim 024149).

### Menu arrows need a scope decision

The session, host, app bar and templates menus remember the destination of the last arrow press. That protects rapid
arrow presses from delayed focus changes, but the remembered destination can override focus moved by a pointer or screen
reader. The next arrow can then start from the wrong item, unexpectedly reaching an action such as Delete (activation
still requires Enter or Space).

Triage authorized an easy GUI fix, with the rapid-arrow protection preserved. The proposed fix clears the remembered
destination when the latest keyboard move lands. It fixes independent focus moves after keyboard navigation settles, but
source review found that it still ignores independent focus while keyboard moves are pending. For example, press Down
twice from the first item, independently focus the fourth before those requests land, then press Down: focus is
requested on the fourth again instead of the fifth.

The existing focus event does not identify its source. Even remembering the one in-flight keyboard destination cannot
always distinguish its delayed arrival from an independent move to that same item. Clearing the remembered destination
on every focus event would bring back the rapid-arrow bug. A full guarantee therefore needs a broader focus-source
design than the plan's simple acknowledgement rule. That design has not been implemented or validated.

### Decision needed

Choose between accepting the narrower fix (independent focus wins after keyboard navigation settles, with the pending
window still affected), authorizing a separate design for independent focus during pending keyboard moves as well, or
leaving the menus unchanged. The broader option needs revised scope and acceptance checks before implementation; it is
not a completed fix waiting for permission to land.

I recommend accepting the narrower fix as a bounded improvement if the remaining pending-window behavior is acceptable.
Otherwise, authorize the separate design. The full guarantee cannot be claimed for the simple fix.

### Current state

No PR was opened and no code was pushed. The prototype is retained privately for a later executor; the feedback item and
its triage record remain unchanged. Its tests cover stale and settled focus, not the reviewer's counterexample. Browser
validation was deferred because the accepted behavior is unresolved. The plan will resume after this decision is
recorded.
