# UI: bind every GUI answer to what the user saw

Written against main at f3848b87 on 2026-10-21. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

It runs after `popover-anchor-close.md`, which was complete and waiting to land when this was written. That plan routes
closing the sidebar rows' anchored popovers through one owner, and this one splits the same rows' inline prompt state
out of their operation state; built at the same time, the two would conflict in the sidebar row code.
`gui-saves-last-write-wins.md` also edits `crates/farhelm-ui/src/list/templates.rs` (number-key saves, not Delete);
expect a rebase there if it lands first.

## The goal

This plan is the TODO.md entry "Bind GUI answers to what the user saw" (Near term). Read that entry in full first; it is
the maintainer's statement of the problem. In short: review keeps finding GUI handlers (buttons, keys, gestures,
deferred submits) that act on whatever state is current when they run rather than on what was on screen when the user
acted. Each instance is a narrow window nobody has reproduced, but the guards involved are safety ones (YOLO questions,
destructive confirmations, link destination disclosure), and the surfaces that bind correctly today each invented their
own way. The goal is to make binding structural, so that the obvious way to write a handler is the correct one:

1. **One shared prompt mechanism** for "answer exactly what was shown". Every question, confirmation or destructive
   prompt uses it: opening a prompt mints a fresh opening number; what the prompt displays (its target, any precondition
   its wording promises) is captured from the render that drew it; its buttons carry the opening, and an answer whose
   opening is not the current one does nothing. Act-on-current entry points cease to exist.
2. **Destructive preconditions travel from the click to the request**, never recomputed at send time.
3. **Declarative link covers**: every surface that draws above a terminal link's destination preview marks itself as a
   cover, and the cover check measures marked surfaces instead of a hand-kept selector list.
4. **Held input is retired when its owner stops observing**: a held link press is cleared when the terminal tears down
   for reconnect, and a captured quick-launch Enter snapshot is consumed or discarded by exactly the dispatch it was
   captured for, including when that dispatch is refused.
5. **A written rule**: SPEC_impl.md states the principle, and the review instructions tell reviewers to flag a handler
   that reads live state for a user-confirmed action.

### The six folded triage outcomes

Six findings from the 2026-10-21 triage were folded into the TODO entry and removed from the review queue. Their
`TRIAGE_OUTCOMES.md` entries (search for each heading) hold the evidence and the fix each needs, and the plan is not
done until every one is fixed with the regression test its entry names. Their ~100-line size gates do not apply here:
they were sized for separate small fixes, and this plan replaces that. Record in each entry's Execution field the PR
that fixed it, appended to what is there.

- `guarded-preview-widens-source-deletion-consent.md`: a confirmed SPEC.md violation that can delete a running session.
  First PR.
- `superseding-launcher-may-retain-old-replace-source.md`: the same handler (`on_launch_guarded`) and the same
  `GuardedSubmitStamp`; in the first PR or directly after it.
- `header-yolo-answer-may-cross-question-opening.md`: the header's YOLO buttons answer whatever question is current.
  `replace.rs` documents this as deliberate ("Header answers intentionally use the current opening instead"); the
  maintainer decided to reverse it. Replace that comment.
- `launch-pill-can-cover-link-disclosure.md`
- `restored-link-press-misses-intervening-drag.md`
- `refused-sequence-displaces-later-binding-snapshots.md`

### You find the instances

A read-only survey and a planning review on main at a2848cf2 found what follows (paths under `crates/farhelm-ui/`).
Treat it as a starting point, not a worklist; other work keeps changing this code. Your first step is your own inventory
on the main you start from: every prompt surface and how its answer binds; every floating surface and its layer; every
piece of held input state. Record it in the working log, re-check it after every rebase, and put it in the report as a
table (surface, how its answer binds now, the PR that changed it or why it did not change), so a reviewer can audit it
against the code.

- **The existing shared helper.** `ConfirmSlot<K, P>` (`src/ops.rs`, around 276–410): `take(&key)` tests and clears in
  one step. Seven production users (`restart_yolo`, `confirming_replace`, `confirming_header_replace`,
  `confirming_header_delete`, `confirming_close`, `yolo_refusal`, `offer`); three are keyed by `()`, which identifies
  only "a prompt is open". The planning review's proposal, which this plan adopts as its starting design: `open` mints
  the opening number inside the slot, the render reader returns a token (opening plus what is shown), and `take` and
  `cancel_for` require that token. All users then move in one mechanical PR, and `RestartYoloQuestion.opening` /
  `restart_yolo_openings` and `LauncherYoloQuestion.opening` fold into the slot.
