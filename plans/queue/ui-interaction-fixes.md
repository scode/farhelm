# Execute three triage outcomes: menu arrow keys, copy on click, and the Replace warning on a failed session

Written against main at 814a1129 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out three triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `arrow-navigation-retains-obsolete-position-item-disappears.md`
- `plain-clicks-overwrite-clipboard-old-selection.md`
- `replace-omits-conversation-loss-warning-failed-resume.md`

All three are small GUI behaviors that can cost the user something:

- A session's menu in the sidebar is keyboard-navigable. If the user has arrowed onto Replace and the agent ends while
  the menu is open, "Mark seen" disappears and the items below it move up, but the menu keeps stepping from the old
  position, so the next Down lands on Delete instead of Stop. Enter on an ended session with no tabs then deletes it
  without a confirmation. Found by reading the code, not reproduced in a browser.
- In the terminal, a program that uses legacy mouse reporting receives plain clicks. A selection made earlier by
  Shift-drag (Option-drag on a Mac) stays on screen, and a later plain click, for example clicking back into the
  terminal after copying something elsewhere, copies that old selection over the newer clipboard contents. The
  copy-on-select module's own header says a plain click without a drag must never clobber the clipboard. Found by
  reading the code, not reproduced interactively.
- When a restart that resumes a conversation fails to start the agent, the session shows an error but can still offer
  Resume. Replace's confirmation then says only that the agent never started, without warning that replacing discards
  the conversation Resume would have opened. Every other Replace wording carries that warning.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the three entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Tests cover a middle menu item disappearing before an arrow step, a plain click under mouse tracking not copying a
  retained selection (a `node --test` case), and the new Replace wording for an errored session.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-10):** a screening of the highest-priority review findings picked out the ones
that are clearly bugs with straightforward fixes, and the user decided: "given the criteria, let's just triage them all
for fixes unless you have a reason to disagree. every one should have a complexity gate so that if anything turns out to
be more complicated than your assessment, it bounces back to me." The criteria were: clearly a bug, no judgment needed;
and fixing it is straightforward, adding no significant complexity or scope. The decisions are recorded in root
`TRIAGE_OUTCOMES.md`, one entry per feedback file, each with its own complexity gate.

**The user's plan-time decisions (2026-10-10):** "group them together into a reasonable number of PRs that are
reasonably related or similar so we don't have 18 distinct plans with tiny amounts of work". Asked to confirm, the user
chose five plans with one PR each, so this plan's outcomes share one PR, overriding root `AGENTS.md`'s
one-PR-per-outcome rule for these outcomes only. When an item trips its complexity gate: drop it from the PR and ship
the rest (see Complexity gate below). Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which
`plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; the rule against tests that modify the test process's
own environment variables), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on main at 814a1129.

- Menu arrow keys: `crates/farhelm-ui/src/menu_panel.rs`, `menu_step_origin`, steps from `requested`, then `focused`,
  then the event's origin. In `crates/farhelm-ui/src/list/row.rs`, the row keeps `menu_requested` beside `menu_focus`;
  the effect that runs when the menu's item set changes re-aims `menu_focus` by action, and the comment above
  `menu_requested` explains why it deliberately does not reconcile `menu_requested`, reasoning that a stale index falls
  out of range. That reasoning is wrong when a middle item disappears: the old index stays in range and names a
  different action. In that same effect, clear `menu_requested` when the item set changes, so the reconciled focus takes
  over. Clearing is preferred to re-aiming: the existing reconcile function has no right answer when the requested item
  itself is withdrawn. Confirm the documented focus-burst race (the reason `requested` exists) stays fixed, and rewrite
  the comment. The row's effect cannot be driven headlessly, so the test is a pure-function scenario next to the
  existing menu-step tests in `menu_panel.rs` (a stale requested index after a middle item is removed must not decide
  the step once cleared); do not build a key-driving harness. `crates/farhelm-ui/src/list/hosts.rs` has the same pattern
  and comment for the host menu; that is not part of this finding, so leave it and name it as a possible follow-up in
  the report.
- Copy on click: `crates/farhelm-ui/assets/copy-on-select.js` decides with `copySelectionOnMouseUp` whether a finished
  gesture copies the current local selection; `terminal.js` calls it on mouse-up and already records, per press, whether
  mouse tracking was on and whether the press was forced (`trackingAtPress`, `forced`, used by
  `dragMayHaveCopiedNothing`). Skip the copy for a press made under mouse tracking without the forcing modifier, by
  passing `trackingAtPress` and `forced` into `copySelectionOnMouseUp`. The decision lives in that module so
  `node --test` exercises exactly the function `terminal.js` calls; do not put the check in `terminal.js`. Without
  tracking, a plain click already clears the selection, so nothing changes there; forced selections still copy. Add a
  case to `crates/farhelm-ui/js-tests/copy-on-select.test.js`.
- Replace warning: `crates/farhelm-ui/src/status.rs`, `replace_consequence_for_agent`, says only "the agent never
  started" for `SessionStatus::Error`. Change it to also warn that replacing discards any conversation the session could
  resume, for example "the agent never started, but replacing still discards any conversation the session could resume".
  The function sees only the status, so make the warning unconditional rather than plumbing the Resume offer in. The
  existing test that asserts the error wording does NOT mention discarding (message "an agent that never started never
  held a conversation to discard") encodes the bug; flip it, and keep the other arms' assertions.
- Changelog: `fix:` with `kind: fixed`, for someone running Farhelm: a session menu's arrow keys stay on the item you
  had selected when the menu changes underneath, a plain click in a terminal no longer re-copies an old selection, and
  Replace on a session whose restart failed warns that its conversation is discarded.

The complexity gate for these three is a small change each plus tests. A new focus-tracking mechanism, plumbing the
Resume offer into the confirmation, or changes to the vendored terminal library is a gate trip.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, clippy on `farhelm-ui` with `--all-targets`, a
focused nextest selection for the touched `farhelm-ui` tests through `scripts/record-test-run.py` (`--tmux none`), and
`cd crates/farhelm-ui/js-tests && node --test`. Run `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md` and apply `.agents/test-authoring.md`. `dprint check` on the changed Markdown. With both
behavioral fixes in pure functions, unit tests are enough and no Playwright run is needed; if a fix ends up outside a
pure function, decide per root `AGENTS.md`'s browser validation rules.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-ui-interaction-fixes-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/ui-interaction-fixes/01-ui-interactions`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, or a product or design decision), do not
grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index line,
and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you
found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape).
Name it among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback only when
every outcome in this plan was dropped, since then there is no PR to deliver. Code that turned out to be already fixed
on main is not a gate trip: record the outcome as `discard` with "already fixed" and the fixing commit, per root
`AGENTS.md`, and remove its queue item.

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

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that the focus-burst race `menu_requested` exists
for stays fixed, and that forced selections still copy. Where you disagree with a finding, decide on the merits and log
the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared formatter or protocol, edits outside the files the outline names beyond tests and bookkeeping), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular re-aiming versus clearing the requested menu position, where the copy-on-click
check lives, the final Replace wording, and whether a browser spec ran, the Conventional Commit type and changelog kind,
every outcome dropped by its complexity gate and why, and every review finding you decided not to follow. The user will
ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
