# Guarded by default wherever supported, and a banner when an agent switch escalates

Written against main at 93092359 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where the outcome below tells you to change them.

This plan has no dependency on another plan.

## The goal

In the New launcher, each agent type has a permission mode. Claude, Codex, Grok and Muse default to Guarded (a safety
mechanism can approve, refuse or escalate actions). OMP also offers Guarded (`--approval-mode write`: edits approved
automatically, other actions ask), but defaults to YOLO (no checks), the agent's stock behavior. A review finding showed
the consequence: explicitly choose Guarded for Claude, switch the agent to OMP, and the mode becomes YOLO, because the
switch clears a permission equal to the previous agent's default and OMP's default fills in. The maintainer decided
three things, all recorded under one heading in `TRIAGE_OUTCOMES.md` (read it in full first; its Decision quotes the
maintainer's words):

- `guarded-choice-becomes-omp-yolo.md` (`fix spec+code`)

1. A SPEC rule: for any agent that supports Guarded, Guarded is the default.
2. OMP's default becomes Guarded. OMP is the only agent today that offers Guarded while defaulting to YOLO; check that
   this is still true when you start, and apply the rule to any other such agent you find.
3. When switching agents in the launcher makes the permission mode more dangerous, a banner makes that very hard to miss
   and says why.

### Acceptance criteria

- **SPEC rule.** SPEC.md states that for any agent type that supports Guarded, Guarded is the default: the permission a
  launch gets when none is chosen. In the maintainer's words, this "does not override launcher remembering most recent
  choice being yolo that's fine, just talking about what the default is, not changing when the default is applied". The
  existing sentences that list which agents' omissions remain YOLO, OMP's description that calls `yolo` "preselected",
  and the sentence on replacing a harness (which clears "a YOLO permission that was the previous harness's default", no
  longer accurate since the guarded defaults landed) are updated to match. SPEC_impl.md is checked for contradicting
  text.
- **Docs site.** Pages under `website/` that state OMP's YOLO default are updated, following `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md`. The docs Overview page and its intro artwork are edited only on the maintainer's request
  (root `AGENTS.md`); if they need a change, say so in the report instead.
- **OMP defaults to Guarded** for new launches (`LaunchHarness::omitted_permission` in
  `crates/farhelm-proto/src/launch.rs`). Stored launches keep the meaning they were recorded with: `stored_permission`
  exists so an older stored command is never reinterpreted, and a Restart of an existing OMP session must not change its
  mode. The helm turns an omitted permission into an explicit one and compiles the flags; supervisors only run them, and
  OMP's Guarded mode (protocol 48) is not yet in any release, so this adds no upgrade risk. Confirm that still holds
  when you start, and keep SPEC.md "Upgrade compatibility and client scale" satisfied.
- **Switching from an explicit Guarded to OMP keeps Guarded.** This follows from the new default (the switch clears a
  permission equal to the source's default, then the destination's default fills in), and the finding's scenario gets a
  test.
- **Escalation banner.** When an agent switch in the launcher changes the permission mode to a more dangerous one, a red
  banner slides down across the top of the launcher saying, for example, "Permission changed: Guarded → YOLO, because Pi
  does not support Guarded." It names the old and new modes, the new agent, and the reason: the new agent does not
  support the previous mode (which is always the case: an agent that supports the previous mode keeps it). It has Undo,
  which restores the whole draft as it was before the switch (each switch already builds the full pre-switch draft, so
  this is a matter of keeping it), and a dismiss control. At the same time the permission mode control gets a thick red
  outline that fades after about two and a half seconds; the banner stays about eight seconds, and hovering or focusing
  it pauses that timer. With reduced motion (`prefers-reduced-motion`), the slide and fade are replaced by a static
  outline shown while the banner is up. The banner is announced to screen readers. Only an automatic change caused by
  switching from one agent to another shows it, never a mode the user picks themselves. Picking the first agent when
  none was selected is not a switch and shows no banner. The "before" mode is whatever the launcher showed for the
  previous agent, whether the user chose it, it was remembered, or it was that agent's default.
- **"More dangerous"** uses this order, safest first: chat and approve; then smart approve and Guarded; then YOLO,
  custom flags and Cursor's own-setting mode (SPEC.md and the existing YOLO check treat the last two as YOLO). A change
  to a later group is an escalation. If an existing helper or SPEC text already orders the modes differently, follow it
  and log the DECISION.
- **Tests.** A proto or UI unit test for the new OMP default and for the switch keeping Guarded; a browser test for the
  banner on a real escalation (for example Claude with Guarded switched to Pi) that checks its text, Undo and that it
  does not appear for a non-escalating switch.
- **Changelog fragment** (`kind: changed`): new OMP launches start in Guarded mode, and the launcher now says so loudly
  when switching agents makes the permission mode more dangerous. Mention that Farhelm does not detect OMP's approval
  prompts, so a Guarded OMP session waiting for approval shows as running or idle rather than waiting.
- The feedback file and its index entry are removed in the same PR, with the Execution field updated.

## Requirement sources

**The user's decisions (2026-10-14):** quoted in the ledger entry. The banner design was chosen from a mockup page
(option C, "Banner"); its behavior is written out above because the mockup lives outside the repository. The maintainer
was told that OMP gets no waiting-state recognition, so Guarded OMP sessions blocked on an approval prompt raise no
waiting alert, and gave the default-change direction after that. Review gate: a single fresh-context gpt-6.1-sol
reviewer at high effort, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** root `AGENTS.md` "Execute triage outcomes" (one PR for a `fix spec+code` outcome,
never split), "Harness-specific code" (per-harness behavior lives in the places
`crates/farhelm-supervisor/src/agent_kind/mod.rs` maps; no `kind == X` in shared code), and the changelog rule; SPEC.md
"Upgrade compatibility and client scale"; SPEC.md's launch composer text on permissions and remembered choices; the
existing incompatibility explanation in `crates/farhelm-ui/src/launch_composer.rs`.

**Planner proposals:** the danger order, the banner's exact wording, Undo restoring the previous agent, the changelog
kind, and the test choices.

## Outline

- `crates/farhelm-proto/src/launch.rs`: `omitted_permission` (OMP moves to the Guarded arm), `offers_permission` (the
  inventory of which agents offer Guarded), and the existing table-driven tests near them.
- `crates/farhelm-proto/src/launcher.rs`: the harness-switch reconciliation that clears a permission equal to the
  source's omitted mode; with the new default it should keep Guarded for OMP without further change, which a test
  confirms.
- `crates/farhelm-ui/src/list/create_form.rs`: the agent switch happens in several places there (find the calls into the
  harness-switch reconciliation); route them through one place that compares the mode before and after, keeps the
  pre-switch draft for Undo, and shows the banner and outline. The incompatibility explanation in
  `crates/farhelm-ui/src/launch_composer.rs` is the nearest existing pattern for the message.
- One PR (`feat:` or `fix:`, your call by the user-visible effect), with SPEC.md, tests, the fragment and the queue
  bookkeeping.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` if supervisor code changes,
the affected proto, helm and UI tests through the recorder, the launcher browser specs touching permissions plus the new
banner test on Chromium and WebKit through the recorder (builds first), the JS unit harness
(`cd crates/farhelm-ui/js-tests && node --test`) if CSS or asset JS it covers changes, the test-sleep checker,
`python3 releasing/check-changelog.py format`, and the docs website build if a page changes. Include the tests that
cover OMP launch flags and stored-command classification.

## Fallbacks

Examples of departures that need the scope reassessment: adding OMP approval-prompt recognition, changing when defaults
apply or how the last choice is remembered, showing the banner outside agent switches, or reinterpreting stored
launches.

Decisions to log: the danger order used, the banner's wording, which docs pages changed, the PR type.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-guarded-default-and-escalation-banner-log.md` in the parent directory of the checkout you run in, derived
as that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory
root `AGENTS.md` describes, not in the checkout.

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
  a single linear stack in the order given under Outline above. Each PR carries its outcome's agreed change, the removal
  of its feedback file and `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution update. Record
  the jj change ID and bookmark in that Execution field before creating the PR, then add the PR's URL (with `/changes`
  appended, per the user's PR-link rule) to the same change and push it again; no separate bookkeeping PR. Replace the
  Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/guarded-default-and-escalation-banner/<nn>-<short-name>` (for example
  `plan/guarded-default-and-escalation-banner/01-guarded-default`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, `feat` or `fix` by the user-visible effect, with a changelog
  fragment (`kind: changed`) in the same commit, per root `AGENTS.md`.
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

PRs in this stack that only remove feedback files and update the ledger, and change nothing else, may share one review
run that covers all of them together, instead of one run each.

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