- **Prompts outside it today**, which move onto it:
  - the Replace question model (`ReplaceController`, `src/replace.rs`): the sidebar uses `answer_stamped` /
    `cancel_stamped`; the session header uses `answer_current` / `cancel_current`, which go away;
  - the sidebar rows' inline Delete and Replace questions (`RowPhase` in `src/list/view.rs`). The same state also keeps
    a second operation from starting on a row while one runs (`Pending`). Split the two jobs: the prompt part moves onto
    the shared mechanism, the operation exclusion stays row state. Today these capture target and guard from the render
    already (`confirm_delete` around 2152, `confirm_replace` around 2318); they lack only opening identity;
  - the header Restart confirmation (a plain boolean, `src/session_view.rs` around 631 and 2311–2335). It is hand-rolled
    for a reason in its comment: its claim is shared with Restart With, which keeps it after a failed attempt. Preserve
    that shared-claim behavior when moving it;
  - the trash dialog's band (`Confirmation` in `src/list/trash.rs`), which already compares exactly what it displayed;
    it moves for consistency. Trash Cancel can only dismiss a prompt; "delete all on host" reads live state only to open
    the band, and the band submits what it froze;
  - host Remove (`src/hosts.rs`, reading the open dialog's id at click), the provisioning Update / Uninstall / Setup
    confirmations (`src/provisioning.rs`, reading the live pending plan instead of the `probe_id` they rendered), and
    tunnel consent Allow (`src/tunnels.rs`, reading the live warning). These bind to what they rendered: the host
    entry's id, the `probe_id`, the warning's source. **Not** to a host's installation identity: SPEC.md (around
    217–223, "Confirmed 2026-10-10") says confirmed operations follow the host entry and Farhelm adds no
    installation-identity check.
- **Not prompts; they keep their own mechanisms** (decided, see below): the launcher's `opening_generation`
  (`src/list/view.rs`, about 9 increments, 4 consumers), which identifies a launcher-window opening for deferred work;
  and the approval cards' `arm_generation` (`src/approvals.rs`, around 84–151), which times the 700 ms arm pause SPEC.md
  names, while answers are already bound by the helm's request id. They must still satisfy the rule: nothing reads live
  state to decide what an answer acts on.
- **The launcher's Guarded path.** `on_launch_guarded` stamps the live `*opening_generation.peek()` and the deferred
  submit recomputes `replace_guard`; capture both from the render into `GuardedSubmitStamp`. The YOLO answer's immediate
  resubmit already sends what is on screen at the answer click (the Replace warning sits right above the question and
  `replace_guard` comes from that render), so it is not in scope unless you find a concrete gap, logged as a DECISION.
- **Template Delete** is a one-click delete with undo, not a prompt, but its label (`permanent_delete` around
  `src/list/templates.rs` 1394) comes from the render while the handler recomputes `restorable` from the live list. In
  scope as a tiny fix: use the kind the label showed. Trash per-entry Delete (reads the host at click by design) and the
  sidebar Stop have no confirmation and stay out of scope.
- **Link covers.** `linkCoveringRects` in `assets/terminal.js` (around 3618) measures only
  `'.approval-cards, .tunnel-error-notice'`; the check is a plain rectangle overlap with no notion of paint order. The
  destination preview (`.terminal-link-target`) is at layer 40 (`assets/app.css` around 1328). Surfaces at or above it:
  approval cards and the tunnel error notice (45, already covers), the quick-launch pill `.launch-sequence-indicator`
  (50), the hover tooltip `.farhelm-tooltip` (50), `.checkout-trash-flight` (10000), and modal backdrops (40–50), whose
  terminals are probably inert already; verify. Surfaces below it (menus and flyouts 20, `.drag-copy-notice` and
  `.templates-number-flyout` 25, `.header-confirm` and `.copy-warning` 30, `.host-update-popup` 15) cannot hide it and
  are not covers. Verify the paint order in a browser first: no ancestor stacking context was found.
- **The refusal explanation** (`.terminal-file-message[data-outcome="disclosure-refusal"]`, layer 46) must draw above
  every cover. With covers only at or above 40 it needs to outrank 50, which does not collide with the rule that a
  header confirmation's consequence text is never covered (header confirmations are at 30, not covers). If the trash
  animation can overlap a terminal at all, exempt it from that requirement rather than raising the refusal above 10000.
- **Held input.** The link gesture (`createLinkGesture`, `assets/terminal-links.js`) has no reset and survives reconnect
  (`assets/terminal.js`, `runReconnect` / `cancelReconnect`). The quick-launch Enter snapshots are a FIFO
  (`assets/template-shortcuts.js`) that the Rust relay (`src/list/view.rs`) consumes only when its own queue is
  non-empty.

### Acceptance criteria

- Every prompt surface in the inventory answers through the shared prompt mechanism, bound to the opening and content it
  rendered; a stale answer does nothing. No act-on-current entry point remains (`answer_current`, `cancel_current`,
  `()`-only slot keys, handlers that read the open dialog to decide what to act on). The report's inventory table shows
  this surface by surface.
- The launcher counter and the approval-card timer stay separate mechanisms, documented as such, and nothing in them
  reads live state to decide what an answer acts on.
- Destructive preconditions are captured at the click and sent as captured through the Guarded deferred submit.
- Every surface that draws at or above the destination preview's layer is a link cover by marking itself; the layer
  ladder in `assets/app.css` lists every layer and states the marker rule next to the preview's entry; the cover check
  measures marked surfaces. The refusal explanation's layer is above every marked cover (trash animation exempted if it
  applies), checked by one browser test that enumerates the marked elements' computed `z-index`.
- The two held-input cases are retired as described.
- The six folded outcomes are each fixed with the regression test their entry names, and their Execution fields record
  the fixing PR.
- SPEC_impl.md states the principle; the review instructions carry the reviewer rule.
- Unit tests cover the shared mechanism. Behavior changes are in `fix:` PRs with changelog fragments; where a fix has no
  user-visible effect (most opening-identity changes), the fragment is `kind: none` with the reason. Restructuring PRs
  change no behavior.
- The last code PR removes the TODO.md entry "Bind GUI answers to what the user saw".
- Docstrings explain the mechanism's contract (what a token captures, when an answer is refused, what callers must not
  do) and why the two non-prompt mechanisms are separate, per the repository's documentation norms.

## Decisions already made

The maintainer's answers when planning this (2026-10-21):

- **Principle: the end state matters far more than churn.** In the maintainer's words: "I care a lot more about the end
  state being better than the amount of churn." Resolve churn-versus-structure forks toward the better end state. Keep a
  surface on its own mechanism only when merging it would make the end state worse (for example by conflating two
  different concepts), and say which in the DECISION.
- **Scope: every surface.** The shared mechanism, every prompt surface, the launcher's Guarded stamp, the link covers,
  both held-input cases, the six folded outcomes and the newly found instances.
- **Every prompt moves onto the shared mechanism, including the trash band and the sidebar row questions** (the rows'
  prompt state split from their operation exclusion). **The launcher's opening counter and the approval cards' arm timer
  stay separate**: neither binds an answer to a prompt; one identifies a launcher-window opening for deferred work, the
  other times a safety pause SPEC.md names. This narrows the TODO entry's "migrate the existing ad hoc stamps" for those
  two only.
- **A stale answer does nothing; the current prompt stays.** No notice.
- **Covers: every surface that draws above the destination preview, tooltips included.** A tooltip refuses a link only
  when it actually hides part of the destination. (The maintainer first chose "every floating surface except tooltips";
  a planning review found most of those surfaces draw beneath the preview and cannot hide it, and the maintainer chose
  this instead.)
- **Enforcement: API shape plus written rules.** No act-on-current calls exist to reach for; SPEC_impl.md states the
  principle; the review instructions (`review_feedback_queue/.agents/recording.md` and the reviewer prompt it carries)
  tell reviewers to flag handlers that read live state for a confirmed action. No check script.
- **Reviewer:** a single fresh-context Opus 5.5 agent at high effort with the general charter, no swarm (see Review
  gate).
- **The header YOLO reversal** was decided in triage (see its `TRIAGE_OUTCOMES.md` entry).
- No-workhorse mode, which `plans/.agents/planning.md` requires.

## Requirement sources

**The user's request and decisions:** the TODO.md entry, the decisions above, and the six folded outcomes' entries.

**Binding repository constraints:** SPEC.md's destructive-confirmation rules (a confirmation authorizes only what the
prompt the user answered said would happen; Replace with carries the click-time precondition), its host-entry rule
(confirmed operations follow the entry; no installation-identity check), its link-disclosure rule, the approval cards'
700 ms pause, and its sub-second acceptance ("Confirmed 2026-10-10"), which still holds: this plan makes the explicit
guards it preserves hold by construction, not by changing that acceptance; SPEC_impl.md's GUI passages on these
surfaces; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
FLAKES.md", "Code rules"; `.agents/test-authoring.md` for tests; `review_feedback_queue/AGENTS.md` for its instruction
files.

**Planner proposals:** the outline below and the slot design above, both from the planning session and its fresh
planning review. A smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

A suggested stack; reshape it if the inventory says otherwise:

1. `fix:` the launcher's Guarded path: `GuardedSubmitStamp` captures the rendered deletion precondition and opening
   (`guarded-preview-widens-source-deletion-consent.md`, then `superseding-launcher-may-retain-old-replace-source.md`,
   one PR each or together if they review better together), with fragments and their tests.
2. `refactor:` the shared prompt mechanism: `ConfirmSlot` mints openings and requires the rendered token; its seven
   users move in the same PR, with unit tests. Any slot API left without callers is deleted in this PR (clippy runs with
   `-D warnings`, so dead code cannot wait for a later PR).
3. `fix:` the header YOLO question onto stamped answers (`header-yolo-answer-may-cross-question-opening.md`), deleting
   `answer_current` / `cancel_current` and their tests in the same PR.
4. PRs moving the remaining prompts onto the mechanism: the sidebar rows (prompt split from operation exclusion), header
   Restart, trash, host Remove, provisioning confirmations, tunnel consent. `refactor:` where nothing observable
   changes, `fix:` with a `kind: none` fragment where only opening identity is gained, `fix:` with a user-facing
   fragment where a stale answer could act differently today. Template Delete's label fix in its own small `fix:` PR.
5. `fix:` declarative link covers (`launch-pill-can-cover-link-disclosure.md` among them), with the layer ladder, the
   refusal layer and SPEC_impl.md's list of covers updated.
6. `fix:` held input: the link gesture reset (`restored-link-press-misses-intervening-drag.md`) and quick-launch
   snapshot pairing (`refused-sequence-displaces-later-binding-snapshots.md`), separate PRs if they review better apart.
7. `docs:` SPEC_impl.md principle and the review-instruction rule (run `python3 scripts/check-agents-md.py`; keep word
   for word any rule the instruction files repeat, as root `AGENTS.md`, "Reading these instructions", says). The TODO.md
   entry removal rides on the last code PR, or on this one if it comes last.

Removals ride with the PR that migrates the last caller; there is no separate removal PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit tests
and the JS unit harness in `crates/farhelm-ui/js-tests` through the recorder as `.agents/validation.md` and
`.agents/narrow-tests.md` describe, and narrowly selected browser specs on Chromium and WebKit for each surface a PR
changes (for example `e2e/tests/terminal-link-clicks.spec.ts` for covers, `e2e/tests/terminal-reconnect.spec.ts` for the
gesture, `e2e/tests/clone.spec.ts` and the launcher specs for Replace with, `e2e/tests/sidebar.spec.ts` for row prompts
and menus, the trash and host specs for theirs).

## Fallbacks

Examples of departures that need the scope reassessment: moving the launcher counter or the approval timer onto the
prompt mechanism (decided against), a second prompt mechanism alongside the shared one, a generic event or form
framework, changing helm or supervisor code, a visible notice on stale answers (decided against), binding host
operations to installation identity (contradicts SPEC.md), or covers below the preview's layer.

Decisions to log: the inventory and each surface's classification, the mechanism's shape, every surface whose behavior
changed and why, any surface kept on its own mechanism and why it would make the end state worse to merge, the layer
order chosen for the refusal explanation, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-bind-gui-answers-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewers included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/bind-gui-answers/<nn>-<short-name>` (for
  example `plan/bind-gui-answers/01-guarded-precondition`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `refactor` for behavior-preserving restructuring, `test` for a change only to tests or their fixtures,
  `fix` (with a changelog fragment in the same commit) for any behavior change, `docs` for documentation and
  specification text only. A `fix` never shares a PR with refactoring.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high
effort, launched as a native sub-agent where your harness serves that model and otherwise through
`$scode-harness-shellout` (that skill owns the launch mechanics). The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range of this one PR;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, Decisions already made and the outline, and for a PR that fixes one of the
  six folded outcomes, that outcome's full `TRIAGE_OUTCOMES.md` entry. For a `refactor:` PR, also tell the reviewer the
  PR must not change behavior and to report any difference it finds as a finding.

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
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth. A surface whose
current behavior SPEC.md, SPEC_impl.md or a recorded decision explicitly endorses keeps it, as a named exception in the
block's API, logged as a DECISION; the header YOLO question is not such a case, since the maintainer decided to reverse
it.

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
passed its review gate, the six folded outcomes' Execution fields name their fixing PRs, and the last code PR removes
the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
